// Built-in shader: the drawn scene multiplied by the light map, pixel for
// pixel, into the lit frame that bloom and the grade read. Both inputs are
// the frame's size; both are linear. See `lights::LightPass`.

@group(0) @binding(0)
var scene: texture_2d<f32>;
@group(0) @binding(1)
var light_map: texture_2d<f32>;

@vertex
fn vs_full(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(index & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_composite(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let at = vec2<i32>(position.xy);
    let colour = textureLoad(scene, at, 0);
    let light = textureLoad(light_map, at, 0).rgb;
    return vec4<f32>(colour.rgb * light, colour.a);
}
