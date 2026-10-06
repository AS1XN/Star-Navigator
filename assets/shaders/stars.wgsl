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
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> settings: StarSettings;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) corner: vec2<f32>,
    // x: apparent magnitude
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
    let world = mesh_position_local_to_world(world_from_local, vec4(v.position, 1.0)).xyz;
    var clip = position_world_to_clip(world);

    let size_px = clamp(1.5 + excess * 0.75, 1.5, 9.0) * settings.size_scale;
    clip = vec4(clip.xy + v.corner * size_px * 2.0 / view.viewport.zw * clip.w, clip.zw);
    out.clip = clip;

    // Stars on the far side of the globe show through, dimmed.
    let facing = dot(normalize(world), normalize(view.world_position));
    let fade = mix(settings.back_fade, 1.0, smoothstep(-0.15, 0.15, facing));
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
