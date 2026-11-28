// The 3D frame's view, ambient, fog and sky: group 0 binding 0 of the
// surface and the sky pipelines (`lighting::FrameUniform3D`).

const PI: f32 = 3.14159265358979;
// The most light a surface sends back, per channel. A mirror-smooth
// highlight near grazing reaches some 10⁵ at energy 1, and emissive is
// unbounded; past 65504 an Rgba16Float texel is infinite, and bloom and the
// curves turn that to NaN. Low enough that bloom's levels, summed, stay
// finite too. `brdf::MAX_LIGHT` on the CPU.
const MAX_LIGHT: f32 = 4096.0;

struct Frame3D {
    view_proj: mat4x4<f32>,
    // Clip space back to the world: where the sky's rays come from.
    inverse_view_proj: mat4x4<f32>,
    // xyz: the eye; w: 1 for a perspective view, 0 for an orthographic one.
    eye: vec4<f32>,
    // xyz: the way the view looks, unit length.
    forward: vec4<f32>,
    // rgb: the ambient, a uniform sky's light; w: the time in seconds,
    // wrapped at an hour.
    ambient: vec4<f32>,
    // rgb: what fog fades to; w: how much the sky's colour replaces it.
    fog_color: vec4<f32>,
    // x: the fog's density at its height, per metre (0: none); y: that
    // height; z: how fast it thins upward, per metre.
    fog: vec4<f32>,
    // The sky, straight up, level and straight down (rgb); a flat
    // background is all three the same.
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    sky_bottom: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame3D;
