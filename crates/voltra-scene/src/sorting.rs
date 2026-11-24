//! Named sorting layers: the coarse draw order, above `sort_order`.

use std::collections::HashSet;
use std::sync::Mutex;

/// The layer a drawable is on when it names none.
pub const DEFAULT_LAYER: &str = "Default";

/// The project's sorting layers, back to front.
///
/// Unity's sorting layers: a drawable names one, and draws after everything
/// on the layers before it whatever its `sort_order`, which orders it only
/// among its own layer. A name rather than a bare number so a level reads
/// "Background" instead of `-10`, and so moving a whole band forward is one
/// edit to this list rather than to every sprite in it.
#[derive(Debug)]
pub struct SortingLayers {
    names: Vec<String>,
    /// Names asked for that are not in the list, each warned about once.
    unknown: Mutex<HashSet<String>>,
}

impl Default for SortingLayers {
    /// Background, Default, Foreground: what a 2D level is first split into.
    fn default() -> Self {
        Self::new(["Background", DEFAULT_LAYER, "Foreground"])
    }
}

impl Clone for SortingLayers {
    fn clone(&self) -> Self {
        Self::new(self.names.iter().cloned())
    }
}

impl SortingLayers {
    /// Layers in `names`, back to front. [`DEFAULT_LAYER`] is added at the
    /// front if missing, since every drawable that names none is on it.
    pub fn new<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Self {
        let mut list: Vec<String> = Vec::new();
        for name in names {
            let name = name.into();
            if !list.contains(&name) {
                list.push(name);
            }
        }
        if !list.iter().any(|n| n == DEFAULT_LAYER) {
            list.insert(0, DEFAULT_LAYER.to_owned());
        }
        Self {
            names: list,
            unknown: Mutex::new(HashSet::new()),
        }
    }

    /// Every layer, back to front.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Where `name` sorts: its position in the list. A name not in it sorts
    /// as [`DEFAULT_LAYER`] and is logged once — a typo in a layer name is
    /// the draw-order equivalent of a missing texture.
    pub fn rank(&self, name: &str) -> i32 {
        if let Some(rank) = self.names.iter().position(|n| n == name) {
            return rank as i32;
        }
        if let Ok(mut unknown) = self.unknown.lock() {
            if unknown.insert(name.to_owned()) {
                log::warn!(
                    "sorting layer `{name}` is not a layer; drawing it on `{DEFAULT_LAYER}`"
                );
            }
        }
        self.names
            .iter()
            .position(|n| n == DEFAULT_LAYER)
            .unwrap_or(0) as i32
    }
}

/// The serde default for a drawable's `sorting_layer`.
pub(crate) fn default_layer() -> String {
    DEFAULT_LAYER.to_owned()
}

/// Whether a drawable's layer is the default, which is not written.
pub(crate) fn is_default_layer(name: &String) -> bool {
    name == DEFAULT_LAYER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_rank_back_to_front() {
        let layers = SortingLayers::default();
        assert!(layers.rank("Background") < layers.rank(DEFAULT_LAYER));
        assert!(layers.rank(DEFAULT_LAYER) < layers.rank("Foreground"));
    }

    #[test]
    fn an_unknown_name_sorts_as_default() {
        let layers = SortingLayers::default();
        assert_eq!(layers.rank("Bakground"), layers.rank(DEFAULT_LAYER));
    }

    #[test]
    fn a_list_without_default_gets_it_at_the_back() {
        let layers = SortingLayers::new(["Sky", "World"]);
        assert_eq!(layers.names()[0], DEFAULT_LAYER);
        assert_eq!(layers.rank("World"), 2);
    }

    #[test]
    fn duplicates_are_dropped() {
        let layers = SortingLayers::new(["A", "A", DEFAULT_LAYER]);
        assert_eq!(layers.names().len(), 2);
    }

    mod in_the_world {
        use voltra_ecs::{Entity, World};
        use voltra_render::glam::Vec2;

        use super::super::*;
        use crate::pick::entity_at;
        use crate::sprite::sheets::Sheets;
        use crate::{Sprite, SpriteBatch, Transform};

        fn sprite_on(world: &mut World, layer: &str, order: i32, color: [f32; 4]) -> Entity {
            let e = world.spawn();
            world.insert(e, Transform::default());
            let mut sprite = Sprite::new(color).with_sort_order(order);
            sprite.sorting_layer = layer.to_owned();
            world.insert(e, sprite);
            e
        }

        #[test]
        fn a_later_layer_draws_over_a_higher_sort_order() {
            let mut world = World::new();
            sprite_on(&mut world, "Foreground", -100, [1.0, 0.0, 0.0, 1.0]);
            sprite_on(&mut world, "Background", 100, [0.0, 0.0, 1.0, 1.0]);
            let batch = SpriteBatch::from_world(&world, Sheets::default());
            assert_eq!(
                batch.quad_vertices()[0].color,
                [0.0, 0.0, 1.0, 1.0],
                "background first"
            );
        }

        #[test]
        fn the_pick_agrees_with_the_layer() {
            let mut world = World::new();
            let front = sprite_on(&mut world, "Foreground", -100, [1.0; 4]);
            sprite_on(&mut world, DEFAULT_LAYER, 100, [1.0; 4]);
            assert_eq!(
                entity_at(&world, Vec2::ZERO, Sheets::default()),
                Some(front)
            );
        }

        #[test]
        fn the_project_s_own_list_decides() {
            let mut world = World::new();
            sprite_on(&mut world, "Sky", 0, [0.0, 0.0, 1.0, 1.0]);
            sprite_on(&mut world, "Ground", 0, [0.0, 1.0, 0.0, 1.0]);
            let layers = SortingLayers::new(["Ground", "Sky"]);
            let batch = SpriteBatch::from_world(&world, Sheets::default().with_layers(&layers));
            assert_eq!(
                batch.quad_vertices()[0].color,
                [0.0, 1.0, 0.0, 1.0],
                "ground is behind here"
            );
        }

        #[test]
        fn the_default_layer_is_not_written_and_an_old_sprite_reads_on_it() {
            let text = ron::to_string(&Sprite::default()).expect("writes");
            assert!(!text.contains("sorting_layer"));
            let back: Sprite = ron::from_str(&text).expect("reads");
            assert_eq!(back.sorting_layer, DEFAULT_LAYER);
        }
    }
}
