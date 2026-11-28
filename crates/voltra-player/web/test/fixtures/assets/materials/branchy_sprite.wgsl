// Breaks WGSL's uniformity rule on purpose: the sprite's texture sampled
// inside a branch on a per-pixel value. naga (native) and WebGL2 run it,
// blue; WebGPU's Tint refuses the module, and the sprite then draws plainly
// — its own orange tint — instead of the sprite pass failing. A web-test
// fixture (scenes/refusals.ron); what not to write: sample first, then
// `select`.
fn fragment(in: Fragment) -> vec4<f32> {
    var color = vec4<f32>(0.1, 0.3, 1.0, 1.0);
    if in.uv.x > -1.0 {
        color = color * textureSample(albedo, albedo_sampler, in.uv);
    }
    return color;
}
