//! The selected star's dossier: data plate, a rotating wireframe model sized by its
//! estimated radius (against a dashed Sol reference), projection lines to the star,
//! and name tags for its nearest neighbours in the 3D field.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use catalog::{LY_PER_PC, Star};

use crate::camera::apply_orbit;
use crate::data::Sky;
use crate::look::{Tinted, Tuner};
use crate::picking::Selection;
use crate::sky::SkyView;
use crate::{AppState, HOLO, Typing, star_world};

pub struct DossierPlugin;

impl Plugin for DossierPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dossier>().add_systems(Startup, spawn_ui).add_systems(
            Update,
            (refresh, layout_plate, draw_model.after(apply_orbit), neighbour_tags)
                .chain()
                .run_if(in_state(AppState::Ready)),
        );
    }
}

const NEIGHBOURS: usize = 8;
/// On-screen radius of the Sol reference circle, in logical pixels.
const SOL_PX: f32 = 34.0;

const PLATE_LEFT: f32 = 14.0;
const PLATE_TOP: f32 = 12.0;

#[derive(Resource, Default)]
struct Dossier {
    star: Option<usize>,
    neighbours: Vec<(usize, f32)>,
    radius: Option<f32>,
    /// False while the FIND box or tuning panel needs the screen.
    shown: bool,
    compact: bool,
    /// Screen areas (logical px) the plate and model occupy, kept clear of tags.
    plate_rect: Option<Rect>,
    model_circle: Option<(Vec2, f32)>,
}

impl Dossier {
    fn covers(&self, p: Vec2) -> bool {
        self.plate_rect.is_some_and(|r| r.inflate(6.0).contains(p))
            || self.model_circle.is_some_and(|(c, r)| c.distance(p) < r + 12.0)
    }
}

/// Hides the plate (and with it the model) while another panel is up, and tracks
/// where it sits so neighbour tags can stay out of the way.
fn layout_plate(
    typing: Res<Typing>,
    tuner: Res<Tuner>,
    scale: Res<UiScale>,
    mut dossier: ResMut<Dossier>,
    mut plate: Single<(&Text, &ComputedNode, &mut Visibility), With<Plate>>,
) {
    let (text, node, vis) = &mut *plate;
    dossier.shown = dossier.star.is_some() && !typing.0 && !tuner.open;
    let visible = dossier.shown && !text.0.is_empty();
    vis.set_if_neq(if visible { Visibility::Inherited } else { Visibility::Hidden });
    dossier.plate_rect = visible.then(|| {
        let min = Vec2::new(PLATE_LEFT, PLATE_TOP) * scale.0;
        Rect::from_corners(min, min + node.size() * node.inverse_scale_factor())
    });
}

#[derive(Component)]
struct Plate;

#[derive(Component)]
struct NeighbourTag(usize);

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        Plate,
        Text::new(""),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(HOLO),
        Tinted(1.0),
        // Above the tags and hover readout, below the FIND box and tuning panel.
        ZIndex(10),
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            left: px(PLATE_LEFT),
            top: px(PLATE_TOP),
            padding: UiRect::axes(px(10), px(6)),
            ..default()
        },
    ));
    // One tag per neighbour plus one for Sol.
    for slot in 0..=NEIGHBOURS {
        commands.spawn((
            NeighbourTag(slot),
            Text::new(""),
            TextFont { font_size: FontSize::Px(12.0), ..default() },
            TextColor(HOLO),
            Tinted(0.75),
            Node { position_type: PositionType::Absolute, ..default() },
        ));
    }
}

/// Below this width (logical px) the plate shows only the essentials and the model
/// is left out, so a phone screen isn't covered in text.
const COMPACT_WIDTH: f32 = 700.0;

fn refresh(
    selection: Res<Selection>,
    sky: Res<Sky>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut dossier: ResMut<Dossier>,
    mut plate: Single<&mut Text, With<Plate>>,
) {
    let compact = window.width() < COMPACT_WIDTH;
    if dossier.star == selection.selected && dossier.compact == compact {
        return;
    }
    if dossier.star != selection.selected {
        dossier.star = selection.selected;
        if let Some(index) = selection.selected {
            dossier.neighbours = sky.catalog.nearest(index, NEIGHBOURS);
            dossier.radius = sky.catalog.stars()[index].luminosity_and_radius().map(|(_, r)| r);
        } else {
            dossier.neighbours.clear();
            dossier.radius = None;
        }
    }
    dossier.compact = compact;
    plate.0 = match selection.selected {
        Some(index) if compact => describe_compact(&sky.catalog.stars()[index]),
        Some(index) => describe(&sky.catalog.stars()[index], &dossier.neighbours, &sky),
        None => String::new(),
    };
}

fn describe_compact(s: &Star) -> String {
    let mut lines = vec![format!("> {}", s.display_name().to_uppercase())];
    if let Some(sp) = s.spectral_type() {
        lines.push(format!("{}  {}", s.spectral, sp.description()));
    }
    if let Some(ly) = s.dist_ly() {
        lines.push(format!("{} LY FROM SOL", ly_text(ly, 1)));
    }
    if let Some((l, r)) = s.luminosity_and_radius() {
        lines.push(format!("{} x SOL BRIGHT  /  {} x SOL WIDE", sig(l), sig(r)));
    }
    lines.join("\n")
}

fn describe(s: &Star, neighbours: &[(usize, f32)], sky: &Sky) -> String {
    let mut lines = vec![format!("> {}", s.display_name().to_uppercase())];
    let others: Vec<String> = s.designations().into_iter().skip(1).collect();
    if !others.is_empty() {
        lines.push(others.join("  /  "));
    }
    lines.push(String::new());
    if let Some(c) = s.constellation() {
        lines.push(format!("CONSTELLATION  {}", c.name.to_uppercase()));
    }
    let ra = s.ra;
    lines.push(format!(
        "RA  {:02}h {:02}m {:04.1}s   DEC  {:+.3} DEG",
        ra.trunc() as u32,
        (ra.fract() * 60.0).trunc() as u32,
        (ra * 3600.0) % 60.0,
        s.dec
    ));
    lines.push(match (s.dist_ly(), s.dist_pc) {
        (Some(ly), Some(pc)) => format!("DISTANCE  {ly:.2} LY  ({pc:.2} PC)"),
        _ => "DISTANCE  UNKNOWN".into(),
    });
    lines.push(format!("MAGNITUDE  {:+.2} APP  /  {:+.2} ABS", s.mag, s.absmag));

    if let Some(sp) = s.spectral_type() {
        lines.push(String::new());
        lines.push(format!("CLASS  {}  //  {}", s.spectral, sp.description()));
        lines.push(format!(
            "TEMPERATURE  ~{} K",
            group_thousands(sp.temperature_k().round() as u32)
        ));
        if let Some((l, r)) = s.luminosity_and_radius() {
            lines.push(format!("LUMINOSITY  {} SOL   RADIUS  {} SOL", sig(l), sig(r)));
        }
    } else if !s.spectral.is_empty() {
        lines.push(format!("CLASS  {}", s.spectral));
    }

    if !neighbours.is_empty() {
        lines.push(String::new());
        lines.push("NEAREST STARS".into());
        for &(i, pc) in neighbours.iter().take(3) {
            let name = sky.catalog.stars()[i].display_name().to_uppercase();
            lines.push(format!("  {name:<22} {:>7} LY", ly_text(pc * LY_PER_PC, 2)));
        }
    }
    lines.join("\n")
}

/// Distance with `decimals` places, or "<0.01" style when it would round to zero.
fn ly_text(ly: f32, decimals: usize) -> String {
    let floor = 10f32.powi(-(decimals as i32));
    if ly < floor { format!("<{floor:.decimals$}") } else { format!("{ly:.decimals$}") }
}

fn group_thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Three significant figures without scientific notation for everyday ranges.
fn sig(v: f32) -> String {
    match v {
        v if v >= 1000.0 => group_thousands(v.round() as u32),
        v if v >= 100.0 => format!("{v:.0}"),
        v if v >= 10.0 => format!("{v:.1}"),
        v if v >= 0.01 => format!("{v:.2}"),
        v => format!("{v:.4}"),
    }
}

/// Tint by spectral class, before the hologram post-process mostly flattens it.
fn class_color(star: &Star) -> LinearRgba {
    let c = match star.spectral_type().map(|s| s.class) {
        Some('O' | 'B' | 'W') => Color::srgb(0.65, 0.78, 1.0),
        Some('A' | 'D') => Color::srgb(0.85, 0.9, 1.0),
        Some('F') => Color::srgb(1.0, 0.97, 0.88),
        Some('G') => Color::srgb(1.0, 0.92, 0.7),
        Some('K') => Color::srgb(1.0, 0.78, 0.55),
        Some('M' | 'C' | 'S') => Color::srgb(1.0, 0.6, 0.45),
        _ => HOLO,
    };
    LinearRgba::from(c)
}

/// Draws the model just in front of the camera at a fixed spot on screen, below
/// the data plate, so it reads as part of the HUD but gets the hologram treatment.
#[allow(clippy::too_many_arguments)]
fn draw_model(
    mut dossier: ResMut<Dossier>,
    sky: Res<Sky>,
    view: Res<SkyView>,
    time: Res<Time>,
    scale: Res<UiScale>,
    camera: Single<(&Camera, &Transform, &Projection)>,
    mut gizmos: Gizmos,
) {
    dossier.model_circle = None;
    if dossier.compact {
        return;
    }
    let (Some(index), Some(plate)) = (dossier.star, dossier.plate_rect) else { return };
    let (camera, cam_tf, projection) = *camera;
    let Some(viewport) = camera.logical_viewport_size() else { return };
    let Projection::Perspective(perspective) = projection else { return };

    let r_px =
        dossier.radius.map_or(SOL_PX, |r| (SOL_PX * r.powf(0.25)).clamp(8.0, 105.0)) * scale.0;
    let max_r = 105.0 * scale.0;
    let center_px =
        Vec2::new(plate.min.x + 10.0 * scale.0 + max_r, plate.max.y + 24.0 * scale.0 + max_r);
    if center_px.y + r_px > viewport.y - 60.0 * scale.0 {
        return; // No room on this screen.
    }
    dossier.model_circle = Some((center_px, r_px));

    // Every point is laid out in screen space and placed a short way in front of
    // the camera with the camera's own projection, so the model stays a true circle
    // wherever it sits on screen. (Camera::viewport_to_world goes through the
    // infinite far plane and loses precision, which made it drift and jitter.)
    // This runs after the orbit update and uses the camera's current Transform, so
    // it doesn't lag a frame behind while the view rotates.
    let depth = 0.05;
    let half_h = (perspective.fov * 0.5).tan() * depth;
    let half_w = half_h * viewport.x / viewport.y;
    let at = |p: Vec2| -> Option<Vec3> {
        let ndc = Vec2::new(p.x / viewport.x * 2.0 - 1.0, 1.0 - p.y / viewport.y * 2.0);
        Some(cam_tf.transform_point(Vec3::new(ndc.x * half_w, ndc.y * half_h, -depth)))
    };
    let cam_forward = cam_tf.forward();
    let cam_pos = cam_tf.translation;
    let star = &sky.catalog.stars()[index];
    let color = class_color(star) * 1.6;
    let spin = Quat::from_rotation_x(0.4) * Quat::from_rotation_y(time.elapsed_secs() * 0.5);

    // A point on the unit sphere -> world position plus a front/back brightness.
    let project = |p: Vec3| -> Option<(Vec3, f32)> {
        let q = spin * p;
        let world = at(center_px + Vec2::new(q.x, -q.y) * r_px)?;
        Some((world, 0.35 + 0.65 * (q.z * 0.5 + 0.5)))
    };
    let mut strip = |points: Vec<Vec3>, strength: f32| {
        let pts: Vec<(Vec3, Color)> = points
            .into_iter()
            .filter_map(&project)
            .map(|(w, f)| (w, (color * strength * f).into()))
            .collect();
        gizmos.linestrip_gradient(pts);
    };

    const STEPS: usize = 48;
    let ring = |f: &dyn Fn(f32) -> Vec3| -> Vec<Vec3> {
        (0..=STEPS).map(|i| f(i as f32 / STEPS as f32 * TAU)).collect()
    };
    for k in 0..6 {
        let lon = k as f32 * PI / 6.0;
        strip(ring(&|a| Vec3::new(a.cos() * lon.cos(), a.sin(), a.cos() * lon.sin())), 0.8);
    }
    for lat in [-60.0f32, -30.0, 0.0, 30.0, 60.0] {
        let (s, c) = lat.to_radians().sin_cos();
        strip(
            ring(&|a| Vec3::new(a.cos() * c, s, a.sin() * c)),
            if lat == 0.0 { 1.2 } else { 0.6 },
        );
    }

    // Dashed Sol reference.
    if dossier.radius.is_some() {
        let sol = SOL_PX * scale.0;
        for s in (0..STEPS).step_by(2) {
            let a0 = s as f32 / STEPS as f32 * TAU;
            let a1 = (s + 1) as f32 / STEPS as f32 * TAU;
            let p = |a: f32| at(center_px + Vec2::new(a.cos(), a.sin()) * sol);
            if let (Some(p0), Some(p1)) = (p(a0), p(a1)) {
                gizmos.line(p0, p1, LinearRgba::from(HOLO) * 0.5);
            }
        }
    }

    // Projection lines from the model's rim to the star itself.
    let ahead = |t: &Vec3| (*t - cam_pos).dot(*cam_forward) > 0.0;
    if let Some(target) = star_world(star, view.unfold).filter(ahead) {
        for dir in [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y] {
            if let Some(rim) = at(center_px + dir * r_px) {
                gizmos.line(rim, target, LinearRgba::from(HOLO) * 0.25);
            }
        }
    }
}

/// Name tags for the selected star's neighbours (and Sol) once the field is open.
fn neighbour_tags(
    dossier: Res<Dossier>,
    sky: Res<Sky>,
    view: Res<SkyView>,
    scale: Res<UiScale>,
    camera: Single<(&Camera, &Transform)>,
    mut tags: Query<(&NeighbourTag, &mut Text, &mut Node)>,
) {
    let (camera, transform) = *camera;
    // Current-frame camera, matching the model (GlobalTransform lags a frame here).
    let cam_tf = &GlobalTransform::from(*transform);
    let show = view.unfold > 0.95 && dossier.shown;
    let stars = sky.catalog.stars();

    // Neighbours first; Sol gets the last slot if it isn't already a neighbour.
    let mut entries: Vec<(usize, Option<f32>)> =
        dossier.neighbours.iter().map(|&(i, pc)| (i, Some(pc))).collect();
    if !entries.iter().any(|(i, _)| *i == 0) {
        entries.push((0, None));
    }

    // Place tags in order, skipping any that would sit on the target or an earlier
    // tag (binary companions are often on top of each other).
    let mut taken: Vec<Vec2> = dossier
        .star
        .and_then(|i| star_world(&stars[i], view.unfold))
        .and_then(|p| camera.world_to_viewport(cam_tf, p).ok())
        .into_iter()
        .collect();
    let mut placed: Vec<Option<(Vec2, String)>> = Vec::new();
    for &(i, pc) in entries.iter().filter(|_| show) {
        let pos = if i == 0 { Some(Vec3::ZERO) } else { star_world(&stars[i], view.unfold) };
        let screen = pos.and_then(|p| camera.world_to_viewport(cam_tf, p).ok());
        // A tag runs about 140 px to the right of its star; keep the whole of it off
        // the data plate and model.
        let free = screen.filter(|s| {
            taken.iter().all(|t| (s.y - t.y).abs() > 16.0 || (s.x - t.x).abs() > 140.0)
                && ![0.0, 70.0, 140.0]
                    .iter()
                    .any(|dx| dossier.covers(*s + Vec2::new(8.0 + dx, -8.0)))
        });
        placed.push(free.map(|screen| {
            taken.push(screen);
            let name =
                if i == 0 { "SOL".to_string() } else { stars[i].display_name().to_uppercase() };
            let label = match pc {
                Some(pc) => format!("{name}  {} LY", ly_text(pc * LY_PER_PC, 1)),
                None => name,
            };
            (screen, label)
        }));
    }

    for (tag, mut text, mut node) in &mut tags {
        match placed.get(tag.0).cloned().flatten() {
            Some((screen, label)) => {
                if text.0 != label {
                    text.0 = label;
                }
                node.left = px((screen.x + 8.0) / scale.0);
                node.top = px((screen.y - 16.0) / scale.0);
            }
            None if !text.0.is_empty() => text.0.clear(),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn significant_figures() {
        assert_eq!(sig(1.0412), "1.04");
        assert_eq!(sig(25.37), "25.4");
        assert_eq!(sig(531.2), "531");
        assert_eq!(sig(52_340.0), "52,340");
        assert_eq!(sig(0.0081), "0.0081");
        assert_eq!(ly_text(0.004, 2), "<0.01");
        assert_eq!(ly_text(4.317, 2), "4.32");
        assert_eq!(ly_text(0.03, 1), "<0.1");
    }
}
