# 10. Big worlds and loading screens

A level that fits in memory and loads in a blink is one scene. A world too
big for that is cut into **regions**, each an ordinary scene, loaded around
the player as they go and let go behind them. And a level change that takes
a moment puts up a **loading screen** rather than freezing the window.

Both load over several frames: files are read and checked on worker
threads, and the world is filled a little each frame within a budget
(4 ms by default, `App::with_load_budget`), so the game keeps drawing at
its rate while it loads. In a browser, with no threads, the same work runs
on the main thread within the same budget.

## A world of regions

Make each region a scene in the editor, its entities placed where they
stand in the world. Then list them in a `.world.ron`:

```ron
(
    base: Some(Path("scenes/overworld.ron")),
    load_radius: 40.0,
    unload_margin: 10.0,
    regions: [
        (scene: Path("scenes/overworld/0_0.ron"), min: (0.0, 0.0), max: (32.0, 32.0)),
        (scene: Path("scenes/overworld/1_0.ron"), min: (32.0, 0.0), max: (64.0, 32.0)),
    ],
)
```

- **`regions`**: each one's scene and the rectangle of the world it covers.
- **`load_radius`**: a region this close to an anchor is loaded (32 units
  if left out).
- **`unload_margin`**: how much further an anchor must go before the region
  is let go (8 if left out), so walking along an edge does not load and
  unload it every few frames.
- **`base`**: the scene the game plays in — the player, the camera, the HUD.
  It is not loaded by the streaming; it is what the editor draws the region
  bounds over (Scene ▸ Show region bounds).

Start streaming from the game, usually in `start`:

```rust
let world = AssetPath::new("worlds/overworld.world.ron").expect("a valid path");
tick.stream_world(&world);
```

Regions are loaded around every entity with a **`StreamingAnchor`** —
the player, or each player in split screen or on a server — each keeping
regions within its own `radius` (or the file's `load_radius`). With no
anchor in the world, the active cameras are the anchors, so a
single-player game need add nothing. The nearest regions load first.

A region that unloads takes with it everything it spawned, and everything
parented under that since; what it held is not kept, so a game that wants
a region remembered saves that itself. An entity marked **`Persistent`** is
lifted out and stays. `tick.streaming()` says how many regions are loaded,
loading and unloading, and how many entities they hold;
`tick.stop_streaming()` lets every region go.

On a networked server, give each player a `StreamingAnchor`: the server
streams around all of them, and a region's replicated entities reach the
clients as any others do. Put those in regions as prefab instances.

## A loading screen

```rust
let level = AssetPath::new("scenes/castle.ron").expect("a valid path");
let screen = AssetPath::new("prefabs/loading.prefab.ron").expect("a valid path");
tick.change_scene(&level, Some(&screen));
```

The prefab — keep it small, it is loaded on the spot — is spawned over the
old scene at the end of the frame. The new scene is read and checked off
the main thread, then the old one goes (all but `Persistent` roots and the
screen), the new one arrives within each frame's budget, and the screen
comes down. Physics and `fixed_update` wait until the level is whole;
`update` keeps running, to draw the bar:

```rust
use voltra_core::SceneChangeStatus;

match tick.scene_change() {
    Some(SceneChangeStatus::Loading { progress }) => { /* a bar `progress` (0 to 1) wide */ }
    Some(SceneChangeStatus::Finished) => { /* the new scene is in: this frame only */ }
    Some(SceneChangeStatus::Failed) => { /* it would not load; the old scene stands */ }
    None => {}
}
```

A file that will not load — missing, or a component that does not read —
is found before anything is touched, and the old scene stays. With `None`
for the screen the change happens the same way, with nothing over it.
`tick.load_scene(&path)` is still there for a small level: it changes in
one frame.
