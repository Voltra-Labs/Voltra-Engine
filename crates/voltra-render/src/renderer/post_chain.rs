//! The frame's post chain as a render graph: the scene lit, bloom, then
//! the grade into the frame.

use super::aspect_of;
use crate::bloom::BloomPass;
use crate::context::GpuContext;
use crate::graph::{RenderGraph, Slot, TexturePool};
use crate::lights::{LightPass, Lighting};
use crate::post::{PostPass, PostSettings};

/// The passes the frame's graph is built from, and what lights it.
pub(super) struct Chain<'a> {
    pub lights: &'a LightPass,
    pub bloom: &'a BloomPass,
    pub grade: &'a PostPass,
    /// The world camera, which the lights are drawn through.
    pub camera: &'a wgpu::BindGroup,
    pub lighting: &'a Lighting,
    /// The scene's normals, when they were drawn this frame.
    pub normals: Option<&'a wgpu::TextureView>,
    /// The scene's light layers, when they were drawn this frame.
    pub layers: Option<&'a wgpu::TextureView>,
}

/// The frame's render graph: the scene lit when the frame is, bloom over
/// that when it is on, then the grade into `frame`. A graph that will not
/// validate is a bug in the passes above; it is logged and the frame is
/// left as it was rather than recorded half-built.
#[allow(clippy::too_many_arguments)]
pub(super) fn record_post(
    ctx: &GpuContext,
    chain: Chain<'_>,
    transients: &mut TexturePool,
    timer: &mut crate::gpu_timer::GpuTimer,
    encoder: &mut wgpu::CommandEncoder,
    scene: &wgpu::TextureView,
    frame: &wgpu::TextureView,
    (width, height): (u32, u32),
    post: &PostSettings,
) {
    let (device, queue) = (ctx.device(), ctx.queue());
    let grade = chain.grade;
    let mut graph = RenderGraph::new();
    graph.time_with(timer);
    let drawn = graph.import(scene);
    let frame = graph.import_output(frame);
    let scene = chain
        .lights
        .add(
            &mut graph,
            device,
            ctx.frame_buffers(),
            chain.camera,
            drawn,
            width,
            height,
            chain.lighting,
            chain.normals,
            chain.layers,
        )
        .unwrap_or(drawn);
    let glow = chain
        .bloom
        .add(&mut graph, device, queue, scene, width, height, &post.bloom);
    let reads: Vec<Slot> = std::iter::once(scene).chain(glow).collect();
    let aspect = aspect_of(width, height);
    graph.add_pass("grade", &reads, &[frame], move |encoder, views| {
        grade.record(
            device,
            queue,
            encoder,
            views.get(scene),
            glow.map(|glow| views.get(glow)),
            views.get(frame),
            post,
            aspect,
        );
    });
    if let Err(e) = graph.run(device, encoder, transients) {
        log::error!("{e}; frame not post-processed");
    }
}
