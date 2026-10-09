//! Sound: short effects cut from CC0 Freesound clips (`cargo xtask sfx`, see
//! xtask/src/sfx.rs for sources), plus a projector hum synthesised at startup.
//! Off by default; M or the menu toggles it.
//!
//! select    two quick beeps
//! locate    targeting computer read-out while the view zooms in
//! lock      "target acquired" beep run
//! chart_in  scanner sweep as the globe flattens onto the table
//! chart_out a second sweep when it lifts back up
//! click     one short beep for buttons and toggles
//! nomatch   short error tone
//! hum       soft projector hum with flickering static and crackle, looped

use std::f32::consts::{PI, TAU};

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::camera::Orbit;
use crate::locate::Locate;
use crate::picking::Selection;
use crate::sky::SkyView;
use crate::{AppState, hotkeys_enabled};

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundOn>().add_systems(Startup, load).add_systems(
            Update,
            (sound_key.run_if(hotkeys_enabled), hum, cues)
                .chain()
                .run_if(in_state(AppState::Ready)),
        );
    }
}

/// Whether sound is enabled (on by default; browsers start it on the first input).
#[derive(Resource)]
pub struct SoundOn(pub bool);

impl Default for SoundOn {
    fn default() -> Self {
        SoundOn(true)
    }
}

const RATE: u32 = 22_050;

#[derive(Resource)]
struct Sounds {
    select: Handle<AudioSource>,
    locate: Handle<AudioSource>,
    lock: Handle<AudioSource>,
    chart_in: Handle<AudioSource>,
    chart_out: Handle<AudioSource>,
    click: Handle<AudioSource>,
    nomatch: Handle<AudioSource>,
    hum: Handle<AudioSource>,
}

fn load(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut sources: ResMut<Assets<AudioSource>>,
) {
    commands.insert_resource(Sounds {
        select: assets.load("sounds/select.wav"),
        locate: assets.load("sounds/locate.wav"),
        lock: assets.load("sounds/lock.wav"),
        chart_in: assets.load("sounds/chart_in.wav"),
        chart_out: assets.load("sounds/chart_out.wav"),
        click: assets.load("sounds/click.wav"),
        nomatch: assets.load("sounds/nomatch.wav"),
        hum: sources.add(AudioSource { bytes: wav(&hum_loop()).into() }),
    });
}

fn sound_key(keys: Res<ButtonInput<KeyCode>>, mut on: ResMut<SoundOn>) {
    if keys.just_pressed(KeyCode::KeyM) {
        on.0 = !on.0;
    }
}

#[derive(Component)]
struct Hum;

/// Keeps the ambient hum running while sound is on.
fn hum(
    mut commands: Commands,
    on: Res<SoundOn>,
    sounds: Res<Sounds>,
    playing: Query<Entity, With<Hum>>,
) {
    if !on.is_changed() {
        return;
    }
    for e in &playing {
        commands.entity(e).despawn();
    }
    if on.0 {
        commands.spawn((
            Hum,
            AudioPlayer(sounds.hum.clone()),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.1)),
        ));
    }
}

/// What the cues last saw, to fire each sound once per change.
#[derive(Default)]
struct Seen {
    selected: Option<usize>,
    located: Option<usize>,
    locked: Option<usize>,
    chart_on: bool,
    note: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn cues(
    mut commands: Commands,
    on: Res<SoundOn>,
    sounds: Res<Sounds>,
    selection: Res<Selection>,
    locate: Res<Locate>,
    view: Res<SkyView>,
    orbit: Res<Orbit>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
    mut seen: Local<Seen>,
) {
    let mut play = |sound: &Handle<AudioSource>, volume: f32| {
        commands.spawn((
            AudioPlayer(sound.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
        ));
    };
    let located = locate.current();
    let locked = located.filter(|_| locate.locked(&view, &orbit));
    let note = locate.note().map(str::to_string);

    if on.0 {
        if selection.selected != seen.selected && selection.selected.is_some() && located.is_none()
        {
            play(&sounds.select, 0.35);
        }
        if located != seen.located && located.is_some() && locate.note().is_none() {
            play(&sounds.locate, 0.4);
        }
        if locked != seen.locked && locked.is_some() {
            play(&sounds.lock, 0.4);
        }
        if view.chart_on != seen.chart_on {
            let sweep = if view.chart_on { &sounds.chart_in } else { &sounds.chart_out };
            play(sweep, 0.5);
        }
        if note != seen.note
            && note.as_deref().is_some_and(|n| n.contains("NO OBJECT") || n.contains("UNREACHABLE"))
        {
            play(&sounds.nomatch, 0.45);
        }
        let toggles = [
            KeyCode::KeyG,
            KeyCode::KeyC,
            KeyCode::KeyP,
            KeyCode::KeyH,
            KeyCode::KeyN,
            KeyCode::KeyM,
            KeyCode::BracketLeft,
            KeyCode::BracketRight,
        ];
        let pressed = buttons.iter().any(|i| *i == Interaction::Pressed);
        if pressed || toggles.iter().any(|k| keys.just_pressed(*k)) {
            play(&sounds.click, 0.25);
        }
    }
    *seen = Seen { selected: selection.selected, located, locked, chart_on: view.chart_on, note };
}

/// Small xorshift generator so the hum is the same on every run.
struct Rng(u32);

impl Rng {
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32
    }

    fn noise(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

fn seconds(s: f32) -> usize {
    (s * RATE as f32) as usize
}

/// Topology-preserving state variable filter; returns (band, high).
#[derive(Default)]
struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    fn run(&mut self, x: f32, cutoff: f32, q: f32) -> (f32, f32) {
        let g = (PI * cutoff / RATE as f32).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v1, x - k * v1 - v2)
    }
}

/// Six seconds of hologram projector: a soft 60 Hz hum, static that flickers
/// in and out, and sparse crackles. The tail is crossfaded into the head so
/// the loop has no seam.
fn hum_loop() -> Vec<f32> {
    let (n, overlap) = (seconds(6.0), seconds(0.5));
    let mut rng = Rng(0x5747_A125);
    let (mut hiss_bp, mut crackle_hp) = (Svf::default(), Svf::default());
    let mut crackle = 0.0f32;
    let mut flicker = 0.5f32;
    let raw: Vec<f32> = (0..n + overlap)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let phase = TAU * 60.0 * t;
            let hum = phase.sin() * 0.45
                + (2.0 * phase).sin() * 0.22
                + (3.0 * phase).sin() * 0.1
                + (5.0 * phase).sin() * 0.04;
            // Flicker drifts randomly and now and then nearly drops out.
            flicker += (rng.unit() - 0.5) * 0.004;
            flicker = flicker.clamp(0.1, 1.0);
            let hiss = hiss_bp.run(rng.noise(), 2200.0, 0.8).0 * 0.2 * flicker;
            if rng.unit() < 7.0 / RATE as f32 {
                crackle = rng.noise() * 0.8;
            }
            crackle *= 0.93;
            let crack = crackle_hp.run(crackle * rng.unit(), 1500.0, 0.7).1;
            let wobble = 0.85 + 0.15 * (TAU * 0.5 * t).sin();
            hum * wobble + hiss + crack
        })
        .collect();
    let mut out: Vec<f32> = raw[..n].to_vec();
    for i in 0..overlap {
        let x = i as f32 / overlap as f32;
        let (a, b) = ((x * PI / 2.0).sin(), (x * PI / 2.0).cos());
        out[i] = out[i] * a + raw[n + i] * b;
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    out.iter_mut().for_each(|s| *s *= 0.8 / peak);
    out
}

/// Mono 16-bit PCM WAV.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_files_exist() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/sounds");
        for name in ["select", "locate", "lock", "chart_in", "chart_out", "click", "nomatch"] {
            let bytes = std::fs::read(dir.join(format!("{name}.wav"))).unwrap();
            assert_eq!(&bytes[0..4], b"RIFF", "{name}.wav is not a WAV");
        }
    }

    #[test]
    fn hum_stays_in_range() {
        let h = hum_loop();
        assert!(h.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
    }

    #[test]
    fn hum_loops_seamlessly() {
        let h = hum_loop();
        // The sample after the last would be the first again: no jump at the seam.
        let jump = (h[0] - h[h.len() - 1]).abs();
        assert!(jump < 0.08, "loop seam jumps by {jump}");
    }

    #[test]
    fn wav_header() {
        let w = wav(&[0.0, 0.5, -0.5]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), RATE);
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 6);
        assert_eq!(w.len(), 44 + 6);
    }
}
