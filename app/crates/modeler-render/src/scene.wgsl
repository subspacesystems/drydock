// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

struct Camera {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    forward: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_params: vec4<f32>,
    background: vec4<f32>,
    parallel: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
};

struct Material {
    color: vec4<f32>,
    opacity: f32,
    unlit: u32,
    has_texture: u32,
    pad: u32,
};

struct Shadow {
    light_view_proj: mat4x4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var material_texture: texture_2d<f32>;
@group(1) @binding(2) var material_sampler: sampler;
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(2) var<uniform> shadow: Shadow;

const AMBIENT: f32 = 0.4;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_face(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.world = in.position;
    out.normal = in.normal;
    out.uv = in.uv;
    return out;
}

@vertex
fn vs_shadow(in: VertexIn) -> @builtin(position) vec4<f32> {
    return camera.view_proj * vec4<f32>(in.position, 1.0);
}

/// 1 where `world` sees the sun, 0 where something blocks it. Outside the shadow map the
/// point counts as lit, so the map only needs to cover the scene.
fn sun_visibility(world: vec3<f32>) -> f32 {
    if (shadow.params.z < 0.5) {
        return 1.0;
    }
    let clip = shadow.light_view_proj * vec4<f32>(world, 1.0);
    if (clip.w <= 0.0) {
        return 1.0;
    }
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 ||
        ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let texel = shadow.params.x;
    let depth = ndc.z - shadow.params.y;
    var lit = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            lit = lit + textureSampleCompareLevel(
                shadow_map,
                shadow_sampler,
                uv + offset,
                depth,
            );
        }
    }
    return lit / 9.0;
}

@fragment
fn fs_face(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(material_texture, material_sampler, vec2<f32>(in.uv.x, 1.0 - in.uv.y));
    var rgb = material.color.rgb * texel.rgb;
    if (material.unlit == 0u) {
        let n = normalize(in.normal);
        var v: vec3<f32>;
        if (camera.parallel != 0u) {
            v = -camera.forward.xyz;
        } else {
            v = normalize(camera.eye.xyz - in.world);
        }
        let n_view = select(-n, n, dot(n, v) >= 0.0);
        if (camera.sun_dir.w > 0.5) {
            var direct = 0.0;
            if (camera.sun_dir.z > 0.0) {
                direct = max(dot(n_view, camera.sun_dir.xyz), 0.0);
                direct = direct * sun_visibility(in.world);
            }
            let shade = min(1.0, camera.sun_params.y + camera.sun_params.x * direct);
            rgb = rgb * shade;
        } else {
            let shade = abs(dot(n, v));
            rgb = rgb * (AMBIENT + (1.0 - AMBIENT) * shade);
        }
    }
    let alpha = material.opacity * texel.a;
    return vec4<f32>(rgb, alpha);
}

struct GroundOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_ground(@builtin(vertex_index) index: u32) -> GroundOut {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: GroundOut;
    let p = corners[index];
    out.clip = vec4<f32>(p, 0.0, 1.0);
    out.ndc = p;
    return out;
}

@fragment
fn fs_ground(in: GroundOut) -> @location(0) vec4<f32> {
    let near4 = camera.inv_view_proj * vec4<f32>(in.ndc, 0.0, 1.0);
    let far4 = camera.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let near = near4.xyz / near4.w;
    let far = far4.xyz / far4.w;
    let ray = far - near;
    if (abs(ray.z) < 1e-12) {
        discard;
    }
    let t = -near.z / ray.z;
    if (t <= 0.0) {
        discard;
    }
    let world = near + ray * t;
    let lit = sun_visibility(world);
    let ground = camera.sun_params.z;
    let shade = mix(ground, 1.0, lit);
    return vec4<f32>(camera.background.rgb * shade, camera.background.a);
}

struct LineIn {
    @location(0) position: vec3<f32>,
};

@vertex
fn vs_line(in: LineIn) -> @builtin(position) vec4<f32> {
    return camera.view_proj * vec4<f32>(in.position, 1.0);
}

@vertex
fn vs_extra_line(in: LineIn) -> @builtin(position) vec4<f32> {
    var clip = camera.view_proj * vec4<f32>(in.position, 1.0);
    clip.z = clamp(clip.z, 0.0, clip.w);
    return clip;
}

@fragment
fn fs_line() -> @location(0) vec4<f32> {
    return vec4<f32>(material.color.rgb, material.opacity);
}
