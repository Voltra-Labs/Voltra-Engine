//! The identity a scene file uses to name an asset.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::AssetError;

/// A validated, normalised path relative to the asset root.
///
/// **This type is a security boundary.** A scene file is external input — it
/// arrives in a pull request, from a collaborator, from the internet. A raw
/// string from one could name `../../../../Windows/System32/config/SAM`, and
/// merely opening the scene would read it; the PNG decode would fail, but the
/// read has already happened and the error distinguishes "missing" from
/// "malformed", which is a filesystem oracle driven by a `.ron` someone sent
/// you.
///
/// The invariant therefore lives in the constructor rather than in every
/// caller's memory: a value of this type cannot name anything outside the
/// root, so [`Textures`](crate::textures::Textures) needs no second check and
/// cannot forget one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetPath(String);

impl AssetPath {
    /// Validates and normalises `raw`.
    ///
    /// Rejects absolute paths, volume prefixes and any `..`. Normalises
    /// backslashes to forward slashes, collapses `.` and repeated separators.
    pub fn new(raw: &str) -> Result<Self, AssetError> {
        // Backslashes first, so a Windows-flavoured path is judged by the same
        // rules as a Unix one rather than arriving as a single component.
        let unified = raw.replace('\\', "/");

        if unified.starts_with('/') {
            return Err(AssetError::Absolute(raw.to_owned()));
        }
        // Catches the `C:` volume prefix and, on NTFS, `file.png:$DATA` — an
        // alternate data stream is a different file wearing the same name.
        if unified.contains(':') {
            return Err(AssetError::Absolute(raw.to_owned()));
        }

        let mut parts = Vec::new();
        for part in unified.split('/') {
            match part {
                "" | "." => continue,
                ".." => return Err(AssetError::EscapesRoot(raw.to_owned())),
                other => parts.push(other),
            }
        }

        if parts.is_empty() {
            return Err(AssetError::Empty);
        }

        Ok(Self(parts.join("/")))
    }

    /// The normalised path, relative to the asset root, forward slashes.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The wire form, kept separate from the type that carries the invariant.
///
/// An enum from the first file written, so a `Uuid` variant can be added later
/// without changing the scene format's `VERSION` — files already on disk say
/// which shape they use. Bevy chose a path as the canonical identity for the
/// same reason we do, and deferred UUIDs; Godot and Unity pay for a sidecar
/// per asset to survive a rename, which needs an editor that manages moves.
///
/// Private, so adding a variant is not a breaking change to this crate's API.
#[derive(Serialize, Deserialize)]
#[serde(rename = "AssetPath")]
enum Repr {
    Path(String),
}

impl Serialize for AssetPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Repr::Path(self.0.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AssetPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Routed through `new` rather than derived. A derived impl would skip
        // the constructor, and with it every check above — on exactly the path
        // a scene file takes, which is the only path that matters here.
        let Repr::Path(raw) = Repr::deserialize(deserializer)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(raw: &str) -> String {
        AssetPath::new(raw)
            .expect("this path is valid")
            .as_str()
            .to_owned()
    }

    #[test]
    fn a_plain_relative_path_survives_unchanged() {
        assert_eq!(ok("sprites/hero.png"), "sprites/hero.png");
    }

    #[test]
    fn a_leading_dot_is_collapsed() {
        // Two spellings of one file must be one cache entry, or the same PNG
        // is uploaded to the GPU twice.
        assert_eq!(ok("./sprites/hero.png"), "sprites/hero.png");
        assert_eq!(ok("sprites/./hero.png"), "sprites/hero.png");
    }

    #[test]
    fn backslashes_normalise_to_forward_slashes() {
        // A path typed on Windows and a path typed on Linux must be the same
        // entry, and the file it names has to be the same on both.
        assert_eq!(ok(r"sprites\hero.png"), "sprites/hero.png");
    }

    #[test]
    fn repeated_separators_collapse() {
        assert_eq!(ok("sprites//hero.png"), "sprites/hero.png");
    }

    #[test]
    fn a_parent_component_is_rejected() {
        // The reason this type exists. A scene file is external input, and
        // `..` in it would read a file outside the project.
        assert!(matches!(
            AssetPath::new("../../etc/passwd"),
            Err(AssetError::EscapesRoot(_))
        ));
        assert!(matches!(
            AssetPath::new("sprites/../../secret"),
            Err(AssetError::EscapesRoot(_))
        ));
    }

    #[test]
    fn a_unix_absolute_path_is_rejected() {
        assert!(matches!(
            AssetPath::new("/etc/passwd"),
            Err(AssetError::Absolute(_))
        ));
    }

    #[test]
    fn a_windows_volume_path_is_rejected() {
        assert!(matches!(
            AssetPath::new(r"C:\Windows\System32\config\SAM"),
            Err(AssetError::Absolute(_))
        ));
    }

    #[test]
    fn a_unc_path_is_rejected() {
        assert!(matches!(
            AssetPath::new(r"\\server\share\file.png"),
            Err(AssetError::Absolute(_))
        ));
        assert!(matches!(
            AssetPath::new(r"\\?\C:\file.png"),
            Err(AssetError::Absolute(_))
        ));
    }

    #[test]
    fn an_alternate_data_stream_is_rejected() {
        // `hero.png:$DATA` names a different stream of the same file on NTFS.
        // The colon check that catches `C:` catches this too, deliberately.
        assert!(matches!(
            AssetPath::new("sprites/hero.png:$DATA"),
            Err(AssetError::Absolute(_))
        ));
    }

    #[test]
    fn an_empty_path_is_rejected() {
        assert!(matches!(AssetPath::new(""), Err(AssetError::Empty)));
        assert!(matches!(AssetPath::new("./"), Err(AssetError::Empty)));
    }

    #[test]
    fn two_spellings_of_one_path_are_the_same_value() {
        let a = AssetPath::new("./sprites/hero.png").expect("valid");
        let b = AssetPath::new(r"sprites\hero.png").expect("valid");
        assert_eq!(a, b);

        let mut map = std::collections::HashMap::new();
        map.insert(a, 1);
        assert_eq!(map.get(&b), Some(&1), "and therefore one cache entry");
    }

    #[test]
    fn it_round_trips_through_ron() {
        let path = AssetPath::new("sprites/hero.png").expect("valid");
        let text = ron::to_string(&path).expect("serializes");
        assert_eq!(text, r#"Path("sprites/hero.png")"#);
        let back: AssetPath = ron::from_str(&text).expect("deserializes");
        assert_eq!(back, path);
    }

    #[test]
    fn a_hostile_path_in_a_document_is_rejected_on_deserialize() {
        // The check has to be on the path a scene file actually takes. A
        // derived `Deserialize` would skip the constructor and let this
        // through, which is the whole reason it is written by hand.
        let hostile = r#"Path("../../../../Windows/System32/config/SAM")"#;
        assert!(ron::from_str::<AssetPath>(hostile).is_err());
    }

    #[test]
    fn a_normalising_path_normalises_on_deserialize_too() {
        let back: AssetPath = ron::from_str(r#"Path("./sprites/hero.png")"#).expect("deserializes");
        assert_eq!(back.as_str(), "sprites/hero.png");
    }
}
