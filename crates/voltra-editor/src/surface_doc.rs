//! The surface material file the inspector has open: its `standard`
//! block, edited in place and written back when an edit settles.
//!
//! **The file, not the entity.** A surface material is shared: every mesh
//! naming it draws the edit, as Unity's material inspector and Godot's
//! `StandardMaterial3D` resource do. So the edit is not the scene's — it is
//! not in the scene's history, and it does not mark the scene unsaved —
//! but written to the file when the pointer is let go and no popup is
//! open, then the material reloaded so every mesh draws it that frame. The
//! live preview draws the draft every frame, a drag included. Switching to
//! another file writes this one's draft first: the switch ends the edit.
//!
//! The file's leading comments are kept over a write, as `cargo xtask
//! migrate` keeps them (`voltra_assets::leading_comments`), and a number
//! that is not finite reads as its default, so a NaN cannot make the draft
//! differ from the file on every frame.

use std::collections::BTreeMap;
use std::path::Path;

use voltra_assets::{
    leading_comments, AssetPath, CustomTextureFile, MaterialDomain, MaterialFile, Materials,
    StandardFile,
};
use voltra_render::material::Uniform;
use voltra_render::render3d::SurfaceLighting;

mod draft;

pub use draft::{DraftShader, Drawn};

/// An open surface material.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceDoc {
    path: AssetPath,
    /// As edited.
    file: MaterialFile,
    /// As last read or written.
    saved: MaterialFile,
    /// The comments the file opened with, written back in front of it.
    header: String,
    /// Why the last write failed, and the draft it failed for: not tried
    /// again until the draft changes.
    failed: Option<(MaterialFile, String)>,
    /// The store's revision of the material when the file was last read
    /// or written: a different one, with nothing unsaved, means the file
    /// changed on disk and is read again.
    pub seen: Option<u64>,
    /// The draft's own shader, composed and checked for the preview.
    pub draft: DraftShader,
}

impl SurfaceDoc {
    /// Reads the material at `path` under `root`.
    pub fn open(root: &Path, path: &AssetPath) -> Result<Self, String> {
        let full = root.join(path.as_str());
        let text =
            std::fs::read_to_string(&full).map_err(|e| format!("{}: {e}", full.display()))?;
        let mut file: MaterialFile =
            ron::from_str(&text).map_err(|e| format!("{}: {e}", full.display()))?;
        file.standard = file.standard.finite().0;
        for value in file.uniforms.values_mut() {
            *value = value.finite();
        }
        Ok(Self {
            path: path.clone(),
            saved: file.clone(),
            file,
            header: leading_comments(&text).to_owned(),
            failed: None,
            seen: None,
            draft: DraftShader::default(),
        })
    }

    /// Writes a new surface material with the default standard block at
    /// `path` under `root`, and opens it. Refuses to overwrite a file.
    pub fn create(root: &Path, path: &AssetPath) -> Result<Self, String> {
        let full = root.join(path.as_str());
        if full.exists() {
            return Err(format!("{} already exists", path.as_str()));
        }
        if let Some(folder) = full.parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        let file = MaterialFile::surface(StandardFile::default());
        write(&full, "", &file)?;
        Self::open(root, path)
    }

    /// The file it edits.
    pub fn path(&self) -> &AssetPath {
        &self.path
    }

    /// What it draws on: a sprite material is shown, not edited, here.
    pub fn domain(&self) -> MaterialDomain {
        self.file.domain
    }

    /// The standard block, to edit.
    pub fn standard_mut(&mut self) -> &mut StandardFile {
        &mut self.file.standard
    }

    /// The shader file, as edited: empty for the standard block alone.
    pub fn shader_mut(&mut self) -> &mut String {
        &mut self.file.shader
    }

    /// Whether a custom surface is lit, to edit.
    pub fn lighting_mut(&mut self) -> &mut SurfaceLighting {
        &mut self.file.lighting
    }

    /// A custom surface's uniforms, to edit.
    pub fn uniforms_mut(&mut self) -> &mut BTreeMap<String, Uniform> {
        &mut self.file.uniforms
    }

    /// A custom surface's own textures, to edit.
    pub fn textures_mut(&mut self) -> &mut BTreeMap<String, CustomTextureFile> {
        &mut self.file.textures
    }

    /// What the draft draws for the preview, its shader read under `root`
    /// again when the store's `revision` of the material changes: see
    /// [`DraftShader::draw`].
    pub fn drawn(&mut self, root: &Path, revision: Option<u64>) -> Drawn {
        self.draft.draw(root, &self.file, revision)
    }

    /// Whether it differs from the file.
    pub fn is_unsaved(&self) -> bool {
        self.file != self.saved
    }

    /// Why the last write failed, while the draft is the one it failed for.
    pub fn error(&self) -> Option<&str> {
        self.failed
            .as_ref()
            .filter(|(draft, _)| *draft == self.file)
            .map(|(_, why)| why.as_str())
    }

    /// Writes the edit to the file unless `busy` (a drag, an open popup) or
    /// nothing changed, then reloads it in `materials`. Answers whether it
    /// wrote. A failed write is kept as [`error`](Self::error) and not tried
    /// again until the draft changes.
    pub fn settle(&mut self, root: &Path, materials: Option<&mut Materials>, busy: bool) -> bool {
        if busy || !self.is_unsaved() || self.error().is_some() {
            return false;
        }
        let full = root.join(self.path.as_str());
        if let Err(e) = write(&full, &self.header, &self.file) {
            log::error!("saving {}: {e}", self.path.as_str());
            self.failed = Some((self.file.clone(), e));
            return false;
        }
        self.failed = None;
        self.saved = self.file.clone();
        if let Some(materials) = materials {
            materials.reload(&self.path);
        }
        true
    }

    /// Makes `slot` hold the file at `path`: the open draft, if it is that
    /// file; otherwise the draft of the file open before is written — the
    /// switch ends its edit, a drag or not — and `path` read.
    pub fn switch_to(
        slot: &mut Option<Self>,
        root: &Path,
        path: &AssetPath,
        materials: Option<&mut Materials>,
    ) -> Result<(), String> {
        if slot.as_ref().is_some_and(|doc| doc.path() == path) {
            return Ok(());
        }
        if let Some(old) = slot.as_mut() {
            old.settle(root, materials, false);
        }
        *slot = None;
        *slot = Some(Self::open(root, path)?);
        Ok(())
    }

    /// Reads the file again over the draft: it changed on disk.
    pub fn reread(&mut self, root: &Path) -> Result<(), String> {
        let seen = self.seen;
        let draft = std::mem::take(&mut self.draft);
        *self = Self::open(root, &self.path)?;
        self.seen = seen;
        self.draft = draft;
        Ok(())
    }
}

fn write(full: &Path, header: &str, file: &MaterialFile) -> Result<(), String> {
    let text = file.to_ron().map_err(|e| e.to_string())?;
    std::fs::write(full, format!("{header}{text}")).map_err(|e| format!("{}: {e}", full.display()))
}

#[cfg(test)]
mod tests {
    use voltra_testkit::scratch_root;

    use super::*;

    #[test]
    fn an_edit_is_written_once_settled_and_reloaded() {
        let root = scratch_root();
        let path = AssetPath::new("materials/new.material.ron").expect("a path");
        let mut doc = SurfaceDoc::create(&root, &path).expect("created");
        assert!(
            SurfaceDoc::create(&root, &path).is_err(),
            "never overwrites"
        );
        assert_eq!(doc.domain(), MaterialDomain::Surface);
        let mut materials = Materials::new(root.path());
        let handle = materials.load(&path);
        doc.standard_mut().roughness = 0.9;
        assert!(doc.is_unsaved());
        assert!(
            !doc.settle(&root, Some(&mut materials), true),
            "not mid-drag"
        );
        assert!(doc.settle(&root, Some(&mut materials), false));
        assert!(!doc.is_unsaved());
        let reloaded = materials
            .get(handle)
            .and_then(|m| m.surface())
            .expect("a surface");
        assert_eq!(reloaded.standard.roughness, 0.9, "every mesh draws it now");
        let mut again = SurfaceDoc::open(&root, &path).expect("read back");
        assert_eq!(again.standard_mut().roughness, 0.9);
        assert!(
            !doc.settle(&root, Some(&mut materials), false),
            "nothing new"
        );
    }

    #[test]
    fn a_custom_surface_s_fields_are_written_and_drawn_by_every_mesh() {
        let root = scratch_root();
        let path = AssetPath::new("materials/own.material.ron").expect("a path");
        let mut doc = SurfaceDoc::create(&root, &path).expect("created");
        std::fs::write(
            root.join("materials/own.wgsl"),
            "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    s.roughness = material.rough;\n    return s;\n}\n",
        )
        .expect("written");
        let mut materials = Materials::new(root.path());
        let handle = materials.load(&path);
        *doc.shader_mut() = "materials/own.wgsl".to_owned();
        doc.uniforms_mut()
            .insert("rough".to_owned(), Uniform::Float(0.75));
        *doc.lighting_mut() = SurfaceLighting::Unlit;
        let drawn = doc.drawn(&root, None);
        assert_eq!(drawn.error, None, "the draft compiles for the preview");
        assert!(doc.settle(&root, Some(&mut materials), false));
        let material = materials.get(handle).expect("loaded");
        assert_eq!(material.error, None);
        let custom = material
            .surface()
            .and_then(|s| s.custom.as_ref())
            .expect("its own shading");
        assert_eq!(custom.uniforms.get("rough"), Some(&Uniform::Float(0.75)));
        assert_eq!(custom.lighting, SurfaceLighting::Unlit);
        // A name the shader does not have: the preview says why, the
        // standard block drawn.
        doc.uniforms_mut().clear();
        let drawn = doc.drawn(&root, None);
        assert!(drawn.error.is_some_and(|e| e.contains("rough")));
        assert!(drawn.source.expect("its standard block").custom.is_none());
    }
}
