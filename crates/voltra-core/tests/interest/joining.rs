//! A client joining late; a child moved off its culled parent; what a late
//! joiner swept let go of with its level.

use super::rig::*;

#[test]
fn a_client_joining_late_is_sent_what_is_near_it_and_finds_its_level_again() {
    let mut rig = Rig::new(lossy(), interest(10.0));
    let early = rig.join(|_| {});
    owned(rig.world(), Vec2::ZERO, early);
    let door_id = SceneId::new();
    let level = move |world: &mut World| {
        let door = world.spawn();
        world.insert(door, door_id);
        world.insert(door, Transform2D::from_translation(Vec2::new(60.0, 0.0)));
        world.insert(door, Replicated::default());
        world.insert(door, Sprite2D::default().with_sort_order(9));
        door
    };
    let door = level(rig.world());
    let (_, crowd_id) = spawn(rig.world(), Vec2::new(30.0, 30.0));
    rig.step(120);

    let late = rig.join(|world| {
        level(world);
    });
    owned(rig.world(), Vec2::new(30.0, 28.0), late);
    rig.step(120);
    let client = rig.clients[1].world();
    assert!(copy(client, crowd_id).is_some(), "near it");
    assert!(
        copy(client, door_id).is_none(),
        "the level's far door, never sent, swept out of the world"
    );
    assert!(
        copy(rig.clients[0].world(), crowd_id).is_none(),
        "far from the first"
    );

    // The door swings to the late client: the level's own copy comes back.
    place(rig.world(), door, Vec2::new(31.0, 25.0));
    rig.step(120);
    let client = rig.clients[1].world();
    let back = copy(client, door_id).expect("relevant now");
    assert_eq!(client.get::<Sprite2D>(back).map(|s| s.sort_order), Some(9));
}

#[test]
fn a_child_moved_off_its_parent_on_the_server_stays_when_the_parent_goes() {
    let mut rig = Rig::new(LinkConditions::default(), interest(10.0));
    // A tank and its turret, from the level both ends loaded.
    let (tank_id, turret_id) = (SceneId::new(), SceneId::new());
    let level = move |world: &mut World| {
        let tank = world.spawn();
        world.insert(tank, tank_id);
        world.insert(tank, Transform2D::from_translation(Vec2::new(2.0, 0.0)));
        world.insert(tank, Replicated::default());
        let turret = world.spawn();
        world.insert(turret, turret_id);
        world.insert(turret, Transform2D::from_translation(Vec2::new(0.0, 1.0)));
        world.insert(turret, Replicated::default());
        hierarchy::set_parent(world, turret, tank).expect("parents");
        (tank, turret)
    };
    let (tank, turret) = level(rig.world());
    let peer = rig.join(|world| {
        level(world);
    });
    owned(rig.world(), Vec2::ZERO, peer);
    rig.step(60);
    let before = copy(rig.clients[0].world(), turret_id).expect("adopted");

    // The server lifts the turret off and drives the tank away: the tank
    // leaves this client's interest, the turret does not.
    hierarchy::clear_parent(rig.world(), turret);
    place(rig.world(), turret, Vec2::new(2.0, 1.0));
    place(rig.world(), tank, Vec2::new(60.0, 0.0));
    rig.step(60);
    let client = rig.clients[0].world();
    assert!(copy(client, tank_id).is_none(), "the tank went");
    assert_eq!(
        copy(client, turret_id),
        Some(before),
        "the turret is the same copy, never despawned"
    );
    assert_eq!(hierarchy::parent_of(client, before), None);
    assert_eq!(
        client.get::<Transform2D>(before).map(|t| t.translation),
        Some(Vec2::new(2.0, 1.0))
    );
}

#[test]
fn what_a_late_joiner_swept_is_let_go_of_with_its_level() {
    use std::cell::Cell;
    use std::rc::Rc;

    let root = voltra_testkit::scratch_root();
    std::fs::create_dir_all(root.join("scenes")).expect("a folder");
    std::fs::write(root.join("scenes/next.ron"), "(version: 1, entities: [])").expect("written");
    let mut rig = Rig::new(LinkConditions::default(), interest(10.0));
    let door_id = SceneId::new();
    let level = move |world: &mut World| {
        let door = world.spawn();
        world.insert(door, door_id);
        world.insert(door, Transform2D::from_translation(Vec2::new(60.0, 0.0)));
        world.insert(door, Replicated::default());
    };
    level(rig.world());
    let change = Rc::new(Cell::new(false));
    let asked = change.clone();
    let peer = rig.join_with(
        |app| {
            app.with_asset_root(root.to_path_buf())
                .with_update(move |tick| {
                    if asked.replace(false) {
                        let next =
                            voltra_assets::AssetPath::new("scenes/next.ron").expect("a path");
                        tick.change_scene(&next, None);
                    }
                })
        },
        level,
    );
    owned(rig.world(), Vec2::ZERO, peer);
    rig.step(60);
    let set_aside = |rig: &Rig| {
        rig.clients[0]
            .net()
            .replication()
            .and_then(|r| r.client())
            .expect("a client")
            .stats()
            .out_of_interest
    };
    assert_eq!(set_aside(&rig), 1, "the far door, swept and set aside");

    change.set(true);
    rig.step(60);
    assert_eq!(set_aside(&rig), 0, "its level went, and it with it");
}
