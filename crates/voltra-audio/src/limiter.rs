//! The last thing between the mix and the device.
//!
//! Ten quiet sounds are quiet; ten loud ones at once are not, and the sum
//! leaves the range a device accepts. Something has to give, and the choice is
//! *what*. Squaring the peaks off — the hard clip this replaces — keeps every
//! sound at the volume it was mixed at and destroys the shape of whichever one
//! was loudest, which is heard as a crack on the sound that mattered most.
//! Turning the whole mix down for as long as it is too loud keeps every shape
//! and costs a moment of level, which is heard as the mix ducking. Every
//! mastering chain in every engine makes the second choice, and so does this.
//!
//! Feed-forward and without look-ahead: the gain reacts to the peak it has
//! already seen rather than one it has been shown early. Look-ahead is the
//! standard way to catch a transient the attack cannot, and it costs the
//! buffer's worth of delay on everything — latency this engine spends nowhere
//! else. Without it a fast transient does get through, which is why the clamp
//! is still here underneath: the limiter is what stops the clip being reached,
//! not what replaces it.

use std::time::Duration;

/// The level the limiter holds the mix at.
///
/// Just under full scale rather than at it: a device handed exactly `1.0`
/// has nothing left for the rounding in its own conversion, and the last
/// percent is inaudible.
pub const THRESHOLD: f32 = 0.99;

/// How fast the gain comes down when the mix gets too loud.
///
/// One millisecond, which is the shortest time that does not itself distort:
/// a gain moving faster than the waveform underneath it modulates the sound
/// rather than levelling it, and the audible result is a buzz on bass.
const ATTACK: Duration = Duration::from_millis(1);

/// How fast the gain goes back up afterwards.
///
/// A hundred milliseconds. Short enough that the mix does not stay ducked
/// after one bang, long enough that a run of them is one duck rather than a
/// pumping that follows the rhythm of the game.
const RELEASE: Duration = Duration::from_millis(100);

/// The gain riding on the master sum, and the peak it is riding.
#[derive(Debug, Clone, Copy)]
pub struct Limiter {
    /// Where the peak is thought to be right now: the level the gain is
    /// derived from, smoothed so it does not follow every sample.
    envelope: f32,
    /// Per-frame blend towards a peak louder than the envelope.
    attack: f32,
    /// Per-frame blend towards a peak quieter than it.
    release: f32,
}

impl Limiter {
    /// A limiter for a device running at `rate`.
    pub fn new(rate: u32) -> Self {
        Self {
            envelope: 0.0,
            attack: coefficient(ATTACK, rate),
            release: coefficient(RELEASE, rate),
        }
    }

    /// Holds `out` at [`THRESHOLD`], `channels` samples per frame.
    ///
    /// One gain across the frame rather than one per channel: a limiter that
    /// ducked the loud ear alone would pull the whole image towards the quiet
    /// one every time something banged, which is a stereo field that wanders.
    /// Every mastering limiter is linked across channels for that reason.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        if channels == 0 {
            return;
        }

        for frame in out.chunks_mut(channels) {
            let mut peak = frame.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
            if !peak.is_finite() {
                peak = 0.0;
            }

            let blend = if peak > self.envelope {
                self.attack
            } else {
                self.release
            };
            self.envelope = peak + blend * (self.envelope - peak);

            let gain = if self.envelope > THRESHOLD {
                THRESHOLD / self.envelope
            } else {
                1.0
            };

            for sample in frame.iter_mut() {
                let value = *sample * gain;
                // The clamp the limiter is meant to stop anything reaching,
                // kept because without look-ahead a transient faster than the
                // attack still arrives before the gain does. A `NaN` from a
                // sample nothing else caught becomes silence here rather than
                // a crack — this is the last code the device's memory sees.
                *sample = if value.is_finite() {
                    value.clamp(-1.0, 1.0)
                } else {
                    0.0
                };
            }
        }
    }

    /// How much the mix is currently being turned down: `1.0` is untouched.
    ///
    /// For a meter, and for the tests that ask whether the gain moved.
    pub fn gain(&self) -> f32 {
        if self.envelope > THRESHOLD {
            THRESHOLD / self.envelope
        } else {
            1.0
        }
    }
}

/// The per-frame blend factor a time constant describes at `rate`.
///
/// `exp(-1 / (rate * seconds))`: the standard one-pole coefficient, the value
/// that leaves `1/e` of the distance after exactly `seconds`.
fn coefficient(time: Duration, rate: u32) -> f32 {
    let samples = time.as_secs_f32() * rate.max(1) as f32;
    if samples <= 0.0 {
        return 0.0;
    }
    (-1.0 / samples).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// `frames` frames of a constant level, in stereo.
    fn steady(level: f32, frames: usize) -> Vec<f32> {
        vec![level; frames * 2]
    }

    #[test]
    fn a_mix_that_fits_is_not_touched_at_all() {
        // Bit-identical, not merely close: a limiter that multiplied every
        // quiet sound by 0.999 would be an effect on everything.
        let mut limiter = Limiter::new(RATE);
        let mut out = steady(0.5, 128);
        let before = out.clone();

        limiter.process(&mut out, 2);

        assert_eq!(out, before);
        assert_eq!(limiter.gain(), 1.0);
    }

    #[test]
    fn a_mix_that_does_not_fit_is_turned_down_to_the_threshold() {
        let mut limiter = Limiter::new(RATE);
        // A tenth of a second: long enough for a one-millisecond attack to
        // have arrived many times over.
        let mut out = steady(4.0, RATE as usize / 10);

        limiter.process(&mut out, 2);

        let settled = out[out.len() - 2];
        assert!(
            (settled - THRESHOLD).abs() < 1e-3,
            "settled at {settled}, wanted {THRESHOLD}"
        );
    }

    #[test]
    fn nothing_ever_leaves_the_range_a_device_accepts() {
        // The property the clamp underneath is kept for: the first
        // milliseconds of a transient are louder than the gain has caught up
        // with, and they still must not reach the device out of range.
        let mut limiter = Limiter::new(RATE);
        let mut out = steady(12.0, 4_096);

        limiter.process(&mut out, 2);

        assert!(out.iter().all(|s| (-1.0..=1.0).contains(s)));
    }

    #[test]
    fn the_gain_comes_back_up_after_the_bang() {
        // Otherwise one loud moment would leave the game quiet for the rest of
        // the session.
        let mut limiter = Limiter::new(RATE);
        limiter.process(&mut steady(8.0, 1_024), 2);
        assert!(limiter.gain() < 0.5, "the bang should duck the mix");

        // A second of silence, which is ten release constants.
        limiter.process(&mut steady(0.0, RATE as usize), 2);

        assert_eq!(limiter.gain(), 1.0);
    }

    #[test]
    fn both_channels_are_ducked_together() {
        // A limiter that ducked only the loud ear would pull the stereo image
        // sideways on every bang.
        let mut limiter = Limiter::new(RATE);
        let mut out = Vec::new();
        for _ in 0..RATE as usize / 10 {
            out.extend_from_slice(&[4.0, 1.0]);
        }

        limiter.process(&mut out, 2);

        let (left, right) = (out[out.len() - 2], out[out.len() - 1]);
        assert!((left / right - 4.0).abs() < 1e-3, "got {left} and {right}");
    }

    #[test]
    fn a_nan_in_the_mix_becomes_silence_rather_than_a_crack() {
        let mut limiter = Limiter::new(RATE);
        let mut out = vec![f32::NAN, 0.5, f32::INFINITY, -0.25];

        limiter.process(&mut out, 2);

        assert!(out.iter().all(|s| s.is_finite()), "got {out:?}");
        assert_eq!(out[0], 0.0);
    }

    #[test]
    fn a_mono_device_is_limited_the_same_way() {
        let mut limiter = Limiter::new(RATE);
        let mut out = vec![3.0; RATE as usize / 10];

        limiter.process(&mut out, 1);

        let settled = out[out.len() - 1];
        assert!((settled - THRESHOLD).abs() < 1e-3, "got {settled}");
    }

    #[test]
    fn an_empty_buffer_and_a_channelless_device_are_not_special_cases() {
        let mut limiter = Limiter::new(RATE);
        limiter.process(&mut [], 2);
        limiter.process(&mut [0.5, 0.5], 0);
    }
}
