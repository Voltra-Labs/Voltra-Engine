//! What an effect is, as a project file and a tick say it: a kind and its
//! numbers, with no buffers and no device rate.

/// One effect on a bus, as written down.
///
/// The description and not the running effect: it is what a bus layout file
/// holds, what [`Audio`](crate::Audio) mirrors on the main thread, and what
/// the main thread builds an [`Effect`](super::Effect) from — allocating its
/// delay lines there, at the device's rate, before it is sent to the mixer.
///
/// Every time is in seconds and every level is linear, like every other
/// volume in the engine.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EffectDesc {
    /// A second-order (12 dB per octave) low-pass: what is above `cutoff`
    /// is taken away. Music heard through a wall, under water, from the
    /// next room.
    LowPass {
        /// Where the slope starts, in hertz.
        cutoff: f32,
        /// The resonance at the cutoff; `0.707` is flat (Butterworth).
        #[serde(default = "butterworth")]
        q: f32,
    },
    /// A second-order high-pass: what is below `cutoff` is taken away. A
    /// radio, a telephone, a small speaker.
    HighPass {
        /// Where the slope starts, in hertz.
        cutoff: f32,
        /// The resonance at the cutoff; `0.707` is flat (Butterworth).
        #[serde(default = "butterworth")]
        q: f32,
    },
    /// An echo `time` seconds later, fed back into itself.
    Delay {
        /// Seconds between the sound and its echo, up to
        /// [`MAX_DELAY`](super::MAX_DELAY).
        time: f32,
        /// How much of each echo is echoed again: `0.0` is one echo, near
        /// `1.0` many. Capped below one so the echoes always die away.
        #[serde(default)]
        feedback: f32,
        /// How much of the output is the echo: `0.0` dry, `1.0` only echo.
        #[serde(default = "half")]
        mix: f32,
    },
    /// A room: Jezar's Freeverb, eight combs and four all-passes a side.
    Reverb {
        /// How long the room rings, `0.0` to `1.0`.
        #[serde(default = "half")]
        room_size: f32,
        /// How fast the highs die in the tail, `0.0` to `1.0`.
        #[serde(default = "half")]
        damping: f32,
        /// How far apart the two sides of the tail are, `0.0` to `1.0`.
        #[serde(default = "one")]
        width: f32,
        /// How much of the output is the room: `0.0` dry, `1.0` only room.
        #[serde(default = "third")]
        mix: f32,
    },
    /// The bus pulled down while another one is sounding — music under
    /// dialogue — and let back up once it stops.
    Duck {
        /// The bus whose sound pulls this one down, by name.
        by: String,
        /// How far down, as the part of the level taken away: `0.75` leaves
        /// a quarter.
        #[serde(default = "three_quarters")]
        depth: f32,
        /// The level on `by` that counts as sounding: `0.01` is −40 dB.
        #[serde(default = "quiet")]
        threshold: f32,
        /// Seconds to go all the way down once `by` sounds.
        #[serde(default = "attack")]
        attack: f32,
        /// Seconds to come all the way back once it stops.
        #[serde(default = "release")]
        release: f32,
    },
}

fn butterworth() -> f32 {
    std::f32::consts::FRAC_1_SQRT_2
}
fn half() -> f32 {
    0.5
}
fn one() -> f32 {
    1.0
}
fn third() -> f32 {
    1.0 / 3.0
}
fn three_quarters() -> f32 {
    0.75
}
fn quiet() -> f32 {
    0.01
}
fn attack() -> f32 {
    0.05
}
fn release() -> f32 {
    0.4
}

/// A number on an effect that a tick can move, as
/// [`Audio::set_bus_param`](crate::Audio::set_bus_param) names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Param {
    /// A filter's cutoff, in hertz.
    Cutoff,
    /// A filter's resonance.
    Q,
    /// A delay's time, in seconds.
    Time,
    /// A delay's feedback.
    Feedback,
    /// The wet part of a delay's or a reverb's output.
    Mix,
    /// A reverb's room size.
    RoomSize,
    /// A reverb's damping.
    Damping,
    /// A reverb's stereo width.
    Width,
    /// How far a duck pulls its bus down.
    Depth,
    /// The level that counts as sounding for a duck.
    Threshold,
    /// A duck's attack, in seconds.
    Attack,
    /// A duck's release, in seconds.
    Release,
}

impl EffectDesc {
    /// The effect's name in a file and a log.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::LowPass { .. } => "LowPass",
            Self::HighPass { .. } => "HighPass",
            Self::Delay { .. } => "Delay",
            Self::Reverb { .. } => "Reverb",
            Self::Duck { .. } => "Duck",
        }
    }

    /// The field `param` names on this kind of effect, or `None` for a
    /// parameter it does not have.
    pub fn param_mut(&mut self, param: Param) -> Option<&mut f32> {
        match (self, param) {
            (Self::LowPass { cutoff, .. } | Self::HighPass { cutoff, .. }, Param::Cutoff) => {
                Some(cutoff)
            }
            (Self::LowPass { q, .. } | Self::HighPass { q, .. }, Param::Q) => Some(q),
            (Self::Delay { time, .. }, Param::Time) => Some(time),
            (Self::Delay { feedback, .. }, Param::Feedback) => Some(feedback),
            (Self::Delay { mix, .. } | Self::Reverb { mix, .. }, Param::Mix) => Some(mix),
            (Self::Reverb { room_size, .. }, Param::RoomSize) => Some(room_size),
            (Self::Reverb { damping, .. }, Param::Damping) => Some(damping),
            (Self::Reverb { width, .. }, Param::Width) => Some(width),
            (Self::Duck { depth, .. }, Param::Depth) => Some(depth),
            (Self::Duck { threshold, .. }, Param::Threshold) => Some(threshold),
            (Self::Duck { attack, .. }, Param::Attack) => Some(attack),
            (Self::Duck { release, .. }, Param::Release) => Some(release),
            _ => None,
        }
    }

    /// Whether every number is one the effect can run with, and why not.
    ///
    /// Only what cannot mean anything is refused — a `NaN`, a negative
    /// time, a duck with no bus. A number merely past a sensible range is
    /// clamped where it is used, so a file written with `feedback: 1.2`
    /// still plays, at the most feedback that dies away.
    pub fn check(&self) -> Result<(), String> {
        let mut copy = self.clone();
        let params = [
            Param::Cutoff,
            Param::Q,
            Param::Time,
            Param::Feedback,
            Param::Mix,
            Param::RoomSize,
            Param::Damping,
            Param::Width,
            Param::Depth,
            Param::Threshold,
            Param::Attack,
            Param::Release,
        ];
        for param in params {
            if let Some(value) = copy.param_mut(param) {
                if !value.is_finite() || *value < 0.0 {
                    return Err(format!(
                        "{}'s {param:?} is {value}; it must be a number, zero or more",
                        self.kind()
                    ));
                }
            }
        }
        if let Self::Duck { by, .. } = self {
            if by.trim().is_empty() {
                return Err("a Duck names no bus to be ducked by".to_owned());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_parameter_is_found_only_on_the_effects_that_have_it() {
        let mut low = EffectDesc::LowPass {
            cutoff: 500.0,
            q: 0.7,
        };
        assert_eq!(low.param_mut(Param::Cutoff).copied(), Some(500.0));
        assert_eq!(low.param_mut(Param::Time), None);
    }

    #[test]
    fn a_number_that_means_nothing_is_refused() {
        let nan = EffectDesc::Delay {
            time: f32::NAN,
            feedback: 0.0,
            mix: 0.5,
        };
        assert!(nan.check().expect_err("a NaN").contains("Time"));
        let nobody = EffectDesc::Duck {
            by: " ".to_owned(),
            depth: 0.5,
            threshold: 0.01,
            attack: 0.1,
            release: 0.1,
        };
        assert!(nobody.check().is_err());
    }

    #[test]
    fn a_number_merely_large_is_allowed() {
        let loud = EffectDesc::Delay {
            time: 0.2,
            feedback: 1.2,
            mix: 0.5,
        };
        assert!(loud.check().is_ok());
    }
}
