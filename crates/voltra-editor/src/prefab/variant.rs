//! A variant of a prefab: a new file whose root is an instance of it, made
//! from the asset browser's menu on a prefab or from prefab mode's bar —
//! Unity's Create ▸ Prefab Variant. See `voltra_scene::prefab::variant`.

use voltra_assets::AssetPath;
use voltra_core::UiFrame;
use voltra_scene::prefab;

use super::free_path;
use crate::editor::Editor;

/// Writes a variant of `base` beside it, `<name> Variant.prefab.ron` —
/// numbered when taken, never an overwrite, as Create prefab names one —
/// and answers its path. Refused, with nothing written, for a base that
/// will not read, or one the new file would loop back through.
///
/// Of the file as saved: in prefab mode, changes not yet saved are not in
/// it, as a scene's instances only follow what is saved.
pub(crate) fn create_variant(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    base: &AssetPath,
) -> Result<AssetPath, String> {
    let folder = base
        .as_str()
        .rsplit_once('/')
        .map(|(dir, _)| AssetPath::new(dir))
        .transpose()
        .map_err(|e| e.to_string())?;
    let name = prefab::variant_name(base);
    let on_disk = frame.prefabs.root().to_path_buf();
    let target = free_path(folder.as_ref(), &name, |path| {
        on_disk.join(path.as_str()).exists()
    })
    .ok_or_else(|| format!("no free file name for a variant called {name:?}"))?;
    prefab::variant(&editor.registry, frame.prefabs, base, &target)
        .map_err(|e| format!("could not create a variant of {}: {e}", base.as_str()))?;
    editor.assets.refresh();
    log::info!(
        "created {}, a variant of {}",
        target.as_str(),
        base.as_str()
    );
    Ok(target)
}
