//! Sounds, keyed by the path a scene file names them with.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voltra_audio::{decode, Clip};

use crate::error::AssetError;
use crate::handle::Handle;
use crate::path::AssetPath;
use crate::store::Assets;

/// Loads clips from an asset root and hands out shared handles to them.
///
/// The same shape as [`Textures`](crate::Textures) and
/// [`Atlases`](crate::Atlases) — load once per path, a stable handle, a
/// placeholder for what will not load — because three stores that behaved
/// differently would be three things to remember. It needs no GPU and no
/// audio device: decoding is arithmetic over a file, which is what keeps every
/// test of it headless.
///
/// Hot reload is deliberately absent for now. Swapping the samples under a
/// handle while a voice is reading them is not the same problem as swapping a
/// texture between frames — the audio thread is inside the buffer — and it
/// wants the same care the reload of a playing sound gets in Wwise. The handle
/// is already stable, so adding it later changes nothing above.
#[derive(Debug)]
pub struct Clips {
    root: PathBuf,
    store: Assets<Clip>,
    by_path: HashMap<AssetPath, Handle<Clip>>,
    silent: Handle<Clip>,
}

impl Clips {
    /// A store rooted at `root`, with the silent clip already in it.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut store = Assets::new();
        let silent = store.insert(Clip::silent());
        Self {
            root: root.into(),
            store,
            by_path: HashMap::new(),
            silent,
        }
    }

    /// The handle for `path`, decoding it if this is the first time.
    ///
    /// Infallible, the way texture and atlas loading are: a scene naming a
    /// sound that will not decode must still open. A failure logs once and
    /// returns a silent clip — see [`Clip::silent`] on why silence and not a
    /// placeholder tone. The answer is cached, or a broken file would be
    /// re-read and re-warned every time something triggered it.
    pub fn load(&mut self, path: &AssetPath) -> Handle<Clip> {
        if let Some(handle) = self.by_path.get(path) {
            return *handle;
        }

        let handle = match decode(&self.root.join(path.as_str())) {
            Ok(clip) => self.store.insert(clip),
            Err(source) => {
                let error = AssetError::Audio {
                    path: self.root.join(path.as_str()),
                    source: Box::new(source),
                };
                log::warn!("{error}; it will play nothing");
                // Its own slot rather than the shared silent one, for the same
                // reason `Textures` gives each broken path its own placeholder:
                // repairing this file must not mean overwriting the clip every
                // other broken path is also using.
                self.store.insert(Clip::silent())
            }
        };

        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// The handle `path` is cached to, if it has ever been loaded.
    pub fn by_path_handle(&self, path: &AssetPath) -> Option<Handle<Clip>> {
        self.by_path.get(path).copied()
    }

    /// The clip `handle` names.
    ///
    /// Every handle this type hands out resolves: the silent one is inserted
    /// at construction and nothing here ever removes. Only a handle forged
    /// elsewhere can reach the `expect`.
    pub fn get(&self, handle: Handle<Clip>) -> &Clip {
        self.try_get(handle)
            .expect("Clips never removes, so every handle it issued resolves")
    }

    /// The clip `handle` names, or `None` for a handle this store never
    /// issued — the same question [`get`](Self::get) answers, without the
    /// invariant, for a caller holding a handle from some other store.
    pub fn try_get(&self, handle: Handle<Clip>) -> Option<&Clip> {
        self.store.get(handle)
    }

    /// The clip with nothing in it, played by anything that would not decode.
    pub fn silent(&self) -> Handle<Clip> {
        self.silent
    }

    /// The directory every [`AssetPath`] is resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

#[cfg(test)]
mod tests {
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
    fn a_new_store_holds_only_silence() {
        let clips = Clips::new(scratch_root());
        assert_eq!(clips.len(), 1);
        assert!(clips.get(clips.silent()).is_empty());
    }

    #[test]
    fn a_handle_from_another_store_does_not_resolve() {
        let a = Clips::new(scratch_root());
        let b = Clips::new(scratch_root());
        assert!(b.try_get(a.silent()).is_some(), "slot zero exists in both");
        assert!(b.try_get(Handle::forge(9, 0)).is_none());
    }
}
