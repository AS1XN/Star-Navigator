//! Sound: every effect is synthesised here at startup (no audio files), in the
//! spirit of old console hardware. Off by default; M or the menu toggles it.
//!
//! select   short rising two-tone chirp
//! locate   warbling upward sweep while the globe unfolds
//! lock     three pips when the target locks
//! chart    falling sweep as the globe flattens onto the table
//! click    tiny tick for buttons and toggles
//! nomatch  low buzz when a lookup finds nothing
//! hum      quiet looping mains hum with a slow flutter

use std::f32::consts::TAU;

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
        app.init_resource::<SoundOn>().add_systems(Startup, synthesise).add_systems(
            Update,
            (sound_key.run_if(hotkeys_enabled), hum, cues)
                .chain()
                .run_if(in_state(AppState::Ready)),
        );
    }
}

/// Whether sound is enabled (off until the user turns it on).
#[derive(Resource, Default)]
pub struct SoundOn(pub bool);

const RATE: u32 = 22_050;

#[derive(Resource)]
struct Sounds {
    select: Handle<AudioSource>,
    locate: Handle<AudioSource>,
    lock: Handle<AudioSource>,
    chart: Handle<AudioSource>,
    click: Handle<AudioSource>,
    nomatch: Handle<AudioSource>,
    hum: Handle<AudioSource>,
}

fn synthesise(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut add = |samples: Vec<f32>| sources.add(AudioSource { bytes: wav(&samples).into() });
    commands.insert_resource(Sounds {
        select: add(chirp()),
        locate: add(sweep(380.0, 1150.0, 0.9, 9.0)),
        lock: add(pips(3, 1320.0)),
        chart: add(sweep(900.0, 260.0, 0.7, 6.0)),
        click: add(tick()),
        nomatch: add(buzz()),
        hum: add(hum_loop()),
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
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.18)),
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
            play(&sounds.select, 0.5);
        }
        if located != seen.located && located.is_some() && locate.note().is_none() {
            play(&sounds.locate, 0.45);
        }
        if locked != seen.locked && locked.is_some() {
            play(&sounds.lock, 0.5);
        }
        if view.chart_on != seen.chart_on {
            play(&sounds.chart, 0.4);
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
            play(&sounds.click, 0.35);
        }
    }
    *seen = Seen { selected: selection.selected, located, locked, chart_on: view.chart_on, note };
}

/// Attack/decay envelope over `n` samples: quick rise, smooth fall.
fn envelope(i: usize, n: usize, attack: f32) -> f32 {
    let t = i as f32 / n as f32;
    let rise = (t / attack).min(1.0);
    rise * (1.0 - t).powf(1.5)
}

/// A slightly hollow square-ish tone, softer than a pure square.
fn tone(phase: f32) -> f32 {
    0.8 * phase.sin() + 0.2 * (3.0 * phase).sin()
}

fn seconds(s: f32) -> usize {
    (s * RATE as f32) as usize
}

fn chirp() -> Vec<f32> {
    let n = seconds(0.11);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let f = if i < n / 2 { 1250.0 } else { 1870.0 };
            phase += TAU * f / RATE as f32;
            tone(phase) * envelope(i, n, 0.05) * 0.8
        })
        .collect()
}

/// Frequency sweep from `f0` to `f1` with a `wobble` Hz vibrato.
fn sweep(f0: f32, f1: f32, secs: f32, wobble: f32) -> Vec<f32> {
    let n = seconds(secs);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let f = f0 * (f1 / f0).powf(t) * (1.0 + 0.04 * (TAU * wobble * t * secs).sin());
            phase += TAU * f / RATE as f32;
            tone(phase) * envelope(i, n, 0.08) * 0.7
        })
        .collect()
}

fn pips(count: usize, freq: f32) -> Vec<f32> {
    let (on, off) = (seconds(0.065), seconds(0.055));
    let mut out = Vec::with_capacity(count * (on + off));
    for _ in 0..count {
        out.extend((0..on).map(|i| {
            let phase = TAU * freq * i as f32 / RATE as f32;
            tone(phase) * envelope(i, on, 0.08) * 0.8
        }));
        out.extend(std::iter::repeat_n(0.0, off));
    }
    out
}

fn tick() -> Vec<f32> {
    let n = seconds(0.03);
    (0..n)
        .map(|i| {
            let phase = TAU * 2300.0 * i as f32 / RATE as f32;
            phase.sin() * envelope(i, n, 0.02) * 0.6
        })
        .collect()
}

fn buzz() -> Vec<f32> {
    let n = seconds(0.32);
    (0..n)
        .map(|i| {
            let phase = TAU * 140.0 * i as f32 / RATE as f32;
            // A clipped, rough low tone.
            let s = (phase.sin() * 3.0).clamp(-1.0, 1.0) * 0.5 + (2.0 * phase).sin() * 0.2;
            s * envelope(i, n, 0.03) * 0.7
        })
        .collect()
}

/// Four seconds of mains hum: 60 Hz plus harmonics, a slow flutter and a little
/// hiss. Every component completes whole cycles so the loop is seamless.
fn hum_loop() -> Vec<f32> {
    let n = seconds(4.0);
    let mut noise = 0x1234_5678u32;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let base = (TAU * 60.0 * t).sin() * 0.5
                + (TAU * 120.0 * t).sin() * 0.25
                + (TAU * 180.0 * t).sin() * 0.12;
            let flutter = 0.85 + 0.15 * (TAU * 0.5 * t).sin();
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            let hiss = (noise as f32 / u32::MAX as f32 - 0.5) * 0.04;
            (base * flutter + hiss) * 0.6
        })
        .collect()
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

    fn all() -> Vec<(&'static str, Vec<f32>)> {
        vec![
            ("chirp", chirp()),
            ("locate", sweep(380.0, 1150.0, 0.9, 9.0)),
            ("lock", pips(3, 1320.0)),
            ("chart", sweep(900.0, 260.0, 0.7, 6.0)),
            ("tick", tick()),
            ("buzz", buzz()),
            ("hum", hum_loop()),
        ]
    }

    #[test]
    fn sounds_stay_in_range_and_have_content() {
        for (name, s) in all() {
            assert!(!s.is_empty(), "{name} is empty");
            let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak <= 1.0, "{name} clips at {peak}");
            assert!(peak > 0.1, "{name} is nearly silent ({peak})");
        }
    }

    #[test]
    fn one_shots_fade_out() {
        for (name, s) in all().into_iter().filter(|(n, _)| *n != "hum") {
            let tail = s[s.len() - 20..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(tail < 0.02, "{name} ends abruptly ({tail})");
        }
    }

    #[test]
    fn hum_loops_seamlessly() {
        let h = hum_loop();
        // The sample after the last would be the first again: no jump at the seam.
        let jump = (h[0] - h[h.len() - 1]).abs();
        assert!(jump < 0.05, "loop seam jumps by {jump}");
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
