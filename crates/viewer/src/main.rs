mod camera;
mod data;
mod hud;
mod picking;
mod sky;

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;

pub const HOLO: Color = Color::srgb(0.72, 0.86, 1.0);
pub const GLOBE_RADIUS: f32 = 4.0;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Loading,
    Ready,
}

/// Equatorial coordinates are Z-up (toward the north celestial pole); Bevy is Y-up.
pub fn sky_to_world([x, y, z]: [f32; 3]) -> Vec3 {
    Vec3::new(x, z, -y)
}

fn asset_root() -> String {
    if cfg!(target_arch = "wasm32") {
        "assets".into()
    } else {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").into()
    }
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Star-Navigator".into(),
                        canvas: Some("#viewer".into()),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root(),
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        )
        .insert_resource(ClearColor(Color::BLACK))
        .init_state::<AppState>()
        .add_plugins((
            data::DataPlugin,
            camera::OrbitPlugin,
            sky::SkyPlugin,
            picking::PickingPlugin,
            hud::HudPlugin,
        ))
        .run();
}
