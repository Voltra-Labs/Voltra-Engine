use super::*;
use voltra_render::glam::Vec2;

/// The geometry a world produces with no sheets loaded, which is what a
/// sprite that names neither an atlas nor a region draws.
fn batch_of(world: &World) -> SpriteBatch {
    SpriteBatch::from_world(world, Sheets::default())
}

fn world_with(sprites: &[(Transform, Sprite)]) -> World {
    let mut world = World::new();
    for (transform, sprite) in sprites {
        let e = world.spawn();
        world.insert(e, *transform);
        world.insert(e, sprite.clone());
    }
    world
}

#[test]
fn an_empty_world_produces_no_geometry() {
    let batch = batch_of(&World::new());
    assert!(batch.vertices.is_empty());
    assert!(batch.indices.is_empty());
    assert_eq!(batch.sprite_count(), 0);
}

#[test]
fn an_entity_without_a_sprite_is_skipped() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Transform::default());

    assert_eq!(batch_of(&world).sprite_count(), 0);
}

#[test]
fn one_sprite_becomes_a_quad() {
    let batch = batch_of(&world_with(&[(Transform::default(), Sprite::default())]));

    assert_eq!(batch.vertices.len(), 4);
    assert_eq!(batch.indices.len(), 6);
    assert_eq!(batch.sprite_count(), 1);
}

#[test]
fn an_untransformed_sprite_spans_one_unit_around_the_origin() {
    let batch = batch_of(&world_with(&[(Transform::default(), Sprite::default())]));

    let xs: Vec<f32> = batch.vertices.iter().map(|v| v.position[0]).collect();
    let ys: Vec<f32> = batch.vertices.iter().map(|v| v.position[1]).collect();

    // Centred, not corner-anchored: rotation and scale should act about
    // the sprite's middle.
    assert!(xs.contains(&-0.5) && xs.contains(&0.5));
    assert!(ys.contains(&-0.5) && ys.contains(&0.5));
}

#[test]
fn the_transform_moves_the_quad() {
    let batch = batch_of(&world_with(&[(
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
    let batch = batch_of(&world_with(&[(
        Transform::default().with_scale(Vec2::splat(4.0)),
        Sprite::default(),
    )]));

    let xs: Vec<f32> = batch.vertices.iter().map(|v| v.position[0]).collect();
    assert!(xs.contains(&-2.0) && xs.contains(&2.0));
}

#[test]
fn the_sprite_colour_reaches_every_vertex() {
    let sprite = Sprite::new([0.25, 0.5, 0.75, 1.0]);
    let batch = batch_of(&world_with(&[(Transform::default(), sprite)]));

    for v in &batch.vertices {
        assert_eq!(v.color, [0.25, 0.5, 0.75]);
    }
}

#[test]
fn uvs_cover_the_whole_texture_with_v_pointing_down() {
    let batch = batch_of(&world_with(&[(Transform::default(), Sprite::default())]));

    let top = batch
        .vertices
        .iter()
        .find(|v| v.position[1] > 0.0)
        .expect("a quad has a top edge");
    assert_eq!(top.uv[1], 0.0, "the top of the quad samples the top row");
}

#[test]
fn a_second_sprite_gets_its_own_index_range() {
    let batch = batch_of(&world_with(&[
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
    let batch = batch_of(&world_with(&[
        (Transform::default(), Sprite::default()),
        (Transform::default(), Sprite::default()),
        (Transform::default(), Sprite::default()),
    ]));

    let count = batch.vertices.len() as u32;
    assert!(batch.indices.iter().all(|&i| i < count));
}

/// Spawns each sprite and hands back the handles, so a test can despawn a
/// specific one.
fn world_returning_entities(sprites: &[(Transform, Sprite)]) -> (World, Vec<voltra_ecs::Entity>) {
    let mut world = World::new();
    let mut entities = Vec::new();
    for (transform, sprite) in sprites {
        let e = world.spawn();
        world.insert(e, *transform);
        world.insert(e, sprite.clone());
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

    let batch = batch_of(&world);
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
    let xs = draw_order(&batch_of(&world));
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

    let xs = draw_order(&batch_of(&world));
    assert_eq!(xs, vec![1.0, 3.0, 4.0]);
}

#[test]
fn a_sprite_defaults_to_sort_order_zero() {
    assert_eq!(Sprite::default().sort_order, 0);
    assert_eq!(Sprite::new([1.0, 0.0, 0.0, 1.0]).sort_order, 0);
}

fn sprite_with_texture(handle: Option<Handle<Texture>>) -> Sprite {
    Sprite {
        texture_handle: handle,
        ..Default::default()
    }
}

/// These four exercise [`SpriteBatch::push`] directly, with no [`World`]
/// and no `from_world` in the call stack: `push` maintains `ranges`
/// itself now, so the invariant has to hold from an empty batch onward,
/// not just after `from_world` has walked one.
#[test]
fn push_alone_merges_contiguous_same_handles_into_one_range() {
    let a = Handle::forge(0, 0);
    let mut batch = SpriteBatch::default();
    batch.push(
        Transform::default().matrix(),
        &sprite_with_texture(Some(a)),
        Sheets::default(),
    );
    batch.push(
        Transform::default().matrix(),
        &sprite_with_texture(Some(a)),
        Sheets::default(),
    );

    assert_eq!(
        batch.ranges,
        vec![SpriteRange {
            texture: Some(a),
            indices: 0..12,
        }]
    );
}

#[test]
fn push_alone_splits_a_range_when_the_handle_changes() {
    let a = Handle::forge(0, 0);
    let b = Handle::forge(1, 0);
    let mut batch = SpriteBatch::default();
    batch.push(
        Transform::default().matrix(),
        &sprite_with_texture(Some(a)),
        Sheets::default(),
    );
    batch.push(
        Transform::default().matrix(),
        &sprite_with_texture(Some(b)),
        Sheets::default(),
    );
    batch.push(
        Transform::default().matrix(),
        &sprite_with_texture(Some(a)),
        Sheets::default(),
    );

    assert_eq!(
        batch.ranges,
        vec![
            SpriteRange {
                texture: Some(a),
                indices: 0..6,
            },
            SpriteRange {
                texture: Some(b),
                indices: 6..12,
            },
            SpriteRange {
                texture: Some(a),
                indices: 12..18,
            },
        ]
    );
}

#[test]
fn push_alone_keeps_none_and_some_in_separate_ranges() {
    let a = Handle::forge(0, 0);
    let mut batch = SpriteBatch::default();
    batch.push(
        Transform::default().matrix(),
        &sprite_with_texture(None),
        Sheets::default(),
    );
    batch.push(
        Transform::default().matrix(),
        &sprite_with_texture(Some(a)),
        Sheets::default(),
    );

    assert_eq!(
        batch.ranges,
        vec![
            SpriteRange {
                texture: None,
                indices: 0..6,
            },
            SpriteRange {
                texture: Some(a),
                indices: 6..12,
            },
        ]
    );
}

#[test]
fn a_fresh_batch_has_no_ranges_until_something_is_pushed() {
    assert!(SpriteBatch::default().ranges.is_empty());
}

#[test]
fn draw_order_still_follows_sort_order() {
    // Same setup as `a_higher_sort_order_is_drawn_later`, but each sprite
    // also carries a distinct forged handle so the ranges can be checked
    // against the same reordering: draw order, not spawn order, decides
    // both the vertex layout and which run each sprite lands in.
    let low = Handle::forge(0, 0);
    let high = Handle::forge(1, 0);
    let (world, _) = world_returning_entities(&[
        (
            Transform::from_translation(Vec2::new(10.0, 0.0)),
            Sprite {
                texture_handle: Some(high),
                ..Sprite::default().with_sort_order(5)
            },
        ),
        (
            Transform::from_translation(Vec2::new(20.0, 0.0)),
            Sprite {
                texture_handle: Some(low),
                ..Sprite::default().with_sort_order(-3)
            },
        ),
    ]);

    let batch = batch_of(&world);
    let xs = draw_order(&batch);

    assert!(xs[0] > 19.0, "expected the -3 sprite first, got {xs:?}");
    assert!(xs[1] < 11.0, "expected the 5 sprite last, got {xs:?}");
    assert_eq!(
        batch.ranges,
        vec![
            SpriteRange {
                texture: Some(low),
                indices: 0..6,
            },
            SpriteRange {
                texture: Some(high),
                indices: 6..12,
            },
        ],
        "ranges must follow draw order, not spawn order"
    );
}

#[test]
fn from_world_builds_one_range_for_untextured_sprites() {
    let batch = batch_of(&world_with(&[
        (Transform::default(), sprite_with_texture(None)),
        (
            Transform::from_translation(Vec2::new(5.0, 0.0)),
            sprite_with_texture(None),
        ),
    ]));

    assert_eq!(
        batch.ranges,
        vec![SpriteRange {
            texture: None,
            indices: 0..12,
        }]
    );
}

#[test]
fn an_empty_world_produces_no_ranges() {
    assert!(batch_of(&World::new()).ranges.is_empty());
}

#[test]
fn a_batch_past_sixty_five_thousand_vertices_still_indexes_its_own_quads() {
    // 16 384 sprites is 65 536 vertices, exactly where a `u16` base
    // wraps. The 16 385th sprite's indices must point at its own four
    // vertices; with a `u16` cast they point at the first sprite's, and
    // nothing — not wgpu, not a validation layer — reports it.
    const SPRITES: u32 = 16_385;

    let mut batch = SpriteBatch::default();
    for _ in 0..SPRITES {
        batch.push(
            Transform::default().matrix(),
            &Sprite::default(),
            Sheets::default(),
        );
    }

    let last = batch
        .indices
        .last()
        .copied()
        .expect("a pushed batch has indices");
    assert!(
        last > u16::MAX as u32,
        "the last index wrapped: got {last}, expected past {}",
        u16::MAX
    );
    assert_eq!(batch.vertices.len() as u32, SPRITES * 4);
    assert!(batch
        .indices
        .iter()
        .all(|&i| i < batch.vertices.len() as u32));
}
