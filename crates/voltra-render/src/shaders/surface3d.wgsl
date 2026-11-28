// The standard surface, instanced: glTF 2.0's metallic-roughness inputs
// lit by Filament's BRDF — GGX, height-correlated Smith, Schlick, Lambert
// diffuse — under one directional light and a uniform ambient.
// `render3d::surface::brdf` is the CPU twin, number for number.
//
// Every texture is sampled before any choice is made: Chromium's Tint
// refuses `textureSample` (and `fwidth`) after a branch on a non-uniform
// value, and then the whole module with it. Choices are `select`s.

const PI: f32 = 3.14159265358979;
// Filament's floor on perceptual roughness: below it a highlight is
// narrower than a half float holds.
const MIN_ROUGHNESS: f32 = 0.045;
// The most light a surface sends back, per channel. A mirror-smooth
// highlight near grazing reaches some 10⁵ at energy 1, and emissive is
// unbounded; past 65504 an Rgba16Float texel is infinite, and bloom and the
// curves turn that to NaN. Low enough that bloom's levels, summed, stay
// finite too. `brdf::MAX_LIGHT` on the CPU.
const MAX_LIGHT: f32 = 4096.0;

struct Frame3D {
    view_proj: mat4x4<f32>,
    // xyz: the eye; w: 1 for a perspective view, 0 for an orthographic one.
    eye: vec4<f32>,
    // xyz: the way the view looks, unit length.
    forward: vec4<f32>,
    // xyz: toward the light, unit length; w: 1 with a light, 0 without.
    light_direction: vec4<f32>,
    // rgb: the light's colour times its energy.
    light_color: vec4<f32>,
    // rgb: the ambient, a uniform sky's light.
    ambient: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame3D;

struct Surface3D {
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

@group(1) @binding(0) var<uniform> surface: Surface3D;
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
    return out;
}

// `v` made unit length, or `fallback` where it has none.
fn unit_or(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let length_squared = dot(v, v);
    return select(fallback, v * inverseSqrt(max(length_squared, 1.0e-30)), length_squared > 1.0e-30);
}

// GGX / Trowbridge-Reitz, Filament's form: 1 − (n·h)² from |n × h|².
fn d_ggx(n_dot_h: f32, n_cross_h: vec3<f32>, a: f32) -> f32 {
    let one_minus = dot(n_cross_h, n_cross_h);
    let ah = n_dot_h * a;
    let k = a / (one_minus + ah * ah);
    return k * k * (1.0 / PI);
}

// Height-correlated Smith, the visibility V = G / (4 n·l n·v).
fn v_smith_ggx_correlated(n_dot_v: f32, n_dot_l: f32, a: f32) -> f32 {
    let a2 = a * a;
    let ggx_v = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    let ggx_l = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
    return 0.5 / (ggx_v + ggx_l);
}

fn f_schlick(f0: vec3<f32>, l_dot_h: f32) -> vec3<f32> {
    let f = pow(1.0 - l_dot_h, 5.0);
    return f + f0 * (1.0 - f);
}

// The split-sum's DFG term for a uniform environment, Karis's analytic fit
// ("Physically Based Shading on Mobile"): what a constant sky reflects.
fn env_brdf(f0: vec3<f32>, roughness: f32, n_dot_v: f32) -> vec3<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + ab.y;
}

struct Shaded {
    color: vec3<f32>,
    alpha: f32,
};

fn shade(in: VertexOut, front: bool) -> Shaded {
    let uv = vec2<f32>(
        dot(surface.uv_x.xy, in.uv) + surface.uv_x.z,
        dot(surface.uv_y.xy, in.uv) + surface.uv_y.z,
    );
    let base_texel = textureSample(base_color_texture, surface_sampler, uv);
    let metallic_roughness = textureSample(metallic_roughness_texture, surface_sampler, uv);
    let normal_texel = textureSample(normal_texture, surface_sampler, uv).xyz;
    let occlusion_texel = textureSample(occlusion_texture, surface_sampler, uv).r;
    let emissive_texel = textureSample(emissive_texture, surface_sampler, uv).rgb;

    let base = surface.base_color * base_texel * in.tint;
    let metallic = clamp(surface.factors.x * metallic_roughness.b, 0.0, 1.0);
    let roughness = clamp(surface.factors.y * metallic_roughness.g, MIN_ROUGHNESS, 1.0);
    let occlusion = 1.0 + surface.factors.w * (occlusion_texel - 1.0);
    let emissive = surface.emissive.rgb * emissive_texel;

    // The way back to the eye: from the eye in perspective, against the
    // view's direction in an orthographic one.
    let to_eye = unit_or(frame.eye.xyz - in.world, -frame.forward.xyz);
    let v = select(-frame.forward.xyz, to_eye, frame.eye.w > 0.5);

    // The geometric normal; a flattened model's side faces have none, and
    // face the eye.
    let geometric = unit_or(in.normal, v);
    // Normal mapping through the tangent: Gram-Schmidt against the normal,
    // the bitangent cross(n, t) turned by the handedness.
    let tangent = unit_or(in.tangent.xyz - geometric * dot(geometric, in.tangent.xyz), vec3<f32>(0.0));
    let bitangent = cross(geometric, tangent) * select(-1.0, 1.0, in.tangent.w >= 0.0);
    let bent = vec3<f32>((normal_texel.xy * 2.0 - 1.0) * surface.factors.z, normal_texel.z * 2.0 - 1.0);
    let mapped = unit_or(tangent * bent.x + bitangent * bent.y + geometric * bent.z, geometric);
    let has_map = surface.flags.z > 0.5 && dot(tangent, tangent) > 0.5;
    // A back face — double-sided, or a flat model's — is lit as the front
    // turned round (Filament turns the whole tangent frame).
    let n = select(-1.0, 1.0, front) * select(geometric, mapped, has_map);

    let diffuse_color = base.rgb * (1.0 - metallic);
    let f0 = mix(vec3<f32>(0.04), base.rgb, metallic);
    let n_dot_v = min(abs(dot(n, v)) + 1.0e-5, 1.0);
    let a = roughness * roughness;

    // The directional light.
    let l = frame.light_direction.xyz;
    let h = unit_or(v + l, n);
    let n_dot_l = clamp(dot(n, l), 0.0, 1.0);
    let n_dot_h = clamp(dot(n, h), 0.0, 1.0);
    let l_dot_h = clamp(dot(l, h), 0.0, 1.0);
    let specular = d_ggx(n_dot_h, cross(n, h), a) * v_smith_ggx_correlated(n_dot_v, n_dot_l, a)
        * f_schlick(f0, l_dot_h);
    let direct = (diffuse_color * (1.0 / PI) + specular) * frame.light_color.rgb * n_dot_l
        * frame.light_direction.w;

    // The ambient: a uniform sky, specular (the split sum's E = f0·A + B)
    // and diffuse (its irradiance π·L times albedo / π) taking what the
    // specular leaves, (1 − E) — Filament's — occluded.
    let reflected = env_brdf(f0, roughness, n_dot_v);
    let ambient = (diffuse_color * (vec3<f32>(1.0) - reflected) + reflected) * frame.ambient.rgb
        * occlusion;

    let lit = min(direct + ambient + emissive, vec3<f32>(MAX_LIGHT));
    var out: Shaded;
    out.color = select(lit, min(base.rgb, vec3<f32>(MAX_LIGHT)), surface.flags.y > 0.5);
    out.alpha = base.a;
    return out;
}

// Opaque: alpha 1 whatever the base's, so a 3D overlay is see-through only
// where nothing was drawn.
@fragment
fn fs_opaque(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let shaded = shade(in, front);
    return vec4<f32>(shaded.color, 1.0);
}

// Cut out under the cutoff, opaque elsewhere: one sample a pixel.
@fragment
fn fs_mask(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let shaded = shade(in, front);
    if shaded.alpha < surface.flags.x {
        discard;
    }
    return vec4<f32>(shaded.color, 1.0);
}

// Cut out by alpha to coverage under MSAA: alpha sharpened to a pixel-wide
// ramp about the cutoff (Ben Golus's), which the samples cover in steps.
// The alpha is written too, so the colour is premultiplied by it, as the
// 3D target is: an overlay's masked edge has no bright fringe.
@fragment
fn fs_mask_coverage(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let shaded = shade(in, front);
    let ramp = max(fwidth(shaded.alpha), 1.0e-4);
    let coverage = clamp((shaded.alpha - surface.flags.x) / ramp + 0.5, 0.0, 1.0);
    return vec4<f32>(shaded.color * coverage, coverage);
}

// Blended: premultiplied, as the 3D target and the composite are.
@fragment
fn fs_blend(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let shaded = shade(in, front);
    let alpha = clamp(shaded.alpha, 0.0, 1.0);
    return vec4<f32>(shaded.color * alpha, alpha);
}
