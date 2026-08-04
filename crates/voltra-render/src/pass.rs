//! Render pass recording, independent of where the pixels end up.
//!
//! Passes take a plain [`wgpu::TextureView`], so the same recording code drives
//! the swapchain, an offscreen texture for a headless test, and — later — the
//! editor viewport.

use crate::mesh::Mesh;

/// Records a clear followed by one mesh drawn with `pipeline`.
pub fn draw_mesh(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    mesh: &Mesh,
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

    pass.set_pipeline(pipeline);
    mesh.draw(&mut pass);
}
