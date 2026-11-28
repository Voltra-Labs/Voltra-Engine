//! A 3D camera's shot: the scene through the depth pass into its HDR
//! targets, then bloom and the tone curve into a picture — the window or
//! a target — with the HUD over it when there is one.

use std::collections::HashSet;

use super::post_chain::{record_post, Chain};
use super::{Layer, Renderer};
use crate::lights::Lighting;
use crate::post::{Grade, PictureAlpha};
use crate::render3d::{
    CustomShading, Hdr3D, HdrDegraded, Look3D, Pass3D, Scene3D, StandardSurface, SurfaceLayouts,
    SurfaceMaterial3D, SurfaceTextures, Targets3D,
};
use crate::target::RenderTarget;

/// How many shots' targets are kept at most, by size: a split screen and a
/// picture in a picture are three. The backstop to the age rule
/// ([`Renderer::begin_frame`]): past it the least recently drawn is let
/// go, and a frame of more 3D sizes than this remakes some each frame.
const KEPT_TARGETS: usize = 4;

/// What the renderer keeps for 3D, made with the first 3D frame: a 2D game
/// never builds a pass or a target.
#[derive(Debug)]
pub(super) struct State3D {
    pass: Option<Pass3D>,
    /// What surface pipelines and materials bind against; made with the
    /// first material.
    layouts: Option<SurfaceLayouts>,
    /// Targets by use, the most recent last, each with the frame it was
    /// last drawn in.
    targets: Vec<(Targets3D, u64)>,
    /// The frame being drawn: counted by [`Renderer::begin_frame`].
    frame: u64,
    /// The samples a pixel asked for: 1 or 4.
    samples: u32,
    /// What has been said about the device falling short, once each.
    warned: HashSet<HdrDegraded>,
}

impl Default for State3D {
    fn default() -> Self {
        Self {
            pass: None,
            layouts: None,
            targets: Vec::new(),
            frame: 0,
            samples: Hdr3D::DEFAULT_SAMPLES,
            warned: HashSet::new(),
        }
    }
}

impl State3D {
    /// Targets for `hdr` at `width` × `height`: one kept from an earlier
    /// shot of that size, or new.
    fn acquire(
        &mut self,
        device: &wgpu::Device,
        hdr: Hdr3D,
        (width, height): (u32, u32),
    ) -> &Targets3D {
        let size = (width.max(1), height.max(1));
        let found = self
            .targets
            .iter()
            .position(|(t, _)| t.hdr() == hdr && t.size() == size);
        let targets = match found {
            Some(at) => self.targets.remove(at).0,
            None => {
                if self.targets.len() >= KEPT_TARGETS {
                    self.targets.remove(0);
                }
                Targets3D::new(device, hdr, width, height)
            }
        };
        self.targets.push((targets, self.frame));
        &self.targets.last().expect("pushed just above").0
    }

    /// Starts frame `frame + 1`: lets go of every target drawn in neither
    /// the frame just ended nor the one before — a size a window was
    /// dragged through, a sample count since changed.
    fn begin_frame(&mut self) {
        self.frame += 1;
        let frame = self.frame;
        self.targets.retain(|(_, used)| used + 2 >= frame);
    }
}

impl Renderer {
    /// Marks the start of a frame. What it keeps between frames for shots
    /// it no longer draws — 3D targets of a size not drawn this frame or
    /// the last — is let go. Call once before each frame's draws; a
    /// renderer never told keeps its [`KEPT_TARGETS`] most recent.
    pub fn begin_frame(&mut self) {
        self.state3d.begin_frame();
    }

    /// How many 3D shots' target sets are kept: for a test, or a memory
    /// readout.
    pub fn kept_3d_targets(&self) -> usize {
        self.state3d.targets.len()
    }

    /// Sets the samples a pixel 3D shots are drawn with: 1 (off) or 4, the
    /// default (`default.render.ron`'s `msaa`). Anything above 1 is 4; the
    /// device may still allow only 1 ([`hdr_3d`](Self::hdr_3d)).
    pub fn set_msaa(&mut self, samples: u32) {
        self.state3d.samples = if samples > 1 { 4 } else { 1 };
    }

    /// The samples a pixel asked for: see [`set_msaa`](Self::set_msaa).
    pub fn msaa(&self) -> u32 {
        self.state3d.samples
    }

    /// The format and samples the next 3D shot is drawn with: `Rgba16Float`
    /// at [`msaa`](Self::msaa) where the device can, less where it cannot,
    /// said once a session.
    pub fn hdr_3d(&mut self) -> Hdr3D {
        let (hdr, degraded) = Hdr3D::choose(
            self.ctx.target_support(),
            self.ctx.format(),
            self.state3d.samples,
        );
        for why in degraded {
            if self.state3d.warned.insert(why) {
                log::warn!("{why}");
            }
        }
        hdr
    }

    /// What surface materials are bound against: made the first time it is
    /// asked for, and the same for the renderer's life.
    pub fn surface_layouts(&mut self) -> &SurfaceLayouts {
        let (device, queue) = (self.ctx.device(), self.ctx.queue());
        self.state3d
            .layouts
            .get_or_insert_with(|| SurfaceLayouts::new(device, queue))
    }

    /// `surface` with `textures`, made for this renderer's 3D pass: what a
    /// [`MeshDraw3D`](crate::render3d::MeshDraw3D) names. Keep it while
    /// it is drawn; [`SurfaceMaterial3D::is_current`] says when to make it
    /// again.
    pub fn surface_material(
        &mut self,
        surface: &StandardSurface,
        textures: &SurfaceTextures<'_>,
    ) -> SurfaceMaterial3D {
        self.custom_surface_material(surface, textures, None)
    }

    /// [`surface_material`](Self::surface_material) drawn through a custom
    /// surface's shader and uniforms, or the standard shader for `None`.
    pub fn custom_surface_material(
        &mut self,
        surface: &StandardSurface,
        textures: &SurfaceTextures<'_>,
        custom: Option<&CustomShading>,
    ) -> SurfaceMaterial3D {
        let device = self.ctx.device().clone();
        SurfaceMaterial3D::new(&device, self.surface_layouts(), surface, textures, custom)
    }

    /// Draws `scene` to the window as `look` says, then `ui` over it in the
    /// window's pixels, and presents: a lone full-screen 3D camera.
    pub fn render_3d(&mut self, scene: &Scene3D<'_>, look: &Look3D, ui: Layer<'_>) {
        let Some(frame) = self.ctx.acquire() else {
            return;
        };
        let view = self.ctx.frame_view(&frame);
        let size = (self.ctx.config().width, self.ctx.config().height);
        self.record_3d(&view, size, scene, look, PictureAlpha::Premultiplied, ui);
        self.ctx.present(frame);
    }

    /// Draws `scene` into `output` as `look` says, then `ui` over it in its
    /// pixels: a 3D shot's picture — the editor's view, a headless
    /// screenshot, a part of a frame of several cameras — premultiplied, as
    /// the composite lays it. `output` is in the renderer's
    /// [`format`](crate::GpuContext::format), as every picture is.
    pub fn render_3d_into(
        &mut self,
        output: &RenderTarget,
        scene: &Scene3D<'_>,
        look: &Look3D,
        ui: Layer<'_>,
    ) {
        self.render_3d_picture(output, scene, look, PictureAlpha::Premultiplied, ui);
    }

    /// [`render_3d_into`](Self::render_3d_into) a render texture's picture:
    /// in straight alpha, as a sprite or a HUD image showing it blends it.
    /// A camera clearing to a half-transparent colour leaves its texture
    /// half-transparent, and not darkened twice.
    pub fn render_3d_texture(&mut self, output: &RenderTarget, scene: &Scene3D<'_>, look: &Look3D) {
        self.render_3d_picture(output, scene, look, PictureAlpha::Straight, Layer::EMPTY);
    }

    fn render_3d_picture(
        &mut self,
        output: &RenderTarget,
        scene: &Scene3D<'_>,
        look: &Look3D,
        alpha: PictureAlpha,
        ui: Layer<'_>,
    ) {
        if output.format() != self.ctx.format() {
            log::error!(
                "a 3D picture into {:?}, not the frame's {:?}; skipped (a caller bug)",
                output.format(),
                self.ctx.format()
            );
            return;
        }
        let size = (output.width(), output.height());
        self.record_3d(output.view(), size, scene, look, alpha, ui);
    }

    fn record_3d(
        &mut self,
        view: &wgpu::TextureView,
        size: (u32, u32),
        scene: &Scene3D<'_>,
        look: &Look3D,
        alpha: PictureAlpha,
        ui: Layer<'_>,
    ) {
        let hdr = self.hdr_3d();
        let device = self.ctx.device().clone();
        // Taken out while the targets are borrowed, put back after.
        let mut pass = match self.state3d.pass.take() {
            Some(pass) if pass.hdr() == hdr => pass,
            _ => Pass3D::new(&device, &self.surface_layouts().clone(), hdr),
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame-3d-encoder"),
        });
        let scope = self.gpu_timer.begin(&mut encoder, "3d");
        let targets = self.state3d.acquire(&device, hdr, size);
        pass.record(&device, self.ctx.queue(), &mut encoder, targets, scene);
        self.gpu_timer.end(&mut encoder, scope);
        let unlit = Lighting::default();
        record_post(
            &self.ctx,
            Chain {
                lights: &self.lights,
                bloom: &self.bloom,
                grade: &self.post,
                camera: self.camera_binding.bind_group(),
                lighting: &unlit,
                normals: None,
                layers: None,
                mode: Grade::Tonemap(look.tonemap, alpha),
            },
            &mut self.transients,
            &mut self.gpu_timer,
            &mut encoder,
            targets.picture(),
            view,
            size,
            &look.post,
        );
        self.state3d.pass = Some(pass);
        let scope = self.gpu_timer.begin(&mut encoder, "hud");
        self.record_ui(&mut encoder, view, size.0, size.1, ui);
        self.gpu_timer.end(&mut encoder, scope);
        self.gpu_timer.finish(&mut encoder);
        self.ctx.submit(encoder.finish());
        self.gpu_timer.submitted();
    }
}
