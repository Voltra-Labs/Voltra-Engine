//! Copying, pasting and duplicating the selection.
//!
//! **The clipboard is the system's, and what is on it is text**: the copied
//! entities as a scene file, in RON, after a header line that says what it
//! is. egui hands the editor a paste only as the clipboard's text (and only
//! when there is some), so a clipboard of our own would never see Ctrl+V;
//! as text it also pastes into a second editor window, and the entities can
//! be read, mailed or diffed. Unity copies to the system clipboard the same
//! way. Text that does not start with the header is somebody else's and is
//! left alone.
//!
//! **A copy is the selection's [`tops`](Selection::tops) and everything under
//! each**, recorded as a save records them, with each top's parent by id and
//! its place in the world. **A paste spawns them as a prefab's copies are
//! spawned** — fresh ids, links inside remapped ([`prefab::spawn_scene`]) —
//! then puts each top back under the parent it had, if the scene still holds
//! it, or where it was in the world if not: a duplicate lands on its
//! original, as in Unity and Godot, and a paste into another scene lands
//! where the copy was taken. Duplicate is a copy and a paste that leaves the
//! clipboard alone. Each is one undo entry, and selects what it made.
//!
//! **A copy is named apart from its new siblings** — `Coin (1)`, `Coin (2)`
//! — once it has landed: see [`numbering`].

mod numbering;

use serde::{Deserialize, Serialize};
use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_scene::format::{entity_with_id, record_entity, SceneFile, VERSION};
use voltra_scene::hierarchy;
use voltra_scene::prefab::{self, Prefab};
use voltra_scene::{ComponentRegistry, SceneError, SceneId, Transform};

use crate::editor::Editor;
use crate::selection::Selection;
use crate::undo::SceneView;

/// What the clipboard's text starts with when it holds entities.
const HEADER: &str = "voltra-entities 1\n";

/// The most clipboard text a paste reads. Anything longer is not a copy
/// this editor made, and parsing it would only be a way to stall the frame.
const MAX_TEXT: usize = 16 * 1024 * 1024;

/// Copied entities, as the clipboard holds them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Clip {
    /// One per copied top, in the order they lead in `scene`.
    tops: Vec<Top>,
    /// Each top, then everything under it.
    scene: SceneFile,
}

/// Where a copied top stood.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Top {
    /// Its parent when copied, by identity; `None` for a root.
    parent: Option<SceneId>,
    /// Its transform in the world when copied, for a paste that cannot put
    /// it back under that parent. `None` without one.
    world: Option<Transform>,
}

/// The selection as a [`Clip`]; `None` when nothing selected is part of the
/// scene.
fn copy(
    world: &World,
    registry: &ComponentRegistry,
    selection: &Selection,
) -> Result<Option<Clip>, SceneError> {
    let mut tops = Vec::new();
    let mut entities = Vec::new();
    for top in selection.tops(world) {
        if world.get::<SceneId>(top).is_none() {
            // Transient: not part of the scene, so not copied either.
            continue;
        }
        tops.push(Top {
            parent: hierarchy::parent_of(world, top)
                .and_then(|parent| world.get::<SceneId>(parent).copied()),
            world: world
                .get::<Transform>(top)
                .map(|_| hierarchy::world_transform(world, top)),
        });
        for entity in std::iter::once(top).chain(hierarchy::descendants(world, top)) {
            if let Some(record) = record_entity(world, registry, entity) {
                entities.push(record?);
            }
        }
    }
    Ok((!tops.is_empty()).then_some(Clip {
        tops,
        scene: SceneFile {
            version: VERSION,
            entities,
        },
    }))
}

/// Spawns a copy of `clip` into `world`: every entity it made, and its tops
/// in the clip's order.
fn paste(
    world: &mut World,
    registry: &ComponentRegistry,
    clip: &Clip,
) -> Result<(Vec<Entity>, Vec<Entity>), SceneError> {
    let instance = prefab::spawn_scene(world, registry, &Prefab::new(clip.scene.clone())?)?;
    // The spawn cut every link that left the clip, so its roots are the tops,
    // in file order. A clip edited by hand to disagree is put down as it is.
    if instance.roots.len() != clip.tops.len() {
        log::warn!(
            "a pasted clip has {} roots for {} copied tops; pasted where they stand",
            instance.roots.len(),
            clip.tops.len()
        );
        numbering::number(world, &instance.roots);
        return Ok((instance.entities, instance.roots));
    }
    for (&root, top) in instance.roots.iter().zip(&clip.tops) {
        let parent = top.parent.and_then(|id| entity_with_id(world, id));
        // Under its old parent its transform is the local one it was copied
        // with; anywhere else it is put back where it was in the world.
        let under = parent.is_some_and(|parent| hierarchy::set_parent(world, root, parent).is_ok());
        if !under {
            if let Some(placed) = &top.world {
                hierarchy::set_world_transform(world, root, placed);
            }
        }
    }
    // Last: the siblings a top is named apart from are where it landed.
    numbering::number(world, &instance.roots);
    Ok((instance.entities, instance.roots))
}

/// `clip` as clipboard text.
fn to_text(clip: &Clip) -> Result<String, ron::Error> {
    Ok(format!("{HEADER}{}", ron::to_string(clip)?))
}

/// The clip in clipboard `text`; `None` for text that is not one.
fn from_text(text: &str) -> Option<Result<Clip, ron::error::SpannedError>> {
    if text.len() > MAX_TEXT {
        return None;
    }
    // Copied from a Windows clipboard, the header can arrive with `\r\n`.
    let body = text
        .strip_prefix(HEADER)
        .or_else(|| text.strip_prefix(&HEADER.replace('\n', "\r\n")))?;
    Some(ron::from_str(body))
}

/// Puts the selection on the system clipboard. Nothing in the scene
/// selected leaves the clipboard as it was.
pub(crate) fn copy_selection(
    editor: &mut Editor,
    ctx: &voltra_core::egui::Context,
    frame: &UiFrame<'_>,
) {
    let clip = match copy(frame.world, &editor.registry, &editor.selection) {
        Ok(Some(clip)) => clip,
        Ok(None) => return,
        Err(error) => {
            log::error!("could not copy the selection: {error}");
            return;
        }
    };
    match to_text(&clip) {
        Ok(text) => ctx.copy_text(text),
        Err(error) => log::error!("could not copy the selection: {error}"),
    }
}

/// Pastes the entities in clipboard `text`, as one undo entry. Text that is
/// not a copy of entities is ignored.
pub(crate) fn paste_text(editor: &mut Editor, frame: &mut UiFrame<'_>, text: &str) {
    match from_text(text) {
        None => {}
        Some(Ok(clip)) => put(editor, frame, "Paste", &clip),
        Some(Err(error)) => {
            log::warn!("the clipboard holds entities this editor cannot read: {error}")
        }
    }
}

/// Copies the selection and pastes it straight back, leaving the clipboard
/// alone.
pub(crate) fn duplicate(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    match copy(frame.world, &editor.registry, &editor.selection) {
        Ok(Some(clip)) => put(editor, frame, "Duplicate", &clip),
        Ok(None) => {}
        Err(error) => log::error!("could not duplicate the selection: {error}"),
    }
}

/// Spawns `clip`, records it as `label` and selects its tops.
fn put(editor: &mut Editor, frame: &mut UiFrame<'_>, label: &'static str, clip: &Clip) {
    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: editor.selection.ids(frame.world),
    };
    editor.history.begin(label, view, []);
    let made = paste(frame.world, &editor.registry, clip);
    let ids: Vec<SceneId> = match &made {
        Ok((entities, tops)) => {
            // Drawn this very frame: their textures, fonts and clips are
            // looked up now.
            frame.resolve_assets_of(entities);
            // A copy of an instance renamed by its number overrides its
            // name; said now, so it is part of this entry rather than an
            // edit of its own next frame.
            for &top in tops {
                if frame.world.get::<voltra_scene::PrefabPart>(top).is_some() {
                    if let Err(error) =
                        prefab::refresh_overrides(frame.world, &editor.registry, frame.prefabs, top)
                    {
                        log::error!("could not compare a pasted instance with its prefab: {error}");
                    }
                }
            }
            editor.selection.clear();
            tops.iter().for_each(|&top| editor.selection.add(top));
            entities
                .iter()
                .filter_map(|&e| frame.world.get::<SceneId>(e).copied())
                .collect()
        }
        Err(error) => {
            // All-or-nothing: the spawn despawned what it made.
            log::error!("could not {}: {error}", label.to_lowercase());
            Vec::new()
        }
    };
    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: editor.selection.ids(frame.world),
    };
    editor.history.commit_including(view, ids);
}

#[cfg(test)]
mod tests;
