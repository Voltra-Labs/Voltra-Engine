# Dodge

A Voltra game, made with `cargo xtask new-game dodge`.

```sh
cargo run -p dodge                                          # play it
cargo build -p voltra-editor -p dodge                       # build the editor with it
cargo run -p voltra-editor -- --game games/dodge/build.ron scenes/main.ron
cargo test -p dodge                                         # load it as the editor does
cargo xtask dist --manifest games/dodge/build.ron           # package it
cargo xtask dist --web --manifest games/dodge/build.ron     # for a browser
```

The code is `src/lib.rs`; the first scene is `assets/scenes/main.ron`. In
the editor, Play runs the game, and saving a change to `src/` rebuilds it
and swaps it in while it plays.
