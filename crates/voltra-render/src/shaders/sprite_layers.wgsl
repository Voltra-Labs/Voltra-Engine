// Built-in shader: the scene's light layers, for lights that leave some out.
//
// The sprite batch drawn again, range for range in painter's order, into an
// R32Uint buffer: each texel holds the light layers of whatever was drawn
// last there, inverted — the layers it is NOT on — so the zero the buffer is
// cleared to puts the background on every layer. Texels the colour leaves
// less than half covered are skipped, as in `sprite_normals.wgsl`.
//
// Vertex layout is `Vertex::LAYOUT` (`vs_layers`) or a quad instance
// (`vs_layers_quad`, `quad.wgsl`); group 0 `CameraBinding`, group 1
// `texture::bind_group_layout` (the colour, for its coverage), group 2 the
// range's layers. Read back by `lights.wgsl`.

struct Camera {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var albedo: texture_2d<f32>;
@group(1) @binding(1)
var albedo_sampler: sampler;

struct Layers {
    // The range's light layers; the rest pads to sixteen bytes.
    bits: vec4<u32>,
}

@group(2) @binding(0)
var<uniform> range: Layers;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
}

fn vertex(position: vec2<f32>, color: vec4<f32>, uv: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    out.alpha = color.a;
    return out;
}

@vertex
fn vs_layers(in: VertexInput) -> VertexOutput {
    return vertex(in.position, in.color, in.uv);
}

@vertex
fn vs_layers_quad(@builtin(vertex_index) index: u32, quad: QuadInput) -> VertexOutput {
    let corner = quad_corner(quad, index);
    return vertex(corner.position, corner.color, corner.uv);
}

@fragment
fn fs_layers(in: VertexOutput) -> @location(0) u32 {
    let covered = textureSample(albedo, albedo_sampler, in.uv).a * in.alpha;
    if covered < 0.5 {
        discard;
    }
    return ~range.bits.x;
}
