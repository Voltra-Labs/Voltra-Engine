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

`Transform` is replicated already. An entity spawned from a prefab is
spawned from the same prefab on each client — its sprite, collider and
children never cross the network — and an entity of the level the client
also loaded is adopted by its id. Each client gets only what changed since
it last acknowledged, the most important entities first
(`Replicated::priority`) inside a byte budget per snapshot
(`ReplicationConfig::bytes_per_snapshot`).

Remote entities are drawn a little in the past, between two snapshots, so
they move smoothly however the network bunches them: the delay is measured
from the link, a few tens of milliseconds on a good one.

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

## Trying it

The arena's dedicated server and two bot clients play a round to its
winner — the same match CI plays:

<!-- ci -->
```sh
cargo test -p arena --test match
```

The server on its own, for a few seconds (`--seconds`), on free ports:

<!-- ci -->
```sh
cargo run -p arena --bin arena-server -- --asset-root games/arena/assets --udp 0 --ws 0 --seconds 2
```

At a desk: `cargo run -p arena -- --host` in one terminal and `cargo run -p
arena -- --join udp://127.0.0.1:7777` in another. Or in the editor, without
a second machine or window: open the arena (`cargo run -p voltra-editor --
--game games/arena/build.ron scenes/main.ron`), set **Window ▸ Network ▸
Clients on Play** to 2, and press Play. The editor's world hosts and plays;
two clients run inside the editor, each drawn as a map of what it sees. Turn
up the latency, jitter, loss or cap the bandwidth there and watch the
clients cope; the Profiler's `net …` counters show the round trip and the
traffic, frame by frame.

A web build of the arena joins a server from its page's address:
`index.html?join=ws://HOST:7778`.
