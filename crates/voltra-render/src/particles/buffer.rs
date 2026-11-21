//! One emitter's particles on the GPU: the ring, its two uniforms and the
//! bind groups the pipelines read them through.

use wgpu::util::DeviceExt;

use super::pipelines::ParticlePipelines;
use super::uniforms::{LookUniform, SimUniform};
use super::{ParticleLook, SimStep, MAX_GPU_PARTICLES, PARTICLE_SIZE};

/// The workgroup size `particles_sim.wgsl` declares.
const WORKGROUP: u32 = 64;

/// A ring of `capacity` particles and what steps and draws it.
pub struct GpuParticles {
    capacity: u32,
    particles: wgpu::Buffer,
    sim: wgpu::Buffer,
    look: wgpu::Buffer,
    simulate_group: wgpu::BindGroup,
    draw_group: wgpu::BindGroup,
}

impl GpuParticles {
    /// A ring of `capacity` slots — at least one, at most
    /// [`MAX_GPU_PARTICLES`] — every one dead.
    pub fn new(device: &wgpu::Device, pipelines: &ParticlePipelines, capacity: u32) -> Self {
        let capacity = capacity.clamp(1, MAX_GPU_PARTICLES);
        // Zeroed by wgpu on creation: age 0, lifetime 0, which is dead.
        let particles = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particles"),
            size: u64::from(capacity) * PARTICLE_SIZE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let sim = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("particles-sim"),
            contents: bytemuck::bytes_of(&SimUniform::new(&SimStep::default(), capacity)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let look = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("particles-look"),
            contents: bytemuck::bytes_of(&LookUniform::new(&ParticleLook::default())),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let group = |label, layout, uniform: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particles.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            })
        };
        let simulate_group = group("particles-simulate", &pipelines.simulate_layout, &sim);
        let draw_group = group("particles-draw", &pipelines.draw_layout, &look);
        Self {
            capacity,
            particles,
            sim,
            look,
            simulate_group,
            draw_group,
        }
    }

    /// Slots in the ring.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// The ring itself, for a test reading particles back.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.particles
    }

    /// Records one step into `encoder`.
    ///
    /// One step per emitter per submission: the uniform is written through
    /// the queue, which lands before the submission runs, so a second step
    /// recorded into the same encoder would see the second step's values
    /// twice.
    pub fn simulate(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &ParticlePipelines,
        step: &SimStep,
    ) {
        queue.write_buffer(
            &self.sim,
            0,
            bytemuck::bytes_of(&SimUniform::new(step, self.capacity)),
        );
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("particles-simulate"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipelines.simulate);
        pass.set_bind_group(0, &self.simulate_group, &[]);
        pass.dispatch_workgroups(self.capacity.div_ceil(WORKGROUP), 1, 1);
    }

    /// Sets how the particles are drawn from the next submission on.
    pub fn set_look(&self, queue: &wgpu::Queue, look: &ParticleLook) {
        queue.write_buffer(&self.look, 0, bytemuck::bytes_of(&LookUniform::new(look)));
    }

    /// Draws every slot into `pass`, which has the particle pipeline and
    /// groups 0 and 1 bound already. Dead slots draw nothing.
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_bind_group(2, &self.draw_group, &[]);
        pass.draw(0..6, 0..self.capacity);
    }
}
