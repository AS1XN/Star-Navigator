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
    cam_pos: vec4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_fwd: vec4<f32>,
    sky: vec4<f32>,
    sky2: vec4<f32>,
    sky3: vec4<f32>,
    wedge_color: vec4<f32>,
    now_color: vec4<f32>,
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

const PI: f32 = 3.14159265;
const HALF_PI: f32 = 1.57079633;
/// Sphere the overlay sits on in the 3D field: 100 pc around Sol.
const FIELD_RADIUS: f32 = 5.0;

fn wrap_pi(a: f32) -> f32 {
    return a - 6.2831853 * floor((a + PI) / 6.2831853);
}

// Anti-aliased line where `v` crosses zero, about `w` pixels wide.
fn line(v: f32, w: f32) -> f32 {
    return 1.0 - smoothstep(0.0, max(fwidth(v), 1e-5) * w, abs(v));
}

// The tonight overlay for a sky direction, Yavin-table style: a wedge of the sky
// that crosses the meridian during tonight's dark hours, out to the observer's
// horizon limit; a bright slice on the meridian right now, which sweeps through
// the wedge as the night goes on; and a thin line along the true horizon.
// `fill` scales the washes (the 3D field keeps only the outlines).
fn zone(ra: f32, dec: f32, fill: f32) -> vec3<f32> {
    let lat = h.sky.x;
    let ha = wrap_pi(h.sky.y - ra);
    let aha = abs(ha);
    let up = step(lat - HALF_PI, dec) * step(dec, lat + HALF_PI);
    let night = abs(wrap_pi(ra - h.sky3.x));
    let half = h.sky3.y;
    let wedge = step(night, half) * up;
    let now = step(aha, PI / 12.0) * up;
    let sin_alt = sin(lat) * sin(dec) + cos(lat) * cos(dec) * cos(ha);

    // No side edges when the night wraps all the way round.
    let sides = step(half, PI - 0.01);
    let wedge_edge = (line(night - half, 1.5) * up * sides
        + line(dec - (lat - HALF_PI), 1.5) * step(night, half)) * 0.8;
    let now_edge = line(aha - PI / 12.0, 1.5) * up;
    let horizon = line(sin_alt, 1.2) * 0.45;

    return h.wedge_color.rgb * (wedge * 0.07 * fill + wedge_edge * 0.55 * (1.0 - now))
        + h.now_color.rgb * (now * 0.16 * fill + now_edge * 0.8 + horizon);
}

fn sky_overlay(uv: vec2<f32>) -> vec3<f32> {
    let ndc = vec2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let dir = normalize(h.cam_fwd.xyz + ndc.x * h.cam_right.xyz + ndc.y * h.cam_up.xyz);
    let o = h.cam_pos.xyz;
    let chart = h.sky.w;
    let unfold = h.sky2.x;

    // The chart table: the XZ plane, pole at the centre.
    let t_plane = -o.y / select(dir.y, 1e-6, abs(dir.y) < 1e-6);
    let p = o + dir * t_plane;
    let r = length(p.xz) / h.sky2.z;
    let on_table = step(0.0, t_plane) * step(r, 1.0);
    let table = zone(atan2(-p.z, p.x), h.sky2.y * (HALF_PI - r * PI), 1.0) * on_table;

    // The globe, or the dome around Sol in the 3D field: the near side from
    // outside, the far side from inside.
    let radius = mix(h.sky2.w, FIELD_RADIUS, unfold);
    let b = dot(o, dir);
    let c = dot(o, o) - radius * radius;
    let disc = b * b - c;
    let sq = sqrt(max(disc, 0.0));
    let t_sphere = select(-b - sq, -b + sq, c < 0.0);
    let n = (o + dir * t_sphere) / radius;
    let on_sphere = step(0.0, disc) * step(0.0, t_sphere);
    let shell = zone(atan2(-n.z, n.x), asin(clamp(n.y, -1.0, 1.0)), 1.0 - unfold) * on_sphere;

    return table * chart + shell * (1.0 - chart) * mix(1.0, 0.6, unfold);
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

    // The tonight overlay keeps its own colours, so it goes in after the tint.
    if h.sky.z > 0.0 {
        c += sky_overlay(uv) * h.sky.z;
    }

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
