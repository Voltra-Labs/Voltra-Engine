//! A folder on disk packed, and the sheet and atlas written beside it.

use std::path::{Path, PathBuf};

use voltra_render::glam::UVec2;

use super::{pack, PackError, PackOptions, Picture};
use crate::atlas::{is_atlas, Atlas, AtlasFile, ATLAS_SUFFIX};
use crate::path::AssetPath;

/// What [`pack_folder`] wrote.
#[derive(Debug, Clone)]
pub struct PackReport {
    /// The sheet's PNG.
    pub sheet: PathBuf,
    /// The `.atlas.ron`.
    pub atlas: PathBuf,
    /// The sheet's size, in texels.
    pub size: UVec2,
    /// How many regions it holds.
    pub regions: usize,
    /// Regions kept empty because their file is gone; see [`Packed::gone`].
    pub gone: Vec<String>,
    /// The texture hint written into the atlas, if the sheet is under the
    /// asset root.
    pub texture: Option<AssetPath>,
}

/// Every PNG under `folder`, at any depth, named as [`Picture::name`]
/// says, in name order. `skip` is left out — the sheet a previous pack wrote
/// into the folder itself, which is not one of its pictures.
pub fn read_folder(folder: &Path, skip: &Path) -> Result<Vec<Picture>, PackError> {
    let mut found = Vec::new();
    collect(folder, folder, skip, &mut found)?;
    if found.is_empty() {
        return Err(PackError::Empty(folder.to_path_buf()));
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

fn collect(
    root: &Path,
    dir: &Path,
    skip: &Path,
    found: &mut Vec<Picture>,
) -> Result<(), PackError> {
    let entries = std::fs::read_dir(dir).map_err(|e| PackError::Io(dir.to_path_buf(), e))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect(root, &path, skip, found)?;
            continue;
        }
        let is_png = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
        if !is_png || same_file(&path, skip) {
            continue;
        }
        let image = image::open(&path)
            .map_err(|e| PackError::Image(path.clone(), e))?
            .to_rgba8();
        let relative = path.strip_prefix(root).unwrap_or(&path).with_extension("");
        let name = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        found.push(Picture { name, image });
    }
    Ok(())
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// The sheet an atlas at `atlas` is packed into: the same name, `.png`
/// for [`ATLAS_SUFFIX`] — `hero.atlas.ron` beside `hero.png`.
pub fn sheet_path(atlas: &Path) -> PathBuf {
    let name = atlas
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = &name[..name.len() - ATLAS_SUFFIX.len().min(name.len())];
    atlas.with_file_name(format!("{stem}.png"))
}

/// Where a pack of `folder` goes by default: beside it, named after it —
/// `sprites/hero/` packs to `sprites/hero.atlas.ron` and `sprites/hero.png`.
pub fn default_atlas_path(folder: &Path) -> PathBuf {
    let name = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "atlas".to_owned());
    folder.with_file_name(format!("{name}{ATLAS_SUFFIX}"))
}

/// Packs every PNG under `folder` into the sheet beside `atlas` and writes
/// both. The atlas already at `atlas`, if any, keeps its regions' indices
/// and pivots. `root`, the asset root, turns the sheet's path into the
/// atlas's texture hint; a sheet outside it gets none.
pub fn pack_folder(
    folder: &Path,
    atlas: &Path,
    root: Option<&Path>,
    options: &PackOptions,
) -> Result<PackReport, PackError> {
    let name = atlas.file_name().map(|n| n.to_string_lossy().into_owned());
    if !name.as_deref().is_some_and(is_atlas) {
        return Err(PackError::NotAnAtlas(atlas.to_path_buf()));
    }
    let sheet_file = sheet_path(atlas);
    let previous = read_previous(atlas)?;
    let pictures = read_folder(folder, &sheet_file)?;
    let mut packed = pack(pictures, previous.as_ref(), options)?;

    let texture = root.and_then(|root| hint(root, &sheet_file));
    if let Some(texture) = &texture {
        packed.atlas = packed.atlas.with_texture(texture.clone());
    }
    for gone in &packed.gone {
        log::warn!("region {gone:?} has no file any more; its index is kept, empty");
    }

    if let Some(parent) = atlas.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| PackError::Io(parent.to_path_buf(), e))?;
    }
    packed
        .sheet
        .save_with_format(&sheet_file, image::ImageFormat::Png)
        .map_err(|e| PackError::Image(sheet_file.clone(), e))?;
    let text = AtlasFile::from_atlas(&packed.atlas)
        .to_ron()
        .map_err(|e| PackError::Io(atlas.to_path_buf(), std::io::Error::other(e)))?;
    let text = format!(
        "// Packed from {} by the texture packer (`cargo xtask pack-atlas`).\n\
         // Re-pack rather than edit: a region keeps its index by its name.\n{text}",
        folder
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    );
    std::fs::write(atlas, text).map_err(|e| PackError::Io(atlas.to_path_buf(), e))?;

    Ok(PackReport {
        sheet: sheet_file,
        atlas: atlas.to_path_buf(),
        size: packed.atlas.sheet(),
        regions: packed.atlas.len(),
        gone: packed.gone,
        texture,
    })
}

/// The atlas already at `path`, if there is one. One that will not read is
/// an error, not a fresh start: its names are what keep scenes pointing at
/// the right regions, and guessing past it would renumber them.
fn read_previous(path: &Path) -> Result<Option<Atlas>, PackError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(PackError::Io(path.to_path_buf(), e)),
    };
    let file: AtlasFile =
        ron::from_str(&text).map_err(|e| PackError::Previous(path.to_path_buf(), e.to_string()))?;
    file.into_atlas()
        .map(Some)
        .map_err(|e| PackError::Previous(path.to_path_buf(), e.to_string()))
}

/// `sheet` as an asset path under `root`, if it is under it.
fn hint(root: &Path, sheet: &Path) -> Option<AssetPath> {
    let root = root.canonicalize().ok()?;
    let dir = sheet.parent()?.canonicalize().ok()?;
    let relative = dir.strip_prefix(&root).ok()?.join(sheet.file_name()?);
    let text = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    AssetPath::new(&text).ok()
}
