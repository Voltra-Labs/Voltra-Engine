# 7. Export

A build is described by its `build.ron`: the game's name, its first scene,
its window, its asset folder, and the game crate when it has code. The
game made in chapter 3 has its own, `games/my-game/build.ron`.

## Native

```sh
cargo xtask dist --manifest games/my-game/build.ron
```

`dist/My Game/` holds the release binary, `build.ron` and the assets: zip
it and ship it. On Windows the binary opens no console, and the log goes
to `log.txt` beside it.

## The web

<!-- ci -->
```sh
cargo xtask dist --web --manifest games/my-game/build.ron --out target/guide-dist
test -f "target/guide-dist/My Game-web/index.html"
test -f "target/guide-dist/My Game-web/game_bg.wasm"
test -f "target/guide-dist/My Game-web/game.pak"
```

`My Game-web/` is four files — the page, the JavaScript glue, the wasm and
the game's pack — that any static server hosts. Try it locally:

```sh
cd "target/guide-dist/My Game-web"
python3 -m http.server 8000      # then open http://localhost:8000
```

It runs on WebGPU where the browser has it and WebGL2 everywhere else;
keys, pads, touch and sound all work. Upload the folder to itch.io, GitHub
Pages or any host, and the game is shipped.
