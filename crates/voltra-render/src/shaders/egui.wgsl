// Draws the triangles egui tessellates.
//
// Adapted from egui-wgpu's `egui.wgsl` (MIT OR Apache-2.0, © 2018-2024 Emil
// Ernerfeldt). egui-wgpu itself is pinned to wgpu 29 and cannot be used here,
// but its colour handling is the specification for what egui expects a backend
// to do, so the conversion functions are kept faithful to it. Dithering and the
// manual-filtering path are dropped; neither is required of a backend.

struct Locals {
    screen_size: vec2<f32>,
    // Pads the struct to 16 bytes. Uniform buffers are laid out in 16-byte
    // rows and a smaller struct is rejected outright.
    _padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> locals: Locals;

@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Still gamma-encoded at this point, which is what egui's blending assumes.
    @location(1) color: vec4<f32>,
};

// 0-1 linear from 0-1 sRGB gamma.
fn linear_from_gamma_rgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

// Colour arrives as four sRGB bytes packed little-endian into one u32.
fn unpack_color(color: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(color & 255u),
        f32((color >> 8u) & 255u),
        f32((color >> 16u) & 255u),
        f32((color >> 24u) & 255u),
    ) / 255.0;
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: u32,
) -> VertexOutput {
    var out: VertexOutput;
    out.uv = uv;
    out.color = unpack_color(color);
    // egui works in logical points with the origin at the top left and Y
    // running down; clip space has the origin in the middle and Y running up.
    out.position = vec4<f32>(
        2.0 * position.x / locals.screen_size.x - 1.0,
        1.0 - 2.0 * position.y / locals.screen_size.y,
        0.0,
        1.0,
    );
    return out;
}

// For an sRGB target, which converts on write. The blend has to happen in gamma
// space to match what egui expects, so the conversion is undone here first.
@fragment
fn fs_main_srgb_target(in: VertexOutput) -> @location(0) vec4<f32> {
    // egui's own textures are uploaded as plain Unorm, so the sample comes back
    // gamma-encoded rather than converted.
    let texel = textureSample(atlas, atlas_sampler, in.uv);
    let gamma = in.color * texel;
    return vec4<f32>(linear_from_gamma_rgb(gamma.rgb), gamma.a);
}

// For a target that stores exactly what it is given.
@fragment
fn fs_main_unorm_target(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, in.uv);
    return in.color * texel;
}
