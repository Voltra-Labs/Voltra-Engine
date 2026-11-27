# Arena

Two to four players race for a coin and zap each other (Space, aimed at
the nearest player; a zap that lands is worth a coin), first to five —
Voltra's networked sample, made with `cargo xtask new-game arena` and grown. See
`docs/guide/08-networking.md` for how it is put together.

```sh
cargo run -p arena                                          # play alone
cargo run -p arena -- --host                                # host a match (UDP 7777, WebSocket 7778) and play
cargo run -p arena -- --join udp://127.0.0.1:7777           # join one
cargo run -p arena --bin arena-server -- --asset-root games/arena/assets   # a dedicated server
cargo test -p arena                                         # a server and two bots play a round
cargo build -p voltra-editor -p arena                       # build the editor with it
cargo run -p voltra-editor -- --game games/arena/build.ron scenes/main.ron
cargo xtask dist --web --manifest games/arena/build.ron     # for a browser: index.html?join=ws://HOST:7778
```

The rules are `src/authority.rs` (run on the server, or alone); a client
predicts its own player with `src/movement.rs`'s `walk` and is sent the
rest. A zap (`src/zap.rs`) is tested where its shooter saw its target:
the server rewinds the players' colliders to that moment (lag
compensation). In the editor, `Window ▸ Network` plays it with clients over a
network made as bad as you like.
