// The sky: one triangle over the whole target at the far plane, drawn
// after the opaque and before the blended, depth-tested so it lands only
// where nothing was drawn (depth still the clear's 0, reversed Z). Follows
// frame3d.wgsl and environment3d.wgsl.

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    // Where the pixel is in normalised device coordinates.
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> SkyOut {
    // (−1, −1), (3, −1), (−1, 3): the target and beyond.
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: SkyOut;
    // z 0: the far plane, reversed.
    out.clip = vec4<f32>(xy, 0.0, 1.0);
    out.ndc = xy;
    return out;
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    // The pixel's point on the near plane (z 1, reversed), back in the
    // world; the ray from the eye through it.
    let near = frame.inverse_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let direction = eye_ray(near.xyz / near.w);
    return vec4<f32>(min(sky_color(direction), vec3<f32>(MAX_LIGHT)), 1.0);
}
