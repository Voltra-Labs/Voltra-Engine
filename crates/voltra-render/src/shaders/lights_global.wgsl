// Built-in shader: global 2D lights, added onto the frame's light map.
//
// One full-screen triangle per global light, one instance each, added onto
// the light map after its clear to the ambient. A pixel gets the light's
// colour when its light layers share one with the light's, read from the
// scene's light-layer buffer as `lights.wgsl` does; group 0 must match
// `SceneInput` (binding 0, the normals, is not read). Must match `lights::global::GlobalInstance`.

// The light layers each pixel is NOT on (`sprite_layers.wgsl`); a 1x1 zero
// stand-in, "on every layer", when the frame did not draw them.
@group(0) @binding(1)
var excluded_layers: texture_2d<u32>;

struct GlobalIn {
    // colour × intensity, unused
    @location(0) color: vec4<f32>,
    // the light layers it lights
    @location(1) layers: u32,
}

struct GlobalOut {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec3<f32>,
    @location(1) @interpolate(flat) layers: u32,
}

@vertex
fn vs_global(@builtin(vertex_index) index: u32, light: GlobalIn) -> GlobalOut {
    // One triangle over the whole target: (-1, -1), (3, -1), (-1, 3).
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: GlobalOut;
    out.position = vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
    out.color = light.color.rgb;
    out.layers = light.layers;
    return out;
}

@fragment
fn fs_global(in: GlobalOut) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(excluded_layers));
    let pixel = min(vec2<i32>(in.position.xy), size - vec2<i32>(1));
    let excluded = textureLoad(excluded_layers, pixel, 0).r;
    if (in.layers & ~excluded) == 0u {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    return vec4<f32>(in.color, 1.0);
}
