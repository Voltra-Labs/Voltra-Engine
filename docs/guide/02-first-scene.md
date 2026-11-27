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

## Many at once

Ctrl-click or Shift-click in the hierarchy, or drag a box in the
viewport, to select several. The gizmo moves them together, and the
inspector edits what they share:

- A field they all agree on shows its value; one that differs shows `—`
  (a number), a half-ticked box, or a `≠` beside its label — and each
  section names its differing fields in a line under its rule. Edit a
  field and only that field is written to all of them: select three
  sprites of three colours, tick `flip x`, and each keeps its colour.
- A component only some have is listed, greyed, as `Collider (2 of 3)`,
  and not edited; **add to the rest** gives it to the others, and then
  it is. Adding a component none has adds it to all.
- Picking a value from a dropdown, ticking a box, or typing a number into
  a `—` field and leaving it sets it on all of them, even when it is the
  value the last picked already had. Picking a kind again (a collider's,
  a joint's) turns only the ones of another kind; each of that kind keeps
  its own sizes.
- Typing a name names them all.
- However many are selected, an edit — a whole drag — is one undo step.
- While playing (or recording on the timeline), only the last picked is
  edited.

Ctrl+D duplicates the selection (Ctrl+C / Ctrl+V copy and paste it).
Each copy is named apart from its new siblings, as in Unity: `Coin`
becomes `Coin (1)`, then `Coin (2)` — the first number no sibling has,
any number the original carried stripped first. A copy pasted where
nothing shares its name keeps it.

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

A prefab inside another stays linked to its own file: edit and save the
inner one (a coin inside a `row.prefab.ron`) and every row in the scene
picks the change up without the row being saved again — but for what the
row changed on that coin, which stays the row's.

A **variant** is a prefab that starts as another and changes some of it — a
blue coin that is otherwise the coin. Right-click a prefab in the asset
browser, "Create variant" (or **Create variant** in prefab mode's bar):
`coin Variant.prefab.ron` appears beside it. Open it and edit it like any
prefab; what you change is recorded as its overrides, listed in the
inspector on its root. Every later edit to the coin reaches the variant and
its instances, except what the variant overrides — and an instance of the
variant can override that in turn. **Apply to prefab** on an instance of a
variant writes the variant; to change the base, Apply from inside the
variant's prefab mode, or edit the base itself. A prefab cannot hold
itself, directly or through a variant: saving one that would is refused.

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

## Cameras

A scene is drawn through its **active `Camera`** with the highest
priority: Scene ▸ Spawn camera, or add the component to any entity. Its
`size` is half the height of world it shows; the Game toggle on the
viewport shows the scene through it. **Turn** the camera's entity, and the
view turns with it; tick **turn with parent** for it to turn with its
parents too — a camera hung off a ship — rather than stay upright while
it rides them. What a turned camera sees is culled, picked and lit as
drawn; a pixel-perfect camera stops snapping while it is turned.

Two components move a camera while the game runs (its inspector's **Add
follow** and **Add shake**):

- **`CameraFollow2D`** keeps it on a **target** — smoothed, with a dead
  zone the target moves in freely, a lead of the target's velocity, and
  bounds its view stays inside. **Also frames** adds more targets: it aims
  at the middle of the box around all of them, and **zoom to fit** eases
  the camera's size so they all show, with a margin, between a least and
  a greatest size — fitted to what that camera draws into, a split-screen
  half or a render texture. A target that leaves the scene drops out of the group.
  With the follower selected, the viewport draws its dead zone (rose),
  its bounds (faded rose) and the group's box (green).
- **`CameraShake`** shakes it by trauma a game adds —
  `world.get_mut::<CameraShake>(camera)` then `.add_trauma(0.5)` — moving
  its view up to `max offset` and, with a **max angle**, turning it too.
  The shake moves only what the camera shows, never its transform, so it
  always settles back where the camera rests — whatever else moves it.
