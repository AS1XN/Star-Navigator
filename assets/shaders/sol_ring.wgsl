// Dashed Sol reference ring. It is UI, so it is drawn after holo.wgsl and keeps
// its accent colour; instead it repeats the hologram's scanlines, line jitter,
// flicker, LED dots and grain itself, in screen pixels so they line up.

#import bevy_ui::ui_vertex_output::UiVertexOutput

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

struct Ring {
    color: vec4<f32>,
    // Ring radius as a fraction of the node width.
    radius: f32,
    // Physical pixels per logical UI pixel.
    px_scale: f32,
    glow: f32,
    dashes: f32,
}

@group(1) @binding(0) var<uniform> h: Holo;
@group(1) @binding(1) var<uniform> ring: Ring;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}

fn noise(x: f32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    return mix(hash(vec2(i, 0.0)), hash(vec2(i + 1.0, 0.0)), f * f * (3.0 - 2.0 * f));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let t = h.time;
    let px = in.position.xy;
    let s = ring.px_scale;

    // Same per-line jitter as the hologram.
    let row = floor(px.y / 2.0);
    let shift = (hash(vec2(row, floor(t * 30.0))) - 0.5) * h.jitter;
    var p = (in.uv - 0.5) * in.size;
    p.x += shift;

    let d = abs(length(p) - ring.radius * in.size.x);
    let core = 1.0 - smoothstep(0.5 * s, 1.2 * s, d);
    let halo = exp(-pow(d / (2.5 * s), 2.0)) * ring.glow * 0.5;

    let phase = fract(atan2(p.y, p.x) / 6.2831853 * ring.dashes);
    let dash = smoothstep(0.0, 0.08, phase) * (1.0 - smoothstep(0.5, 0.58, phase));

    var a = (core + halo) * dash;

    let scan = 0.5 + 0.5 * cos((px.y - t * h.roll * 30.0) * 6.2831853 / h.scan_period);
    a *= 1.0 - h.scanlines * (1.0 - scan);

    let uv_y = px.y / h.resolution.y;
    let band = abs(fract(uv_y - fract(t * 0.06) + 0.5) - 0.5);
    a *= 1.0 + h.roll * 0.3 * exp(-band * band * 300.0);

    let cell = fract(px / 3.0) - 0.5;
    let dots = 1.0 - smoothstep(0.12, 0.24, dot(cell, cell));
    a *= mix(1.0, 0.4 + 0.9 * dots, h.mask);

    let dropout = step(0.993, hash(vec2(floor(t * 20.0), 1.3))) * 0.5;
    a *= 1.0 - h.flicker * (0.6 * noise(t * 8.0) + 0.4 * noise(t * 27.0)) - dropout * h.flicker;

    a += (hash(px + fract(t * 7.3) * 113.0) - 0.5) * h.grain * (0.25 + a) * step(0.02, a);

    return vec4(ring.color.rgb, clamp(a * ring.color.a, 0.0, 1.0));
}
