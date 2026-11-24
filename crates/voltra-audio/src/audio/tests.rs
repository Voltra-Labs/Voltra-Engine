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
