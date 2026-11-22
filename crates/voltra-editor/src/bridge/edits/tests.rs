//! Bridge edits as undo entries, against a world with no window.

use std::collections::BTreeMap;

use voltra_bridge::Command;
use voltra_ecs::World;
use voltra_scene::{Name, SceneId};

use super::apply;
use crate::editor::Editor;
use crate::undo::{UndoContext, UndoHost};

struct Host {
    world: World,
}

impl UndoHost for Host {
    fn world(&mut self) -> &mut World {
        &mut self.world
    }
    fn reset_physics(&mut self) {}
    fn resolve_scene_assets(&mut self) {}
}

fn undo(editor: &mut Editor, host: &mut Host) -> bool {
    let ctx = UndoContext {
        registry: &editor.registry,
        selected: &mut editor.selection,
        gizmo: &mut editor.gizmo,
    };
    editor.history.undo(ctx, host)
}

fn spawn(name: &str, parent: Option<&str>) -> Command {
    Command::Spawn {
        name: Some(name.into()),
        parent: parent.map(str::to_owned),
        components: BTreeMap::from([(
            "Transform".to_owned(),
            "(translation: (1.0, 2.0), rotation: 0.0, scale: (1.0, 1.0))".to_owned(),
        )]),
    }
}

#[test]
fn a_bridge_spawn_is_one_undo_entry_and_undo_takes_it_out() {
    let mut editor = Editor::default();
    let mut host = Host {
        world: World::new(),
    };
    let answer = apply(&mut editor, &mut host.world, &spawn("Crate", None)).expect("spawns");
    assert!(answer["id"].as_str().is_some());
    assert_eq!(host.world.query::<SceneId>().count(), 1);
    assert_eq!(editor.history.undo_label(), Some("AI: spawn"));
    assert!(undo(&mut editor, &mut host));
    assert_eq!(host.world.query::<SceneId>().count(), 0, "undone");
}

#[test]
fn a_component_set_by_name_path_is_undone_back_to_what_it_was() {
    let mut editor = Editor::default();
    let mut host = Host {
        world: World::new(),
    };
    apply(&mut editor, &mut host.world, &spawn("Player", None)).expect("spawns");
    apply(&mut editor, &mut host.world, &spawn("Arm", Some("Player"))).expect("spawns");
    let set = Command::ComponentSet {
        entity: "Player/Arm".into(),
        component: "PointLight2D".into(),
        ron: "(radius: 3.0)".into(),
    };
    apply(&mut editor, &mut host.world, &set).expect("sets");
    assert_eq!(host.world.query::<voltra_scene::PointLight2D>().count(), 1);
    assert!(undo(&mut editor, &mut host));
    assert_eq!(
        host.world.query::<voltra_scene::PointLight2D>().count(),
        0,
        "undone"
    );
    assert_eq!(
        host.world.query::<Name>().count(),
        2,
        "the arm is still there"
    );
}

#[test]
fn a_refused_edit_records_nothing() {
    let mut editor = Editor::default();
    let mut host = Host {
        world: World::new(),
    };
    let before = editor.history.len();
    let bad = Command::Spawn {
        name: Some("Bad".into()),
        parent: None,
        components: BTreeMap::from([("Transform".to_owned(), "(oops)".to_owned())]),
    };
    assert!(apply(&mut editor, &mut host.world, &bad).is_err());
    assert!(apply(
        &mut editor,
        &mut host.world,
        &Command::Delete {
            entity: "Nobody".into()
        }
    )
    .is_err());
    assert_eq!(editor.history.len(), before);
    assert_eq!(host.world.query::<SceneId>().count(), 0);
}
