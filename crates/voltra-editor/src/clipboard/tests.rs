//! Copies and pastes through a world, no GPU: what a paste makes, where it
//! puts it, and what the clipboard text holds.

use voltra_render::glam::Vec2;
use voltra_scene::{Name, Sprite};

use super::*;

fn registry() -> ComponentRegistry {
    ComponentRegistry::with_defaults()
}

fn spawn(world: &mut World, name: &str, at: Vec2) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Name::new(name));
    world.insert(e, Transform::from_translation(at));
    world.insert(e, Sprite::default());
    e
}

fn named(world: &World, name: &str) -> Vec<Entity> {
    world
        .query::<Name>()
        .filter(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .collect()
}

fn selecting(entities: &[Entity]) -> Selection {
    let mut selection = Selection::default();
    entities.iter().for_each(|&e| selection.add(e));
    selection
}

#[test]
fn a_paste_copies_each_top_and_what_is_under_it_with_fresh_ids() {
    let registry = registry();
    let mut world = World::new();
    let parent = spawn(&mut world, "parent", Vec2::new(5.0, 0.0));
    let child = spawn(&mut world, "child", Vec2::new(1.0, 0.0));
    hierarchy::set_parent(&mut world, child, parent).expect("no cycle");
    let loner = spawn(&mut world, "loner", Vec2::new(-3.0, 2.0));

    // The child is selected with its parent: copied once, under it.
    let clip = copy(&world, &registry, &selecting(&[child, parent, loner]))
        .expect("records")
        .expect("something to copy");
    assert_eq!(clip.tops.len(), 2);
    assert_eq!(clip.scene.entities.len(), 3);

    let (made, tops) = paste(&mut world, &registry, &clip).expect("spawns");
    assert_eq!(made.len(), 3);
    assert_eq!(tops.len(), 2);
    for name in ["parent", "child", "loner"] {
        assert_eq!(named(&world, name).len(), 2, "{name} twice");
    }
    let mut ids: Vec<SceneId> = world.query::<SceneId>().map(|(_, id)| *id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 6, "every copy has an id of its own");
    // The copied child hangs off the copied parent, not the original.
    let copies: Vec<Entity> = named(&world, "child")
        .into_iter()
        .filter(|&c| c != child)
        .collect();
    let new_parent = hierarchy::parent_of(&world, copies[0]).expect("a parent");
    assert_ne!(new_parent, parent);
    assert!(made.contains(&new_parent));
}

#[test]
fn a_duplicated_child_stays_under_its_parent_where_it_was() {
    let registry = registry();
    let mut world = World::new();
    let parent = spawn(&mut world, "parent", Vec2::new(5.0, 0.0));
    let child = spawn(&mut world, "child", Vec2::new(1.0, 0.0));
    hierarchy::set_parent(&mut world, child, parent).expect("no cycle");

    let clip = copy(&world, &registry, &selecting(&[child]))
        .expect("records")
        .expect("something to copy");
    let (_, tops) = paste(&mut world, &registry, &clip).expect("spawns");
    assert_eq!(hierarchy::parent_of(&world, tops[0]), Some(parent));
    let at = hierarchy::world_transform(&world, tops[0]).translation;
    assert!((at - Vec2::new(6.0, 0.0)).length() < 1e-5, "{at}");
}

#[test]
fn a_paste_whose_parent_is_gone_lands_where_the_copy_was_taken() {
    let registry = registry();
    let mut world = World::new();
    let parent = spawn(&mut world, "parent", Vec2::new(5.0, 0.0));
    let child = spawn(&mut world, "child", Vec2::new(1.0, 0.0));
    hierarchy::set_parent(&mut world, child, parent).expect("no cycle");
    let clip = copy(&world, &registry, &selecting(&[child]))
        .expect("records")
        .expect("something to copy");

    // Pasted into another scene.
    let mut other = World::new();
    let (_, tops) = paste(&mut other, &registry, &clip).expect("spawns");
    assert_eq!(hierarchy::parent_of(&other, tops[0]), None);
    let at = hierarchy::world_transform(&other, tops[0]).translation;
    assert!((at - Vec2::new(6.0, 0.0)).length() < 1e-5, "{at}");
}

#[test]
fn the_clipboard_text_round_trips_and_other_text_is_not_a_clip() {
    let registry = registry();
    let mut world = World::new();
    let one = spawn(&mut world, "one", Vec2::ZERO);
    let clip = copy(&world, &registry, &selecting(&[one]))
        .expect("records")
        .expect("something to copy");
    let text = to_text(&clip).expect("writes");
    assert!(text.starts_with(HEADER));
    assert_eq!(from_text(&text).expect("ours").expect("reads"), clip);
    let windows = text.replacen('\n', "\r\n", 1);
    assert!(from_text(&windows).is_some_and(|read| read.is_ok()));
    assert!(from_text("hello").is_none());
    assert!(
        from_text(&format!("{HEADER}not ron")).is_some_and(|read| read.is_err()),
        "ours, but broken: an error, not a silent nothing"
    );
}

#[test]
fn nothing_in_the_scene_selected_copies_nothing() {
    let registry = registry();
    let mut world = World::new();
    let transient = world.spawn();
    world.insert(transient, Transform::default());
    assert!(copy(&world, &registry, &selecting(&[transient]))
        .expect("records")
        .is_none());
    assert!(copy(&world, &registry, &Selection::default())
        .expect("records")
        .is_none());
}
