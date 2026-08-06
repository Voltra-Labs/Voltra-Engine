//! Turning a world full of sprites into vertex and index data.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::Vertex;

use crate::sprite::{draw_key, Sprite};
use crate::transform::Transform;

/// The unit quad every sprite is built from, as `(corner, uv)` pairs.
///
/// Centred on the origin so scale and rotation act about the sprite's middle
/// rather than dragging it away from its own position. V runs opposite to Y
/// because image rows go down while clip space goes up.
const CORNERS: [(Vec2, [f32; 2]); 4] = [
    (
        Vec2::new(-Sprite::HALF_EXTENT, Sprite::HALF_EXTENT),
        [0.0, 0.0],
    ),
    (
        Vec2::new(-Sprite::HALF_EXTENT, -Sprite::HALF_EXTENT),
        [0.0, 1.0],
    ),
    (
        Vec2::new(Sprite::HALF_EXTENT, -Sprite::HALF_EXTENT),
        [1.0, 1.0],
    ),
    (
        Vec2::new(Sprite::HALF_EXTENT, Sprite::HALF_EXTENT),
        [1.0, 0.0],
    ),
];

/// Two triangles over those four corners.
const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

/// Vertex and index data for every sprite in a world.
///
/// Built on the CPU each frame. Kept as plain `Vec`s rather than GPU buffers
/// so it can be produced and asserted on without a device.
#[derive(Debug, Default, Clone)]
pub struct SpriteBatch {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

impl SpriteBatch {
    /// Walks every entity holding both a [`Transform`] and a [`Sprite`].
    ///
    /// Collected and sorted rather than pushed as we go. With no depth buffer,
    /// the order geometry is written in is the order it is drawn in — and
    /// `query2` yields sparse-set storage order, which shifts when an unrelated
    /// entity is despawned. `Entity::index` breaks ties so sprites sharing a
    /// `sort_order` keep a stable order too; without it the common case, where
    /// everything sits on 0, would be exactly as unstable as before.
    ///
    /// Sorted with the unstable variant: `draw_key` is unique per entity, so
    /// there are never any equal keys for stability to preserve, and a
    /// dropped tiebreaker fails louder without it.
    pub fn from_world(world: &World) -> Self {
        let mut sprites: Vec<_> = world.query2::<Transform, Sprite>().collect();
        sprites.sort_unstable_by_key(|(entity, _, sprite)| draw_key(*entity, sprite));

        let mut batch = Self::default();
        for (_entity, transform, sprite) in sprites {
            batch.push(transform, sprite);
        }
        batch
    }

    /// Appends one quad.
    pub fn push(&mut self, transform: &Transform, sprite: &Sprite) {
        let matrix = transform.matrix();
        // Every quad's indices are relative to its own first vertex; without
        // this offset each sprite would redraw the first one.
        let base = self.vertices.len() as u16;

        for (corner, uv) in CORNERS {
            let world = matrix.transform_point2(corner);
            self.vertices.push(Vertex::new(
                [world.x, world.y],
                [sprite.color[0], sprite.color[1], sprite.color[2]],
                uv,
            ));
        }

        self.indices
            .extend(QUAD_INDICES.iter().map(|offset| base + offset));
    }

    pub fn sprite_count(&self) -> usize {
        self.vertices.len() / CORNERS.len()
    }

    /// Uploads the batch, or `None` when there is nothing to draw.
    ///
    /// Allocates fresh buffers every call. A persistent growable buffer
    /// written with `Queue::write_buffer` is the obvious next step, and worth
    /// doing once a frame's sprite count justifies it.
    pub fn upload(&self, device: &voltra_render::wgpu::Device) -> Option<voltra_render::Mesh> {
        if self.is_empty() {
            return None;
        }
        Some(voltra_render::Mesh::indexed(
            device,
            "sprite-batch",
            &self.vertices,
            &self.indices,
        ))
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_with(sprites: &[(Transform, Sprite)]) -> World {
        let mut world = World::new();
        for (transform, sprite) in sprites {
            let e = world.spawn();
            world.insert(e, *transform);
            world.insert(e, *sprite);
        }
        world
    }

    #[test]
    fn an_empty_world_produces_no_geometry() {
        let batch = SpriteBatch::from_world(&World::new());
        assert!(batch.vertices.is_empty());
        assert!(batch.indices.is_empty());
        assert_eq!(batch.sprite_count(), 0);
    }

    #[test]
    fn an_entity_without_a_sprite_is_skipped() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Transform::default());

        assert_eq!(SpriteBatch::from_world(&world).sprite_count(), 0);
    }

    #[test]
    fn one_sprite_becomes_a_quad() {
        let batch =
            SpriteBatch::from_world(&world_with(&[(Transform::default(), Sprite::default())]));

        assert_eq!(batch.vertices.len(), 4);
        assert_eq!(batch.indices.len(), 6);
        assert_eq!(batch.sprite_count(), 1);
    }

    #[test]
    fn an_untransformed_sprite_spans_one_unit_around_the_origin() {
        let batch =
            SpriteBatch::from_world(&world_with(&[(Transform::default(), Sprite::default())]));

        let xs: Vec<f32> = batch.vertices.iter().map(|v| v.position[0]).collect();
        let ys: Vec<f32> = batch.vertices.iter().map(|v| v.position[1]).collect();

        // Centred, not corner-anchored: rotation and scale should act about
        // the sprite's middle.
        assert!(xs.contains(&-0.5) && xs.contains(&0.5));
        assert!(ys.contains(&-0.5) && ys.contains(&0.5));
    }

    #[test]
    fn the_transform_moves_the_quad() {
        let batch = SpriteBatch::from_world(&world_with(&[(
            Transform::from_translation(Vec2::new(10.0, 0.0)),
            Sprite::default(),
        )]));

        for v in &batch.vertices {
            assert!(
                v.position[0] >= 9.0,
                "quad should sit around x=10, got {v:?}"
            );
        }
    }

    #[test]
    fn scale_grows_the_quad() {
        let batch = SpriteBatch::from_world(&world_with(&[(
            Transform::default().with_scale(Vec2::splat(4.0)),
            Sprite::default(),
        )]));

        let xs: Vec<f32> = batch.vertices.iter().map(|v| v.position[0]).collect();
        assert!(xs.contains(&-2.0) && xs.contains(&2.0));
    }

    #[test]
    fn the_sprite_colour_reaches_every_vertex() {
        let sprite = Sprite::new([0.25, 0.5, 0.75, 1.0]);
        let batch = SpriteBatch::from_world(&world_with(&[(Transform::default(), sprite)]));

        for v in &batch.vertices {
            assert_eq!(v.color, [0.25, 0.5, 0.75]);
        }
    }

    #[test]
    fn uvs_cover_the_whole_texture_with_v_pointing_down() {
        let batch =
            SpriteBatch::from_world(&world_with(&[(Transform::default(), Sprite::default())]));

        let top = batch
            .vertices
            .iter()
            .find(|v| v.position[1] > 0.0)
            .expect("a quad has a top edge");
        assert_eq!(top.uv[1], 0.0, "the top of the quad samples the top row");
    }

    #[test]
    fn a_second_sprite_gets_its_own_index_range() {
        let batch = SpriteBatch::from_world(&world_with(&[
            (Transform::default(), Sprite::default()),
            (
                Transform::from_translation(Vec2::new(5.0, 0.0)),
                Sprite::default(),
            ),
        ]));

        assert_eq!(batch.vertices.len(), 8);
        assert_eq!(batch.indices.len(), 12);

        // Forgetting to offset the second quad's indices makes every sprite
        // after the first draw on top of the first one.
        let second = &batch.indices[6..];
        assert!(
            second.iter().all(|&i| i >= 4),
            "second quad must index its own vertices, got {second:?}"
        );
        assert_eq!(batch.sprite_count(), 2);
    }

    #[test]
    fn indices_never_point_past_the_vertices() {
        let batch = SpriteBatch::from_world(&world_with(&[
            (Transform::default(), Sprite::default()),
            (Transform::default(), Sprite::default()),
            (Transform::default(), Sprite::default()),
        ]));

        let count = batch.vertices.len() as u16;
        assert!(batch.indices.iter().all(|&i| i < count));
    }

    /// Spawns each sprite and hands back the handles, so a test can despawn a
    /// specific one.
    fn world_returning_entities(
        sprites: &[(Transform, Sprite)],
    ) -> (World, Vec<voltra_ecs::Entity>) {
        let mut world = World::new();
        let mut entities = Vec::new();
        for (transform, sprite) in sprites {
            let e = world.spawn();
            world.insert(e, *transform);
            world.insert(e, *sprite);
            entities.push(e);
        }
        (world, entities)
    }

    /// The x coordinate of each sprite's centre, in the order the GPU will
    /// receive them.
    ///
    /// Asserting on the vertex buffer rather than on an internal list checks
    /// the thing that actually reaches the driver. The mean of the four
    /// corners rather than the first one: a corner sits half a unit off the
    /// sprite's position, so reading one directly would make these tests
    /// depend on which corner `CORNERS` happens to list first, which is
    /// incidental to draw order.
    fn draw_order(batch: &SpriteBatch) -> Vec<f32> {
        batch
            .vertices
            .chunks(4)
            .map(|quad| quad.iter().map(|v| v.position[0]).sum::<f32>() / 4.0)
            .collect()
    }

    #[test]
    fn a_higher_sort_order_is_drawn_later() {
        // Spawned back-to-front on purpose: if the sort is missing, the
        // insertion order survives and this fails.
        let (world, _) = world_returning_entities(&[
            (
                Transform::from_translation(Vec2::new(10.0, 0.0)),
                Sprite::default().with_sort_order(5),
            ),
            (
                Transform::from_translation(Vec2::new(20.0, 0.0)),
                Sprite::default().with_sort_order(-3),
            ),
        ]);

        let batch = SpriteBatch::from_world(&world);
        let xs = draw_order(&batch);

        // The -3 sprite sits at x = 20 and must come first; painter's order
        // means later is on top.
        assert!(xs[0] > 19.0, "expected the -3 sprite first, got {xs:?}");
        assert!(xs[1] < 11.0, "expected the 5 sprite last, got {xs:?}");
    }

    #[test]
    fn sprites_are_drawn_in_spawn_order_by_default() {
        let (world, _) = world_returning_entities(&[
            (
                Transform::from_translation(Vec2::new(1.0, 0.0)),
                Sprite::default(),
            ),
            (
                Transform::from_translation(Vec2::new(2.0, 0.0)),
                Sprite::default(),
            ),
            (
                Transform::from_translation(Vec2::new(3.0, 0.0)),
                Sprite::default(),
            ),
        ]);

        // A baseline, and deliberately named as one. With nothing despawned,
        // storage order already equals spawn order, so this would pass with no
        // sort at all — the test that actually exercises the ordering is below.
        let xs = draw_order(&SpriteBatch::from_world(&world));
        assert_eq!(xs, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn despawning_scrambles_storage_but_not_draw_order() {
        let (mut world, entities) = world_returning_entities(&[
            (
                Transform::from_translation(Vec2::new(1.0, 0.0)),
                Sprite::default(),
            ),
            (
                Transform::from_translation(Vec2::new(2.0, 0.0)),
                Sprite::default(),
            ),
            (
                Transform::from_translation(Vec2::new(3.0, 0.0)),
                Sprite::default(),
            ),
            (
                Transform::from_translation(Vec2::new(4.0, 0.0)),
                Sprite::default(),
            ),
        ]);

        // Four sprites, and the second one goes. `SparseSet::remove` is a
        // `swap_remove`, so the *last* element takes the freed slot: storage
        // becomes [1, 4, 3] and an unsorted batch would draw it that way.
        //
        // Four rather than three on purpose. With three, removing index 1 puts
        // the last element exactly where a stable removal would have left it,
        // so [1, 3] comes out identical with and without the sort and the test
        // proves nothing.
        //
        // Every sprite is on the default sort_order of 0, so `entity.index()`
        // is the only thing producing this order. Drop that half of the key and
        // `sort_by_key`'s stability preserves storage order and this fails.
        world.despawn(entities[1]);

        let xs = draw_order(&SpriteBatch::from_world(&world));
        assert_eq!(xs, vec![1.0, 3.0, 4.0]);
    }

    #[test]
    fn a_sprite_defaults_to_sort_order_zero() {
        assert_eq!(Sprite::default().sort_order, 0);
        assert_eq!(Sprite::new([1.0, 0.0, 0.0, 1.0]).sort_order, 0);
    }
}
