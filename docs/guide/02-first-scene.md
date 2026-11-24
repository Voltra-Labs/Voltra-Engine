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
- **Window** menu: the Profiler, the Network window (chapter 8), the
  Console (the engine's log, filtered by level), Project Settings (the
  input map, the sorting layers and the audio buses, saved as the files
  they are), the Tile Palette and the Atlas Editor (below).

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

## Find things

The hierarchy and the asset browser each have a search box, with Unity's
syntax: words a name holds (every word, case aside), and `t:` for a type.
In the hierarchy `t:` is a component — `t:Camera`, `t:Sprite t:Collider`
(both) — and a match's parents stay in the tree, dimmed, so you see where
it lives. In the asset browser the search covers every folder, and `t:` is
a kind of file: `texture`, `atlas`, `sound`, `font`, `scene`, `prefab`,
`material`, `animation`, `animator`, `input`, `layers`, `buses`, `folder`.
A `t:` nobody knows says so rather than showing an empty list.

## Prefabs

Select an entity and choose Scene ▸ Create prefab from selection: its
subtree is written to `<name>.prefab.ron` in the asset browser's folder,
and the entity becomes an instance of it. Drag the file into the viewport
for more. Each instance follows the file, except for what it changes
itself — the inspector lists those as overrides, with **Revert** for each
and **Apply to prefab** to push them into the file.

To change the prefab itself, double-click it in the asset browser (or
**Edit prefab** in an instance's inspector, or right-click any scene's
tile, "Edit as prefab"). The scene is set aside and the prefab opens alone, with its
own undo history; the toolbar shows the trail (`level.ron ›
coin.prefab.ron`). Edit it with every tool, **Save**, then **◀ Back**: the
scene comes back as it was, undo history and all, and every instance has
the prefab's changes. An instance inside the prefab opens in turn, one
level deeper.

## Atlases

An atlas is a sheet of pictures and the `.atlas.ron` that cuts it into
frames; a sprite names its frame by index. There are two ways to make one.

**Pack a folder**: every PNG under it becomes one sheet — right-click the
folder in the asset browser, "Pack into atlas" (or "…, trimmed"), or from a
shell:

<!-- ci -->
```sh
cargo xtask pack-atlas assets/sprites/walker --trim --out target/guide-atlas/walker.atlas.ron --root target/guide-atlas
test -f target/guide-atlas/walker.atlas.ron
test -f target/guide-atlas/walker.png
grep -q 'name: Some("head")' target/guide-atlas/walker.atlas.ron
```

Without `--out` the sheet and the atlas go beside the folder
(`sprites/walker.png`, `sprites/walker.atlas.ron`); `--root` is the asset
root the atlas names its sheet against (`assets/` by default). Each frame
is named after its file (`arm`, `head`, …), and packing again after files
come and go keeps every old frame's index, so sprites still show what they
showed. `--trim` cuts transparent borders off and remembers where the
picture sat; `--padding` and `--extrude` keep neighbours from bleeding into
each other; `--max-size` caps the sheet (4096 by default).

**Cut a sheet** you already have: Window ▸ Atlas Editor, or right-click a
texture in the asset browser, "Slice in Atlas Editor" (double-click an
atlas to edit it). Slice by a grid (cell, offset, spacing) or **Draw
region** by hand; name each region, set its pivot (**Centre**, **Feet**),
preview a run of frames as an animation, and **Save**. The Atlas Editor
keeps its own undo history: `Ctrl+Z` over its window undoes there, and in
the scene everywhere else.

## Tilemaps

Scene ▸ Spawn tilemap makes a layer; its inspector names the atlas it
paints with. Open Window ▸ Tile Palette (or "Tile Palette…" in the layer's
inspector): the sheet laid out as it is. Click a tile, or drag over a block
of them, and paint in the viewport with the layer selected:

| Key | Tool |
| --- | --- |
| `B` | Brush: the stamp on every cell the pointer crosses |
| `U` | Rectangle: drag out a box, filled with the stamp as a pattern |
| `D` | Eraser |
| `G` | Bucket: fills the area joined to the clicked cell |
| `Z` / Shift+`Z` | Turn the stamp a quarter, either way |
| `X` / `Y` | Mirror the stamp |
| Alt held | Eyedropper: pick a tile — or drag, a block — off the map |

Each stroke is one undo entry. `W`, `E` or `R` go back to the transform
tools. A layer's terrains, chosen in its inspector, paint tiles that join
up with their neighbours. What a tile does in the physics, and values a
game reads off it, are in [chapter 4](04-physics.md#tiles).
