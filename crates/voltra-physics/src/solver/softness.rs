//! A constraint's spring, as three precomputed coefficients.

use std::f32::consts::PI;

/// How hard a constraint pushes, and how much of that push is taken back.
///
/// Soft constraints are the evolution of Baumgarte stabilisation: instead of a
/// magic fraction of the overlap fed back as velocity, the response is a
/// mass-spring-damper tuned by a natural frequency and a damping ratio, both of
/// which mean something a human can reason about. The three coefficients here
/// are what that spring reduces to once the step size is known.
///
/// The formulas are `b2MakeSoft` from the Box2D v3 source, verbatim:
///
/// ```text
/// omega         = 2π · hertz
/// a1            = 2ζ + h·omega
/// a2            = h·omega·a1
/// a3            = 1 / (1 + a2)
/// bias_rate     = omega / a1
/// mass_scale    = a2 · a3
/// impulse_scale = a3
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Softness {
    /// Multiplies the separation to give the velocity that pushes bodies apart.
    pub bias_rate: f32,
    /// Scales the relative velocity in the impulse. Below one, the constraint
    /// gives way rather than resolving the whole error in one solve.
    pub mass_scale: f32,
    /// Fraction of the accumulated impulse removed each solve, which is what
    /// keeps a soft constraint from storing energy indefinitely.
    pub impulse_scale: f32,
}

impl Softness {
    /// No softening at all: the constraint is solved hard.
    ///
    /// Box2D returns three zeroes for a zero frequency, because there the case
    /// arises from a joint whose spring is disabled and a zero `mass_scale`
    /// removes the constraint. Here every caller that switches softening off
    /// wants the *hard* constraint instead, so the neutral element is used: a
    /// `mass_scale` of one and no bias leaves the plain impulse.
    pub const RIGID: Self = Self {
        bias_rate: 0.0,
        mass_scale: 1.0,
        impulse_scale: 0.0,
    };

    /// The coefficients of a spring at `hertz` and `damping_ratio`, solved at a
    /// step of `h` seconds.
    ///
    /// A non-positive frequency or step gives [`Softness::RIGID`]: both would
    /// divide by zero below, and both mean "do not soften this".
    pub fn new(hertz: f32, damping_ratio: f32, h: f32) -> Self {
        if hertz <= 0.0 || h <= 0.0 {
            return Self::RIGID;
        }

        let omega = 2.0 * PI * hertz;
        let a1 = 2.0 * damping_ratio.max(0.0) + h * omega;
        let a2 = h * omega * a1;
        let a3 = 1.0 / (1.0 + a2);

        Self {
            bias_rate: omega / a1,
            mass_scale: a2 * a3,
            impulse_scale: a3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: f32 = 1.0 / 240.0;

    #[test]
    fn a_stiffer_spring_biases_harder() {
        let soft = Softness::new(30.0, 10.0, H);
        let stiff = Softness::new(60.0, 10.0, H);

        assert!(stiff.bias_rate > soft.bias_rate);
        assert!(soft.bias_rate.is_finite() && soft.mass_scale.is_finite());
    }

    #[test]
    fn the_scales_stay_inside_the_unit_interval() {
        // mass_scale = a2·a3 and impulse_scale = a3 with a3 = 1/(1 + a2) and
        // a2 >= 0, so both are bounded. A mass_scale above one would amplify
        // every impulse the solver applies.
        let softness = Softness::new(30.0, 10.0, H);

        assert!((0.0..=1.0).contains(&softness.mass_scale), "{softness:?}");
        assert!(
            (0.0..=1.0).contains(&softness.impulse_scale),
            "{softness:?}"
        );
    }

    #[test]
    fn a_non_positive_hertz_or_step_is_a_rigid_constraint() {
        assert_eq!(Softness::new(0.0, 10.0, H), Softness::RIGID);
        assert_eq!(Softness::new(-30.0, 10.0, H), Softness::RIGID);
        assert_eq!(Softness::new(30.0, 10.0, 0.0), Softness::RIGID);
    }

    #[test]
    fn a_negative_damping_ratio_does_not_produce_a_negative_bias() {
        // A scene can hold any number, and a negative damping ratio would make
        // a1 negative, flipping the sign of every push.
        let softness = Softness::new(30.0, -10.0, H);

        assert!(softness.bias_rate > 0.0, "{softness:?}");
        assert!(softness.mass_scale >= 0.0, "{softness:?}");
    }

    #[test]
    fn more_damping_softens_the_push() {
        let ringing = Softness::new(30.0, 1.0, H);
        let damped = Softness::new(30.0, 10.0, H);

        assert!(damped.bias_rate < ringing.bias_rate);
    }
}
