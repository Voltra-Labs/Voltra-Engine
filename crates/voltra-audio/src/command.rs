//! What the main thread asks the audio thread to do.

use std::time::Duration;

use crate::bus::BusId;
use crate::clip::Clip;
use crate::effect::{Effect, Param};
use crate::params::PlayParams;
use crate::ramp::Curve;
use crate::stream::Feed;
use crate::voice::VoiceId;

/// One message from the game to the mixer.
///
/// Messages rather than a shared, locked mixer. The device callback runs on a
/// thread the operating system will not wait for: if it blocks on a mutex the
/// game holds, the buffer is late and the speakers get a gap. Every engine
/// solves it the same way — Wwise, FMOD, Unreal and Godot all queue commands
/// into the audio thread and let it own its own state.
///
/// A `Clip` travels inside [`Play`](Self::Play), which is a pointer bump on
/// the sending side and nothing at all on the receiving one; so does a
/// streamed clip's [`Feed`], made and primed on the sending side. Both travel
/// back out through the retirement channel when the voice ends, so the last
/// reference is dropped by the thread that can afford to.
///
/// Times travel as [`Duration`]s, not frames: the device's rate is the mixer's
/// to know, and the conversion happens once, on the thread that has it.
#[derive(Debug)]
pub enum Command {
    /// Start `clip` as a new voice under `id`, rising from silence over
    /// `fade_in` — zero starts it at full level.
    Play {
        /// The id the new voice is known by.
        id: VoiceId,
        /// The sound to play.
        clip: Clip,
        /// Where a streamed clip's frames come from; `None` for a resident
        /// one.
        feed: Option<Feed>,
        /// Volume, pitch, pan, looping and the bus it plays on.
        params: PlayParams,
        /// How long the rise from silence takes.
        fade_in: Duration,
        /// The rise's shape: [`Curve::EqualPower`] for the incoming half of a
        /// cross-fade.
        curve: Curve,
    },
    /// Move a voice that is already playing. Ignored if it has finished.
    ///
    /// Volume and pan only — see [`Voice::set_gain`](crate::Voice::set_gain).
    /// Smoothed over [`SMOOTHING`](crate::voice::SMOOTHING), because this is
    /// sent every frame for a moving source and a frame is long enough for a
    /// step to click.
    SetGain {
        /// The voice to move.
        id: VoiceId,
        /// The new volume, as a linear multiplier.
        volume: f32,
        /// The new pan: `-1.0` left, `0.0` centre, `1.0` right.
        pan: f32,
    },
    /// Slide a voice's fader to `volume` over `over`. Ignored if it has
    /// finished.
    ///
    /// A multiplier on top of whatever [`SetGain`](Self::SetGain) says, so a
    /// fade on a moving source is not undone by the next frame's position.
    Fade {
        /// The voice to fade.
        id: VoiceId,
        /// The multiplier to slide to.
        volume: f32,
        /// How long the slide takes.
        over: Duration,
        /// The slide's shape.
        curve: Curve,
    },
    /// End one voice, fading it to silence over `over` first. Ignored if it
    /// has finished.
    Stop {
        /// The voice to end.
        id: VoiceId,
        /// How long the fade to silence takes.
        over: Duration,
        /// The fade's shape: [`Curve::EqualPower`] for the outgoing half of a
        /// cross-fade.
        curve: Curve,
    },
    /// End every voice the same way: what an editor's Stop button and a scene
    /// change both want, and the only way to end voices whose ids the caller
    /// has lost.
    StopAll {
        /// How long the fade to silence takes.
        over: Duration,
    },
    /// Slide one bus's volume to `volume` over `over`.
    SetBusVolume {
        /// The bus to move.
        bus: BusId,
        /// The volume to slide to, as a linear multiplier.
        volume: f32,
        /// How long the slide takes.
        over: Duration,
    },
    /// Replace one bus's effects with `effects`, built on the main thread.
    /// The chain it replaces comes back through the retirement channel.
    SetBusEffects {
        /// The bus to change.
        bus: BusId,
        /// Its new chain, first to last.
        effects: Vec<Effect>,
    },
    /// Move one number on one effect of one bus to `value` over `over`.
    /// Ignored for an effect that is not there or a parameter it lacks.
    SetBusParam {
        /// The bus the effect is on.
        bus: BusId,
        /// Which effect, counting from zero in the chain.
        effect: usize,
        /// Which of its numbers.
        param: Param,
        /// The value to head for.
        value: f32,
        /// How long the slide takes.
        over: Duration,
    },
}
