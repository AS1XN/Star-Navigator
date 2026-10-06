//! Hover and click selection of stars, plus the target reticles.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::Orbit;
use crate::data::Sky;
use crate::sky::SkyView;
use crate::{AppState, HOLO, field_position, globe_position, star_world};

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

/// Every star's globe and field positions, brightest first.
#[derive(Resource)]
struct Targets(Vec<Target>);

const PICK_RADIUS_PX: f32 = 14.0;

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
    commands.insert_resource(Targets(targets));
}

fn hover(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    targets: Res<Targets>,
    view: Res<SkyView>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut selection: ResMut<Selection>,
) {
    let Some(cursor) = window.cursor_position() else {
        selection.hovered = None;
        return;
    };
    if buttons.pressed(MouseButton::Left) {
        return;
    }
    let (camera, cam_tf) = *camera;
    let eye = cam_tf.translation();
    let on_globe = view.unfold < 0.5;

    let mut best: Option<(usize, f32)> = None;
    // Targets are sorted by magnitude, so stop at the display limit.
    for t in targets.0.iter().take_while(|t| t.mag <= view.mag_limit) {
        let pos = match (on_globe, t.field) {
            (true, _) => t.globe.lerp(t.field.unwrap_or(t.globe * 12.0), view.unfold),
            (false, Some(f)) => t.globe.lerp(f, view.unfold),
            (false, None) => continue,
        };
        // On the globe only the near hemisphere is pickable.
        if on_globe && pos.normalize().dot(eye.normalize()) < 0.05 {
            continue;
        }
        let Ok(screen) = camera.world_to_viewport(cam_tf, pos) else { continue };
        let d = screen.distance(cursor);
        if d > PICK_RADIUS_PX {
            continue;
        }
        // Prefer bright stars when several are under the cursor.
        let score = d + t.mag * 1.5;
        if best.is_none_or(|(_, s)| score < s) {
            best = Some((t.index, score));
        }
    }
    selection.hovered = best.map(|(i, _)| i);
}

fn click(buttons: Res<ButtonInput<MouseButton>>, orbit: Res<Orbit>, mut sel: ResMut<Selection>) {
    if buttons.just_released(MouseButton::Left) && orbit.drag_px < 4.0 {
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
    if let Some(pos) = selection.selected.and_then(|i| star_world(&stars[i], view.unfold)) {
        let pulse = 2.5 + (time.elapsed_secs() * 4.0).sin() * 0.8;
        let color = (holo * pulse).into();
        reticle(&mut gizmos, eye, pos, 0.022, color, time.elapsed_secs() * 0.6);
    }
    let hovered = selection.hovered.filter(|&h| Some(h) != selection.selected);
    if let Some(pos) = hovered.and_then(|i| star_world(&stars[i], view.unfold)) {
        reticle(&mut gizmos, eye, pos, 0.014, (holo * 1.2).into(), 0.0);
    }
}
