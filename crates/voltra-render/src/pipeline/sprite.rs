//! A sprite-batch pipeline as the pair it is: one for the batch's
//! triangles, one for its quad instances, sharing a fragment stage.

use crate::mesh::{Primitives, Vertex2D};
use crate::quad::QuadInstance;

/// The two pipelines one sprite-batch shader is drawn through: its
/// triangles read [`Vertex2D::LAYOUT`], its quads [`QuadInstance::LAYOUT`]
/// with corners made in the vertex stage. Everything else — layout,
/// fragment stage, blending, target — is the same, so a range picks its
/// pipeline by what it draws ([`for_primitives`](Self::for_primitives)) and
/// looks the same either way.
pub struct SpritePipeline {
    /// Draws [`Primitives::Triangles`].
    pub triangles: wgpu::RenderPipeline,
    /// Draws [`Primitives::Quads`].
    pub quads: wgpu::RenderPipeline,
}

/// What a [`SpritePipeline`] is made from: a shader module with both vertex
/// entry points, and what both pipelines share.
pub struct SpritePipelineDescriptor<'a> {
    /// Labels both pipelines; the quad one gets `-quads` after it.
    pub label: &'a str,
    /// Bind group layouts, shared.
    pub layout: &'a wgpu::PipelineLayout,
    /// The module holding both vertex entry points and the fragment one.
    pub module: &'a wgpu::ShaderModule,
    /// The vertex entry point reading [`Vertex2D`]s.
    pub triangles_entry: &'a str,
    /// The vertex entry point reading [`QuadInstance`]s and the vertex index.
    pub quads_entry: &'a str,
    /// The fragment entry point.
    pub fragment_entry: &'a str,
    /// The colour target, with its blending.
    pub target: wgpu::ColorTargetState,
}

impl SpritePipeline {
    /// Both pipelines `desc` describes.
    pub fn new(device: &wgpu::Device, desc: &SpritePipelineDescriptor<'_>) -> Self {
        let make = |label: &str, entry: &str, buffers: &[Option<wgpu::VertexBufferLayout<'_>>]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(desc.layout),
                vertex: wgpu::VertexState {
                    module: desc.module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    // 2D geometry is always viewed from one side, so culling
                    // would only cost a winding rule to get wrong — and a
                    // flipped sprite's quad winds the other way.
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: desc.module,
                    entry_point: Some(desc.fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(desc.target.clone())],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            triangles: make(desc.label, desc.triangles_entry, &[Some(Vertex2D::LAYOUT)]),
            quads: make(
                &format!("{}-quads", desc.label),
                desc.quads_entry,
                &[Some(QuadInstance::LAYOUT)],
            ),
        }
    }

    /// The pipeline `primitives` are drawn through.
    pub fn for_primitives(&self, primitives: &Primitives) -> &wgpu::RenderPipeline {
        if primitives.is_quads() {
            &self.quads
        } else {
            &self.triangles
        }
    }
}
