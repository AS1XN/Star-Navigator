//! The chart view: the sky flattened onto a round table, the way a briefing room
//! would show it. V switches between globe and chart, N centres the chart on the
//! other celestial pole. The stars themselves are moved by the star shader (see
//! `chart_position`); this module animates the switch and draws the table: rim
//! with hour ticks, declination rings, hour spokes and a sector wedge on the
//! selected star.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::camera::{GLOBE_DIST, Orbit};
use crate::data::Sky;
use crate::locate::Locate;
use crate::picking::Selection;
use crate::sky::SkyView;
use crate::{AppState, CHART_RADIUS, HOLO, hotkeys_enabled};

pub struct ChartPlugin;

impl Plugin for ChartPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (chart_keys.run_if(hotkeys_enabled), animate, draw_table)
                .chain()
                .run_if(in_state(AppState::Ready)),
        );
    }
}

/// Seconds for the globe to flatten onto the table (or lift back).
const FLATTEN_SECS: f32 = 1.8;
/// Camera tilt while looking down at the table, and the usual globe tilt.
const TABLE_PITCH: f32 = -1.12;
const GLOBE_PITCH: f32 = -0.3;

fn chart_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut view: ResMut<SkyView>,
    mut locate: ResMut<Locate>,
    mut orbit: ResMut<Orbit>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        toggle_chart(&mut view, &mut locate, &mut orbit);
    }
    if keys.just_pressed(KeyCode::KeyN) {
        view.south = !view.south;
    }
}

/// Switches globe <-> chart, leaving the 3D field first if a star is located.
pub fn toggle_chart(view: &mut SkyView, locate: &mut Locate, orbit: &mut Orbit) {
    locate.back_to_chart(orbit);
    view.chart_on = !view.chart_on;
    // The table is wider than the globe, so step back to take it all in.
    orbit.goal.distance = if view.chart_on { CHART_RADIUS * 3.1 } else { GLOBE_DIST };
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn animate(
    time: Res<Time>,
    mut view: ResMut<SkyView>,
    mut orbit: ResMut<Orbit>,
    mut progress: Local<f32>,
) {
    let goal = if view.chart_on { 1.0 } else { 0.0 };
    if *progress == goal {
        return;
    }
    let step = time.delta_secs() / FLATTEN_SECS;
    *progress =
        if goal > *progress { (*progress + step).min(1.0) } else { (*progress - step).max(0.0) };
    let eased = smoothstep(*progress);
    view.chart = eased;
    // Tilt the camera down over the table as it flattens, and back up after.
    orbit.pitch = GLOBE_PITCH + (TABLE_PITCH - GLOBE_PITCH) * eased;
}

/// Points around a circle of radius `r` on the table.
fn ring(r: f32, steps: usize) -> impl Iterator<Item = Vec3> {
    (0..=steps).map(move |i| {
        let a = i as f32 / steps as f32 * TAU;
        Vec3::new(r * a.cos(), 0.0, -r * a.sin())
    })
}

/// Table radius for a declination (degrees), for the chosen pole.
fn dec_radius(dec: f32, south: bool) -> f32 {
    let pole = if south { -1.0 } else { 1.0 };
    (90.0 - dec * pole) / 180.0 * CHART_RADIUS
}

fn at(r: f32, ra: f32) -> Vec3 {
    Vec3::new(r * ra.cos(), 0.0, -r * ra.sin())
}

fn draw_table(view: Res<SkyView>, selection: Res<Selection>, sky: Res<Sky>, mut gizmos: Gizmos) {
    let a = view.chart;
    if a < 0.02 {
        return;
    }
    let holo = LinearRgba::from(HOLO);
    let line = |s: f32| -> Color { (holo * s * a).into() };
    let r = CHART_RADIUS;

    // Double rim with ticks every 5 degrees of RA, long ones every hour.
    gizmos.linestrip(ring(r * 1.03, 180), line(0.9));
    gizmos.linestrip(ring(r * 1.09, 180), line(0.6));
    for i in 0..72 {
        let ra = i as f32 / 72.0 * TAU;
        let outer = if i % 3 == 0 { 1.09 } else { 1.055 };
        gizmos.line(at(r * 1.03, ra), at(r * outer, ra), line(0.8));
    }

    if view.grid {
        for dec in [60.0, 30.0, -30.0, -60.0] {
            gizmos.linestrip(ring(dec_radius(dec, view.south), 144), line(0.22));
        }
        // Hour spokes every 2h, stopping short of the pole.
        for h in 0..12 {
            let ra = h as f32 / 12.0 * TAU;
            gizmos.line(at(dec_radius(80.0, view.south), ra), at(r, ra), line(0.22));
        }
        // Pole crosshair.
        let c = r * 0.03;
        gizmos.line(Vec3::new(-c, 0.0, 0.0), Vec3::new(c, 0.0, 0.0), line(0.6));
        gizmos.line(Vec3::new(0.0, 0.0, -c), Vec3::new(0.0, 0.0, c), line(0.6));
    }

    // Sector wedge on the selected star: two radial edges an hour either side of
    // it, the rim arc between them, and a range arc through the star.
    let Some(star) = selection.selected.map(|i| &sky.catalog.stars()[i]) else { return };
    if view.chart < 0.5 {
        return;
    }
    let ra = (star.ra / 24.0) * TAU;
    let half = PI / 12.0;
    let star_r = dec_radius(star.dec, view.south);
    let wedge = line(1.6);
    for edge in [ra - half, ra + half] {
        gizmos.line(Vec3::ZERO, at(r * 1.03, edge), wedge);
    }
    let arc =
        |radius: f32| (0..=24).map(move |i| at(radius, ra - half + i as f32 / 24.0 * 2.0 * half));
    gizmos.linestrip(arc(r * 1.03), wedge);
    gizmos.linestrip(arc(star_r), line(1.0));
    // A short bearing tick at the star's hour on the rim.
    gizmos.line(at(r * 1.09, ra), at(r * 1.16, ra), wedge);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GLOBE_RADIUS, chart_position};
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn rings_match_the_star_projection() {
        // A star on the celestial equator lands on the equator ring, at half radius.
        let p = chart_position(Vec3::new(GLOBE_RADIUS, 0.0, 0.0), false);
        assert!((p.length() - dec_radius(0.0, false)).abs() < 1e-4);
        assert!((dec_radius(0.0, false) - CHART_RADIUS * 0.5).abs() < 1e-4);
        // The north pole sits at the centre of a north chart, the rim of a south one.
        let pole = Vec3::new(0.0, GLOBE_RADIUS, 0.0);
        assert!(chart_position(pole, false).length() < 1e-3);
        assert!((chart_position(pole, true).length() - CHART_RADIUS).abs() < 1e-3);
        // +60 dec on a north chart sits on the 60-degree ring.
        let d = 60f32.to_radians();
        let p = chart_position(Vec3::new(d.cos(), d.sin(), 0.0) * GLOBE_RADIUS, false);
        assert!((p.length() - dec_radius(60.0, false)).abs() < 1e-3);
    }

    #[test]
    fn hour_angles_agree() {
        // RA 6h (90 degrees) on the globe and on the rim ticks point the same way.
        let ra = FRAC_PI_2;
        let globe = Vec3::new(ra.cos(), 0.0, -ra.sin()) * GLOBE_RADIUS;
        let on_chart = chart_position(globe, false).normalize();
        assert!(on_chart.distance(at(1.0, ra)) < 1e-4);
    }
}
