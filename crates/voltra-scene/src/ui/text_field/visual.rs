//! The caret moved by arrows in visual order: in mixed-direction text the
//! right arrow goes right on screen, whichever way the letters under it run.
//!
//! What a browser's `<input>` and Godot's `LineEdit` do (Unity's
//! `InputField` moves logically, so an arrow in Hebrew goes the "wrong"
//! way). The stops come from the layout (one per character, in logical
//! order, at its leading edge), so the order of positions on screen is the
//! order of their x; where two logical positions share an x — a run
//! boundary — they are visited one after the other.

/// The caret index one stop right of `caret` (or left), given `stops`: the
/// x of each caret index, `chars + 1` of them. `None` when there is no
/// stop that way, or the stops do not fit the text.
pub(crate) fn step(stops: &[f32], caret: usize, right: bool) -> Option<usize> {
    if caret >= stops.len() || stops.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let mut order: Vec<usize> = (0..stops.len()).collect();
    order.sort_by(|a, b| stops[*a].total_cmp(&stops[*b]).then(a.cmp(b)));
    let at = order.iter().position(|i| *i == caret)?;
    let next = if right {
        at.checked_add(1).filter(|n| *n < order.len())?
    } else {
        at.checked_sub(1)?
    };
    Some(order[next])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_to_right_text_steps_as_its_indices_do() {
        let stops = [0.0, 10.0, 20.0];
        assert_eq!(step(&stops, 0, true), Some(1));
        assert_eq!(step(&stops, 2, true), None);
        assert_eq!(step(&stops, 1, false), Some(0));
    }

    #[test]
    fn right_to_left_text_steps_backward_through_its_indices() {
        // A Hebrew word: index 0 is on the right.
        let stops = [30.0, 20.0, 10.0, 0.0];
        assert_eq!(step(&stops, 0, true), None, "already rightmost");
        assert_eq!(step(&stops, 0, false), Some(1));
        assert_eq!(step(&stops, 3, true), Some(2));
    }

    #[test]
    fn a_mixed_line_is_walked_left_to_right_on_screen() {
        // "ab" then a Hebrew word of three letters: ab at 0 and 10, the
        // Hebrew from its right end (50) leftward, its end stop at 20.
        let stops = [0.0, 10.0, 50.0, 40.0, 30.0, 20.0];
        let mut caret = 0;
        let mut walked = vec![caret];
        while let Some(next) = step(&stops, caret, true) {
            caret = next;
            walked.push(caret);
        }
        let xs: Vec<f32> = walked.iter().map(|i| stops[*i]).collect();
        assert!(xs.windows(2).all(|w| w[1] >= w[0]), "{xs:?}");
        assert_eq!(walked.len(), stops.len(), "every position visited");
    }

    #[test]
    fn stops_that_do_not_fit_move_nothing() {
        assert_eq!(step(&[0.0, 1.0], 5, true), None);
        assert_eq!(step(&[0.0, f32::NAN], 0, true), None);
    }
}
