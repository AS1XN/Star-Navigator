//! Hover and click selection of stars, plus the target reticles.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::Orbit;
use crate::data::Sky;
use crate::sky::SkyView;
use crate::{AppState, GLOBE_RADIUS, HOLO, sky_to_world};

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

/// Star index, world position and magnitude, brightest first.
#[derive(Resource)]
struct Targets(Vec<(usize, Vec3, f32)>);

const PICK_RADIUS_PX: f32 = 14.0;

fn build_targets(mut commands: Commands, sky: Res<Sky>) {
    let targets = sky
        .catalog
        .stars()
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.is_sun())
        .map(|(i, s)| (i, sky_to_world(s.direction()) * GLOBE_RADIUS, s.mag))
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
    let eye = cam_tf.translation().normalize();

    let mut best: Option<(usize, f32)> = None;
    // Targets are sorted by magnitude, so stop at the display limit.
    for &(index, pos, mag) in targets.0.iter().take_while(|t| t.2 <= view.mag_limit) {
        if pos.normalize().dot(eye) < 0.05 {
            continue;
        }
        let Ok(screen) = camera.world_to_viewport(cam_tf, pos) else { continue };
        let d = screen.distance(cursor);
        if d > PICK_RADIUS_PX {
            continue;
        }
        // Prefer bright stars when several are under the cursor.
        let score = d + mag * 1.5;
        if best.is_none_or(|(_, s)| score < s) {
            best = Some((index, score));
        }
    }
    selection.hovered = best.map(|(i, _)| i);
}

fn click(buttons: Res<ButtonInput<MouseButton>>, orbit: Res<Orbit>, mut sel: ResMut<Selection>) {
    if buttons.just_released(MouseButton::Left) && orbit.drag_px < 4.0 {
        sel.selected = sel.hovered;
    }
}

fn draw_reticles(
    selection: Res<Selection>,
    sky: Res<Sky>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    time: Res<Time>,
    mut gizmos: Gizmos,
) {
    let eye = camera.translation();
    let reticle = |gizmos: &mut Gizmos, index: usize, size: f32, color: Color, spin: f32| {
        let pos = sky_to_world(sky.catalog.stars()[index].direction()) * GLOBE_RADIUS;
        let to_eye = (eye - pos).normalize();
        let r = eye.distance(pos) * size;
        let facing = Quat::from_rotation_arc(Vec3::Z, to_eye);
        gizmos.circle(Isometry3d::new(pos, facing), r, color);
        for k in 0..4 {
            let dir = facing
                * (Quat::from_rotation_z(spin + k as f32 * std::f32::consts::FRAC_PI_2) * Vec3::X);
            gizmos.line(pos + dir * r * 1.25, pos + dir * r * 1.9, color);
        }
    };

    let holo = LinearRgba::from(HOLO);
    if let Some(i) = selection.selected {
        let pulse = 2.5 + (time.elapsed_secs() * 4.0).sin() * 0.8;
        reticle(&mut gizmos, i, 0.022, (holo * pulse).into(), time.elapsed_secs() * 0.6);
    }
    if let Some(i) = selection.hovered.filter(|&h| Some(h) != selection.selected) {
        reticle(&mut gizmos, i, 0.014, (holo * 1.2).into(), 0.0);
    }
}
