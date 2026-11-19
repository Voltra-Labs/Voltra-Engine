//! What the main thread asks the audio thread to do.

use std::time::Duration;

use crate::bus::BusId;
use crate::clip::Clip;
use crate::params::PlayParams;
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
/// the sending side and nothing at all on the receiving one. It travels back
/// out through the retirement channel when the voice ends, so the last
/// reference is dropped by the thread that can afford to.
///
/// Times travel as [`Duration`]s, not frames: the device's rate is the mixer's
/// to know, and the conversion happens once, on the thread that has it.
#[derive(Debug)]
pub enum Command {
    /// Start `clip` as a new voice under `id`, rising from silence over
    /// `fade_in` — zero starts it at full level.
    Play {
        id: VoiceId,
        clip: Clip,
        params: PlayParams,
        fade_in: Duration,
    },
    /// Move a voice that is already playing. Ignored if it has finished.
    ///
    /// Volume and pan only — see [`Voice::set_gain`](crate::Voice::set_gain).
    /// Smoothed over [`SMOOTHING`](crate::voice::SMOOTHING), because this is
    /// sent every frame for a moving source and a frame is long enough for a
    /// step to click.
    SetGain { id: VoiceId, volume: f32, pan: f32 },
    /// Slide a voice's fader to `volume` over `over`. Ignored if it has
    /// finished.
    ///
    /// A multiplier on top of whatever [`SetGain`](Self::SetGain) says, so a
    /// fade on a moving source is not undone by the next frame's position.
    Fade {
        id: VoiceId,
        volume: f32,
        over: Duration,
    },
    /// End one voice, fading it to silence over `over` first. Ignored if it
    /// has finished.
    Stop { id: VoiceId, over: Duration },
    /// End every voice the same way: what an editor's Stop button and a scene
    /// change both want, and the only way to end voices whose ids the caller
    /// has lost.
    StopAll { over: Duration },
    /// Slide one bus's volume to `volume` over `over`.
    SetBusVolume {
        bus: BusId,
        volume: f32,
        over: Duration,
    },
}
