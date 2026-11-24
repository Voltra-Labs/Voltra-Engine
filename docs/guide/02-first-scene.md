# 2. A first scene

```sh
cargo run -p voltra-editor
```

## The editor

- **Hierarchy** (left): every entity in the scene. Drag one onto another to
  parent it.
- **Viewport** (middle): the scene. Middle-drag pans, the wheel zooms.
  `W`, `E` and `R` pick move, rotate and scale; the gizmo on the selection
  does the rest. Edit ▸ Snap to grid snaps the gizmo (Ctrl flips it).
- **Inspector** (right): the selected entity's components, each field
  editable. Every edit is one undo step (`Ctrl+Z`).
- **Assets** (bottom): the files under `assets/`. Drag a PNG into the
  viewport to make a sprite of it, a sound to make a source.
- **Window** menu: the Profiler, the Console (the engine's log, filtered by
  level), and Project Settings (the input map, the sorting layers and
  `build.ron`, saved as the files they are).

## Make something

1. Scene ▸ New.
2. Drag `sprites/checker.png` from the assets into the viewport: a sprite.
3. In the inspector, tint it — `Sprite ▸ colour` — and scale it.
4. Physics ▸ Spawn falling body, twice; then press **Play** in the toolbar.
   The bodies fall and land. **Stop** puts everything back.
5. Scene ▸ Save As… `scenes/first.ron`.

A scene is a RON file — one entry per entity, a map of components — that
reads well in a diff and merges in version control. Open it in a text
editor and look.

## Play it without the editor

```sh
cargo run -p voltra-player -- assets/scenes/first.ron
```

The player runs a scene with no editor linked: physics, animation, audio,
the HUD. A scene alone can be a whole small game — the next chapter adds
code of your own.
