//! Render pass recording, independent of where the pixels end up.
//!
//! Passes take a plain [`wgpu::TextureView`], so the same recording code drives
//! the swapchain, an offscreen texture for a headless test, and — later — the
//! editor viewport.

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
