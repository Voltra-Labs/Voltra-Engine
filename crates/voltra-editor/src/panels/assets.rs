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

use voltra_assets::browse::{self, Entry};
use voltra_assets::AssetPath;
use voltra_core::egui::{self, RichText, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;

mod tile;

use tile::{allocate, paint_block, paint_name, tile};

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
    /// The directory being shown, `None` for the asset root: where a file
    /// the editor writes goes, the way Unity writes into the open folder.
    pub(crate) fn dir(&self) -> Option<&AssetPath> {
        self.dir.as_ref()
    }

    /// Reads the directory again next frame rather than when the listing
    /// ages out: the editor just wrote a file the author expects to see.
    pub(crate) fn refresh(&mut self) {
        self.scanned = None;
    }

    /// Moves to `dir` and reads it on the next frame.
    pub(super) fn go(&mut self, dir: Option<AssetPath>) {
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
