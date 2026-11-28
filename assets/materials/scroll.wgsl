// A texture sliding across the surface: `pattern` read `speed` texture
// widths a second along, on top of the standard block's colour. The time is
// the game's, wrapped at an hour — a speed that is a whole number of
// widths an hour loops without a jump.
fn surface(in: SurfaceInput) -> Surface {
    var s = standard_surface(in);
    let uv = surface_uv(in) + material.speed * in.time;
    s.base_color = s.base_color * textureSample(pattern, surface_sampler, uv).rgb;
    return s;
}
