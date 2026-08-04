//! Sprites and the vertex data a world full of them produces.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::Vertex;

use crate::transform::Transform;

/// A coloured quad. Its size comes from the entity's [`Transform`] scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    /// Multiplied with the bound texture. White leaves the texture as-is.
    pub color: [f32; 4],
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

impl Sprite {
    pub fn new(color: [f32; 4]) -> Self {
        Self { color }
    }
}

/// The unit quad every sprite is built from, as `(corner, uv)` pairs.
///
/// Centred on the origin so scale and rotation act about the sprite's middle
/// rather than dragging it away from its own position. V runs opposite to Y
/// because image rows go down while clip space goes up.
const CORNERS: [(Vec2, [f32; 2]); 4] = [
    (Vec2::new(-0.5, 0.5), [0.0, 0.0]),
    (Vec2::new(-0.5, -0.5), [0.0, 1.0]),
    (Vec2::new(0.5, -0.5), [1.0, 1.0]),
    (Vec2::new(0.5, 0.5), [1.0, 0.0]),
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
    pub fn from_world(world: &World) -> Self {
        let mut batch = Self::default();

        for (_entity, transform, sprite) in world.query2::<Transform, Sprite>() {
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
    use voltra_render::glam::Vec2;

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
}
