//! Interest over the 3D world (stage 134): a 3D entity placed on the
//! ground plane by its (x, z) and kept by its distance in space, and
//! points that see only entities of their own dimension.

use std::time::Duration;

use voltra_core::net::{Conditioned, LinkConditions, MemoryNetwork, NetConfig, PeerId};
use voltra_core::replication::{Interest, NetOwner, ReplicationConfig};
use voltra_core::{App, Headless};
use voltra_ecs::{Entity, World};
use voltra_render::glam::{Vec2, Vec3};
use voltra_scene::{Replicated, SceneId, Transform2D, Transform3D};

const FRAME: f32 = 1.0 / 60.0;

fn link(
    seed: u64,
    network: &MemoryNetwork,
    latency: u64,
) -> Conditioned<voltra_core::net::MemoryTransport> {
    Conditioned::new(
        network.endpoint(),
        LinkConditions {
            latency: Duration::from_millis(latency),
            jitter: Duration::ZERO,
            loss: 0.0,
            duplicate: 0.0,
            bandwidth: 0,
        },
        seed,
    )
}

/// The client's copy of `id`.
fn copy(world: &World, id: SceneId) -> Option<Entity> {
    world
        .query::<SceneId>()
        .find(|(_, other)| **other == id)
        .map(|(entity, _)| entity)
}

/// A server culling by interest at 10 m, and one client of it.
struct Rig {
    server: Headless,
    client: Headless,
    peer: PeerId,
}

impl Rig {
    fn new() -> Self {
        let config = ReplicationConfig {
            interest: Some(Interest::new(10.0)),
            ..ReplicationConfig::default()
        };
        let network = MemoryNetwork::new();
        let transport = link(5, &network, 10);
        let address = transport.inner().address();
        let mut server = Headless::new(App::default().with_replication(config.clone()));
        server.net_mut().host_on(transport, NetConfig::default());
        let mut client = Headless::new(App::default().with_replication(config));
        client
            .net_mut()
            .join_on(link(6, &network, 10), address, NetConfig::default());
        for _ in 0..120 {
            server.step(1, FRAME);
            client.step(1, FRAME);
            if let Some(peer) = client.net().local_id() {
                return Self {
                    server,
                    client,
                    peer,
                };
            }
        }
        panic!("the client never joined");
    }

    fn step(&mut self, frames: u32) {
        for _ in 0..frames {
            self.server.step(1, FRAME);
            self.client.step(1, FRAME);
        }
    }

    fn spawn(&mut self, component: impl Fn(&mut World, Entity)) -> (Entity, SceneId) {
        let world = self.server.world_mut();
        let entity = world.spawn();
        let id = SceneId::new();
        world.insert(entity, id);
        world.insert(entity, Replicated::default());
        component(world, entity);
        (entity, id)
    }

    fn move_to(&mut self, entity: Entity, at: Vec3) {
        self.server
            .world_mut()
            .get_mut::<Transform3D>(entity)
            .expect("a 3D transform")
            .translation = at;
    }

    fn has(&self, id: SceneId) -> bool {
        copy(self.client.world(), id).is_some()
    }
}

fn at3(at: Vec3) -> impl Fn(&mut World, Entity) {
    move |world, entity| {
        world.insert(entity, Transform3D::from_translation(at));
    }
}

#[test]
fn a_3d_entity_enters_and_leaves_by_horizontal_distance_and_by_height() {
    let mut rig = Rig::new();
    let peer = rig.peer;
    // The client's own 3D player at the origin: its interest point.
    let (_player, player_id) = rig.spawn(move |world, entity| {
        world.insert(entity, Transform3D::default());
        world.insert(entity, NetOwner(peer));
    });
    let (walker, walker_id) = rig.spawn(at3(Vec3::new(5.0, 0.0, 0.0)));
    // Far along Z: 30 m on the ground plane's second axis, not its height.
    let (_far, far_id) = rig.spawn(at3(Vec3::new(0.0, 0.0, 30.0)));
    rig.step(30);
    assert!(rig.has(player_id) && rig.has(walker_id), "5 m: relevant");
    assert!(!rig.has(far_id), "30 m along Z: placed, and out of range");

    // Out across the ground: past the 11 m leave radius along Z.
    rig.move_to(walker, Vec3::new(0.0, 0.0, 12.0));
    rig.step(30);
    assert!(!rig.has(walker_id), "12 m along Z: left");
    // Back to 9.5 m, inside the radius: entered again.
    rig.move_to(walker, Vec3::new(0.0, 0.0, 9.5));
    rig.step(30);
    assert!(rig.has(walker_id), "9.5 m: back");
    // Straight up from 3 m out: 3 m across, 12 m up is 12.4 m away — the
    // cell is the same, the 3D distance is not.
    rig.move_to(walker, Vec3::new(3.0, 12.0, 0.0));
    rig.step(30);
    assert!(!rig.has(walker_id), "12.4 m away overhead: left by height");
    rig.move_to(walker, Vec3::new(3.0, 9.0, 0.0));
    rig.step(30);
    assert!(rig.has(walker_id), "9.5 m away overhead: back");
}

#[test]
fn a_point_sees_only_entities_of_its_own_dimension() {
    let mut rig = Rig::new();
    let peer = rig.peer;
    // A 2D entity at (1, 0) and a 3D one whose x and y are (1, 0) too, but
    // 40 m along Z.
    let (_flat, flat_id) = rig.spawn(|world, entity| {
        world.insert(entity, Transform2D::from_translation(Vec2::new(1.0, 0.0)));
    });
    let (_deep, deep_id) = rig.spawn(at3(Vec3::new(1.0, 0.0, 40.0)));
    // A 3D one near a 3D point at the origin.
    let (_near, near_id) = rig.spawn(at3(Vec3::new(0.0, 1.0, 2.0)));
    rig.step(20);
    assert!(
        !rig.has(flat_id) && !rig.has(deep_id) && !rig.has(near_id),
        "no point, nothing placed is sent"
    );

    assert!(rig.server.net_mut().set_interest_points(peer, [Vec2::ZERO]));
    rig.step(30);
    assert!(rig.has(flat_id), "a 2D point sees the 2D entity");
    assert!(!rig.has(deep_id), "and not a 3D one at the same (x, y)");
    assert!(!rig.has(near_id), "nor one near its (x, z)");

    assert!(rig.server.net_mut().set_interest_points(peer, [Vec3::ZERO]));
    rig.step(30);
    assert!(rig.has(near_id), "a 3D point sees the 3D entity 2.2 m off");
    assert!(!rig.has(flat_id), "and not the 2D one 1 m off");
    assert!(!rig.has(deep_id), "nor the 3D one 40 m off");
}
