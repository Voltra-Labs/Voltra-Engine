// Built-in shader: 2D point lights, added onto the frame's light map.
//
// `vs_light` / `fs_light` draw each point light as a quad around it, one
// instance per light, through the world camera, added onto a light map
// cleared to the ambient colour. Group 0 must match `CameraBinding`.
// `lights_composite.wgsl` multiplies the scene by the map.
//
// A shadowed light keeps its light only where the pixel is nearer than the
// nearest occluder its way, read from its four rows of the shadow map
// (`light_shadows.wgsl`); `quarter` must match the one there.
//
// Everything is linear light. Must match `lights::LightInstance`.

struct Camera {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

// Distance to the nearest occluder, over the radius, per direction: four
// rows per shadowed light. A 1×1 stand-in when no light is shadowed.
@group(1) @binding(0)
var shadow_map: texture_2d<f32>;

// The scene's normals (`sprite_normals.wgsl`): `n * 0.5 + 0.5`, alpha 1
// where something was drawn. A 1x1 zero stand-in when they were not, which
// reads as facing the screen everywhere.
@group(2) @binding(0)
var scene_normals: texture_2d<f32>;

// The light layers each pixel is NOT on (`sprite_layers.wgsl`), so the zero
// it is cleared to — and the 1x1 zero stand-in — is "on every layer".
@group(3) @binding(0)
var excluded_layers: texture_2d<u32>;

struct LightIn {
    // centre.xy, radius, falloff
    @location(0) place: vec4<f32>,
    // colour × intensity, height for normal maps (0: ignores them)
    @location(1) color: vec4<f32>,
    // strength, softness, shadow map row (-1 unshadowed), unused
    @location(2) shadow: vec4<f32>,
    // direction.xy, cos of the outer half-angle, cos of the inner; a point
    // light's edges are below -1, so every direction is in
    @location(3) cone: vec4<f32>,
    // the light layers it lights
    @location(4) layers: u32,
}

struct LightOut {
    @builtin(position) position: vec4<f32>,
    // Where in the light's disc, -1..1 on each axis.
    @location(0) local: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) falloff: f32,
    @location(3) @interpolate(flat) shadow: vec3<f32>,
    @location(4) @interpolate(flat) cone: vec4<f32>,
    // radius, height for normal maps (0: ignores them)
    @location(5) @interpolate(flat) reach: vec2<f32>,
    @location(6) @interpolate(flat) layers: u32,
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
    out.shadow = light.shadow.xyz;
    out.cone = light.cone;
    out.reach = vec2<f32>(light.place.z, light.color.w);
    out.layers = light.layers;
    return out;
}

@fragment
fn fs_light(in: LightOut) -> @location(0) vec4<f32> {
    // A pixel on none of the light's layers gets nothing from it.
    let excluded = textureLoad(excluded_layers, clamped(in.position.xy, excluded_layers_size()), 0).r;
    if (in.layers & ~excluded) == 0u {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    // Full at the centre, nothing at the radius; the falloff bends the
    // curve between: 1 is a straight ramp, 2 the default soft one.
    let edge = clamp(1.0 - length(in.local), 0.0, 1.0);
    let strength = pow(edge, in.falloff) * within_cone(in.local, in.cone)
        * facing(in.position.xy, in.local, in.reach);
    var lit = 1.0;
    if in.shadow.z >= 0.0 {
        lit = visible(in.local, in.shadow);
    }
    return vec4<f32>(in.color * strength * lit, 1.0);
}

// `pixel` as a texel of a texture `size` big. The buffers are the frame's
// size, but a frame that did not draw one binds a 1x1 stand-in, and reading
// past its edge is only defined where wgpu enforces robust access.
fn clamped(pixel: vec2<f32>, size: vec2<i32>) -> vec2<i32> {
    return min(vec2<i32>(pixel), size - vec2<i32>(1));
}

fn excluded_layers_size() -> vec2<i32> {
    return vec2<i32>(textureDimensions(excluded_layers));
}

// How squarely the surface at a pixel faces the light: the cosine between
// its normal and the way to a light `reach.y` above the scene, `reach.x`
// being the radius that turns `offset` (in radii) into world units. A pixel
// nothing was drawn to faces the screen, (0, 0, 1), as a plain sprite's
// stand-in does — Godot's and Unity's rule — so a normal-mapped sprite and a
// plain one beside it agree on the flat parts. 1 for a light with no height, which ignores all of this.
fn facing(pixel: vec2<f32>, offset: vec2<f32>, reach: vec2<f32>) -> f32 {
    if reach.y <= 0.0 {
        return 1.0;
    }
    let texel = textureLoad(scene_normals, clamped(pixel, vec2<i32>(textureDimensions(scene_normals))), 0);
    var normal = vec3<f32>(0.0, 0.0, 1.0);
    if texel.a >= 0.5 {
        normal = normalize(texel.rgb * 2.0 - 1.0);
    }
    let to_light = normalize(vec3<f32>(-offset * reach.x, reach.y));
    return max(dot(normal, to_light), 0.0);
}

// 1 inside a spot light's cone, 0 outside, fading between its edges; 1
// everywhere for a point light, whose edges are below any cosine. The
// light's own centre is inside every cone.
fn within_cone(offset: vec2<f32>, cone: vec4<f32>) -> f32 {
    let distance = length(offset);
    if distance <= 0.0 {
        return 1.0;
    }
    let along = dot(offset / distance, cone.xy);
    return smoothstep(cone.z, cone.w, along);
}

// Slack on the stored distance so an edge does not shadow itself: a
// fraction of it, since a half float's rounding is a fraction of the value
// (two of its steps at least), plus a floor for the edges at the light.
const BIAS_RELATIVE: f32 = 1.0 / 512.0;
const BIAS_FLOOR: f32 = 1e-5;

// Taps either side of the direction, for a soft edge. With
// `MAX_SHADOW_SOFTNESS` (0.1 rad) they are a degree apart at most.
const TAPS: i32 = 6;

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

// The quarter `direction` falls in: the one whose axis is nearest it.
fn quarter_of(direction: vec2<f32>) -> u32 {
    if abs(direction.x) >= abs(direction.y) {
        return select(2u, 0u, direction.x >= 0.0);
    }
    return select(3u, 1u, direction.y >= 0.0);
}

// How much of the light reaches `offset` (from the light, in radii): one
// minus the strength where an occluder is nearer the light that way,
// averaged over taps turned up to `softness` radians either side. Each tap
// finds its own quarter, so a soft edge crosses the seam between two rows
// rather than stopping at it.
fn visible(offset: vec2<f32>, shadow: vec3<f32>) -> f32 {
    let distance = length(offset);
    if distance <= 0.0 {
        // The light's own centre: nothing can be in front of it.
        return 1.0;
    }
    let width = f32(textureDimensions(shadow_map).x);
    let last = i32(width) - 1;
    let light_row = i32(shadow.z) * 4;
    var clear = 0.0;
    for (var i = -TAPS; i <= TAPS; i++) {
        let turn = shadow.y * f32(i) / f32(TAPS);
        let c = cos(turn);
        let s = sin(turn);
        let direction = vec2<f32>(offset.x * c - offset.y * s, offset.x * s + offset.y * c);
        let q = quarter_of(direction);
        let seen = quarter(direction, q);
        let across = seen.x / seen.y;
        let x = clamp(i32(floor((across * 0.5 + 0.5) * width)), 0, last);
        let nearest = textureLoad(shadow_map, vec2<i32>(x, light_row + i32(q)), 0).r;
        let slack = nearest * BIAS_RELATIVE + BIAS_FLOOR;
        clear += select(0.0, 1.0, distance <= nearest + slack);
    }
    let fraction = clear / f32(2 * TAPS + 1);
    return 1.0 - shadow.x * (1.0 - fraction);
}
