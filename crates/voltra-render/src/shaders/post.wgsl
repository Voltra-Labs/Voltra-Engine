// Full-screen post-processing: colour grading and a vignette over the scene.
//
// One triangle covering the target, generated from the vertex index — no
// vertex buffer. The source is sampled through its sRGB view, so values
// arrive linear; every operation below is in linear light, and the sRGB
// target encodes on write. Sampling the raw view here would grade encoded
// values and darken the mid-tones (see "egui for the editor UI" in
// ARCHITECTURE.md for the same trap).
//
// Neutral parameters — exposure 1, tint white, contrast 1, saturation 1,
// vignette 0 — are the identity.

struct Params {
    // rgb: tint, a: exposure
    tint_exposure: vec4<f32>,
    // x: contrast, y: saturation, z: vignette, w: aspect (width / height)
    grade: vec4<f32>,
}

@group(0) @binding(0)
var source: texture_2d<f32>;
@group(0) @binding(1)
var source_sampler: sampler;
@group(0) @binding(2)
var<uniform> params: Params;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(source, source_sampler, in.uv);
    var c = sampled.rgb * params.tint_exposure.rgb * params.tint_exposure.a;
    c = max(vec3<f32>(MID_GREY) + (c - vec3<f32>(MID_GREY)) * params.grade.x, vec3<f32>(0.0));
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = max(mix(vec3<f32>(luma), c, params.grade.y), vec3<f32>(0.0));
    // Distance from the centre, corrected for aspect so the vignette is round.
    let centred = (in.uv - vec2<f32>(0.5)) * vec2<f32>(params.grade.w, 1.0);
    let fall = smoothstep(0.35, 0.85, length(centred));
    c = c * (1.0 - params.grade.z * fall);
    return vec4<f32>(c, sampled.a);
}
