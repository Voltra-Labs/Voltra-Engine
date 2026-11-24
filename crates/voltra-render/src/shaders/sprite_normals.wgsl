// Built-in shader: the scene's normals, for lights with a height.
//
// The sprite batch drawn again, range for range in painter's order, into a
// normal buffer instead of the frame: each texel is the world-space normal
// of whatever was drawn last there, encoded `n * 0.5 + 0.5`, alpha 1. A
// range without a normal map binds a flat stand-in, so it still covers what
// is under it, facing the screen. The normal map's own alpha is ignored (a
// map with height packed there is still all normals). Texels the colour
// leaves less than half covered are skipped: coverage is a 0.5 alpha test,
// not a blend.
//
// The tangent frame comes from screen-space derivatives of the world
// position and the UV, so a turned, scaled or flipped sprite needs nothing
// extra in its vertices: red follows the image's +u, green its up (-v, as
// image rows run down), blue points out of the screen. OpenGL's convention.
//
// Vertex layout is `Vertex::LAYOUT` (`vs_normals`) or a quad instance
// (`vs_normals_quad`, `quad.wgsl`); group 0 `CameraBinding`, groups 1 and
// 2 `texture::bind_group_layout` (the colour, then the normal map through
// its linear view). Read back by `lights.wgsl`.

struct Camera {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var albedo: texture_2d<f32>;
@group(1) @binding(1)
var albedo_sampler: sampler;

@group(2) @binding(0)
var normal_map: texture_2d<f32>;
@group(2) @binding(1)
var normal_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec2<f32>,
    @location(2) alpha: f32,
}

fn vertex(position: vec2<f32>, color: vec4<f32>, uv: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    out.world = position;
    out.alpha = color.a;
    return out;
}

@vertex
fn vs_normals(in: VertexInput) -> VertexOutput {
    return vertex(in.position, in.color, in.uv);
}

@vertex
fn vs_normals_quad(@builtin(vertex_index) index: u32, quad: QuadInput) -> VertexOutput {
    let corner = quad_corner(quad, index);
    return vertex(corner.position, corner.color, corner.uv);
}

@fragment
fn fs_normals(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sampled and differentiated before any discard, in uniform control flow.
    let covered = textureSample(albedo, albedo_sampler, in.uv).a * in.alpha;
    let texel = textureSample(normal_map, normal_sampler, in.uv);
    let dp_x = dpdx(in.world);
    let dp_y = dpdy(in.world);
    let duv_x = dpdx(in.uv);
    let duv_y = dpdy(in.uv);
    if covered < 0.5 {
        discard;
    }

    // World per UV: the world derivatives times the inverse of the UV ones.
    let det = duv_x.x * duv_y.y - duv_x.y * duv_y.x;
    var along_u = vec2<f32>(1.0, 0.0);
    var along_v = vec2<f32>(0.0, -1.0);
    if abs(det) > 1e-12 {
        along_u = (dp_x * duv_y.y - dp_y * duv_x.y) / det;
        along_v = (dp_y * duv_x.x - dp_x * duv_y.x) / det;
    }
    let right = normalize(along_u);
    let up = normalize(-along_v);

    let local = texel.rgb * 2.0 - 1.0;
    let world = normalize(vec3<f32>(right * local.x + up * local.y, local.z));
    return vec4<f32>(world * 0.5 + 0.5, 1.0);
}
