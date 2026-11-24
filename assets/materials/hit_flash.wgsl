// A hit flash: the sprite washed towards `flash_color` by `flash`, 0 to 1,
// its shape kept. Set `flash` to 1 from a tick when the sprite is hit and
// tween it back to 0.
fn fragment(in: Fragment) -> vec4<f32> {
    let base = sprite_color(in);
    let flashed = mix(base.rgb, material.flash_color.rgb, clamp(material.flash, 0.0, 1.0));
    return vec4<f32>(flashed, base.a);
}
