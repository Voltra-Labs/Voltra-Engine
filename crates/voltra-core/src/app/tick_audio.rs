//! What a game's tick may do to the sound.

use std::time::Duration;

use voltra_assets::{Clips, Handle};
use voltra_audio::{Audio, BusId, Clip, EffectDesc, Param, PlayParams, VoiceId};

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
        self.play_fading_in(clip, params, Duration::ZERO)
    }

    /// As [`play_with`](Self::play_with), rising from silence over `fade_in`
    /// — the incoming half of a music cross-fade.
    pub fn play_fading_in(
        &mut self,
        clip: Handle<Clip>,
        params: PlayParams,
        fade_in: Duration,
    ) -> VoiceId {
        match self.clips.and_then(|clips| clips.try_get(clip)) {
            Some(clip) => self.audio.play_fading_in(clip, params, fade_in),
            None => self.audio.play_fading_in(&Clip::silent(), params, fade_in),
        }
    }

    /// Makes `clip` the music, cross-fading from the track already playing
    /// over `fade` — equal-power curves, so the mix does not dip at the
    /// middle — and returns its voice.
    ///
    /// Looping, on the `music` bus. Asking for the track already playing
    /// leaves it playing, so a tick may name its area's music every frame.
    /// A long track wants a `*.clip.ron` with `stream: true`, so its samples
    /// are decoded while it plays rather than held resident; the settings
    /// file is also where its loop points go.
    pub fn play_music(&mut self, clip: Handle<Clip>, fade: Duration) -> VoiceId {
        let params = PlayParams {
            looping: true,
            bus: self.audio.bus("music"),
            ..Default::default()
        };
        self.play_music_with(clip, params, fade)
    }

    /// [`play_music`](Self::play_music) with the volume, pitch, loop and bus
    /// the caller chose.
    pub fn play_music_with(
        &mut self,
        clip: Handle<Clip>,
        params: PlayParams,
        fade: Duration,
    ) -> VoiceId {
        match self.clips.and_then(|clips| clips.try_get(clip)) {
            Some(clip) => self.audio.play_music(clip, params, fade),
            None => self.audio.play_music(&Clip::silent(), params, fade),
        }
    }

    /// Fades the music out over `fade` and forgets it.
    pub fn stop_music(&mut self, fade: Duration) {
        self.audio.stop_music(fade);
    }

    /// The voice the music is playing on, if any.
    pub fn music(&self) -> Option<VoiceId> {
        self.audio.music()
    }

    /// Ends one voice now — how a looping sound a tick started is stopped.
    pub fn stop(&mut self, voice: VoiceId) {
        self.audio.stop(voice);
    }

    /// Fades a voice to silence over `over` and ends it: the outgoing half of
    /// a cross-fade.
    pub fn fade_out(&mut self, voice: VoiceId, over: Duration) {
        self.audio.fade_out(voice, over);
    }

    /// Slides a voice's fader to `volume` over `over` without ending it.
    pub fn fade(&mut self, voice: VoiceId, volume: f32, over: Duration) {
        self.audio.fade(voice, volume, over);
    }

    /// The bus called `name`, for a [`PlayParams::bus`] or a volume change.
    pub fn bus(&mut self, name: &str) -> BusId {
        self.audio.bus(name)
    }

    /// Slides a whole bus to `volume` over `over` — the music down under a
    /// dialogue line, everything down under a pause menu.
    pub fn set_bus_volume(&mut self, bus: BusId, volume: f32, over: Duration) {
        self.audio.set_bus_volume(bus, volume, over);
    }

    /// The volume `bus` was last set to.
    pub fn bus_volume(&self, bus: BusId) -> f32 {
        self.audio.bus_volume(bus)
    }

    /// Slides `param` of the effect at `effect` on `bus` to `value` over
    /// `over` — a low-pass opening as the player surfaces, a reverb growing
    /// as they walk into a cave. The bus's effects come from the project's
    /// `default.buses.ron` or from [`set_bus_effects`](Self::set_bus_effects);
    /// `effect` counts from zero along that chain.
    ///
    /// `false`, changing nothing, for an effect that is not there or a
    /// parameter it does not have.
    pub fn set_bus_param(
        &mut self,
        bus: BusId,
        effect: usize,
        param: Param,
        value: f32,
        over: Duration,
    ) -> bool {
        self.audio.set_bus_param(bus, effect, param, value, over)
    }

    /// Replaces `bus`'s effects, first to last — a level that sounds under
    /// water, a radio that crackles in. Built now, so not every frame.
    pub fn set_bus_effects(&mut self, bus: BusId, effects: Vec<EffectDesc>) {
        self.audio.set_bus_effects(bus, effects);
    }

    /// The effects on `bus`, with every number a tick moved since.
    pub fn bus_effects(&self, bus: BusId) -> &[EffectDesc] {
        self.audio.bus_effects(bus)
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

    #[test]
    fn a_tick_can_duck_a_bus_and_read_it_back() {
        let mut audio = Audio::silent();
        let mut tick = TickAudio::new(&mut audio, None);
        let music = tick.bus("music");

        tick.set_bus_volume(music, 0.3, Duration::from_millis(250));

        assert_eq!(tick.bus_volume(music), 0.3, "the target, not the ramp");
    }

    #[test]
    fn a_cross_fade_is_two_calls() {
        let mut audio = Audio::silent();
        let mut tick = TickAudio::new(&mut audio, None);
        let over = Duration::from_secs(1);

        let old = tick.play(Handle::forge(0, 0));
        tick.fade_out(old, over);
        let new = tick.play_fading_in(Handle::forge(0, 0), PlayParams::default(), over);

        assert_ne!(old, new);
    }

    #[test]
    fn music_named_every_frame_starts_once_and_another_track_replaces_it() {
        let root = scratch_root();
        let mut clips = Clips::new(&root);
        let calm = clips.load(&AssetPath::new("calm.wav").expect("valid"));
        let storm = clips.load(&AssetPath::new("storm.wav").expect("valid"));
        let mut audio = Audio::silent();
        let mut tick = TickAudio::new(&mut audio, Some(&clips));
        let fade = Duration::from_secs(2);

        let first = tick.play_music(calm, fade);
        let again = tick.play_music(calm, fade);
        let next = tick.play_music(storm, fade);

        assert_eq!(first, again, "the same track keeps playing");
        assert_ne!(next, first, "a new one cross-fades in");
        assert_eq!(tick.music(), Some(next));
        tick.stop_music(fade);
        assert_eq!(tick.music(), None);
    }
}
