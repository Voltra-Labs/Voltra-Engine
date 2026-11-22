//! Clips, keyed by the path a player names them with.

use super::clip::AnimationClip;
use crate::ron_store::{RonAsset, RonAssetError, RonStore};

/// Reads clips from an asset root and hands out shared handles to them —
/// a [`RonStore`]; a clip that will not read animates nothing.
pub type AnimationClips = RonStore<AnimationClip>;

/// Why a clip file could not be read or written.
pub type ClipError = RonAssetError;

impl RonAsset for AnimationClip {
    const KIND: &'static str = "animation";

    fn checked(self, source: &str) -> Self {
        AnimationClip::checked(self, source)
    }
}
