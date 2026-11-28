//! What the post pass's shader is handed: the grade's numbers, made safe.

use bytemuck::{Pod, Zeroable};

use super::PostSettings;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct PostUniform {
    tint_exposure: [f32; 4],
    grade: [f32; 4],
    /// x: bloom intensity, y: the tone curve's number; the rest unused.
    bloom: [f32; 4],
}

impl PostUniform {
    /// With the tone curve `grade` names, if it names one.
    pub(super) fn with_grade(mut self, grade: super::Grade) -> Self {
        if let super::Grade::Tonemap(curve) = grade {
            self.bloom[1] = curve.index();
        }
        self
    }

    pub(super) fn new(settings: &PostSettings, aspect: f32) -> Self {
        let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        Self {
            tint_exposure: [
                finite(settings.tint[0], 1.0).max(0.0),
                finite(settings.tint[1], 1.0).max(0.0),
                finite(settings.tint[2], 1.0).max(0.0),
                finite(settings.exposure, 1.0).max(0.0),
            ],
            grade: [
                finite(settings.contrast, 1.0).max(0.0),
                finite(settings.saturation, 1.0).max(0.0),
                finite(settings.vignette, 0.0).clamp(0.0, 1.0),
                finite(aspect, 1.0).max(1e-3),
            ],
            bloom: [
                finite(settings.bloom.intensity, 0.0).max(0.0),
                0.0,
                0.0,
                0.0,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bloom::BloomSettings;

    #[test]
    fn nonsense_settings_become_safe_ones() {
        let wild = PostSettings {
            exposure: f32::NAN,
            contrast: -3.0,
            saturation: f32::INFINITY,
            tint: [-1.0, f32::NAN, 2.0],
            vignette: 7.0,
            bloom: BloomSettings {
                intensity: -2.0,
                ..BloomSettings::OFF
            },
        };
        let u = PostUniform::new(&wild, f32::NAN);
        assert_eq!(u.tint_exposure, [0.0, 1.0, 2.0, 1.0]);
        assert_eq!(u.grade, [0.0, 1.0, 1.0, 1.0]);
        assert_eq!(u.bloom[0], 0.0);
    }
}
