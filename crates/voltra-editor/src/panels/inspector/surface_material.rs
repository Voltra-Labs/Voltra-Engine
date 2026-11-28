//! The surface material a mesh draws through, its file edited in place,
//! with a live preview on a lit sphere.
//!
//! The file is shared by every mesh naming it, so its edits are not the
//! scene's: see [`SurfaceDoc`] for when they are written.

use voltra_assets::{AssetPath, MaterialDomain};
use voltra_core::egui::{self, RichText, Ui};
use voltra_core::UiFrame;

use crate::surface_doc::SurfaceDoc;

mod fields;

/// Points a side the preview is drawn at.
const PREVIEW: f32 = 128.0;

/// The file at `path` — opened into `slot` when it is not the one open —
/// as fields per block, the preview beside them. Edits go to the file when
/// they settle; nothing here is an edit of the scene.
pub(super) fn show(
    ui: &mut Ui,
    frame: &mut UiFrame<'_>,
    slot: &mut Option<SurfaceDoc>,
    path: &AssetPath,
) {
    let root = frame.textures.root().to_path_buf();
    let revision = revision(frame, path);
    // Another file, or this one changed on disk (a text editor, a hot
    // reload) with nothing unsaved here: read it again rather than write
    // a stale draft over it.
    let stale = slot.as_ref().is_none_or(|doc| {
        doc.path() != path || (!doc.is_unsaved() && doc.seen.is_some() && doc.seen != revision)
    });
    if stale {
        match SurfaceDoc::open(&root, path) {
            Ok(mut doc) => {
                doc.seen = revision;
                *slot = Some(doc);
            }
            Err(error) => {
                *slot = None;
                ui.label(
                    RichText::new(format!("cannot open it: {error}"))
                        .color(ui.visuals().error_fg_color)
                        .small(),
                );
                return;
            }
        }
    }
    let Some(doc) = slot.as_mut() else {
        return;
    };
    if doc.domain() != MaterialDomain::Surface {
        ui.label(
            RichText::new("a sprite material: the mesh draws the default white standard")
                .color(ui.visuals().warn_fg_color)
                .small(),
        );
        return;
    }
    // What the file holds now that cannot be drawn — a texture path that is
    // no asset path — said as the draw list says it.
    let error = frame
        .materials
        .as_deref()
        .and_then(|m| m.by_path_handle(path).and_then(|h| m.get(h)))
        .and_then(|m| m.error.clone());
    if let Some(error) = error {
        ui.label(
            RichText::new(error)
                .color(ui.visuals().error_fg_color)
                .small(),
        );
    }
    ui.label(
        RichText::new("edits the file: every mesh naming it changes")
            .weak()
            .small(),
    );
    match doc.standard().resolve() {
        Ok(source) => {
            let pixels = (PREVIEW * ui.ctx().pixels_per_point()).round() as u32;
            let key = format!("inspector/{}", path.as_str());
            match frame.surface_preview(&key, pixels, &source) {
                Some(id) => {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        id,
                        egui::vec2(PREVIEW, PREVIEW),
                    )));
                }
                None => {
                    ui.allocate_space(egui::vec2(PREVIEW, PREVIEW));
                }
            }
        }
        Err(error) => {
            ui.label(
                RichText::new(error)
                    .color(ui.visuals().error_fg_color)
                    .small(),
            );
        }
    }
    fields::show(ui, doc.standard_mut());
    // Written once the pointer is let go, no popup is open and nothing is
    // being typed: one write per gesture, not one a frame of a drag.
    let ctx = ui.ctx().clone();
    let busy = ctx.dragged_id().is_some()
        || egui::Popup::is_any_open(&ctx)
        || ctx.egui_wants_keyboard_input();
    if doc.settle(&root, frame.materials.as_deref_mut(), busy) {
        log::info!("saved {}", path.as_str());
        doc.seen = self::revision(frame, path);
    } else if doc.seen.is_none() {
        doc.seen = self::revision(frame, path);
    }
    if doc.is_unsaved() {
        ui.label(RichText::new("unsaved…").weak().small());
    }
}

/// The store's revision of the material at `path`, once loaded.
fn revision(frame: &UiFrame<'_>, path: &AssetPath) -> Option<u64> {
    let materials = frame.materials.as_deref()?;
    let handle = materials.by_path_handle(path)?;
    materials.get(handle).map(|m| m.revision)
}
