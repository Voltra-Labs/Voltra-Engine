//! What a drag looks like while it is in flight.
//!
//! Shared because two panels start drags and three accept them: the hierarchy
//! carries an `Entity`, the asset browser carries an `AssetPath`, and the
//! viewport and the inspector are targets. The feedback has to look the same
//! whichever pair is involved, which is what a second copy of these would
//! eventually stop doing.

use voltra_core::egui::{
    Align2, Color32, CornerRadius, Id, LayerId, Order, Rect, Stroke, StrokeKind, TextStyle, Ui,
    Vec2,
};

/// Draws the box that says a drop would land here.
///
/// An outline rather than a filled highlight: the target is drawn by the time
/// this runs, and a fill over it would hide what the user is aiming at.
pub(crate) fn outline(ui: &Ui, rect: Rect, color: Color32) {
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.5, color),
        StrokeKind::Inside,
    );
}

/// Draws what is being carried, at the cursor.
///
/// On the tooltip layer, so it is over every panel: a drag that leaves the
/// panel it started in still has to show what it holds. This is the part
/// `dnd_drag_source` does for free, and the reason it is hand-rolled is that
/// the same call also swallows the click that selects.
pub(crate) fn ghost(ui: &Ui, text: &str) {
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let painter = ui
        .ctx()
        .layer_painter(LayerId::new(Order::Tooltip, Id::new("drag-ghost")));
    painter.text(
        pointer + Vec2::new(12.0, 0.0),
        Align2::LEFT_CENTER,
        text,
        TextStyle::Body.resolve(ui.style()),
        ui.visuals().strong_text_color(),
    );
}
