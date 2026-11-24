//! A particle animated through frames of a sheet.
//!
//! Unity's Texture Sheet Animation module, read from an atlas the way a
//! sprite is (`*.atlas.ron`, stage 25): the emitter names its texture as
//! before, the flipbook names the slicing and which of its frames to play.
//! Smoke that billows, a spark that flickers, without one emitter per pose.

use serde::{Deserialize, Serialize};
use voltra_assets::{AssetPath, Atlas, Atlases, Handle};
use voltra_render::particles::TABLE_SAMPLES;
use voltra_render::FrameTiming;

/// The most frames a flipbook plays: what the GPU's table holds, so both
/// simulations play the same ones.
pub const MAX_FLIPBOOK_FRAMES: usize = TABLE_SAMPLES;

/// How a flipbook's frames follow a particle's life.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FlipbookTiming {
    /// Through every frame `cycles` times from birth to death, whatever the
    /// lifetime: a puff that always ends on its last frame.
    OverLife {
        /// Passes through the frames in one life.
        cycles: f32,
    },
    /// At a fixed rate from the first frame at birth, round again after
    /// the last: a flame that flickers at the same speed however long it
    /// lives.
    PerSecond {
        /// Frames a second.
        fps: f32,
    },
}

impl Default for FlipbookTiming {
    /// Once through, birth to death.
    fn default() -> Self {
        Self::OverLife { cycles: 1.0 }
    }
}

impl FlipbookTiming {
    /// Which of `count` frames a particle `age` seconds old, `t` of the way
    /// through its life, shows — `particles_draw.wgsl`'s `frame_of`. Zero
    /// with no frames or a rate that is not finite.
    pub fn index(&self, age: f32, t: f32, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        let steps = match *self {
            Self::OverLife { cycles } => (t * cycles * count as f32).floor(),
            Self::PerSecond { fps } => (age * fps).floor(),
        };
        if !steps.is_finite() {
            return 0;
        }
        (steps as i64).rem_euclid(count as i64) as usize
    }

    fn gpu(&self) -> FrameTiming {
        match *self {
            Self::OverLife { cycles } => FrameTiming::OverLife { cycles },
            Self::PerSecond { fps } => FrameTiming::PerSecond { fps },
        }
    }
}

/// Frames of an atlas a particle steps through.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flipbook {
    /// The slicing the frames are cut from, by path. The emitter's
    /// `texture` is the sheet, as a sprite's is.
    pub atlas: AssetPath,
    /// What `atlas` resolves to in this session's atlases. Never written.
    #[serde(skip)]
    pub atlas_handle: Option<Handle<Atlas>>,
    /// Which of the atlas's frames, in the order played. Empty plays every
    /// frame in index order. An index past the atlas's last frame is left
    /// out; past [`MAX_FLIPBOOK_FRAMES`], the rest are.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<u32>,
    /// How the frames follow a particle's life.
    #[serde(default)]
    pub timing: FlipbookTiming,
}

impl Flipbook {
    /// Every frame of `atlas`, once through a life.
    pub fn new(atlas: AssetPath) -> Self {
        Self {
            atlas,
            atlas_handle: None,
            frames: Vec::new(),
            timing: FlipbookTiming::default(),
        }
    }

    /// Refreshes the handle from the path against `atlases`.
    pub fn resolve(&mut self, atlases: &mut Atlases) {
        self.atlas_handle = Some(atlases.load(&self.atlas));
    }

    /// The frames played, as UV rectangles `(min.x, min.y, max.x, max.y)`:
    /// empty before the atlas is resolved or when none of its frames can be
    /// drawn — the whole texture, then.
    pub fn uvs(&self, atlases: Option<&Atlases>) -> Vec<[f32; 4]> {
        let Some(atlas) = self
            .atlas_handle
            .and_then(|handle| atlases.and_then(|atlases| atlases.try_get(handle)))
        else {
            return Vec::new();
        };
        let all: Vec<u32>;
        let indices = if self.frames.is_empty() {
            all = (0..atlas.len() as u32).collect();
            &all
        } else {
            &self.frames
        };
        indices
            .iter()
            .filter_map(|&index| atlas.frame(index)?.uv(atlas.sheet()))
            .map(|(min, max)| [min[0], min[1], max[0], max[1]])
            .take(MAX_FLIPBOOK_FRAMES)
            .collect()
    }

    /// How the GPU steps through the frames.
    pub(crate) fn gpu_timing(&self) -> FrameTiming {
        self.timing.gpu()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn over_life_ends_on_the_last_frame() {
        let timing = FlipbookTiming::OverLife { cycles: 1.0 };
        assert_eq!(timing.index(0.0, 0.0, 4), 0);
        assert_eq!(timing.index(0.0, 0.5, 4), 2);
        assert_eq!(timing.index(0.0, 0.99, 4), 3);
    }

    #[test]
    fn a_rate_wraps_round() {
        let timing = FlipbookTiming::PerSecond { fps: 10.0 };
        assert_eq!(timing.index(0.25, 0.0, 4), 2);
        assert_eq!(timing.index(0.55, 0.0, 4), 1);
        assert_eq!(timing.index(-0.15, 0.0, 4), 2, "a negative step wraps");
    }

    #[test]
    fn nonsense_shows_the_first_frame() {
        let timing = FlipbookTiming::PerSecond { fps: f32::NAN };
        assert_eq!(timing.index(1.0, 0.5, 4), 0);
        assert_eq!(FlipbookTiming::default().index(1.0, 0.5, 0), 0);
    }

    #[test]
    fn unresolved_frames_are_the_whole_texture() {
        let path = AssetPath::new("fx.atlas.ron").expect("a path");
        assert!(Flipbook::new(path).uvs(None).is_empty());
    }
}
