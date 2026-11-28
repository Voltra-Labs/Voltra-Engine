//! How a point or spot light fades: by the inverse square of the
//! distance, windowed smoothly to nothing at its range, and a spot's cone
//! — Filament's punctual-light model, number for number with
//! `surface3d_light.wgsl`.

use glam::Vec3;

/// The smallest squared distance the inverse square divides by: a light
/// is taken to be a centimetre across (Filament's), so it stays finite at
/// the point it sits on.
pub const MIN_DISTANCE_SQUARED: f32 = 1.0e-4;

/// The light reaching `distance_squared` m² from a point light of range
/// `range`, per unit of its energy: `w / max(d², 1 cm²)` with Filament's
/// and Godot's smooth window `w = (1 − (d²/r²)²)²` — 1 near the light,
/// its slope 0 where it meets 0 at the range, so a light's edge is never
/// a visible ring. 0 for a range that is not positive or not finite.
pub fn distance_attenuation(distance_squared: f32, range: f32) -> f32 {
    if !(range.is_finite() && range > 0.0) || !distance_squared.is_finite() {
        return 0.0;
    }
    let factor = distance_squared / (range * range);
    let window = (1.0 - factor * factor).clamp(0.0, 1.0);
    window * window / distance_squared.max(MIN_DISTANCE_SQUARED)
}

/// A spot light's cone: the way it shines and the angles, from its axis,
/// it is whole within and gone past.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotCone {
    /// The way it shines, in the world: its entity's −Z.
    pub direction: Vec3,
    /// Radians from the axis within which it shines whole.
    pub inner: f32,
    /// Radians from the axis past which it gives nothing.
    pub outer: f32,
}

impl SpotCone {
    /// The widest a cone opens: a half-turn across. Wider is a point light.
    pub const MAX_ANGLE: f32 = std::f32::consts::FRAC_PI_2;

    /// The cosine ramp's scale and offset, Filament's: the cone's light
    /// along a direction `c` radians off its axis is `saturate(cos c ·
    /// scale + offset)²` — 1 at `inner`, 0 at `outer`. The angles are kept
    /// in `0..=MAX_ANGLE`, `inner` within `outer`; a broken number is 0.
    pub fn scale_offset(&self) -> (f32, f32) {
        let sane = |a: f32| {
            if a.is_finite() {
                a.clamp(0.0, Self::MAX_ANGLE)
            } else {
                0.0
            }
        };
        let outer = sane(self.outer);
        let inner = sane(self.inner).min(outer);
        let (cos_outer, cos_inner) = (outer.cos(), inner.cos());
        let scale = 1.0 / (cos_inner - cos_outer).max(1.0e-4);
        (scale, -cos_outer * scale)
    }

    /// The cone's share of the light along `from_light`, the unit way from
    /// the light to the lit point.
    pub fn attenuation(&self, from_light: Vec3) -> f32 {
        let (scale, offset) = self.scale_offset();
        let cd = self.direction.normalize_or_zero().dot(from_light);
        let ramp = (cd * scale + offset).clamp(0.0, 1.0);
        ramp * ramp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_falloff_is_the_inverse_square_reaching_zero_at_the_range() {
        let range = 5.0;
        // Near the light the window is 1: the inverse square.
        for d in [0.2f32, 0.5, 1.0] {
            let got = distance_attenuation(d * d, range);
            let factor = (d * d) / (range * range);
            assert!((got * d * d - (1.0 - factor * factor).powi(2)).abs() < 1e-6);
        }
        assert_eq!(distance_attenuation(range * range, range), 0.0);
        assert_eq!(distance_attenuation(36.0, range), 0.0, "past the range");
        // Just inside the range: small and positive, and the slope there
        // flat (the window's derivative is 0 at the range).
        let just = distance_attenuation((range - 0.01).powi(2), range);
        assert!(just > 0.0 && just < 1.0e-5, "{just}");
        // Finite at the light itself: a centimetre across.
        assert_eq!(distance_attenuation(0.0, range), 1.0 / MIN_DISTANCE_SQUARED);
        for broken in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(distance_attenuation(1.0, broken), 0.0, "{broken}");
        }
        assert_eq!(distance_attenuation(f32::NAN, range), 0.0);
    }

    #[test]
    fn a_cone_is_whole_inside_its_inner_angle_and_gone_past_its_outer() {
        let cone = SpotCone {
            direction: Vec3::NEG_Y,
            inner: 20f32.to_radians(),
            outer: 30f32.to_radians(),
        };
        let at = |degrees: f32| {
            let r = degrees.to_radians();
            cone.attenuation(Vec3::new(r.sin(), -r.cos(), 0.0))
        };
        assert!((at(0.0) - 1.0).abs() < 1e-6);
        assert!((at(19.9) - 1.0).abs() < 1e-6);
        assert!(at(21.0) < 1.0);
        assert!(at(29.0) > 0.0);
        assert_eq!(at(30.1), 0.0);
        assert_eq!(at(90.0), 0.0);
        // Inner past outer: a hard edge at the outer.
        let hard = SpotCone {
            inner: 1.0,
            outer: 0.5,
            ..cone
        };
        let (scale, offset) = hard.scale_offset();
        assert!(scale.is_finite() && offset.is_finite());
        let broken = SpotCone {
            inner: f32::NAN,
            outer: f32::INFINITY,
            ..cone
        };
        let (scale, offset) = broken.scale_offset();
        assert!(scale.is_finite() && offset.is_finite());
    }
}
