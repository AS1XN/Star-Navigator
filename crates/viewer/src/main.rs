mod camera;
mod data;
mod deeplink;
mod dossier;
mod hud;
mod locate;
mod look;
mod picking;
mod search;
mod simbad;
mod sky;
mod touch;

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{Backends, WgpuSettings};

pub const HOLO: Color = Color::srgb(0.72, 0.86, 1.0);
pub const GLOBE_RADIUS: f32 = 4.0;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Loading,
    Ready,
}

/// World units per parsec in the 3D field view (Sol at the origin).
pub const FIELD_SCALE: f32 = 0.05;

/// Equatorial coordinates are Z-up (toward the north celestial pole); Bevy is Y-up.
pub fn sky_to_world([x, y, z]: [f32; 3]) -> Vec3 {
    Vec3::new(x, z, -y)
}

/// Where a star sits on the celestial globe.
pub fn globe_position(star: &catalog::Star) -> Vec3 {
    sky_to_world(star.direction()) * GLOBE_RADIUS
}

/// Where a star sits in the 3D field, if its distance is known.
pub fn field_position(star: &catalog::Star) -> Option<Vec3> {
    star.position().map(|p| sky_to_world(p) * FIELD_SCALE)
}

/// Current on-screen position: blends globe and field by the unfold amount.
/// `None` for stars without a distance once the field has mostly unfolded.
pub fn star_world(star: &catalog::Star, unfold: f32) -> Option<Vec3> {
    let globe = globe_position(star);
    match field_position(star) {
        Some(f) => Some(globe.lerp(f, unfold)),
        None if unfold < 0.5 => Some(globe.lerp(globe * 12.0, unfold)),
        None => None,
    }
}

/// True while the search box has the keyboard, so single-key shortcuts stay quiet.
#[derive(Resource, Default)]
pub struct Typing(pub bool);

pub fn hotkeys_enabled(typing: Res<Typing>) -> bool {
    !typing.0
}

pub fn asset_root() -> String {
    if cfg!(target_arch = "wasm32") {
        "assets".into()
    } else {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").into()
    }
}

/// On Windows, prefer DirectX 12: with the same scene, Vulkan's allocator reserved
/// about 680 MB of GPU memory against about 280 MB for DX12 (shared memory on
/// integrated GPUs counts against system RAM). `WGPU_BACKEND` still overrides.
fn render_settings() -> WgpuSettings {
    let mut settings = WgpuSettings::default();
    if cfg!(target_os = "windows") && std::env::var_os("WGPU_BACKEND").is_none() {
        settings.backends = Some(Backends::DX12);
    }
    settings
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
                })
                .set(RenderPlugin { render_creation: render_settings().into(), ..default() }),
        )
        .insert_resource(ClearColor(Color::BLACK))
        .init_state::<AppState>()
        .init_resource::<Typing>()
        .add_plugins((
            data::DataPlugin,
            camera::OrbitPlugin,
            sky::SkyPlugin,
            picking::PickingPlugin,
            hud::HudPlugin,
            look::LookPlugin,
            search::SearchPlugin,
            locate::LocatePlugin,
            dossier::DossierPlugin,
            deeplink::DeepLinkPlugin,
            touch::TouchPlugin,
            simbad::SimbadPlugin,
        ))
        .run();
}
