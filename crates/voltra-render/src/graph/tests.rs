//! What the graph runs, what it refuses, and how few textures it needs.
//! Skips itself when no GPU adapter is available.

use std::cell::RefCell;

use super::*;

const DESC: TextureDesc = TextureDesc {
    label: "graph-test",
    width: 8,
    height: 8,
    format: wgpu::TextureFormat::Rgba16Float,
};

fn device() -> Option<(wgpu::Device, wgpu::Queue, RenderTargetPair)> {
    let (device, queue) = voltra_testkit::headless_device()?;
    let make = |label| {
        crate::RenderTarget::new(
            &device,
            label,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            8,
            8,
            crate::Filter::Linear,
        )
    };
    let pair = RenderTargetPair {
        input: make("in"),
        output: make("out"),
    };
    Some((device, queue, pair))
}

struct RenderTargetPair {
    input: crate::RenderTarget,
    output: crate::RenderTarget,
}

fn encoder(device: &wgpu::Device) -> wgpu::CommandEncoder {
    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None })
}

#[test]
fn a_pass_leading_to_no_output_is_culled() {
    let Some((device, queue, targets)) = device() else {
        return;
    };
    let ran = RefCell::new(Vec::new());
    let mut graph = RenderGraph::new();
    let input = graph.import(targets.input.view());
    let output = graph.import_output(targets.output.view());
    let middle = graph.create(DESC);
    let unused = graph.create(DESC);
    graph.add_pass("blur", &[input], &[middle], |_, _| {
        ran.borrow_mut().push("blur")
    });
    graph.add_pass("debug", &[input], &[unused], |_, _| {
        ran.borrow_mut().push("debug")
    });
    graph.add_pass("composite", &[input, middle], &[output], |_, _| {
        ran.borrow_mut().push("composite")
    });
    assert_eq!(graph.live_passes(), Ok(2));

    let mut pool = TexturePool::default();
    let mut enc = encoder(&device);
    graph.run(&device, &mut enc, &mut pool).expect("valid");
    queue.submit(Some(enc.finish()));
    assert_eq!(*ran.borrow(), ["blur", "composite"]);
    assert_eq!(pool.len(), 1, "the culled pass's texture was never made");
}

#[test]
fn reading_what_nothing_wrote_is_an_error_and_records_nothing() {
    let Some((device, _queue, targets)) = device() else {
        return;
    };
    let ran = RefCell::new(0);
    let mut graph = RenderGraph::new();
    let output = graph.import_output(targets.output.view());
    let never = graph.create(DESC);
    graph.add_pass("composite", &[never], &[output], |_, _| {
        *ran.borrow_mut() += 1
    });
    let mut pool = TexturePool::default();
    let result = graph.run(&device, &mut encoder(&device), &mut pool);
    assert_eq!(
        result,
        Err(GraphError::ReadBeforeWrite { pass: "composite" })
    );
    assert_eq!(*ran.borrow(), 0);
}

/// A chain a → b → c → out: `a` is done before `c` is written, so the two
/// share one texture; `b` overlaps both and needs its own.
fn chain<'a>(graph: &mut RenderGraph<'a>, input: Slot, output: Slot) {
    let a = graph.create(DESC);
    let b = graph.create(DESC);
    let c = graph.create(DESC);
    graph.add_pass("a", &[input], &[a], |_, _| {});
    graph.add_pass("b", &[a], &[b], |_, _| {});
    graph.add_pass("c", &[b], &[c], |_, _| {});
    graph.add_pass("out", &[c], &[output], |_, _| {});
}

#[test]
fn textures_are_reused_within_a_frame_and_across_frames() {
    let Some((device, queue, targets)) = device() else {
        return;
    };
    let mut pool = TexturePool::default();
    for _ in 0..3 {
        let mut graph = RenderGraph::new();
        let input = graph.import(targets.input.view());
        let output = graph.import_output(targets.output.view());
        chain(&mut graph, input, output);
        let mut enc = encoder(&device);
        graph.run(&device, &mut enc, &mut pool).expect("valid");
        queue.submit(Some(enc.finish()));
        assert_eq!(pool.len(), 2, "three slots, two textures, every frame");
    }

    // Frames that ask for nothing let them go.
    for _ in 0..TexturePool::KEEP_FRAMES {
        let mut graph = RenderGraph::new();
        graph.import_output(targets.output.view());
        graph
            .run(&device, &mut encoder(&device), &mut pool)
            .expect("valid");
    }
    assert!(pool.is_empty());
}

#[test]
fn a_pass_sees_the_views_it_declared() {
    let Some((device, queue, targets)) = device() else {
        return;
    };
    let seen = RefCell::new(None);
    let mut graph = RenderGraph::new();
    let output = graph.import_output(targets.output.view());
    graph.add_pass("clear", &[], &[output], |encoder, views| {
        let view = views.get(output);
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
        *seen.borrow_mut() = Some(());
    });
    let mut pool = TexturePool::default();
    let mut enc = encoder(&device);
    graph.run(&device, &mut enc, &mut pool).expect("valid");
    queue.submit(Some(enc.finish()));
    let pixels = voltra_testkit::read_texture(&device, &queue, targets.output.texture(), 8, 8);
    assert!(seen.borrow().is_some());
    assert_eq!((pixels[0].r, pixels[0].g), (255, 0));
}
