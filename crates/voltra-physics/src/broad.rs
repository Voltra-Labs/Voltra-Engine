//! Which pairs are worth an exact test.
//!
//! Sweep-and-prune on x: every collider's world bounds sorted by their left
//! edge, then each one tested only against those that start before it ends.
//! A 2D level is wide, so x is the axis that separates most; a stack of
//! crates shares an x range and pays roughly what the old O(n²) loop did,
//! but a level of thousands of tiles stops paying for every tile against
//! every other.
//!
//! Pairs that could never produce anything are dropped before the exact
//! test: two colliders neither of which is a dynamic body — tile against
//! tile, wall against platform — unless one is a sensor, which still reports
//! what overlaps it. Box2D never makes those contacts either.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{BodyType, Collider, CollisionLayers, RigidBody, Sensor, Transform};

/// A collider's world bounds, what it may touch, and whether a pair with it
/// can matter on its own.
struct Candidate {
    entity: Entity,
    min: Vec2,
    max: Vec2,
    layers: Option<CollisionLayers>,
    /// A dynamic body: the only thing a contact can move.
    dynamic: bool,
    sensor: bool,
}

/// What a sweep found, and how much work it took — the count is what the
/// tests pin, since a timing is not deterministic.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sweep {
    /// The overlapping pairs found.
    pub pairs: Vec<(Entity, Entity)>,
    /// Bounds tests performed: pairs whose x ranges overlapped.
    pub tests: usize,
}

/// Pairs whose bounds overlap and whose layers let them interact, each once,
/// lower entity index first, sorted — so the solver sees the same order
/// whatever order the colliders were stored in.
///
/// The layer test runs before the y test because it is the cheaper of the
/// two. Sensors stay candidates: they are detected here and only stop being
/// ordinary at the solver.
pub fn candidate_pairs(world: &World) -> Vec<(Entity, Entity)> {
    sweep(world).pairs
}

/// [`candidate_pairs`], with the number of bounds tests it took.
pub fn sweep(world: &World) -> Sweep {
    let mut candidates: Vec<Candidate> = world
        .query::<Collider>()
        .filter_map(|(entity, collider)| {
            let transform = world.get::<Transform>(entity)?;
            let (min, max) = collider.world_aabb(transform);
            // A NaN edge would sort nowhere and overlap nothing; dropping it
            // keeps one bad transform from disturbing the sweep for the rest.
            if !(min.is_finite() && max.is_finite()) {
                return None;
            }
            Some(Candidate {
                entity,
                min,
                max,
                layers: world.get::<CollisionLayers>(entity).copied(),
                dynamic: world
                    .get::<RigidBody>(entity)
                    .is_some_and(|body| body.body_type == BodyType::Dynamic),
                sensor: world.get::<Sensor>(entity).is_some(),
            })
        })
        .collect();
    candidates.sort_unstable_by(|a, b| {
        a.min
            .x
            .total_cmp(&b.min.x)
            .then(a.entity.index().cmp(&b.entity.index()))
    });

    let mut found = Sweep::default();
    for (i, a) in candidates.iter().enumerate() {
        // Everything after `a` starts at or right of `a`'s left edge; the
        // first that starts past its right edge ends the run, and so do all
        // after it. Each pair is met once, from its leftmost member.
        for b in candidates[i + 1..]
            .iter()
            .take_while(|b| b.min.x <= a.max.x)
        {
            found.tests += 1;
            if !(a.dynamic || b.dynamic || a.sensor || b.sensor) {
                continue;
            }
            if CollisionLayers::interact(a.layers.as_ref(), b.layers.as_ref())
                && a.min.y <= b.max.y
                && a.max.y >= b.min.y
            {
                let pair = if a.entity.index() <= b.entity.index() {
                    (a.entity, b.entity)
                } else {
                    (b.entity, a.entity)
                };
                found.pairs.push(pair);
            }
        }
    }
    found
        .pairs
        .sort_unstable_by_key(|(a, b)| (a.index(), b.index()));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dynamic box: a pair needs a dynamic side to be worth testing.
    fn spawn(world: &mut World, at: Vec2, half: f32) -> Entity {
        let entity = spawn_static(world, at, half);
        world.insert(entity, RigidBody::new_dynamic(1.0));
        entity
    }

    fn spawn_static(world: &mut World, at: Vec2, half: f32) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(
            entity,
            Collider::Box {
                half_extents: Vec2::splat(half),
            },
        );
        entity
    }

    #[test]
    fn distant_bodies_are_not_a_pair() {
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);
        spawn(&mut world, Vec2::new(100.0, 0.0), 1.0);

        assert!(candidate_pairs(&world).is_empty());
    }

    #[test]
    fn overlapping_bodies_are_a_pair() {
        let mut world = World::new();
        let a = spawn(&mut world, Vec2::ZERO, 1.0);
        let b = spawn(&mut world, Vec2::new(0.5, 0.0), 1.0);

        assert_eq!(candidate_pairs(&world), vec![(a, b)]);
    }

    #[test]
    fn a_pair_appears_once_and_never_mirrored() {
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);
        spawn(&mut world, Vec2::new(0.5, 0.0), 1.0);

        assert_eq!(candidate_pairs(&world).len(), 1);
    }

    #[test]
    fn a_pair_the_layers_separate_is_not_a_candidate() {
        let mut world = World::new();
        let a = spawn(&mut world, Vec2::ZERO, 1.0);
        let b = spawn(&mut world, Vec2::new(0.5, 0.0), 1.0);
        world.insert(a, CollisionLayers::on(0).looking_at(0));
        world.insert(b, CollisionLayers::on(1).looking_at(1));

        assert!(
            candidate_pairs(&world).is_empty(),
            "they overlap, but neither looks at the other"
        );
    }

    #[test]
    fn one_side_looking_away_separates_the_pair() {
        // The symmetric rule, at the only place it can still be got wrong.
        let mut world = World::new();
        let a = spawn(&mut world, Vec2::ZERO, 1.0);
        let b = spawn(&mut world, Vec2::new(0.5, 0.0), 1.0);
        // `a` looks at every layer, so it sees `b`. `b` looks only at layer
        // one, which `a` is not on. Asymmetric rules would pair them.
        world.insert(a, CollisionLayers::on(0));
        world.insert(b, CollisionLayers::on(1).looking_at(1));

        assert!(candidate_pairs(&world).is_empty());
    }

    #[test]
    fn layers_that_look_at_each_other_still_pair() {
        let mut world = World::new();
        let a = spawn(&mut world, Vec2::ZERO, 1.0);
        let b = spawn(&mut world, Vec2::new(0.5, 0.0), 1.0);
        world.insert(a, CollisionLayers::on(0).looking_at(1));
        world.insert(b, CollisionLayers::on(1).looking_at(0));

        assert_eq!(candidate_pairs(&world), vec![(a, b)]);
    }

    #[test]
    fn a_collider_with_no_layers_still_pairs_with_a_filtered_one() {
        let mut world = World::new();
        let a = spawn(&mut world, Vec2::ZERO, 1.0);
        let b = spawn(&mut world, Vec2::new(0.5, 0.0), 1.0);
        world.insert(b, CollisionLayers::on(2));

        assert_eq!(candidate_pairs(&world), vec![(a, b)]);
    }

    #[test]
    fn a_body_is_never_paired_with_itself() {
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);

        assert!(candidate_pairs(&world).is_empty());
    }

    #[test]
    fn an_entity_without_a_collider_is_never_in_a_pair() {
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);
        let bare = world.spawn();
        world.insert(bare, Transform::default());

        assert!(candidate_pairs(&world).is_empty());
    }

    #[test]
    fn a_collider_without_a_transform_is_never_in_a_pair() {
        // Where a shape sits comes from its transform. Without one there is no
        // position to test, and assuming the origin would collide everything
        // with everything at 0,0.
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);
        let floating = world.spawn();
        world.insert(floating, Collider::Circle { radius: 5.0 });

        assert!(candidate_pairs(&world).is_empty());
    }

    #[test]
    fn three_overlapping_bodies_give_three_pairs() {
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 2.0);
        spawn(&mut world, Vec2::new(0.5, 0.0), 2.0);
        spawn(&mut world, Vec2::new(1.0, 0.0), 2.0);

        assert_eq!(candidate_pairs(&world).len(), 3);
    }

    #[test]
    fn bodies_that_only_touch_are_still_candidates() {
        // The broad phase is a filter, not an answer: it must not reject
        // anything the exact test might accept, and touching is the boundary.
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);
        spawn(&mut world, Vec2::new(2.0, 0.0), 1.0);

        assert_eq!(candidate_pairs(&world).len(), 1);
    }

    #[test]
    fn two_static_colliders_are_never_a_pair() {
        let mut world = World::new();
        spawn_static(&mut world, Vec2::ZERO, 1.0);
        spawn_static(&mut world, Vec2::new(0.5, 0.0), 1.0);
        assert!(candidate_pairs(&world).is_empty());
    }

    #[test]
    fn a_static_sensor_still_pairs_with_static_geometry() {
        let mut world = World::new();
        let wall = spawn_static(&mut world, Vec2::ZERO, 1.0);
        let zone = spawn_static(&mut world, Vec2::new(0.5, 0.0), 1.0);
        world.insert(zone, Sensor);
        assert_eq!(candidate_pairs(&world), vec![(wall, zone)]);
    }

    #[test]
    fn a_kinematic_body_and_a_wall_are_not_a_pair() {
        let mut world = World::new();
        spawn_static(&mut world, Vec2::ZERO, 1.0);
        let platform = spawn_static(&mut world, Vec2::new(0.5, 0.0), 1.0);
        world.insert(
            platform,
            RigidBody {
                body_type: BodyType::Kinematic,
                ..RigidBody::new_static()
            },
        );
        assert!(candidate_pairs(&world).is_empty());
    }

    #[test]
    fn pairs_come_out_in_index_order_whatever_the_positions() {
        let mut world = World::new();
        let a = spawn(&mut world, Vec2::new(3.0, 0.0), 1.0);
        let b = spawn(&mut world, Vec2::new(2.0, 0.0), 1.0);
        let c = spawn(&mut world, Vec2::new(1.0, 0.0), 1.0);
        assert_eq!(candidate_pairs(&world), vec![(a, b), (a, c), (b, c)]);
    }

    #[test]
    fn a_wide_level_costs_tests_per_neighbour_not_per_pair() {
        // A tilemap's worth of floor and a few bodies on it. The O(n²) loop
        // made two million tests of this; the sweep makes a few per tile.
        let mut world = World::new();
        for i in 0..2000 {
            spawn_static(&mut world, Vec2::new(i as f32, 0.0), 0.5);
        }
        for i in 0..20 {
            spawn(&mut world, Vec2::new(i as f32 * 50.0 + 0.5, 1.0), 0.5);
        }
        let found = sweep(&world);
        assert!(found.tests < 10_000, "{} tests", found.tests);
        assert_eq!(
            found.pairs.len(),
            20 * 2,
            "each body touches the two tiles under it"
        );
    }

    #[test]
    fn a_collider_at_a_nan_position_is_left_out() {
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);
        spawn(&mut world, Vec2::new(f32::NAN, 0.0), 1.0);
        assert!(candidate_pairs(&world).is_empty());
    }
}
