// {{title}}: what `cargo xtask dist --manifest games/{{package}}/build.ron`
// packages, and what the editor opens with `--game` this file.
(
    version: 1,
    name: "{{title}}",
    window: (width: 1280, height: 720),
    assets: "assets",
    game: (package: "{{package}}"),
)
