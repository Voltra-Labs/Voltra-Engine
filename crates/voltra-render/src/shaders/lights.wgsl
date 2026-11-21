// Built-in shader: 2D point lights, added onto the frame's light map.
//
// `vs_light` / `fs_light` draw each point light as a quad around it, one
// instance per light, through the world camera, added onto a light map
// cleared to the ambient colour. Group 0 must match `CameraBinding`.
// `lights_composite.wgsl` multiplies the scene by the map.
//
// Everything is linear light. Must match `lights::LightInstance`.

struct Camera {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

struct LightIn {
    // centre.xy, radius, falloff
    @location(0) place: vec4<f32>,
    // colour × intensity, unused
    @location(1) color: vec4<f32>,
}

struct LightOut {
    @builtin(position) position: vec4<f32>,
    // Where in the light's disc, -1..1 on each axis.
    @location(0) local: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) falloff: f32,
}

@vertex
fn vs_light(@builtin(vertex_index) index: u32, light: LightIn) -> LightOut {
    // Two triangles: corners 0 1 2, 2 1 3 of the unit square.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    let corner = corners[index];
    let world = light.place.xy + corner * light.place.z;
    var out: LightOut;
    out.position = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.local = corner;
    out.color = light.color.rgb;
    out.falloff = light.place.w;
    return out;
}

@fragment
fn fs_light(in: LightOut) -> @location(0) vec4<f32> {
    // Full at the centre, nothing at the radius; the falloff bends the
    // curve between: 1 is a straight ramp, 2 the default soft one.
    let edge = clamp(1.0 - length(in.local), 0.0, 1.0);
    let strength = pow(edge, in.falloff);
    return vec4<f32>(in.color * strength, 1.0);
}
