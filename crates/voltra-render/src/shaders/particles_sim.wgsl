// Built-in shader: one emitter's particles, aged, moved and born on the GPU.
//
// Must match `particles::uniforms::SimUniform` and `GpuParticle`. Each
// invocation owns one slot of the ring. Slots in the spawn range are born
// this step (age 0, not yet moved — the CPU path's order); every other slot
// still alive is integrated by `dt`. A slot never written is zeroes: age 0,
// lifetime 0, which is dead.

struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    age: f32,
    lifetime: f32,
    _pad: vec2<f32>,
}

struct Sim {
    // origin.xy, dt, spread
    origin_dt: vec4<f32>,
    // basis x axis .xy, y axis .zw: the emitter's rotation and scale, which
    // a world-space velocity goes through.
    basis: vec4<f32>,
    // speed min, max, lifetime min, max
    ranges: vec4<f32>,
    // acceleration.xy, direction, unused
    accel_dir: vec4<f32>,
    // spawn start, spawn count, capacity, seed
    spawn: vec4<u32>,
}

@group(0) @binding(0)
var<storage, read_write> particles: array<Particle>;

@group(0) @binding(1)
var<uniform> sim: Sim;

// PCG hash: a well-mixed 32-bit value from a slot and the step's seed.
fn hash(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn unit(v: u32) -> f32 {
    return f32(hash(v) >> 8u) / 16777216.0;
}

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let capacity = sim.spawn.z;
    let i = id.x;
    if (i >= capacity) {
        return;
    }
    let dt = sim.origin_dt.z;
    let born = (i + capacity - sim.spawn.x) % capacity < sim.spawn.y;
    var p = particles[i];
    if (born) {
        let seed = hash(i ^ hash(sim.spawn.w));
        let angle = sim.accel_dir.z + (unit(seed) - 0.5) * sim.origin_dt.w;
        let speed = mix(sim.ranges.x, sim.ranges.y, unit(seed + 1u));
        let local = vec2<f32>(cos(angle), sin(angle)) * speed;
        p.position = sim.origin_dt.xy;
        p.velocity = sim.basis.xy * local.x + sim.basis.zw * local.y;
        p.age = 0.0;
        p.lifetime = max(mix(sim.ranges.z, sim.ranges.w, unit(seed + 2u)), 0.0);
    } else if (p.age < p.lifetime) {
        p.age = p.age + dt;
        p.velocity = p.velocity + sim.accel_dir.xy * dt;
        p.position = p.position + p.velocity * dt;
    }
    particles[i] = p;
}
