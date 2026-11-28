//! egui's colour buttons, writing the colour back only when the person
//! changes it.
//!
//! `color_edit_button_rgba_unmultiplied` and `_rgb` convert the colour
//! through HSV every frame they are drawn and write the result back: about
//! nine colours in ten come back an ulp off. Handed a field directly, the
//! button rewrote it each frame it was merely shown — a material file
//! rewritten and reloaded by selecting a mesh, a component drifting under a
//! still inspector. These take a copy and assign it only on `changed()`.

use voltra_core::egui::{Response, Ui};

/// An RGBA colour button (linear values, shown in sRGB) over `colour`.
pub(crate) fn rgba(ui: &mut Ui, colour: &mut [f32; 4]) -> Response {
    let mut copy = *colour;
    let response = ui.color_edit_button_rgba_unmultiplied(&mut copy);
    if response.changed() {
        *colour = copy;
    }
    response
}

/// An RGB colour button (linear values, shown in sRGB) over `colour`.
pub(crate) fn rgb(ui: &mut Ui, colour: &mut [f32; 3]) -> Response {
    let mut copy = *colour;
    let response = ui.color_edit_button_rgb(&mut copy);
    if response.changed() {
        *colour = copy;
    }
    response
}

#[cfg(test)]
mod tests {
    use voltra_core::egui;

    use super::*;

    /// Colours spread over the cube, as a material file holds them.
    fn colours() -> Vec<[f32; 4]> {
        (0..64)
            .map(|i| {
                let f = |k: u32| ((i * 37 + k * 11) % 97) as f32 / 97.0;
                [f(1), f(2), f(3), 1.0]
            })
            .collect()
    }

    #[test]
    fn a_colour_shown_and_not_touched_stays_bit_for_bit() {
        let ctx = egui::Context::default();
        let mut drifted_raw = 0;
        for colour in colours() {
            let (mut kept, mut kept_rgb, mut raw) =
                (colour, [colour[0], colour[1], colour[2]], colour);
            for _ in 0..3 {
                let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                    rgba(ui, &mut kept);
                    rgb(ui, &mut kept_rgb);
                    ui.color_edit_button_rgba_unmultiplied(&mut raw);
                });
            }
            assert_eq!(kept.map(f32::to_bits), colour.map(f32::to_bits));
            assert_eq!(kept_rgb, [colour[0], colour[1], colour[2]]);
            drifted_raw += usize::from(raw != colour);
        }
        // What the plain button does to the same colours, so this test
        // pins something: most come back changed.
        assert!(
            drifted_raw > 10,
            "{drifted_raw} of 64 drifted through the plain button"
        );
    }
}
