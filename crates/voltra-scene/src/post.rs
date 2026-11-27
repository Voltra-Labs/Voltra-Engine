//! A camera's look: the grading its frame is drawn with.

use voltra_ecs::World;
use voltra_render::{BloomSettings, PostSettings};

use crate::camera::{self, Camera2D};

/// How the frame seen through a [`Camera2D`] is graded: exposure, contrast,
/// saturation, a tint and a vignette — Godot's `Environment` adjustments and
/// Unity's colour-grading volume, cut to what a 2D game reaches for.
///
/// Neutral by default, and not written to a scene while it is.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PostProcess {
    /// Multiplies every colour; `1` leaves it.
    pub exposure: f32,
    /// Around a linear middle grey; `1` leaves it, `0` is flat grey.
    pub contrast: f32,
    /// `1` leaves colour, `0` is greyscale, above `1` is more vivid.
    pub saturation: f32,
    /// Multiplies each channel; white leaves it.
    pub tint: [f32; 3],
    /// How far the corners darken, `0` to `1`.
    pub vignette: f32,
    /// Glow around what is bright. Off by default.
    pub bloom: Bloom,
}

/// How much a camera's frame glows, and from what: see
/// [`BloomSettings`](voltra_render::BloomSettings).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Bloom {
    /// How strongly the glow is added; `0` is off.
    pub intensity: f32,
    /// Linear brightness where bloom begins.
    pub threshold: f32,
    /// How gradually it begins, `0` (a hard edge) to `1`.
    pub softness: f32,
    /// How far each upsample reaches, in texels of its source; larger is a wider, softer glow.
    pub radius: f32,
}

impl Default for Bloom {
    fn default() -> Self {
        Self::from(BloomSettings::OFF)
    }
}

impl From<BloomSettings> for Bloom {
    fn from(s: BloomSettings) -> Self {
        Self {
            intensity: s.intensity,
            threshold: s.threshold,
            softness: s.softness,
            radius: s.radius,
        }
    }
}

impl Bloom {
    /// These values as the renderer's [`BloomSettings`].
    pub fn settings(&self) -> BloomSettings {
        BloomSettings {
            intensity: self.intensity,
            threshold: self.threshold,
            softness: self.softness,
            radius: self.radius,
        }
    }
}

impl Default for PostProcess {
    fn default() -> Self {
        Self::from(PostSettings::NEUTRAL)
    }
}

impl From<PostSettings> for PostProcess {
    fn from(s: PostSettings) -> Self {
        Self {
            exposure: s.exposure,
            contrast: s.contrast,
            saturation: s.saturation,
            tint: s.tint,
            vignette: s.vignette,
            bloom: Bloom::from(s.bloom),
        }
    }
}

impl PostProcess {
    /// Whether this changes nothing.
    pub fn is_neutral(&self) -> bool {
        *self == Self::default()
    }

    /// The renderer's form.
    pub fn settings(&self) -> PostSettings {
        PostSettings {
            exposure: self.exposure,
            contrast: self.contrast,
            saturation: self.saturation,
            tint: self.tint,
            vignette: self.vignette,
            bloom: self.bloom.settings(),
        }
    }
}

/// The grading of the camera the scene renders through, or neutral with no
/// active camera. The editor previews it too, as Godot shows its
/// environment while editing.
pub fn active(world: &World) -> PostSettings {
    camera::active(world)
        .and_then(|entity| world.get::<Camera2D>(entity))
        .map_or(PostSettings::NEUTRAL, |camera| camera.post.settings())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_neutral_grade_is_not_written_and_reads_back_neutral() {
        let text = ron::to_string(&Camera2D::default()).expect("writes");
        assert!(!text.contains("post"));
        let back: Camera2D = ron::from_str(&text).expect("reads");
        assert!(back.post.is_neutral());
    }

    #[test]
    fn a_partial_grade_fills_the_rest_neutral() {
        let post: PostProcess = ron::from_str("(vignette: 0.6)").expect("reads");
        assert_eq!(post.vignette, 0.6);
        assert_eq!(post.exposure, 1.0);
    }

    #[test]
    fn bloom_reads_on_its_own_and_reaches_the_renderer() {
        let post: PostProcess = ron::from_str("(bloom: (intensity: 0.7))").expect("reads");
        assert!(!post.is_neutral());
        let settings = post.settings();
        assert_eq!(settings.bloom.intensity, 0.7);
        assert_eq!(settings.bloom.threshold, BloomSettings::OFF.threshold);
    }

    #[test]
    fn the_active_camera_s_grade_is_the_frame_s() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(
            e,
            Camera2D {
                post: PostProcess {
                    saturation: 0.0,
                    ..PostProcess::default()
                },
                ..Camera2D::default()
            },
        );
        assert_eq!(active(&world).saturation, 0.0);
        assert_eq!(active(&World::new()), PostSettings::NEUTRAL);
    }
}
