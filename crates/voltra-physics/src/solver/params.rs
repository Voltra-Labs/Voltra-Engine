//! What the solver is allowed to be tuned by.

use super::softness::Softness;

/// The solver's tuning, all of it.
///
/// Every value here is a field rather than a constant: a scene that stacks
/// crates and a scene that fires projectiles want different numbers, and the
/// engine needs the parameter either way. The defaults are Box2D v3's, from
/// `b2DefaultWorldDef`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolverParams {
    /// Sub-steps per fixed step. TGS spends its budget here rather than on
    /// iterations: four short steps beat four passes over one long one.
    pub sub_steps: u32,
    /// Natural frequency of a contact, before the Nyquist cap in
    /// [`SolverParams::softness`].
    pub contact_hertz: f32,
    /// Damping ratio of a contact. Well above one: a contact should absorb,
    /// not ring.
    pub contact_damping_ratio: f32,
    /// Ceiling on how fast overlap is pushed out, in world units per second.
    /// Without it, a deep overlap — a body spawned inside a wall — resolves by
    /// launching it across the scene.
    pub max_push_speed: f32,
    /// Approach speed below which restitution is discarded. This is what stops
    /// a resting body from bouncing forever on its own numerical noise.
    pub restitution_threshold: f32,
    /// Whether to seed each contact with the previous step's impulses.
    ///
    /// A switch rather than an assumption, because turning it off is how its
    /// effect is demonstrated: a stack that settles becomes a stack that sinks.
    pub warm_starting: bool,
}

impl Default for SolverParams {
    fn default() -> Self {
        Self {
            sub_steps: 4,
            contact_hertz: 30.0,
            contact_damping_ratio: 10.0,
            max_push_speed: 3.0,
            restitution_threshold: 1.0,
            warm_starting: true,
        }
    }
}

impl SolverParams {
    /// The length of one sub-step of a fixed step of `dt`.
    ///
    /// Zero sub-steps would divide by zero and mean "simulate nothing", which
    /// no caller wants; it reads as one.
    pub fn sub_step(&self, dt: f32) -> f32 {
        dt / self.sub_steps.max(1) as f32
    }

    /// The two springs a step uses, as `(dynamic, against_immovable)`.
    ///
    /// Contacts against something that cannot be pushed are stiffer — twice the
    /// frequency, as Box2D and Avian both do — because all of the correction
    /// has to come from the one body that can move, and a soft ground is a
    /// ground bodies sink into.
    ///
    /// The frequency is capped at `0.125 / h`. A spring faster than the rate it
    /// is sampled at is noise, not stiffness (Nyquist), so a long step lowers
    /// the frequency rather than ringing at it.
    pub fn softness(&self, h: f32) -> (Softness, Softness) {
        let hertz = if h > 0.0 {
            self.contact_hertz.min(0.125 / h)
        } else {
            0.0
        };

        (
            Softness::new(hertz, self.contact_damping_ratio, h),
            Softness::new(2.0 * hertz, self.contact_damping_ratio, h),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn contacts_against_immovable_bodies_are_stiffer() {
        let params = SolverParams::default();

        let (dynamic, immovable) = params.softness(params.sub_step(DT));

        assert!(immovable.bias_rate > dynamic.bias_rate);
    }

    #[test]
    fn the_hertz_is_capped_well_under_the_sub_step_rate() {
        let params = SolverParams::default();
        let h = 1.0 / 10.0;

        let (dynamic, _) = params.softness(h);

        let capped = Softness::new(0.125 / h, params.contact_damping_ratio, h);
        assert!((dynamic.bias_rate - capped.bias_rate).abs() < 1e-6);
    }

    #[test]
    fn the_cap_leaves_an_ordinary_step_alone() {
        // Four sub-steps of a 60 Hz step is h = 1/240, so 0.125/h is 30 — the
        // default frequency exactly, which is where the default came from.
        let params = SolverParams::default();
        let h = params.sub_step(DT);

        let (dynamic, _) = params.softness(h);

        let uncapped = Softness::new(params.contact_hertz, params.contact_damping_ratio, h);
        assert!((dynamic.bias_rate - uncapped.bias_rate).abs() < 1e-6);
    }

    #[test]
    fn zero_sub_steps_still_produces_a_usable_step() {
        let params = SolverParams {
            sub_steps: 0,
            ..Default::default()
        };

        assert!(params.sub_step(DT) > 0.0);
    }

    #[test]
    fn a_zero_step_gives_rigid_springs_rather_than_infinities() {
        let (dynamic, immovable) = SolverParams::default().softness(0.0);

        assert_eq!(dynamic, Softness::RIGID);
        assert_eq!(immovable, Softness::RIGID);
    }
}
