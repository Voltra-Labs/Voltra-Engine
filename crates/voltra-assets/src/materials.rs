//! Materials, keyed by the path a scene file names them with.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::AssetError;
use crate::handle::Handle;
use crate::material::{Material, MaterialFile};
use crate::path::AssetPath;
use crate::store::Assets;

/// Loads materials from an asset root and hands out shared handles, the
/// same shape as [`Atlases`](crate::Atlases): once per path, a stable
/// handle, reloaded in place — when the material file changes, or the
/// shader file it names.
///
/// Infallible: a material that will not read or compile is still a
/// handle, whose [`Material::error`] says why and whose sprites draw
/// plainly. The error is logged when it is found, not every frame.
#[derive(Debug)]
pub struct Materials {
    root: PathBuf,
    store: Assets<Material>,
    by_path: HashMap<AssetPath, Handle<Material>>,
    revision: u64,
}

impl Materials {
    /// An empty set that resolves paths against `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            store: Assets::new(),
            by_path: HashMap::new(),
            revision: 0,
        }
    }

    /// The handle for `path`, reading and compiling it the first time.
    pub fn load(&mut self, path: &AssetPath) -> Handle<Material> {
        if let Some(handle) = self.by_path.get(path) {
            return *handle;
        }
        let material = self.read(path);
        let handle = self.store.insert(material);
        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// Reads every loaded material that `path` is — or whose shader `path`
    /// is — again. Answers whether one was.
    pub fn reload(&mut self, path: &AssetPath) -> bool {
        let affected: Vec<(AssetPath, Handle<Material>)> = self
            .by_path
            .iter()
            .filter(|(material, handle)| {
                *material == path
                    || self
                        .store
                        .get(**handle)
                        .is_some_and(|m| m.shader.as_ref() == Some(path))
            })
            .map(|(p, h)| (p.clone(), *h))
            .collect();
        for (material_path, handle) in &affected {
            let material = self.read(material_path);
            if let Some(slot) = self.store.get_mut(*handle) {
                *slot = material;
            }
            log::info!("reloaded {}", material_path.as_str());
        }
        !affected.is_empty()
    }

    fn read(&mut self, path: &AssetPath) -> Material {
        self.revision += 1;
        let mut material = match self.read_file(path) {
            Ok((file, shader, fragment)) => Material::build(&file, shader, &fragment),
            Err(e) => Material {
                error: Some(e.to_string()),
                ..Material::default()
            },
        };
        material.revision = self.revision;
        if let Some(error) = &material.error {
            log::warn!(
                "material {} cannot be drawn with; its sprites draw plainly:\n{error}",
                path.as_str()
            );
        }
        material
    }

    fn read_file(&self, path: &AssetPath) -> Result<(MaterialFile, AssetPath, String), AssetError> {
        let full = self.root.join(path.as_str());
        let text = crate::fs::read_to_string(&full).map_err(|source| AssetError::Read {
            path: full.clone(),
            source,
        })?;
        let file: MaterialFile = ron::from_str(&text).map_err(|source| AssetError::Parse {
            path: full.clone(),
            source: Box::new(source),
        })?;
        let shader_path = AssetPath::new(&file.shader)?;
        let shader = self.root.join(shader_path.as_str());
        let fragment = crate::fs::read_to_string(&shader).map_err(|source| AssetError::Read {
            path: shader,
            source,
        })?;
        Ok((file, shader_path, fragment))
    }

    /// The material `handle` addresses, if it is current.
    pub fn get(&self, handle: Handle<Material>) -> Option<&Material> {
        self.store.get(handle)
    }

    /// The handle of the material loaded from `path`, if there is one.
    pub fn by_path_handle(&self, path: &AssetPath) -> Option<Handle<Material>> {
        self.by_path.get(path).copied()
    }

    /// The directory every asset path is resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How many materials are stored.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Whether no material is stored.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

#[cfg(test)]
mod tests;
