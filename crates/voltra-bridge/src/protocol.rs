//! What goes over the socket.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod response;

pub use response::Response;

/// One command, with the id its answer will carry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The id the answer will carry.
    pub id: u64,
    /// What to do.
    pub command: Command,
}

/// What a tool asks the editor to do. Entities are named as the MCP tools
/// name them: an id (a UUID) or a path of names from a root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    /// What the editor has open, its play state, its selection.
    Status,
    /// The open scene's entities, as `scene_read` answers a file.
    Read,
    /// Creates an entity named `name`, under `parent`, with `components`.
    Spawn {
        /// The new entity's name.
        #[serde(default)]
        name: Option<String>,
        /// The parent entity; none makes it a root.
        #[serde(default)]
        parent: Option<String>,
        /// Registered component name → RON.
        #[serde(default)]
        components: BTreeMap<String, String>,
    },
    /// Removes `entity` and everything under it.
    Delete {
        /// The entity to remove.
        entity: String,
    },
    /// Moves `entity` under `parent`.
    Reparent {
        /// The entity to move.
        entity: String,
        /// The new parent; none makes it a root.
        #[serde(default)]
        parent: Option<String>,
    },
    /// Renames `entity` to `name`.
    Rename {
        /// The entity to rename.
        entity: String,
        /// The new name.
        name: String,
    },
    /// Sets `component` on `entity` from RON.
    ComponentSet {
        /// The entity to change.
        entity: String,
        /// The registered component name.
        component: String,
        /// The component's value, as RON.
        ron: String,
    },
    /// Removes `component` from `entity`.
    ComponentRemove {
        /// The entity to change.
        entity: String,
        /// The registered component name.
        component: String,
    },
    /// Selects these entities, the last the primary; none clears it.
    Select {
        /// The entities to select, by id or path.
        entities: Vec<String>,
    },
    /// Undoes the last edit.
    Undo,
    /// Redoes the last edit that was undone.
    Redo,
    /// Writes the open scene to its file, or to `path` (an asset path),
    /// which becomes its file.
    Save {
        /// An asset path to save to, which becomes the scene's file.
        #[serde(default)]
        path: Option<String>,
    },
    /// Opens the scene at `path` in place of the open one. Refused over
    /// unsaved changes unless `discard_changes`.
    Open {
        /// The asset path of the scene to open.
        path: String,
        /// Open even over unsaved changes.
        #[serde(default)]
        discard_changes: bool,
    },
    /// Replaces the open scene with an empty, untitled one, on the same
    /// terms as `Open`.
    New {
        /// Replace even over unsaved changes.
        #[serde(default)]
        discard_changes: bool,
    },
    /// Starts playing the open scene.
    Play,
    /// Pauses play.
    Pause,
    /// Runs this many fixed steps while paused.
    Step {
        /// How many fixed steps to run, clamped to 1 to 600.
        frames: u32,
    },
    /// Stops play and returns to editing.
    Stop,
    /// Holds an action down for this many frames of play.
    Input {
        /// The input action, as the input map names it.
        action: String,
        /// How many frames to hold it, clamped to 1 to 6000.
        frames: u32,
    },
    /// The viewport as it was last drawn, as PNG bytes in base64.
    Screenshot,
    /// The grid and snapping: sets what is given, answers all of it.
    Snap {
        /// Whether snapping is on.
        #[serde(default)]
        enabled: Option<bool>,
        /// The grid's spacing, in world units; ignored unless positive.
        #[serde(default)]
        step: Option<f32>,
        /// The angle a turn snaps to, in degrees; ignored unless positive.
        #[serde(default)]
        angle: Option<f32>,
        /// The increment a scale snaps to; ignored unless positive.
        #[serde(default)]
        scale: Option<f32>,
    },
    /// What a gizmo drag of `entity` with `tool` — `move`, `rotate` or
    /// `scale` — from `from` to `to`, world units, does: snapped as the
    /// viewport's would be, one undo entry.
    Drag {
        /// The entity to drag.
        entity: String,
        /// The gizmo tool: `move`, `rotate` or `scale`.
        tool: String,
        /// Where the drag starts, in world units.
        from: [f32; 2],
        /// Where the drag ends, in world units.
        to: [f32; 2],
    },
    /// The Console's lines at `level` (`error` … `trace`, default `info`)
    /// or louder holding `filter`, the last `last` of them.
    Console {
        /// The quietest level to show, from `error` to `trace`; `info` when none.
        #[serde(default)]
        level: Option<String>,
        /// Text a line must hold to be shown.
        #[serde(default)]
        filter: Option<String>,
        /// How many of the newest matching lines to answer with.
        #[serde(default)]
        last: Option<usize>,
    },
    /// A project settings file — `input`, `layers`, `buses`, `render` or `build` — read, or,
    /// with `text`, checked and saved as the Project Settings window saves.
    Settings {
        /// Which file: `input`, `layers`, `buses`, `render` or `build`.
        file: String,
        /// New contents to check and save; none reads the file instead.
        #[serde(default)]
        text: Option<String>,
    },
    /// Packs every PNG under the folder `folder` (an asset path) into
    /// `<folder>.png` and `<folder>.atlas.ron` beside it, as the asset
    /// browser's "Pack into atlas" does.
    PackAtlas {
        /// The folder of loose pictures, as an asset path.
        folder: String,
        /// Cut transparent borders off first.
        #[serde(default)]
        trim: bool,
    },
    /// Opens `path` — an `.atlas.ron`, or a sheet to cut — in the Atlas
    /// Editor, in place of what it had open.
    AtlasOpen {
        /// The asset path of the atlas or the sheet.
        path: String,
    },
    /// The atlas the Atlas Editor has open: its file, sheet and regions.
    AtlasRead,
    /// Replaces the open atlas's regions with every whole cell of a grid,
    /// named after the sheet, as the Slice button does. One undo entry.
    AtlasSlice {
        /// One cell's width and height, in texels.
        cell: [u32; 2],
        /// The texels before the first cell, across and down.
        #[serde(default)]
        offset: [u32; 2],
        /// The texels between two cells, across and down.
        #[serde(default)]
        spacing: [u32; 2],
    },
    /// Changes region `index` of the open atlas — or, with no `index`, adds
    /// one over `rect`. One undo entry per change.
    AtlasRegion {
        /// The region to change; none adds one.
        #[serde(default)]
        index: Option<usize>,
        /// Its new name.
        #[serde(default)]
        name: Option<String>,
        /// Its rectangle on the sheet: `[x, y, width, height]`, in texels.
        #[serde(default)]
        rect: Option<[u32; 4]>,
        /// Its pivot, `0..1` across and *down* the picture.
        #[serde(default)]
        pivot: Option<[f32; 2]>,
        /// Removes it instead.
        #[serde(default)]
        remove: bool,
    },
    /// Undoes the Atlas Editor's last edit.
    AtlasUndo,
    /// Redoes the Atlas Editor's last undone edit.
    AtlasRedo,
    /// Saves the open atlas to its file, or to `path`, which becomes its file.
    AtlasSave {
        /// An asset path ending in `.atlas.ron` to save to instead.
        #[serde(default)]
        path: Option<String>,
    },
    /// Picks the block of `layer`'s tileset from palette cell `from` to `to`
    /// — columns and rows from the top-left, as the Tile Palette window
    /// lays the sheet out — as the paint stamp, as a drag over it does.
    TilePick {
        /// The tilemap layer, by id or path.
        layer: String,
        /// One corner of the block, `[column, row]`.
        from: [u32; 2],
        /// The other corner, `[column, row]`.
        to: [u32; 2],
    },
    /// Turns and mirrors the paint stamp, as `Z`, `X` and `Y` do.
    TileStamp {
        /// Quarter turns clockwise, as seen; negative turns the other way.
        #[serde(default)]
        turns: i32,
        /// Mirror it left to right, after turning.
        #[serde(default)]
        flip_x: bool,
        /// Mirror it top to bottom, after turning.
        #[serde(default)]
        flip_y: bool,
    },
    /// One stroke of a paint tool on `layer` through `cells` — pressed on the
    /// first, released on the last — as a drag in the viewport makes it. One
    /// undo entry.
    TilePaint {
        /// The tilemap layer, by id or path.
        layer: String,
        /// `brush`, `rectangle`, `eraser` or `fill`.
        tool: String,
        /// The cells the pointer goes through, `[x, y]`, `y` up.
        cells: Vec<[i32; 2]>,
        /// The eyedropper: picks the cells as the stamp, as Alt held does,
        /// and paints nothing.
        #[serde(default)]
        pick: bool,
        /// Paint this terrain of the layer instead of the stamp, joined up.
        #[serde(default)]
        terrain: Option<usize>,
    },
    /// The cells of `layer` from `from` to `to`, rows from the top: each a
    /// tile, or null where the cell is empty.
    TileRead {
        /// The tilemap layer, by id or path.
        layer: String,
        /// One corner, `[x, y]`.
        from: [i32; 2],
        /// The other corner, `[x, y]`.
        to: [i32; 2],
    },
    /// The last `frames` recorded frames, summarised; `record` first starts
    /// or stops recording.
    Profile {
        /// How many recorded frames to summarise.
        frames: u32,
        /// Starts (`true`) or stops (`false`) recording first; none leaves it as it is.
        #[serde(default)]
        record: Option<bool>,
    },
    /// Opens a prefab on its own, over the open scene, as the asset
    /// browser's "Edit as prefab" does: the file at `path`, or the one the
    /// instance `entity` is part of — one of the two. Every edit then lands
    /// in the prefab, with an undo history of its own; `save` writes its
    /// file, and `prefab_close` returns.
    PrefabOpen {
        /// The prefab's asset path.
        #[serde(default)]
        path: Option<String>,
        /// An entity of an instance, by id or path, whose prefab to open.
        #[serde(default)]
        entity: Option<String>,
    },
    /// Returns from the prefab being edited to the document it was opened
    /// from, whose instances follow the prefab as last saved. Refused over
    /// unsaved changes to the prefab unless `discard_changes`.
    PrefabClose {
        /// Return even over unsaved changes, dropping them.
        #[serde(default)]
        discard_changes: bool,
    },
    /// Writes a variant of the prefab at `path` beside it — a prefab whose
    /// root is an instance of it, following every edit to it but for what
    /// the variant is edited to override — as the asset browser's "Create
    /// variant" does, and answers the new file's path
    /// (`<name> Variant.prefab.ron`, numbered when taken).
    PrefabVariant {
        /// The base prefab's asset path.
        path: String,
    },
    /// Puts `query` in the hierarchy's search box — words a name holds,
    /// `t:Component` for a component every match has — and answers the
    /// entities it matches and the rows kept to show where they are.
    HierarchySearch {
        /// What to search for; empty clears the search.
        query: String,
    },
    /// Puts `query` in the asset browser's search box — words a file name
    /// holds, `t:kind` for a kind (texture, sound, scene, prefab, atlas,
    /// material…) — and answers what matches under the whole asset root.
    AssetSearch {
        /// What to search for; empty clears the search.
        query: String,
    },
    /// Network play: how many clients Play starts beside the editor's own
    /// world, which hosts, and how bad every link between them is — live.
    /// A field left out keeps its value. Answers the session: its
    /// settings, the server's connections and each client's.
    NetPlay {
        /// Clients the next Play starts; `0` plays offline.
        #[serde(default)]
        clients: Option<usize>,
        /// One-way delay on every link, both ways, in milliseconds.
        #[serde(default)]
        latency_ms: Option<f64>,
        /// Up to this much more, at random per datagram, in milliseconds.
        #[serde(default)]
        jitter_ms: Option<f64>,
        /// The share of datagrams lost, `0.0..=1.0`.
        #[serde(default)]
        loss: Option<f32>,
        /// Bytes a second each link carries; `0` for no cap.
        #[serde(default)]
        bandwidth: Option<u32>,
    },
}

impl Command {
    /// What an undo entry made by this command is called.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Spawn { .. } => "AI: spawn",
            Self::Delete { .. } => "AI: delete",
            Self::Reparent { .. } => "AI: reparent",
            Self::Rename { .. } => "AI: rename",
            Self::ComponentSet { .. } => "AI: set component",
            Self::ComponentRemove { .. } => "AI: remove component",
            Self::Drag { .. } => "AI: drag",
            Self::TilePaint { .. } => "AI: paint tiles",
            _ => "AI",
        }
    }
}

#[cfg(test)]
mod tests;
