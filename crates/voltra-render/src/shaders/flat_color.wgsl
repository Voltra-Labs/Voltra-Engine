// Built-in shader: vertex-coloured, textured 2D geometry through the camera.
//
// Two vertex stages, one fragment: `vs_main` reads a vertex buffer described
// by `Vertex::LAYOUT` (the @location numbers here must match its attribute
// shader_locations), `vs_quad` a quad instance (`quad.wgsl`, put before
// this file) and the vertex index.
// Group 0 must match `CameraBinding`, group 1 `texture::bind_group_layout`.
//
// The final colour is texture * vertex colour, alpha included, so binding a 1x1 white texture
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
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

fn vertex(position: vec2<f32>, color: vec4<f32>, uv: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.uv = uv;
    return out;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    return vertex(in.position, in.color, in.uv);
}

@vertex
fn vs_quad(@builtin(vertex_index) index: u32, quad: QuadInput) -> VertexOutput {
    let corner = quad_corner(quad, index);
    return vertex(corner.position, corner.color, corner.uv);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(albedo, albedo_sampler, in.uv);
    return sampled * in.color;
}
