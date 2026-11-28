// The surface prelude, instanced: the bindings, the vertex stage, what a
// material's `surface()` is handed and returns, and the standard surface —
// glTF 2.0's metallic-roughness inputs. It follows frame3d.wgsl; its
// lighting is surface3d_light.wgsl and its fog environment3d.wgsl, which
// follow it (`shader::SURFACE_3D`).
// `render3d::surface::compose` puts the material's own source before them
// and the fragment entry points after.
//
// Every texture is sampled before any choice is made: Chromium's Tint
// refuses `textureSample` (and `fwidth`) after a branch on a non-uniform
// value, and then the whole module with it. Choices are `select`s.

// Filament's floor on perceptual roughness: below it a highlight is
// narrower than a half float holds.
const MIN_ROUGHNESS: f32 = 0.045;

struct StandardBlock {
    base_color: vec4<f32>,
    // rgb: the emissive colour times its strength.
    emissive: vec4<f32>,
    // metallic, roughness, normal scale, occlusion strength.
    factors: vec4<f32>,
    // x: the mask's cutoff; y: 1 unlit; z: 1 with a normal map.
    flags: vec4<f32>,
    // The UV transform's rows: uv' = (dot(x.xy, uv) + x.z, dot(y.xy, uv) + y.z).
    uv_x: vec4<f32>,
    uv_y: vec4<f32>,
};

@group(1) @binding(0) var<uniform> standard: StandardBlock;
@group(1) @binding(1) var surface_sampler: sampler;
// Colour, through the sRGB view: linear when sampled.
@group(1) @binding(2) var base_color_texture: texture_2d<f32>;
// Data, through the raw view: G roughness, B metallic.
@group(1) @binding(3) var metallic_roughness_texture: texture_2d<f32>;
// Data, through the raw view: a tangent-space normal.
@group(1) @binding(4) var normal_texture: texture_2d<f32>;
// Data, through the raw view: R occlusion.
@group(1) @binding(5) var occlusion_texture: texture_2d<f32>;
// Colour, through the sRGB view.
@group(1) @binding(6) var emissive_texture: texture_2d<f32>;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};

struct InstanceIn {
    @location(4) model_x: vec4<f32>,
    @location(5) model_y: vec4<f32>,
    @location(6) model_z: vec4<f32>,
    // The cofactor matrix's rows; `normal_x.w` is the sign of the model's
    // determinant (1 for a flat model), which turns the bitangent.
    @location(7) normal_x: vec4<f32>,
    @location(8) normal_y: vec4<f32>,
    @location(9) normal_z: vec4<f32>,
    @location(10) tint: vec4<f32>,
    // The lights that reach it (`LightList3D::packed`): eight point and
    // spot light indices, a byte each, in x and y; the directional lights'
    // bits in z.
    @location(11) lights: vec3<u32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Not unit length: the cofactor matrix scales it.
    @location(1) normal: vec3<f32>,
    // xyz: the tangent in the world; w: its handedness, turned by the
    // model's.
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) tint: vec4<f32>,
    // The instance's light list, the same at every vertex: `either` is
    // what WebGL2's GLSL can say.
    @location(5) @interpolate(flat, either) lights: vec3<u32>,
};

@vertex
fn vs_main(vertex: VertexIn, instance: InstanceIn) -> VertexOut {
    let local = vec4<f32>(vertex.position, 1.0);
    let world = vec3<f32>(
        dot(instance.model_x, local),
        dot(instance.model_y, local),
        dot(instance.model_z, local),
    );
    var out: VertexOut;
    out.clip = frame.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.normal = vec3<f32>(
        dot(instance.normal_x.xyz, vertex.normal),
        dot(instance.normal_y.xyz, vertex.normal),
        dot(instance.normal_z.xyz, vertex.normal),
    );
    let tangent = vertex.tangent.xyz;
    out.tangent = vec4<f32>(
        dot(instance.model_x.xyz, tangent),
        dot(instance.model_y.xyz, tangent),
        dot(instance.model_z.xyz, tangent),
        vertex.tangent.w * instance.normal_x.w,
    );
    out.uv = vertex.uv;
    out.tint = instance.tint;
    out.lights = instance.lights;
    return out;
}

// `v` made unit length, or `fallback` where it has none.
fn unit_or(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let length_squared = dot(v, v);
    return select(fallback, v * inverseSqrt(max(length_squared, 1.0e-30)), length_squared > 1.0e-30);
}

// What a material's `surface()` is handed, for one fragment.
struct SurfaceInput {
    // The fragment's place in the target, in pixels, and its depth.
    position: vec4<f32>,
    // Where it is in the world.
    world: vec3<f32>,
    // The surface's normal, unit length, turned toward the side seen: a
    // back face's is the front's turned round.
    normal: vec3<f32>,
    // The tangent frame, turned with the normal; zero where the mesh has
    // no tangents.
    tangent: vec3<f32>,
    bitangent: vec3<f32>,
    // The mesh's texture coordinate, before the standard block's UV
    // transform (`surface_uv(in)` is after).
    uv: vec2<f32>,
    // The mesh instance's colour.
    tint: vec4<f32>,
    // Toward the eye, unit length.
    view: vec3<f32>,
    // Whether the front face is seen.
    front_facing: bool,
    // Seconds since the game started, wrapped at an hour.
    time: f32,
    // Which of the frame's lights reach it: `light_surface` reads it.
    light_list: vec3<u32>,
};

// What a material's `surface()` returns: the standard's inputs, lit by
// the engine (or not, `lighting: Unlit`).
struct Surface {
    // Linear RGB.
    base_color: vec3<f32>,
    // A mask's or a blend's alpha; ignored when opaque.
    alpha: f32,
    metallic: f32,
    // Perceptual roughness.
    roughness: f32,
    // In the world, unit length, facing the side seen.
    normal: vec3<f32>,
    // Linear RGB light given off.
    emissive: vec3<f32>,
    // How much ambient reaches the point.
    occlusion: f32,
};

fn surface_input(in: VertexOut, front: bool) -> SurfaceInput {
    // The way back to the eye: from the eye in perspective, against the
    // view's direction in an orthographic one.
    let to_eye = unit_or(frame.eye.xyz - in.world, -frame.forward.xyz);
    let v = select(-frame.forward.xyz, to_eye, frame.eye.w > 0.5);
    // The geometric normal; a flattened model's side faces have none, and
    // face the eye.
    let geometric = unit_or(in.normal, v);
    // The tangent made perpendicular to the normal (Gram-Schmidt), the
    // bitangent cross(n, t) turned by the handedness.
    let tangent = unit_or(in.tangent.xyz - geometric * dot(geometric, in.tangent.xyz), vec3<f32>(0.0));
    let bitangent = cross(geometric, tangent) * select(-1.0, 1.0, in.tangent.w >= 0.0);
    // A back face — double-sided, or a flat model's — is lit as the front
    // turned round: the whole frame turns (Filament's).
    let side = select(-1.0, 1.0, front);
    var out: SurfaceInput;
    out.position = in.clip;
    out.world = in.world;
    out.normal = geometric * side;
    out.tangent = tangent * side;
    out.bitangent = bitangent * side;
    out.uv = in.uv;
    out.tint = in.tint;
    out.view = v;
    out.front_facing = front;
    out.time = frame.ambient.w;
    out.light_list = in.lights;
    return out;
}

// `in`'s texture coordinate through the standard block's UV transform:
// where the standard textures are read.
fn surface_uv(in: SurfaceInput) -> vec2<f32> {
    return vec2<f32>(
        dot(standard.uv_x.xy, in.uv) + standard.uv_x.z,
        dot(standard.uv_y.xy, in.uv) + standard.uv_y.z,
    );
}

// The material file's `standard` block, evaluated: what a material with no
// shader draws, and where a custom one may start.
fn standard_surface(in: SurfaceInput) -> Surface {
    let uv = surface_uv(in);
    let base_texel = textureSample(base_color_texture, surface_sampler, uv);
    let metallic_roughness = textureSample(metallic_roughness_texture, surface_sampler, uv);
    let normal_texel = textureSample(normal_texture, surface_sampler, uv).xyz;
    let occlusion_texel = textureSample(occlusion_texture, surface_sampler, uv).r;
    let emissive_texel = textureSample(emissive_texture, surface_sampler, uv).rgb;

    let base = standard.base_color * base_texel * in.tint;
    // Normal mapping through the tangent frame, already turned to the side
    // seen.
    let bent = vec3<f32>((normal_texel.xy * 2.0 - 1.0) * standard.factors.z, normal_texel.z * 2.0 - 1.0);
    let mapped = unit_or(in.tangent * bent.x + in.bitangent * bent.y + in.normal * bent.z, in.normal);
    let has_map = standard.flags.z > 0.5 && dot(in.tangent, in.tangent) > 0.5;

    var out: Surface;
    out.base_color = base.rgb;
    out.alpha = base.a;
    out.metallic = standard.factors.x * metallic_roughness.b;
    out.roughness = standard.factors.y * metallic_roughness.g;
    out.normal = select(in.normal, mapped, has_map);
    out.emissive = standard.emissive.rgb * emissive_texel;
    out.occlusion = 1.0 + standard.factors.w * (occlusion_texel - 1.0);
    return out;
}

// The mask's cutoff, from the standard block's alpha mode.
fn surface_cutoff() -> f32 {
    return standard.flags.x;
}
