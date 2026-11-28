// A surface tinted by where it is in the world: bands `period` metres wide
// along x, from `start_color` to `end_color`, over the standard block's
// colour. Shaded as the standard surface is; only the base colour changes.
fn surface(in: SurfaceInput) -> Surface {
    var s = standard_surface(in);
    let t = fract(in.world.x / material.period);
    s.base_color = s.base_color * mix(material.start_color.rgb, material.end_color.rgb, t);
    return s;
}
