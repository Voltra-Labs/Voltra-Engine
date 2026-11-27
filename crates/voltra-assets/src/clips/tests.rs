use super::*;
use voltra_testkit::scratch_root;

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a valid asset path")
}

/// A 16-bit mono PCM WAV of `frames` samples at 8 kHz.
///
/// Hand-written for the reason `voltra-audio`'s decoder tests give: a
/// fixture produced by an encoder would test the encoder.
fn wav(frames: u32) -> Vec<u8> {
    let data_len = frames * 2;
    let mut file = Vec::new();
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(&(36 + data_len).to_le_bytes());
    file.extend_from_slice(b"WAVE");
    file.extend_from_slice(b"fmt ");
    file.extend_from_slice(&16u32.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&8_000u32.to_le_bytes());
    file.extend_from_slice(&16_000u32.to_le_bytes());
    file.extend_from_slice(&2u16.to_le_bytes());
    file.extend_from_slice(&16u16.to_le_bytes());
    file.extend_from_slice(b"data");
    file.extend_from_slice(&data_len.to_le_bytes());
    file.extend_from_slice(&vec![0u8; data_len as usize]);
    file
}

#[test]
fn a_file_becomes_a_clip() {
    let root = scratch_root();
    std::fs::write(root.join("coin.wav"), wav(16)).expect("the fixture writes");
    let mut clips = Clips::new(&root);

    let handle = clips.load(&path("coin.wav"));

    let clip = clips.get(handle);
    assert_eq!(clip.frames(), 16);
    assert_eq!(clip.rate(), 8_000);
}

#[test]
fn one_path_is_decoded_once() {
    // The whole point of the store: a scene with fifty coins holds one
    // buffer, not fifty.
    let root = scratch_root();
    std::fs::write(root.join("coin.wav"), wav(16)).expect("the fixture writes");
    let mut clips = Clips::new(&root);

    let first = clips.load(&path("coin.wav"));
    let before = clips.len();
    let second = clips.load(&path("coin.wav"));

    assert_eq!(first, second);
    assert_eq!(clips.len(), before, "the second load stored nothing");
}

#[test]
fn a_file_that_is_not_there_is_silence_rather_than_a_panic() {
    let root = scratch_root();
    let mut clips = Clips::new(&root);

    let handle = clips.load(&path("nowhere.wav"));

    assert!(clips.get(handle).is_empty());
    assert_ne!(handle, clips.silent(), "and it gets its own slot to repair");
}

#[test]
fn a_file_that_will_not_decode_is_the_same_answer() {
    let root = scratch_root();
    std::fs::write(root.join("bad.wav"), "not a wav at all").expect("the fixture writes");
    let mut clips = Clips::new(&root);

    let handle = clips.load(&path("bad.wav"));

    assert!(clips.get(handle).is_empty());
}

#[test]
fn two_broken_paths_do_not_share_a_slot() {
    // Repairing one of them must not mean overwriting the clip the other
    // is also playing.
    let root = scratch_root();
    let mut clips = Clips::new(&root);

    let first = clips.load(&path("one.wav"));
    let second = clips.load(&path("two.wav"));

    assert_ne!(first, second);
}

#[test]
fn a_path_that_was_loaded_can_be_looked_up_again() {
    let root = scratch_root();
    std::fs::write(root.join("coin.wav"), wav(4)).expect("the fixture writes");
    let mut clips = Clips::new(&root);

    let handle = clips.load(&path("coin.wav"));

    assert_eq!(clips.by_path_handle(&path("coin.wav")), Some(handle));
    assert_eq!(clips.by_path_handle(&path("other.wav")), None);
}

#[test]
fn a_reload_swaps_the_samples_under_the_same_handle() {
    let root = scratch_root();
    std::fs::write(root.join("coin.wav"), wav(4)).expect("the fixture writes");
    let mut clips = Clips::new(&root);
    let handle = clips.load(&path("coin.wav"));
    let playing = clips.get(handle).clone();

    std::fs::write(root.join("coin.wav"), wav(32)).expect("the edit writes");
    assert_eq!(clips.reload(&path("coin.wav")), vec![handle]);

    assert_eq!(clips.get(handle).frames(), 32);
    assert_eq!(
        playing.frames(),
        4,
        "a voice already playing keeps its take"
    );
}

#[test]
fn a_broken_file_is_repaired_by_a_reload() {
    // The reason each broken path gets its own slot.
    let root = scratch_root();
    let mut clips = Clips::new(&root);
    let handle = clips.load(&path("late.wav"));
    assert!(clips.get(handle).is_empty());

    std::fs::write(root.join("late.wav"), wav(8)).expect("the file arrives");
    assert_eq!(clips.reload(&path("late.wav")), vec![handle]);

    assert_eq!(clips.get(handle).frames(), 8);
    assert!(
        clips.get(clips.silent()).is_empty(),
        "the shared silence is untouched"
    );
}

#[test]
fn a_reload_that_will_not_decode_keeps_the_old_samples() {
    // An editor mid-save leaves the file truncated.
    let root = scratch_root();
    std::fs::write(root.join("coin.wav"), wav(4)).expect("the fixture writes");
    let mut clips = Clips::new(&root);
    let handle = clips.load(&path("coin.wav"));

    std::fs::write(root.join("coin.wav"), "RIFF").expect("the truncation writes");

    assert!(clips.reload(&path("coin.wav")).is_empty());
    assert_eq!(clips.get(handle).frames(), 4);
}

#[test]
fn reloading_a_path_nobody_loaded_does_nothing() {
    let root = scratch_root();
    std::fs::write(root.join("coin.wav"), wav(4)).expect("the fixture writes");
    let mut clips = Clips::new(&root);

    assert!(clips.reload(&path("coin.wav")).is_empty());
    assert_eq!(clips.len(), 1);
}

#[test]
fn a_new_store_holds_only_silence() {
    let scratch = scratch_root();
    let clips = Clips::new(&scratch);
    assert_eq!(clips.len(), 1);
    assert!(clips.get(clips.silent()).is_empty());
}

#[test]
fn a_handle_from_another_store_does_not_resolve() {
    let scratch = scratch_root();
    let a = Clips::new(&scratch);
    let scratch = scratch_root();
    let b = Clips::new(&scratch);
    assert!(b.try_get(a.silent()).is_some(), "slot zero exists in both");
    assert!(b.try_get(Handle::forge(9, 0)).is_none());
}

/// A root holding `theme.wav` (one second at 8 kHz) and `theme.clip.ron`
/// saying `settings` about it.
fn root_with_settings(settings: &str) -> voltra_testkit::Scratch {
    let root = scratch_root();
    std::fs::write(root.join("theme.wav"), wav(8_000)).expect("the sound writes");
    std::fs::write(root.join("theme.clip.ron"), settings).expect("the settings write");
    root
}

#[test]
fn settings_that_stream_make_a_streamed_clip_from_the_file_on_disk() {
    let root = root_with_settings(
        r#"(version: 1, file: "theme.wav", stream: true, loop_start: 0.25, loop_end: Some(0.75))"#,
    );
    let mut clips = Clips::new(&root);

    let handle = clips.load(&path("theme.clip.ron"));
    let clip = clips.get(handle).clone();

    assert!(clip.is_streamed());
    assert!(clip.samples().is_empty(), "nothing decoded at load");
    assert_eq!(clip.frames(), 8_000);
    assert_eq!((clip.loop_start(), clip.loop_end()), (2_000, Some(6_000)));
    assert!(matches!(
        clip.stream_source().map(|source| &**source),
        Some(StreamSource::File(_))
    ));
}

#[test]
fn settings_that_do_not_stream_decode_the_file_whole_and_keep_the_loop() {
    let root = root_with_settings(r#"(version: 1, file: "theme.wav", loop_start: 0.5)"#);
    let mut clips = Clips::new(&root);

    let handle = clips.load(&path("theme.clip.ron"));
    let clip = clips.get(handle).clone();

    assert!(!clip.is_streamed());
    assert_eq!(clip.samples().len(), 8_000);
    assert_eq!((clip.loop_start(), clip.loop_end()), (4_000, None));
}

#[test]
fn a_streamed_clip_in_a_mounted_pack_streams_from_the_pack_s_bytes() {
    // The web's case: there is no disk to open per voice.
    let scratch = scratch_root();
    let root = scratch.join("packed");
    crate::fs::mount(
        &root,
        [
            ("theme.wav".to_owned(), wav(800)),
            (
                "theme.clip.ron".to_owned(),
                br#"(version: 1, file: "theme.wav", stream: true)"#.to_vec(),
            ),
        ],
    );
    let mut clips = Clips::new(&root);

    let handle = clips.load(&path("theme.clip.ron"));
    let clip = clips.get(handle).clone();
    crate::fs::unmount(&root);

    assert!(clip.is_streamed());
    assert!(matches!(
        clip.stream_source().map(|source| &**source),
        Some(StreamSource::Bytes { .. })
    ));
}

#[test]
fn broken_settings_are_silence_rather_than_a_panic() {
    let root = root_with_settings(r#"(version: 9, file: "theme.wav")"#);
    let mut clips = Clips::new(&root);

    let handle = clips.load(&path("theme.clip.ron"));

    assert!(clips.get(handle).is_empty());
}

#[test]
fn an_edited_sound_reaches_the_clips_its_settings_make() {
    let root = root_with_settings(r#"(version: 1, file: "theme.wav")"#);
    let mut clips = Clips::new(&root);
    let handle = clips.load(&path("theme.clip.ron"));

    std::fs::write(root.join("theme.wav"), wav(16)).expect("the edit writes");
    let changed = clips.reload(&path("theme.wav"));

    assert_eq!(changed, vec![handle], "the sound itself was never loaded");
    assert_eq!(clips.get(handle).frames(), 16);
}

#[test]
fn edited_settings_are_read_again() {
    let root = root_with_settings(r#"(version: 1, file: "theme.wav")"#);
    let mut clips = Clips::new(&root);
    let handle = clips.load(&path("theme.clip.ron"));

    std::fs::write(
        root.join("theme.clip.ron"),
        r#"(version: 1, file: "theme.wav", stream: true)"#,
    )
    .expect("the edit writes");

    assert_eq!(clips.reload(&path("theme.clip.ron")), vec![handle]);
    assert!(clips.get(handle).is_streamed());
}
