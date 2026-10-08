//! Hover and click selection of stars, plus the target reticles.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::{Orbit, over_ui};
use crate::data::Sky;
use crate::sky::SkyView;
use crate::{AppState, HOLO, field_position, globe_position, star_world, view_position};

pub struct PickingPlugin;

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selection>()
            .add_systems(OnEnter(AppState::Ready), build_targets)
            .add_systems(
                Update,
                (hover, click, draw_reticles).chain().run_if(in_state(AppState::Ready)),
            );
    }
}

#[derive(Resource, Default)]
pub struct Selection {
    pub hovered: Option<usize>,
    pub selected: Option<usize>,
}

struct Target {
    index: usize,
    globe: Vec3,
    field: Option<Vec3>,
    mag: f32,
}

/// Every star's globe and field positions, brightest first, plus stars added from
/// online lookups (checked regardless of the magnitude limit).
#[derive(Resource)]
pub struct Targets(Vec<Target>, Vec<Target>);

impl Targets {
    pub fn push_external(&mut self, index: usize, globe: Vec3, field: Option<Vec3>, mag: f32) {
        self.1.push(Target { index, globe, field, mag });
    }
}

const PICK_RADIUS_PX: f32 = 14.0;
/// Fingers are less precise than a mouse pointer.
pub const TAP_RADIUS_PX: f32 = 28.0;

fn build_targets(mut commands: Commands, sky: Res<Sky>) {
    let targets = sky
        .catalog
        .stars()
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.is_sun())
        .map(|(index, s)| Target {
            index,
            globe: globe_position(s),
            field: field_position(s),
            mag: s.mag,
        })
        .collect();
    commands.insert_resource(Targets(targets, Vec::new()));
}

/// The star nearest to `point` (logical px) within the pick radius, preferring
/// bright stars when several are close.
pub fn pick_at(
    point: Vec2,
    camera: &Camera,
    cam_tf: &GlobalTransform,
    targets: &Targets,
    view: &SkyView,
    radius_px: f32,
) -> Option<usize> {
    let eye = cam_tf.translation();
    let on_globe = view.unfold < 0.5 && view.chart < 0.5;
    let mut best: Option<(usize, f32)> = None;
    // Targets are sorted by magnitude, so stop at the display limit.
    let catalogued = targets.0.iter().take_while(|t| t.mag <= view.mag_limit);
    for t in catalogued.chain(&targets.1) {
        let Some(pos) = view_position(t.globe, t.field, view) else { continue };
        // On the globe only the near hemisphere is pickable.
        if on_globe && pos.normalize().dot(eye.normalize()) < 0.05 {
            continue;
        }
        let Ok(screen) = camera.world_to_viewport(cam_tf, pos) else { continue };
        let d = screen.distance(point);
        if d > radius_px {
            continue;
        }
        let score = d + t.mag * 1.5;
        if best.is_none_or(|(_, s)| score < s) {
            best = Some((t.index, score));
        }
    }
    best.map(|(i, _)| i)
}

fn hover(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    targets: Res<Targets>,
    view: Res<SkyView>,
    buttons: Res<ButtonInput<MouseButton>>,
    ui: Query<&Interaction>,
    mut selection: ResMut<Selection>,
) {
    let Some(cursor) = window.cursor_position().filter(|_| !over_ui(&ui)) else {
        selection.hovered = None;
        return;
    };
    if buttons.pressed(MouseButton::Left) {
        return;
    }
    let (camera, cam_tf) = *camera;
    selection.hovered = pick_at(cursor, camera, cam_tf, &targets, &view, PICK_RADIUS_PX);
}

fn click(
    buttons: Res<ButtonInput<MouseButton>>,
    orbit: Res<Orbit>,
    ui: Query<&Interaction>,
    mut pressed_on_ui: Local<bool>,
    mut sel: ResMut<Selection>,
) {
    // A press that began on a HUD button (which may be gone by the release) isn't a
    // click on the sky.
    if buttons.just_pressed(MouseButton::Left) {
        *pressed_on_ui = over_ui(&ui);
    }
    if buttons.just_released(MouseButton::Left) && orbit.drag_px < 4.0 && !*pressed_on_ui {
        sel.selected = sel.hovered;
    }
}

/// Camera-facing circle with four ticks, sized to stay constant on screen.
pub fn reticle(gizmos: &mut Gizmos, eye: Vec3, pos: Vec3, size: f32, color: Color, spin: f32) {
    let to_eye = (eye - pos).normalize();
    let r = eye.distance(pos) * size;
    let facing = Quat::from_rotation_arc(Vec3::Z, to_eye);
    gizmos.circle(Isometry3d::new(pos, facing), r, color);
    for k in 0..4 {
        let dir = facing
            * (Quat::from_rotation_z(spin + k as f32 * std::f32::consts::FRAC_PI_2) * Vec3::X);
        gizmos.line(pos + dir * r * 1.25, pos + dir * r * 1.9, color);
    }
}

fn draw_reticles(
    selection: Res<Selection>,
    sky: Res<Sky>,
    view: Res<SkyView>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    time: Res<Time>,
    mut gizmos: Gizmos,
) {
    let eye = camera.translation();
    let stars = sky.catalog.stars();
    let holo = LinearRgba::from(HOLO);
    if let Some(pos) = selection.selected.and_then(|i| star_world(&stars[i], &view)) {
        let pulse = 2.5 + (time.elapsed_secs() * 4.0).sin() * 0.8;
        let color = (holo * pulse).into();
        reticle(&mut gizmos, eye, pos, 0.022, color, time.elapsed_secs() * 0.6);
    }
    let hovered = selection.hovered.filter(|&h| Some(h) != selection.selected);
    if let Some(pos) = hovered.and_then(|i| star_world(&stars[i], &view)) {
        reticle(&mut gizmos, eye, pos, 0.014, (holo * 1.2).into(), 0.0);
    }
}
