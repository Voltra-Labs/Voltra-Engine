//! Right panel: the components of the selected entity.

use voltra_assets::{AssetPath, Textures};
use voltra_core::egui::{self, Color32, DragValue, RichText, TextEdit, Ui};
use voltra_core::UiFrame;
use voltra_render::wgpu;
use voltra_scene::{SceneId, Sprite, Transform};

use crate::editor::Editor;
use crate::undo::SceneView;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::right("inspector")
        .default_size(240.0)
        .show(ui, |ui| {
            ui.heading("Inspector");
            ui.separator();

            // A stale handle is normal, not a bug: the entity may have been
            // despawned since it was selected.
            let Some(entity) = editor.selected.filter(|e| frame.world.is_alive(*e)) else {
                ui.label(RichText::new("nothing selected").italics().weak());
                return;
            };

            // Every `DragValue` below gets its id from layout position, not from
            // `entity`. Without this, selecting a different sprite that lands the
            // same row on screen reuses the previous widget's id, and egui — which
            // still thinks that id is mid-edit — applies the leftover text to the
            // new entity instead of the one the user clicked. Salting the whole
            // body by `entity` makes every id here a function of the entity too, so
            // a selection change is a real id change and egui starts clean.
            ui.push_id(entity, |ui| {
                ui.label(format!(
                    "Entity {} (gen {})",
                    entity.index(),
                    entity.generation()
                ));
                ui.separator();

                let mut claim = None;
                if let Some(transform) = frame.world.get_mut::<Transform>(entity) {
                    claim = transform_ui(ui, transform);
                }
                if let Some(sprite) = frame.world.get_mut::<Sprite>(entity) {
                    ui.separator();
                    claim = claim.or(sprite_ui(
                        ui,
                        sprite,
                        frame.textures,
                        frame.device,
                        frame.queue,
                    ));
                }
                // A field held down or focused keeps one entry open for the
                // whole edit, the way a gizmo drag does. Collected and applied
                // here rather than pushed from inside the widget helpers: they
                // hold a `&mut` borrowed out of the world, and the history has
                // to read the world.
                if let Some(label) = claim {
                    editor.history.claim(label);
                }

                ui.separator();
                if ui
                    .button(RichText::new("Delete").color(Color32::LIGHT_RED))
                    .clicked()
                {
                    // Explicit rather than watched: the watcher's `after` is
                    // read at the end of the frame from an entity that no longer
                    // exists, which is right, but the selection has to be
                    // recorded as it was *before* the despawn cleared it.
                    let id = frame.world.get::<SceneId>(entity).copied();
                    let view = SceneView {
                        world: frame.world,
                        registry: &editor.registry,
                        selected: id,
                    };
                    editor.history.begin("Delete", view, id);

                    frame.world.despawn(entity);
                    editor.selected = None;

                    let view = SceneView {
                        world: frame.world,
                        registry: &editor.registry,
                        selected: None,
                    };
                    editor.history.commit(view);
                }
            });
        });
}

/// What to call an edit made through this widget this frame, if it is live.
///
/// Dragged *or* focused: a `DragValue` can also be clicked into and typed in,
/// and an entry that closed between two keystrokes would be one undo per
/// character.
fn active(response: &egui::Response, label: &'static str) -> Option<&'static str> {
    (response.dragged() || response.has_focus()).then_some(label)
}

fn transform_ui(ui: &mut Ui, transform: &mut Transform) -> Option<&'static str> {
    ui.label(RichText::new("Transform").strong());

    let mut claim = None;
    egui::Grid::new("transform").num_columns(2).show(ui, |ui| {
        ui.label("position");
        ui.horizontal(|ui| {
            claim = claim
                .or(active(
                    &ui.add(DragValue::new(&mut transform.translation.x).speed(0.01)),
                    "Move",
                ))
                .or(active(
                    &ui.add(DragValue::new(&mut transform.translation.y).speed(0.01)),
                    "Move",
                ));
        });
        ui.end_row();

        ui.label("rotation");
        claim = claim.or(active(&ui.drag_angle(&mut transform.rotation), "Rotate"));
        ui.end_row();

        ui.label("scale");
        ui.horizontal(|ui| {
            claim = claim
                .or(active(
                    &ui.add(DragValue::new(&mut transform.scale.x).speed(0.01)),
                    "Scale",
                ))
                .or(active(
                    &ui.add(DragValue::new(&mut transform.scale.y).speed(0.01)),
                    "Scale",
                ));
        });
        ui.end_row();
    });
    claim
}

fn sprite_ui(
    ui: &mut Ui,
    sprite: &mut Sprite,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<&'static str> {
    ui.label(RichText::new("Sprite").strong());

    let mut claim = None;
    egui::Grid::new("sprite").num_columns(2).show(ui, |ui| {
        ui.label("colour");
        // The colour is edited inside a popup, so the button itself is never
        // dragged or focused while the value moves and [`active`] would report
        // nothing — one entry per frame of the pick. The popup's id is
        // `ui.auto_id_with("popup")` taken before the button is allocated
        // (egui-0.35 `widgets/color_picker.rs:519`), and `auto_id_with` does
        // not advance the counter, so reading it here gives the same id the
        // widget will derive a line later.
        let popup = ui.auto_id_with("popup");
        let response = ui.color_edit_button_rgba_unmultiplied(&mut sprite.color);
        if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
            claim = claim.or(Some("Set colour"));
        }
        ui.end_row();

        ui.label("sort order");
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut sprite.sort_order)),
            "Set sort order",
        ));
        ui.end_row();
    });

    ui.separator();
    claim.or(texture_ui(ui, sprite, textures, device, queue))
}

/// The texture path editor: a `TextEdit` plus a `Clear` button.
///
/// The typed text lives in egui's own per-id storage, not a field on
/// `Editor` — the id is salted by `entity` through the caller's `push_id`,
/// so switching the selection lands on a fresh slot instead of showing the
/// previous entity's half-typed path. Seeded once from `sprite.texture` and
/// left alone after that: re-cloning the path into the buffer every frame
/// would erase whatever the user just typed the moment the buffer is read
/// back before a commit.
fn texture_ui(
    ui: &mut Ui,
    sprite: &mut Sprite,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<&'static str> {
    ui.label(RichText::new("Texture").strong());

    let buffer_id = ui.id().with("texture_path");
    // Owned rather than borrowed from `sprite.texture`: the comparison below
    // runs inside a closure that also calls `sprite.set_texture`, and a
    // `&str` still borrowing `sprite` at that point would fight that
    // mutation for no reason — the committed path does not change shape
    // once read.
    let committed = sprite
        .texture
        .as_ref()
        .map(AssetPath::as_str)
        .unwrap_or("")
        .to_owned();
    let mut buffer = ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_insert_with::<String>(buffer_id, || committed.clone())
            .clone()
    });

    let mut claim = None;
    ui.horizontal(|ui| {
        let response = ui.add(TextEdit::singleline(&mut buffer).hint_text("path/to/texture.png"));
        let clear_clicked = ui.button("Clear").clicked();

        // Clicking `Clear` also moves focus off the `TextEdit`, so both
        // conditions can be true on the same frame; `else if` makes Clear win
        // rather than letting the stale buffer commit and then immediately
        // get overwritten, which loaded a texture just to discard it.
        //
        // `lost_focus` alone also covers Enter: a singleline `TextEdit`
        // surrenders focus on it. Guarded on a real change so clicking in
        // and out without editing does not re-run `set_texture` for no
        // reason.
        if clear_clicked {
            claim = Some("Set texture");
            sprite.set_texture(None, textures, device, queue);
            buffer.clear();
        } else if response.lost_focus() && buffer != committed {
            claim = Some("Set texture");
            match AssetPath::new(&buffer) {
                Ok(path) => sprite.set_texture(Some(path), textures, device, queue),
                Err(e) => log::error!("invalid texture path {buffer:?}: {e}"),
            }
            // Resyncs the box: an accepted edit shows the normalised path,
            // a rejected one reverts rather than leaving bad text sitting
            // there looking committed.
            buffer = sprite
                .texture
                .as_ref()
                .map(AssetPath::as_str)
                .unwrap_or("")
                .to_owned();
        }
    });

    ui.ctx()
        .data_mut(|data| data.insert_temp(buffer_id, buffer));

    claim
}
