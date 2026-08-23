//! How a sound is to be played.

/// The settings a voice starts with.
///
/// Everything here can also be changed while the voice runs — a source that
/// moves relative to the listener is exactly that — except [`looping`], which
/// decides what happens at the end of the buffer and is read once.
///
/// [`looping`]: Self::looping
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayParams {
    /// Linear gain. `1.0` plays the file as recorded.
    ///
    /// Linear rather than decibels, matching Unity's `AudioSource.volume` and
    /// Godot's `volume_linear`. Decibels are the right unit for a mixing desk
    /// and the wrong one for a field an author drags to zero: `-80 dB` is the
    /// number that means silence, and no slider should have to know that.
    pub volume: f32,
    /// Playback speed, which also shifts the pitch. `1.0` is as recorded.
    ///
    /// One control rather than two, because resampling is the only mechanism
    /// here: shifting pitch without changing length is a phase vocoder, which
    /// is a subsystem and not a field. Unity's `pitch` behaves the same way.
    pub pitch: f32,
    /// Where the sound sits between the ears: `-1.0` left, `0.0` centre,
    /// `1.0` right.
    ///
    /// Applied with a constant-power law — the gains are `cos` and `sin` of
    /// the angle the pan describes, so the two channels sum to the same power
    /// at every position and a sound does not swell as it crosses the centre.
    /// A centred sound is therefore about `-3 dB` in each ear, which is what
    /// Unity, Wwise and every mixing desk do.
    pub pan: f32,
    /// Whether the clip starts again at its end instead of finishing.
    ///
    /// A looping voice runs until something stops it, so the caller must keep
    /// its [`VoiceId`](crate::VoiceId) — nothing else will ever end it.
    pub looping: bool,
}

impl Default for PlayParams {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pitch: 1.0,
            pan: 0.0,
            looping: false,
        }
    }
}

impl PlayParams {
    /// The two channel gains this volume and pan describe: left, then right.
    ///
    /// A negative volume is clamped to zero rather than inverting the phase —
    /// a volume slider dragged past its end must go quiet, not turn the sound
    /// upside down — and a `NaN` in either field becomes silence rather than
    /// poisoning every sample it is added to. That guard lives here, once,
    /// because this is the only place either value reaches the mix.
    pub fn gains(&self) -> (f32, f32) {
        let volume = if self.volume.is_finite() {
            self.volume.max(0.0)
        } else {
            0.0
        };
        let pan = if self.pan.is_finite() {
            self.pan.clamp(-1.0, 1.0)
        } else {
            0.0
        };

        // -1 maps to 0 and +1 to a quarter turn, so the gains sweep from
        // (1, 0) through (0.707, 0.707) to (0, 1).
        let angle = (pan + 1.0) * (std::f32::consts::FRAC_PI_4);
        (volume * angle.cos(), volume * angle.sin())
    }

    /// The playback speed, guarded the way [`gains`](Self::gains) guards
    /// volume: never negative, never `NaN`.
    ///
    /// Zero is allowed and means the voice never advances, which is what a
    /// pitch dragged to zero should sound like — held, not reversed. Playing
    /// backwards is a real feature and a different one: it needs the cursor to
    /// start at the end and the finish test to run the other way.
    pub fn speed(&self) -> f32 {
        if self.pitch.is_finite() {
            self.pitch.max(0.0)
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(volume: f32, pan: f32) -> PlayParams {
        PlayParams {
            volume,
            pan,
            ..Default::default()
        }
    }

    #[test]
    fn the_default_plays_the_file_as_recorded() {
        let params = PlayParams::default();
        assert_eq!(params.volume, 1.0);
        assert_eq!(params.speed(), 1.0);
        assert!(!params.looping);
    }

    #[test]
    fn a_centred_sound_is_equal_in_both_ears() {
        let (left, right) = params(1.0, 0.0).gains();
        assert!((left - right).abs() < 1e-6);
        assert!((left - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    }

    #[test]
    fn hard_left_reaches_only_the_left() {
        let (left, right) = params(1.0, -1.0).gains();
        assert!((left - 1.0).abs() < 1e-6);
        assert!(right.abs() < 1e-6);
    }

    #[test]
    fn hard_right_reaches_only_the_right() {
        let (left, right) = params(1.0, 1.0).gains();
        assert!(left.abs() < 1e-6);
        assert!((right - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_power_is_the_same_wherever_the_sound_sits() {
        // The property the law is chosen for: a sound panned across the front
        // must not get louder in the middle.
        for step in 0..=20 {
            let pan = -1.0 + step as f32 / 10.0;
            let (left, right) = params(1.0, pan).gains();
            let power = left * left + right * right;
            assert!((power - 1.0).abs() < 1e-5, "at pan {pan} power was {power}");
        }
    }

    #[test]
    fn a_pan_beyond_the_ends_is_clamped() {
        assert_eq!(params(1.0, -4.0).gains(), params(1.0, -1.0).gains());
        assert_eq!(params(1.0, 4.0).gains(), params(1.0, 1.0).gains());
    }

    #[test]
    fn a_negative_volume_is_silence_rather_than_an_inverted_phase() {
        let (left, right) = params(-1.0, 0.0).gains();
        assert_eq!((left, right), (0.0, 0.0));
    }

    #[test]
    fn a_nan_never_reaches_the_mix() {
        // One NaN sample added to the output buffer stays NaN for every voice
        // after it, for the rest of the callback.
        let (left, right) = params(f32::NAN, f32::NAN).gains();
        assert!(left.is_finite() && right.is_finite());
        assert_eq!((left, right), (0.0, 0.0));

        let held = PlayParams {
            pitch: f32::NAN,
            ..Default::default()
        };
        assert_eq!(held.speed(), 1.0);
    }

    #[test]
    fn a_negative_pitch_holds_rather_than_playing_backwards() {
        let backwards = PlayParams {
            pitch: -2.0,
            ..Default::default()
        };
        assert_eq!(backwards.speed(), 0.0);
    }
}
