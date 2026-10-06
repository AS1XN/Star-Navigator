//! Orbit camera: drag to rotate, wheel to zoom, slow drift when idle.

use bevy::camera::Hdr;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use crate::GLOBE_RADIUS;
use crate::look::HoloEffect;

pub struct OrbitPlugin;

impl Plugin for OrbitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Orbit>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (orbit_input, apply_orbit).chain());
    }
}

const MIN_DIST: f32 = GLOBE_RADIUS * 1.5;
const MAX_DIST: f32 = GLOBE_RADIUS * 6.0;
const IDLE_SPIN_DELAY: f32 = 4.0;

#[derive(Resource)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    target_distance: f32,
    pub auto_spin: bool,
    idle: f32,
    /// Cursor travel since the button went down; small values count as a click.
    pub drag_px: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.3,
            distance: GLOBE_RADIUS * 2.3,
            target_distance: GLOBE_RADIUS * 2.3,
            auto_spin: true,
            idle: IDLE_SPIN_DELAY,
            drag_px: 0.0,
        }
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        HoloEffect::default(),
        Transform::default(),
    ));
}

fn orbit_input(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut orbit: ResMut<Orbit>,
) {
    let dt = time.delta_secs();
    if buttons.just_pressed(MouseButton::Left) {
        orbit.drag_px = 0.0;
    }
    let mut active = false;
    if buttons.pressed(MouseButton::Left) && motion.delta != Vec2::ZERO {
        // Scale rotation with zoom so close-up drags don't overshoot.
        let speed = 0.004 * (orbit.distance / (GLOBE_RADIUS * 3.0)).clamp(0.3, 1.5);
        orbit.yaw -= motion.delta.x * speed;
        orbit.pitch = (orbit.pitch - motion.delta.y * speed).clamp(-1.5, 1.5);
        orbit.drag_px += motion.delta.length();
        active = true;
    }
    if scroll.delta.y != 0.0 {
        let step = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y * 0.12,
            MouseScrollUnit::Pixel => scroll.delta.y * 0.0015,
        };
        orbit.target_distance = (orbit.target_distance * (1.0 - step)).clamp(MIN_DIST, MAX_DIST);
        active = true;
    }
    if keys.just_pressed(KeyCode::Space) {
        orbit.auto_spin = !orbit.auto_spin;
    }

    orbit.idle = if active { 0.0 } else { orbit.idle + dt };
    if orbit.auto_spin && orbit.idle > IDLE_SPIN_DELAY {
        let ramp = ((orbit.idle - IDLE_SPIN_DELAY) / 2.0).min(1.0);
        orbit.yaw += 0.04 * ramp * dt;
    }
    let t = 1.0 - (-10.0 * dt).exp();
    orbit.distance += (orbit.target_distance - orbit.distance) * t;
}

fn apply_orbit(orbit: Res<Orbit>, mut camera: Single<&mut Transform, With<Camera3d>>) {
    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    camera.translation = rotation * Vec3::new(0.0, 0.0, orbit.distance);
    camera.rotation = rotation;
}
