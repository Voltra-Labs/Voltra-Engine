//! Render pass recording, independent of where the pixels end up.
//!
//! Passes take a plain [`wgpu::TextureView`], so the same recording code drives
//! the swapchain, an offscreen texture for a headless test, and — later — the
//! editor viewport.

use std::ops::Range;

use crate::mesh::Mesh;

/// Records a clear followed by `mesh`, if there is one.
///
/// `None` still records the pass: the clear is what stops the previous frame
/// showing through when the scene is empty.
///
/// `camera` is bound to group 0 and `texture` to group 1; the pipeline layout
/// and the shader both expect them there.
pub fn draw_mesh(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    camera: &wgpu::BindGroup,
    texture: &wgpu::BindGroup,
    mesh: Option<&Mesh>,
    clear: wgpu::Color,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("flat-color-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    let Some(mesh) = mesh else {
        return;
    };

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, camera, &[]);
    pass.set_bind_group(1, texture, &[]);
    mesh.draw(&mut pass);
}

/// One draw call within a [`draw_mesh_batches`] pass: an index range bound
/// against its own texture.
///
/// Carries `&wgpu::BindGroup` rather than any asset type — `voltra-render`
/// does not know what a texture cache is, only that group 1 needs something
/// bindable. Building the bind group from a path or a `Handle` is the
/// caller's job.
pub struct MeshDraw<'a> {
    pub texture: &'a wgpu::BindGroup,
    pub indices: Range<u32>,
}

/// Records a clear, then one draw per entry of `draws`, each against its own
/// bind group but sharing the pipeline, camera and mesh buffers.
///
/// `camera` is bound to group 0 once; `draws[i].texture` is (re)bound to
/// group 1 before `mesh.draw_range(draws[i].indices)`. Splitting sprites into
/// contiguous same-texture runs is the caller's job — this only walks
/// whatever ranges it is handed, in order, so painter's order survives.
///
/// `None` mesh or empty `draws` still records the pass and its clear, same as
/// [`draw_mesh`] — an empty scene must still overwrite the previous frame.
pub fn draw_mesh_batches(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    camera: &wgpu::BindGroup,
    mesh: Option<&Mesh>,
    draws: &[MeshDraw<'_>],
    clear: wgpu::Color,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("mesh-batches-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    let Some(mesh) = mesh else {
        return;
    };
    if draws.is_empty() {
        return;
    }

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, camera, &[]);
    for draw in draws {
        pass.set_bind_group(1, draw.texture, &[]);
        mesh.draw_range(&mut pass, draw.indices.clone());
    }
}
