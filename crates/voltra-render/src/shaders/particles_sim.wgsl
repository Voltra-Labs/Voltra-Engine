// Built-in shader: one emitter's particles, aged, moved, collided and born
// on the GPU.
//
// Must match `particles::uniforms::SimUniform`, `GpuParticle` and
// `particles::colliders::ColliderGpu`. Each invocation owns one slot of the
// ring. Slots in the spawn range are born this step (age 0, not yet moved:
// the CPU path's order); every other slot still alive is integrated by `dt`
// and its path tested against the colliders. A slot never written is
// zeroes: age 0, lifetime 0, which is dead.
//
// The collision is the CPU's `particles::collide` line for line: the path
// from the old position to the new one, in the world, swept against every
// shape; the earliest hit puts the particle just outside the surface and
// turns its velocity; the rest of the step's motion is dropped. A particle
// that starts a step inside a shape dies.

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
    // The particle's space into the world: axes, then translation.
    to_world: vec4<f32>,
    to_world_t: vec4<f32>,
    // And back.
    from_world: vec4<f32>,
    from_world_t: vec4<f32>,
    // bounce, friction, lifetime loss, unused
    response: vec4<f32>,
    // colliders, unused
    colliders: vec4<u32>,
}

struct Collider {
    // centre.xy, cos, sin
    place: vec4<f32>,
    // half extents.xy (box) or radius in x (circle); kind (0 box, 1 circle)
    shape: vec4<f32>,
}

@group(0) @binding(0)
var<storage, read_write> particles: array<Particle>;

@group(0) @binding(1)
var<uniform> sim: Sim;

@group(0) @binding(2)
var<storage, read> colliders: array<Collider>;

// How far outside a surface a hit particle is put, so the next step starts
// clear of it. Must match `particles::collide::SKIN`.
const SKIN: f32 = 1e-3;
// No hit: past the end of the path.
const MISS: f32 = 2.0;
// Started inside: a time before the path begins.
const INSIDE: f32 = -1.0;

// PCG hash: a well-mixed 32-bit value from a slot and the step's seed.
fn hash(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn unit(v: u32) -> f32 {
    return f32(hash(v) >> 8u) / 16777216.0;
}

fn point_to_world(p: vec2<f32>) -> vec2<f32> {
    return sim.to_world.xy * p.x + sim.to_world.zw * p.y + sim.to_world_t.xy;
}

fn point_from_world(p: vec2<f32>) -> vec2<f32> {
    return sim.from_world.xy * p.x + sim.from_world.zw * p.y + sim.from_world_t.xy;
}

fn vector_to_world(v: vec2<f32>) -> vec2<f32> {
    return sim.to_world.xy * v.x + sim.to_world.zw * v.y;
}

fn vector_from_world(v: vec2<f32>) -> vec2<f32> {
    return sim.from_world.xy * v.x + sim.from_world.zw * v.y;
}

// When along `was + path·t`, t in 0..1, the path enters the shape (`MISS`
// if it does not, `INSIDE` if it starts in it), and the outward normal there.
fn sweep(c: Collider, was: vec2<f32>, path: vec2<f32>) -> vec3<f32> {
    let centre = c.place.xy;
    if (c.shape.z > 0.5) {
        let radius = c.shape.x;
        let m = was - centre;
        let k = dot(m, m) - radius * radius;
        if (k < 0.0) {
            return vec3<f32>(INSIDE, 0.0, 0.0);
        }
        let a = dot(path, path);
        let b = dot(m, path);
        let disc = b * b - a * k;
        if (a <= 0.0 || b >= 0.0 || disc < 0.0) {
            return vec3<f32>(MISS, 0.0, 0.0);
        }
        let t = (-b - sqrt(disc)) / a;
        if (t > 1.0) {
            return vec3<f32>(MISS, 0.0, 0.0);
        }
        return vec3<f32>(t, normalize(m + path * t));
    }
    // A box: into its own frame, where it is axis-aligned.
    let cs = c.place.z;
    let sn = c.place.w;
    let d = was - centre;
    let o = vec2<f32>(d.x * cs + d.y * sn, -d.x * sn + d.y * cs);
    let v = vec2<f32>(path.x * cs + path.y * sn, -path.x * sn + path.y * cs);
    let h = c.shape.xy;
    if (abs(o.x) < h.x && abs(o.y) < h.y) {
        return vec3<f32>(INSIDE, 0.0, 0.0);
    }
    var enter = -1e30;
    var leave = 1e30;
    var normal = vec2<f32>(0.0, 0.0);
    for (var axis = 0u; axis < 2u; axis = axis + 1u) {
        let start = o[axis];
        let delta = v[axis];
        let half = h[axis];
        if (abs(delta) < 1e-12) {
            if (abs(start) >= half) {
                return vec3<f32>(MISS, 0.0, 0.0);
            }
            continue;
        }
        var near = (-half - start) / delta;
        var far = (half - start) / delta;
        var side = -1.0;
        if (near > far) {
            let swap = near;
            near = far;
            far = swap;
            side = 1.0;
        }
        if (near > enter) {
            enter = near;
            normal = vec2<f32>(0.0, 0.0);
            normal[axis] = side;
        }
        leave = min(leave, far);
        if (enter > leave) {
            return vec3<f32>(MISS, 0.0, 0.0);
        }
    }
    // Outside, so a path that reaches the box enters it at or after its
    // start; before it is the box behind the particle.
    if (enter < 0.0 || enter > 1.0) {
        return vec3<f32>(MISS, 0.0, 0.0);
    }
    let world_normal = vec2<f32>(normal.x * cs - normal.y * sn, normal.x * sn + normal.y * cs);
    return vec3<f32>(enter, world_normal);
}

fn collide(before: vec2<f32>, p: ptr<function, Particle>) {
    let count = sim.colliders.x;
    if (count == 0u) {
        return;
    }
    let was = point_to_world(before);
    let path = point_to_world((*p).position) - was;
    var best = vec3<f32>(MISS, 0.0, 0.0);
    for (var i = 0u; i < count; i = i + 1u) {
        let hit = sweep(colliders[i], was, path);
        if (hit.x == INSIDE) {
            (*p).lifetime = 0.0;
            return;
        }
        if (hit.x < best.x) {
            best = hit;
        }
    }
    if (best.x > 1.0) {
        return;
    }
    let normal = best.yz;
    let velocity = vector_to_world((*p).velocity);
    let into = dot(velocity, normal);
    var turned = velocity;
    if (into < 0.0) {
        let along = velocity - normal * into;
        turned = along * (1.0 - sim.response.y) - normal * into * sim.response.x;
    }
    (*p).position = point_from_world(was + path * best.x + normal * SKIN);
    (*p).velocity = vector_from_world(turned);
    (*p).lifetime = (*p).lifetime * (1.0 - sim.response.z);
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
        let before = p.position;
        p.age = p.age + dt;
        p.velocity = p.velocity + sim.accel_dir.xy * dt;
        p.position = p.position + p.velocity * dt;
        collide(before, &p);
    }
    particles[i] = p;
}
