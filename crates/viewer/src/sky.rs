//! The celestial globe: star dots, equator band, coordinate grid and
//! constellation figures.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::data::Sky;
use crate::{AppState, GLOBE_RADIUS, HOLO, sky_to_world};

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<StarMaterial>::default())
            .init_resource::<SkyView>()
            .add_systems(OnEnter(AppState::Ready), (spawn_stars, build_figures))
            .add_systems(
                Update,
                (controls, update_material, draw_grid, draw_figures)
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
}

impl Default for SkyView {
    fn default() -> Self {
        Self { mag_limit: 6.5, grid: true, figures: true }
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
struct StarField(Handle<StarMaterial>);

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
    let mut corners = Vec::with_capacity(n * 4);
    let mut params = Vec::with_capacity(n * 4);
    let mut colors = Vec::with_capacity(n * 4);
    let mut indices = Vec::with_capacity(n * 6);

    for (i, star) in stars.iter().enumerate() {
        let p = (sky_to_world(star.direction()) * GLOBE_RADIUS).to_array();
        let color = star_color(star.ci);
        for corner in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
            positions.push(p);
            corners.push(corner);
            params.push([star.mag, 0.0]);
            colors.push(color);
        }
        let b = i as u32 * 4;
        indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
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
        Mesh3d(meshes.add(Torus::new(GLOBE_RADIUS - 0.012, GLOBE_RADIUS + 0.012))),
        MeshMaterial3d(band),
    ));
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
    if !view.grid {
        return;
    }
    let eye = camera.translation().normalize();
    let color = grid_color(0.22);
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
    if !view.figures {
        return;
    }
    let eye = camera.translation().normalize();
    let color = grid_color(0.55);
    for strip in &figures.0 {
        gizmos.linestrip_gradient(faded(strip.iter().copied(), eye, color));
    }
}
