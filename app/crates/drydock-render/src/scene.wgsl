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
    sky: vec4<f32>,
    ground_color: vec4<f32>,
    horizon: vec4<f32>,
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
    light_view_proj: array<mat4x4<f32>, 4>,
    far: vec4<f32>,
    texel: vec4<f32>,
    bias: vec4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var material_texture: texture_2d<f32>;
@group(1) @binding(2) var material_sampler: sampler;
@group(2) @binding(0) var shadow_map: texture_depth_2d_array;
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

fn cascade_index(world: vec3<f32>) -> u32 {
    let depth = dot(world - camera.eye.xyz, camera.forward.xyz);
    let count = u32(shadow.params.w);
    var index = count - 1u;
    for (var i = 0u; i < 4u; i = i + 1u) {
        if (i < count && depth <= shadow.far[i]) {
            index = i;
            break;
        }
    }
    return index;
}

/// 1 where `world` sees the sun, 0 where something blocks it. Outside the shadow map the
/// point counts as lit, so the map only needs to cover the scene.
fn sun_visibility(world: vec3<f32>, normal: vec3<f32>) -> f32 {
    if (shadow.params.z < 0.5) {
        return 1.0;
    }
    let index = cascade_index(world);
    let n = normalize(normal);
    let to_sun = select(-n, n, dot(n, camera.sun_dir.xyz) >= 0.0);
    let texel = shadow.texel[index];
    let p = world + to_sun * (1.5 * texel);
    let clip = shadow.light_view_proj[index] * vec4<f32>(p, 1.0);
    if (clip.w <= 0.0) {
        return 1.0;
    }
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 ||
        ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let step = shadow.params.x;
    let depth = ndc.z - shadow.bias[index];
    var lit = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * step;
            lit = lit + textureSampleCompareLevel(
                shadow_map,
                shadow_sampler,
                uv + offset,
                i32(index),
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
                direct = direct * sun_visibility(in.world, in.normal);
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
    var origin: vec3<f32>;
    var ray: vec3<f32>;
    if (camera.parallel != 0u) {
        origin = near;
        ray = far - near;
    } else {
        origin = camera.eye.xyz;
        ray = far - origin;
    }
    if (ray.z >= 0.0) {
        discard;
    }
    if (camera.parallel == 0u && camera.eye.z <= 0.0) {
        discard;
    }
    let t = -origin.z / ray.z;
    let world = origin + ray * t;
    let lit = sun_visibility(world, vec3<f32>(0.0, 0.0, 1.0));
    let ground = camera.sun_params.z;
    let shade = mix(ground, 1.0, lit);
    let base = select(camera.background, camera.ground_color, camera.ground_color.w >= 0.5);
    return vec4<f32>(base.rgb * shade, base.a);
}

@fragment
fn fs_sky(in: GroundOut) -> @location(0) vec4<f32> {
    let h = camera.horizon.x + in.ndc.y * camera.horizon.y;
    if (h > 0.0) {
        if (camera.sky.w < 0.5) {
            discard;
        }
        return vec4<f32>(camera.sky.rgb, 1.0);
    }
    if (camera.ground_color.w < 0.5) {
        discard;
    }
    return vec4<f32>(camera.ground_color.rgb, 1.0);
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
