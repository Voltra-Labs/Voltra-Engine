//! The texture packer, run from the editor: a folder of the project packed
//! into a sheet and its `.atlas.ron` beside it, and both picked up at once.
//!
//! The same `voltra_assets::packer` that `cargo xtask pack-atlas` runs, with
//! its defaults, so a sheet packed from either is the same sheet.

use voltra_assets::packer::{default_atlas_path, pack_folder, PackOptions, PackReport};
use voltra_assets::AssetPath;
use voltra_core::UiFrame;

/// Packs the folder at `folder`, under the asset root, into
/// `<folder>.atlas.ron` and `<folder>.png` beside it — trimmed if `trim` —
/// then reloads both in this session, so what already draws them updates
/// without waiting for the file watcher. Answers the atlas's asset path.
pub(crate) fn pack(
    frame: &mut UiFrame<'_>,
    folder: &AssetPath,
    trim: bool,
) -> Result<(AssetPath, PackReport), String> {
    let root = frame.textures.root().to_path_buf();
    let dir = root.join(folder.as_str());
    if !dir.is_dir() {
        return Err(format!("{} is not a folder", folder.as_str()));
    }
    let options = PackOptions {
        trim,
        ..PackOptions::default()
    };
    let atlas_file = default_atlas_path(&dir);
    let report =
        pack_folder(&dir, &atlas_file, Some(&root), &options).map_err(|e| e.to_string())?;

    let atlas =
        AssetPath::new(&format!("{}.atlas.ron", folder.as_str())).map_err(|e| e.to_string())?;
    frame.atlases.reload(&atlas);
    if let Some(sheet) = &report.texture {
        frame.textures.reload(frame.device, frame.queue, sheet);
    }
    log::info!(
        "packed {} into {} ({} regions, {}×{})",
        folder.as_str(),
        atlas.as_str(),
        report.regions,
        report.size.x,
        report.size.y
    );
    Ok((atlas, report))
}
