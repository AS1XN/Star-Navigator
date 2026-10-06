use bevy::camera::Hdr;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

const HOLO: Color = Color::srgb(0.72, 0.86, 1.0);
const GLOBE_RADIUS: f32 = 4.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Star-Navigator".into(),
                canvas: Some("#viewer".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, (spawn_camera, spawn_globe, spawn_hud))
        .add_systems(Update, spin_globe)
        .run();
}

#[derive(Component)]
struct Globe;

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        Transform::from_xyz(0.0, 1.5, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Placeholder globe: equator band plus the sample stars on the celestial sphere.
fn spawn_globe(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Unlit materials only use base_color; values above 1.0 feed the bloom.
    let glow = materials.add(StandardMaterial {
        base_color: (LinearRgba::from(HOLO) * 4.0).into(),
        unlit: true,
        ..default()
    });
    let dim = materials.add(StandardMaterial {
        base_color: (LinearRgba::from(HOLO) * 1.2).into(),
        unlit: true,
        ..default()
    });

    let star_mesh = meshes.add(Sphere::new(0.06).mesh().ico(1).unwrap());
    let band_mesh = meshes.add(Torus::new(GLOBE_RADIUS - 0.015, GLOBE_RADIUS + 0.015));

    commands.spawn((Globe, Transform::default(), Visibility::default())).with_children(|globe| {
        globe.spawn((Mesh3d(band_mesh), MeshMaterial3d(dim.clone())));

        for star in catalog::sample() {
            // Equatorial frame is Z-up; Bevy is Y-up.
            let [x, y, z] = star.direction();
            let pos = Vec3::new(x as f32, z as f32, -y as f32) * GLOBE_RADIUS;
            let scale = (2.0 - star.mag * 0.4).clamp(0.6, 2.5);
            globe.spawn((
                Mesh3d(star_mesh.clone()),
                MeshMaterial3d(glow.clone()),
                Transform::from_translation(pos).with_scale(Vec3::splat(scale)),
            ));
        }
    });
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("STAR-NAVIGATOR // SYSTEM STANDBY"),
        TextFont { font_size: FontSize::Px(18.0), ..default() },
        TextColor(HOLO),
        Node { position_type: PositionType::Absolute, left: px(24), bottom: px(20), ..default() },
    ));
}

fn spin_globe(time: Res<Time>, mut globe: Query<&mut Transform, With<Globe>>) {
    for mut t in &mut globe {
        t.rotate_y(time.delta_secs() * 0.15);
    }
}
