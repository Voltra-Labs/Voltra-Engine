# Sprite textures — design

Date: 2026-08-08
Status: approved
Stage: 12b of 12a/12b/12c

## Why this follows 12a

12a delivered `voltra-assets` with no consumer. 12b is the first consumer: a
sprite names a path, the store turns it into a shared GPU texture, the batch
splits draw calls by texture without breaking painter's order, and the scene
file round-trips the path.

| | Delivers | Visible on screen |
| --- | --- | --- |
| **12a** | `voltra-assets`: identity, handle, store, PNG loading, cache | No |
| **12b**, this spec | `Sprite` carries path+handle, batch runs, renderer binds per run, scene + inspector | Yes |
| **12c** | Hot reload under a stable handle | Yes |

## Decisions locked in brainstorming

Researched against Bevy, Godot, Unity and Unreal; chosen for robustness under
Voltra's layering (no AssetServer task system, no GUID sidecars, 2D painter's
order only).

### What `Sprite` stores

**Path on disk + handle at runtime** (the common engine pattern).

```rust
pub struct Sprite {
    pub color: [f32; 4],
    pub sort_order: i32,
    pub texture: Option<AssetPath>,
    #[serde(skip)]
    pub texture_handle: Option<Handle<Texture>>,
}
```

- `texture` is the identity the `.ron` understands. Serde writes only this.
- `texture_handle` is filled by `Textures::load` when the path is set or a
  scene is opened. Never serialised — a handle is a session index.
- `Copy` is lost (`AssetPath` holds a `String`). Batching and picking already
  take references; Bevy and Godot do not treat the sprite component as a
  cheap `Copy` either. Acceptable cost for a correct split of identities.

Rejected: path-only (re-resolve or re-read on the hot path). Rejected:
handle-only (nothing stable to put in the scene file without a reverse map
that becomes a second identity system).

### No path vs failed path

| Case | Draw |
| --- | --- |
| `texture: None` | 1×1 white × sprite colour (today's behaviour) |
| Path set, load fails | Magenta-and-black checker from 12a |

Unity and Godot draw nothing when the sprite/texture slot is empty; Bevy keeps
a default white-ish handle so a tinted quad still shows. Voltra already ships
coloured editor sprites with no PNG — matching Bevy (white × colour) preserves
that. Magenta stays the *error* signal only, as in Unity's missing-material
pink and as 12a already decided for a bad path.

### Who owns `Textures`, when the handle is filled

**The app/editor wiring owns `Textures`.** Resolve on scene Open, on inspector
path commit, and when code sets a path. `SpriteBatch::from_world` never
touches the disk.

Bevy's `AssetServer` / Godot's `ResourceLoader` / Unity's scene-load
resolution all load outside the draw loop. Putting I/O inside `from_world`
would warn and stall every frame for a broken path until the miss was cached —
and would couple scene geometry to GPU device lifetime.

`voltra-scene` depends on `voltra-assets` for `AssetPath` and `Handle`
types only. It does not own the store.

### Batching vs draw order

**Sort by `draw_key` first, then split into contiguous same-handle runs.**

Unity and Godot both batch only when adjacent sprites after sorting share a
texture/material. Reordering purely by texture breaks painter's order. Godot's
overlap-lookahead reordering is an optional optimisation; not in 12b.

Concrete shape:

1. Collect `(entity, transform, sprite)`, sort by `draw_key` (unchanged).
2. Emit one mesh in that order.
3. Record `ranges: Vec<(Option<Handle<Texture>>, Range<u32>)>` — index ranges
   over that mesh. `None` means "use the renderer's white bind group".
4. Renderer draws each range with the matching bind group.

Interleaved textures (`A, A, B, A`) correctly become three draws. Contiguous
same-path sprites become one. Two sprites naming one PNG still share one
handle and one GPU texture (12a's promise); whether they share a draw call
depends only on whether anything with another handle sits between them in
sort order.

### Renderer layering

`voltra-render` still does not depend on `voltra-assets`. It receives bind
groups and index ranges (or equivalent) from the caller. The white bind group
stays on `Renderer` as the sentinel for `None`. Bind groups for loaded
textures are cached on `Textures` at load time (it already has the device),
not recreated every frame.

### Scene format

- Field: `texture: Option<AssetPath>` on `Sprite`, `#[serde(default)]`.
- Old scenes without the field open as `None`. **No `VERSION` bump** — a
  missing optional field with a default is not a breaking format change.
- After load, a resolve pass walks sprites with `Some(path)` and fills
  handles via `Textures::load`.

### Inspector

A text field for the path plus Clear. Committing the field validates through
`AssetPath::new`, then resolves. A native file picker matches Unity/Godot UX
but is polish; the wire format is the path string, and a text field is enough
to exercise the full pipeline in 12b.

## Helper surface

Something equivalent to:

```rust
impl Sprite {
    pub fn set_texture(
        &mut self,
        path: Option<AssetPath>,
        textures: &mut Textures,
        device: &Device,
        queue: &Queue,
    );
}
```

- `Some(path)` → store path, `texture_handle = Some(textures.load(...))`.
- `None` → clear both fields (white draw).

Open uses the same path after deserialising.

## Crate edges after 12b

```
voltra-editor / voltra-core
        │ owns Textures, resolves, passes bind groups
        ▼
voltra-scene ──► voltra-assets ──► voltra-render
                     │
                     └── AssetPath, Handle, Textures
```

Exactly the edge the 12a design deferred to this stage.

## Tests

- Unit: `Sprite` round-trips through RON with and without `texture`; handle
  is never in the RON; hostile paths still rejected by `AssetPath`.
- Unit: `SpriteBatch` ranges — same handle contiguous → one range; interleaved
  → one range per run; `None` and a real handle are different runs; draw order
  of vertices still matches `draw_key`.
- Unit/integration: resolve after load fills handles; clear path clears handle.
- Headless GPU (skip without adapter): two sprites same path share pixels from
  one texture; a bad path shows checker-sized texture; `None` still tints via
  white.
- Existing batch/pick/`sort_order` tests updated for non-`Copy` `Sprite` and
  kept green.

## Out of scope

- Hot reload (12c).
- Texture atlases / packing.
- Overlap-based batch reordering (Godot lookahead).
- Native file-picker UI.
- Pixel-perfect picking — ARCHITECTURE.md already notes it waits on per-sprite
  alpha; 12b makes that possible later but does not implement it.
- Async loading / a task system.

## Success

A scene that names `texture: Some(Path("sprites/hero.png"))` draws that PNG.
Two entities with the same path share one GPU texture. A missing file draws
magenta checks and the scene still opens. A sprite with no path draws as a
tinted white quad. Save/Open round-trips the path. Old scenes keep working.
