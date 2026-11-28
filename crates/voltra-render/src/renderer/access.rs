//! What a renderer hands out and what it is told: its context, layouts and
//! pipelines, vsync, particles stepped, the surface resized.

use super::{aspect_of, Renderer};
use crate::context::GpuContext;
use crate::particles::{GpuParticles, ParticleLook, ParticlePipelines, SimStep};

impl Renderer {
    /// The GPU context the renderer draws on.
    pub fn context(&self) -> &GpuContext {
        &self.ctx
    }

    /// Whether frames wait for the display's refresh; see
    /// [`GpuContext::vsync`].
    pub fn vsync(&self) -> bool {
        self.ctx.vsync()
    }

    /// Turns vsync on or off: the surface's present mode, reconfigured.
    /// What an options screen's toggle asks for, through the platform layer,
    /// which owns no surface of its own.
    pub fn set_vsync(&mut self, on: bool) {
        self.ctx.set_vsync(on);
    }

    /// The layout of group 0, the camera, every sprite pipeline binds:
    /// what a material pipeline is made against.
    pub fn camera_layout(&self) -> &wgpu::BindGroupLayout {
        self.camera_binding.layout()
    }

    /// Layout every texture bind group must be built against.
    ///
    /// The pipeline was created with this exact layout object; a second one
    /// built from the same descriptor is a distinct type as far as wgpu's
    /// validation is concerned and will not bind against it.
    pub fn texture_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_layout
    }

    /// The bind group drawn against when geometry has no texture of its own.
    /// The GPU particle pipelines, or `None` where the adapter cannot run
    /// them — the caller's cue to simulate on the CPU.
    pub fn particle_pipelines(&self) -> Option<&ParticlePipelines> {
        self.particles.as_ref()
    }

    /// The pipeline skinned meshes are made against and drawn through.
    pub fn skin_pipeline(&self) -> &crate::skinning::SkinPipeline {
        &self.skin
    }

    /// Steps every emitter in `steps` and sets how each is drawn, in one
    /// submission ahead of the frame's. Nothing without particle pipelines.
    pub fn step_particles<'a>(
        &self,
        steps: impl IntoIterator<Item = (&'a GpuParticles, &'a SimStep, &'a ParticleLook)>,
    ) {
        let Some(pipelines) = &self.particles else {
            return;
        };
        let mut encoder =
            self.ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("particles-encoder"),
                });
        let mut any = false;
        for (particles, step, look) in steps {
            particles.simulate(self.ctx.queue(), &mut encoder, pipelines, step);
            particles.set_look(self.ctx.queue(), look);
            any = true;
        }
        if any {
            self.ctx.submit(encoder.finish());
        }
    }

    /// The bind group drawn against when geometry has no texture of its own.
    pub fn white_bind_group(&self) -> &wgpu::BindGroup {
        &self.white_bind_group
    }

    /// Resizes the surface and sets the camera's aspect to match.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.ctx.resize(width, height);
        // Without this the image stretches when the window is not square.
        self.camera.aspect = aspect_of(width, height);
    }
}
