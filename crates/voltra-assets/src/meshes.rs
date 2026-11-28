//! 3D meshes under handles: the built-in primitives, and a magenta cube
//! for a mesh that will not load.
//!
//! Generated on the CPU when the store is made and uploaded to the GPU by
//! [`Meshes::upload`], so a 2D game that never draws 3D uploads nothing,
//! and culling reads a mesh's bounds without a device. Mesh files arrive
//! with glTF (stage 143): until then a [`MeshPath::File`] has nothing to
//! read it, draws the magenta cube and is warned about once.

pub mod builtin;
pub mod mesh_path;

use std::collections::{HashMap, HashSet};

use voltra_render::render3d::{Aabb3, Mesh3D, MeshData3D};
use voltra_render::wgpu;

pub use builtin::BuiltinMesh;
pub use mesh_path::{MeshPath, MeshPathError, BUILTIN_PREFIX};

use crate::handle::Handle;
use crate::path::AssetPath;
use crate::store::Assets;

/// The tint a mesh that will not load is drawn in, linear: the asset rule
/// "a bad name draws magenta", as a texture's does.
pub const MISSING_TINT: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

/// What [`Meshes::resolve`] answers: the mesh to draw, and whether it is
/// the stand-in for one that will not load (draw it [`MISSING_TINT`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedMesh {
    /// The mesh to draw.
    pub handle: Handle<Mesh3D>,
    /// Whether it stands in for a mesh that will not load.
    pub missing: bool,
}

/// One stored mesh: its triangles, its bounds, and its GPU buffers once
/// uploaded.
#[derive(Debug)]
struct Entry {
    data: MeshData3D,
    bounds: Aabb3,
    gpu: Option<Mesh3D>,
    label: String,
}

/// Meshes by handle: every [`BuiltinMesh`], made once.
#[derive(Debug)]
pub struct Meshes {
    store: Assets<Entry>,
    builtins: HashMap<BuiltinMesh, Handle<Entry>>,
    /// Paths already warned about, so a missing mesh is said once.
    warned: HashSet<AssetPath>,
}

impl Default for Meshes {
    fn default() -> Self {
        Self::new()
    }
}

impl Meshes {
    /// A store holding every built-in mesh, on the CPU.
    pub fn new() -> Self {
        let mut store = Assets::new();
        let builtins = BuiltinMesh::ALL
            .into_iter()
            .map(|mesh| {
                let data = mesh.generate();
                let bounds = data.bounds().expect("a built-in mesh has vertices");
                let label = format!("{BUILTIN_PREFIX}{}", mesh.name());
                let entry = Entry {
                    data,
                    bounds,
                    gpu: None,
                    label,
                };
                (mesh, store.insert(entry))
            })
            .collect();
        Self {
            store,
            builtins,
            warned: HashSet::new(),
        }
    }

    /// The mesh `path` names, or the magenta stand-in — warned about the
    /// first time a path is asked for.
    pub fn resolve(&mut self, path: &MeshPath) -> ResolvedMesh {
        match path {
            MeshPath::Builtin(mesh) => ResolvedMesh {
                handle: self.builtin(*mesh),
                missing: false,
            },
            MeshPath::File(file) => {
                if self.warned.insert(file.clone()) {
                    log::warn!(
                        "mesh {} cannot load: no mesh file format is read yet (glTF is planned); \
                         drawing a magenta cube",
                        file.as_str()
                    );
                }
                ResolvedMesh {
                    handle: self.builtin(BuiltinMesh::Cube),
                    missing: true,
                }
            }
        }
    }

    /// The handle of a built-in mesh.
    pub fn builtin(&self, mesh: BuiltinMesh) -> Handle<Mesh3D> {
        let handle = self
            .builtins
            .get(&mesh)
            .copied()
            .expect("every built-in mesh is made with the store");
        typed(handle)
    }

    /// The box around `handle`'s mesh in its own space, if it is current.
    pub fn bounds(&self, handle: Handle<Mesh3D>) -> Option<Aabb3> {
        self.entry(handle).map(|entry| entry.bounds)
    }

    /// `handle`'s triangles, if it is current.
    pub fn data(&self, handle: Handle<Mesh3D>) -> Option<&MeshData3D> {
        self.entry(handle).map(|entry| &entry.data)
    }

    /// Uploads every mesh not on the GPU yet. Cheap when all are.
    pub fn upload(&mut self, device: &wgpu::Device) {
        let handles: Vec<Handle<Entry>> = self.builtins.values().copied().collect();
        for handle in handles {
            let Some(entry) = self.store.get_mut(handle) else {
                continue;
            };
            if entry.gpu.is_some() {
                continue;
            }
            match Mesh3D::new(device, &entry.label, &entry.data) {
                Ok(mesh) => entry.gpu = Some(mesh),
                Err(e) => log::error!("mesh {} cannot be drawn: {e}", entry.label),
            }
        }
    }

    /// `handle`'s GPU buffers, once [`upload`](Self::upload)ed.
    pub fn gpu(&self, handle: Handle<Mesh3D>) -> Option<&Mesh3D> {
        self.entry(handle).and_then(|entry| entry.gpu.as_ref())
    }

    fn entry(&self, handle: Handle<Mesh3D>) -> Option<&Entry> {
        self.store
            .get(Handle::new(handle.index(), handle.generation()))
    }
}

/// The public handle of a stored entry: the same slot, typed by what it
/// draws as.
fn typed(handle: Handle<Entry>) -> Handle<Mesh3D> {
    Handle::new(handle.index(), handle.generation())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_builtin_resolves_to_its_mesh_and_a_file_to_the_magenta_cube() {
        let mut meshes = Meshes::new();
        let sphere = meshes.resolve(&MeshPath::Builtin(BuiltinMesh::Sphere));
        assert!(!sphere.missing);
        assert_eq!(sphere.handle, meshes.builtin(BuiltinMesh::Sphere));
        let file = MeshPath::File(AssetPath::new("models/crate.glb").expect("a path"));
        let missing = meshes.resolve(&file);
        assert!(missing.missing);
        assert_eq!(missing.handle, meshes.builtin(BuiltinMesh::Cube));
        // Said once, however often asked.
        meshes.resolve(&file);
        assert_eq!(meshes.warned.len(), 1);
        let bounds = meshes.bounds(sphere.handle).expect("current");
        assert!((bounds.max.y - 0.5).abs() < 1e-5);
        assert!(meshes.gpu(sphere.handle).is_none(), "not uploaded yet");
    }

    #[test]
    fn the_builtins_upload_once() {
        let Some((device, _queue)) = voltra_testkit::headless_device() else {
            return;
        };
        let mut meshes = Meshes::new();
        meshes.upload(&device);
        for mesh in BuiltinMesh::ALL {
            let gpu = meshes.gpu(meshes.builtin(mesh)).expect("uploaded");
            assert_eq!(gpu.index_count() as usize, mesh.generate().indices.len());
        }
        meshes.upload(&device);
    }
}
