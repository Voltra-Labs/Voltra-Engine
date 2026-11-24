//! A text's content, face and look.

use voltra_assets::Fonts;
use voltra_core::egui::{self, DragValue, RichText, TextEdit, Ui};
use voltra_scene::{Align, Text};

use super::active;
use super::path_field::{self, PathEdit};

/// Every field of a [`Text`]. One undo entry per edit through the caller's
/// `claim`, as every other section.
///
/// `on_screen` for a text in a `UiRect`: its size is in points and it is
/// rasterized at that size, so `pixels per em` does nothing and is hidden.
pub(super) fn show(
    ui: &mut Ui,
    text: &mut Text,
    fonts: &mut Fonts,
    on_screen: bool,
) -> Option<&'static str> {
    ui.label(RichText::new("Text").strong());

    // Multiline, and claimed while focused: one entry for a sentence typed,
    // not one per keystroke. A keyed text's content is its language's
    // string, written every frame: shown, not edited.
    let mut claim = active(
        &ui.add_enabled(
            text.key.is_none(),
            TextEdit::multiline(&mut text.content)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        ),
        "Edit text",
    );
    claim = claim.or(key(ui, text));

    egui::Grid::new("text").num_columns(2).show(ui, |ui| {
        ui.label("size").on_hover_text(if on_screen {
            "the em height, in points"
        } else {
            "the em height, in entity units"
        });
        claim = claim.or(active(
            &ui.add(
                DragValue::new(&mut text.size)
                    .speed(0.01)
                    .range(0.01..=f32::MAX),
            ),
            "Set text size",
        ));
        ui.end_row();

        if !on_screen {
            ui.label("pixels per em")
                .on_hover_text("raster resolution: higher is crisper when zoomed in");
            claim = claim.or(active(
                &ui.add(DragValue::new(&mut text.pixels_per_em).range(1..=Text::MAX_PIXELS_PER_EM)),
                "Set text resolution",
            ));
            ui.end_row();
        }

        claim = claim.or(wrap(ui, text, on_screen));

        ui.label("colour");
        // A popup, as the sprite's colour is: see `sprite::show` for why the
        // id is read before the button.
        let popup = ui.auto_id_with("popup");
        let response = ui.color_edit_button_rgba_unmultiplied(&mut text.color);
        if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
            claim = claim.or(Some("Set text colour"));
        }
        ui.end_row();

        ui.label("align");
        ui.horizontal(|ui| {
            for (align, label) in [
                (Align::Left, "Left"),
                (Align::Center, "Center"),
                (Align::Right, "Right"),
            ] {
                if ui.selectable_value(&mut text.align, align, label).changed() {
                    claim = claim.or(Some("Align text"));
                }
            }
        });
        ui.end_row();

        ui.label("sort order");
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut text.sort_order)),
            "Set sort order",
        ));
        ui.end_row();
    });

    ui.label("font");
    let committed = text
        .font
        .as_ref()
        .map(|path| path.as_str().to_owned())
        .unwrap_or_default();
    if let Some(edit) = path_field::show(ui, "font_path", "default font", &committed) {
        let path = match edit {
            PathEdit::Cleared => None,
            PathEdit::Set(path) => Some(path),
        };
        text.set_font(path, fonts);
        claim = claim.or(Some("Set font"));
    }
    claim.or(super::light_layers::show(
        ui,
        "text_light_layers",
        "light layers",
        "its glyphs are lit only by lights sharing one of these",
        &mut text.light_layers,
    ))
}

/// The string-table key the content comes from, or none.
fn key(ui: &mut Ui, text: &mut Text) -> Option<&'static str> {
    let mut claim = None;
    ui.horizontal(|ui| {
        let mut keyed = text.key.is_some();
        if ui
            .checkbox(&mut keyed, "translated")
            .on_hover_text("the content is the current language's string for a key in locale/")
            .changed()
        {
            text.key = keyed.then(|| text.content.clone());
            claim = Some("Translate text");
        }
        if let Some(key) = &mut text.key {
            claim = claim.or(active(
                &ui.add(TextEdit::singleline(key).hint_text("key")),
                "Edit text key",
            ));
        }
    });
    claim
}

/// Wrapping: on or off, the width, and the spacing between lines.
fn wrap(ui: &mut Ui, text: &mut Text, on_screen: bool) -> Option<&'static str> {
    let mut claim = None;
    ui.label("wrap").on_hover_text(if on_screen {
        "break lines between words at this width in points; the rect's width caps it"
    } else {
        "break lines between words at this width, in entity units"
    });
    ui.horizontal(|ui| {
        let mut wraps = text.max_width.is_some();
        if ui.checkbox(&mut wraps, "").changed() {
            let width = if on_screen {
                10_000.0
            } else {
                text.size * 10.0
            };
            text.max_width = wraps.then_some(width);
            claim = Some("Wrap text");
        }
        if let Some(width) = &mut text.max_width {
            claim = claim.or(active(
                &ui.add(DragValue::new(width).speed(0.05).range(0.01..=f32::MAX)),
                "Set wrap width",
            ));
        }
    });
    ui.end_row();
    ui.label("line spacing");
    claim = claim.or(active(
        &ui.add(
            DragValue::new(&mut text.line_spacing)
                .speed(0.01)
                .range(0.1..=10.0),
        ),
        "Set line spacing",
    ));
    ui.end_row();
    claim
}
