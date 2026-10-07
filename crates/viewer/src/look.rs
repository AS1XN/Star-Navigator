//! The hologram look: post-process effect, palettes, presets and the tuning panel.
//!
//! T opens the panel; Up/Down pick a setting, Left/Right change it (Shift for
//! bigger steps), P cycles the palette, H bypasses all effects, R resets, and S
//! saves the preset (to `assets/presets/look.preset` on desktop, to the browser
//! console on the web).

use bevy::core_pipeline::Core3dSystems;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::ecs::schedule::ScheduleConfigs;
use bevy::ecs::system::BoxedSystem;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;
use bevy::window::PrimaryWindow;

use crate::data::BinaryFile;
use crate::sky::{StarField, StarMaterial};

pub struct LookPlugin;

impl Plugin for LookPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<HoloEffect>::default())
            .init_resource::<Look>()
            .init_resource::<Tuner>()
            .add_systems(Startup, (load_preset, spawn_panel))
            .add_systems(
                Update,
                (
                    apply_preset,
                    tune_input.run_if(crate::hotkeys_enabled),
                    update_effect,
                    update_stars,
                    tint_text,
                    show_backed,
                    draw_panel,
                ),
            );
    }
}

/// Uniforms for `holo.wgsl`. Lives on the camera; rebuilt from [`Look`] each frame.
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default)]
pub struct HoloEffect {
    tint: Vec4,
    resolution: Vec2,
    time: f32,
    scanlines: f32,
    scan_period: f32,
    roll: f32,
    flicker: f32,
    jitter: f32,
    tear: f32,
    grain: f32,
    fringe: f32,
    vignette: f32,
    mono: f32,
    mask: f32,
    _pad: Vec2,
}

impl FullscreenMaterial for HoloEffect {
    fn fragment_shader() -> ShaderRef {
        "shaders/holo.wgsl".into()
    }

    // After tonemapping, so the effect works on display-range values.
    fn schedule_configs(system: ScheduleConfigs<BoxedSystem>) -> ScheduleConfigs<BoxedSystem> {
        system.in_set(Core3dSystems::PostProcess).after(tonemapping)
    }
}

pub struct Palette {
    pub key: &'static str,
    pub label: &'static str,
    pub color: Color,
}

pub const PALETTES: [Palette; 4] = [
    Palette { key: "holo", label: "HOLO BLUE", color: Color::srgb(0.72, 0.86, 1.0) },
    Palette { key: "tactical", label: "TACTICAL RED", color: Color::srgb(1.0, 0.36, 0.28) },
    Palette { key: "targeting", label: "TARGETING AMBER", color: Color::srgb(1.0, 0.74, 0.3) },
    Palette { key: "wireframe", label: "WIREFRAME GREEN", color: Color::srgb(0.5, 1.0, 0.6) },
];

struct Param {
    key: &'static str,
    label: &'static str,
    default: f32,
    min: f32,
    max: f32,
    step: f32,
}

const fn p(
    key: &'static str,
    label: &'static str,
    default: f32,
    min: f32,
    max: f32,
    step: f32,
) -> Param {
    Param { key, label, default, min, max, step }
}

const PARAMS: [Param; 15] = [
    p("scanlines", "SCANLINES", 0.35, 0.0, 1.0, 0.05),
    p("scan_period", "SCAN PERIOD PX", 3.0, 1.5, 8.0, 0.25),
    p("roll", "ROLL", 0.5, 0.0, 2.0, 0.1),
    p("flicker", "FLICKER", 0.12, 0.0, 1.0, 0.02),
    p("jitter", "LINE JITTER PX", 0.6, 0.0, 4.0, 0.1),
    p("tear", "TEAR PX", 6.0, 0.0, 40.0, 1.0),
    p("grain", "GRAIN", 0.06, 0.0, 0.5, 0.01),
    p("fringe", "FRINGE PX", 2.0, 0.0, 10.0, 0.25),
    p("vignette", "VIGNETTE", 0.6, 0.0, 1.5, 0.05),
    p("mono", "MONOCHROME", 0.85, 0.0, 1.0, 0.05),
    p("mask", "LED MASK", 0.25, 0.0, 1.0, 0.05),
    p("bloom", "BLOOM", 0.3, 0.0, 1.0, 0.02),
    p("star_size", "STAR SIZE", 1.0, 0.5, 3.0, 0.1),
    p("star_glow", "STAR GLOW", 1.6, 0.5, 4.0, 0.1),
    p("back_fade", "FAR SIDE", 0.3, 0.0, 1.0, 0.05),
];

const fn idx(key: &str) -> usize {
    let mut i = 0;
    while i < PARAMS.len() {
        if const_eq(PARAMS[i].key, key) {
            return i;
        }
        i += 1;
    }
    panic!("unknown look parameter");
}

const fn const_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

#[derive(Resource, Clone, PartialEq)]
pub struct Look {
    pub palette: usize,
    values: [f32; PARAMS.len()],
    /// H toggles all effects off for comparison.
    bypass: bool,
}

impl Default for Look {
    fn default() -> Self {
        Self { palette: 0, values: PARAMS.map(|p| p.default), bypass: false }
    }
}

impl Look {
    fn get(&self, key: &str) -> f32 {
        self.values[idx(key)]
    }

    pub fn color(&self) -> Color {
        PALETTES[self.palette].color
    }

    fn to_preset(&self) -> String {
        let mut s = String::from("# Star-Navigator look preset\n");
        s += &format!("palette = {}\n", PALETTES[self.palette].key);
        for (p, v) in PARAMS.iter().zip(self.values) {
            s += &format!("{} = {}\n", p.key, (v * 1000.0).round() / 1000.0);
        }
        s
    }

    fn parse_preset(text: &str) -> Look {
        let mut look = Look::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else { continue };
            let (key, value) = (key.trim(), value.trim());
            if key == "palette" {
                if let Some(i) = PALETTES.iter().position(|p| p.key == value) {
                    look.palette = i;
                }
            } else if let (Some(i), Ok(v)) =
                (PARAMS.iter().position(|p| p.key == key), value.parse::<f32>())
            {
                look.values[i] = v.clamp(PARAMS[i].min, PARAMS[i].max);
            } else {
                warn!("look preset: ignoring line {line:?}");
            }
        }
        look
    }
}

#[derive(Resource, Default)]
pub struct Tuner {
    pub open: bool,
    cursor: usize,
    preset: Option<Handle<BinaryFile>>,
    message: String,
}

const PRESET_PATH: &str = "presets/look.preset";

fn load_preset(assets: Res<AssetServer>, mut tuner: ResMut<Tuner>) {
    tuner.preset = Some(assets.load(PRESET_PATH));
}

fn apply_preset(mut tuner: ResMut<Tuner>, files: Res<Assets<BinaryFile>>, mut look: ResMut<Look>) {
    let Some(handle) = &tuner.preset else { return };
    if let Some(file) = files.get(handle) {
        *look = Look::parse_preset(&String::from_utf8_lossy(&file.0));
        tuner.preset = None;
    }
}

impl Tuner {
    /// Moves the panel cursor up (negative) or down (positive), wrapping.
    pub fn select(&mut self, step: i32) {
        let n = PARAMS.len() as i32;
        self.cursor = (self.cursor as i32 + step).rem_euclid(n) as usize;
    }

    /// Nudges the selected setting by `steps` of its step size.
    pub fn adjust(&self, look: &mut Look, steps: f32) {
        let param = &PARAMS[self.cursor];
        let v = &mut look.values[self.cursor];
        *v = (*v + param.step * steps).clamp(param.min, param.max);
    }
}

impl Look {
    pub fn next_palette(&mut self) {
        self.palette = (self.palette + 1) % PALETTES.len();
    }

    pub fn bypassed(&self) -> bool {
        self.bypass
    }

    pub fn toggle_bypass(&mut self) {
        self.bypass = !self.bypass;
    }
}

fn tune_input(keys: Res<ButtonInput<KeyCode>>, mut tuner: ResMut<Tuner>, mut look: ResMut<Look>) {
    if keys.just_pressed(KeyCode::KeyT) {
        tuner.open = !tuner.open;
        tuner.message.clear();
    }
    if keys.just_pressed(KeyCode::KeyP) {
        look.next_palette();
    }
    if keys.just_pressed(KeyCode::KeyH) {
        look.toggle_bypass();
    }
    if !tuner.open {
        return;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        tuner.select(1);
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        tuner.select(-1);
    }
    let fast = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        5.0
    } else {
        1.0
    };
    if keys.just_pressed(KeyCode::ArrowRight) {
        tuner.adjust(&mut look, fast);
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        tuner.adjust(&mut look, -fast);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        *look = Look { palette: look.palette, ..default() };
        tuner.message = "DEFAULTS RESTORED".into();
    }
    if keys.just_pressed(KeyCode::KeyS) {
        tuner.message = save_preset(&look.to_preset());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_preset(text: &str) -> String {
    let path = std::path::Path::new(&crate::asset_root()).join(PRESET_PATH);
    let result = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(&path, text));
    match result {
        Ok(()) => format!("SAVED {PRESET_PATH}"),
        Err(e) => format!("SAVE FAILED: {e}"),
    }
}

#[cfg(target_arch = "wasm32")]
fn save_preset(text: &str) -> String {
    info!("look preset:\n{text}");
    "PRESET PRINTED TO BROWSER CONSOLE".into()
}

fn update_effect(
    look: Res<Look>,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut camera: Single<(&mut HoloEffect, &mut Bloom)>,
) {
    let (effect, bloom) = &mut *camera;
    let on = if look.bypass { 0.0 } else { 1.0 };
    **effect = HoloEffect {
        tint: LinearRgba::from(look.color()).to_vec4(),
        resolution: Vec2::new(window.physical_width() as f32, window.physical_height() as f32),
        time: time.elapsed_secs_wrapped(),
        scanlines: look.get("scanlines") * on,
        scan_period: look.get("scan_period"),
        roll: look.get("roll") * on,
        flicker: look.get("flicker") * on,
        jitter: look.get("jitter") * on,
        tear: look.get("tear") * on,
        grain: look.get("grain") * on,
        fringe: look.get("fringe") * on,
        vignette: look.get("vignette") * on,
        mono: look.get("mono") * on,
        mask: look.get("mask") * on,
        _pad: Vec2::ZERO,
    };
    bloom.intensity = look.get("bloom");
}

fn update_stars(
    look: Res<Look>,
    field: Option<Single<Ref<StarField>>>,
    mut materials: ResMut<Assets<StarMaterial>>,
) {
    let Some(field) = field else { return };
    if !look.is_changed() && !field.is_added() {
        return;
    }
    if let Some(mut m) = materials.get_mut(&field.0) {
        m.settings.size_scale = look.get("star_size");
        m.settings.brightness = look.get("star_glow");
        m.settings.back_fade = look.get("back_fade");
    }
}

/// HUD text drawn in the palette color with the given alpha. The UI renders after
/// the post-process, so it has to be tinted directly.
#[derive(Component)]
pub struct Tinted(pub f32);

/// Dark translucent backing behind a HUD text block, so stars, reticles and labels
/// behind it can't make it unreadable. `z` orders overlapping blocks; the block is
/// hidden while its text is empty.
#[derive(Component)]
pub struct Backed;

pub fn backed(z: i32) -> impl Bundle {
    (Backed, BackgroundColor(Color::BLACK.with_alpha(0.6)), ZIndex(z), Visibility::Hidden)
}

type BackedText<'a> = (&'a Text, &'a mut Visibility);

fn show_backed(mut blocks: Query<BackedText, (With<Backed>, Changed<Text>)>) {
    for (text, mut vis) in &mut blocks {
        vis.set_if_neq(if text.0.is_empty() { Visibility::Hidden } else { Visibility::Inherited });
    }
}

/// Outline drawn in the palette color with the given alpha (touch buttons).
#[derive(Component)]
pub struct TintedBorder(pub f32);

fn tint_text(
    look: Res<Look>,
    mut texts: Query<(Ref<Tinted>, &mut TextColor)>,
    mut borders: Query<(Ref<TintedBorder>, &mut BorderColor)>,
) {
    let color = look.color();
    for (t, mut c) in &mut texts {
        if look.is_changed() || t.is_added() {
            c.0 = color.with_alpha(t.0);
        }
    }
    for (t, mut b) in &mut borders {
        if look.is_changed() || t.is_added() {
            *b = BorderColor::all(color.with_alpha(t.0));
        }
    }
    if look.is_changed() {
        share_palette_with_page(color);
    }
}

/// The page's own FIND button (touch screens) reads `--holo` to match the palette.
#[cfg(target_arch = "wasm32")]
fn share_palette_with_page(color: Color) {
    use wasm_bindgen::JsCast;
    let c = color.to_srgba();
    let hex = format!(
        "#{:02x}{:02x}{:02x}",
        (c.red * 255.0) as u8,
        (c.green * 255.0) as u8,
        (c.blue * 255.0) as u8
    );
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
    if let Some(root) = root {
        let _ = root.style().set_property("--holo", &hex);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn share_palette_with_page(_color: Color) {}

#[derive(Component)]
struct Panel;

fn spawn_panel(mut commands: Commands) {
    commands.spawn((
        Panel,
        Tinted(0.9),
        backed(20),
        Text::new(""),
        TextFont { font_size: FontSize::Px(13.0), ..default() },
        TextColor::WHITE,
        Node {
            position_type: PositionType::Absolute,
            right: px(24),
            top: px(52),
            padding: UiRect::axes(px(10), px(6)),
            ..default()
        },
    ));
}

fn draw_panel(
    tuner: Res<Tuner>,
    look: Res<Look>,
    touch: Res<crate::touch::TouchState>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut panel: Single<&mut Text, With<Panel>>,
) {
    let touch_mode = crate::touch::touch_mode(&touch, &window);
    if !tuner.is_changed() && !look.is_changed() && !touch.is_changed() {
        return;
    }
    if !tuner.open {
        panel.0.clear();
        return;
    }
    let mut lines = vec![
        "== HOLO CALIBRATION ==".to_string(),
        format!("PALETTE  {}  [P]", PALETTES[look.palette].label),
        if look.bypass { "EFFECTS  BYPASSED  [H]".into() } else { "EFFECTS  ACTIVE  [H]".into() },
        String::new(),
    ];
    for (i, (param, v)) in PARAMS.iter().zip(look.values).enumerate() {
        let marker = if i == tuner.cursor { ">" } else { " " };
        let filled = ((v - param.min) / (param.max - param.min) * 10.0).round() as usize;
        let bar: String = (0..10).map(|j| if j < filled { '|' } else { '.' }).collect();
        lines.push(format!("{marker} {:<15} {bar} {v:>6.2}", param.label));
    }
    lines.push(String::new());
    if touch_mode {
        lines.push("PREV / NEXT SELECT   - / + ADJUST".into());
    } else {
        lines.push("UP/DN SELECT  LT/RT ADJUST  SHIFT x5".into());
        lines.push("R RESET  S SAVE  T CLOSE".into());
    }
    if !tuner.message.is_empty() {
        lines.push(format!("> {}", tuner.message));
    }
    panel.0 = lines.join("\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_roundtrip() {
        let mut look = Look { palette: 2, ..default() };
        look.values[idx("tear")] = 12.0;
        assert!(Look::parse_preset(&look.to_preset()) == look);
    }

    #[test]
    fn preset_clamps_and_ignores_junk() {
        let look = Look::parse_preset("palette = wireframe\nscanlines = 9\nnonsense\nfoo = 1\n");
        assert_eq!(look.palette, 3);
        assert_eq!(look.get("scanlines"), 1.0);
        assert_eq!(look.get("roll"), Look::default().get("roll"));
    }
}
