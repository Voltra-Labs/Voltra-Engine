// Breaks WGSL's uniformity rule on purpose: a texture sampled inside a
// branch on a per-pixel value. naga (native) and WebGL2 run it; WebGPU's
// Tint refuses the whole module, and the engine then draws this material's
// standard block instead of a black frame. scenes/custom3d.ron shows both;
// what not to write — sample first, then `select` (docs/guide/11-3d.md).
fn surface(in: SurfaceInput) -> Surface {
    var s = standard_surface(in);
    if in.uv.x > 0.0 {
        let texel = textureSample(base_color_texture, surface_sampler, in.uv).rgb;
        s.base_color = vec3<f32>(0.9, 0.75, 0.1) * texel;
    }
    return s;
}
