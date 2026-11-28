//! How a scene names a mesh: a built-in one, or a mesh file.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::builtin::BuiltinMesh;
use crate::path::AssetPath;

/// The prefix that names a [`BuiltinMesh`] in a file: `builtin:cube`.
pub const BUILTIN_PREFIX: &str = "builtin:";

/// A mesh as a scene names it, written as one string: `"builtin:cube"`,
/// or a path under the asset root.
///
/// Not an [`AssetPath`] alone, which refuses a `:` (a Windows volume, an
/// NTFS stream) as part of being the asset root's security boundary: a
/// built-in mesh is no file, and a path is still checked by `AssetPath`.
/// An unknown built-in name is refused when the file loads, naming the
/// ones there are, as a misspelt enum variant is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MeshPath {
    /// One the engine generates.
    Builtin(BuiltinMesh),
    /// A mesh file under the asset root.
    File(AssetPath),
}

impl Default for MeshPath {
    fn default() -> Self {
        Self::Builtin(BuiltinMesh::Cube)
    }
}

/// Why a string names no mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshPathError {
    /// `builtin:` followed by a name no built-in mesh has.
    UnknownBuiltin(String),
    /// Not a valid path under the asset root.
    Path(String),
}

impl fmt::Display for MeshPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBuiltin(name) => {
                let names: Vec<&str> = BuiltinMesh::ALL.iter().map(|m| m.name()).collect();
                write!(
                    f,
                    "no built-in mesh is called `{name}`; there are {}",
                    names.join(", ")
                )
            }
            Self::Path(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for MeshPathError {}

impl MeshPath {
    /// The mesh `text` names.
    pub fn parse(text: &str) -> Result<Self, MeshPathError> {
        match text.strip_prefix(BUILTIN_PREFIX) {
            Some(name) => BuiltinMesh::from_name(name)
                .map(Self::Builtin)
                .ok_or_else(|| MeshPathError::UnknownBuiltin(name.to_owned())),
            None => AssetPath::new(text)
                .map(Self::File)
                .map_err(|e| MeshPathError::Path(e.to_string())),
        }
    }
}

impl fmt::Display for MeshPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builtin(mesh) => write!(f, "{BUILTIN_PREFIX}{}", mesh.name()),
            Self::File(path) => write!(f, "{}", path.as_str()),
        }
    }
}

impl From<BuiltinMesh> for MeshPath {
    fn from(mesh: BuiltinMesh) -> Self {
        Self::Builtin(mesh)
    }
}

impl Serialize for MeshPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for MeshPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mesh_path_round_trips_as_one_string() {
        for text in ["builtin:cube", "builtin:capsule", "models/crate.glb"] {
            let path = MeshPath::parse(text).expect("valid");
            assert_eq!(path.to_string(), text);
            let ron = ron::to_string(&path).expect("serialises");
            assert_eq!(ron, format!("\"{text}\""));
            assert_eq!(ron::from_str::<MeshPath>(&ron).expect("parses"), path);
        }
    }

    #[test]
    fn a_misspelt_builtin_and_an_escaping_path_are_refused() {
        let error = MeshPath::parse("builtin:cub").expect_err("no such mesh");
        assert!(error.to_string().contains("cube, sphere"), "{error}");
        assert!(MeshPath::parse("../secret.glb").is_err());
        assert!(MeshPath::parse("C:/model.glb").is_err());
        assert!(ron::from_str::<MeshPath>("\"builtin:torus\"").is_err());
    }
}
