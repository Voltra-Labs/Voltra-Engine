//! A new variant of a prefab: a file whose root is an instance of the base,
//! overriding nothing yet — Unity's Create ▸ Prefab Variant.
//!
//! Stored as a scene stores an instance (`resolve.rs`): the base's records
//! in full under ids of the variant's own, a [`super::PrefabInstance`] on
//! the root naming the base and a [`super::PrefabPart`] on each. What the
//! variant is then edited to differ by is recorded on those parts as
//! overrides, exactly as an instance's are, and every edit to the base
//! reaches the variant — and its instances — but for those.

use voltra_assets::AssetPath;
use voltra_ecs::World;

use super::cycle::reaches;
use super::resolve::written;
use super::write::WriteError;
use super::{spawn, Placement, Prefabs};
use crate::format::{ComponentRegistry, SceneError};

/// What a variant of `base` is called before a number makes it free: the
/// base's file name without its suffix, then ` Variant` — Unity's name.
pub fn variant_name(base: &AssetPath) -> String {
    let file = base.as_str().rsplit('/').next().unwrap_or(base.as_str());
    let lower = file.to_ascii_lowercase();
    let stem = [".prefab.ron", ".ron"]
        .iter()
        .find(|suffix| lower.ends_with(*suffix))
        .map_or(file, |suffix| &file[..file.len() - suffix.len()]);
    format!("{stem} Variant")
}

/// Writes `target` as a variant of `base`, and refreshes the cached copy.
///
/// Refused, with nothing written, when `target` is `base` or `base` holds
/// an instance of `target` already (the variant would contain itself), and
/// when `base` will not read or holds nothing. The write is atomic, like a
/// scene save.
pub fn variant(
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    base: &AssetPath,
    target: &AssetPath,
) -> Result<(), WriteError> {
    if let Some(through) = reaches(prefabs, [base.clone()], target) {
        return Err(WriteError::Cycle {
            target: target.clone(),
            through,
        });
    }
    let handle = prefabs.load(base);
    let prefab = prefabs.get(handle);
    if prefab.source().is_none() || prefab.is_empty() {
        return Err(WriteError::Scene(SceneError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} would not read, or holds nothing", base.as_str()),
        ))));
    }

    // An instance of the base as authored, in whichever dimension, written
    // out whole.
    let mut world = World::new();
    let instance = spawn(&mut world, registry, prefab, Placement::AsAuthored)?;
    let file = written(&world, registry, &instance.entities)?;

    let path = prefabs.root().join(target.as_str());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(SceneError::Io)?;
    }
    crate::format::write(&file, &path)?;
    prefabs.reload(target);
    Ok(())
}
