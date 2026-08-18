//! What an entity is called.

use serde::{Deserialize, Serialize};

/// A human-readable label for an entity.
///
/// Optional, like every other component: an entity without one is still a
/// perfectly good entity, and the editor falls back to its index. Unity and
/// Godot both make the name mandatory and unique-ish per parent; ours is
/// neither, because nothing addresses an entity by name — [`SceneId`] is what
/// identity means here, and a name that is also an address turns a rename into
/// a broken reference.
///
/// [`SceneId`]: crate::SceneId
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Name(pub String);

impl Name {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Name {
    /// What a freshly spawned entity is called before anyone renames it.
    fn default() -> Self {
        Self::new("Entity")
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_round_trips_as_a_bare_string() {
        // `transparent`: the file says `Name: "Player"`, not `Name: ("Player")`.
        // A scene file is read and merged by hand, so the shape of a name in it
        // is part of the format.
        let text = ron::to_string(&Name::new("Player")).expect("a string serializes");
        assert_eq!(text, "\"Player\"");
        assert_eq!(
            ron::from_str::<Name>(&text).expect("what we just wrote"),
            Name::new("Player")
        );
    }

    #[test]
    fn names_are_not_unique_and_do_not_have_to_be() {
        // Two entities may share a name. Nothing addresses an entity by name,
        // so there is no collision to resolve.
        assert_eq!(Name::new("Crate"), Name::new("Crate"));
    }

    #[test]
    fn an_empty_name_is_allowed() {
        // Mid-edit, a text field is empty for exactly one keystroke. Rejecting
        // it would mean the editor cannot clear the field before typing.
        assert_eq!(Name::new("").as_str(), "");
    }
}
