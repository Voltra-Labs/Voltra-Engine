//! Continuous collision: a body that would pass through something in one
//! step stops where it first meets it.
//!
//! A step collides once, at the poses it starts from, and then moves every
//! body by its solved velocity. A body crossing more than its own size in
//! that move can start on one side of a wall and end on the other with no
//! contact ever found. Box2D v3's answer, and its split: a **fast** body —
//! one moving more than half its thinnest extent in a step — is swept
//! against **static** geometry; a [`RigidBody2D::bullet`] against moving
//! bodies too. Only the fast few pay: the test is a length against an extent.
//!
//! Two passes, as Box2D's speculative contacts and its time of impact:
//!
//! - **Before the solve** ([`speculative_contacts`]), each fast body's
//!   predicted move is swept; what it would meet is handed to the solver as
//!   a contact of positive separation — the gap — which the solver lets
//!   close exactly and no further. The stop is rigid, in the step it
//!   happens, with restitution and friction as for any contact. An overlap
//!   found a step late would only be pushed back softly, and a fast enough
//!   body sinks through a thin wall before that push wins.
//! - **After the solve** ([`sweep`]), each fast body's actual move is swept,
//!   and one that still meets something — knocked off its predicted path by
//!   another contact, or turning — is moved back to the time of impact, a
//!   hair short of it, keeping its velocity: the next step's speculative
//!   contact takes it from there.
//!
//! [`RigidBody2D::bullet`]: voltra_scene::RigidBody2D::bullet

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{
    BodyType, Collider2D, CollisionLayers, OneWay, RigidBody2D, Sensor, Transform2D,
};

use crate::geometry::{closest, Hull};
use crate::hash::FastSet;
use crate::narrow::{speculative, Contact};
use crate::one_way::OneWayPasses;
use crate::sleep::Sleep;
use crate::solver::SolverBodies;

mod toi;

use toi::{time_of_impact, Motion};

/// A body is fast once it moves more than this share of its thinnest
/// extent in a step: Box2D's safety factor.
const FAST: f32 = 0.5;

/// How much wider than the gap the speculative contact is measured, so the
/// faces either side of the closest point are found too.
const SLACK: f32 = 0.01;

/// Something a fast body may be stopped by.
struct Target {
    entity: Entity,
    collider: Collider2D,
    at: Transform2D,
    hull: Hull,
    min: Vec2,
    max: Vec2,
    /// How far it moves in the step, for a bullet's sweep against it.
    moves: Vec2,
    layers: Option<CollisionLayers>,
    one_way: Option<OneWay>,
}

/// A body moving fast enough to be swept, and its move.
struct Mover {
    entity: Entity,
    collider: Collider2D,
    start: Transform2D,
    translation: Vec2,
    rotation: f32,
    bullet: bool,
    layers: Option<CollisionLayers>,
}

/// The contacts the awake fast bodies are about to make, predicted from
/// their velocities and gravity over `dt`, for the solver to stop them at.
/// Pairs in `touching` already have a contact, and pairs in `apart` are
/// kept from colliding by a joint.
pub(crate) fn speculative_contacts(
    world: &World,
    sleep: &Sleep,
    gravity: Vec2,
    dt: f32,
    passes: &OneWayPasses,
    apart: &FastSet<(Entity, Entity)>,
    touching: &FastSet<(Entity, Entity)>,
) -> Vec<Contact> {
    let movers: Vec<Mover> = world
        .query::<RigidBody2D>()
        .filter(|(entity, _)| sleep.is_awake_dynamic(world, *entity))
        .filter_map(|(entity, body)| {
            let velocity = body.velocity + gravity * body.gravity_scale * dt;
            let spin = if body.lock_rotation {
                0.0
            } else {
                body.angular_velocity
            };
            mover(world, entity, velocity * dt, spin * dt)
        })
        .collect();
    if movers.is_empty() {
        return Vec::new();
    }
    let still = targets(world, |entity, body| {
        is_still(body).then(|| still_pose(world, entity))?
    });
    let moving = if movers.iter().any(|m| m.bullet) {
        targets(world, |entity, body| {
            let body = body.filter(|b| !is_still(Some(b)) && !b.bullet)?;
            let at = *world.get::<Transform2D>(entity)?;
            Some((at, body.velocity * dt))
        })
    } else {
        Vec::new()
    };

    let mut contacts = Vec::new();
    for mover in &movers {
        let candidates = still.iter().chain(moving.iter().filter(|_| mover.bullet));
        let Some((_, _, target)) = first_impact(mover, candidates, passes, apart) else {
            continue;
        };
        if touching.contains(&ordered(mover.entity, target.entity)) {
            continue;
        }
        let start = Hull::of(&mover.collider, &mover.start);
        let gap = closest(start.vertices(), target.hull.vertices()).distance
            - (start.radius + target.hull.radius);
        let Some(manifold) = speculative(
            (&mover.collider, &mover.start),
            (&target.collider, &target.at),
            gap.max(0.0) + SLACK,
        ) else {
            continue;
        };
        contacts.push(Contact::new(mover.entity, target.entity, manifold));
    }
    contacts
}

/// Sweeps every fast body's solved move, moving the ones that met something
/// back to where they met it. `apart` holds the pairs a joint keeps from
/// colliding.
pub(crate) fn sweep(
    world: &World,
    bodies: &mut SolverBodies,
    passes: &OneWayPasses,
    apart: &FastSet<(Entity, Entity)>,
) {
    let movers: Vec<(usize, Mover)> = (0..bodies.len())
        .filter_map(|index| {
            let body = bodies.get(index);
            if body.body_type != BodyType::Dynamic || body.inverse_mass <= 0.0 {
                return None;
            }
            let found = mover(world, body.entity, body.delta_position, body.delta_rotation)?;
            Some((index, found))
        })
        .collect();
    if movers.is_empty() {
        return;
    }
    let still = targets(world, |entity, body| {
        is_still(body).then(|| still_pose(world, entity))?
    });
    let (plain, bullets): (Vec<_>, Vec<_>) = movers.into_iter().partition(|(_, m)| !m.bullet);
    for (index, mover) in &plain {
        stop_at_first(bodies, *index, mover, &still, passes, apart);
    }
    if bullets.is_empty() {
        return;
    }
    // Bullets meet the others where they ended up, after the sweep above;
    // two bullets do not meet each other, as in Box2D.
    // Where everything ended the step.
    let end = |entity: Entity| {
        let mut at = *world.get::<Transform2D>(entity)?;
        if let Some(body) = bodies.index_of(entity).map(|i| bodies.get(i)) {
            at.translation += body.delta_position;
            at.rotation += body.delta_rotation;
        }
        Some(at)
    };
    let moving = targets(world, |entity, body| {
        body.filter(|b| !is_still(Some(b)) && !b.bullet)?;
        Some((end(entity)?, Vec2::ZERO))
    });
    let all: Vec<&Target> = still.iter().chain(&moving).collect();
    for (index, mover) in &bullets {
        stop_at_first(bodies, *index, mover, all.iter().copied(), passes, apart);
    }
}

/// `entity` as a mover over `translation` and `rotation`, if it has a solid
/// shape and that move is fast for it.
fn mover(world: &World, entity: Entity, translation: Vec2, rotation: f32) -> Option<Mover> {
    if translation == Vec2::ZERO && rotation == 0.0 {
        return None;
    }
    let collider = world.get::<Collider2D>(entity)?;
    let start = world.get::<Transform2D>(entity)?;
    if world.get::<Sensor>(entity).is_some() || collider.is_degenerate(start) {
        return None;
    }
    let (thinnest, widest) = collider_extents(collider, start);
    let moved = translation.length() + rotation.abs() * widest;
    if !(moved.is_finite() && moved > FAST * thinnest) {
        return None;
    }
    Some(Mover {
        entity,
        collider: *collider,
        start: *start,
        translation,
        rotation,
        bullet: world
            .get::<RigidBody2D>(entity)
            .is_some_and(|body| body.bullet),
        layers: world.get::<CollisionLayers>(entity).copied(),
    })
}

/// Static geometry: a collider with no body, or a static one.
fn is_still(body: Option<&RigidBody2D>) -> bool {
    body.is_none_or(|b| b.body_type == BodyType::Static)
}

fn still_pose(world: &World, entity: Entity) -> Option<(Transform2D, Vec2)> {
    Some((*world.get::<Transform2D>(entity)?, Vec2::ZERO))
}

/// [`extents`] about the shape's own origin, without building a hull for the
/// two shapes most bodies are: this runs for every awake body every step.
fn collider_extents(collider: &Collider2D, at: &Transform2D) -> (f32, f32) {
    match collider {
        Collider2D::Box { .. } => {
            let half = collider.world_half_extents(at);
            (half.min_element(), half.length())
        }
        Collider2D::Circle { .. } => {
            let radius = collider.world_radius(at);
            (radius, radius)
        }
        _ => extents(&Hull::of(collider, at), at.translation),
    }
}

/// How thin and how wide a placed shape is about `origin`: the nearest any
/// face comes to its middle, and the furthest any of it reaches from
/// `origin`. Box2D's `b2ComputeShapeExtent`.
fn extents(hull: &Hull, origin: Vec2) -> (f32, f32) {
    let vertices = hull.vertices();
    let widest = vertices
        .iter()
        .map(|v| v.distance(origin))
        .fold(0.0, f32::max)
        + hull.radius;
    let thinnest = if vertices.len() < 3 {
        hull.radius
    } else {
        let middle = vertices.iter().copied().sum::<Vec2>() / vertices.len() as f32;
        hull.normals()
            .iter()
            .zip(vertices)
            .map(|(n, v)| n.dot(*v - middle))
            .fold(f32::INFINITY, f32::min)
            + hull.radius
    };
    (thinnest.max(0.0), widest)
}

/// The colliders a fast body may meet, each where `pose` places it and with
/// how far it moves; `pose` answers `None` for one to leave out. Sensors and
/// shapes with no area are left out too.
fn targets(
    world: &World,
    pose: impl Fn(Entity, Option<&RigidBody2D>) -> Option<(Transform2D, Vec2)>,
) -> Vec<Target> {
    let mut found = Vec::new();
    for (entity, collider) in world.query::<Collider2D>() {
        if world.get::<Sensor>(entity).is_some() {
            continue;
        }
        let Some((at, moves)) = pose(entity, world.get::<RigidBody2D>(entity)) else {
            continue;
        };
        if collider.is_degenerate(&at) {
            continue;
        }
        let (min, max) = collider.world_aabb(&at);
        if !(min.is_finite() && max.is_finite()) {
            continue;
        }
        found.push(Target {
            entity,
            collider: *collider,
            at,
            hull: Hull::of(collider, &at),
            min: min.min(min + moves),
            max: max.max(max + moves),
            moves,
            layers: world.get::<CollisionLayers>(entity).copied(),
            one_way: world.get::<OneWay>(entity).copied(),
        });
    }
    found
}

/// The first of `targets` `mover`'s sweep meets: the share of the move free
/// of it, the normal pushing the mover away, and which.
fn first_impact<'t>(
    mover: &Mover,
    targets: impl IntoIterator<Item = &'t Target>,
    passes: &OneWayPasses,
    apart: &FastSet<(Entity, Entity)>,
) -> Option<(f32, Vec2, &'t Target)> {
    let reach = extents(
        &Hull::of(&mover.collider, &mover.start),
        mover.start.translation,
    )
    .1;
    let (from, to) = (
        mover.start.translation,
        mover.start.translation + mover.translation,
    );
    // Wherever it turned on the way, the shape stayed within `reach` of its
    // path.
    let (min, max) = (
        from.min(to) - Vec2::splat(reach),
        from.max(to) + Vec2::splat(reach),
    );

    let mut first: Option<(f32, Vec2, &Target)> = None;
    for target in targets {
        if target.entity == mover.entity
            || target.min.x > max.x
            || target.min.y > max.y
            || target.max.x < min.x
            || target.max.y < min.y
            || !CollisionLayers::interact(mover.layers.as_ref(), target.layers.as_ref())
            || apart.contains(&ordered(mover.entity, target.entity))
        {
            continue;
        }
        // Against something moving, the move relative to it.
        let motion = Motion {
            collider: &mover.collider,
            start: mover.start,
            translation: mover.translation - target.moves,
            rotation: mover.rotation,
        };
        let Some((t, normal)) = time_of_impact(&motion, &target.hull) else {
            continue;
        };
        if first.is_some_and(|(best, _, _)| best <= t) {
            continue;
        }
        // A one-way collider stops only what meets its solid side, and
        // nothing it is already letting through.
        if let Some(one_way) = &target.one_way {
            if passes.is_passing(mover.entity, target.entity) || !one_way.admits(&target.at, normal)
            {
                continue;
            }
        }
        first = Some((t, normal, target));
    }
    first
}

/// Moves the body at `index` back to the first of `targets` its solved move
/// meets, if any, keeping its velocity.
fn stop_at_first<'t>(
    bodies: &mut SolverBodies,
    index: usize,
    mover: &Mover,
    targets: impl IntoIterator<Item = &'t Target>,
    passes: &OneWayPasses,
    apart: &FastSet<(Entity, Entity)>,
) {
    let Some((t, _, _)) = first_impact(mover, targets, passes, apart) else {
        return;
    };
    let body = bodies.get_mut(index);
    body.delta_position = mover.translation * t;
    body.delta_rotation = mover.rotation * t;
}

fn ordered(a: Entity, b: Entity) -> (Entity, Entity) {
    if a.index() <= b.index() {
        (a, b)
    } else {
        (b, a)
    }
}
