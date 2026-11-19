//! A gain that slides to where it was put instead of jumping there.
//!
//! Every gain the mixer applies goes through one of these, and the reason is
//! the same one every mixer has: a gain written straight into the multiply
//! steps between two values inside a buffer, and a step in a waveform is a
//! click. Wwise calls it interpolation, JUCE calls it a `SmoothedValue`, Web
//! Audio calls it `linearRampToValueAtTime`; all three exist because the
//! alternative is audible.
//!
//! Linear rather than exponential. An exponential ramp is the better fade for
//! a long musical cross-fade, because loudness is logarithmic — but the ramp
//! that runs a thousand times more often here is the fifteen-millisecond one
//! that smooths a source moving across the screen, and over fifteen
//! milliseconds the two curves are indistinguishable. One curve that is right
//! for the common case and close enough for the rare one beats two.

/// A gain on its way from what it was to what it was last set to.
///
/// Advanced one output frame at a time by [`tick`](Self::tick), or in bulk by
/// [`advance`](Self::advance) for a bus whose ramp has to stay on schedule
/// through a buffer where nothing was routed into it.
///
/// Never negative and never `NaN`: a gain is the last thing multiplied into a
/// sample, so a bad value here reaches the speakers as noise. Both are folded
/// to silence on the way in, once, rather than checked at every use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    value: f32,
    target: f32,
    /// How far `value` moves per frame while the ramp is running.
    step: f32,
    /// Frames left before `value` is exactly `target`. Zero means settled.
    remaining: u32,
}

impl Ramp {
    /// A ramp sitting at `value`, going nowhere.
    pub fn new(value: f32) -> Self {
        let value = gain(value);
        Self {
            value,
            target: value,
            step: 0.0,
            remaining: 0,
        }
    }

    /// Sets the value with no ramp at all — what a voice's opening gain is.
    pub fn jump(&mut self, target: f32) {
        let target = gain(target);
        self.value = target;
        self.target = target;
        self.step = 0.0;
        self.remaining = 0;
    }

    /// Heads for `target`, arriving in `frames` frames.
    ///
    /// Retargeting mid-ramp is not a special case: the new step is measured
    /// from wherever the value has got to, so a fade interrupted by another
    /// fade continues from the gain that is actually being heard rather than
    /// snapping back to where the first one started.
    pub fn set(&mut self, target: f32, frames: u32) {
        let target = gain(target);
        if frames == 0 {
            self.jump(target);
            return;
        }
        self.target = target;
        self.remaining = frames;
        self.step = (target - self.value) / frames as f32;
    }

    /// The gain right now.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Where the ramp is heading, which is where it already is once settled.
    pub fn target(&self) -> f32 {
        self.target
    }

    /// Whether the ramp has arrived.
    ///
    /// What tells a fading-out voice it has finished fading, which is the only
    /// thing that ever ends one.
    pub fn is_settled(&self) -> bool {
        self.remaining == 0
    }

    /// The gain for this frame, advancing one frame.
    pub fn tick(&mut self) -> f32 {
        let value = self.value;
        self.advance(1);
        value
    }

    /// Advances `frames` frames without reading the gain.
    ///
    /// For a bus with nothing routed into it this buffer. Its gain multiplied
    /// nothing, but a fade that paused whenever the bus fell silent would
    /// arrive late — or never, for a bus fading out.
    pub fn advance(&mut self, frames: u32) {
        if self.remaining == 0 {
            return;
        }
        let frames = frames.min(self.remaining);
        self.remaining -= frames;
        // The last frame assigns rather than accumulates: `step * frames`
        // summed in `f32` lands near the target, not on it, and a gain that
        // settles at 0.999 never lets a fading voice finish.
        if self.remaining == 0 {
            self.value = self.target;
        } else {
            self.value += self.step * frames as f32;
        }
    }
}

/// A value fit to multiply a sample by: finite, and never below silence.
///
/// A negative gain inverts the phase rather than going quiet, which is what a
/// volume slider dragged past its end must not do, and one `NaN` multiplied
/// into the mix stays `NaN` for every sample after it.
fn gain(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_ramp_is_already_where_it_was_put() {
        let ramp = Ramp::new(0.5);
        assert_eq!(ramp.value(), 0.5);
        assert_eq!(ramp.target(), 0.5);
        assert!(ramp.is_settled());
    }

    #[test]
    fn a_ramp_arrives_exactly_on_its_target() {
        // Exactly, not nearly: a fade-out that settles at 1e-8 is a voice that
        // never finishes and a slot that is never freed.
        let mut ramp = Ramp::new(1.0);
        ramp.set(0.0, 3);

        for _ in 0..3 {
            ramp.tick();
        }

        assert_eq!(ramp.value(), 0.0);
        assert!(ramp.is_settled());
    }

    #[test]
    fn the_frames_in_between_are_evenly_spaced() {
        let mut ramp = Ramp::new(0.0);
        ramp.set(1.0, 4);

        let heard: Vec<f32> = (0..4).map(|_| ramp.tick()).collect();

        assert_eq!(heard, vec![0.0, 0.25, 0.5, 0.75]);
        assert_eq!(ramp.value(), 1.0, "and the fifth frame is the target");
    }

    #[test]
    fn a_settled_ramp_holds_its_value_forever() {
        let mut ramp = Ramp::new(0.25);
        for _ in 0..100 {
            assert_eq!(ramp.tick(), 0.25);
        }
    }

    #[test]
    fn zero_frames_is_a_jump() {
        // What a voice that is not fading in wants, and what a `Stop` wants.
        let mut ramp = Ramp::new(1.0);
        ramp.set(0.0, 0);
        assert_eq!(ramp.value(), 0.0);
        assert!(ramp.is_settled());
    }

    #[test]
    fn retargeting_continues_from_the_gain_being_heard() {
        // A fade-out interrupted by a fade-in must not snap back to full
        // volume first — that is the click the ramp exists to remove.
        let mut ramp = Ramp::new(1.0);
        ramp.set(0.0, 4);
        ramp.tick();
        ramp.tick();
        let interrupted = ramp.value();

        ramp.set(1.0, 4);

        assert_eq!(ramp.value(), interrupted);
        assert!((interrupted - 0.5).abs() < 1e-6, "got {interrupted}");
    }

    #[test]
    fn advancing_in_bulk_lands_where_one_frame_at_a_time_does() {
        // The property that lets a silent bus skip its buffers: a fade must
        // not depend on whether anything was routed through it.
        let mut stepped = Ramp::new(1.0);
        let mut bulk = Ramp::new(1.0);
        stepped.set(0.0, 64);
        bulk.set(0.0, 64);

        for _ in 0..64 {
            stepped.tick();
        }
        bulk.advance(64);

        assert_eq!(stepped.value(), bulk.value());
        assert!(stepped.is_settled() && bulk.is_settled());
    }

    #[test]
    fn advancing_past_the_end_settles_rather_than_overshooting() {
        let mut ramp = Ramp::new(0.0);
        ramp.set(1.0, 8);

        ramp.advance(1_000);

        assert_eq!(ramp.value(), 1.0);
    }

    #[test]
    fn a_negative_gain_is_silence_rather_than_an_inverted_phase() {
        let mut ramp = Ramp::new(-2.0);
        assert_eq!(ramp.value(), 0.0);

        ramp.set(-1.0, 4);
        assert_eq!(ramp.target(), 0.0);
    }

    #[test]
    fn a_nan_never_becomes_a_gain() {
        // One NaN multiplied into the mix stays NaN for the rest of the
        // buffer, and reaches the device as a crack.
        let mut ramp = Ramp::new(f32::NAN);
        assert_eq!(ramp.value(), 0.0);

        ramp.jump(1.0);
        ramp.set(f32::INFINITY, 2);
        ramp.advance(2);
        assert!(ramp.value().is_finite());
        assert_eq!(ramp.value(), 0.0);
    }
}
