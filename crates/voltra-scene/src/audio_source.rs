//! The audio source component: an entity that makes a sound.
//!
//! Unity's `AudioSource`, Godot's `AudioStreamPlayer2D`, Unreal's
//! `AudioComponent`. All three settled on the same shape — a clip, a gain, a
//! pitch, a loop flag, an autoplay flag and a falloff — and on the same
//! division of labour: the component says *what* and *how loud*, and the
//! engine decides where in the mix that lands each frame. This is that
//! component; the arithmetic behind "where in the mix" is
//! [`voltra_audio::spatial`].

use voltra_assets::{AssetPath, Clips, Handle};
use voltra_audio::{Clip, PlayParams};

/// A sound an entity can play, positioned by that entity's [`Transform`].
///
/// [`Transform`]: crate::transform::Transform
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AudioSource {
    /// The sound this source plays, by path relative to the asset root.
    ///
    /// `None` is a source with nothing to play, which is what an entity looks
    /// like between being given the component and being given a file.
    #[serde(default)]
    pub clip: Option<AssetPath>,
    /// The handle `clip` currently resolves to, if any.
    ///
    /// Never serialised, for the same reason [`Sprite::texture_handle`] is
    /// not: it addresses a slot in whichever store loaded this session, which
    /// means nothing across a save, a load, or two runs of the same binary.
    ///
    /// [`Sprite::texture_handle`]: crate::sprite::Sprite::texture_handle
    #[serde(skip)]
    pub clip_handle: Option<Handle<Clip>>,
    /// Linear gain before the distance falloff. `1.0` plays it as recorded.
    #[serde(default = "one")]
    pub volume: f32,
    /// Playback speed, which also shifts the pitch. `1.0` is as recorded.
    #[serde(default = "one")]
    pub pitch: f32,
    /// Whether the clip starts again at its end instead of finishing.
    ///
    /// Ambience and music; a coin is not this. A looping source is stopped by
    /// the engine when its entity loses the component or the world stops, and
    /// by nothing else.
    #[serde(default)]
    pub looping: bool,
    /// Whether the engine starts it as soon as the world is live.
    ///
    /// Unity's `playOnAwake`, Godot's `autoplay`. Off by default: most sources
    /// are triggered by something happening, and one that started itself the
    /// moment it was authored would be a surprise in the editor's play mode.
    #[serde(default)]
    pub play_on_spawn: bool,
    /// How far away the source can still be heard, in world units.
    ///
    /// The distance at which it reaches silence, and the distance that maps to
    /// a fully panned ear — see [`voltra_audio::spatial`] for the curve and
    /// why it is that curve. `0.0` means the sound is not positional at all:
    /// full volume, centred, wherever the listener is. That is the right
    /// answer for music and for UI, and it is why this is a number rather than
    /// a `spatial: bool` beside it.
    #[serde(default = "default_range")]
    pub range: f32,
}

/// `1.0`, for the fields a scene file may leave out.
///
/// A missing `volume` has to mean "as recorded" rather than `f32::default()`,
/// which is silence — a file written before a field existed must not go quiet
/// when the field arrives.
fn one() -> f32 {
    1.0
}

/// Ten world units: far enough that a source across a small room is audible
/// and one across a level is not.
///
/// A number the author is expected to change, not a constant the engine
/// depends on. It exists because a source dropped into a scene with `0.0`
/// would not be positional at all, and "it plays but it does not move" is a
/// worse first impression than "it fades sooner than I wanted".
const DEFAULT_RANGE: f32 = 10.0;

fn default_range() -> f32 {
    DEFAULT_RANGE
}

impl Default for AudioSource {
    fn default() -> Self {
        Self {
            clip: None,
            clip_handle: None,
            volume: 1.0,
            pitch: 1.0,
            looping: false,
            play_on_spawn: false,
            range: DEFAULT_RANGE,
        }
    }
}

impl AudioSource {
    /// A source naming `path`, not yet resolved against any store.
    pub fn new(path: AssetPath) -> Self {
        Self {
            clip: Some(path),
            ..Default::default()
        }
    }

    /// Sets or clears the clip path and refreshes the runtime handle.
    ///
    /// No device, the way [`Sprite::set_atlas`] needs no GPU: decoding is
    /// arithmetic over a file, and whether anything can be heard is settled
    /// much later, by [`Audio`](voltra_audio::Audio).
    ///
    /// [`Sprite::set_atlas`]: crate::sprite::Sprite::set_atlas
    pub fn set_clip(&mut self, path: Option<AssetPath>, clips: &mut Clips) {
        match path {
            None => {
                self.clip = None;
                self.clip_handle = None;
            }
            Some(path) => {
                let handle = clips.load(&path);
                self.clip = Some(path);
                self.clip_handle = Some(handle);
            }
        }
    }

    /// How this source wants to be played, before anything positional.
    ///
    /// The pan and the final volume are decided per frame from where the
    /// entity is; this carries what the author set. Keeping the conversion
    /// here rather than in the loop is what stops a second caller — a prefab
    /// spawning a sound, a tick playing one directly — inventing a slightly
    /// different one.
    pub fn params(&self) -> PlayParams {
        PlayParams {
            volume: self.volume,
            pitch: self.pitch,
            pan: 0.0,
            looping: self.looping,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_testkit::scratch_root;

    fn path(raw: &str) -> AssetPath {
        AssetPath::new(raw).expect("a valid asset path")
    }

    #[test]
    fn a_default_source_plays_nothing_at_full_volume() {
        let source = AudioSource::default();
        assert_eq!(source.clip, None);
        assert_eq!(source.volume, 1.0);
        assert_eq!(source.pitch, 1.0);
        assert!(!source.looping);
        assert!(!source.play_on_spawn);
    }

    #[test]
    fn the_params_carry_what_the_author_set() {
        let source = AudioSource {
            volume: 0.5,
            pitch: 2.0,
            looping: true,
            ..Default::default()
        };

        let params = source.params();

        assert_eq!(params.volume, 0.5);
        assert_eq!(params.pitch, 2.0);
        assert!(params.looping);
        assert_eq!(params.pan, 0.0, "the pan is the frame's, not the author's");
    }

    #[test]
    fn setting_a_clip_resolves_a_handle() {
        let root = scratch_root();
        let mut clips = Clips::new(&root);
        let mut source = AudioSource::default();

        source.set_clip(Some(path("coin.wav")), &mut clips);

        assert_eq!(source.clip, Some(path("coin.wav")));
        assert!(source.clip_handle.is_some(), "even a missing file resolves");
    }

    #[test]
    fn clearing_a_clip_clears_the_handle_with_it() {
        // A handle left behind would keep playing a sound the author removed.
        let root = scratch_root();
        let mut clips = Clips::new(&root);
        let mut source = AudioSource::default();
        source.set_clip(Some(path("coin.wav")), &mut clips);

        source.set_clip(None, &mut clips);

        assert_eq!(source.clip, None);
        assert_eq!(source.clip_handle, None);
    }

    #[test]
    fn a_source_round_trips_through_ron() {
        let source = AudioSource {
            clip: Some(path("sfx/coin.wav")),
            volume: 0.25,
            looping: true,
            range: 4.0,
            ..Default::default()
        };

        let text = ron::to_string(&source).expect("serializes");
        let back: AudioSource = ron::from_str(&text).expect("deserializes");

        assert_eq!(back, source);
    }

    #[test]
    fn the_handle_is_never_written_to_the_file() {
        // It addresses a slot in this session's store. A file carrying one
        // would resolve to a different sound — or to nothing — on the next run.
        let root = scratch_root();
        let mut clips = Clips::new(&root);
        let mut source = AudioSource::default();
        source.set_clip(Some(path("coin.wav")), &mut clips);

        let text = ron::to_string(&source).expect("serializes");

        assert!(!text.contains("clip_handle"), "got {text}");
        let back: AudioSource = ron::from_str(&text).expect("deserializes");
        assert_eq!(back.clip_handle, None);
    }

    #[test]
    fn a_file_written_before_a_field_existed_still_loads_at_full_volume() {
        // The reason `volume` and `pitch` default to one rather than to
        // `f32::default()`: a scene saved by an older build must not go silent
        // or freeze on its first frame.
        let older = r#"(clip: Some(Path("sfx/coin.wav")))"#;

        let source: AudioSource = ron::from_str(older).expect("an older scene still loads");

        assert_eq!(source.volume, 1.0);
        assert_eq!(source.pitch, 1.0);
        assert_eq!(source.range, DEFAULT_RANGE);
    }
}
