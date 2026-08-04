// Built-in shader: vertex-coloured, textured 2D geometry through the camera.
//
// Positions arrive from a vertex buffer described by `Vertex::LAYOUT`; the
// @location numbers here must match the attribute shader_locations there.
// Group 0 must match `CameraBinding`, group 1 `texture::bind_group_layout`.
//
// The final colour is texture * vertex colour, so binding a 1x1 white texture
// (`Texture::white`) makes this behave exactly like a flat-colour shader.
// That is what saves having a second pipeline for untextured geometry.

struct Camera {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var albedo: texture_2d<f32>;

@group(1) @binding(1)
var albedo_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(albedo, albedo_sampler, in.uv);
    return vec4<f32>(sampled.rgb * in.color, sampled.a);
}
