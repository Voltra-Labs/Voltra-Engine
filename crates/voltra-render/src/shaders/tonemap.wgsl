// A 3D picture's post pass, put after `grade.wgsl`: the linear HDR scene,
// with its glow, exposed, brought into range by a tone curve, tinted and
// graded — the same sRGB picture a 2D shot's post pass writes.
//
// The source is the 3D target, linear (`Rgba16Float`, or an sRGB target
// read through its sRGB view where the device cannot draw float). It is
// premultiplied: an overlay's background is transparent black, and an
// MSAA edge resolves to colour times coverage. A curve is not linear, so
// the colour is unpremultiplied first and premultiplied after, and the
// alpha passes through: an opaque shot is alpha 1 throughout.
//
// The curves match `render3d::Tonemap::apply` on the CPU, number for
// number; `headless_tonemap.rs` pins the two together.

// Benjamin Wrensch's fit of Troy Sobotka's AgX, in linear sRGB primaries:
// the inset into AgX's log space, its sigmoid, and the outset back.
const AGX_MIN_EV: f32 = -12.47393;
const AGX_MAX_EV: f32 = 4.026069;

fn agx_contrast(x: vec3<f32>) -> vec3<f32> {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2
        + 0.1191 * x - 0.00232;
}

fn agx(colour: vec3<f32>) -> vec3<f32> {
    // Columns, as WGSL's constructor takes them.
    let inset = mat3x3<f32>(
        vec3<f32>(0.842479062253094, 0.0423282422610123, 0.0423756549057051),
        vec3<f32>(0.0784335999999992, 0.878468636469772, 0.0784336),
        vec3<f32>(0.0792237451477643, 0.0791661274605434, 0.879142973793104),
    );
    let outset = mat3x3<f32>(
        vec3<f32>(1.19687900512017, -0.0528968517574562, -0.0529716355144438),
        vec3<f32>(-0.0980208811401368, 1.15190312990417, -0.0980434501171241),
        vec3<f32>(-0.0990297440797205, -0.0989611768448433, 1.15107367264116),
    );
    var c = inset * colour;
    c = clamp(log2(max(c, vec3<f32>(1e-10))), vec3<f32>(AGX_MIN_EV), vec3<f32>(AGX_MAX_EV));
    c = (c - AGX_MIN_EV) / (AGX_MAX_EV - AGX_MIN_EV);
    c = agx_contrast(c);
    c = outset * c;
    // The sigmoid's output is display-encoded at 2.2; linear again, for
    // the grade and the sRGB target.
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

// Stephen Hill's fit of ACES's reference rendering and output transforms.
fn aces(colour: vec3<f32>) -> vec3<f32> {
    let input = mat3x3<f32>(
        vec3<f32>(0.59719, 0.07600, 0.02840),
        vec3<f32>(0.35458, 0.90834, 0.13383),
        vec3<f32>(0.04823, 0.01566, 0.83777),
    );
    let output = mat3x3<f32>(
        vec3<f32>(1.60475, -0.10208, -0.00327),
        vec3<f32>(-0.53108, 1.10813, -0.07276),
        vec3<f32>(-0.07367, -0.00605, 1.07602),
    );
    let v = input * colour;
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return clamp(output * (a / b), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tone(colour: vec3<f32>, curve: f32) -> vec3<f32> {
    let c = max(colour, vec3<f32>(0.0));
    if curve < 0.5 {
        return c;
    }
    if curve < 1.5 {
        return c / (vec3<f32>(1.0) + c);
    }
    if curve < 2.5 {
        return aces(c);
    }
    return agx(c);
}

// Both samples come before any choice: WebGPU allows `textureSample` in
// uniform control flow only.
@fragment
fn fs_tonemap(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(source, source_sampler, in.uv);
    let glowing = textureSample(glow, source_sampler, in.uv).rgb;
    let alpha = sampled.a;
    let bloomed = sampled.rgb + glowing * params.bloom.x;
    let exposed = bloomed / max(alpha, 1e-6) * params.tint_exposure.a;
    let c = grade(tone(exposed, params.bloom.y) * params.tint_exposure.rgb, in.uv);
    // Premultiplied for the composite; straight for a render texture,
    // which sprites blend by its alpha (`bloom.z`).
    let colour = select(c * alpha, c, params.bloom.z > 0.5);
    return select(vec4<f32>(0.0), vec4<f32>(colour, alpha), alpha > 0.0);
}
