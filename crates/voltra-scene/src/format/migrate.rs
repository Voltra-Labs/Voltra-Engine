//! Bringing a file an older build wrote up to the version this one reads.
//!
//! The loader migrates and the writer writes the current version: a file
//! from an older release is renamed in memory and then read as if it were
//! new, and nothing on disk changes until it is saved (or `cargo xtask
//! migrate` rewrites it). ARCHITECTURE, "The migration: format version 2".
//!
//! A rename reaches every place a component's name is written: the record
//! keys, and the values that hold component names — a prefab part's
//! overridden set, which is also where a variant's overrides live.

use std::borrow::Cow;

use ron::value::RawValue;

use super::error::SceneError;
use super::save::{EntityRecord, SceneFile, VERSION};
use crate::prefab::PrefabPart;

/// The oldest version this build reads.
pub const OLDEST: u32 = 1;

/// The renames that take a file from each version to the next:
/// `(from, [(old name, new name)])`, one row per version.
///
/// Data, so the next rename is one more pair, and the strings are history —
/// the names as each version spelled them, not the current `NAME` consts,
/// which a later rename moves on.
pub const RENAMES: &[(u32, &[(&str, &str)])] = &[(
    1,
    &[
        ("Transform", "Transform2D"),
        ("Camera", "Camera2D"),
        ("Sprite", "Sprite2D"),
        ("RigidBody", "RigidBody2D"),
        ("Collider", "Collider2D"),
    ],
)];

/// The renames from `version` to the next, or `None` past the table.
fn renames_from(version: u32) -> Option<Renames> {
    RENAMES
        .iter()
        .find(|(from, _)| *from == version)
        .map(|(_, renames)| *renames)
}

/// Whether a file of `version` can be read: as old as [`OLDEST`], no newer
/// than this build.
fn check(version: u32) -> Result<(), SceneError> {
    if (OLDEST..=VERSION).contains(&version) {
        Ok(())
    } else {
        Err(SceneError::UnsupportedVersion {
            found: version,
            supported: VERSION,
        })
    }
}

/// `file` as the current version says it: renamed through every step
/// between, or refused when it is newer than this build (or older than any
/// release). A current file comes back as it was.
pub fn upgrade(mut file: SceneFile) -> Result<SceneFile, SceneError> {
    check(file.version)?;
    while file.version < VERSION {
        let renames = renames_from(file.version)
            .expect("RENAMES has a row for every version from OLDEST to VERSION");
        for record in &mut file.entities {
            rename_record(record, renames, file.version)?;
        }
        file.version += 1;
    }
    Ok(file)
}

/// [`upgrade`], for a file that is borrowed: no copy when it is current.
pub fn current(file: &SceneFile) -> Result<Cow<'_, SceneFile>, SceneError> {
    check(file.version)?;
    if file.version == VERSION {
        Ok(Cow::Borrowed(file))
    } else {
        upgrade(file.clone()).map(Cow::Owned)
    }
}

/// One version step's renames.
type Renames = &'static [(&'static str, &'static str)];

/// The refusal of a name an older file holds that the step gives to an
/// engine component: no component of that version was called it, so it is
/// a game's own, and reading it on would make it the engine's — or drop
/// one of the two.
fn taken(record: &EntityRecord, name: &str, version: u32, held: &str) -> SceneError {
    SceneError::Component {
        name: name.to_owned(),
        source: ron::Error::Message(format!(
            "entity {} {held} `{name}`, which version {version} did not name but version {} \
             gives the engine's component; rename your component and its files first",
            record.id.0,
            version + 1
        )),
    }
}

/// Renames one record's components, and the names its values hold, from
/// `version` to the next.
fn rename_record(
    record: &mut EntityRecord,
    renames: Renames,
    version: u32,
) -> Result<(), SceneError> {
    let renamed = |name: &str| -> Option<&'static str> {
        renames
            .iter()
            .find(|(old, _)| *old == name)
            .map(|(_, new)| *new)
    };
    let is_target = |name: &str| renames.iter().any(|(_, new)| *new == name);
    for name in record.components.keys() {
        if is_target(name) {
            return Err(taken(record, name, version, "holds"));
        }
    }
    let part = match record.components.get(PrefabPart::NAME) {
        Some(value) => Some(read_part(value)?),
        None => None,
    };
    if let Some(name) = part
        .iter()
        .flat_map(|part| part.overridden.iter())
        .find(|name| is_target(name))
    {
        return Err(taken(record, name, version, "overrides"));
    }

    let old = std::mem::take(&mut record.components);
    record.components = old
        .into_iter()
        .map(|(name, value)| (renamed(&name).map_or(name, str::to_owned), value))
        .collect();
    if let Some(mut part) = part {
        // Left exactly as written when it names nothing that moved.
        if part.overridden.iter().any(|name| renamed(name).is_some()) {
            part.overridden = part
                .overridden
                .into_iter()
                .map(|name| renamed(&name).map_or(name, str::to_owned))
                .collect();
            let value = RawValue::from_rust(&part).map_err(SceneError::Serialize)?;
            record.components.insert(PrefabPart::NAME.to_owned(), value);
        }
    }
    Ok(())
}

/// A stored prefab part, or the error naming it.
fn read_part(value: &RawValue) -> Result<PrefabPart, SceneError> {
    value.into_rust().map_err(|source| SceneError::Component {
        name: PrefabPart::NAME.to_owned(),
        source: source.into(),
    })
}

/// `text` read as a scene file of any version this build reads, and brought
/// up to the current one: what every reader of a scene or prefab file
/// calls, so none of them can skip the migration.
pub fn parse(text: &str) -> Result<SceneFile, SceneError> {
    upgrade(ron::from_str(text).map_err(SceneError::Parse)?)
}

/// [`parse`], answering also the version the text was written in — for a
/// caller that tells its user an older file will be written anew.
pub fn parse_versioned(text: &str) -> Result<(SceneFile, u32), SceneError> {
    let file: SceneFile = ron::from_str(text).map_err(SceneError::Parse)?;
    let written = file.version;
    Ok((upgrade(file)?, written))
}

#[cfg(test)]
mod tests;
