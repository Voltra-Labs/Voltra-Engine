//! Scene load: a RON scene file parsed, and its entities spawned.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::format::{from_scene_file, to_scene_file, SceneFile};
use voltra_scene::{
    Collider2D, ComponentRegistry, Name, RigidBody2D, SceneId, Sprite2D, Transform2D,
};

use super::{Bench, Outcome};
use crate::harness::{iter, iter_batched, measure};

/// Entities in the scenes parsed: a small level, and one twice its size —
/// small because a large one takes seconds (1 000 entities: 1.5 s).
/// The pair shows how the cost grows with the file, which is not linearly
/// (docs/ARCHITECTURE.md, stage 107: ron rescans the rest of the input for
/// every number it reads into a `RawValue`).
const PARSED: [(usize, f64); 2] = [(250, 15.0), (500, 20.0)];

/// Entities in the scene loaded whole.
const LOADED: usize = 250;

pub(super) fn benches() -> Vec<Bench> {
    let mut benches: Vec<Bench> = PARSED
        .into_iter()
        .map(|(count, threshold)| {
            Bench::new(
                format!("scene/parse/{count}-entities"),
                threshold,
                move |config| {
                    let Some(text) = scene_text(count) else {
                        return unwritable();
                    };
                    measure(
                        config,
                        1,
                        iter(|| ron::from_str::<SceneFile>(&text).map(|file| file.entities.len())),
                    )
                    .into()
                },
            )
        })
        .collect();
    benches.push(Bench::new(
        format!("scene/load/{LOADED}-entities"),
        15.0,
        |config| {
            let Some(text) = scene_text(LOADED) else {
                return unwritable();
            };
            let registry = ComponentRegistry::with_defaults();
            measure(
                config,
                1,
                // The world is made before the clock and dropped after it;
                // parsing and spawning are what a load costs.
                iter_batched(World::new, |mut world| {
                    let loaded = ron::from_str::<SceneFile>(&text)
                        .ok()
                        .map(|file| from_scene_file(&file, &registry, &mut world).is_ok());
                    debug_assert_eq!(loaded, Some(true));
                    world
                }),
            )
            .into()
        },
    ));
    benches
}

fn unwritable() -> Outcome {
    Outcome::Skipped("the generated scene would not write".to_owned())
}

/// `count` entities as a scene file writes them: a name, a transform and
/// a sprite each, and every fourth a dynamic body with a box.
fn scene_text(count: usize) -> Option<String> {
    let registry = ComponentRegistry::with_defaults();
    let mut world = World::new();
    for i in 0..count {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Name::new(format!("Entity {i}")));
        world.insert(
            entity,
            Transform2D::from_translation(Vec2::new((i % 100) as f32, (i / 100) as f32)),
        );
        world.insert(
            entity,
            Sprite2D {
                sort_order: (i % 5) as i32,
                ..Sprite2D::default()
            },
        );
        if i % 4 == 0 {
            world.insert(entity, RigidBody2D::new_dynamic(1.0));
            world.insert(
                entity,
                Collider2D::Box {
                    half_extents: Vec2::splat(0.5),
                },
            );
        }
    }
    let file = to_scene_file(&world, &registry).ok()?;
    ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).ok()
}
