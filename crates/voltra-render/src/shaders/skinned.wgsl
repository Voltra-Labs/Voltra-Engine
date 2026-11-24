// Built-in shader: a sprite mesh deformed by bones — linear blend skinning.
//
// Group 0 must match `CameraBinding`, group 1 `texture::bind_group_layout`,
// group 2 `skinning::SkinPipeline`'s pose layout and `skinning::mesh`'s
// `SkinUniform`. Each vertex is its bind-pose position carried by up to four
// bones' matrices (bind pose to world), weighted; slot 64 is the mesh's own
// placement, for a vertex no bone holds. Colour is texture * tint, as
// `flat_color.wgsl`.

struct Camera {
    view_proj: mat4x4<f32>,
}

struct Bone {
    // x axis .xy, y axis .zw
    axes: vec4<f32>,
    // translation .xy
    translation: vec4<f32>,
}

struct Skin {
    tint: vec4<f32>,
    // Must match `skinning::MAX_BONES` + 1.
    bones: array<Bone, 65>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var albedo: texture_2d<f32>;

@group(1) @binding(1)
var albedo_sampler: sampler;

@group(2) @binding(0)
var<uniform> skin: Skin;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) bones: vec4<u32>,
    @location(3) weights: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

fn carried(index: u32, p: vec2<f32>) -> vec2<f32> {
    let bone = skin.bones[min(index, 64u)];
    return bone.axes.xy * p.x + bone.axes.zw * p.y + bone.translation.xy;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // Written out rather than looped over the four: a loop indexing a vector
    // by a runtime value is what FXC (D3D12) fails to unroll.
    let p = in.position;
    let world = carried(in.bones.x, p) * in.weights.x
        + carried(in.bones.y, p) * in.weights.y
        + carried(in.bones.z, p) * in.weights.z
        + carried(in.bones.w, p) * in.weights.w;
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.color = skin.tint;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(albedo, albedo_sampler, in.uv) * in.color;
}
