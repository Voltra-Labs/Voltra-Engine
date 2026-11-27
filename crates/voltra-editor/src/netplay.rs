//! Play with a server and clients in one editor, over a network made bad
//! on purpose.
//!
//! With clients asked for, Play hosts the editor's own world as a listen
//! server — its game plays as the host, in the viewport, as in any play —
//! and starts that many clients in this process: each the game run
//! headless ([`Headless`]) on the scene as Play found it, joined over an
//! in-memory network. Every link, the server's and each client's, is a
//! [`Conditioned`] transport whose [`LinkControl`] the Network window
//! turns: latency, jitter, loss, a bandwidth cap — live, both ways. Stop
//! ends them with the rest of the session.
//!
//! Unity's Multiplayer Play Mode and Godot's "run multiple instances" start
//! clients as processes of their own, each a window; here they are worlds
//! in the editor's process, stepped in its frame, so their state is right
//! there to show and to read through the bridge, and the link between them
//! is one the editor owns end to end. What they do not have is a picture:
//! the renderer draws the editor's one world, so a client is shown as a map
//! of its replicated entities (`panels/network.rs`), not a viewport of its
//! own.

use std::time::Duration;

use voltra_core::net::{
    Conditioned, LinkConditions, LinkControl, MemoryNetwork, NetConfig, PeerId,
};
use voltra_core::replication::ReplicationConfig;
use voltra_core::{App, Headless, UiFrame, WindowConfig};

use crate::editor::Editor;

/// The most clients a play session starts: a party of four with the host.
pub(crate) const MAX_CLIENTS: usize = 3;

/// The editor's network play: what the next Play starts, and the session
/// running.
#[derive(Default)]
pub(crate) struct NetPlay {
    /// Clients the next Play starts; none plays offline, as before.
    pub(crate) clients: usize,
    /// How every link misbehaves, applied as it changes.
    pub(crate) conditions: LinkConditions,
    /// Whether the Network window is up.
    pub(crate) open: bool,
    session: Option<Session>,
}

/// One play session's clients and links.
struct Session {
    clients: Vec<Headless>,
    /// The server's link, then each client's.
    links: Vec<LinkControl>,
    /// The server's clock at the last step, which the clients follow.
    clock: Duration,
}

impl NetPlay {
    /// The clients running, in the order they joined.
    pub(crate) fn clients(&self) -> &[Headless] {
        self.session.as_ref().map_or(&[], |s| &s.clients)
    }

    /// Whether a session with clients is running.
    pub(crate) fn is_running(&self) -> bool {
        self.session.is_some()
    }
}

/// Starts the clients asked for, if any, as Play leaves editing: the
/// editor's world hosts, and each client joins with the scene as it is now.
pub(crate) fn begin(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    let wanted = editor.netplay.clients.min(MAX_CLIENTS);
    if wanted == 0 {
        return;
    }
    let scene = match voltra_scene::format::to_scene_file(frame.world, &editor.registry) {
        Ok(scene) => scene,
        Err(error) => {
            log::error!("network play: the scene would not copy to its clients: {error}");
            return;
        }
    };
    let conditions = editor.netplay.conditions;
    let network = MemoryNetwork::new();
    let server = Conditioned::new(network.endpoint(), conditions, 1);
    let address = server.inner().address();
    let mut links = vec![server.control()];
    if !frame.net.replicates() {
        frame.net.enable_replication(ReplicationConfig::default());
    }
    frame.net.host_on(server, NetConfig::default());
    if let Some(endpoint) = frame.net.endpoint_mut() {
        endpoint.set_host_plays(true);
    }

    let root = frame.textures.root().to_path_buf();
    let mut clients = Vec::with_capacity(wanted);
    for n in 0..wanted {
        let mut app = App::new(WindowConfig {
            title: format!("client {}", n + 1),
            ..WindowConfig::default()
        })
        .with_asset_root(root.clone())
        .with_replication(ReplicationConfig::default());
        if let Some(game) = editor.game.new_game() {
            app = app.with_game(game);
        }
        if let Err(error) =
            voltra_scene::format::from_scene_file(&scene, &editor.registry, &mut app.world)
        {
            log::error!(
                "network play: client {} could not load the scene: {error}",
                n + 1
            );
            continue;
        }
        let mut client = Headless::new(app).unprofiled();
        let link = Conditioned::new(network.endpoint(), conditions, 10 + n as u64);
        links.push(link.control());
        client
            .net_mut()
            .join_on(link, address, NetConfig::default());
        clients.push(client);
    }
    log::info!(
        "network play: hosting, {} client{} joining",
        clients.len(),
        if clients.len() == 1 { "" } else { "s" }
    );
    editor.netplay.session = Some(Session {
        clients,
        links,
        clock: frame.net.now(),
    });
}

/// Steps every client by the time the server's frame took, simulating while
/// the editor plays, and puts the conditions on every link.
pub(crate) fn step(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    let simulating = frame.simulating();
    let conditions = editor.netplay.conditions;
    let Some(session) = editor.netplay.session.as_mut() else {
        return;
    };
    for link in &session.links {
        link.set(conditions);
    }
    let now = frame.net.now();
    let delta = now.saturating_sub(session.clock).as_secs_f32();
    session.clock = now;
    for client in &mut session.clients {
        client.set_simulating(simulating);
        client.step(1, delta);
    }
}

/// Ends the session: the clients leave, the host stops hosting.
pub(crate) fn end(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    if editor.netplay.session.take().is_some() {
        frame.net.stop();
    }
}

/// A change to the settings, each field left out kept.
pub(crate) struct Settings {
    pub(crate) clients: Option<usize>,
    pub(crate) latency_ms: Option<f64>,
    pub(crate) jitter_ms: Option<f64>,
    pub(crate) loss: Option<f32>,
    pub(crate) bandwidth: Option<u32>,
}

/// Applies `settings`, refusing a value out of range rather than clamping
/// it into something the caller did not ask for. The clients count takes
/// effect at the next Play; the conditions at once.
pub(crate) fn configure(editor: &mut Editor, settings: Settings) -> Result<(), String> {
    let millis = |name: &str, value: f64| {
        if value.is_finite() && (0.0..=10_000.0).contains(&value) {
            Ok(Duration::from_secs_f64(value / 1000.0))
        } else {
            Err(format!("{name} must be 0 to 10000 ms, not {value}"))
        }
    };
    let mut next = editor.netplay.conditions;
    if let Some(latency) = settings.latency_ms {
        next.latency = millis("latency", latency)?;
    }
    if let Some(jitter) = settings.jitter_ms {
        next.jitter = millis("jitter", jitter)?;
    }
    if let Some(loss) = settings.loss {
        if !(0.0..=1.0).contains(&loss) {
            return Err(format!("loss is a share, 0 to 1, not {loss}"));
        }
        next.loss = loss;
    }
    if let Some(bandwidth) = settings.bandwidth {
        next.bandwidth = bandwidth;
    }
    if let Some(clients) = settings.clients {
        if clients > MAX_CLIENTS {
            return Err(format!("at most {MAX_CLIENTS} clients, not {clients}"));
        }
        editor.netplay.clients = clients;
    }
    editor.netplay.conditions = next;
    Ok(())
}

/// What the session is doing, for the bridge and the Network window.
pub(crate) fn status(editor: &Editor, frame: &UiFrame<'_>) -> serde_json::Value {
    use serde_json::json;
    let peer = |net: &voltra_core::Net, peer: PeerId| {
        net.stats(peer).map(|s| {
            json!({
                "peer": peer.0,
                "rtt_ms": s.rtt.as_secs_f64() * 1000.0,
                "bytes_sent": s.bytes_sent,
                "bytes_received": s.bytes_received,
                "packets_sent": s.packets_sent,
                "packets_received": s.packets_received,
            })
        })
    };
    let replication = frame.net.replication().and_then(|r| r.server());
    let server: Vec<_> = frame
        .net
        .peers()
        .collect::<Vec<_>>()
        .into_iter()
        .filter_map(|p| {
            let mut connection = peer(frame.net, p)?;
            connection["relevant"] =
                json!(replication.and_then(|s| s.stats(p)).map(|s| s.relevant));
            Some(connection)
        })
        .collect();
    let clients: Vec<_> = editor
        .netplay
        .clients()
        .iter()
        .map(|client| {
            let replica = client
                .net()
                .replication()
                .and_then(|r| r.client())
                .map(|c| c.stats());
            json!({
                "id": client.net().local_id().map(|p| p.0),
                "connection": peer(client.net(), PeerId::SERVER),
                "replicated": client.world().query::<voltra_scene::Replicated>().count(),
                "snapshots": replica.map(|r| r.snapshots),
                "out_of_interest": replica.map(|r| r.out_of_interest),
                "level_waiting": replica.map(|r| r.level_waiting),
                "delay_ms": replica.map(|r| r.delay.as_secs_f64() * 1000.0),
                "jitter_ms": replica.map(|r| r.jitter.as_secs_f64() * 1000.0),
            })
        })
        .collect();
    let c = editor.netplay.conditions;
    json!({
        "running": editor.netplay.is_running(),
        "clients_on_play": editor.netplay.clients,
        "conditions": {
            "latency_ms": c.latency.as_secs_f64() * 1000.0,
            "jitter_ms": c.jitter.as_secs_f64() * 1000.0,
            "loss": c.loss,
            "bandwidth": c.bandwidth,
        },
        "server": server,
        "clients": clients,
    })
}
