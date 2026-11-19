//! Stamping a stored fragment of a scene into a world, as many times as wanted.
//!
//! Unity's prefabs, Godot's instanced scenes, Unreal's blueprint classes: all
//! three separate *the asset* — a template on disk — from *the instance*, the
//! entities a spawn made out of it. This is the spawn half, and the asset is
//! the format the engine already has. A prefab **is** a scene file: one or
//! more roots and whatever hangs off them, in the same `.ron` every scene is
//! saved as, read through the same [`ComponentRegistry`]. Nothing new has to
//! be able to read one, and any scene can be instanced.
//!
//! **Instantiate and forget.** An instance is plain entities, edited and
//! saved like any other, which is Godot's "make local" taken as the default.
//! Each root does carry a [`PrefabInstance`] naming its file, but nothing
//! reads it back: override storage, propagating an edit to the prefab into
//! its instances, and what an instance becomes when the prefab gains a
//! component are where all three engines have paid, and they are their own
//! stage. The link is recorded now so that stage finds it in every scene.

mod link;
mod store;

use std::collections::HashMap;

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use crate::format::{ComponentRegistry, SceneError, SceneFile, VERSION};
use crate::hierarchy::Parent;
use crate::scene_id::SceneId;
use crate::transform::Transform;

pub use link::PrefabInstance;
pub use store::Prefabs;

/// A scene fragment, read and checked once, ready to be spawned.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Prefab {
    file: Option<SceneFile>,
    /// The path it was read from, which every root it spawns is linked to.
    /// `None` for one built in memory, whose instances have no file to name.
    source: Option<AssetPath>,
}

impl Prefab {
    /// A prefab holding `file`, or the version error that makes it unusable.
    ///
    /// Checked here rather than at every spawn: a bullet prefab is spawned a
    /// hundred times a second, and the answer does not change between them.
    pub fn new(file: SceneFile) -> Result<Self, SceneError> {
        if file.version != VERSION {
            return Err(SceneError::UnsupportedVersion {
                found: file.version,
                supported: VERSION,
            });
        }
        Ok(Self {
            file: Some(file),
            source: None,
        })
    }

    /// The same prefab, its instances linked to `source`.
    pub fn with_source(self, source: AssetPath) -> Self {
        Self {
            source: Some(source),
            ..self
        }
    }

    /// The path this prefab's instances are linked to, if it has one.
    pub fn source(&self) -> Option<&AssetPath> {
        self.source.as_ref()
    }

    /// The prefab that spawns nothing: what a file that will not read becomes.
    pub fn empty() -> Self {
        Self::default()
    }

    /// How many entities one spawn makes.
    pub fn len(&self) -> usize {
        self.file.as_ref().map_or(0, |file| file.entities.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The entities one spawn made.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Instance {
    /// Every entity, in the prefab's own order.
    pub entities: Vec<Entity>,
    /// The ones with no parent inside the instance — the ones `at` placed.
    pub roots: Vec<Entity>,
}

impl Instance {
    /// The first root, which is *the* root of the common one-root prefab.
    pub fn root(&self) -> Option<Entity> {
        self.roots.first().copied()
    }
}

/// Spawns a copy of `prefab` into `world`, its roots placed by `at`.
///
/// What a copy means, rule by rule:
///
/// - **Every entity gets a fresh [`SceneId`].** Two instances of one prefab
///   sharing ids would be two entities a save writes under one name, and a
///   load that put one of them back.
/// - **Parent links inside the prefab are remapped** to the fresh ids and
///   resolved on the spot, without the whole-world pass a load needs: every
///   entity a link can name is one this call just made.
/// - **A link to a parent outside the prefab is cut**, and its entity is a
///   root. The id it names belongs to whatever scene the fragment was cut
///   from, and following it would hang every instance off one entity there.
/// - **Roots are placed by `at`**, composed in front of their own transform,
///   so a prefab authored around the origin lands around `at` and keeps the
///   offsets it was authored with. Children are left alone: they are local to
///   their parent, and moving the root moved them. A root with no `Transform`
///   is not positional and is not given one.
/// - **Every root is linked to the prefab's source** with a
///   [`PrefabInstance`], replacing whatever link the record itself carried:
///   a root is an instance of the file it was just spawned from. A child's
///   own link is kept — that child is a nested instance.
///
/// All-or-nothing, like a load: a component that will not read despawns every
/// entity this call spawned before the error is returned.
pub fn spawn(
    world: &mut World,
    registry: &ComponentRegistry,
    prefab: &Prefab,
    at: &Transform,
) -> Result<Instance, SceneError> {
    let Some(file) = prefab.file.as_ref() else {
        return Ok(Instance::default());
    };

    // One fresh id per record, and a map from the old ids for the links. The
    // map keeps the first record under a duplicated id, which is what a link
    // to it resolved to in the file as well.
    let fresh: Vec<SceneId> = file.entities.iter().map(|_| SceneId::new()).collect();
    let mut renamed: HashMap<SceneId, SceneId> = HashMap::with_capacity(fresh.len());
    for (record, id) in file.entities.iter().zip(&fresh) {
        renamed.entry(record.id).or_insert(*id);
    }

    let mut entities = Vec::with_capacity(file.entities.len());
    for (record, id) in file.entities.iter().zip(&fresh) {
        let entity = world.spawn();
        entities.push(entity);
        world.insert(entity, *id);

        match crate::format::record::insert_components(world, registry, entity, &record.components)
        {
            Ok(unknown) if !unknown.0.is_empty() => {
                world.insert(entity, unknown);
            }
            Ok(_) => {}
            Err(error) => {
                for entity in entities {
                    world.despawn(entity);
                }
                return Err(error);
            }
        }
    }

    let by_id: HashMap<SceneId, Entity> = fresh
        .iter()
        .copied()
        .zip(entities.iter().copied())
        .collect();
    let mut roots = Vec::new();
    for &entity in &entities {
        let Some(link) = world.get::<Parent>(entity).map(|link| link.id) else {
            roots.push(entity);
            continue;
        };
        match renamed.get(&link) {
            Some(&parent) => {
                world.insert(entity, Parent::resolved(parent, by_id[&parent]));
            }
            None => {
                log::debug!(
                    "prefab entity names a parent outside the prefab; spawning it as a root"
                );
                world.remove::<Parent>(entity);
                roots.push(entity);
            }
        }
    }

    for &root in &roots {
        if let Some(transform) = world.get_mut::<Transform>(root) {
            *transform = place(at, transform);
        }
        if let Some(source) = &prefab.source {
            world.insert(root, PrefabInstance::new(source.clone()));
        }
    }

    Ok(Instance { entities, roots })
}

/// `local` as seen through `at`.
///
/// A placement that only moves is applied as an addition rather than through
/// a matrix: it is the common case — a bullet at the muzzle, a coin on a tile
/// — and the decomposition [`Transform::from_matrix`] does would put float
/// noise into an authored rotation and scale that nothing asked to change.
fn place(at: &Transform, local: &Transform) -> Transform {
    if at.rotation == 0.0 && at.scale == voltra_render::glam::Vec2::ONE {
        return Transform {
            translation: local.translation + at.translation,
            ..*local
        };
    }
    Transform::from_matrix(at.matrix() * local.matrix())
}

#[cfg(test)]
mod tests;
