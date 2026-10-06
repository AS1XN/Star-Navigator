//! Orbit camera: drag to rotate, wheel to zoom, slow drift when idle. Other systems
//! steer it by setting goals (focus point, distance, heading), which it eases toward.

use bevy::camera::Hdr;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use crate::look::HoloEffect;
use crate::{GLOBE_RADIUS, hotkeys_enabled};

pub struct OrbitPlugin;

impl Plugin for OrbitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Orbit>().add_systems(Startup, spawn_camera).add_systems(
            Update,
            (spin_toggle.run_if(hotkeys_enabled), orbit_input, apply_orbit).chain(),
        );
    }
}

pub const GLOBE_DIST: f32 = GLOBE_RADIUS * 2.3;
const IDLE_SPIN_DELAY: f32 = 4.0;

#[derive(Resource)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub focus: Vec3,
    pub auto_spin: bool,
    /// Cursor travel since the button went down; small values count as a click.
    pub drag_px: f32,
    pub goal: OrbitGoal,
    idle: f32,
}

/// Where the camera is heading. `heading` is cleared as soon as the user drags.
pub struct OrbitGoal {
    pub focus: Vec3,
    pub distance: f32,
    pub heading: Option<(f32, f32)>,
    pub min_distance: f32,
    pub max_distance: f32,
}

impl OrbitGoal {
    pub fn globe() -> Self {
        Self {
            focus: Vec3::ZERO,
            distance: GLOBE_DIST,
            heading: None,
            min_distance: GLOBE_RADIUS * 1.5,
            max_distance: GLOBE_RADIUS * 6.0,
        }
    }
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.3,
            distance: GLOBE_DIST,
            focus: Vec3::ZERO,
            auto_spin: true,
            drag_px: 0.0,
            goal: OrbitGoal::globe(),
            idle: IDLE_SPIN_DELAY,
        }
    }
}

/// Yaw and pitch that put the camera on the `dir` side of the focus, looking back at it.
pub fn heading_toward(dir: Vec3) -> (f32, f32) {
    let d = dir.normalize();
    (d.x.atan2(d.z), -d.y.clamp(-1.0, 1.0).asin())
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        HoloEffect::default(),
        // Close-up views of neighbouring stars need a short near plane.
        Projection::Perspective(PerspectiveProjection { near: 0.002, ..default() }),
        Transform::default(),
    ));
}

fn spin_toggle(keys: Res<ButtonInput<KeyCode>>, mut orbit: ResMut<Orbit>) {
    if keys.just_pressed(KeyCode::Space) {
        orbit.auto_spin = !orbit.auto_spin;
    }
}

fn orbit_input(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
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
        let speed = 0.004 * (orbit.distance / GLOBE_DIST).clamp(0.3, 1.5);
        orbit.yaw -= motion.delta.x * speed;
        orbit.pitch = (orbit.pitch - motion.delta.y * speed).clamp(-1.5, 1.5);
        orbit.drag_px += motion.delta.length();
        orbit.goal.heading = None;
        active = true;
    }
    if scroll.delta.y != 0.0 {
        let step = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y * 0.12,
            MouseScrollUnit::Pixel => scroll.delta.y * 0.0015,
        };
        let g = &mut orbit.goal;
        g.distance = (g.distance * (1.0 - step)).clamp(g.min_distance, g.max_distance);
        active = true;
    }

    orbit.idle = if active { 0.0 } else { orbit.idle + dt };
    if orbit.auto_spin && orbit.idle > IDLE_SPIN_DELAY && orbit.goal.heading.is_none() {
        let ramp = ((orbit.idle - IDLE_SPIN_DELAY) / 2.0).min(1.0);
        orbit.yaw += 0.04 * ramp * dt;
    }

    // Ease toward the goals. Distance eases in log space so long zooms feel even.
    let t = 1.0 - (-2.5 * dt).exp();
    orbit.focus = orbit.focus.lerp(orbit.goal.focus, t);
    let (d, gd) = (orbit.distance.ln(), orbit.goal.distance.ln());
    orbit.distance = (d + (gd - d) * t).exp();
    if let Some((yaw, pitch)) = orbit.goal.heading {
        let dy = (yaw - orbit.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        orbit.yaw += dy * t;
        orbit.pitch += (pitch - orbit.pitch) * t;
    }
}

fn apply_orbit(orbit: Res<Orbit>, mut camera: Single<&mut Transform, With<Camera3d>>) {
    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    camera.translation = orbit.focus + rotation * Vec3::new(0.0, 0.0, orbit.distance);
    camera.rotation = rotation;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_points_camera_at_direction() {
        for dir in [Vec3::X, Vec3::NEG_Z, Vec3::new(0.3, 0.8, -0.5), Vec3::new(-1.0, -0.4, 0.2)] {
            let (yaw, pitch) = heading_toward(dir);
            let back = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::Z;
            assert!(back.distance(dir.normalize()) < 1e-4, "{dir} -> {back}");
        }
    }
}
