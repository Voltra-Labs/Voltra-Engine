//! One emitter's particles on the GPU: the ring, its two uniforms, the
//! shapes it collides with and the bind groups the pipelines read them
//! through.

use wgpu::util::DeviceExt;

use super::colliders::{ColliderGpu, ParticleCollider, COLLIDER_SIZE};
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
    /// Room for `collider_room` shapes, the first `collider_count` live.
    colliders: wgpu::Buffer,
    collider_room: u32,
    collider_count: u32,
    simulate_group: wgpu::BindGroup,
    draw_group: wgpu::BindGroup,
}

impl GpuParticles {
    /// A ring of `capacity` slots — at least one, at most
    /// [`MAX_GPU_PARTICLES`] — every one dead, colliding with nothing.
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
            contents: bytemuck::bytes_of(&SimUniform::new(&SimStep::default(), capacity, 0)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let look = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("particles-look"),
            contents: bytemuck::bytes_of(&LookUniform::new(&ParticleLook::default())),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        // A binding cannot be empty: room for one shape, none of it live.
        let colliders = collider_buffer(device, 1);
        let simulate_group = simulate_group(device, pipelines, &particles, &sim, &colliders);
        let draw_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particles-draw"),
            layout: &pipelines.draw_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: particles.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: look.as_entire_binding(),
                },
            ],
        });
        Self {
            capacity,
            particles,
            sim,
            look,
            colliders,
            collider_room: 1,
            collider_count: 0,
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

    /// How many shapes the next step tests each particle against.
    pub fn collider_count(&self) -> u32 {
        self.collider_count
    }

    /// Sets the shapes every step from the next on tests against, in the
    /// world. One that is not [valid](ParticleCollider::is_valid) is left
    /// out. The buffer grows to fit and is kept when the list shrinks.
    pub fn set_colliders(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipelines: &ParticlePipelines,
        colliders: &[ParticleCollider],
    ) {
        let packed: Vec<ColliderGpu> = colliders
            .iter()
            .filter(|collider| collider.is_valid())
            .map(ColliderGpu::new)
            .collect();
        let count = u32::try_from(packed.len()).unwrap_or(u32::MAX);
        if count > self.collider_room {
            // Doubling, so a list that creeps up one shape a frame does not
            // rebuild the buffer every frame.
            let room = count.max(self.collider_room.saturating_mul(2));
            self.colliders = collider_buffer(device, room);
            self.collider_room = room;
            self.simulate_group = simulate_group(
                device,
                pipelines,
                &self.particles,
                &self.sim,
                &self.colliders,
            );
        }
        if !packed.is_empty() {
            queue.write_buffer(&self.colliders, 0, bytemuck::cast_slice(&packed));
        }
        self.collider_count = count.min(self.collider_room);
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
            bytemuck::bytes_of(&SimUniform::new(step, self.capacity, self.collider_count)),
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

fn collider_buffer(device: &wgpu::Device, room: u32) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("particles-colliders"),
        size: u64::from(room.max(1)) * COLLIDER_SIZE,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn simulate_group(
    device: &wgpu::Device,
    pipelines: &ParticlePipelines,
    particles: &wgpu::Buffer,
    sim: &wgpu::Buffer,
    colliders: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("particles-simulate"),
        layout: &pipelines.simulate_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: particles.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: sim.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: colliders.as_entire_binding(),
            },
        ],
    })
}
