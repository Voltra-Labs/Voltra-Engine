// An outline: transparent texels next to opaque ones, `width` texels away,
// drawn in `color`. The sprite needs transparent texels around its shape
// for the outline to fall on.
fn fragment(in: Fragment) -> vec4<f32> {
    let base = sprite_color(in);
    let step = material.width / vec2<f32>(textureDimensions(albedo));
    // Every sample before any branch: WGSL wants texture sampling in
    // uniform control flow.
    let right = textureSample(albedo, albedo_sampler, in.uv + vec2<f32>(step.x, 0.0)).a;
    let left = textureSample(albedo, albedo_sampler, in.uv - vec2<f32>(step.x, 0.0)).a;
    let down = textureSample(albedo, albedo_sampler, in.uv + vec2<f32>(0.0, step.y)).a;
    let up = textureSample(albedo, albedo_sampler, in.uv - vec2<f32>(0.0, step.y)).a;
    let around = max(max(right, left), max(down, up));
    let edge = base.a < 0.5 && around > 0.5;
    return select(base, material.color, edge);
}
