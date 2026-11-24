//! Making, opening and saving the scene the editor has open.
//!
//! One place for the four, because the menu, the keys, the asset browser and
//! the bridge all reach them, and each has the same rules: stop play first,
//! leave the world as it was when anything fails, and keep the
//! [`SceneDocument`](crate::document::SceneDocument) and the history in step
//! with what is on screen.

use std::path::PathBuf;

use voltra_assets::AssetPath;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::SceneId;

use crate::editor::Editor;

/// Where Save As starts when the scene has no file yet.
pub(crate) const UNTITLED: &str = "scenes/untitled.ron";

/// Checks that `text` names a scene file under the asset root: an asset path
/// (no `..`, not absolute) ending in `.ron` that is not another RON asset
/// (an atlas, an input map, a clip…). Answers why not.
pub(crate) fn scene_path(text: &str) -> Result<AssetPath, String> {
    let path = AssetPath::new(text.trim()).map_err(|e| format!("not an asset path: {e}"))?;
    let name = path.as_str().rsplit('/').next().unwrap_or(path.as_str());
    if !voltra_assets::watch::is_scene(name) {
        return Err(format!(
            "{} is not a scene file name: it should end in .ron, and not in .atlas.ron, \
             .anim.ron, .animator.ron, .clip.ron, .input.ron or .layers.ron",
            path.as_str()
        ));
    }
    Ok(path)
}

/// Where `path` is on disk.
pub(crate) fn file_of(frame: &UiFrame<'_>, path: &AssetPath) -> PathBuf {
    frame.textures.root().join(path.as_str())
}

/// Empties the scene and starts a new, untitled one.
pub(crate) fn new_scene(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    // Silently: a new scene replaces the world the snapshot belongs to.
    editor.stop_play(frame);
    // Every caller leaves prefab mode first; one that did not loses the
    // scenes set aside, and is told.
    editor.prefab_mode.abandon();
    // Identity, not appearance, decides what belongs to the scene — the
    // rule `save` uses — so what is left is exactly what was never saved.
    let all: Vec<Entity> = frame.world.query::<SceneId>().map(|(e, _)| e).collect();
    for entity in all {
        frame.world.despawn(entity);
    }
    replaced(editor, None);
    log::info!("new scene");
}

/// Replaces the scene with the one at `path`. A file that will not read
/// leaves the open scene as it was, history and all.
pub(crate) fn open(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    path: &AssetPath,
) -> Result<(), String> {
    // Silently: Open replaces the world the snapshot belongs to, so keeping
    // play alive across it is meaningless. This is Unity's answer — opening a
    // scene exits play mode.
    editor.stop_play(frame);
    // Replaces rather than merges: "Open" meaning "add another copy of
    // everything" would surprise anyone. But the world is not cleared up
    // front — `load` can fail (missing file, bad RON, a component that won't
    // parse), and its own rollback only covers entities it spawned itself; an
    // entity despawned before the call is outside its reach and gone for
    // good. So the pre-existing set is captured first and only despawned once
    // `load` has reported success, which keeps a failed Open a no-op instead
    // of a scene-emptying one.
    //
    // Between a successful load and the despawn loop, both scenes are live in
    // the world at once — fine as long as nothing in between observes it, so
    // the despawn runs unconditionally right after `Ok`. An entity in the file
    // whose `SceneId` matches one already in the world: the old one is in
    // `previous` and goes, so the file's copy wins.
    let previous: Vec<Entity> = frame.world.query::<SceneId>().map(|(e, _)| e).collect();
    let file = file_of(frame, path);
    voltra_scene::format::load(&file, &editor.registry, frame.world)
        .map_err(|e| format!("could not open {}: {e}", path.as_str()))?;
    // As for a new scene: the world the scenes set aside belong under is
    // going.
    editor.prefab_mode.abandon();
    for entity in previous {
        frame.world.despawn(entity);
    }
    // Instances saved with an older copy of their prefab open in step with it.
    voltra_scene::prefab::sync_all(frame.world, &editor.registry, frame.prefabs);
    // The world just changed under every handle in it. After the despawns, so
    // no lookup is wasted on an entity about to disappear.
    frame.resolve_scene_assets();
    replaced(editor, Some(path.clone()));
    log::info!("opened {}", path.as_str());
    Ok(())
}

/// Saves the scene to its file. An untitled scene has none: Save As names
/// one.
pub(crate) fn save(editor: &mut Editor, frame: &mut UiFrame<'_>) -> Result<AssetPath, String> {
    let path = editor
        .document
        .path()
        .cloned()
        .ok_or_else(|| "the scene has no file yet: Save As names one".to_owned())?;
    save_as(editor, frame, path.clone())?;
    Ok(path)
}

/// Saves the scene to `path`, which becomes its file.
pub(crate) fn save_as(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    path: AssetPath,
) -> Result<(), String> {
    // Stops first so what lands on disk is the authored scene rather than a
    // mid-flight one, and says so: unlike Open, saving is not itself
    // destructive, so a silent mode change would be a surprise.
    if editor.stop_play(frame) {
        log::info!("stopped play so the saved scene is the authored one");
    }
    if editor.prefab_mode.is_active() && editor.document.path() != Some(&path) {
        return Err(format!(
            "{} is a prefab, saved to its own file: Save writes it, and it cannot be saved as {}",
            editor.document.name(),
            path.as_str()
        ));
    }
    // A file holding an instance of itself — a prefab dropped into its own
    // mode, a level dropped into itself — is refused, as `prefab::write`
    // refuses one.
    if let Some(through) = crate::prefab_mode::cycle_through(frame.world, frame.prefabs, &path) {
        return Err(format!(
            "{} would contain itself, through the instance of {}: nothing saved",
            path.as_str(),
            through.as_str()
        ));
    }
    let file = file_of(frame, &path);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    voltra_scene::format::save(frame.world, &editor.registry, &file)
        .map_err(|e| format!("could not save {}: {e}", path.as_str()))?;
    // Anything that spawns it next — a drop, a tick, prefab mode's way back
    // — gets what was just written, not what the watcher has yet to see.
    frame.prefabs.reload(&path);
    editor.document.saved(path, &editor.history);
    Ok(())
}

/// What every replacement of the scene resets: the selection, and the
/// history, whose ids all belong to the scene that was closed (Unity's
/// answer, and the only coherent one).
fn replaced(editor: &mut Editor, path: Option<AssetPath>) {
    editor.selection.clear();
    editor.shape.cancel_drag();
    editor.history.clear();
    editor.document.replaced(path, &editor.history);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_path_is_an_asset_path_to_a_scene_file() {
        assert!(scene_path(" scenes/a.ron ").is_ok());
        for bad in [
            "../a.ron",
            "/a.ron",
            "scenes/a.png",
            "a.anim.ron",
            "a.atlas.ron",
            "",
        ] {
            assert!(scene_path(bad).is_err(), "{bad}");
        }
    }
}
