//! An atlas read from its file, or started for a sheet, and written back.

use std::path::{Path, PathBuf};

use voltra_assets::atlas::is_atlas;
use voltra_assets::packer::sheet_path;
use voltra_assets::{AssetPath, Atlas, AtlasFile, FrameMeta};
use voltra_render::glam::UVec2;

use super::{AtlasDoc, Region};

impl AtlasDoc {
    /// Opens `path` under `root`: an `.atlas.ron`, or a sheet — whose atlas
    /// beside it (`hero.png`, `hero.atlas.ron`) is opened if there is one,
    /// and started empty if not. `sheet_size` answers a texture's size.
    pub fn open(
        root: &Path,
        path: &AssetPath,
        sheet_size: impl FnOnce(&AssetPath) -> Option<UVec2>,
    ) -> Result<Self, String> {
        let (atlas_path, texture, atlas) = if is_atlas(path.as_str()) {
            let atlas = read(&root.join(path.as_str()))?
                .ok_or_else(|| format!("{} does not exist", path.as_str()))?;
            let texture = match atlas.texture() {
                Some(texture) => texture.clone(),
                None => sibling(path, &sheet_path(Path::new(path.as_str())))?,
            };
            (path.clone(), texture, Some(atlas))
        } else {
            let atlas_path = sibling(path, &atlas_beside(Path::new(path.as_str())))?;
            let atlas = read(&root.join(atlas_path.as_str()))?;
            (atlas_path, path.clone(), atlas)
        };
        let sheet = sheet_size(&texture)
            .filter(|size| size.x > 0 && size.y > 0)
            .ok_or_else(|| format!("{} did not load as a sheet", texture.as_str()))?;
        let regions: Vec<Region> = atlas.map_or_else(Vec::new, |atlas| {
            (0..atlas.len() as u32)
                .map(|i| {
                    let meta = atlas.meta(i).cloned().unwrap_or_default();
                    Region {
                        name: meta.name.unwrap_or_default(),
                        frame: atlas.frame(i).unwrap_or_default(),
                        pivot: meta.pivot,
                        trim: meta.trim,
                    }
                })
                .collect()
        });
        Ok(Self {
            path: atlas_path,
            texture,
            sheet,
            committed: regions.clone(),
            saved: regions.clone(),
            regions,
            undo: Vec::new(),
            redo: Vec::new(),
            selected: None,
        })
    }

    /// The atlas as it would be saved.
    pub fn atlas(&self) -> Atlas {
        let mut atlas = Atlas::new(self.sheet, Vec::new()).with_texture(self.texture.clone());
        for region in &self.regions {
            atlas.push(
                region.frame,
                FrameMeta {
                    name: (!region.name.is_empty()).then(|| region.name.clone()),
                    trim: region.trim,
                    pivot: region.pivot,
                },
            );
        }
        atlas
    }

    /// Writes the atlas to its file under `root`, or to `to`, which becomes
    /// its file. Through a temporary file, so the engine's watch never reads
    /// a half-written one.
    pub fn save(&mut self, root: &Path, to: Option<AssetPath>) -> Result<PathBuf, String> {
        if self.regions.is_empty() {
            return Err("an atlas with no regions would not load; slice or draw one first".into());
        }
        let path = to.unwrap_or_else(|| self.path.clone());
        if !is_atlas(path.as_str()) {
            return Err(format!("{} does not end in .atlas.ron", path.as_str()));
        }
        // Typed in place, a name can collide; the file must not say two
        // regions are one.
        for (index, region) in self.regions.iter().enumerate() {
            let first = self.regions.iter().position(|r| r.name == region.name);
            if !region.name.is_empty() && first != Some(index) {
                return Err(format!(
                    "regions {} and {index} are both called {:?}",
                    first.unwrap_or(index),
                    region.name
                ));
            }
        }
        let text = AtlasFile::from_atlas(&self.atlas())
            .to_ron()
            .map_err(|e| e.to_string())?;
        let file = root.join(path.as_str());
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let temporary = file.with_extension("ron.tmp");
        std::fs::write(&temporary, text).map_err(|e| format!("{}: {e}", temporary.display()))?;
        std::fs::rename(&temporary, &file).map_err(|e| format!("{}: {e}", file.display()))?;
        self.settle(true);
        self.saved = self.regions.clone();
        self.path = path;
        Ok(file)
    }
}

/// An atlas file, if there is one at `file`.
fn read(file: &Path) -> Result<Option<Atlas>, String> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", file.display())),
    };
    let parsed: AtlasFile = ron::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    parsed
        .into_atlas()
        .map(Some)
        .map_err(|e| format!("{}: {e}", file.display()))
}

/// `hero.png` → `hero.atlas.ron`, beside it.
fn atlas_beside(texture: &Path) -> PathBuf {
    let stem = texture
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    texture.with_file_name(format!("{stem}.atlas.ron"))
}

/// `relative`, a path made from `of`'s, as an asset path again.
fn sibling(of: &AssetPath, relative: &Path) -> Result<AssetPath, String> {
    let text = relative.to_string_lossy().replace('\\', "/");
    AssetPath::new(&text).map_err(|e| format!("{}: {e}", of.as_str()))
}

/// The sheet's name without folder or extension: what grid cells are named
/// after.
pub(super) fn stem(texture: &AssetPath) -> String {
    Path::new(texture.as_str())
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "cell".to_owned())
}
