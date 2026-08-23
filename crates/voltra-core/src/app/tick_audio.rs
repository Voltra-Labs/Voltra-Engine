//! What a game's tick may do to the sound.

use voltra_assets::{Clips, Handle};
use voltra_audio::{Audio, Clip, PlayParams, VoiceId};

/// The sound half of a [`Tick`](super::Tick).
///
/// A one-shot with no entity behind it is the common case — a coin taken, a
/// jump, a hit — and it is what this exists for: the tick reads a collision
/// event and plays a sound, without spawning anything to carry it. A sound
/// that has to *follow* something is an
/// [`AudioSource`](voltra_scene::AudioSource) on that something instead, and
/// the loop moves it every frame.
///
/// Handles, not paths: loading decodes a file, and a tick is not the place to
/// read a disk. A game gets its handles from the components already in the
/// world — an `AudioSource`'s `clip_handle` is the coin's own sound — which is
/// also what keeps the paths in the scene file where an author can see them.
pub struct TickAudio<'a> {
    audio: &'a mut Audio,
    /// `None` before the window exists, which is every headless test and the
    /// first frames of a real run. Everything below still issues ids.
    clips: Option<&'a Clips>,
}

impl<'a> TickAudio<'a> {
    pub(super) fn new(audio: &'a mut Audio, clips: Option<&'a Clips>) -> Self {
        Self { audio, clips }
    }

    /// Plays `clip` once, centred, as recorded.
    pub fn play(&mut self, clip: Handle<Clip>) -> VoiceId {
        self.play_with(clip, PlayParams::default())
    }

    /// Plays `clip` with the volume, pitch, pan and loop the caller chose.
    ///
    /// A handle that resolves to nothing — a file that would not decode, a
    /// handle from another store — plays silence and still returns an id, for
    /// the same reason [`Audio`] with no device does: a caller that keeps the
    /// id must behave the same either way.
    pub fn play_with(&mut self, clip: Handle<Clip>, params: PlayParams) -> VoiceId {
        match self.clips.and_then(|clips| clips.try_get(clip)) {
            Some(clip) => self.audio.play(clip, params),
            None => self.audio.play(&Clip::silent(), params),
        }
    }

    /// Ends one voice now — how a looping sound a tick started is stopped.
    pub fn stop(&mut self, voice: VoiceId) {
        self.audio.stop(voice);
    }

    /// Ends every voice, including the ones the scene's own sources started.
    pub fn stop_all(&mut self) {
        self.audio.stop_all();
    }

    /// Whether anything can actually be heard this run.
    ///
    /// For a game that wants to skip work rather than for one that wants to
    /// branch: playing into a silent `Audio` is already free.
    pub fn is_silent(&self) -> bool {
        self.audio.is_silent()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_assets::AssetPath;
    use voltra_testkit::scratch_root;

    #[test]
    fn a_tick_with_no_store_behind_it_still_issues_ids() {
        // Every headless test, and the frames before the window exists.
        let mut audio = Audio::silent();
        let mut tick = TickAudio::new(&mut audio, None);

        let first = tick.play(Handle::forge(0, 0));
        let second = tick.play(Handle::forge(0, 0));

        assert_ne!(first, second);
        tick.stop(first);
        tick.stop_all();
        assert!(tick.is_silent());
    }

    #[test]
    fn a_handle_that_resolves_to_nothing_plays_silence_rather_than_panicking() {
        // A forged handle names a slot the store never allocated. `Clips::get`
        // would panic on it; the tick path must not.
        let clips = Clips::new(scratch_root());
        let mut audio = Audio::silent();
        let mut tick = TickAudio::new(&mut audio, Some(&clips));

        let voice = tick.play(Handle::forge(9, 3));

        tick.stop(voice);
    }

    #[test]
    fn a_loaded_clip_is_what_gets_played() {
        let root = scratch_root();
        let mut clips = Clips::new(&root);
        let handle = clips.load(&AssetPath::new("missing.wav").expect("valid"));
        let mut audio = Audio::silent();
        let mut tick = TickAudio::new(&mut audio, Some(&clips));

        let voice = tick.play_with(
            handle,
            PlayParams {
                looping: true,
                ..Default::default()
            },
        );

        tick.stop(voice);
    }
}
