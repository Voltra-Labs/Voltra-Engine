// The grade both dimensions share, put before `post.wgsl` (a 2D picture)
// and `tonemap.wgsl` (a 3D one): the uniform, the bindings, the
// full-screen triangle and `grade`, contrast to the vignette.
//
// One triangle covering the target, generated from the vertex index — no
// vertex buffer. Every operation is in linear light, and the sRGB target
// encodes on write.
//
// Neutral parameters — exposure 1, tint white, contrast 1, saturation 1,
// vignette 0 — are the identity.

struct Params {
    // rgb: tint, a: exposure
    tint_exposure: vec4<f32>,
    // x: contrast, y: saturation, z: vignette, w: aspect (width / height)
    grade: vec4<f32>,
    // x: bloom intensity, y: the tone curve (`tonemap.wgsl`'s; 0 elsewhere)
    bloom: vec4<f32>,
}

@group(0) @binding(0)
var source: texture_2d<f32>;
@group(0) @binding(1)
var source_sampler: sampler;
@group(0) @binding(2)
var<uniform> params: Params;
// The glow at half size, linear; black when bloom is off.
@group(0) @binding(3)
var glow: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    // (-1,-1), (3,-1), (-1,3): one triangle whose inside is the whole screen.
    let x = f32((index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(index & 2u) * 2.0 - 1.0;
    var out: VertexOutput;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return out;
}

// Linear-light middle grey, which contrast pivots around.
const MID_GREY: f32 = 0.18;

// Contrast about middle grey, saturation and the vignette, over colour
// already exposed and tinted: the grade after the light is settled.
fn grade(colour: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    var c = max(vec3<f32>(MID_GREY) + (colour - vec3<f32>(MID_GREY)) * params.grade.x, vec3<f32>(0.0));
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = max(mix(vec3<f32>(luma), c, params.grade.y), vec3<f32>(0.0));
    // Distance from the centre, corrected for aspect so the vignette is round.
    let centred = (uv - vec2<f32>(0.5)) * vec2<f32>(params.grade.w, 1.0);
    let fall = smoothstep(0.35, 0.85, length(centred));
    return c * (1.0 - params.grade.z * fall);
}
