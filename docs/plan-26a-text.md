# Stage 26a — text: fonts, a glyph atlas, `Text` in the world

Working file. Delete once landed and the decisions are in ARCHITECTURE.md.
Branch `feature/game-ui`, stacked on `feature/tilemap-paint` (#89).

## Decided (2026-11-20)

- **26 splits in two.** 26a: font asset with hot reload, glyph atlas, a
  `Text` component drawn by the sprite pipeline in world space, inspector.
  26b: screen-space pass, `UiRect` anchors + offsets, a HUD in the
  platformer. **Display-only**: buttons and focus are a later stage.
- **Rasterizer: `fontdue`** (0.9.x, leaf crate) — rasterizes and lays out a
  line, bitmap per size. Into root `[workspace.dependencies]`; only
  `voltra-assets` depends on it.
- **Default font bundled**: copy `Ubuntu-Light.ttf` from the vendored
  `epaint_default_fonts-0.35.0` into `assets/fonts/`, with its licence
  (`UFL.txt`) beside it. A `Text` with no font uses it.

## Facts the design rests on (checked)

- Sprite pipeline blends with `ALPHA_BLENDING`; textures are
  `Rgba8UnormSrgb`, whose **alpha is linear** — glyph coverage goes in alpha
  with white RGB, and the vertex colour tints it.
- `Vertex` carries RGB only; `Sprite::color[3]` is not sent. Text inherits
  that: colour alpha unsupported, same as sprites. Do not widen the vertex
  in 26a.
- `SpriteBatch::push_quad(matrix, corners, uvs, color, texture)` is the seam
  tilemaps already use; text is quads into the same sorted list.
- Stores (`Textures`, `Atlases`, `Clips`) share one shape: load once per
  path, stable `Handle`, placeholder for what will not load, `reload` swaps
  under the handle. `Fonts` follows it.

## Shape

**`voltra-assets`**
- `fonts.rs`: `Fonts` store — `load(path) -> Handle<Font>`, `reload`,
  `try_get`; a file that will not parse logs and keeps the old font (a fresh
  load with no previous one resolves to the default font, or to nothing if
  that is missing too — logged, never a panic).
- `fonts/atlas.rs` + `fonts/shelf.rs`: a glyph atlas per font — shelf
  packing into an RGBA8 CPU image, keyed by `(glyph index, px size)`.
  Grows by doubling and repacking when full; a glyph larger than the
  maximum side is refused and logged. Dirty flag → re-uploaded as one
  texture under a stable `Handle<Texture>`.
- `Textures` needs a way to hold a generated (non-file) texture under a
  stable handle and replace it: add `insert_generated` / `replace` (check
  what `reload` already does before adding).
- Watcher: `.ttf` / `.otf` join the extension lists (`is_font`).

**`voltra-scene`**
- `text.rs` (+ `text/` for layout/mesh): `Text { content: String,
  font: Option<AssetPath>, size: f32 /* em height, entity units */,
  pixels_per_em: u32 /* raster resolution, default 48 */, color: [f32; 4],
  align: Align { Left, Center, Right }, sort_order: i32,
  #[serde(skip)] font_handle }`. Registered for save.
- Layout with `fontdue::layout::Layout` (multi-line via `\n`, alignment);
  origin = baseline-left of the first line, so a `Text` lines up with
  integer coordinates the way a tile does.
- `SpriteBatch::from_world` collects `Text` into the same `(sort_order,
  entity)` draw order as sprites and layers; meshing asks the glyph atlas for
  each glyph's rect (rasterizing on a miss).
- Picking: a text's laid-out bounds, through its world matrix.

**`voltra-core`**
- Resolve `Text::font_handle` where sprites resolve theirs
  (`resolve_world_*`, `spawning::resolve_entity`); hot reload of fonts in
  `reload_changed_assets`; upload dirty atlases before the draw.

**`voltra-editor`**
- Inspector section: content (multiline), font path field, size, pixels per
  em, colour, align, sort order — one undo entry per edit via `claim`.
- **Scene ▸ Spawn text**; dropping a `.ttf` on the viewport spawns a `Text`
  using it.

**Platformer / sandbox**: one world-space label, so the path is exercised
by a real scene.

## Tests

shelf packing (fits, grows, refuses oversize); atlas keyed by size; layout
positions and alignment; batch order with sprites/layers; font store
fallback and reload; a headless render test that a glyph's coverage lands in
alpha (beside `headless_egui.rs`'s colour-space pins).

## Open for the builder

- Glyph padding (1 px) to avoid bleeding with linear sampling.
- Whether one atlas per font or per (font, size) — per font is the default;
  switch only if repacking churn shows.
- Kerning: `fontdue` layout applies it; keep it on.
