//! Materials, keyed by the path a scene file names them with.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::AssetError;
use crate::handle::Handle;
use crate::material::{Material, MaterialDomain, MaterialFile};
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
            Ok(file) if file.domain == MaterialDomain::Surface => {
                let shader = (!file.shader.trim().is_empty())
                    .then(|| self.read_surface_shader(&file))
                    .flatten();
                Material::build_surface(&file, shader)
            }
            Ok(file) => match self.read_shader(&file) {
                Ok((shader, fragment)) => Material::build(&file, shader, &fragment),
                Err(e) => Material {
                    error: Some(e.to_string()),
                    ..Material::default()
                },
            },
            Err(e) => Material {
                error: Some(e.to_string()),
                ..Material::default()
            },
        };
        material.revision = self.revision;
        for warning in &material.warnings {
            log::warn!("material {}: {warning}", path.as_str());
        }
        if let Some(error) = &material.error {
            let drawn = match (material.domain, material.surface.is_some()) {
                (MaterialDomain::Sprite, _) => "its sprites draw plainly",
                (MaterialDomain::Surface, true) => "its meshes draw its standard block",
                (MaterialDomain::Surface, false) => "its meshes draw as the default white standard",
            };
            log::warn!(
                "material {} cannot be drawn with; {drawn}:\n{error}",
                path.as_str()
            );
        }
        material
    }

    fn read_file(&self, path: &AssetPath) -> Result<MaterialFile, AssetError> {
        read_material_file(&self.root, path)
    }

    /// A sprite material's shader: its path and its text.
    fn read_shader(&self, file: &MaterialFile) -> Result<(AssetPath, String), AssetError> {
        let shader_path = AssetPath::new(&file.shader)?;
        let shader = self.root.join(shader_path.as_str());
        let fragment = crate::fs::read_to_string(&shader).map_err(|source| AssetError::Read {
            path: shader,
            source,
        })?;
        Ok((shader_path, fragment))
    }

    /// A surface material's shader: its path — kept even when the file
    /// will not read, so writing it reloads the material — and its text or
    /// why not. `None` when the name is no asset path.
    fn read_surface_shader(
        &self,
        file: &MaterialFile,
    ) -> Option<(AssetPath, Result<String, String>)> {
        let path = AssetPath::new(&file.shader).ok()?;
        let full = self.root.join(path.as_str());
        let text = crate::fs::read_to_string(&full)
            .map_err(|source| AssetError::Read { path: full, source }.to_string());
        Some((path, text))
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

/// The material file at `path` under `root`, read and parsed but not
/// checked: what a scene's loader reads on a worker to find the images a
/// surface names before its meshes spawn.
pub fn read_material_file(root: &Path, path: &AssetPath) -> Result<MaterialFile, AssetError> {
    let full = root.join(path.as_str());
    let text = crate::fs::read_to_string(&full).map_err(|source| AssetError::Read {
        path: full.clone(),
        source,
    })?;
    ron::from_str(&text).map_err(|source| AssetError::Parse {
        path: full,
        source: Box::new(source),
    })
}

#[cfg(test)]
mod custom_tests;
#[cfg(test)]
mod tests;
