# 3. A first game

A game's code is a crate in `games/`: a library the editor loads, plays and
reloads while you edit it, and a binary that ships.

<!-- ci -->
```sh
cargo xtask new-game my-game --title "My Game"
cargo test -p my-game
```

`new-game` writes `games/my-game/`: its `Cargo.toml`, `src/lib.rs`, a
`src/main.rs`, a first scene in `assets/scenes/main.ron`, the engine's
starting assets — among them the options and lobby screens
([chapter 5](05-ui.md#an-options-screen),
[chapter 8](08-networking.md#before-the-match-a-lobby)) — a `build.ron`,
and tests: one loads the library as the editor does, one walks the lobby
screen from host to start. Their passing means the game builds and plays.

## The game

`src/lib.rs` holds one value implementing `voltra_core::Game`:

```rust
#[derive(Default)]
pub struct MyGame;

impl Game for MyGame {
    fn register(&self, registry: &mut ComponentRegistry) {
        registry.register::<Player>("Player").with_template();
    }

    fn fixed_update(&mut self, tick: &mut Tick<'_>) {
        let direction = Vec2::new(
            tick.input.action_axis("move_left", "move_right"),
            tick.input.action_axis("move_down", "move_up"),
        );
        for (_, transform) in tick.world.query_mut::<Transform>() {
            transform.translation += direction * 3.0 * tick.delta;
        }
    }
}

voltra_core::export_game!(MyGame);
```

- **`register`** names the game's components, so scenes can hold them and
  the inspector can add and edit them. A component is any `Serialize +
  Deserialize` struct; state that must survive a reload — a score — lives
  on one.
- **`update`** runs once a frame: read input edges (`pressed()`), update
  text. **`fixed_update`** runs once a physics step: move things, read
  collision events. **`start`** runs once, when play begins.
- **`tick`** is everything a turn may touch: `world` (query, spawn,
  despawn), `input` (actions from `assets/default.input.ron`) and
  `actions` (the map itself, to rebind), `events` (collisions), `audio`,
  `ui` (what was clicked), `tween`/`after`/`every` (tweens and timers),
  `locale` (the language), `saves`, `window` (fullscreen, size, vsync),
  `net` (chapter 8), and scene changes and streamed worlds (chapter 10).

## In the editor

```sh
cargo build -p voltra-editor -p my-game
cargo run -p voltra-editor -- --game games/my-game/build.ron scenes/main.ron
```

Press Play: the editor runs your game. Leave it playing, change a number in
`src/lib.rs` and save — the editor rebuilds the crate and swaps the new
code in mid-play, the scene carried across. A build error is shown in the
Game menu, and the last good code keeps running.

The editor and the game must be built together (`-p voltra-editor -p
my-game`): the editor checks a library was built by the same compiler,
engine and settings before it loads it, and says why when it refuses.

## Run it on its own

```sh
cargo run -p my-game
```

## Test it

`tests/plugin.rs` plays the scene headless with a key held and checks the
player moved. `voltra_core::Headless` runs any scene without a window:
step it, press actions, feed it touches and a screen, read the world. The
sample games' `tests/level.rs` show more.
