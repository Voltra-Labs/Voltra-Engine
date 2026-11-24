// Shared by every sprite-batch shader: a quad as one instance
// (`QuadInstance::LAYOUT`, stepped per instance), its corners made here.
// Prepended to the shader that uses it (see `shader.rs`); the @location
// numbers must match the layout's attributes.
//
// Six vertices an instance, two triangles over corners 0 1 2 and 0 2 3
// (`QuadInstance::CORNERS`). Corner 0 is `origin`, 3 is `origin + across`,
// 1 is `origin + down`, 2 both; the texture coordinates follow the same
// rule. `QuadInstance::corners` is the same arithmetic on the CPU.

struct QuadInput {
    @location(0) origin: vec2<f32>,
    @location(1) across: vec2<f32>,
    @location(2) down: vec2<f32>,
    @location(3) uv_origin: vec2<f32>,
    @location(4) uv_across: vec2<f32>,
    @location(5) uv_down: vec2<f32>,
    @location(6) color: vec4<f32>,
}

// One corner of a quad, as a vertex buffer would have handed it.
struct QuadCorner {
    position: vec2<f32>,
    color: vec4<f32>,
    uv: vec2<f32>,
}

// The corner the `index`th of an instance's six vertices is. Compared
// rather than looked up in an array, so no backend has to index a
// constant array at run time.
fn quad_corner(quad: QuadInput, index: u32) -> QuadCorner {
    let s = f32(index == 2u || index == 4u || index == 5u);
    let t = f32(index == 1u || index == 2u || index == 4u);
    var corner: QuadCorner;
    corner.position = quad.origin + quad.across * s + quad.down * t;
    corner.uv = quad.uv_origin + quad.uv_across * s + quad.uv_down * t;
    corner.color = quad.color;
    return corner;
}
