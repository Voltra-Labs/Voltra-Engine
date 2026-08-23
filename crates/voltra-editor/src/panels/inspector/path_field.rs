//! A text box for an asset path that is also a drop target.
//!
//! Shared by every component field that names a file — the sprite's texture
//! and its atlas today. The typing, the drop, the Clear button and the rule
//! about which of them wins on a frame where two happen are one behaviour, and
//! a second copy of it would be the one that gets the rule wrong.

use voltra_assets::AssetPath;
use voltra_core::egui::{self, Frame, Stroke, TextEdit, Ui};

/// What the field says the path should now be.
pub(super) enum PathEdit {
    /// The Clear button, or an empty box committed.
    Cleared,
    Set(AssetPath),
}

/// Draws the field for `committed` and reports a change, if there was one.
///
/// The typed text lives in egui's own per-id storage, not a field on `Editor`
/// — the id is salted by the entity through the caller's `push_id`, so
/// switching the selection lands on a fresh slot instead of showing the
/// previous entity's half-typed path. Seeded once from `committed` and left
/// alone after that: re-cloning it into the buffer every frame would erase
/// whatever the user just typed the moment the buffer is read back.
///
/// `salt` separates two fields on the same entity; without it the texture and
/// the atlas would share one buffer and one another's text.
pub(super) fn show(
    ui: &mut Ui,
    salt: &'static str,
    hint: &str,
    committed: &str,
) -> Option<PathEdit> {
    let buffer_id = ui.id().with(salt);
    let mut buffer = ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_insert_with::<String>(buffer_id, || committed.to_owned())
            .clone()
    });

    let mut edit = None;
    // The field is a drop target as well as a text box, which is how the same
    // assignment is made in Unity, Unreal and Godot: a path is typed once and
    // dragged from the browser every time after that. The frame gains a border
    // while a payload is in the air so the target is visible before the release
    // rather than discovered by trying it.
    let armed = egui::DragAndDrop::has_payload_of_type::<AssetPath>(ui.ctx());
    let zone = if armed {
        Frame::default().stroke(Stroke::new(1.5, ui.visuals().selection.bg_fill))
    } else {
        Frame::default()
    };

    let (_, dropped) = ui.dnd_drop_zone::<AssetPath, _>(zone, |ui| {
        ui.horizontal(|ui| {
            let response = ui.add(TextEdit::singleline(&mut buffer).hint_text(hint));
            let clear_clicked = ui.button("Clear").clicked();

            // Clicking `Clear` also moves focus off the `TextEdit`, so both
            // conditions can be true on the same frame; `else if` makes Clear
            // win rather than letting the stale buffer commit and then
            // immediately get overwritten, which loaded an asset just to
            // discard it.
            //
            // `lost_focus` alone also covers Enter: a singleline `TextEdit`
            // surrenders focus on it. Guarded on a real change so clicking in
            // and out without editing does not re-load for no reason.
            if clear_clicked {
                edit = Some(PathEdit::Cleared);
                buffer.clear();
            } else if response.lost_focus() && buffer != committed {
                if buffer.is_empty() {
                    // Emptying the box is the other way to say Clear, and the
                    // one a keyboard reaches. The path invariant refuses an
                    // empty string, so without this it would log an error and
                    // revert on the way to doing nothing.
                    edit = Some(PathEdit::Cleared);
                } else {
                    match AssetPath::new(&buffer) {
                        Ok(path) => {
                            // The normalised spelling, so an accepted edit
                            // shows what was actually stored.
                            buffer = path.as_str().to_owned();
                            edit = Some(PathEdit::Set(path));
                        }
                        Err(e) => {
                            log::error!("invalid asset path {buffer:?}: {e}");
                            // Reverted rather than left sitting there looking
                            // committed.
                            buffer = committed.to_owned();
                        }
                    }
                }
            }
        });
    });

    // After the box, so a path dropped onto a field being typed into wins: the
    // drop is the more deliberate of the two gestures, and the text it replaces
    // was never committed.
    if let Some(path) = dropped {
        buffer = path.as_str().to_owned();
        edit = Some(PathEdit::Set((*path).clone()));
    }

    ui.ctx()
        .data_mut(|data| data.insert_temp(buffer_id, buffer));

    edit
}
