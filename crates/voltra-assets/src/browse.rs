//! Reading the asset root as a list of things an editor can show.
//!
//! One concept, and deliberately not two: this knows about directories, names
//! and extensions, and nothing about textures, handles or the GPU. A browser
//! panel joins this to [`Textures`](crate::textures::Textures) the same way
//! `voltra_core::App` joins [`AssetWatcher`](crate::watch::AssetWatcher) to it.
//!
//! Every path handed out is an [`AssetPath`], so a name the invariant refuses —
//! a Windows alternate data stream, a byte sequence that is not UTF-8 — is
//! dropped here rather than becoming a string some caller later has to check.

use std::path::Path;

mod find;
pub use find::find;

use crate::atlas::is_atlas;
use crate::error::AssetError;
use crate::material::is_material;
use crate::path::AssetPath;
use crate::watch::{
    is_animation, is_animator, is_bus_layout, is_clip, is_font, is_input_map, is_render_settings,
    is_scene, is_sorting_layers, is_texture,
};

/// What one row of a listing is.
///
/// `Other` is listed rather than hidden: a browser that silently omits the
/// `.psd` an artist just copied in reads as a failed copy. It says "this is
/// here and this engine cannot draw it", which is Unity's answer too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A directory, which can be opened.
    Directory,
    /// A file with an extension in
    /// [`TEXTURE_EXTENSIONS`](crate::watch::TEXTURE_EXTENSIONS).
    Texture,
    /// A file named `*.atlas.ron`: a slicing of one of those.
    Atlas,
    /// A file with an extension in
    /// [`CLIP_EXTENSIONS`](crate::watch::CLIP_EXTENSIONS): a sound, or a
    /// clip's settings (`*.clip.ron`) naming one.
    Clip,
    /// A file with an extension in
    /// [`FONT_EXTENSIONS`](crate::watch::FONT_EXTENSIONS): a face a `Text`
    /// can draw with.
    Font,
    /// A file [`is_input_map`](crate::watch::is_input_map) accepts: a game's
    /// action bindings. Listed, not placed — there is nothing to drop.
    InputMap,
    /// A file [`is_sorting_layers`](crate::watch::is_sorting_layers)
    /// accepts: the project's draw-order bands. Listed, not placed.
    SortingLayers,
    /// A file [`is_bus_layout`](crate::watch::is_bus_layout) accepts: the
    /// project's audio buses and their effects. Listed, not placed.
    BusLayout,
    /// A file [`is_render_settings`](crate::watch::is_render_settings)
    /// accepts: the project's render settings. Listed, not placed.
    RenderSettings,
    /// A file [`is_animation`](crate::watch::is_animation) accepts: keyed
    /// property tracks an `AnimationPlayer` plays — dragged onto its field,
    /// not into the scene.
    Animation,
    /// A file [`is_animator`](crate::watch::is_animator) accepts: a state
    /// machine an `Animator` runs — dragged onto its field.
    Animator,
    /// A file [`is_material`](crate::material::is_material) accepts: a
    /// shader and its parameters, dragged onto a sprite's material field.
    Material,
    /// A file [`is_scene`](crate::watch::is_scene) accepts: a scene, which is
    /// also what a prefab is, so every one of them can be dropped in.
    Scene,
    /// A file this engine has no loader for.
    Other,
}

/// One directory or file under the asset root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The last component, which is what a row says.
    pub name: String,
    /// Where it is, relative to the asset root.
    pub path: AssetPath,
    /// What kind of entry it is.
    pub kind: EntryKind,
}

impl Entry {
    /// Whether this entry can be opened as a directory.
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Directory
    }
}

/// Lists `dir` under `root`. `None` lists the root itself.
///
/// Sorted directories first and then by name, case-insensitively — the order
/// Unity, Unreal and Godot all show, and the one a human scanning a column
/// expects. The sort is here rather than in the caller because a listing that
/// arrives in `read_dir` order is different on every filesystem, and a browser
/// whose rows move between platforms is a bug reported as a flicker.
///
/// Dotfiles are skipped: `.gitkeep`, `.DS_Store` and `.import` sidecars are
/// bookkeeping, not assets.
pub fn list(root: &Path, dir: Option<&AssetPath>) -> Result<Vec<Entry>, AssetError> {
    let base = match dir {
        Some(dir) => root.join(dir.as_str()),
        None => root.to_path_buf(),
    };

    let reader = std::fs::read_dir(&base).map_err(|source| AssetError::Read {
        path: base.clone(),
        source,
    })?;

    let mut entries = Vec::new();
    for entry in reader {
        let entry = match entry {
            Ok(entry) => entry,
            // One unreadable entry must not lose the rest of the directory:
            // a file being written while the scan runs is normal, not an error.
            Err(e) => {
                log::warn!("skipping an entry of {}: {e}", base.display());
                continue;
            }
        };

        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            log::warn!("skipping a non-UTF-8 name in {}", base.display());
            continue;
        };
        if name.starts_with('.') {
            continue;
        }

        // Built from the parent's already-validated path plus one component, so
        // the only way this fails is a name the invariant refuses.
        let raw = match dir {
            Some(dir) => format!("{}/{}", dir.as_str(), name),
            None => name.clone(),
        };
        let path = match AssetPath::new(&raw) {
            Ok(path) => path,
            Err(e) => {
                log::warn!("skipping {raw:?}: {e}");
                continue;
            }
        };

        // `file_type` rather than `metadata`: it does not follow symlinks, so a
        // link pointing outside the root is listed as the file it is here and
        // never walked into.
        let is_dir = match entry.file_type() {
            Ok(kind) => kind.is_dir(),
            Err(e) => {
                log::warn!("skipping {raw:?}: {e}");
                continue;
            }
        };

        let kind = if is_dir {
            EntryKind::Directory
        } else if is_texture(&name) {
            EntryKind::Texture
        } else if is_atlas(&name) {
            // Before `Other` and after `Texture`: the suffix is longer than an
            // extension, and only one of the two can match a given name.
            EntryKind::Atlas
        } else if is_clip(&name) {
            EntryKind::Clip
        } else if is_font(&name) {
            EntryKind::Font
        } else if is_input_map(&name) {
            EntryKind::InputMap
        } else if is_sorting_layers(&name) {
            EntryKind::SortingLayers
        } else if is_bus_layout(&name) {
            EntryKind::BusLayout
        } else if is_render_settings(&name) {
            EntryKind::RenderSettings
        } else if is_animation(&name) {
            EntryKind::Animation
        } else if is_animator(&name) {
            EntryKind::Animator
        } else if is_material(&name) {
            // Before scenes: `is_scene` takes any other `.ron`.
            EntryKind::Material
        } else if is_scene(&name) {
            // After `Atlas`, which is a `.ron` too; `is_scene` refuses one
            // anyway, and the order says so without having to read it.
            EntryKind::Scene
        } else {
            EntryKind::Other
        };

        entries.push(Entry { name, path, kind });
    }

    entries.sort_by(|a, b| {
        b.is_dir()
            .cmp(&a.is_dir())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// The directory holding `dir`. `None` is the asset root, which has no parent.
///
/// Here rather than in the panel because it is the inverse of the join `list`
/// performs, and the two have to agree about the separator.
pub fn parent(dir: &AssetPath) -> Option<AssetPath> {
    let (head, _) = dir.as_str().rsplit_once('/')?;
    // Cannot fail: `head` is a non-empty prefix of an already validated path.
    AssetPath::new(head).ok()
}

#[cfg(test)]
mod tests;
