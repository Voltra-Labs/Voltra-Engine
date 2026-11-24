// Built-in shader: one emitter's particles, aged, moved, collided and born
// on the GPU.
//
// Must match `particles::uniforms::SimUniform`, `GpuParticle`,
// `particles::colliders::ColliderGpu` and `particles::tables`' rows. Each invocation owns one slot of the
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
    rotation: f32,
    spin: f32,
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
    // colliders, shape kind (0 point, 1 ring, 2 box, 3 edge, 4 outline),
    // outline corners, unused
    counts: vec4<u32>,
    // ring inner, outer; box half x, y; edge half length
    shape: vec4<f32>,
    // rotation min, max, spin min, max
    spin: vec4<f32>,
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

// Rows: colour, (size, speed, alpha, -), flipbook frames, outline corners
// (x, y, fraction of the perimeter before it, -).
@group(0) @binding(3)
var tables: texture_2d<f32>;

// Must match `particles::tables::TABLE_SAMPLES`.
const SAMPLES: i32 = 64;
const ROW_CURVES: i32 = 1;
const ROW_OUTLINE: i32 = 3;
const TAU: f32 = 6.283185307179586;

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

// Row `row` at `t` of a life, blended between its two nearest samples.
fn table_at(row: i32, t: f32) -> vec4<f32> {
    let x = clamp(t, 0.0, 1.0) * f32(SAMPLES - 1);
    let i = min(i32(floor(x)), SAMPLES - 2);
    let a = textureLoad(tables, vec2<i32>(i, row), 0);
    let b = textureLoad(tables, vec2<i32>(i + 1, row), 0);
    return mix(a, b, x - f32(i));
}

// Where along the outline `u` (0..1) of the perimeter is: the CPU's
// `particles::shape::on_outline`, line for line.
fn on_outline(u: f32) -> vec2<f32> {
    let n = i32(sim.counts.z);
    for (var i = 0; i < n; i = i + 1) {
        let here = textureLoad(tables, vec2<i32>(i, ROW_OUTLINE), 0);
        let next = textureLoad(tables, vec2<i32>((i + 1) % n, ROW_OUTLINE), 0);
        var end = next.z;
        if (i + 1 == n) {
            end = 1.0;
        }
        if (u < end || i + 1 == n) {
            let span = end - here.z;
            var f = 0.0;
            if (span > 0.0) {
                f = clamp((u - here.z) / span, 0.0, 1.0);
            }
            return mix(here.xy, next.xy, f);
        }
    }
    return vec2<f32>(0.0, 0.0);
}

// A newborn's offset from the origin, from two units: the CPU's
// `particles::shape::offset`.
fn spawn_offset(a: f32, b: f32) -> vec2<f32> {
    let kind = sim.counts.y;
    if (kind == 1u) {
        let inner = sim.shape.x;
        let outer = sim.shape.y;
        let r = sqrt(mix(inner * inner, outer * outer, a));
        let angle = b * TAU;
        return vec2<f32>(cos(angle), sin(angle)) * r;
    }
    if (kind == 2u) {
        return vec2<f32>((a * 2.0 - 1.0) * sim.shape.x, (b * 2.0 - 1.0) * sim.shape.y);
    }
    if (kind == 3u) {
        return vec2<f32>((a * 2.0 - 1.0) * sim.shape.x, 0.0);
    }
    if (kind == 4u) {
        return on_outline(a);
    }
    return vec2<f32>(0.0, 0.0);
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
    // Each axis's entry and exit times, written out rather than looped: FXC
    // (D3D12) must unroll a loop that writes a vector by a runtime index,
    // and fails to. A still axis inside its slab never limits the path; one
    // outside it is a miss.
    var near = vec2<f32>(-1e30, -1e30);
    var far = vec2<f32>(1e30, 1e30);
    if (abs(v.x) >= 1e-12) {
        let a = (-h.x - o.x) / v.x;
        let b = (h.x - o.x) / v.x;
        near.x = min(a, b);
        far.x = max(a, b);
    } else if (abs(o.x) >= h.x) {
        return vec3<f32>(MISS, 0.0, 0.0);
    }
    if (abs(v.y) >= 1e-12) {
        let a = (-h.y - o.y) / v.y;
        let b = (h.y - o.y) / v.y;
        near.y = min(a, b);
        far.y = max(a, b);
    } else if (abs(o.y) >= h.y) {
        return vec3<f32>(MISS, 0.0, 0.0);
    }
    let enter = max(near.x, near.y);
    let leave = min(far.x, far.y);
    // Outside, so a path that reaches the box enters it at or after its
    // start; before it is the box behind the particle.
    if (enter > leave || enter < 0.0 || enter > 1.0) {
        return vec3<f32>(MISS, 0.0, 0.0);
    }
    // Through the face of the axis entered last; x on a tie (a corner).
    var normal = vec2<f32>(0.0, -sign(v.y));
    if (near.x >= near.y) {
        normal = vec2<f32>(-sign(v.x), 0.0);
    }
    let world_normal = vec2<f32>(normal.x * cs - normal.y * sn, normal.x * sn + normal.y * cs);
    return vec3<f32>(enter, world_normal);
}

fn collide(before: vec2<f32>, p: ptr<function, Particle>) {
    let count = sim.counts.x;
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
        let offset = spawn_offset(unit(seed + 3u), unit(seed + 4u));
        p.position = sim.origin_dt.xy + sim.basis.xy * offset.x + sim.basis.zw * offset.y;
        p.velocity = sim.basis.xy * local.x + sim.basis.zw * local.y;
        p.age = 0.0;
        p.lifetime = max(mix(sim.ranges.z, sim.ranges.w, unit(seed + 2u)), 0.0);
        p.rotation = mix(sim.spin.x, sim.spin.y, unit(seed + 5u));
        p.spin = mix(sim.spin.z, sim.spin.w, unit(seed + 6u));
    } else if (p.age < p.lifetime) {
        let before = p.position;
        p.age = p.age + dt;
        let speed = table_at(ROW_CURVES, p.age / p.lifetime).y;
        p.velocity = p.velocity + sim.accel_dir.xy * dt;
        p.position = p.position + p.velocity * (speed * dt);
        p.rotation = p.rotation + p.spin * dt;
        collide(before, &p);
    }
    particles[i] = p;
}
