//! Loads the star catalog and constellation figures through the asset server, so
//! the same code works from disk on desktop and over HTTP on the web.

use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::*;
use catalog::{Catalog, Polyline};

use crate::AppState;

pub struct DataPlugin;

impl Plugin for DataPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<BinaryFile>()
            .register_asset_loader(BinaryLoader)
            .add_systems(Startup, start_loading)
            .add_systems(Update, finish_loading.run_if(in_state(AppState::Loading)));
    }
}

#[derive(Asset, TypePath)]
pub struct BinaryFile(pub Vec<u8>);

#[derive(TypePath)]
struct BinaryLoader;

impl AssetLoader for BinaryLoader {
    type Asset = BinaryFile;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _ctx: &mut LoadContext<'_>,
    ) -> Result<BinaryFile, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(BinaryFile(bytes))
    }

    fn extensions(&self) -> &[&str] {
        &["bin"]
    }
}

#[derive(Resource)]
struct Pending {
    stars: Handle<BinaryFile>,
    lines: Handle<BinaryFile>,
}

#[derive(Resource)]
pub struct Sky {
    pub catalog: Catalog,
    pub lines: Vec<Polyline>,
}

#[derive(Resource, Default)]
pub struct LoadError(pub Option<String>);

fn start_loading(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Pending {
        stars: assets.load("catalog/stars.bin"),
        lines: assets.load("catalog/constellations.bin"),
    });
    commands.init_resource::<LoadError>();
}

fn finish_loading(
    mut commands: Commands,
    pending: Res<Pending>,
    assets: Res<AssetServer>,
    mut files: ResMut<Assets<BinaryFile>>,
    mut error: ResMut<LoadError>,
    mut next: ResMut<NextState<AppState>>,
) {
    for handle in [&pending.stars, &pending.lines] {
        if let Some(bevy::asset::LoadState::Failed(e)) = assets.get_load_state(handle) {
            error.0 = Some(e.to_string());
            return;
        }
    }
    let (Some(stars), Some(lines)) = (files.get(&pending.stars), files.get(&pending.lines)) else {
        return;
    };

    let sky = Catalog::from_bytes(&stars.0)
        .and_then(|catalog| Ok(Sky { catalog, lines: catalog::decode_lines(&lines.0)? }));
    match sky {
        Ok(sky) => {
            info!("catalog loaded: {} stars, {} figure lines", sky.catalog.len(), sky.lines.len());
            commands.insert_resource(sky);
            next.set(AppState::Ready);
        }
        Err(e) => error.0 = Some(e.to_string()),
    }
    files.remove(&pending.stars);
    files.remove(&pending.lines);
    commands.remove_resource::<Pending>();
}
