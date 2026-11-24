# Breakout

A Voltra game, made with `cargo xtask new-game breakout`.

```sh
cargo run -p breakout                                          # play it
cargo build -p voltra-editor -p breakout                       # build the editor with it
cargo run -p voltra-editor -- --game games/breakout/build.ron scenes/main.ron
cargo test -p breakout                                         # load it as the editor does
cargo xtask dist --manifest games/breakout/build.ron           # package it
cargo xtask dist --web --manifest games/breakout/build.ron     # for a browser
```

The code is `src/lib.rs`; the first scene is `assets/scenes/main.ron`. In
the editor, Play runs the game, and saving a change to `src/` rebuilds it
and swaps it in while it plays.
