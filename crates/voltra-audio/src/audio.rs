//! What the engine holds to make a sound.

use std::fmt;
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::clip::Clip;
use crate::command::Command;
use crate::device::Output;
use crate::params::PlayParams;
use crate::voice::VoiceId;

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
}

impl Audio {
    /// Opens the default output device, or falls back to silence.
    ///
    /// Never fails, for the reason on the type. The failure is logged once,
    /// here, rather than at every call that would have made a sound.
    pub fn open() -> Self {
        let (sender, receiver) = channel();
        let (retire_sender, retire_receiver) = channel();

        match Output::open(receiver, retire_sender) {
            Ok(output) => Self {
                commands: Some(sender),
                retired: Some(retire_receiver),
                output: Some(output),
                next: 1,
            },
            Err(e) => {
                log::warn!("{e}; the game will run without sound");
                Self::silent()
            }
        }
    }

    /// An `Audio` with no device behind it, which accepts everything and plays
    /// nothing.
    pub fn silent() -> Self {
        Self {
            commands: None,
            retired: None,
            output: None,
            next: 1,
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
        let id = VoiceId::new(self.next);
        self.next += 1;

        self.send(Command::Play {
            id,
            clip: clip.clone(),
            params,
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

    /// Ends one voice now.
    pub fn stop(&mut self, id: VoiceId) {
        self.send(Command::Stop(id));
    }

    /// Ends every voice: a scene change, or an editor's Stop.
    pub fn stop_all(&mut self) {
        self.send(Command::StopAll);
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
            retired: None,
            output: None,
            next: 1,
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
            retired: None,
            output: None,
            next: 1,
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
        assert!(matches!(receiver.try_recv(), Ok(Command::Stop(id)) if id == voice));
    }

    #[test]
    fn end_of_frame_drops_the_clips_the_mixer_handed_back() {
        let (sender, receiver) = channel();
        let mut audio = Audio {
            commands: None,
            retired: Some(receiver),
            output: None,
            next: 1,
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
}
