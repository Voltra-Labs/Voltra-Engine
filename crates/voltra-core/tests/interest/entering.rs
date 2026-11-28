//! Entering at the radius and leaving past it; what reaches everyone; the
//! owned entity; the points a game watches.

use super::rig::*;

#[test]
fn an_entity_enters_at_the_radius_and_leaves_past_it_under_loss() {
    let mut rig = Rig::new(lossy(), interest(10.0));
    let peer = rig.join(|_| {});
    owned(rig.world(), Vec2::ZERO, peer);
    let (mover, id) = spawn(rig.world(), Vec2::new(20.0, 0.0));
    rig.step(60);
    assert!(copy(rig.clients[0].world(), id).is_none(), "far: not sent");
    assert_eq!(rig.relevant(peer), 1, "only its own");

    place(rig.world(), mover, Vec2::new(9.5, 0.0));
    rig.step(60);
    let first = copy(rig.clients[0].world(), id).expect("near: spawned");
    assert_eq!(rig.relevant(peer), 2);

    // Back and forth across the radius, inside the leave radius (11): no
    // flapping — the client keeps the one copy throughout.
    for n in 0..240 {
        let x = if (n / 7) % 2 == 0 { 10.2 } else { 10.9 };
        place(rig.world(), mover, Vec2::new(x, 0.0));
        rig.step(1);
        assert_eq!(
            copy(rig.clients[0].world(), id),
            Some(first),
            "frame {n}: the same copy"
        );
    }

    place(rig.world(), mover, Vec2::new(11.5, 0.0));
    rig.step(90);
    assert!(
        copy(rig.clients[0].world(), id).is_none(),
        "past the leave radius: despawned, through 20 % loss"
    );
    assert!(
        rig.server.world().is_alive(mover),
        "on the server it lives on"
    );
    let stats = rig.clients[0]
        .net()
        .replication()
        .and_then(|r| r.client())
        .expect("a client")
        .stats();
    assert_eq!(stats.out_of_interest, 1, "set aside");

    // And it comes back where the server has it.
    place(rig.world(), mover, Vec2::new(-4.0, 3.0));
    rig.step(90);
    let back = copy(rig.clients[0].world(), id).expect("back");
    assert_eq!(
        rig.clients[0]
            .world()
            .get::<Transform2D>(back)
            .map(|t| t.translation),
        Some(Vec2::new(-4.0, 3.0))
    );

    // Away again and despawned for real meanwhile: the client lets go of
    // what it set aside.
    place(rig.world(), mover, Vec2::new(40.0, 0.0));
    rig.step(90);
    assert!(copy(rig.clients[0].world(), id).is_none());
    rig.world().despawn(mover);
    rig.step(90);
    let stats = rig.clients[0]
        .net()
        .replication()
        .and_then(|r| r.client())
        .expect("a client")
        .stats();
    assert_eq!(stats.out_of_interest, 0, "nothing kept");
}

#[test]
fn an_always_relevant_entity_and_one_with_no_place_reach_everyone() {
    let mut rig = Rig::new(LinkConditions::default(), interest(5.0));
    let peer = rig.join(|_| {});
    owned(rig.world(), Vec2::ZERO, peer);
    let (beacon, beacon_id) = spawn(rig.world(), Vec2::new(1000.0, 0.0));
    rig.world()
        .insert(beacon, Replicated::default().always_relevant());
    let round = rig.world().spawn();
    let round_id = SceneId::new();
    rig.world().insert(round, round_id);
    rig.world().insert(round, Replicated::default());
    rig.world().insert(round, Health(5));
    let (_, wide_id) = spawn(rig.world(), Vec2::new(0.0, 40.0));
    let wide = copy(rig.server.world(), wide_id).expect("there");
    rig.world()
        .insert(wide, Replicated::default().with_relevance_radius(50.0));
    let (_, far_id) = spawn(rig.world(), Vec2::new(0.0, 40.0));
    rig.step(30);

    let client = rig.clients[0].world();
    assert!(copy(client, beacon_id).is_some(), "always relevant");
    assert!(copy(client, round_id).is_some(), "no transform: everywhere");
    assert!(copy(client, wide_id).is_some(), "its own radius reaches");
    assert!(copy(client, far_id).is_none(), "the session's does not");
}

#[test]
fn the_owned_entity_and_its_hierarchy_are_relevant_wherever_they_are() {
    let mut rig = Rig::new(LinkConditions::default(), interest(5.0));
    let peer = rig.join(|_| {});
    // Its player rides in a seat a long way from the vehicle's origin: the
    // vehicle is the replicated root, relevant through the seat.
    let (vehicle, vehicle_id) = spawn(rig.world(), Vec2::new(-300.0, 0.0));
    let (seat, seat_id) = spawn(rig.world(), Vec2::new(200.0, 0.0));
    rig.world().insert(seat, NetOwner(peer));
    hierarchy::set_parent(rig.world(), seat, vehicle).expect("parents");
    // A turret on a vehicle nobody is near, its child placed by the player.
    let (tank, tank_id) = spawn(rig.world(), Vec2::new(500.0, 0.0));
    let (turret, turret_id) = spawn(rig.world(), Vec2::new(-600.0, 0.0));
    hierarchy::set_parent(rig.world(), turret, tank).expect("parents");
    rig.step(30);

    let client = rig.clients[0].world();
    assert!(
        copy(client, vehicle_id).is_some(),
        "the owned entity's root"
    );
    assert!(copy(client, seat_id).is_some(), "the owned entity");
    assert!(
        copy(client, tank_id).is_none() && copy(client, turret_id).is_none(),
        "a child follows its root, even sitting next to the player"
    );
    assert_eq!(rig.relevant(peer), 2);

    // The player brings the tank near: both come, root and child.
    place(rig.world(), tank, Vec2::new(-98.0, 0.0));
    rig.step(30);
    let client = rig.clients[0].world();
    assert!(copy(client, tank_id).is_some() && copy(client, turret_id).is_some());
    assert_eq!(rig.relevant(peer), 4);
}

#[test]
fn with_its_owner_gone_a_client_sees_what_the_game_watches_for_it() {
    let mut rig = Rig::new(LinkConditions::default(), interest(8.0));
    let peer = rig.join(|_| {});
    let player = owned(rig.world(), Vec2::ZERO, peer);
    let (_, near_id) = spawn(rig.world(), Vec2::new(3.0, 0.0));
    let (_, over_there_id) = spawn(rig.world(), Vec2::new(100.0, 0.0));
    rig.step(30);
    assert!(copy(rig.clients[0].world(), near_id).is_some());

    // The player dies: nothing to watch from, so nothing near it is sent.
    rig.world().despawn(player);
    rig.step(30);
    assert!(copy(rig.clients[0].world(), near_id).is_none());
    assert_eq!(rig.relevant(peer), 0);

    // A spectator camera, set by the game.
    assert!(rig
        .server
        .net_mut()
        .set_interest_points(peer, vec![Vec2::new(98.0, 0.0)]));
    rig.step(30);
    let client = rig.clients[0].world();
    assert!(copy(client, over_there_id).is_some(), "watched");
    assert!(copy(client, near_id).is_none(), "and only that");
    let points = rig
        .server
        .net()
        .replication()
        .and_then(|r| r.server())
        .expect("a server")
        .interest_points(peer)
        .to_vec();
    assert_eq!(points, [Vec2::new(98.0, 0.0).into()]);

    // Capped, and refused for a peer that is not there.
    let many = vec![Vec2::new(98.0, 0.0); 100];
    assert!(rig.server.net_mut().set_interest_points(peer, many));
    rig.step(2);
    let server = rig
        .server
        .net()
        .replication()
        .and_then(|r| r.server())
        .expect("a server");
    assert_eq!(
        server.interest_points(peer).len(),
        voltra_core::replication::MAX_INTEREST_POINTS
    );
    assert!(!rig
        .server
        .net_mut()
        .set_interest_points(PeerId(999), vec![Vec2::ZERO]));
}
