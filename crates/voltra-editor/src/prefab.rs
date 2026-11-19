//! Turning the selection into a prefab: write it out, then link it back.
//!
//! Unity's drag of a hierarchy row into the Project window and Godot's
//! "Save Branch as Scene" are the same command: the selected object and
//! everything under it become a file in the asset folder, and the object in
//! the scene becomes an instance of that file. This is the command without
//! the drag — the asset browser's current folder is where the file goes.

use voltra_assets::AssetPath;
use voltra_core::UiFrame;
use voltra_scene::{prefab, Name, PrefabInstance, SceneId};

use crate::editor::Editor;
use crate::play::PlayState;
use crate::undo::SceneView;

/// What a selection with no name, or one made only of characters a file name
/// cannot hold, is written as.
const FALLBACK: &str = "Prefab";

/// How many `name N.ron` siblings are tried before giving up on a free name.
///
/// A folder with a thousand copies of one prefab is something an author
/// should be told about, not one the editor should quietly add to.
const MAX_SUFFIX: u32 = 1000;

/// Writes the selection and its descendants to the browser's folder as a
/// prefab, then links the selection to it, as one undo entry.
///
/// Named after the selection, with a number appended when that file exists:
/// Unity's answer, and the one that cannot overwrite somebody's prefab with a
/// menu click. The undo takes the link back off; the file stays, the way a
/// Unity prefab asset outlives an undone Create — an undo that deleted files
/// from the asset folder would be the only one in the editor that did.
///
/// Refused while playing, like every other edit, and with nothing selected.
/// A prefab that would contain itself is refused by [`prefab::write`] and
/// nothing is written or linked.
pub(crate) fn create_from_selection(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    if editor.play.state() != PlayState::Editing {
        return;
    }
    let Some(root) = editor.selected.filter(|&e| frame.world.is_alive(e)) else {
        return;
    };
    let Some(&id) = frame.world.get::<SceneId>(root) else {
        // Transient: it is not part of the scene, so it cannot be part of a
        // prefab cut from it either.
        log::warn!("the selection is not part of the scene; no prefab made");
        return;
    };

    let name = frame.world.get::<Name>(root).map_or(FALLBACK, Name::as_str);
    let on_disk = frame.prefabs.root().to_path_buf();
    let Some(target) = free_path(editor.assets.dir(), name, |path| {
        on_disk.join(path.as_str()).exists()
    }) else {
        log::error!("no free file name for a prefab called {name:?}; nothing written");
        return;
    };

    if let Err(e) = prefab::write(frame.world, &editor.registry, frame.prefabs, root, &target) {
        log::error!("could not create the prefab: {e}");
        return;
    }
    log::info!("created prefab {}", target.as_str());
    editor.assets.refresh();

    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: Some(id),
    };
    editor.history.begin("Create prefab", view, [id]);
    frame.world.insert(root, PrefabInstance::new(target));
    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: Some(id),
    };
    editor.history.commit(view);
}

/// The first of `dir/name.ron`, `dir/name 1.ron`, `dir/name 2.ron`, … that
/// `exists` says is free, or `None` after [`MAX_SUFFIX`] tries.
fn free_path(
    dir: Option<&AssetPath>,
    name: &str,
    exists: impl Fn(&AssetPath) -> bool,
) -> Option<AssetPath> {
    let stem = file_stem(name);
    (0..=MAX_SUFFIX).find_map(|n| {
        let file = match n {
            0 => format!("{stem}.ron"),
            n => format!("{stem} {n}.ron"),
        };
        let raw = match dir {
            Some(dir) => format!("{}/{file}", dir.as_str()),
            None => file,
        };
        let path = AssetPath::new(&raw).ok()?;
        (!exists(&path)).then_some(path)
    })
}

/// `name` as something every file system will take as one file name.
///
/// Anything but letters, digits, spaces, `-` and `_` becomes `_`: a name is
/// free text, and `Door/Left` or `Boss: phase 2` are names somebody gives an
/// entity. Trimmed, because Windows refuses a name ending in a space, and
/// [`FALLBACK`] when nothing is left.
fn file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        FALLBACK.to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> AssetPath {
        AssetPath::new(raw).expect("a valid asset path")
    }

    #[test]
    fn a_free_name_is_used_as_it_is() {
        let found = free_path(Some(&path("prefabs")), "coin", |_| false);

        assert_eq!(found, Some(path("prefabs/coin.ron")));
    }

    #[test]
    fn a_taken_name_gets_the_first_free_number() {
        let taken = [path("coin.ron"), path("coin 1.ron")];

        let found = free_path(None, "coin", |p| taken.contains(p));

        assert_eq!(found, Some(path("coin 2.ron")));
    }

    #[test]
    fn a_folder_full_of_copies_gives_up_rather_than_spinning() {
        assert_eq!(free_path(None, "coin", |_| true), None);
    }

    #[test]
    fn a_name_a_file_system_would_refuse_is_cleaned() {
        assert_eq!(file_stem("Door/Left"), "Door_Left");
        assert_eq!(file_stem("Boss: phase 2 "), "Boss_ phase 2");
        assert_eq!(file_stem("..."), "___");
        assert_eq!(file_stem("  "), FALLBACK);
    }
}
