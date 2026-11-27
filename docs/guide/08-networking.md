# 8. Networking

A networked Voltra game has one authority — a server — and clients that
send it their inputs and draw what it sends back. The sample `games/arena`
is the whole shape in a few hundred lines: two to four players race for a
coin, first to five.

## A session

`tick.net` is the game's session. Turn replication on, then host or join:

```rust
fn start(&mut self, tick: &mut Tick<'_>) {
    tick.net.enable_replication(ReplicationConfig::default());
    // A listen server: this run hosts on UDP 7777 (native players) and
    // WebSocket 7778 (browsers), and plays too.
    tick.net.host(&HostOptions { listen: true, ..HostOptions::default() })
        .expect("the ports are free");
    // Or, elsewhere: tick.net.join("udp://192.168.1.20:7777", NetConfig::default())
}
```

Every end must make its session the same way — the handshake refuses a
client whose channels or `NetConfig::protocol` differ. In a browser only
`ws://` (or `wss://`) can be joined, and nothing can host.

`tick.net.events()` says who came and went this frame. On a listen server
the host's own player joins first, as `PeerId::SERVER`, so one piece of
code spawns every player:

```rust
for event in tick.net.events().to_vec() {
    if let NetEvent::Connected(peer) = event {
        let prefab = tick.prefab(&player_prefab);
        let root = tick.spawn(prefab, &start).expect("spawns").root().expect("a root");
        tick.world.insert(root, NetOwner(peer));
    }
}
```

## Replication

The server sends the components a game marks replicated, on entities
marked `Replicated`:

```rust
fn register(&self, registry: &mut ComponentRegistry) {
    registry.register::<Score>("Score").replicated();
}
```

`Transform2D` is replicated already. An entity spawned from a prefab is
spawned from the same prefab on each client — its sprite, collider and
children never cross the network — and an entity of the level the client
also loaded is adopted by its id. Each client gets only what changed since
it last acknowledged, the most important entities first
(`Replicated::priority`) inside a byte budget per snapshot
(`ReplicationConfig::bytes_per_snapshot`).

Remote entities are drawn a little in the past, between two snapshots, so
they move smoothly however the network bunches them: the delay is measured
from the link, a few tens of milliseconds on a good one.

## Only what is near: interest

On a big map, a player has no use for what happens on the far side of it,
and sending it costs every player bandwidth. Turn interest management on
and each client is sent only the entities near what it watches:

```rust
ReplicationConfig {
    interest: Some(Interest::new(30.0)), // world units
    ..ReplicationConfig::default()
}
```

What a client watches is, by default, the entities it owns (`NetOwner`).
For a spectator or a camera that roams, give the server points of your
own for that player, each tick if they move:

```rust
tick.net.set_interest_points(peer, vec![camera_position]);
```

An entity that drifts out of range is despawned on that client and spawned
again, with everything that changed meanwhile, when it comes back; an
entity of the level comes back with what the client loaded it with (its
sprite, say). It enters at the radius and leaves a little further out (10 %
by default, `Interest::hysteresis`), so one on the edge does not blink. A
child is sent exactly when the replicated entity at the top of its
hierarchy is. Your own player is always sent to you.

Some things every player must see wherever they are — the score, the
objective. Tick **always relevant** on their `Replicated` component in the
inspector (`always_relevant: true` in the file), or give one entity a
radius of its own (`relevance_radius`). An entity with no `Transform2D`, like
a match's state, is sent to everyone. The Network window shows how many
entities each client is sent and draws its radius on its map.

## Your own player: prediction

Waiting a round trip for the server to move your own player feels like
wading through mud. Instead the client moves it at once and checks with the
server later. Write the movement once, as a function of the entity, its
input and the step:

```rust
fn walk(world: &mut World, entity: Entity, steer: &Steer, step: f32) { /* … */ }
```

The server runs it with each client's input for that step —
`tick.net.input::<Steer>(peer)` in its fixed tick — and a client runs it
through its prediction in its fixed tick:

```rust
tick.predict(&mut self.prediction, steer, walk);
```

When the server's answer arrives, the client puts its player back where the
server says and replays the inputs the server has not seen yet: if both ran
the same steps, nothing moves on screen. Keep `walk` reading only the
entity's own replicated components, the input and the step, and move the
player kinematically rather than with physics forces — the replay runs
`walk`, not the physics step.

## Shots where the shooter saw them: lag compensation

Because remote players are drawn a little in the past, a client aims at
where its target *was*: a round trip and the interpolation delay ago. A
server that tested the shot against where the target is now would make
every player on a slow link lead their targets. Instead it rewinds: mark
what can be shot **lag compensated** (the checkbox under *Replicated* in
the inspector, `"LagCompensated": ()` in the file — it needs a
`Collider2D`), and in the server's fixed tick ask the world as the shooter
saw it when it made this step's input:

```rust
if let Some(steer) = tick.net.input::<Steer>(peer) {
    let seen = tick.net.rewound(tick.world, peer);
    if let Some(hit) = seen.ray(from, aim, range, QueryFilter::new().excluding(me)) {
        // hit.entity was there when this player fired
    }
}
```

`rewound` answers the same `ray`, `cast`, `point` and `overlap_aabb` as
`query`, with every lag-compensated collider where it was at the moment
the client says it was drawing — each input carries it — and everything
else (walls, unmarked entities) where it is. Nothing in the world moves.
A shot is rewound no further than the server measured that player's lag
to be — its round trip, how much that varies, the snapshot rate and how
long its input waited, with a little slack — and never more than half a
second (`LagCompensation::max_rewind`), whatever the client claims. So a
player on a quick link cannot pretend to be slow and shoot into the past,
and a player on a slow one is rewound as far as their link really is, up
to that cap; a client claiming the future gets the present. Something
spawned after that moment, or despawned since, is not there to hit; with
interest management on, neither is any replicated collider the client was
not sent (the level's own walls, loaded on every end, always are). The
server keeps a second of history by default (`LagCompensation::history`);
`lag_compensation: None` in the `ReplicationConfig` keeps none, and every
rewound query is live.

Move lag-compensated entities in the fixed tick (`fixed_update`), as you
move anything the server replicates: their poses are recorded after each
fixed step, so a move made in the per-frame `update` is not in the
history until the next step records it, and a shot is tested against
where the step left it.

The arena's zap is one: a bot zaps the nearest other player where it sees
it, and the server's log says how far back each zap was tested.

## Before the match: a lobby

A match rarely starts the moment someone connects. Turn the lobby on — on
every end, before hosting or joining, as replication is — and the server
holds the room: who is in, under what name, who is ready, and who starts
it.

```rust
tick.net.enable_lobby(LobbyConfig {
    name: "Ada".into(),   // this player's name: cut, cleaned, made unique
    min_players: 2,       // a match starts with at least two
    max_players: 4,       // one more is refused: "the lobby is full"
    late_join: LateJoin::Refuse, // or Play: drop into a match under way
    ..LobbyConfig::default()
});
```

`tick.net.lobby()` is the room as this end knows it, and what this end's
player may ask; `tick.net.lobby_events()` what changed this frame:

```rust
if let Some(lobby) = tick.net.lobby_mut() {
    if !lobby.is_ready() {
        lobby.set_ready(true)?;
    }
    if lobby.is_host() {
        lobby.start()?; // refused until everyone is ready, and there are enough
    }
}
for event in tick.net.lobby_events().to_vec() {
    match event {
        LobbyEvent::Started => { /* load the level, spawn a player per entry */ }
        LobbyEvent::Left { peer } => { /* take their player away */ }
        _ => {}
    }
}
```

The host is the listen server's own player — and its leaving ends the
lobby for everyone — or, on a dedicated server, the first player in,
passed to the next in join order when they go. A dedicated server's game
may start the match itself (`start()` there needs no host's word, only
the readiness). What "start" does is the game's: the engine only says
when. Lobby messages travel on a channel of their own, so
`tick.net.received()` never holds one; a client of another build's lobby
is refused at the handshake. A peer that connects and never joins is cut
off after the session's timeout plus `JOIN_GRACE` (12 s by default;
`join_timeout` may lengthen it, never shorten it below that — a client
the session keeps through an outage keeps its place), and with the lobby
on, replication sends the world only to the lobby's players.

The lobby screen does the clicking: `assets/prefabs/lobby.prefab.ron`
(every `cargo xtask new-game` copies it) with `LobbyScreen::open(tick,
&LobbyScreenConfig::for_game("my-game"))`, its `update(tick)` each frame
until it answers `LobbyOutcome::Started`. It hosts, finds games on the
LAN, joins by address, and in the room shows the players, Ready, and
Start for the host. Restyle it in the editor; keep the parts' names. Names
are other people's words: the rows and the status line are plain text
(`rich: false`), not markup — a name of `[size=999]` shows as typed.

## Finding a game on the LAN

A host that sets `HostOptions::discovery` answers probes on UDP 7779
(`DEFAULT_DISCOVERY_PORT`) with its name, players and fingerprint; a
client asks the whole network and lists what answers:

```rust
tick.net.host(&HostOptions {
    listen: true,
    discovery: Some(Advertise { name: "Ada's game".into(), game: "my-game".into(), ..Advertise::default() }),
    ..HostOptions::default()
})?;
// Elsewhere:
tick.net.discover_game("my-game", NetConfig::default())?;
// ...frames later:
for server in tick.net.discovered() {
    // server.name, server.players / server.max_players, server.address()
    // server.compatible: false for another build — joining it would be refused
}
```

A search probes for `SearchOptions::timeout` (1.5 s), resending as it
goes. A network that drops broadcasts (some VPNs and containers) is
reached by naming addresses in `SearchOptions::targets`. A server answers
each source a few times a second at most, with no more bytes than the
probe brought, and only sources on the local network unless its
`Advertise::scope` is `DiscoveryScope::Anyone`. In a browser there is no
UDP: join by address.

The tests probe over loopback, which every machine passes:

<!-- ci -->
```sh
cargo test -p voltra-net --test lobby --test lobby_peers --test discovery
```

## Trying it

The arena's dedicated server and two bot clients find it on the LAN,
ready up in its lobby, and play a round to its winner — the same match CI
plays:

<!-- ci -->
```sh
cargo test -p arena --test match
```

The server on its own, for a few seconds (`--seconds`), on free ports:

<!-- ci -->
```sh
cargo run -p arena --bin arena-server -- --asset-root games/arena/assets --udp 0 --ws 0 --seconds 2
```

The arena's server is `voltra-server`'s command line with the arena's game
in it (`games/arena/src/bin/server.rs`, a few lines). `voltra-server`
itself serves any scene with no game code — the same flags, `--help` for
the rest (`--tick`, `--max-clients`, `--protocol`, `--bind`, `--lobby`,
`--min-players`, `--max-players`, and `--discovery PORT|off`, `--name`,
`--game`, `--discovery-public` for the LAN):

<!-- ci -->
```sh
cargo run -p voltra-server -- scenes/sandbox.ron --udp 0 --ws 0 --seconds 2
```

Without `--udp`/`--ws` it listens on 7777 and 7778, answers LAN probes
on 7779, and without `--seconds` it runs until stopped. It answers probes
only from the local network (private, link-local, loopback and VPN-mesh
addresses): a broadcast never crosses a router, so a probe from anywhere
else is a stranger's. `--discovery-public` answers anyone, for a server
you find by address across networks.

At a desk: `cargo run -p arena` opens the lobby screen — Host in one
window, Find on LAN (or Join by address) in another, Ready in both, Start
on the host. `cargo run -p arena -- --host` and `-- --join
udp://127.0.0.1:7777` skip the menu. Or in the editor, without
a second machine or window: open the arena (`cargo run -p voltra-editor --
--game games/arena/build.ron scenes/main.ron`), set **Window ▸ Network ▸
Clients on Play** to 2, and press Play. The editor's world hosts and plays;
two clients run inside the editor, each drawn as a map of what it sees. Turn
up the latency, jitter, loss or cap the bandwidth there and watch the
clients cope; the Profiler's `net …` counters show the round trip and the
traffic, frame by frame.

A web build of the arena joins a server from its page's address:
`index.html?join=ws://HOST:7778` (a page cannot search the LAN, nor host).

A server can stream a big world around every player it hosts, and its
clients stream the same scenery: [chapter 10](10-big-worlds.md).
