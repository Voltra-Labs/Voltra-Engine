//! What the main thread asks the audio thread to do.

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
#[derive(Debug)]
pub enum Command {
    /// Start `clip` as a new voice under `id`.
    Play {
        id: VoiceId,
        clip: Clip,
        params: PlayParams,
    },
    /// Move a voice that is already playing. Ignored if it has finished.
    ///
    /// Volume and pan only — see [`Voice::set_gain`](crate::Voice::set_gain).
    SetGain { id: VoiceId, volume: f32, pan: f32 },
    /// End one voice now. Ignored if it has finished.
    Stop(VoiceId),
    /// End every voice: what an editor's Stop button and a scene change both
    /// want, and the only way to end voices whose ids the caller has lost.
    StopAll,
}
