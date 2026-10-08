//! The celestial globe: star dots, equator band, coordinate grid and
//! constellation figures.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::data::Sky;
use crate::{
    AppState, CHART_RADIUS, GLOBE_RADIUS, HOLO, chart_position, field_position, globe_position,
    hotkeys_enabled, sky_to_world,
};

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<StarMaterial>::default())
            .init_resource::<SkyView>()
            .add_systems(OnEnter(AppState::Ready), (spawn_stars, build_figures))
            .add_systems(
                Update,
                (
                    controls.run_if(hotkeys_enabled),
                    update_material,
                    fold_band,
                    draw_grid,
                    draw_figures,
                )
                    .run_if(in_state(AppState::Ready)),
            );
    }
}

/// User-adjustable display settings.
#[derive(Resource)]
pub struct SkyView {
    pub mag_limit: f32,
    pub grid: bool,
    pub figures: bool,
    /// 0 = celestial globe, 1 = stars at their true 3D positions around Sol.
    pub unfold: f32,
    /// 0 = globe, 1 = flat chart (animated toward `chart_on`).
    pub chart: f32,
    pub chart_on: bool,
    /// Chart centred on the south celestial pole instead of the north.
    pub south: bool,
}

impl Default for SkyView {
    fn default() -> Self {
        Self {
            mag_limit: 6.5,
            grid: true,
            figures: true,
            unfold: 0.0,
            chart: 0.0,
            chart_on: false,
            south: false,
        }
    }
}

pub const MAG_MIN: f32 = 2.0;
pub const MAG_MAX: f32 = 12.0;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct StarMaterial {
    #[uniform(0)]
    pub settings: StarSettings,
}

#[derive(Clone, Copy, ShaderType)]
pub struct StarSettings {
    pub tint: Vec4,
    pub mag_limit: f32,
    pub size_scale: f32,
    pub back_fade: f32,
    pub brightness: f32,
    pub unfold: f32,
    pub chart: f32,
    /// +1 for a north-centred chart, -1 for south.
    pub chart_pole: f32,
    // Uniforms must be a multiple of 16 bytes on WebGL2.
    _pad: f32,
}

impl Material for StarMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/stars.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/stars.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

#[derive(Component)]
pub struct StarField(pub Handle<StarMaterial>);

/// Rough B-V color index to RGB, pulled most of the way toward white so the
/// hologram stays monochrome with just a hint of temperature.
fn star_color(ci: Option<f32>) -> [f32; 4] {
    let t = ((ci.unwrap_or(0.6) + 0.4) / 2.4).clamp(0.0, 1.0);
    let warm = Vec3::new(1.0, 0.82, 0.62);
    let cool = Vec3::new(0.7, 0.82, 1.0);
    let c = cool.lerp(warm, t).lerp(Vec3::ONE, 0.6);
    [c.x, c.y, c.z, 1.0]
}

fn spawn_stars(
    mut commands: Commands,
    sky: Res<Sky>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut star_materials: ResMut<Assets<StarMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let stars: Vec<_> = sky.catalog.stars().iter().filter(|s| !s.is_sun()).collect();
    let n = stars.len();
    let mut positions = Vec::with_capacity(n * 4);
    let mut field = Vec::with_capacity(n * 4);
    let mut corners = Vec::with_capacity(n * 4);
    let mut params = Vec::with_capacity(n * 4);
    let mut colors = Vec::with_capacity(n * 4);
    let mut indices = Vec::with_capacity(n * 6);

    for (i, star) in stars.iter().enumerate() {
        let globe = globe_position(star);
        // Stars without a distance drift outward and fade as the globe unfolds.
        let (f, known) = match field_position(star) {
            Some(f) => (f, 1.0),
            None => (globe * 12.0, 0.0),
        };
        let color = star_color(star.ci);
        for corner in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
            positions.push(globe.to_array());
            // The normal attribute is free, so it carries the 3D field position.
            field.push(f.to_array());
            corners.push(corner);
            params.push([star.mag, known]);
            colors.push(color);
        }
        let b = i as u32 * 4;
        indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, field)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, params)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices));

    let material = star_materials.add(StarMaterial {
        settings: StarSettings {
            tint: LinearRgba::from(HOLO).to_vec4(),
            mag_limit: SkyView::default().mag_limit,
            size_scale: 1.0,
            back_fade: 0.3,
            brightness: 1.6,
            unfold: 0.0,
            chart: 0.0,
            chart_pole: 1.0,
            _pad: 0.0,
        },
    });
    commands.spawn((
        StarField(material.clone()),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Transform::default(),
        NoFrustumCulling,
    ));

    // Celestial equator band.
    let band = materials.add(StandardMaterial {
        base_color: (LinearRgba::from(HOLO) * 1.4).into(),
        unlit: true,
        ..default()
    });
    commands.spawn((
        EquatorBand,
        Mesh3d(meshes.add(Torus::new(GLOBE_RADIUS - 0.012, GLOBE_RADIUS + 0.012))),
        MeshMaterial3d(band),
    ));
}

#[derive(Component)]
struct EquatorBand;

/// The band shrinks into Sol as the globe unfolds into the 3D field.
fn fold_band(
    view: Res<SkyView>,
    mut band: Single<(&mut Transform, &mut Visibility), With<EquatorBand>>,
) {
    if !view.is_changed() {
        return;
    }
    let (transform, visibility) = &mut *band;
    // Into the field it shrinks into Sol; onto the chart it settles exactly on the
    // chart's equator ring (half the chart radius).
    let to_chart = 1.0 + (CHART_RADIUS * 0.5 / GLOBE_RADIUS - 1.0) * view.chart;
    let s = (1.0 - view.unfold) * to_chart;
    transform.scale = Vec3::new(s, 1.0 - view.chart * 0.9, s);
    **visibility = if s < 0.01 { Visibility::Hidden } else { Visibility::Inherited };
}

fn controls(keys: Res<ButtonInput<KeyCode>>, mut view: ResMut<SkyView>) {
    if keys.just_pressed(KeyCode::BracketRight) || keys.just_pressed(KeyCode::Equal) {
        view.mag_limit = (view.mag_limit + 0.5).min(MAG_MAX);
    }
    if keys.just_pressed(KeyCode::BracketLeft) || keys.just_pressed(KeyCode::Minus) {
        view.mag_limit = (view.mag_limit - 0.5).max(MAG_MIN);
    }
    if keys.just_pressed(KeyCode::KeyG) {
        view.grid = !view.grid;
    }
    if keys.just_pressed(KeyCode::KeyC) {
        view.figures = !view.figures;
    }
}

fn update_material(
    view: Res<SkyView>,
    field: Single<&StarField>,
    mut materials: ResMut<Assets<StarMaterial>>,
) {
    if !view.is_changed() {
        return;
    }
    if let Some(mut m) = materials.get_mut(&field.0) {
        m.settings.mag_limit = view.mag_limit;
        m.settings.unfold = view.unfold;
        m.settings.chart = view.chart;
        m.settings.chart_pole = if view.south { -1.0 } else { 1.0 };
    }
}

fn on_sphere(ra_hours: f32, dec_deg: f32) -> Vec3 {
    let ra = (ra_hours * 15.0).to_radians();
    let dec = dec_deg.to_radians();
    sky_to_world([dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin()]) * GLOBE_RADIUS
}

fn grid_color(strength: f32) -> LinearRgba {
    LinearRgba::from(HOLO) * strength
}

/// Lines on the far side of the globe are drawn dimmer, matching the stars.
fn faded(
    points: impl IntoIterator<Item = Vec3>,
    eye: Vec3,
    color: LinearRgba,
) -> Vec<(Vec3, Color)> {
    points
        .into_iter()
        .map(|p| {
            let facing = p.normalize().dot(eye);
            let t = ((facing + 0.15) / 0.3).clamp(0.0, 1.0);
            (p, (color * (0.18 + 0.82 * t * t * (3.0 - 2.0 * t))).into())
        })
        .collect()
}

fn draw_grid(
    view: Res<SkyView>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut gizmos: Gizmos,
) {
    // The chart draws its own grid (see chart.rs).
    let globe = (1.0 - view.unfold) * (1.0 - view.chart);
    if !view.grid || globe < 0.02 {
        return;
    }
    let eye = camera.translation().normalize();
    let color = grid_color(0.22 * globe);
    for dec in [-60.0, -30.0, 30.0, 60.0] {
        let pts = (0..=96).map(|i| on_sphere(i as f32 * 24.0 / 96.0, dec));
        gizmos.linestrip_gradient(faded(pts, eye, color));
    }
    for hour in (0..24).step_by(2) {
        let pts = (0..=40).map(|i| on_sphere(hour as f32, -80.0 + i as f32 * 4.0));
        gizmos.linestrip_gradient(faded(pts, eye, color));
    }
}

/// Constellation figures resampled along great circles so long segments hug the globe.
#[derive(Resource)]
struct Figures(Vec<Vec<Vec3>>);

fn build_figures(mut commands: Commands, sky: Res<Sky>) {
    let strips = sky
        .lines
        .iter()
        .map(|line| {
            let pts: Vec<Vec3> = line.points.iter().map(|&(ra, dec)| on_sphere(ra, dec)).collect();
            let mut out = Vec::new();
            for pair in pts.windows(2) {
                let (a, b) = (pair[0].normalize(), pair[1].normalize());
                let steps = (a.angle_between(b).to_degrees() / 2.0).ceil().max(1.0) as usize;
                for s in 0..steps {
                    out.push(a.slerp(b, s as f32 / steps as f32) * GLOBE_RADIUS);
                }
            }
            out.extend(pts.last());
            out
        })
        .collect();
    commands.insert_resource(Figures(strips));
}

fn draw_figures(
    view: Res<SkyView>,
    figures: Res<Figures>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut gizmos: Gizmos,
) {
    if !view.figures || view.unfold > 0.98 {
        return;
    }
    let eye = camera.translation().normalize();
    let globe = (1.0 - view.unfold) * (1.0 - view.chart);
    if globe > 0.02 {
        let color = grid_color(0.55 * globe);
        for strip in &figures.0 {
            gizmos.linestrip_gradient(faded(strip.iter().copied(), eye, color));
        }
    }
    // The same figures flattened onto the chart. A segment that wraps around the
    // chart (crossing RA 0h near the rim) is skipped rather than drawn across it.
    if view.chart > 0.02 {
        let color: Color = (grid_color(0.55) * view.chart).into();
        for strip in &figures.0 {
            let flat: Vec<Vec3> = strip.iter().map(|p| chart_position(*p, view.south)).collect();
            for pair in flat.windows(2) {
                if pair[0].distance(pair[1]) < CHART_RADIUS * 0.2 {
                    gizmos.line(pair[0], pair[1], color);
                }
            }
        }
    }
}
