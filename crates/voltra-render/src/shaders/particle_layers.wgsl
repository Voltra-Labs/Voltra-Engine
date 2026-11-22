// Built-in shader fragment: a GPU emitter's light layers, for lights that
// leave some out. Appended to `particles_draw.wgsl` — its vertex stage, its
// groups 0 to 2 — so a particle lands on the same texels in both passes.
//
// As `sprite_layers.wgsl`: the emitter's layers, inverted, where a particle
// covers at least half the texel; group 3 the emitter's layers.

struct EmitterLayers {
    // The emitter's light layers; the rest pads to sixteen bytes.
    bits: vec4<u32>,
}

@group(3) @binding(0)
var<uniform> emitter_layers: EmitterLayers;

@fragment
fn fs_layers(in: VertexOutput) -> @location(0) u32 {
    let covered = textureSample(albedo, albedo_sampler, in.uv).a * in.color.a;
    if covered < 0.5 {
        discard;
    }
    return ~emitter_layers.bits.x;
}
