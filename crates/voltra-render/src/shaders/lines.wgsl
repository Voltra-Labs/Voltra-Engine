// Widening happens here, after projection, so the width is in pixels and a
// line keeps its thickness at any zoom. See `lines.rs` for why a line is a quad
// at all: WebGPU has no line width.

struct Camera {
    view_proj: mat4x4<f32>,
};

struct Viewport {
    // Logical pixels in `.xy`. A vec4 because a uniform buffer binding is
    // aligned to 16 bytes, and two unused floats are cheaper than a padding
    // field nobody remembers the reason for.
    size: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> viewport: Viewport;

struct VertexIn {
    @location(0) a: vec2<f32>,
    @location(1) b: vec2<f32>,
    @location(2) corner: vec2<f32>,
    @location(3) width: f32,
    @location(4) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    let half_size = viewport.size.xy * 0.5;

    // Both endpoints into the same half-pixel space, so the perpendicular below
    // is a screen direction rather than a world one. The camera is orthographic
    // so w is 1 and the divide is a formality — written out anyway, because it
    // stops being one the day a projection is not ortho.
    let clip_a = camera.view_proj * vec4<f32>(in.a, 0.0, 1.0);
    let clip_b = camera.view_proj * vec4<f32>(in.b, 0.0, 1.0);
    let screen_a = clip_a.xy / clip_a.w * half_size;
    let screen_b = clip_b.xy / clip_b.w * half_size;

    let delta = screen_b - screen_a;
    // `len`, not `length`: shadowing the builtin would make the call itself a
    // type error, which is a confusing way to find out.
    let len = max(length(delta), 1e-6);
    let dir = delta / len;
    let normal = vec2<f32>(-dir.y, dir.x);

    let at = select(screen_a, screen_b, in.corner.x > 0.5);
    let widened = at + normal * in.corner.y * in.width * 0.5;

    var out: VertexOut;
    out.clip = vec4<f32>(widened / half_size, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
