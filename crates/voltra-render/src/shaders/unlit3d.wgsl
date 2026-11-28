// Unlit 3D meshes, instanced: each instance's model matrix as three rows,
// a flat tint. Normals, UVs and tangents are in the vertex buffer for the
// lit materials; this pass reads the position alone.

struct View3D {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> view: View3D;

struct VertexIn {
    @location(0) position: vec3<f32>,
};

struct InstanceIn {
    @location(4) model_x: vec4<f32>,
    @location(5) model_y: vec4<f32>,
    @location(6) model_z: vec4<f32>,
    @location(10) tint: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) tint: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexIn, instance: InstanceIn) -> VertexOut {
    let local = vec4<f32>(vertex.position, 1.0);
    let world = vec3<f32>(
        dot(instance.model_x, local),
        dot(instance.model_y, local),
        dot(instance.model_z, local),
    );
    var out: VertexOut;
    out.clip = view.view_proj * vec4<f32>(world, 1.0);
    out.tint = instance.tint;
    return out;
}

// Opaque: alpha 1 whatever the tint's, so a 3D overlay is see-through
// only where nothing was drawn (alpha modes are the materials').
@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.tint.rgb, 1.0);
}
