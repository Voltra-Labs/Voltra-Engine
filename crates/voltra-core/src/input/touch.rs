//! Fingers on the screen: where each went down, where it is, and whether it
//! went down or up this frame.

use voltra_render::glam::Vec2;

/// One finger, in physical pixels from the window's top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    /// The platform's id for the finger, stable while it is down.
    pub id: u64,
    /// Where it went down.
    pub start: Vec2,
    /// Where it is now.
    pub position: Vec2,
    /// It went down this frame.
    pub began: bool,
    /// It lifted, or the platform cancelled it, this frame. Kept until the
    /// frame ends, so a tap inside one frame is still seen.
    pub ended: bool,
}

/// What a platform said happened to a finger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// Every finger down this frame, and those that lifted in it.
#[derive(Debug, Clone, Default)]
pub(super) struct Touches {
    list: Vec<Touch>,
}

impl Touches {
    pub(super) fn on_touch(&mut self, id: u64, phase: TouchPhase, position: Vec2) {
        let found = self.list.iter_mut().find(|t| t.id == id && !t.ended);
        match (phase, found) {
            (TouchPhase::Started, _) => self.list.push(Touch {
                id,
                start: position,
                position,
                began: true,
                ended: false,
            }),
            (TouchPhase::Moved, Some(touch)) => touch.position = position,
            (TouchPhase::Ended | TouchPhase::Cancelled, Some(touch)) => {
                touch.position = position;
                touch.ended = true;
            }
            // A move or an end for a finger never seen down: the window got
            // focus mid-touch. Nothing to follow.
            _ => {}
        }
    }

    pub(super) fn list(&self) -> &[Touch] {
        &self.list
    }

    /// Lifted fingers go; the rest are no longer new.
    pub(super) fn end_frame(&mut self) {
        self.list.retain(|t| !t.ended);
        for touch in &mut self.list {
            touch.began = false;
        }
    }

    /// Every finger lifts: the window lost focus.
    pub(super) fn release_all(&mut self) {
        for touch in &mut self.list {
            touch.ended = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finger_goes_down_moves_and_lifts_over_frames() {
        let mut touches = Touches::default();
        touches.on_touch(7, TouchPhase::Started, Vec2::new(10.0, 20.0));
        assert!(touches.list()[0].began);
        touches.end_frame();
        touches.on_touch(7, TouchPhase::Moved, Vec2::new(30.0, 20.0));
        let t = touches.list()[0];
        assert!(!t.began && t.start == Vec2::new(10.0, 20.0) && t.position.x == 30.0);
        touches.on_touch(7, TouchPhase::Ended, Vec2::new(31.0, 20.0));
        assert!(touches.list()[0].ended, "seen lifting this frame");
        touches.end_frame();
        assert!(touches.list().is_empty());
    }

    #[test]
    fn a_tap_inside_one_frame_is_seen_down_and_up() {
        let mut touches = Touches::default();
        touches.on_touch(1, TouchPhase::Started, Vec2::ZERO);
        touches.on_touch(1, TouchPhase::Ended, Vec2::ZERO);
        let t = touches.list()[0];
        assert!(t.began && t.ended);
    }

    #[test]
    fn a_move_for_an_unknown_finger_is_ignored() {
        let mut touches = Touches::default();
        touches.on_touch(3, TouchPhase::Moved, Vec2::ONE);
        assert!(touches.list().is_empty());
    }
}
