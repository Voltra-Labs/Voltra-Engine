// Built-in shader: bloom as a mip chain, after Jimenez's "Next Generation
// Post Processing in Call of Duty: Advanced Warfare" (SIGGRAPH 2014), the
// scheme Bevy and Unity URP use.
//
// - `fs_prefilter` keeps what is brighter than the threshold (a soft knee,
//   so there is no hard edge where bloom begins) and halves the size.
// - `fs_downsample` halves again with a 13-tap filter, wide enough that a
//   thin bright line does not flicker as it moves across texels.
// - `fs_upsample` doubles with a 3×3 tent, added onto the level above it by
//   the pipeline's blend, so every level contributes to the one result.
//
// Sources are sampled linearly with clamped edges; everything is linear
// light. Must match `bloom::BloomUniform`.

struct Bloom {
    // threshold, knee (threshold × softness), upsample radius, unused
    params: vec4<f32>,
}

@group(0) @binding(0)
var source: texture_2d<f32>;
@group(0) @binding(1)
var source_sampler: sampler;
@group(0) @binding(2)
var<uniform> bloom: Bloom;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let x = f32((index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(index & 2u) * 2.0 - 1.0;
    var out: VertexOutput;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return out;
}

// The most light a tap carries: an infinite texel (a half float
// overflowed upstream) would make the threshold's ratio inf / inf, NaN, and
// spread it over the whole glow. Clamped to a finite value instead; the
// levels, summed, stay under a half float's 65504.
const MAX_TAP: f32 = 4096.0;

fn tap(uv: vec2<f32>) -> vec3<f32> {
    return clamp(textureSample(source, source_sampler, uv).rgb, vec3<f32>(0.0), vec3<f32>(MAX_TAP));
}

// The 13-tap downsample: a centre box, four corner boxes and the
// overlapping inner one, weighted to sum to one.
fn downsample13(uv: vec2<f32>) -> vec3<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(source));
    let a = tap(uv + t * vec2<f32>(-2.0, -2.0));
    let b = tap(uv + t * vec2<f32>(0.0, -2.0));
    let c = tap(uv + t * vec2<f32>(2.0, -2.0));
    let d = tap(uv + t * vec2<f32>(-2.0, 0.0));
    let e = tap(uv);
    let f = tap(uv + t * vec2<f32>(2.0, 0.0));
    let g = tap(uv + t * vec2<f32>(-2.0, 2.0));
    let h = tap(uv + t * vec2<f32>(0.0, 2.0));
    let i = tap(uv + t * vec2<f32>(2.0, 2.0));
    let j = tap(uv + t * vec2<f32>(-1.0, -1.0));
    let k = tap(uv + t * vec2<f32>(1.0, -1.0));
    let l = tap(uv + t * vec2<f32>(-1.0, 1.0));
    let m = tap(uv + t * vec2<f32>(1.0, 1.0));
    var sum = e * 0.125;
    sum += (a + c + g + i) * 0.03125;
    sum += (b + d + f + h) * 0.0625;
    sum += (j + k + l + m) * 0.125;
    return sum;
}

// Brightness past the threshold, eased in over the knee (Karis's quadratic
// soft threshold), scaling the colour so its hue survives.
fn threshold(c: vec3<f32>) -> vec3<f32> {
    let brightness = max(c.r, max(c.g, c.b));
    let knee = bloom.params.y;
    var soft = clamp(brightness - bloom.params.x + knee, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee + 1e-5);
    let contribution = max(soft, brightness - bloom.params.x) / max(brightness, 1e-5);
    return c * max(contribution, 0.0);
}

@fragment
fn fs_prefilter(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(threshold(downsample13(in.uv)), 1.0);
}

@fragment
fn fs_downsample(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(downsample13(in.uv), 1.0);
}

@fragment
fn fs_upsample(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = bloom.params.z / vec2<f32>(textureDimensions(source));
    var sum = tap(in.uv) * 4.0;
    sum += (tap(in.uv + vec2<f32>(-t.x, 0.0)) + tap(in.uv + vec2<f32>(t.x, 0.0))
        + tap(in.uv + vec2<f32>(0.0, -t.y)) + tap(in.uv + vec2<f32>(0.0, t.y))) * 2.0;
    sum += tap(in.uv + vec2<f32>(-t.x, -t.y)) + tap(in.uv + vec2<f32>(t.x, -t.y))
        + tap(in.uv + vec2<f32>(-t.x, t.y)) + tap(in.uv + vec2<f32>(t.x, t.y));
    return vec4<f32>(sum / 16.0, 1.0);
}
