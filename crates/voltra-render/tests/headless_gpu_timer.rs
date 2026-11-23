//! A pass timed on a real GPU: the timings come back by name, without the
//! caller waiting for them, once the GPU has run the encoder.
//!
//! Skips itself without an adapter, or with one that cannot write
//! timestamps inside an encoder (the timer then times nothing, which is
//! also checked).

use voltra_render::{wgpu, GpuContext, GpuTimer};

#[test]
fn a_timed_pass_comes_back_by_name() {
    let Some(ctx) = GpuContext::headless(16, 16) else {
        eprintln!("skipping: no GPU adapter");
        return;
    };
    let (device, queue) = (ctx.device(), ctx.queue());
    let mut timer = GpuTimer::new(device, queue);
    timer.set_enabled(true);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("timed"),
        size: wgpu::Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    timer.time(&mut encoder, "clear", |encoder| {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    });
    timer.finish(&mut encoder);
    queue.submit(Some(encoder.finish()));
    timer.submitted();

    if !timer.is_supported() {
        assert!(
            timer.collect(device).is_empty(),
            "an unsupported timer times nothing"
        );
        eprintln!("skipping: the adapter cannot time passes");
        return;
    }
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the GPU finishes");
    let timings = timer.collect(device);
    assert_eq!(timings.len(), 1, "{timings:?}");
    assert_eq!(timings[0].0, "clear");
    assert!(timings[0].1 >= 0.0 && timings[0].1 < 1000.0, "{timings:?}");
    assert!(timer.collect(device).is_empty(), "each timing once");
}
