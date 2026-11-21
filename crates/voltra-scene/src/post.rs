//! A camera's look: the grading its frame is drawn with.

use voltra_ecs::World;
use voltra_render::PostSettings;

use crate::camera::{self, Camera};

/// How the frame seen through a [`Camera`] is graded: exposure, contrast,
/// saturation, a tint and a vignette — Godot's `Environment` adjustments and
/// Unity's colour-grading volume, cut to what a 2D game reaches for.
///
/// Neutral by default, and not written to a scene while it is.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PostProcess {
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub tint: [f32; 3],
    pub vignette: f32,
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
        }
    }
}

/// The grading of the camera the scene renders through, or neutral with no
/// active camera. The editor previews it too, as Godot shows its
/// environment while editing.
pub fn active(world: &World) -> PostSettings {
    camera::active(world)
        .and_then(|entity| world.get::<Camera>(entity))
        .map_or(PostSettings::NEUTRAL, |camera| camera.post.settings())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_neutral_grade_is_not_written_and_reads_back_neutral() {
        let text = ron::to_string(&Camera::default()).expect("writes");
        assert!(!text.contains("post"));
        let back: Camera = ron::from_str(&text).expect("reads");
        assert!(back.post.is_neutral());
    }

    #[test]
    fn a_partial_grade_fills_the_rest_neutral() {
        let post: PostProcess = ron::from_str("(vignette: 0.6)").expect("reads");
        assert_eq!(post.vignette, 0.6);
        assert_eq!(post.exposure, 1.0);
    }

    #[test]
    fn the_active_camera_s_grade_is_the_frame_s() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(
            e,
            Camera {
                post: PostProcess {
                    saturation: 0.0,
                    ..PostProcess::default()
                },
                ..Camera::default()
            },
        );
        assert_eq!(active(&world).saturation, 0.0);
        assert_eq!(active(&World::new()), PostSettings::NEUTRAL);
    }
}
