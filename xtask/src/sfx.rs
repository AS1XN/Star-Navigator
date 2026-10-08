//! Sound effects: cuts the Freesound clips cached in `data/raw/sfx/<id>.ogg`
//! into the short mono WAVs the viewer plays (`assets/sounds/`).
//!
//!   sfx            write every cut in `CUTS`
//!   sfx probe ID   print a loudness and pitch timeline to find cut points

use std::{fs, path::Path};

use crate::{Result, root};

/// One output file: a window of a source clip with fades, normalised to `peak`.
struct Cut {
    out: &'static str,
    source: u32,
    start: f32,
    len: f32,
    fade_in: f32,
    fade_out: f32,
    peak: f32,
}

/// Sources (all CC0, credited on the credits page):
/// 819347 "Onboard Targeting Computer x5" by harrisonlace (variation 2 at 8.2 s),
/// 262859 "Blip 7" by deleted_user_2906614, 584918 "Scanner Sci-Fi" by
/// smokinghotdog, 176238 "Sci-fi_short_error" by melissapons.
const CUTS: &[Cut] = &[
    Cut {
        out: "locate.wav",
        source: 819347,
        start: 8.2,
        len: 2.4,
        fade_in: 0.01,
        fade_out: 0.3,
        peak: 0.8,
    },
    Cut {
        out: "lock.wav",
        source: 262859,
        start: 0.0,
        len: 1.15,
        fade_in: 0.003,
        fade_out: 0.05,
        peak: 0.8,
    },
    Cut {
        out: "select.wav",
        source: 262859,
        start: 0.0,
        len: 0.24,
        fade_in: 0.003,
        fade_out: 0.02,
        peak: 0.8,
    },
    Cut {
        out: "click.wav",
        source: 262859,
        start: 0.0,
        len: 0.09,
        fade_in: 0.003,
        fade_out: 0.02,
        peak: 0.8,
    },
    Cut {
        out: "chart_in.wav",
        source: 584918,
        start: 0.4,
        len: 1.95,
        fade_in: 0.02,
        fade_out: 0.25,
        peak: 0.8,
    },
    Cut {
        out: "chart_out.wav",
        source: 584918,
        start: 2.9,
        len: 1.95,
        fade_in: 0.02,
        fade_out: 0.25,
        peak: 0.8,
    },
    Cut {
        out: "nomatch.wav",
        source: 176238,
        start: 0.0,
        len: 0.58,
        fade_in: 0.003,
        fade_out: 0.05,
        peak: 0.8,
    },
];

pub fn sfx(args: &[String]) -> Result {
    match args {
        [cmd, id] if cmd == "probe" => probe(id.parse()?),
        [] => build(),
        _ => Err("usage: cargo xtask sfx [probe ID]".into()),
    }
}

/// Decodes an Ogg Vorbis file into mono samples and its sample rate.
fn decode(id: u32) -> Result<(Vec<f32>, u32)> {
    let path = root().join(format!("data/raw/sfx/{id}.ogg"));
    let file = fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut reader = lewton::inside_ogg::OggStreamReader::new(file)?;
    let channels = reader.ident_hdr.audio_channels as usize;
    let rate = reader.ident_hdr.audio_sample_rate;
    let mut mono = Vec::new();
    while let Some(packet) = reader.read_dec_packet_itl()? {
        for frame in packet.chunks(channels) {
            let sum: f32 = frame.iter().map(|s| *s as f32 / 32768.0).sum();
            mono.push(sum / channels as f32);
        }
    }
    Ok((mono, rate))
}

fn probe(id: u32) -> Result {
    let (s, rate) = decode(id)?;
    let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    println!("{id}: {:.2} s at {rate} Hz, peak {peak:.2}", s.len() as f32 / rate as f32);
    let hop = rate as usize / 20; // 50 ms
    let win = 2048.min(hop * 2);
    for (k, start) in (0..s.len().saturating_sub(win)).step_by(hop).enumerate() {
        let frame = &s[start..start + win];
        let rms = (frame.iter().map(|v| v * v).sum::<f32>() / win as f32).sqrt();
        let db = 20.0 * rms.max(1e-6).log10();
        let bar = "#".repeat(((db + 60.0).max(0.0) / 1.5) as usize);
        println!("{:6.2}s {db:6.1} dB {:5.0} Hz {bar}", k as f32 * 0.05, peak_freq(frame, rate));
    }
    Ok(())
}

/// Strongest frequency in a frame (Hann window, DFT on a coarse grid).
fn peak_freq(frame: &[f32], rate: u32) -> f32 {
    let n = frame.len() as f32;
    let mut best = (0.0, 0.0);
    let mut f = 100.0;
    while f < 8000.0 {
        let w = std::f32::consts::TAU * f / rate as f32;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, x) in frame.iter().enumerate() {
            let hann = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n).cos();
            re += x * hann * (w * i as f32).cos();
            im += x * hann * (w * i as f32).sin();
        }
        let mag = re * re + im * im;
        if mag > best.1 {
            best = (f, mag);
        }
        f *= 1.03;
    }
    best.0
}

fn build() -> Result {
    let dir = root().join("assets/sounds");
    fs::create_dir_all(&dir)?;
    for cut in CUTS {
        let (s, rate) = decode(cut.source)?;
        let at = |t: f32| ((t * rate as f32) as usize).min(s.len());
        let window = &s[at(cut.start)..at(cut.start + cut.len)];
        let n = window.len();
        let (fi, fo) = (cut.fade_in * rate as f32, cut.fade_out * rate as f32);
        let out: Vec<f32> = window
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let edge = (i as f32 / fi.max(1.0)).min((n - 1 - i) as f32 / fo.max(1.0));
                v * edge.min(1.0)
            })
            .collect();
        let top = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
        let out: Vec<f32> = out.iter().map(|v| v * cut.peak / top).collect();
        let path = dir.join(cut.out);
        write_wav(&path, &out, rate)?;
        let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        eprintln!("{}: {:.2} s, peak {peak:.2}", cut.out, n as f32 / rate as f32);
    }
    Ok(())
}

/// Mono 16-bit PCM WAV.
fn write_wav(path: &Path, samples: &[f32], rate: u32) -> Result {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    fs::write(path, out)?;
    Ok(())
}
