//! What a particle looks like and how fast it goes at a point in its life:
//! the start/end ranges shaped by the emitter's curves and gradient. One
//! answer for the CPU's quads and the GPU's tables.

use voltra_assets::Atlases;
use voltra_render::particles::TABLE_SAMPLES;
use voltra_render::ParticleTables;

use super::ParticleEmitter;
use crate::curve::{table, Curve};

/// Side of a particle's square `t` of the way through its life: the size
/// range blended, times the size curve.
pub fn size_at(emitter: &ParticleEmitter, t: f32) -> f32 {
    lerp(emitter.size.0, emitter.size.1, t) * curve_at(emitter.size_curve.as_ref(), t)
}

/// What a particle's distance travelled is scaled by, `t` of the way
/// through its life: the speed curve, `1` without one.
pub fn speed_at(emitter: &ParticleEmitter, t: f32) -> f32 {
    curve_at(emitter.speed_curve.as_ref(), t)
}

/// A particle's linear RGBA `t` of the way through its life: the gradient
/// (or the start/end colours without one), its alpha times the alpha curve.
pub fn color_at(emitter: &ParticleEmitter, t: f32) -> [f32; 4] {
    let mut color = base_color_at(emitter, t);
    color[3] *= curve_at(emitter.alpha_curve.as_ref(), t);
    color
}

/// The colour before the alpha curve.
fn base_color_at(emitter: &ParticleEmitter, t: f32) -> [f32; 4] {
    emitter
        .gradient
        .as_ref()
        .and_then(|gradient| gradient.sample(t))
        .unwrap_or_else(|| {
            std::array::from_fn(|i| lerp(emitter.color_start[i], emitter.color_end[i], t))
        })
}

/// The tables a GPU emitter reads in place of the curves: the same
/// functions, sampled; the flipbook's frames from `atlases`; the outline it
/// was last stepped with.
pub fn tables(emitter: &ParticleEmitter, atlases: Option<&Atlases>) -> ParticleTables {
    ParticleTables {
        color: table(TABLE_SAMPLES, |t| base_color_at(emitter, t)),
        curves: table(TABLE_SAMPLES, |t| {
            [
                size_at(emitter, t),
                speed_at(emitter, t),
                curve_at(emitter.alpha_curve.as_ref(), t),
                0.0,
            ]
        }),
        frames: emitter
            .flipbook
            .as_ref()
            .map_or_else(Vec::new, |flipbook| flipbook.uvs(atlases)),
        outline: emitter.state.outline.clone(),
    }
}

/// The most the size and the speed reach over a life, for the GPU's
/// culling box.
pub(crate) fn peaks(emitter: &ParticleEmitter) -> (f32, f32) {
    let peak = |curve: Option<&Curve>| {
        curve.map_or(1.0, |curve| {
            let (lo, hi) = curve.range();
            lo.abs().max(hi.abs())
        })
    };
    let size = emitter.size.0.abs().max(emitter.size.1.abs()) * peak(emitter.size_curve.as_ref());
    (size, peak(emitter.speed_curve.as_ref()))
}

fn curve_at(curve: Option<&Curve>, t: f32) -> f32 {
    curve.map_or(1.0, |curve| curve.sample(t))
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::CurveKey;
    use crate::gradient::Gradient;

    #[test]
    fn without_curves_a_particle_blends_its_ranges() {
        let emitter = ParticleEmitter {
            size: (2.0, 0.0),
            ..ParticleEmitter::default()
        };
        assert_eq!(size_at(&emitter, 0.25), 1.5);
        assert_eq!(speed_at(&emitter, 0.5), 1.0);
        assert_eq!(color_at(&emitter, 0.5), [1.0, 1.0, 1.0, 0.5]);
    }

    #[test]
    fn curves_and_a_gradient_shape_the_ranges() {
        let emitter = ParticleEmitter {
            size: (2.0, 2.0),
            size_curve: Some(Curve::linear(0.0, 1.0)),
            alpha_curve: Some(Curve::new(vec![CurveKey::new(0.0, 0.5)])),
            gradient: Some(Gradient::linear([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0])),
            ..ParticleEmitter::default()
        };
        assert_eq!(size_at(&emitter, 0.5), 1.0);
        assert_eq!(color_at(&emitter, 0.5), [0.5, 0.0, 0.5, 0.5]);
        assert_eq!(peaks(&emitter).0, 2.0);
        let tables = tables(&emitter, None);
        assert_eq!(tables.curves[0], [0.0, 1.0, 0.5, 0.0]);
        assert_eq!(tables.curves[TABLE_SAMPLES - 1][0], 2.0);
        assert_eq!(tables.color[0], [1.0, 0.0, 0.0, 1.0], "alpha apart");
    }
}
