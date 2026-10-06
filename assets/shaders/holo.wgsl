// Analog hologram post-process: palette tint, scanlines, rolling interference,
// line jitter and tears, flicker, chromatic fringe, LED dot mask, grain, vignette.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Holo {
    tint: vec4<f32>,
    resolution: vec2<f32>,
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
    _pad: vec2<f32>,
}

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> h: Holo;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}

fn noise(x: f32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    return mix(hash(vec2(i, 0.0)), hash(vec2(i + 1.0, 0.0)), f * f * (3.0 - 2.0 * f));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let t = h.time;
    var uv = in.uv;
    var px = uv * h.resolution;

    // Per-line jitter, re-rolled 30 times a second.
    let row = floor(px.y / 2.0);
    var shift = (hash(vec2(row, floor(t * 30.0))) - 0.5) * h.jitter;

    // Occasional tear: a horizontal band that slips sideways for a moment.
    let slot = floor(t * 4.0);
    let tearing = step(0.96, hash(vec2(slot, 3.7)));
    let band_center = hash(vec2(slot, 9.1));
    let in_band = 1.0 - smoothstep(0.0, 0.035, abs(uv.y - band_center));
    shift += tearing * in_band * h.tear * (hash(vec2(slot, row)) * 0.5 + 0.5);

    uv.x += shift / h.resolution.x;

    // Chromatic fringe grows toward the edges.
    let d = uv - 0.5;
    let off = d * length(d) * 2.0 * h.fringe / h.resolution.x;
    var c = vec3(
        textureSample(screen, screen_sampler, uv + off).r,
        textureSample(screen, screen_sampler, uv).g,
        textureSample(screen, screen_sampler, uv - off).b,
    );

    // Monochrome palette: keep luminance, take hue from the tint.
    let weights = vec3(0.2126, 0.7152, 0.0722);
    let lum = dot(c, weights);
    let tint = h.tint.rgb / max(dot(h.tint.rgb, weights), 0.001);
    c = mix(c, lum * tint, h.mono);

    // Scanlines, slowly rolling downward.
    let scan = 0.5 + 0.5 * cos((px.y - t * h.roll * 30.0) * 6.2831853 / h.scan_period);
    c *= 1.0 - h.scanlines * (1.0 - scan);

    // A soft brighter interference band drifting down the screen.
    let band = abs(fract(uv.y - fract(t * 0.06) + 0.5) - 0.5);
    c *= 1.0 + h.roll * 0.3 * exp(-band * band * 300.0);

    // LED dot matrix.
    let cell = fract(px / 3.0) - 0.5;
    let dots = 1.0 - smoothstep(0.12, 0.24, dot(cell, cell));
    c *= mix(1.0, 0.4 + 0.9 * dots, h.mask);

    // Brightness flicker with rare dropouts.
    let dropout = step(0.993, hash(vec2(floor(t * 20.0), 1.3))) * 0.5;
    c *= 1.0 - h.flicker * (0.6 * noise(t * 8.0) + 0.4 * noise(t * 27.0)) - dropout * h.flicker;

    // Grain, a little stronger on lit areas like film.
    c += (hash(px + fract(t * 7.3) * 113.0) - 0.5) * h.grain * (0.25 + lum);

    // Vignette.
    c *= max(1.0 - h.vignette * dot(d, d) * 2.0, 0.0);

    return vec4(max(c, vec3(0.0)), 1.0);
}
