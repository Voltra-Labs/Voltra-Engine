//! Emitters to start from: smoke, sparks and rain.
//!
//! What Unity's and Godot's particle editors offer as a menu of starting
//! points, here as data a game can use too. A preset is the emitter's
//! motion and look; where it draws (texture, sorting, light layers), how it
//! is simulated and what it collides with are the emitter's own and kept.

use std::f32::consts::{FRAC_PI_2, TAU};

use voltra_render::glam::Vec2;

use super::{EmitShape, ParticleEmitter};
use crate::curve::{Curve, CurveKey};
use crate::gradient::{Gradient, GradientKey};
use crate::keyframes::Ease;

/// A starting point for an emitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// Soft grey puffs that rise, grow, turn and fade.
    Smoke,
    /// A spray of hot sparks that fall, slow and burn out.
    Sparks,
    /// Drops falling from a wide line across the top.
    Rain,
}

impl Preset {
    /// Every preset, as an editor lists them.
    pub const ALL: [Self; 3] = [Self::Smoke, Self::Sparks, Self::Rain];

    /// What an editor calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Smoke => "smoke",
            Self::Sparks => "sparks",
            Self::Rain => "rain",
        }
    }

    /// `base` with this preset's motion and look: its texture, sort order,
    /// sorting layer, light layers, simulation, collision, one-shot flag and
    /// flipbook kept, its live particles gone.
    pub fn apply(self, base: &ParticleEmitter) -> ParticleEmitter {
        let look = match self {
            Self::Smoke => smoke(),
            Self::Sparks => sparks(),
            Self::Rain => rain(),
        };
        ParticleEmitter {
            texture: base.texture.clone(),
            texture_handle: base.texture_handle,
            sort_order: base.sort_order,
            sorting_layer: base.sorting_layer.clone(),
            light_layers: base.light_layers,
            simulation: base.simulation,
            collision: base.collision,
            one_shot: base.one_shot,
            flipbook: base.flipbook.clone(),
            ..look
        }
    }
}

fn smoke() -> ParticleEmitter {
    let grey = |a: f32| [0.55, 0.55, 0.58, a];
    ParticleEmitter {
        rate: 12.0,
        max_particles: 128,
        lifetime: (2.0, 3.0),
        speed: (0.3, 0.6),
        direction: FRAC_PI_2,
        spread: 0.6,
        acceleration: Vec2::new(0.15, 0.25),
        shape: EmitShape::Circle { radius: 0.2 },
        size: (0.5, 0.5),
        size_curve: Some(Curve::new(vec![
            CurveKey {
                ease: Ease::EaseOut,
                ..CurveKey::new(0.0, 0.6)
            },
            CurveKey::new(1.0, 2.5),
        ])),
        speed_curve: Some(Curve::linear(1.0, 0.4)),
        gradient: Some(Gradient::new(vec![
            GradientKey::new(0.0, grey(0.0)),
            GradientKey::new(0.15, grey(0.6)),
            GradientKey::new(1.0, grey(0.0)),
        ])),
        rotation: (0.0, TAU),
        angular_velocity: (-0.6, 0.6),
        ..ParticleEmitter::default()
    }
}

fn sparks() -> ParticleEmitter {
    ParticleEmitter {
        rate: 40.0,
        max_particles: 256,
        lifetime: (0.3, 0.7),
        speed: (3.0, 6.0),
        direction: FRAC_PI_2,
        spread: TAU,
        acceleration: Vec2::new(0.0, -6.0),
        size: (0.08, 0.08),
        size_curve: Some(Curve::linear(1.0, 0.3)),
        speed_curve: Some(Curve::linear(1.0, 0.3)),
        gradient: Some(Gradient::new(vec![
            GradientKey::new(0.0, [1.0, 0.95, 0.6, 1.0]),
            GradientKey::new(0.4, [1.0, 0.55, 0.1, 1.0]),
            GradientKey::new(1.0, [0.7, 0.1, 0.0, 0.0]),
        ])),
        ..ParticleEmitter::default()
    }
}

fn rain() -> ParticleEmitter {
    ParticleEmitter {
        rate: 150.0,
        max_particles: 512,
        lifetime: (1.2, 1.2),
        speed: (9.0, 11.0),
        direction: -FRAC_PI_2 - 0.1,
        spread: 0.05,
        acceleration: Vec2::ZERO,
        shape: EmitShape::Edge { half_length: 8.0 },
        size: (0.06, 0.06),
        color_start: [0.6, 0.7, 0.9, 0.7],
        color_end: [0.6, 0.7, 0.9, 0.5],
        alpha_curve: Some(Curve::new(vec![
            CurveKey::new(0.0, 0.0),
            CurveKey::new(0.1, 1.0),
        ])),
        ..ParticleEmitter::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preset_keeps_where_and_how_the_emitter_draws() {
        let base = ParticleEmitter {
            sort_order: 7,
            light_layers: 0b10,
            simulation: super::super::Simulation::Gpu,
            ..ParticleEmitter::default()
        };
        for preset in Preset::ALL {
            let made = preset.apply(&base);
            assert_eq!(made.sort_order, 7);
            assert_eq!(made.light_layers, 0b10);
            assert_eq!(made.simulation, base.simulation);
            assert_ne!(made.rate, base.rate, "{preset:?} changes the motion");
        }
    }
}
