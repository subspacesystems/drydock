// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

struct Camera {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    forward: vec4<f32>,
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

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var material_texture: texture_2d<f32>;
@group(1) @binding(2) var material_sampler: sampler;

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
        let shade = abs(dot(n, v));
        rgb = rgb * (AMBIENT + (1.0 - AMBIENT) * shade);
    }
    let alpha = material.opacity * texel.a;
    return vec4<f32>(rgb, alpha);
}

struct LineIn {
    @location(0) position: vec3<f32>,
};

@vertex
fn vs_line(in: LineIn) -> @builtin(position) vec4<f32> {
    return camera.view_proj * vec4<f32>(in.position, 1.0);
}

@fragment
fn fs_line() -> @location(0) vec4<f32> {
    return vec4<f32>(material.color.rgb, material.opacity);
}
