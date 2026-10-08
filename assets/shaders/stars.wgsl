// Draws every star as a screen-space dot. Each star is a quad whose four vertices
// share the star's position; the vertex shader pushes them out by `corner` pixels.

#import bevy_pbr::{
    mesh_functions::{get_world_from_local, mesh_position_local_to_world},
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}

struct StarSettings {
    tint: vec4<f32>,
    mag_limit: f32,
    size_scale: f32,
    back_fade: f32,
    brightness: f32,
    unfold: f32,
    chart: f32,
    chart_pole: f32,
    _pad: f32,
}

// Flat chart: must match `chart_position` in main.rs.
const GLOBE_RADIUS: f32 = 4.0;
const CHART_RADIUS: f32 = 6.0;

fn chart_position(globe: vec3<f32>, pole: f32) -> vec3<f32> {
    let g = globe / GLOBE_RADIUS;
    let dec = asin(clamp(g.y, -1.0, 1.0));
    let ra = atan2(-g.z, g.x);
    let r = (1.5707964 - dec * pole) / 3.1415927 * CHART_RADIUS;
    return vec3(r * cos(ra), 0.0, -r * sin(ra));
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> settings: StarSettings;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // Position on the celestial globe.
    @location(0) position: vec3<f32>,
    // True 3D position around Sol (carried in the normal slot).
    @location(1) field: vec3<f32>,
    @location(2) corner: vec2<f32>,
    // x: apparent magnitude, y: 1 if the distance is known
    @location(3) params: vec2<f32>,
    @location(5) color: vec4<f32>,
};

struct StarOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) corner: vec2<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vertex(v: Vertex) -> StarOut {
    var out: StarOut;
    let excess = settings.mag_limit - v.params.x;
    if excess < 0.0 {
        // Fainter than the limit: emit a degenerate vertex outside the clip volume.
        out.clip = vec4(2.0, 2.0, 2.0, 1.0);
        return out;
    }

    let world_from_local = get_world_from_local(v.instance_index);
    let spatial = mix(v.position, v.field, settings.unfold);
    let local = mix(spatial, chart_position(v.position, settings.chart_pole), settings.chart);
    let world = mesh_position_local_to_world(world_from_local, vec4(local, 1.0)).xyz;
    var clip = position_world_to_clip(world);

    let size_px = clamp(1.5 + excess * 0.75, 1.5, 9.0) * settings.size_scale;
    clip = vec4(clip.xy + v.corner * size_px * 2.0 / view.viewport.zw * clip.w, clip.zw);
    out.clip = clip;

    // Stars on the far side of the globe show through, dimmed.
    let facing = dot(normalize(world), normalize(view.world_position));
    var fade = mix(settings.back_fade, 1.0, smoothstep(-0.15, 0.15, facing));
    // In the 3D field there is no far side; stars without a distance fade out.
    fade = mix(fade, v.params.y, settings.unfold);
    // The chart is flat: every star faces up.
    fade = mix(fade, 1.0, settings.chart);
    let intensity = clamp(0.3 + excess * 0.4, 0.3, 4.0) * settings.brightness;

    out.corner = v.corner;
    out.color = v.color.rgb * settings.tint.rgb * intensity * fade;
    return out;
}

@fragment
fn fragment(in: StarOut) -> @location(0) vec4<f32> {
    let r2 = dot(in.corner, in.corner);
    if r2 > 1.0 {
        discard;
    }
    let glow = exp(-r2 * 3.5);
    return vec4(in.color * glow, glow);
}
