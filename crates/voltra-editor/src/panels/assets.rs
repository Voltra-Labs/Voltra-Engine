//! Bottom panel: what is under the asset root, as tiles you can drag.
//!
//! Unity's Project window, Unreal's Content Browser and Godot's FileSystem dock
//! are the same panel three times: a listing of the asset root, a thumbnail per
//! entry, and a drag that ends on the scene or on a property field. This is
//! that panel, minus the folder tree — a second tree next to the hierarchy is
//! noise at this size, and Unreal's path bar navigates the same tree in one
//! row. Where they disagree, the reasons are in `docs/ARCHITECTURE.md`.
//!
//! It is docked between the two side panels rather than across the whole
//! window: the drop targets are the viewport above it and the inspector beside
//! it, and a browser far from both makes for a long drag.

use std::time::{Duration, Instant};

use voltra_assets::browse::{self, Entry, EntryKind};
use voltra_assets::AssetPath;
use voltra_core::egui::{self, Color32, CursorIcon, Rect, RichText, Sense, Ui, Vec2};
use voltra_core::UiFrame;

use crate::drag;
use crate::editor::Editor;

/// Side of a tile's picture, in points.
const TILE: f32 = 64.0;

/// Height the name under a tile gets.
const LABEL: f32 = 16.0;

/// How long a listing is trusted before the directory is read again.
///
/// A browser that only refreshes on a button is stale exactly when it matters —
/// an artist alt-tabs back from an image editor expecting to see the file they
/// just saved. Reading the directory every frame instead would be a syscall per
/// entry at the frame rate for a picture that changes a few times an hour.
///
/// The asset watcher is not the answer here even though it exists: it reports
/// only the extensions that can become textures, and this panel also lists the
/// directories and the files no loader claims.
const RESCAN: Duration = Duration::from_millis(1000);

/// Where the browser is looking, and what it last saw there.
///
/// The listing is state rather than a per-frame read for the reason above; the
/// directory is state because it is a place the user navigated to and expects
/// to still be in next frame.
#[derive(Default)]
pub struct AssetBrowser {
    /// `None` is the asset root itself.
    dir: Option<AssetPath>,
    entries: Vec<Entry>,
    /// When `entries` was read. `None` means "never", which forces a scan.
    scanned: Option<Instant>,
    /// Why the last scan failed, if it did.
    ///
    /// Kept and shown rather than logged and forgotten: a directory deleted
    /// under the browser leaves an empty panel, and an empty panel that means
    /// "gone" must not look like an empty panel that means "nothing here".
    error: Option<String>,
}

impl AssetBrowser {
    /// Moves to `dir` and reads it on the next frame.
    fn go(&mut self, dir: Option<AssetPath>) {
        self.dir = dir;
        self.scanned = None;
    }

    /// Reads the directory if the listing is old enough to distrust.
    fn rescan(&mut self, root: &std::path::Path) {
        let fresh = self.scanned.is_some_and(|at| at.elapsed() < RESCAN);
        if fresh {
            return;
        }
        self.scanned = Some(Instant::now());

        match browse::list(root, self.dir.as_ref()) {
            Ok(entries) => {
                self.entries = entries;
                self.error = None;
            }
            Err(e) => {
                // Not `log::error!` on a loop: this runs once a second forever
                // while the directory is missing, and a log filling at 1 Hz
                // hides everything else.
                self.entries.clear();
                self.error = Some(e.to_string());
            }
        }
    }
}

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::bottom("assets")
        .resizable(true)
        .default_size(150.0)
        .show(ui, |ui| {
            editor.assets.rescan(frame.textures.root());

            ui.horizontal(|ui| {
                ui.heading("Assets");
                breadcrumb(&mut editor.assets, ui);
            });
            ui.separator();

            if let Some(error) = editor.assets.error.clone() {
                ui.label(RichText::new(error).color(ui.visuals().error_fg_color));
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Wrapped rather than a fixed grid: the panel is resizable, and
                // a grid with a column count baked in either overflows or
                // leaves a gap at every other width.
                ui.horizontal_wrapped(|ui| {
                    if editor.assets.dir.is_some() {
                        up(&mut editor.assets, ui);
                    }
                    // Cloned so the loop can hand `editor` to a spawn while it
                    // walks: the listing is a few dozen short strings, and it
                    // is replaced wholesale by the next scan anyway.
                    for entry in editor.assets.entries.clone() {
                        tile(editor, ui, frame, &entry);
                    }
                });
            });
        });
}

/// The path bar: the root, then one button per component of the current
/// directory.
///
/// Unreal's shape. Each component is a target, so leaving a deep directory is
/// one click rather than one click per level.
fn breadcrumb(browser: &mut AssetBrowser, ui: &mut Ui) {
    if ui.button("assets").clicked() {
        browser.go(None);
    }
    let Some(dir) = browser.dir.clone() else {
        return;
    };

    let mut walked = String::new();
    for part in dir.as_str().split('/') {
        ui.label("/");
        if !walked.is_empty() {
            walked.push('/');
        }
        walked.push_str(part);
        if !ui.button(part).clicked() {
            continue;
        }
        // Cannot fail: every prefix of a validated path is one.
        match AssetPath::new(&walked) {
            Ok(path) => browser.go(Some(path)),
            Err(e) => log::warn!("cannot navigate to {walked:?}: {e}"),
        }
    }
}

/// The tile that leaves the current directory.
fn up(browser: &mut AssetBrowser, ui: &mut Ui) {
    let (rect, response) = allocate(ui);
    let response = response.on_hover_text("up one level");
    paint_block(ui, rect, ui.visuals().widgets.inactive.bg_fill);
    paint_name(ui, rect, "..");
    if response.clicked() {
        let parent = browser.dir.as_ref().and_then(browse::parent);
        browser.go(parent);
    }
}

/// One directory or file.
fn tile(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>, entry: &Entry) {
    let (rect, response) = allocate(ui);
    // The full path, because the name under a tile is clipped and two
    // directories deep in the tree can share one.
    let response = response.on_hover_text(entry.path.as_str());

    match entry.kind {
        EntryKind::Directory => {
            paint_block(ui, rect, ui.visuals().widgets.inactive.bg_fill);
            if response.clicked() {
                editor.assets.go(Some(entry.path.clone()));
            }
        }
        EntryKind::Texture => texture_tile(ui, frame, entry, &response),
        // Listed, dimmed, and inert. Hiding it would read as a failed copy;
        // letting it be dragged would promise a loader that does not exist.
        EntryKind::Other => paint_block(ui, rect, ui.visuals().faint_bg_color),
    }

    paint_name(ui, rect, &entry.name);
}

/// A texture tile: its own pixels, and the drag that carries its path.
///
/// The load is here rather than in the scan because it is the tile being drawn
/// that decides a texture is worth uploading: a directory of a thousand PNGs
/// costs the ones on screen, not all of them. `Textures` caches, so this is a
/// hash lookup on every frame after the first.
fn texture_tile(ui: &mut Ui, frame: &mut UiFrame<'_>, entry: &Entry, response: &egui::Response) {
    let handle = frame.textures.load(frame.device, frame.queue, &entry.path);

    let rect = picture(response.rect);
    match frame.thumbnail(handle) {
        Some(id) => {
            ui.painter().image(
                id,
                rect,
                // The whole texture, top-left to bottom-right.
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        // The frame a texture is first named: registration happens before the
        // layout, so the id exists next frame. A blank tile for one frame is
        // the whole cost.
        None => paint_block(ui, rect, ui.visuals().faint_bg_color),
    }

    let response = response.interact(Sense::drag());
    response.dnd_set_drag_payload(entry.path.clone());
    if response.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        drag::ghost(ui, entry.path.as_str());
    }
}

/// Takes a tile's worth of room and senses a click and a hover.
fn allocate(ui: &mut Ui) -> (Rect, egui::Response) {
    ui.allocate_exact_size(Vec2::new(TILE, TILE + LABEL), Sense::click())
}

/// The part of a tile the picture goes in — everything above the name.
fn picture(rect: Rect) -> Rect {
    Rect::from_min_size(rect.min, Vec2::splat(TILE))
}

/// Fills a tile's picture area, for everything that has no picture of its own.
fn paint_block(ui: &Ui, rect: Rect, color: Color32) {
    ui.painter()
        .rect_filled(picture(rect), egui::CornerRadius::same(3), color);
}

/// Writes the name under a tile, clipped to the tile's width.
///
/// Clipped rather than shortened with an ellipsis: the hover text already
/// carries the full path, and a run of names all ending in `…` says less than
/// their first ten characters do.
fn paint_name(ui: &Ui, rect: Rect, name: &str) {
    let bottom = Rect::from_min_max(egui::pos2(rect.min.x, rect.max.y - LABEL), rect.max);
    ui.painter().with_clip_rect(bottom).text(
        bottom.center_top(),
        egui::Align2::CENTER_TOP,
        name,
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().text_color(),
    );
}
