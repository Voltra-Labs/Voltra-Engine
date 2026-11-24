# {{title}}

A Voltra game, made with `cargo xtask new-game {{package}}`.

```sh
cargo run -p {{package}}                                          # play it
cargo build -p voltra-editor -p {{package}}                       # build the editor with it
cargo run -p voltra-editor -- --game games/{{package}}/build.ron scenes/main.ron
cargo test -p {{package}}                                         # load it as the editor does
cargo xtask dist --manifest games/{{package}}/build.ron           # package it
cargo xtask dist --web --manifest games/{{package}}/build.ron     # for a browser
```

The code is `src/lib.rs`; the first scene is `assets/scenes/main.ron`. In
the editor, Play runs the game, and saving a change to `src/` rebuilds it
and swaps it in while it plays.
