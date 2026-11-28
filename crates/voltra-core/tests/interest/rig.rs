//! The server, its clients and the helpers every interest test uses.

pub use std::collections::BTreeMap;
pub use std::time::Duration;

pub use serde::{Deserialize, Serialize};
pub use voltra_core::net::{Conditioned, LinkConditions, MemoryNetwork, NetConfig, PeerId};
pub use voltra_core::replication::{Interest, NetOwner, ReplicationConfig};
pub use voltra_core::{App, Game, Headless};
pub use voltra_ecs::{Entity, World};
pub use voltra_render::glam::Vec2;
pub use voltra_scene::{hierarchy, ComponentRegistry, Replicated, SceneId, Sprite2D, Transform2D};

pub const FRAME: f32 = 1.0 / 60.0;

/// A replicated component besides `Transform2D`, to change while away.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Health(pub u32);

pub struct Rules;

impl Game for Rules {
    fn register(&self, registry: &mut ComponentRegistry) {
        registry.register::<Health>("Health").replicated();
    }
}

pub fn lossy() -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::from_millis(20),
        loss: 0.2,
        duplicate: 0.02,
        bandwidth: 0,
    }
}

pub fn interest(radius: f32) -> ReplicationConfig {
    ReplicationConfig {
        interest: Some(Interest::new(radius)),
        ..ReplicationConfig::default()
    }
}

pub struct Rig {
    pub network: MemoryNetwork,
    pub address: voltra_core::net::Address,
    pub link: LinkConditions,
    pub config: ReplicationConfig,
    pub server: Headless,
    pub clients: Vec<Headless>,
}

impl Rig {
    pub fn new(link: LinkConditions, config: ReplicationConfig) -> Self {
        let network = MemoryNetwork::new();
        let transport = Conditioned::new(network.endpoint(), link, 1);
        let address = transport.inner().address();
        let mut server = Headless::new(
            App::default()
                .with_game(Rules)
                .with_replication(config.clone()),
        );
        server.net_mut().host_on(transport, NetConfig::default());
        Self {
            network,
            address,
            link,
            config,
            server,
            clients: Vec::new(),
        }
    }

    /// A client joined, and its peer once it is in. `level` runs on its
    /// world first: the level it loaded.
    pub fn join(&mut self, level: impl FnOnce(&mut World)) -> PeerId {
        self.join_with(|app| app, level)
    }

    /// [`join`](Self::join), its app shaped by `app` first.
    pub fn join_with(
        &mut self,
        app: impl FnOnce(App) -> App,
        level: impl FnOnce(&mut World),
    ) -> PeerId {
        let seed = 10 + self.clients.len() as u64;
        let transport = Conditioned::new(self.network.endpoint(), self.link, seed);
        let mut client = Headless::new(app(App::default()
            .with_game(Rules)
            .with_replication(self.config.clone())));
        level(client.world_mut());
        client
            .net_mut()
            .join_on(transport, self.address, NetConfig::default());
        self.clients.push(client);
        let index = self.clients.len() - 1;
        for _ in 0..120 {
            self.step(1);
            if let Some(peer) = self.clients[index].net().local_id() {
                return peer;
            }
        }
        panic!("the client never joined");
    }

    pub fn step(&mut self, frames: u32) {
        for _ in 0..frames {
            self.server.step(1, FRAME);
            for client in &mut self.clients {
                client.step(1, FRAME);
            }
        }
    }

    pub fn world(&mut self) -> &mut World {
        self.server.world_mut()
    }

    pub fn relevant(&self, peer: PeerId) -> usize {
        self.server
            .net()
            .replication()
            .and_then(|r| r.server())
            .and_then(|s| s.stats(peer))
            .expect("replicating to the client")
            .relevant
    }
}

pub fn spawn(world: &mut World, at: Vec2) -> (Entity, SceneId) {
    let entity = world.spawn();
    let id = SceneId::new();
    world.insert(entity, id);
    world.insert(entity, Transform2D::from_translation(at));
    world.insert(entity, Replicated::default());
    (entity, id)
}

pub fn owned(world: &mut World, at: Vec2, peer: PeerId) -> Entity {
    let (entity, _) = spawn(world, at);
    world.insert(entity, NetOwner(peer));
    entity
}

pub fn place(world: &mut World, entity: Entity, at: Vec2) {
    world
        .get_mut::<Transform2D>(entity)
        .expect("a transform")
        .translation = at;
}

/// The client's copy of `id`.
pub fn copy(world: &World, id: SceneId) -> Option<Entity> {
    world
        .query::<SceneId>()
        .find(|(_, other)| **other == id)
        .map(|(entity, _)| entity)
}

/// Every replicated entity's id and transform.
pub fn replicated(world: &World) -> BTreeMap<SceneId, Transform2D> {
    world
        .query::<Replicated>()
        .filter_map(|(entity, _)| {
            Some((
                *world.get::<SceneId>(entity)?,
                *world.get::<Transform2D>(entity)?,
            ))
        })
        .collect()
}
