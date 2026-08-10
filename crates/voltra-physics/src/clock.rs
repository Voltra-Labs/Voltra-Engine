//! How many fixed steps a variable frame owes.
//!
//! Physics integrated with the render delta changes behaviour with the frame
//! rate — the same scene settles differently at 60 and 144 Hz, and a stall
//! makes it explode. A fixed step with an accumulator is what every engine does
//! instead.

/// Steps allowed in one frame.
///
/// The spiral of death is why there is a cap at all: a frame slow enough to owe
/// many steps gets slower still by running them, and then owes more. Past this
/// the debt is dropped and simulated time runs slow, which is what every engine
/// chooses — the alternative is a hang.
const MAX_STEPS: u32 = 8;

/// Default step: 60 Hz. Fast enough that a body does not tunnel at ordinary
/// speeds, slow enough to leave the frame budget alone.
const DEFAULT_STEP: f32 = 1.0 / 60.0;

/// The fixed step, and the time debt carried between frames.
#[derive(Debug, Clone)]
pub struct PhysicsClock {
    accumulator: f32,
    step: f32,
    max_steps: u32,
}

impl Default for PhysicsClock {
    fn default() -> Self {
        Self::new(DEFAULT_STEP)
    }
}

impl PhysicsClock {
    /// A clock stepping at `step` seconds.
    ///
    /// A step of zero or less falls back to the default: it would divide by
    /// zero below, and a caller or a scene can say anything.
    pub fn new(step: f32) -> Self {
        Self {
            accumulator: 0.0,
            step: if step > 0.0 { step } else { DEFAULT_STEP },
            max_steps: MAX_STEPS,
        }
    }

    /// How many steps to run for a frame of `delta` seconds.
    ///
    /// The remainder stays in the accumulator, so a frame owing three quarters
    /// of a step is not simply lost — dropping it makes physics run slow by a
    /// fraction that depends on the frame rate.
    pub fn steps(&mut self, delta: f32) -> u32 {
        if delta > 0.0 {
            self.accumulator += delta;
        }

        let owed = (self.accumulator / self.step) as u32;
        if owed >= self.max_steps {
            // Dropped, not carried. Carrying it means the cap only defers the
            // spiral to the next frame, which starts already owing the rest.
            self.accumulator = 0.0;
            return self.max_steps;
        }

        self.accumulator -= owed as f32 * self.step;
        owed
    }

    /// The fixed step, in seconds. What to hand to `step`.
    pub fn step(&self) -> f32 {
        self.step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: f32 = 1.0 / 60.0;

    #[test]
    fn a_delta_below_one_step_runs_nothing() {
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.steps(STEP * 0.5), 0);
    }

    #[test]
    fn exactly_one_step_runs_once() {
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.steps(STEP), 1);
    }

    #[test]
    fn two_steps_worth_runs_twice() {
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.steps(STEP * 2.0), 2);
    }

    #[test]
    fn the_remainder_carries_to_the_next_frame() {
        // Two frames of 0.75 steps owe zero and then one, and the leftover
        // half must survive. Dropping it makes physics run slow by a fraction
        // that depends on the frame rate.
        let mut clock = PhysicsClock::default();

        assert_eq!(clock.steps(STEP * 0.75), 0);
        assert_eq!(clock.steps(STEP * 0.75), 1);
    }

    #[test]
    fn a_huge_delta_is_capped() {
        // The spiral of death: running every owed step makes the next frame
        // longer, which owes more steps again. Every engine drops the debt.
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.steps(STEP * 1000.0), 8);
    }

    #[test]
    fn the_accumulator_does_not_keep_the_dropped_debt() {
        // Otherwise the cap only defers the spiral: the next frame would start
        // already owing 992 steps.
        let mut clock = PhysicsClock::default();
        clock.steps(STEP * 1000.0);

        assert_eq!(clock.steps(0.0), 0);
    }

    #[test]
    fn a_negative_delta_runs_nothing_and_does_not_rewind() {
        let mut clock = PhysicsClock::default();

        assert_eq!(clock.steps(-1.0), 0);
        assert_eq!(clock.steps(STEP), 1);
    }

    #[test]
    fn a_custom_step_is_honoured() {
        let mut clock = PhysicsClock::new(0.5);

        assert_eq!(clock.steps(1.0), 2);
        assert_eq!(clock.step(), 0.5);
    }

    #[test]
    fn a_non_positive_step_falls_back_to_the_default() {
        assert_eq!(PhysicsClock::new(0.0).step(), STEP);
        assert_eq!(PhysicsClock::new(-1.0).step(), STEP);
    }
}
