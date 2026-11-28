//! An entity back in interest; the budget; interest covering everything
//! against none.

use super::rig::*;

#[test]
fn an_entity_back_in_interest_carries_what_changed_and_keeps_what_was_its_own() {
    let mut rig = Rig::new(lossy(), interest(10.0));
    // A level both ends loaded: a crate the client draws with its sprite.
    let crate_id = SceneId::new();
    let level = move |world: &mut World| {
        let entity = world.spawn();
        world.insert(entity, crate_id);
        world.insert(entity, Transform2D::from_translation(Vec2::new(2.0, 0.0)));
        world.insert(entity, Replicated::default());
        world.insert(entity, Health(10));
        world.insert(entity, Sprite2D::default().with_sort_order(7));
        entity
    };
    let on_server = level(rig.world());
    let peer = rig.join(|world| {
        level(world);
    });
    owned(rig.world(), Vec2::ZERO, peer);
    let (shield, shield_id) = spawn(rig.world(), Vec2::new(-2.0, 0.0));
    rig.world().insert(shield, Health(1));
    rig.step(60);
    let adopted = copy(rig.clients[0].world(), crate_id).expect("adopted");
    assert_eq!(
        rig.clients[0].world().get::<Health>(adopted),
        Some(&Health(10))
    );

    // Out of interest; changed while away: moved, hurt, a component gone.
    place(rig.world(), on_server, Vec2::new(50.0, 0.0));
    place(rig.world(), shield, Vec2::new(-50.0, 0.0));
    rig.step(60);
    assert!(copy(rig.clients[0].world(), crate_id).is_none());
    rig.world().insert(on_server, Health(3));
    rig.world()
        .get_mut::<Transform2D>(on_server)
        .expect("a transform")
        .rotation = 1.5;
    rig.world().remove::<Health>(shield);
    rig.step(30);

    place(rig.world(), on_server, Vec2::new(4.0, 0.0));
    place(rig.world(), shield, Vec2::new(-4.0, 0.0));
    rig.step(90);
    let client = rig.clients[0].world();
    let back = copy(client, crate_id).expect("back");
    assert_eq!(client.get::<Health>(back), Some(&Health(3)), "what changed");
    assert_eq!(
        client
            .get::<Transform2D>(back)
            .map(|t| (t.translation, t.rotation)),
        Some((Vec2::new(4.0, 0.0), 1.5))
    );
    assert_eq!(
        client.get::<Sprite2D>(back).map(|s| s.sort_order),
        Some(7),
        "the level's sprite, the client's own, kept"
    );
    let shield_back = copy(client, shield_id).expect("back too");
    assert_eq!(
        client.get::<Health>(shield_back),
        None,
        "removed while away stays removed"
    );
    let copies = client
        .query::<SceneId>()
        .filter(|(_, id)| **id == crate_id)
        .count();
    assert_eq!(copies, 1, "one copy");
}

#[test]
fn relevant_entities_share_the_budget_and_far_ones_cost_nothing() {
    let budget = 600;
    let config = ReplicationConfig {
        bytes_per_snapshot: budget,
        ..interest(10.0)
    };
    let mut rig = Rig::new(LinkConditions::default(), config);
    let peer = rig.join(|_| {});
    owned(rig.world(), Vec2::ZERO, peer);
    // A crowd far away moving every tick, and twenty near.
    let far: Vec<Entity> = (0..300)
        .map(|n| spawn(rig.world(), Vec2::new(100.0 + n as f32, 0.0)).0)
        .collect();
    let near: Vec<Entity> = (0..20)
        .map(|n| spawn(rig.world(), Vec2::new(-5.0 + n as f32 * 0.5, 1.0)).0)
        .collect();
    rig.step(30);

    let mut largest = 0;
    let mut cut = 0;
    for frame in 0..120 {
        for (n, &entity) in far.iter().chain(&near).enumerate() {
            if let Some(t) = rig.world().get_mut::<Transform2D>(entity) {
                t.translation.y = (frame as f32 * 0.1 + n as f32).sin();
            }
        }
        rig.step(1);
        let stats = rig
            .server
            .net()
            .replication()
            .and_then(|r| r.server())
            .and_then(|s| s.stats(peer))
            .expect("replicating");
        largest = largest.max(stats.snapshot_bytes);
        if !stats.complete {
            cut += 1;
        }
        assert_eq!(stats.relevant, 21);
    }
    assert!(largest <= budget, "a snapshot of {largest} bytes");
    assert_eq!(cut, 0, "the twenty near ones fit every snapshot");
    rig.step(30);
    let seen = replicated(rig.clients[0].world());
    assert_eq!(seen.len(), 21, "its own and the near ones, never the crowd");
    let truth = replicated(rig.server.world());
    for (id, transform) in &seen {
        assert_eq!(truth.get(id), Some(transform), "exactly the server's");
    }
}

/// `None` and a radius covering everything send snapshots of the same
/// size, frame by frame, and leave the same world. (Both are this
/// build's wire: `None` costs one byte a snapshot over stage 114's, the
/// empty cull list — `REPLICATION_WIRE_VERSION` 2.)
#[test]
fn interest_covering_everything_sends_what_none_does() {
    let run = |config: ReplicationConfig| {
        let mut rig = Rig::new(LinkConditions::default(), config);
        let peer = rig.join(|_| {});
        owned(rig.world(), Vec2::ZERO, peer);
        let movers: Vec<Entity> = (0..40)
            .map(|n| spawn(rig.world(), Vec2::new(n as f32 * 3.0, 0.0)).0)
            .collect();
        let mut bytes = Vec::new();
        for frame in 0..90 {
            for (n, &entity) in movers.iter().enumerate() {
                if let Some(t) = rig.world().get_mut::<Transform2D>(entity) {
                    t.translation = Vec2::new(n as f32 * 3.0, (frame as f32 * 0.2).sin());
                }
            }
            if frame == 45 {
                rig.world().despawn(movers[3]);
            }
            rig.step(1);
            let stats = rig
                .server
                .net()
                .replication()
                .and_then(|r| r.server())
                .and_then(|s| s.stats(peer))
                .expect("replicating");
            bytes.push((stats.snapshot_bytes, stats.relevant));
        }
        // Ids are fresh each run: the world compared by what is in it.
        let mut seen: Vec<Transform2D> = replicated(rig.clients[0].world()).into_values().collect();
        seen.sort_by(|a, b| {
            (a.translation.x, a.translation.y)
                .partial_cmp(&(b.translation.x, b.translation.y))
                .expect("finite")
        });
        (bytes, seen)
    };
    let (none, seen_none) = run(ReplicationConfig::default());
    let (all, seen_all) = run(interest(1.0e6));
    assert_eq!(seen_none.len(), 40, "its own and 39 movers");
    assert_eq!(seen_all, seen_none);
    assert_eq!(all, none, "the same snapshot sizes, frame by frame");
}
