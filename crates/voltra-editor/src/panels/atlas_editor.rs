//! The Atlas Editor window: a sheet cut into regions by a grid or by hand,
//! named, given pivots, previewed as an animation, and saved as the
//! `.atlas.ron` it is.
//!
//! Unity's Sprite Editor and Godot's atlas texture region editor, in one
//! window over the viewport. Opened from Window ▸ Atlas Editor, from an atlas
//! or a texture in the asset browser, or through the bridge. What it edits is
//! an [`AtlasDoc`], with its own undo: the buttons here, or Ctrl+Z while the
//! pointer is over the window.

mod canvas;
mod preview;

use voltra_assets::{AssetPath, Frame, TextureSizes};
use voltra_core::egui::{self, Color32, DragValue, RichText, Ui};
use voltra_core::UiFrame;
use voltra_render::glam::{UVec2, Vec2};

use crate::atlas_doc::AtlasDoc;
use crate::editor::Editor;
use crate::undo::shortcut::UndoAction;

/// The window, the atlas it has open and the fields around it.
pub(crate) struct AtlasEditorWindow {
    pub(crate) open: bool,
    pub(crate) doc: Option<AtlasDoc>,
    /// The path typed into the Open field.
    path: String,
    /// The grid the Slice button cuts.
    cell: [u32; 2],
    offset: [u32; 2],
    spacing: [u32; 2],
    canvas: canvas::Canvas,
    preview: preview::Preview,
    /// What the last action said.
    status: Option<Result<String, String>>,
    /// Where the window was drawn last frame: Ctrl+Z over it is its own.
    rect: Option<egui::Rect>,
}

impl Default for AtlasEditorWindow {
    fn default() -> Self {
        Self {
            open: false,
            doc: None,
            path: String::new(),
            cell: [16, 16],
            offset: [0, 0],
            spacing: [0, 0],
            canvas: canvas::Canvas::default(),
            preview: preview::Preview::default(),
            status: None,
            rect: None,
        }
    }
}

impl AtlasEditorWindow {
    /// Opens `path` — an `.atlas.ron`, or a sheet to cut — and shows the
    /// window. The sheet is loaded through the session's textures, which
    /// is also what answers its size.
    pub(crate) fn open_path(
        &mut self,
        frame: &mut UiFrame<'_>,
        path: &AssetPath,
    ) -> Result<(), String> {
        let root = frame.textures.root().to_path_buf();
        let doc = AtlasDoc::open(&root, path, |texture| {
            let handle = frame.textures.load(frame.device, frame.queue, texture);
            if frame.textures.is_broken(handle) {
                return None;
            }
            frame.textures.size(handle)
        })?;
        self.path = path.as_str().to_owned();
        self.preview = preview::Preview::default();
        self.status = Some(Ok(format!(
            "opened {} ({} regions)",
            doc.path().as_str(),
            doc.regions().len()
        )));
        self.doc = Some(doc);
        self.open = true;
        Ok(())
    }

    /// Whether the pointer is over the window, so the undo keys are its.
    pub(crate) fn has_pointer(&self, ctx: &egui::Context) -> bool {
        self.open
            && self.doc.is_some()
            && self
                .rect
                .zip(ctx.pointer_hover_pos())
                .is_some_and(|(rect, at)| rect.contains(at))
    }

    /// Applies an undo key pressed over the window.
    pub(crate) fn shortcut(&mut self, action: UndoAction) {
        if let Some(doc) = &mut self.doc {
            match action {
                UndoAction::Undo => doc.undo(),
                UndoAction::Redo => doc.redo(),
            };
        }
    }
}

pub(crate) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    let window = &mut editor.atlas_editor;
    if !window.open {
        window.rect = None;
        return;
    }
    let mut open = true;
    let title = match &window.doc {
        Some(doc) if doc.is_dirty() => format!("Atlas Editor — {}*", doc.path().as_str()),
        Some(doc) => format!("Atlas Editor — {}", doc.path().as_str()),
        None => "Atlas Editor".to_owned(),
    };
    let shown = egui::Window::new(title)
        .id(egui::Id::new("atlas_editor"))
        .open(&mut open)
        .default_size([760.0, 520.0])
        .show(ui.ctx(), |ui| contents(window, ui, frame));
    window.rect = shown.map(|shown| shown.response.rect);
    window.open = open;
    if let Some(doc) = &mut window.doc {
        // One entry per gesture: nothing held and nothing being typed.
        let settled = !ui.input(|i| i.pointer.any_down()) && !ui.ctx().egui_wants_keyboard_input();
        doc.settle(settled);
    }
}

fn contents(window: &mut AtlasEditorWindow, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    ui.horizontal(|ui| {
        ui.label("Sheet or atlas");
        ui.text_edit_singleline(&mut window.path);
        if ui.button("Open").clicked() {
            let result = AssetPath::new(window.path.trim())
                .map_err(|e| e.to_string())
                .and_then(|path| window.open_path(frame, &path));
            if let Err(e) = result {
                window.status = Some(Err(e));
            }
        }
    });
    let root = frame.textures.root().to_path_buf();
    let Some(doc) = window.doc.as_mut() else {
        status(ui, &window.status);
        ui.weak("Open a texture to cut, or an .atlas.ron to edit.");
        return;
    };

    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            window.status = Some(
                doc.save(&root, None)
                    .map(|file| format!("saved {}", file.display())),
            );
            frame.atlases.reload(doc.path());
        }
        if ui
            .add_enabled(doc.can_undo(), egui::Button::new("Undo"))
            .clicked()
        {
            doc.undo();
        }
        if ui
            .add_enabled(doc.can_redo(), egui::Button::new("Redo"))
            .clicked()
        {
            doc.redo();
        }
        ui.separator();
        ui.weak(format!(
            "{} · {}×{} · {} regions",
            doc.texture().as_str(),
            doc.sheet().x,
            doc.sheet().y,
            doc.regions().len()
        ));
    });

    ui.horizontal(|ui| {
        ui.label("Grid: cell");
        pair(ui, &mut window.cell, 1);
        ui.label("offset");
        pair(ui, &mut window.offset, 0);
        ui.label("spacing");
        pair(ui, &mut window.spacing, 0);
        if ui
            .button("Slice")
            .on_hover_text("replaces every region with the grid's cells")
            .clicked()
        {
            window.status = Some(
                doc.slice_grid(
                    UVec2::from(window.cell),
                    UVec2::from(window.offset),
                    UVec2::from(window.spacing),
                )
                .map(|cells| format!("sliced into {cells} cells")),
            );
        }
        ui.toggle_value(&mut window.canvas.draw, "Draw region")
            .on_hover_text("drag on the sheet to add a region");
    });
    status(ui, &window.status);
    ui.separator();

    ui.horizontal_top(|ui| {
        let texture = frame
            .textures
            .load(frame.device, frame.queue, doc.texture());
        let image = frame.thumbnail(texture);
        window.canvas.show(ui, doc, image);
        ui.separator();
        ui.vertical(|ui| {
            region_list(ui, doc);
            ui.separator();
            region_fields(ui, doc);
            ui.separator();
            window.preview.show(ui, doc, image);
        });
    });
}

/// Two numbers side by side, no lower than `min`.
fn pair(ui: &mut Ui, value: &mut [u32; 2], min: u32) {
    ui.add(DragValue::new(&mut value[0]).range(min..=8192));
    ui.add(DragValue::new(&mut value[1]).range(min..=8192));
}

fn status(ui: &mut Ui, status: &Option<Result<String, String>>) {
    match status {
        Some(Ok(message)) => {
            ui.label(RichText::new(message).color(Color32::from_rgb(120, 200, 120)));
        }
        Some(Err(error)) => {
            ui.label(RichText::new(error).color(Color32::from_rgb(240, 90, 90)));
        }
        None => {}
    }
}

fn region_list(ui: &mut Ui, doc: &mut AtlasDoc) {
    egui::ScrollArea::vertical()
        .id_salt("atlas_regions")
        .max_height(160.0)
        .show(ui, |ui| {
            let mut picked = None;
            for (index, region) in doc.regions().iter().enumerate() {
                let label = if region.name.is_empty() {
                    format!("{index}")
                } else {
                    format!("{index}  {}", region.name)
                };
                if ui
                    .selectable_label(doc.selected == Some(index), label)
                    .clicked()
                {
                    picked = Some(index);
                }
            }
            if picked.is_some() {
                doc.selected = picked;
            }
        });
}

/// The selected region's name, rectangle and pivot, edited in place; the
/// window settles them into one undo entry per gesture.
fn region_fields(ui: &mut Ui, doc: &mut AtlasDoc) {
    let Some(index) = doc.selected else {
        ui.weak("Click a region to edit it.");
        return;
    };
    let sheet = doc.sheet();
    let taken: Vec<String> = doc
        .regions()
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, region)| region.name.clone())
        .collect();
    let mut remove = false;
    let Some(region) = doc.region_mut(index) else {
        return;
    };
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut region.name);
    });
    if !region.name.is_empty() && taken.contains(&region.name) {
        ui.colored_label(
            Color32::from_rgb(240, 90, 90),
            "another region has this name",
        );
    }
    let before = region.frame;
    let Frame {
        mut x,
        mut y,
        mut width,
        mut height,
    } = region.frame;
    ui.horizontal(|ui| {
        ui.label("x");
        ui.add(DragValue::new(&mut x).range(0..=sheet.x.saturating_sub(1)));
        ui.label("y");
        ui.add(DragValue::new(&mut y).range(0..=sheet.y.saturating_sub(1)));
        ui.label("w");
        ui.add(DragValue::new(&mut width).range(1..=sheet.x.saturating_sub(x).max(1)));
        ui.label("h");
        ui.add(DragValue::new(&mut height).range(1..=sheet.y.saturating_sub(y).max(1)));
    });
    let frame = Frame::new(x, y, width, height);
    if frame != before {
        region.frame = frame;
        region.trim = None;
    }
    let mut pivot = region.pivot.unwrap_or(Vec2::splat(0.5));
    ui.horizontal(|ui| {
        ui.label("Pivot");
        let px = ui.add(DragValue::new(&mut pivot.x).range(0.0..=1.0).speed(0.01));
        let py = ui.add(DragValue::new(&mut pivot.y).range(0.0..=1.0).speed(0.01));
        if px.changed() || py.changed() {
            region.pivot = Some(pivot);
        }
        if ui.button("Centre").clicked() {
            region.pivot = None;
        }
        if ui
            .button("Feet")
            .on_hover_text("the bottom middle")
            .clicked()
        {
            region.pivot = Some(Vec2::new(0.5, 1.0));
        }
    });
    if ui
        .button("Remove region")
        .on_hover_text("the regions after it move down an index")
        .clicked()
    {
        remove = true;
    }
    if remove {
        if let Err(e) = doc.remove(index) {
            log::warn!("{e}");
        }
        doc.selected = None;
    }
}
