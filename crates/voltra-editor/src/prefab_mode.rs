//! Prefab mode: a prefab opened on its own, as a temporary scene, and the
//! way back to the scene it was opened from.
//!
//! Unity's Prefab Mode, in isolation: the scene is set aside — its world
//! written out, its file, history, selection and camera parked — and the
//! prefab's file is read into the world in its place. Every tool edits it as
//! it would a scene; Save writes the prefab's file. Back puts the scene
//! back, and its instances follow the prefab as it was last saved through
//! stage 64's sync, but for what each overrides. Each document keeps its
//! own undo history, so Ctrl+Z in the prefab never reaches into the scene
//! and the scene's history is where it was on the way back.
//!
//! The modes nest: an instance inside the prefab can be opened in turn, and
//! Back returns one level, as Unity's breadcrumb does.
//!
//! Every way in or out that a panel asks for is queued and taken at the
//! start of the next frame, by [`crate::scene_dialogs::show`]: a panel
//! drawing an entity must not have the world replaced under it mid-draw.

mod swap;

use voltra_assets::AssetPath;
use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_render::Camera2D;
use voltra_scene::format::SceneFile;
use voltra_scene::{prefab, PrefabInstance, Prefabs, SceneId};

pub(crate) use swap::cycle_through;

use crate::document::SceneDocument;
use crate::editor::Editor;
use crate::scene_file;
use crate::selection::Selection;
use crate::undo::{History, SceneView};

/// A document set aside while a prefab is edited over it.
#[derive(Debug)]
struct Parked {
    scene: SceneFile,
    document: SceneDocument,
    history: History,
    selected: Vec<SceneId>,
    camera: Camera2D,
}

/// What a panel asked prefab mode to do, taken next frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pending {
    /// Open this prefab over whatever is open.
    Edit(AssetPath),
    /// Return to the document under the prefab, asking to save it first.
    Back,
}

/// The documents set aside, the scene first, and what was asked for.
#[derive(Debug, Default)]
pub(crate) struct PrefabMode {
    stack: Vec<Parked>,
    pending: Option<Pending>,
}

impl PrefabMode {
    /// Whether a prefab is open over a scene.
    pub(crate) fn is_active(&self) -> bool {
        !self.stack.is_empty()
    }

    /// The names of the documents set aside, the scene first: the
    /// breadcrumb's trail up to the prefab being edited.
    pub(crate) fn trail(&self) -> impl Iterator<Item = &str> {
        self.stack.iter().map(|parked| parked.document.name())
    }

    /// What Back returns to, if a prefab is open.
    pub(crate) fn returns_to(&self) -> Option<&str> {
        self.stack.last().map(|parked| parked.document.name())
    }

    /// Opens `path` next frame.
    pub(crate) fn request_edit(&mut self, path: AssetPath) {
        self.pending = Some(Pending::Edit(path));
    }

    /// Returns to the document under the prefab next frame, through "save
    /// changes?".
    pub(crate) fn request_back(&mut self) {
        self.pending = Some(Pending::Back);
    }

    /// What a panel asked for since last frame.
    pub(crate) fn take_pending(&mut self) -> Option<Pending> {
        self.pending.take()
    }

    /// Drops every document set aside, unsaved changes and all. For a scene
    /// replaced wholesale while one was — which every caller avoids by
    /// leaving first; logged, because it loses work.
    pub(crate) fn abandon(&mut self) {
        if self.is_active() {
            log::warn!(
                "left prefab mode without returning: {} set aside and dropped",
                self.trail().collect::<Vec<_>>().join(", ")
            );
        }
        self.stack.clear();
    }
}

/// The prefab an instance's entity belongs to: its instance root's file.
pub(crate) fn source_of(world: &World, prefabs: &mut Prefabs, entity: Entity) -> Option<AssetPath> {
    let root = prefab::instance_root(world, prefabs, entity)?;
    world
        .get::<PrefabInstance>(root)
        .map(|link| link.source.clone())
}

/// What the window is called: the scene's title, or in prefab mode the
/// prefab and what it was opened from.
pub(crate) fn title(editor: &Editor) -> String {
    match editor.prefab_mode.returns_to() {
        None => editor.document.title(&editor.history),
        Some(from) => {
            let star = if editor.document.is_dirty(&editor.history) {
                "*"
            } else {
                ""
            };
            format!(
                "{}{star} (prefab, from {from}) — Voltra Editor",
                editor.document.name()
            )
        }
    }
}

/// Opens the prefab at `path` over the open document, which is set aside
/// with its history, selection and camera. Refused — the document left as
/// it was — for a path that is not a scene file, is not there, will not
/// read, or is already open in this stack.
pub(crate) fn enter(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    path: &AssetPath,
) -> Result<(), String> {
    let path = scene_file::scene_path(path.as_str())?;
    let open = std::iter::once(&editor.document)
        .chain(editor.prefab_mode.stack.iter().map(|p| &p.document))
        .any(|document| document.path() == Some(&path));
    if open {
        return Err(format!("{} is open already", path.as_str()));
    }
    let file = scene_file::file_of(frame, &path);
    if !file.is_file() {
        return Err(format!("there is no prefab at {}", path.as_str()));
    }
    editor.stop_play(frame);

    // What this frame did so far is the document's, closed into its own
    // history before both are set aside.
    let selected = editor.selection.ids(frame.world);
    editor.history.end_frame(SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: selected.clone(),
    });
    let scene = swap::set_aside(frame.world, &editor.registry)?;
    if let Err(e) = swap::open_prefab(frame.world, &editor.registry, frame.prefabs, &file) {
        swap::put_back(frame.world, &editor.registry, &scene);
        frame.reset_physics();
        frame.resolve_scene_assets();
        return Err(format!("could not open {}: {e}", path.as_str()));
    }
    frame.reset_physics();
    frame.resolve_scene_assets();

    let camera = *frame.camera;
    // A prefab's root is saved at the origin, so that is where it is.
    frame.camera.position = Vec2::ZERO;
    frame.camera.rotation = 0.0;
    editor.prefab_mode.stack.push(Parked {
        scene,
        document: std::mem::take(&mut editor.document),
        history: std::mem::take(&mut editor.history),
        selected,
        camera,
    });
    editor.history.skip_frame();
    editor
        .document
        .replaced(Some(path.clone()), &editor.history);
    editor.selection.clear();
    drop_held_tools(editor);
    log::info!("editing prefab {}", path.as_str());
    Ok(())
}

/// Puts the document under the prefab back, its instances following the
/// prefab as it was last saved. Unsaved changes to the prefab are dropped:
/// the caller asked about them first. Answers the prefab left.
pub(crate) fn leave(editor: &mut Editor, frame: &mut UiFrame<'_>) -> Result<AssetPath, String> {
    if !editor.prefab_mode.is_active() {
        return Err("no prefab is open".to_owned());
    }
    editor.stop_play(frame);
    let parked = editor
        .prefab_mode
        .stack
        .pop()
        .expect("checked to be active just above");
    let left = editor
        .document
        .path()
        .cloned()
        .expect("a prefab is opened from its file, so it has one");

    swap::put_back(frame.world, &editor.registry, &parked.scene);
    let report = swap::follow(frame.world, &editor.registry, frame.prefabs, &left);
    frame.reset_physics();
    frame.resolve_scene_assets();

    // Its history is gone with it, as Unity's is on leaving Prefab Mode.
    editor.history = parked.history;
    editor.history.skip_frame();
    editor.document = parked.document;
    editor.selection = Selection::from_ids(frame.world, &parked.selected);
    drop_held_tools(editor);
    // Where it was, at the viewport's shape now.
    let mut camera = parked.camera;
    camera.aspect = frame.camera.aspect;
    *frame.camera = camera;
    log::info!(
        "back in {} from {}: {} instance component(s) followed it",
        editor.document.name(),
        left.as_str(),
        report.changed + report.spawned + report.despawned
    );
    Ok(left)
}

/// Lets go of every drag that names an entity of the world just replaced.
fn drop_held_tools(editor: &mut Editor) {
    editor.gizmo.cancel_drag();
    editor.shape.cancel_drag();
    editor.marquee.cancel();
}
