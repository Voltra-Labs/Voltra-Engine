// Built-in shader: one emitter's GPU particles as quads, one instance each.
//
// No vertex buffer: the instance is the particle's slot, the vertex index
// picks a corner of two triangles. Group 0 must match `CameraBinding`,
// group 1 `texture::bind_group_layout`, group 2 `ParticlePipelines`' draw
// layout. Colour is texture * tint, as `flat_color.wgsl`; a dead particle
// collapses to a point and draws nothing. Size, colour, alpha and the
// flipbook's frames are read from the emitter's tables (`particles::tables`),
// as the CPU's `particles::mesh` computes them.

struct Camera {
    view_proj: mat4x4<f32>,
}

struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    age: f32,
    lifetime: f32,
    rotation: f32,
    spin: f32,
}

struct Look {
    // The emitter's world matrix for a local-space emitter, identity for a
    // world-space one: columns x, y, translation.
    place_x: vec4<f32>,
    place_y: vec4<f32>,
    place_t: vec4<f32>,
    // frames, timing (0 whole texture, 1 over life, 2 per second), cycles
    // or fps, unused
    frames: vec4<f32>,
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

@group(2) @binding(2)
var tables: texture_2d<f32>;

// Must match `particles::tables::TABLE_SAMPLES`.
const SAMPLES: i32 = 64;

fn table_at(row: i32, t: f32) -> vec4<f32> {
    let x = clamp(t, 0.0, 1.0) * f32(SAMPLES - 1);
    let i = min(i32(floor(x)), SAMPLES - 2);
    let a = textureLoad(tables, vec2<i32>(i, row), 0);
    let b = textureLoad(tables, vec2<i32>(i + 1, row), 0);
    return mix(a, b, x - f32(i));
}

// Which of `count` frames a particle shows: the CPU's
// `particles::flipbook::index`.
fn frame_of(p: Particle, t: f32, count: i32) -> i32 {
    var steps = 0.0;
    if (look.frames.y > 1.5) {
        steps = floor(p.age * look.frames.z);
    } else {
        steps = floor(t * look.frames.z * f32(count));
    }
    let wrapped = i32(steps) % count;
    return select(wrapped, wrapped + count, wrapped < 0);
}

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
    let curves = table_at(1, t);
    var size = max(curves.x, 0.0);
    if (p.age >= p.lifetime) {
        size = 0.0;
    }
    let c = cos(p.rotation);
    let s = sin(p.rotation);
    let k = CORNER[corner] * size;
    let local = p.position + vec2<f32>(k.x * c - k.y * s, k.x * s + k.y * c);
    let world = look.place_x.xy * local.x + look.place_y.xy * local.y + look.place_t.xy;
    out.clip_position = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    var color = table_at(0, t);
    color.a = color.a * curves.z;
    out.color = color;
    var uv = UV[corner];
    let count = i32(look.frames.x);
    if (count > 0 && look.frames.y > 0.5) {
        let rect = textureLoad(tables, vec2<i32>(frame_of(p, t, count), 2), 0);
        uv = mix(rect.xy, rect.zw, uv);
    }
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(albedo, albedo_sampler, in.uv);
    return sampled * in.color;
}
