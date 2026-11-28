// A dissolve: the surface is cut away where the `noise` texture (data, its
// red channel) is under `amount`, and glows `edge_color` within `edge` of
// the cut. The material's alpha is `Mask`, so the cut is the engine's: it
// casts and picks as it is drawn, and smooths under MSAA.
fn surface(in: SurfaceInput) -> Surface {
    var s = standard_surface(in);
    let noise = textureSample(noise, surface_sampler, surface_uv(in)).r;
    let kept = noise >= material.amount;
    s.alpha = select(0.0, s.alpha, kept);
    let rim = 1.0 - smoothstep(0.0, max(material.edge, 1.0e-4), noise - material.amount);
    s.emissive = s.emissive + material.edge_color.rgb * rim * select(0.0, 1.0, kept);
    return s;
}
