//! What the engine holds to make a sound.

use std::fmt;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use crate::bus::{BusId, Buses, MAX_BUSES};
use crate::clip::Clip;
use crate::command::Command;
use crate::device::Output;
use crate::params::PlayParams;
use crate::voice::{VoiceId, SMOOTHING};

/// The game's end of the audio thread.
///
/// One per app. Every method is a message posted to the mixer and returns
/// immediately; nothing here blocks, allocates a buffer, or touches a sample.
///
/// **A silent `Audio` is a working `Audio`.** When there is no output device —
/// a CI runner, a machine with the sound card disabled, a laptop with
/// everything unplugged — [`open`](Self::open) logs once and hands back one of
/// these that accepts every call and issues real [`VoiceId`]s. That is the
/// same rule the rest of the engine already follows: a missing texture draws a
/// checker, a missing camera keeps the default framing, and neither stops the
/// app. It is also what makes the layer above testable, because
/// [`Audio::default`] is the silent one.
pub struct Audio {
    /// `None` once there is no thread to talk to: no device at startup, or a
    /// stream that has gone away since.
    commands: Option<Sender<Command>>,
    /// Clips coming back from finished voices, to be dropped on this thread.
    retired: Option<Receiver<Clip>>,
    /// The live stream. Never read — dropping it is what stops the sound.
    output: Option<Output>,
    /// The next id to hand out. See [`VoiceId`] on why it never wraps.
    next: u64,
    /// The bus names, which only this thread ever reads.
    buses: Buses,
    /// The volume each bus was last told to head for, by [`BusId::index`].
    ///
    /// The main thread's copy of what the mixer holds, kept for two readers:
    /// a settings screen showing its sliders, and [`connect`](Self::connect)
    /// handing a new device the mix the game already set on a silent one.
    bus_volumes: [f32; MAX_BUSES],
}

impl Audio {
    /// Opens the default output device, or falls back to silence.
    ///
    /// Never fails, for the reason on the type. The failure is logged once,
    /// here, rather than at every call that would have made a sound.
    pub fn open() -> Self {
        let mut audio = Self::silent();
        audio.connect();
        audio
    }

    /// An `Audio` with no device behind it, which accepts everything and plays
    /// nothing.
    pub fn silent() -> Self {
        Self {
            commands: None,
            retired: None,
            output: None,
            next: 1,
            buses: Buses::default(),
            bus_volumes: [1.0; MAX_BUSES],
        }
    }

    /// The same `Audio` with `buses` as its table instead of
    /// [`Buses::default`], every one at unity.
    pub fn with_buses(mut self, buses: Buses) -> Self {
        self.buses = buses;
        self.bus_volumes = [1.0; MAX_BUSES];
        for index in 0..MAX_BUSES {
            self.send(Command::SetBusVolume {
                bus: BusId::from_index(index),
                volume: 1.0,
                over: Duration::ZERO,
            });
        }
        self
    }

    /// Opens the default output device under this `Audio`, keeping its buses,
    /// their volumes and its ids.
    ///
    /// What lets a game configure the mix before there is a window: the app
    /// holds a silent `Audio` until the device can be opened, and the volumes
    /// set on it are handed to the new mixer here rather than lost. A no-op
    /// if a device is already open; logs once and stays silent if none can be.
    pub fn connect(&mut self) {
        if self.commands.is_some() {
            return;
        }

        let (sender, receiver) = channel();
        let (retire_sender, retire_receiver) = channel();

        match Output::open(receiver, retire_sender) {
            Ok(output) => {
                self.commands = Some(sender);
                self.retired = Some(retire_receiver);
                self.output = Some(output);
                self.replay_bus_volumes();
            }
            Err(e) => log::warn!("{e}; the game will run without sound"),
        }
    }

    /// Sends the mixer every bus volume that is not unity, which is all a new
    /// mixer needs told.
    fn replay_bus_volumes(&mut self) {
        let volumes = self.bus_volumes;
        for (index, volume) in volumes.into_iter().enumerate() {
            if volume != 1.0 {
                self.send(Command::SetBusVolume {
                    bus: BusId::from_index(index),
                    volume,
                    over: Duration::ZERO,
                });
            }
        }
    }

    /// Starts output the platform held back until the user interacted — a
    /// browser does, until the first key, click or touch. A no-op without a
    /// device, and natively, where the stream is already playing.
    pub fn resume(&self) {
        if let Some(output) = &self.output {
            output.resume();
        }
    }

    /// Whether anything can actually be heard.
    pub fn is_silent(&self) -> bool {
        self.commands.is_none()
    }

    /// Samples per second per channel the device runs at, if there is one.
    pub fn rate(&self) -> Option<u32> {
        self.output.as_ref().map(Output::rate)
    }

    /// Samples per frame the device expects, if there is one.
    pub fn channels(&self) -> Option<u16> {
        self.output.as_ref().map(Output::channels)
    }

    /// Starts `clip` and returns the voice it is playing on.
    ///
    /// The clip is shared, not copied: see [`Clip`]. The id is issued whether
    /// or not there is a device, so a caller that keeps one — a looping
    /// source, a sound that follows an entity — behaves the same either way.
    pub fn play(&mut self, clip: &Clip, params: PlayParams) -> VoiceId {
        self.play_fading_in(clip, params, Duration::ZERO)
    }

    /// As [`play`](Self::play), rising from silence over `fade_in`: the
    /// incoming half of a cross-fade.
    pub fn play_fading_in(
        &mut self,
        clip: &Clip,
        params: PlayParams,
        fade_in: Duration,
    ) -> VoiceId {
        let id = VoiceId::new(self.next);
        self.next += 1;

        self.send(Command::Play {
            id,
            clip: clip.clone(),
            params,
            fade_in,
        });
        id
    }

    /// Moves a voice that is already playing.
    ///
    /// Volume and pan only, and silently ignored for a voice that has already
    /// finished — see [`Command::SetGain`].
    pub fn set_gain(&mut self, id: VoiceId, volume: f32, pan: f32) {
        self.send(Command::SetGain { id, volume, pan });
    }

    /// Slides a voice's fader to `volume` over `over`, on top of its own
    /// volume. `0.0` silences it without ending it.
    pub fn fade(&mut self, id: VoiceId, volume: f32, over: Duration) {
        self.send(Command::Fade { id, volume, over });
    }

    /// Fades a voice to silence over `over` and ends it there: the outgoing
    /// half of a cross-fade.
    pub fn fade_out(&mut self, id: VoiceId, over: Duration) {
        self.send(Command::Stop { id, over });
    }

    /// Ends one voice now — over [`SMOOTHING`] rather than in one sample,
    /// because a waveform cut off mid-cycle clicks.
    pub fn stop(&mut self, id: VoiceId) {
        self.fade_out(id, SMOOTHING);
    }

    /// Ends every voice, de-clicked the same way: a scene change, or an
    /// editor's Stop.
    pub fn stop_all(&mut self) {
        self.send(Command::StopAll { over: SMOOTHING });
    }

    /// The bus called `name`, registering it at unity if nothing has named it
    /// yet — see [`Buses::id`] on why registering.
    pub fn bus(&mut self, name: &str) -> BusId {
        self.buses.id(name)
    }

    /// The bus table, in id order.
    pub fn buses(&self) -> &Buses {
        &self.buses
    }

    /// The bus table, for a caller resolving several names at once.
    pub fn buses_mut(&mut self) -> &mut Buses {
        &mut self.buses
    }

    /// Slides `bus` to `volume` over `over`.
    ///
    /// Linear gain, like every volume here. [`BusId::MASTER`] is the master
    /// volume: everything, after every other bus.
    pub fn set_bus_volume(&mut self, bus: BusId, volume: f32, over: Duration) {
        let Some(slot) = self.bus_volumes.get_mut(bus.index()) else {
            return;
        };
        // Guarded the way the mixer's own ramp guards it, so the mirror never
        // reports a volume the speakers are not getting.
        *slot = if volume.is_finite() {
            volume.max(0.0)
        } else {
            0.0
        };
        self.send(Command::SetBusVolume { bus, volume, over });
    }

    /// The volume `bus` was last set to — its target, not where a fade in
    /// progress has got to.
    pub fn bus_volume(&self, bus: BusId) -> f32 {
        self.bus_volumes.get(bus.index()).copied().unwrap_or(1.0)
    }

    /// Frees the clips finished voices handed back.
    ///
    /// Called once a frame by the loop, next to the other end-of-frame work.
    /// Doing it here rather than in the callback is the whole point of the
    /// retirement channel: this is the thread that can afford to free a
    /// megabyte of samples.
    pub fn end_frame(&mut self) {
        let Some(retired) = self.retired.as_ref() else {
            return;
        };
        // Each `recv` moves a clip into this scope, where it is dropped.
        for clip in retired.try_iter() {
            drop(clip);
        }
    }

    /// Posts a message, going silent if the audio thread has gone away.
    ///
    /// A closed channel means the stream was torn down — an unplugged device
    /// on some hosts. Dropping the sender turns this into the silent `Audio`
    /// the type documents, so the next thousand sounds cost nothing and log
    /// nothing rather than failing one at a time.
    fn send(&mut self, command: Command) {
        let Some(sender) = self.commands.as_ref() else {
            return;
        };
        if sender.send(command).is_err() {
            log::warn!("the audio device stopped; the game will run without sound");
            self.commands = None;
            self.retired = None;
            self.output = None;
        }
    }
}

/// Silent, so `App::default()` and every test that builds one need no device.
impl Default for Audio {
    fn default() -> Self {
        Self::silent()
    }
}

impl fmt::Debug for Audio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Audio")
            .field("silent", &self.is_silent())
            .field("rate", &self.rate())
            .field("channels", &self.channels())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_audio_accepts_everything() {
        // The CI runner's case, and `App::default`'s. None of this may panic.
        let mut audio = Audio::silent();
        assert!(audio.is_silent());
        assert_eq!(audio.rate(), None);
        assert_eq!(audio.channels(), None);

        let voice = audio.play(&Clip::silent(), PlayParams::default());
        audio.set_gain(voice, 0.5, -1.0);
        audio.stop(voice);
        audio.stop_all();
        audio.end_frame();
    }

    #[test]
    fn every_voice_gets_its_own_id() {
        // Ids are how a caller reaches one sound of many, so two plays must
        // never come back the same — device or no device.
        let mut audio = Audio::silent();
        let clip = Clip::silent();

        let first = audio.play(&clip, PlayParams::default());
        let second = audio.play(&clip, PlayParams::default());

        assert_ne!(first, second);
    }

    #[test]
    fn a_silent_audio_still_issues_ids_in_order() {
        // A looping source keeps its id and stops it later. That has to work
        // the same on a machine with no sound card, or the code path a
        // developer tests is not the one that ships.
        let mut audio = Audio::silent();
        let clip = Clip::silent();

        let ids: Vec<_> = (0..3)
            .map(|_| audio.play(&clip, PlayParams::default()))
            .collect();

        assert!(ids[0] < ids[1] && ids[1] < ids[2]);
    }

    #[test]
    fn a_dead_audio_thread_turns_the_whole_thing_silent() {
        // An unplugged device. The next sound must cost nothing rather than
        // failing one send at a time.
        let (sender, receiver) = channel();
        let mut audio = Audio {
            commands: Some(sender),
            ..Audio::silent()
        };
        drop(receiver);

        audio.play(&Clip::silent(), PlayParams::default());

        assert!(audio.is_silent());
        audio.play(&Clip::silent(), PlayParams::default());
    }

    #[test]
    fn what_is_played_reaches_the_mixer_s_end_of_the_channel() {
        let (sender, receiver) = channel();
        let mut audio = Audio {
            commands: Some(sender),
            ..Audio::silent()
        };

        let voice = audio.play(&Clip::new(vec![1.0, 1.0], 1, 8), PlayParams::default());
        audio.stop(voice);

        let played = receiver.try_recv().expect("the play should be queued");
        match played {
            Command::Play { id, clip, .. } => {
                assert_eq!(id, voice);
                assert_eq!(clip.frames(), 2);
            }
            other => panic!("expected a Play, got {other:?}"),
        }
        assert!(matches!(receiver.try_recv(), Ok(Command::Stop { id, .. }) if id == voice));
    }

    #[test]
    fn end_of_frame_drops_the_clips_the_mixer_handed_back() {
        let (sender, receiver) = channel();
        let mut audio = Audio {
            retired: Some(receiver),
            ..Audio::silent()
        };
        sender
            .send(Clip::new(vec![0.0; 64], 1, 8))
            .expect("the queue takes it");

        audio.end_frame();

        assert!(
            sender.send(Clip::silent()).is_ok(),
            "the receiver should still be there for the next frame"
        );
    }

    #[test]
    fn a_bus_volume_set_while_silent_is_remembered() {
        // A settings screen read before the window opened, or a game that
        // set its music level in `main`.
        let mut audio = Audio::silent();
        let music = audio.bus("music");

        audio.set_bus_volume(music, 0.25, Duration::ZERO);

        assert_eq!(audio.bus_volume(music), 0.25);
        assert_eq!(audio.bus_volume(BusId::MASTER), 1.0);
    }

    #[test]
    fn a_bus_volume_the_mixer_would_refuse_is_not_reported_either() {
        let mut audio = Audio::silent();
        audio.set_bus_volume(BusId::MASTER, f32::NAN, Duration::ZERO);
        assert_eq!(audio.bus_volume(BusId::MASTER), 0.0);
        audio.set_bus_volume(BusId::MASTER, -3.0, Duration::ZERO);
        assert_eq!(audio.bus_volume(BusId::MASTER), 0.0);
    }

    #[test]
    fn the_volumes_a_silent_audio_holds_are_replayed_to_a_new_mixer() {
        // `connect` opening a device under an `Audio` the game already
        // configured: only what differs from unity needs sending.
        let (sender, receiver) = channel();
        let mut audio = Audio::silent();
        let sfx = audio.bus("sfx");
        audio.set_bus_volume(sfx, 0.5, Duration::ZERO);

        audio.commands = Some(sender);
        audio.replay_bus_volumes();

        let replayed: Vec<Command> = receiver.try_iter().collect();
        assert_eq!(replayed.len(), 1, "got {replayed:?}");
        assert!(matches!(
            replayed[0],
            Command::SetBusVolume { bus, volume, .. } if bus == sfx && volume == 0.5
        ));
    }

    #[test]
    fn a_stop_is_de_clicked_rather_than_cut() {
        let (sender, receiver) = channel();
        let mut audio = Audio {
            commands: Some(sender),
            ..Audio::silent()
        };

        audio.stop(VoiceId::forge(7));
        audio.stop_all();

        assert!(matches!(receiver.try_recv(), Ok(Command::Stop { over, .. }) if over == SMOOTHING));
        assert!(matches!(receiver.try_recv(), Ok(Command::StopAll { over }) if over == SMOOTHING));
    }

    #[test]
    fn a_game_s_own_table_replaces_the_default_one() {
        let audio = Audio::silent().with_buses(Buses::new(["dialogue"]));
        assert_eq!(audio.buses().find("dialogue"), Some(BusId::from_index(1)));
        assert_eq!(audio.buses().find("music"), None);
    }
}
