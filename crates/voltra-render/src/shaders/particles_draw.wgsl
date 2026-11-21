// Built-in shader: one emitter's GPU particles as quads, one instance each.
//
// No vertex buffer: the instance is the particle's slot, the vertex index
// picks a corner of two triangles. Group 0 must match `CameraBinding`,
// group 1 `texture::bind_group_layout`, group 2 `ParticlePipelines`' draw
// layout. Colour is texture * tint, as `flat_color.wgsl`; a dead particle
// collapses to a point and draws nothing.

struct Camera {
    view_proj: mat4x4<f32>,
}

struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    age: f32,
    lifetime: f32,
    _pad: vec2<f32>,
}

struct Look {
    // The emitter's world matrix for a local-space emitter, identity for a
    // world-space one: columns x, y, translation.
    place_x: vec4<f32>,
    place_y: vec4<f32>,
    place_t: vec4<f32>,
    // size at birth, size at death
    size: vec4<f32>,
    color_start: vec4<f32>,
    color_end: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var albedo: texture_2d<f32>;

@group(1) @binding(1)
var albedo_sampler: sampler;

@group(2) @binding(0)
var<storage, read> particles: array<Particle>;

@group(2) @binding(1)
var<uniform> look: Look;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

// Top-left, bottom-left, bottom-right, top-right as a sprite's corners are,
// drawn as triangles 0 1 2 and 0 2 3.
const CORNER = array<vec2<f32>, 4>(
    vec2<f32>(-0.5, 0.5),
    vec2<f32>(-0.5, -0.5),
    vec2<f32>(0.5, -0.5),
    vec2<f32>(0.5, 0.5),
);
const UV = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(1.0, 0.0),
);
const ORDER = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> VertexOutput {
    let p = particles[instance];
    let corner = ORDER[vertex];
    var out: VertexOutput;
    var t = 1.0;
    if (p.lifetime > 0.0) {
        t = clamp(p.age / p.lifetime, 0.0, 1.0);
    }
    var size = max(mix(look.size.x, look.size.y, t), 0.0);
    if (p.age >= p.lifetime) {
        size = 0.0;
    }
    let local = p.position + CORNER[corner] * size;
    let world = look.place_x.xy * local.x + look.place_y.xy * local.y + look.place_t.xy;
    out.clip_position = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.color = mix(look.color_start, look.color_end, t);
    out.uv = UV[corner];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(albedo, albedo_sampler, in.uv);
    return sampled * in.color;
}
