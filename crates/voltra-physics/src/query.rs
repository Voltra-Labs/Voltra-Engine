//! Asking the world a question without touching it.
//!
//! A step reports what collided; a query answers what *would*. Every engine
//! has the pair — Unity's `Physics2D.Raycast`, Godot's
//! `PhysicsDirectSpaceState2D.intersect_ray`, Box2D's `b2World::RayCast`,
//! Rapier's `QueryPipeline` — because gameplay asks constantly: what is under
//! the character's feet, what does this shot hit, what is inside the blast.
//!
//! These take `&World` and nothing else, so they work from either of a game's
//! ticks, from the editor, and from a test with no [`PhysicsWorld`] at all.
//!
//! [`PhysicsWorld`]: crate::PhysicsWorld

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, CollisionLayers, Sensor, Transform};

use crate::narrow::manifold;

/// Which colliders a query is allowed to find.
///
/// One mask, not the pair a collider carries: a query is not a collider, it
/// has no layers of its own to be looked at, and only the caller's side of the
/// test exists. Unity's `LayerMask` argument and Rapier's `QueryFilter` are
/// the same one-sided shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryFilter {
    /// Layers this query looks at. A collider is a candidate when it is on any
    /// of them.
    pub mask: u32,
    /// An entity the query ignores — the caster, almost always. A ground check
    /// that hits the character it started inside answers its own question.
    pub exclude: Option<Entity>,
    /// Whether sensors are found. Off by default: a shot should not stop at a
    /// trigger volume, and a ground check should not stand on one.
    pub sensors: bool,
}

impl Default for QueryFilter {
    fn default() -> Self {
        Self {
            mask: CollisionLayers::ALL,
            exclude: None,
            sensors: false,
        }
    }
}

impl QueryFilter {
    /// Looks at everything solid.
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks only at the layers in `mask`.
    pub const fn looking_at(mut self, mask: u32) -> Self {
        self.mask = mask;
        self
    }

    /// Ignores `entity`.
    pub const fn excluding(mut self, entity: Entity) -> Self {
        self.exclude = Some(entity);
        self
    }

    /// Finds sensors as well as solid colliders.
    pub const fn with_sensors(mut self) -> Self {
        self.sensors = true;
        self
    }

    /// Whether this query may find `entity`.
    fn allows(&self, world: &World, entity: Entity) -> bool {
        if self.exclude == Some(entity) {
            return false;
        }
        if !self.sensors && world.get::<Sensor>(entity).is_some() {
            return false;
        }
        let layers = world
            .get::<CollisionLayers>(entity)
            .copied()
            .unwrap_or_default();
        self.mask & layers.layers != 0
    }
}

/// Where a ray met a collider.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub entity: Entity,
    /// Where the ray met the surface, in world space.
    pub point: Vec2,
    /// The surface normal there, pointing back along the ray. The reverse of
    /// the ray's own direction when it started inside the shape.
    pub normal: Vec2,
    /// How far along the ray the hit is, in world units.
    pub distance: f32,
}

/// The nearest collider a ray of `max` length meets, if any.
///
/// `direction` need not be normalised. A direction of no length, or a
/// non-positive `max`, finds nothing rather than dividing by it.
///
/// A ray that starts *inside* a shape hits it at distance zero, with the ray
/// reversed as the normal. Reporting nothing there is the classic surprise —
/// a ground check begun a hair inside the floor would say the character is in
/// the air.
pub fn ray(
    world: &World,
    origin: Vec2,
    direction: Vec2,
    max: f32,
    filter: QueryFilter,
) -> Option<RayHit> {
    let direction = normalized(direction)?;
    // NaN included, which is why this is not `max <= 0.0` alone: a length
    // nothing compares to is not a length.
    if max.is_nan() || max <= 0.0 {
        return None;
    }

    let mut nearest: Option<RayHit> = None;
    for (entity, collider, transform) in colliders(world, filter) {
        let Some(hit) = cast(collider, transform, origin, direction, max) else {
            continue;
        };
        let hit = RayHit {
            entity,
            point: hit.0,
            normal: hit.1,
            distance: hit.2,
        };
        if nearest.is_none_or(|best| hit.distance < best.distance) {
            nearest = Some(hit);
        }
    }
    nearest
}

/// A collider covering `at`, if any.
///
/// Where several cover it, which one is reported is the order the world stores
/// them in. Use [`overlap_aabb`] when every answer matters — a point query is
/// for "is there anything here", which is how Godot's and Unity's point checks
/// are used too.
pub fn point(world: &World, at: Vec2, filter: QueryFilter) -> Option<Entity> {
    colliders(world, filter)
        .find(|(_, collider, transform)| contains(collider, transform, at))
        .map(|(entity, _, _)| entity)
}

/// Every collider overlapping the box `(min, max)`.
///
/// The *shapes*, not their bounds: the box is collided against each collider
/// exactly as a step would, so a rotated crate whose bounds cover the corner
/// of the query but whose body does not is not reported. Merely touching does
/// not count, for the same reason it does not in a step.
///
/// An inverted or empty box finds nothing.
pub fn overlap_aabb(world: &World, min: Vec2, max: Vec2, filter: QueryFilter) -> Vec<Entity> {
    let half = (max - min) * 0.5;
    if half.x <= 0.0 || half.y <= 0.0 {
        return Vec::new();
    }
    let query = Collider::Box { half_extents: half };
    let at = Transform::from_translation(min + half);

    colliders(world, filter)
        .filter(|(_, collider, transform)| manifold((&query, &at), (collider, transform)).is_some())
        .map(|(entity, _, _)| entity)
        .collect()
}

/// Every collider this query may find, with where it sits.
fn colliders(
    world: &World,
    filter: QueryFilter,
) -> impl Iterator<Item = (Entity, &Collider, &Transform)> {
    world.query::<Collider>().filter_map(move |(entity, c)| {
        let transform = world.get::<Transform>(entity)?;
        // A shape with no area is not a surface: it would report a hit at a
        // point nothing is drawn at, and `is_degenerate` is what the narrow
        // phase rejects it with too.
        if c.is_degenerate(transform) || !filter.allows(world, entity) {
            return None;
        }
        Some((entity, c, transform))
    })
}

/// Whether `at` is inside the shape.
fn contains(collider: &Collider, transform: &Transform, at: Vec2) -> bool {
    match collider {
        Collider::Box { .. } => {
            let local = to_local(transform, at);
            let half = collider.world_half_extents(transform);
            local.x.abs() <= half.x && local.y.abs() <= half.y
        }
        Collider::Circle { .. } => {
            let radius = collider.world_radius(transform);
            at.distance_squared(transform.translation) <= radius * radius
        }
    }
}

/// `(point, normal, distance)` where a unit ray meets the shape within `max`.
fn cast(
    collider: &Collider,
    transform: &Transform,
    origin: Vec2,
    direction: Vec2,
    max: f32,
) -> Option<(Vec2, Vec2, f32)> {
    let distance = match collider {
        Collider::Box { .. } => {
            let half = collider.world_half_extents(transform);
            slab(
                to_local(transform, origin),
                rotated(-transform.rotation, direction),
                half,
            )?
        }
        Collider::Circle { .. } => {
            let radius = collider.world_radius(transform);
            sphere(origin - transform.translation, direction, radius)?
        }
    };
    if distance > max {
        return None;
    }

    let point = origin + direction * distance;
    // Inside the shape: there is no surface between the origin and the hit, so
    // the only honest normal is the one facing whoever asked.
    let normal = if distance <= 0.0 {
        -direction
    } else {
        match collider {
            Collider::Box { .. } => face_normal(collider, transform, point),
            Collider::Circle { .. } => (point - transform.translation)
                .try_normalize()
                .unwrap_or(-direction),
        }
    };
    Some((point, normal, distance))
}

/// How far a unit ray travels to enter a box of `half` extents, in the box's
/// own frame. Zero when it starts inside.
fn slab(origin: Vec2, direction: Vec2, half: Vec2) -> Option<f32> {
    let mut enter = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;

    for axis in 0..2 {
        let (o, d, h) = (origin[axis], direction[axis], half[axis]);
        if d.abs() < f32::EPSILON {
            // Parallel to this pair of faces: either it is between them for
            // the whole ray, or it never is.
            if o.abs() > h {
                return None;
            }
            continue;
        }
        let (near, far) = ((-h - o) / d, (h - o) / d);
        enter = enter.max(near.min(far));
        exit = exit.min(near.max(far));
    }

    if enter > exit || exit < 0.0 {
        return None;
    }
    Some(enter.max(0.0))
}

/// How far a unit ray travels to reach a circle of `radius` centred on the
/// origin of `to_centre`. Zero when it starts inside.
fn sphere(to_centre: Vec2, direction: Vec2, radius: f32) -> Option<f32> {
    let along = to_centre.dot(direction);
    let outside = to_centre.length_squared() - radius * radius;
    if outside > 0.0 && along > 0.0 {
        // Outside and pointing away.
        return None;
    }
    let discriminant = along * along - outside;
    if discriminant < 0.0 {
        return None;
    }
    Some((-along - discriminant.sqrt()).max(0.0))
}

/// The outward normal of the box face `point` sits on.
fn face_normal(collider: &Collider, transform: &Transform, point: Vec2) -> Vec2 {
    let local = to_local(transform, point);
    let half = collider.world_half_extents(transform);
    // Whichever axis the point is nearest the edge of, in units of that axis'
    // extent: on a corner both are one, and either face is a correct answer.
    let local = if (local.x.abs() / half.x) >= (local.y.abs() / half.y) {
        Vec2::new(local.x.signum(), 0.0)
    } else {
        Vec2::new(0.0, local.y.signum())
    };
    rotated(transform.rotation, local)
}

/// `point` in the shape's own frame: rotation undone, centre at the origin.
fn to_local(transform: &Transform, point: Vec2) -> Vec2 {
    rotated(-transform.rotation, point - transform.translation)
}

fn rotated(radians: f32, vector: Vec2) -> Vec2 {
    Vec2::from_angle(radians).rotate(vector)
}

/// The unit vector along `direction`, or nothing if it has no length.
fn normalized(direction: Vec2) -> Option<Vec2> {
    direction.try_normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::Sensor;

    const NEAR: f32 = 1e-4;

    fn box_at(world: &mut World, at: Vec2, half: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, Collider::Box { half_extents: half });
        entity
    }

    fn circle_at(world: &mut World, at: Vec2, radius: f32) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, Collider::Circle { radius });
        entity
    }

    #[test]
    fn a_ray_reports_where_it_met_the_surface() {
        let mut world = World::new();
        let wall = box_at(&mut world, Vec2::new(5.0, 0.0), Vec2::new(1.0, 1.0));

        let hit = ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            10.0,
            QueryFilter::new(),
        )
        .expect("it is straight ahead");

        assert_eq!(hit.entity, wall);
        assert!((hit.distance - 4.0).abs() < NEAR, "{}", hit.distance);
        assert!(hit.point.distance(Vec2::new(4.0, 0.0)) < NEAR);
        assert!(
            hit.normal.distance(Vec2::new(-1.0, 0.0)) < NEAR,
            "{:?}",
            hit.normal
        );
    }

    #[test]
    fn a_ray_stops_at_the_nearer_of_two() {
        let mut world = World::new();
        let near = box_at(&mut world, Vec2::new(3.0, 0.0), Vec2::splat(0.5));
        box_at(&mut world, Vec2::new(6.0, 0.0), Vec2::splat(0.5));

        let hit = ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            10.0,
            QueryFilter::new(),
        )
        .expect("both are ahead");

        assert_eq!(hit.entity, near);
    }

    #[test]
    fn a_ray_that_falls_short_finds_nothing() {
        let mut world = World::new();
        box_at(&mut world, Vec2::new(5.0, 0.0), Vec2::splat(0.5));

        assert!(ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            2.0,
            QueryFilter::new()
        )
        .is_none());
    }

    #[test]
    fn a_ray_hits_the_turned_face_of_a_turned_box() {
        // A box turned 45° reaches half a diagonal closer than its own extent.
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(
            entity,
            Transform::from_translation(Vec2::new(4.0, 0.0))
                .with_rotation(std::f32::consts::FRAC_PI_4),
        );
        world.insert(
            entity,
            Collider::Box {
                half_extents: Vec2::splat(1.0),
            },
        );

        let hit = ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            10.0,
            QueryFilter::new(),
        )
        .expect("straight ahead");

        let expected = 4.0 - std::f32::consts::SQRT_2;
        assert!(
            (hit.distance - expected).abs() < NEAR,
            "{} vs {expected}",
            hit.distance
        );
        assert!(
            hit.normal.dot(Vec2::new(-1.0, 0.0)) > 0.0,
            "{:?}",
            hit.normal
        );
    }

    #[test]
    fn a_ray_that_starts_inside_hits_at_no_distance() {
        let mut world = World::new();
        let entity = box_at(&mut world, Vec2::ZERO, Vec2::splat(1.0));

        let hit = ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            10.0,
            QueryFilter::new(),
        )
        .expect("it is inside it");

        assert_eq!(hit.entity, entity);
        assert_eq!(hit.distance, 0.0);
        assert!(
            hit.normal.distance(Vec2::new(-1.0, 0.0)) < NEAR,
            "the way back"
        );
    }

    #[test]
    fn a_ray_with_no_direction_or_no_length_finds_nothing() {
        let mut world = World::new();
        box_at(&mut world, Vec2::ZERO, Vec2::splat(1.0));

        assert!(ray(&world, Vec2::ZERO, Vec2::ZERO, 10.0, QueryFilter::new()).is_none());
        assert!(ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            0.0,
            QueryFilter::new()
        )
        .is_none());
        assert!(ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            -1.0,
            QueryFilter::new()
        )
        .is_none());
    }

    #[test]
    fn a_ray_meets_a_circle_at_its_edge() {
        let mut world = World::new();
        circle_at(&mut world, Vec2::new(0.0, 5.0), 2.0);

        let hit = ray(
            &world,
            Vec2::ZERO,
            Vec2::new(0.0, 1.0),
            10.0,
            QueryFilter::new(),
        )
        .expect("above");

        assert!((hit.distance - 3.0).abs() < NEAR, "{}", hit.distance);
        assert!(hit.normal.distance(Vec2::new(0.0, -1.0)) < NEAR);
    }

    #[test]
    fn a_ray_pointing_away_from_a_circle_finds_nothing() {
        let mut world = World::new();
        circle_at(&mut world, Vec2::new(0.0, 5.0), 2.0);

        assert!(ray(
            &world,
            Vec2::ZERO,
            Vec2::new(0.0, -1.0),
            10.0,
            QueryFilter::new()
        )
        .is_none());
    }

    #[test]
    fn a_query_ignores_what_it_excludes() {
        let mut world = World::new();
        let caster = box_at(&mut world, Vec2::ZERO, Vec2::splat(1.0));
        let wall = box_at(&mut world, Vec2::new(5.0, 0.0), Vec2::splat(1.0));

        let hit = ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            10.0,
            QueryFilter::new().excluding(caster),
        )
        .expect("the wall is still there");

        assert_eq!(hit.entity, wall);
    }

    #[test]
    fn a_query_only_finds_the_layers_it_looks_at() {
        let mut world = World::new();
        let ground = box_at(&mut world, Vec2::new(0.0, -5.0), Vec2::new(10.0, 1.0));
        world.insert(ground, CollisionLayers::on(1));
        let water = box_at(&mut world, Vec2::new(0.0, -2.0), Vec2::new(10.0, 1.0));
        world.insert(water, CollisionLayers::on(2));

        let hit = ray(
            &world,
            Vec2::ZERO,
            Vec2::new(0.0, -1.0),
            10.0,
            QueryFilter::new().looking_at(CollisionLayers::bit(1)),
        )
        .expect("the ground is on layer one");

        assert_eq!(hit.entity, ground, "the water is nearer and not looked at");
    }

    #[test]
    fn a_query_walks_past_sensors_unless_asked() {
        let mut world = World::new();
        let trigger = box_at(&mut world, Vec2::new(2.0, 0.0), Vec2::splat(1.0));
        world.insert(trigger, Sensor);
        let wall = box_at(&mut world, Vec2::new(6.0, 0.0), Vec2::splat(1.0));

        let solid = ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            10.0,
            QueryFilter::new(),
        )
        .expect("the wall");
        assert_eq!(solid.entity, wall);

        let either = ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            10.0,
            QueryFilter::new().with_sensors(),
        )
        .expect("the trigger");
        assert_eq!(either.entity, trigger);
    }

    #[test]
    fn a_degenerate_shape_is_not_a_surface() {
        let mut world = World::new();
        circle_at(&mut world, Vec2::new(3.0, 0.0), 0.0);

        assert!(ray(
            &world,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            10.0,
            QueryFilter::new()
        )
        .is_none());
    }

    #[test]
    fn a_point_finds_what_covers_it() {
        let mut world = World::new();
        let entity = box_at(&mut world, Vec2::ZERO, Vec2::splat(1.0));

        assert_eq!(
            point(&world, Vec2::new(0.5, -0.5), QueryFilter::new()),
            Some(entity)
        );
        assert_eq!(point(&world, Vec2::new(2.0, 0.0), QueryFilter::new()), None);
    }

    #[test]
    fn a_point_respects_a_turned_box() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(
            entity,
            Transform::default().with_rotation(std::f32::consts::FRAC_PI_4),
        );
        world.insert(
            entity,
            Collider::Box {
                half_extents: Vec2::splat(1.0),
            },
        );

        // Inside the bounds of the turned box, outside the box itself.
        assert_eq!(point(&world, Vec2::new(0.9, 0.9), QueryFilter::new()), None);
        assert_eq!(
            point(&world, Vec2::new(0.9, 0.0), QueryFilter::new()),
            Some(entity)
        );
    }

    #[test]
    fn a_point_finds_a_circle_by_its_radius() {
        let mut world = World::new();
        let entity = circle_at(&mut world, Vec2::ZERO, 1.0);

        assert_eq!(
            point(&world, Vec2::new(0.9, 0.0), QueryFilter::new()),
            Some(entity)
        );
        assert_eq!(point(&world, Vec2::new(0.8, 0.8), QueryFilter::new()), None);
    }

    #[test]
    fn an_overlap_reports_every_shape_in_the_box() {
        let mut world = World::new();
        let a = box_at(&mut world, Vec2::ZERO, Vec2::splat(0.5));
        let b = circle_at(&mut world, Vec2::new(1.0, 0.0), 0.5);
        box_at(&mut world, Vec2::new(20.0, 0.0), Vec2::splat(0.5));

        let found = overlap_aabb(
            &world,
            Vec2::new(-2.0, -2.0),
            Vec2::new(2.0, 2.0),
            QueryFilter::new(),
        );

        assert_eq!(found, vec![a, b]);
    }

    #[test]
    fn an_overlap_tests_the_shape_and_not_its_bounds() {
        // The corner of a turned box: its bounds cover the query, its body
        // does not.
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(
            entity,
            Transform::default().with_rotation(std::f32::consts::FRAC_PI_4),
        );
        world.insert(
            entity,
            Collider::Box {
                half_extents: Vec2::splat(1.0),
            },
        );

        let found = overlap_aabb(
            &world,
            Vec2::new(1.2, 1.2),
            Vec2::new(1.35, 1.35),
            QueryFilter::new(),
        );

        assert!(found.is_empty(), "the corner is empty space");
    }

    #[test]
    fn an_empty_or_inverted_box_overlaps_nothing() {
        let mut world = World::new();
        box_at(&mut world, Vec2::ZERO, Vec2::splat(1.0));

        assert!(overlap_aabb(&world, Vec2::ZERO, Vec2::ZERO, QueryFilter::new()).is_empty());
        assert!(overlap_aabb(
            &world,
            Vec2::splat(1.0),
            Vec2::splat(-1.0),
            QueryFilter::new()
        )
        .is_empty());
    }
}
