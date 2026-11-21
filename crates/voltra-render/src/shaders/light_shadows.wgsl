// Built-in shader: occluding edges into the 2D shadow map.
//
// Each instance is one edge and one shadowed light. It is drawn four times,
// once per quarter turn around the light, as a quad filling that quarter's
// row of the map. Through a 90° perspective projection centred on the light
// the quad lands on the texels whose directions cross the edge; the
// rasteriser clips it to the quarter (x within ±w) and to in front of the
// light (z ≤ w), and interpolates the point on the edge each direction hits.
// The fragment writes how far that is, over the light's radius; the target
// blends with `Min`, so the nearest edge in each direction is what stays.
//
// Must match `shadows::ShadowInstance`, and `quarter` must match the one in
// `lights.wgsl`, which reads the map back.

struct EdgeIn {
    // a.xy, b.xy, in the world
    @location(0) ends: vec4<f32>,
    // centre.xy, radius, top of the light's first row in clip space
    @location(1) light: vec4<f32>,
    // one row's height in clip space
    @location(2) row_height: f32,
}

struct EdgeOut {
    @builtin(position) position: vec4<f32>,
    // Where on the edge, from the light, in radii.
    @location(0) offset: vec2<f32>,
}

// How close to the light an edge is still drawn, in radii. The projection
// divides by the distance ahead, so something has to stop it at zero.
const NEAR: f32 = 1e-4;

// `offset` turned so that quarter `q`'s direction is ahead: x across the
// quarter, y ahead. Quarter 0 looks right (+x), 1 up, 2 left, 3 down.
fn quarter(offset: vec2<f32>, q: u32) -> vec2<f32> {
    switch q {
        case 0u: { return vec2<f32>(offset.y, offset.x); }
        case 1u: { return vec2<f32>(-offset.x, offset.y); }
        case 2u: { return vec2<f32>(-offset.y, -offset.x); }
        default: { return vec2<f32>(offset.x, -offset.y); }
    }
}

@vertex
fn vs_edge(@builtin(vertex_index) index: u32, edge: EdgeIn) -> EdgeOut {
    let q = index / 6u;
    let corner = index % 6u;
    // Two triangles per quarter: (a, top) (b, top) (a, bottom), then
    // (a, bottom) (b, top) (b, bottom).
    var at_b = array<bool, 6>(false, true, false, false, true, true);
    var at_bottom = array<f32, 6>(0.0, 0.0, 1.0, 1.0, 0.0, 1.0);
    let end = select(edge.ends.xy, edge.ends.zw, at_b[corner]);
    let offset = (end - edge.light.xy) / edge.light.z;
    let seen = quarter(offset, q);
    // The row's band, multiplied by w so the divide leaves it in place.
    let y = edge.light.w - (f32(q) + at_bottom[corner]) * edge.row_height;
    var out: EdgeOut;
    out.position = vec4<f32>(seen.x, y * seen.y, NEAR, seen.y);
    out.offset = offset;
    return out;
}

@fragment
fn fs_edge(in: EdgeOut) -> @location(0) vec4<f32> {
    return vec4<f32>(length(in.offset), 0.0, 0.0, 1.0);
}
