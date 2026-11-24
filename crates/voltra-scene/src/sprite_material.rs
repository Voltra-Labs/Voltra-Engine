//! The material a sprite or a tile layer is drawn with, and the values it
//! gives that material's uniforms.

use std::collections::BTreeMap;

use voltra_assets::{AssetPath, Handle, Material, Materials};
use voltra_render::material::Uniform;

/// Draws a [`Sprite`](crate::Sprite) or [`Tilemap`](crate::Tilemap)
/// through a material instead of the plain sprite shader — a hit flash, an
/// outline, a dissolve — with this entity's own values for its uniforms.
///
/// The material is shared, as Unity's and Godot's are; the values are the
/// entity's, so one enemy flashes while the rest do not. A uniform this
/// does not name keeps the material's default. Set from the inspector, or
/// from a tick with [`set`](Self::set).
///
/// Drawn in the same pass and painter's order as every sprite: a run of
/// sprites splits only where the material or its values change, so a scene
/// with none batches exactly as before.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpriteMaterial {
    /// The `.material.ron` file, from the asset root.
    pub path: AssetPath,
    /// This entity's values, by uniform name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub uniforms: BTreeMap<String, Uniform>,
    /// What `path` resolves to in this session's [`Materials`]. Never
    /// written.
    #[serde(skip)]
    pub handle: Option<Handle<Material>>,
}

impl SpriteMaterial {
    /// `path`, with the material's defaults.
    pub fn new(path: AssetPath) -> Self {
        Self {
            path,
            uniforms: BTreeMap::new(),
            handle: None,
        }
    }

    /// Gives the uniform `name` this entity's `value`.
    pub fn set(&mut self, name: &str, value: Uniform) {
        self.uniforms.insert(name.to_owned(), value);
    }

    /// Resolves `path` in `materials`.
    pub fn resolve(&mut self, materials: &mut Materials) {
        self.handle = Some(materials.load(&self.path));
    }
}

/// Resolves the material of every sprite and tile layer in `world`.
pub fn resolve_world(world: &mut voltra_ecs::World, materials: &mut Materials) {
    for (_, sprite) in world.query_mut::<crate::Sprite>() {
        if let Some(slot) = &mut sprite.material {
            slot.resolve(materials);
        }
    }
    for (_, map) in world.query_mut::<crate::Tilemap>() {
        if let Some(slot) = &mut map.material {
            slot.resolve(materials);
        }
    }
}

/// Resolves the material of `entity`'s sprite and tile layer, if it has
/// them: what a spawn needs, without walking the world.
pub fn resolve_entity(
    world: &mut voltra_ecs::World,
    entity: voltra_ecs::Entity,
    materials: &mut Materials,
) {
    if let Some(slot) = world
        .get_mut::<crate::Sprite>(entity)
        .and_then(|s| s.material.as_mut())
    {
        slot.resolve(materials);
    }
    if let Some(slot) = world
        .get_mut::<crate::Tilemap>(entity)
        .and_then(|m| m.material.as_mut())
    {
        slot.resolve(materials);
    }
}
