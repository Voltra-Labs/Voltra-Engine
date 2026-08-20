//! Which pairs are worth an exact test.
//!
//! O(n²) with a world-space AABB rejection, deliberately. For the tens of
//! bodies a scene holds today this beats a spatial hash, which pays for
//! bucketing before it saves anything.
//!
//! **Replace it past roughly 200 bodies** — n² is 20 000 pair tests per step at
//! 60 Hz there, which is the point it stops being free. Sweep-and-prune on the
//! x axis is the replacement, because a 2D world is wide. The signature below
//! does not change when that happens; that is what it is for.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, CollisionLayers, Transform};

/// A collider's world bounds and what it is allowed to touch.
type Candidate = (Entity, (Vec2, Vec2), Option<CollisionLayers>);

/// Pairs whose bounds overlap and whose layers let them interact, each once,
/// with the lower entity index first.
///
/// The layer test comes first because it is the cheaper of the two and because
/// a pair that can never interact is not a candidate — filtering it later
/// would mean paying for its bounds test on every step for the whole life of
/// the scene. Sensors stay candidates: they are detected here and only stop
/// being ordinary at the solver.
pub fn candidate_pairs(world: &World) -> Vec<(Entity, Entity)> {
    let bounds: Vec<Candidate> = world
        .query::<Collider>()
        .filter_map(|(entity, collider)| {
            let transform = world.get::<Transform>(entity)?;
            let layers = world.get::<CollisionLayers>(entity).copied();
            Some((entity, collider.world_aabb(transform), layers))
        })
        .collect();

    let mut pairs = Vec::new();
    for (i, (a, a_bounds, a_layers)) in bounds.iter().enumerate() {
        // From `i + 1`, which is what gives each pair exactly once, never
        // mirrored, and never a body against itself. Emitting both `(a,b)` and
        // `(b,a)` would double every impulse the solver applies in the next
        // stage.
        for (b, b_bounds, b_layers) in &bounds[i + 1..] {
            if CollisionLayers::interact(a_layers.as_ref(), b_layers.as_ref())
                && overlaps(*a_bounds, *b_bounds)
            {
                pairs.push((*a, *b));
            }
        }
    }
    pairs
}

/// Whether two `(min, max)` boxes share any area.
fn overlaps((a_min, a_max): (Vec2, Vec2), (b_min, b_max): (Vec2, Vec2)) -> bool {
    a_min.x <= b_max.x && a_max.x >= b_min.x && a_min.y <= b_max.y && a_max.y >= b_min.y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn(world: &mut World, at: Vec2, half: f32) -> Entity {
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
}
