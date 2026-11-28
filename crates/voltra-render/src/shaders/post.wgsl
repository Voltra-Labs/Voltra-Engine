// A 2D picture's post pass, put after `grade.wgsl`: the drawn scene, with
// its glow, exposed, tinted and graded.
//
// The source is sampled through its sRGB view, so values arrive linear.
// Sampling the raw view here would grade encoded values and darken the
// mid-tones (see "egui for the editor UI" in ARCHITECTURE.md for the same
// trap).

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(source, source_sampler, in.uv);
    let bloomed = sampled.rgb + textureSample(glow, source_sampler, in.uv).rgb * params.bloom.x;
    let c = grade(bloomed * params.tint_exposure.rgb * params.tint_exposure.a, in.uv);
    return vec4<f32>(c, sampled.a);
}

// A 2D overlay's: drawn into a transparent target, its sprites blended
// over nothing are premultiplied, and contrast about grey on a
// premultiplied edge would halo it — a transparent pixel graded to grey.
// So the colour is unpremultiplied, graded straight and premultiplied
// again. A glow over a pixel with nothing drawn has no alpha to show by
// and is left out. Both samples come before any choice: WebGPU allows
// `textureSample` in uniform control flow only.
@fragment
fn fs_overlay(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(source, source_sampler, in.uv);
    let glowing = textureSample(glow, source_sampler, in.uv).rgb;
    let alpha = sampled.a;
    let straight = (sampled.rgb + glowing * params.bloom.x) / max(alpha, 1e-6);
    let c = grade(straight * params.tint_exposure.rgb * params.tint_exposure.a, in.uv);
    return select(vec4<f32>(0.0), vec4<f32>(c * alpha, alpha), alpha > 0.0);
}
