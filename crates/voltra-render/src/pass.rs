//! Render pass recording, independent of where the pixels end up.
//!
//! Passes take a plain [`wgpu::TextureView`], so the same recording code drives
//! the swapchain, an offscreen texture for a headless test, and — later — the
//! editor viewport.

use crate::mesh::{Mesh2D, Primitives};
use crate::pipeline::SpritePipeline;

/// Records a clear followed by all of `mesh`, if there is one.
///
/// The whole-mesh path, and the only one that can draw geometry with no index
/// buffer: [`draw_mesh_batches`] goes through [`Mesh2D::draw_range`], which is
/// indexed by construction. Sprites take the batched path because they are
/// split into per-texture ranges over one buffer; anything drawn in a single
/// call against a single bind group belongs here.
///
/// `None` still records the pass: the clear is what stops the previous frame
/// showing through when the scene is empty.
///
/// `camera` is bound to group 0 and `texture` to group 1; the pipeline layout
/// and the shader both expect them there. The mesh's triangles draw first,
/// then its quads, each through its half of `pipeline`.
pub fn draw_mesh(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &SpritePipeline,
    camera: &wgpu::BindGroup,
    texture: &wgpu::BindGroup,
    mesh: Option<&Mesh2D>,
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

    pass.set_pipeline(&pipeline.triangles);
    pass.set_bind_group(0, camera, &[]);
    pass.set_bind_group(1, texture, &[]);
    mesh.draw(&mut pass);
    if mesh.quad_count() > 0 {
        pass.set_pipeline(&pipeline.quads);
        pass.set_bind_group(0, camera, &[]);
        pass.set_bind_group(1, texture, &[]);
        mesh.draw_quads(&mut pass, 0..mesh.quad_count());
    }
}

/// One draw call within a [`draw_mesh_batches`] pass: a range of triangles
/// or quads bound against its own texture.
///
/// Carries `&wgpu::BindGroup` rather than any asset type — `voltra-render`
/// does not know what a texture cache is, only that group 1 needs something
/// bindable. Building the bind group from a path or a `Handle` is the
/// caller's job.
pub struct MeshDraw<'a> {
    /// The texture the range is drawn with, bound at group 1.
    pub texture: &'a wgpu::BindGroup,
    /// The range's normal map, bound through its linear view, for the
    /// normals pass (see [`NormalPass`](crate::normals::NormalPass)); `None`
    /// is flat. The colour pass ignores it.
    pub normal: Option<&'a wgpu::BindGroup>,
    /// The range's light layers, for the light-layer pass (see
    /// [`LayerPass`](crate::light_layers::LayerPass)). The colour pass
    /// ignores it.
    pub light_layers: u32,
    /// What to draw: a range of the mesh's triangle indices or of its quad
    /// instances.
    pub primitives: Primitives,
    /// The material the range is drawn with, or `None` for the sprite
    /// pipeline the pass was handed.
    pub material: Option<MaterialDraw<'a>>,
}

/// A range drawn through a material: its pipeline, and where its uniforms
/// are in the frame's uniform buffer.
#[derive(Clone, Copy)]
pub struct MaterialDraw<'a> {
    /// The pipelines the range is drawn with.
    pub pipeline: &'a SpritePipeline,
    /// The frame's uniform blocks, bound at group 2.
    pub uniforms: &'a wgpu::BindGroup,
    /// This range's block in it.
    pub offset: u32,
}

/// Which pipeline a pass last bound, so a run of ranges on one binds it
/// once and a range on another rebinds what changed.
pub(crate) struct Binder<'p> {
    bound: Option<*const wgpu::RenderPipeline>,
    default: &'p SpritePipeline,
    camera: &'p wgpu::BindGroup,
}

impl<'p> Binder<'p> {
    pub(crate) fn new(default: &'p SpritePipeline, camera: &'p wgpu::BindGroup) -> Self {
        Self {
            bound: None,
            default,
            camera,
        }
    }

    /// Binds what `range` is drawn with: its material's pipeline and block,
    /// or the default pipeline — the half for its primitives; then its
    /// texture.
    pub(crate) fn bind(&mut self, pass: &mut wgpu::RenderPass<'_>, range: &MeshDraw<'_>) {
        let pipeline = range
            .material
            .map_or(self.default, |m| m.pipeline)
            .for_primitives(&range.primitives);
        let pointer: *const wgpu::RenderPipeline = pipeline;
        if self.bound != Some(pointer) {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, self.camera, &[]);
            self.bound = Some(pointer);
        }
        pass.set_bind_group(1, range.texture, &[]);
        if let Some(material) = range.material {
            pass.set_bind_group(2, material.uniforms, &[material.offset]);
        }
    }

    /// Something else bound a pipeline: the next range binds its own.
    pub(crate) fn forget(&mut self) {
        self.bound = None;
    }
}

/// Records a clear, then one draw per entry of `draws`, each against its own
/// bind group but sharing the pipeline, camera and mesh buffers.
///
/// `camera` is bound to group 0 with each pipeline; `draws[i].texture` is
/// (re)bound to group 1 before `draws[i].primitives` are drawn. Splitting sprites into
/// contiguous same-texture runs is the caller's job — this only walks
/// whatever ranges it is handed, in order, so painter's order survives.
///
/// `None` mesh or empty `draws` still records the pass and its clear, same as
/// [`draw_mesh`] — an empty scene must still overwrite the previous frame.
pub fn draw_mesh_batches(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &SpritePipeline,
    camera: &wgpu::BindGroup,
    mesh: Option<&Mesh2D>,
    draws: &[MeshDraw<'_>],
    clear: wgpu::Color,
) {
    record_batches(
        encoder,
        view,
        pipeline,
        camera,
        mesh,
        draws,
        wgpu::LoadOp::Clear(clear),
    );
}

/// [`draw_mesh_batches`] over whatever is already in `view`: the
/// screen-space UI, drawn after the world into the same target.
///
/// With nothing to draw no pass is recorded, for the reason [`draw_lines`]
/// gives.
pub fn draw_mesh_batches_over(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &SpritePipeline,
    camera: &wgpu::BindGroup,
    mesh: Option<&Mesh2D>,
    draws: &[MeshDraw<'_>],
) {
    if mesh.is_none() || draws.is_empty() {
        return;
    }
    record_batches(
        encoder,
        view,
        pipeline,
        camera,
        mesh,
        draws,
        wgpu::LoadOp::Load,
    );
}

fn record_batches(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &SpritePipeline,
    camera: &wgpu::BindGroup,
    mesh: Option<&Mesh2D>,
    draws: &[MeshDraw<'_>],
    load: wgpu::LoadOp<wgpu::Color>,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("mesh-batches-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
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

    let mut binder = Binder::new(pipeline, camera);
    for draw in draws {
        binder.bind(&mut pass, draw);
        mesh.draw_primitives(&mut pass, &draw.primitives);
    }
}

/// Records `mesh` over whatever is already in `view`.
///
/// The only pass in the engine that **loads** rather than clears. Every other
/// one is the single thing drawn into its target that frame; an overlay is by
/// definition the second, and clearing here would erase the scene it annotates.
///
/// `camera` is bound to group 0 and `viewport` to group 1, matching
/// [`pipeline::create_lines`](crate::pipeline::create_lines).
///
/// `None` records nothing at all — not even a pass. Unlike the passes above,
/// there is no clear to be the point of recording one: there would be nothing
/// to preserve and nothing to draw, and a load-store pass over an untouched
/// target is bandwidth spent on a no-op.
pub fn draw_lines(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    camera: &wgpu::BindGroup,
    viewport: &wgpu::BindGroup,
    mesh: Option<&Mesh2D>,
) {
    let Some(mesh) = mesh else {
        return;
    };

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("lines-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, camera, &[]);
    pass.set_bind_group(1, viewport, &[]);
    mesh.draw(&mut pass);
}
