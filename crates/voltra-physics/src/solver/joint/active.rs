//! The impulses a joint starts a step from: only those of the parts it has.
//!
//! A joint's impulses are warm started every sub-step and stored back after
//! it, so a part nothing solves any more — a motor switched off, a spring
//! taken out, a limit removed, a kind changed — would push with its last
//! impulse for ever: nothing else ever decays it. Box2D's `Enable*` setters
//! reset a part's impulse for this; here the component is edited directly,
//! by a game or the inspector, so the step resets it instead, every time it
//! loads one. A motor's or a mouse's impulse is also kept within what it
//! may now push with: a cap lowered at run time is felt from the next step.

use voltra_render::glam::Vec2;
use voltra_scene::{JointImpulses, JointKind, RestSpring};

/// `impulses` with every part of `kind` that is absent or does nothing
/// zeroed, and every capped part within its cap over a sub-step `h` long.
pub(super) fn active(kind: &JointKind, impulses: &JointImpulses, h: f32) -> JointImpulses {
    let i = impulses;
    let mut kept = JointImpulses::default();
    let springs = |spring: Option<RestSpring>| spring.is_some_and(|s| s.hertz > 0.0);
    // A cap that is not a number holds nothing.
    let cap = |most: f32| (h * most.max(0.0)).max(0.0);
    let within = |impulse: f32, most: f32| impulse.clamp(-cap(most), cap(most));
    match *kind {
        JointKind::Revolute {
            limit,
            motor,
            spring,
            ..
        } => {
            kept.linear = i.linear;
            if springs(spring) {
                kept.spring = i.spring;
            }
            if let Some(motor) = motor {
                kept.motor = within(i.motor, motor.max_torque);
            }
            if limit.is_some() {
                (kept.lower, kept.upper) = (i.lower, i.upper);
            }
        }
        JointKind::Weld { .. } => {
            kept.linear = i.linear;
            kept.angular = i.angular;
        }
        JointKind::Distance {
            min, max, spring, ..
        } => {
            if min < max {
                if spring.is_some_and(|s| s.hertz > 0.0) {
                    kept.axial = i.axial;
                }
                (kept.lower, kept.upper) = (i.lower, i.upper);
            } else {
                kept.axial = i.axial;
            }
        }
        JointKind::Wheel {
            spring,
            limit,
            motor,
            ..
        } => {
            kept.perp = i.perp;
            if springs(spring) {
                kept.spring = i.spring;
            }
            if let Some(motor) = motor {
                kept.motor = within(i.motor, motor.max_torque);
            }
            if limit.is_some() {
                (kept.lower, kept.upper) = (i.lower, i.upper);
            }
        }
        JointKind::Prismatic {
            limit,
            motor,
            spring,
            ..
        } => {
            kept.perp = i.perp;
            kept.angular = i.angular;
            if springs(spring) {
                kept.spring = i.spring;
            }
            if let Some(motor) = motor {
                kept.motor = within(i.motor, motor.max_force);
            }
            if limit.is_some() {
                (kept.lower, kept.upper) = (i.lower, i.upper);
            }
        }
        JointKind::Mouse { max_force, .. } => {
            let most = cap(max_force);
            kept.linear = if i.linear.length() > most {
                i.linear.normalize_or_zero() * most
            } else {
                i.linear
            };
        }
    }
    // Whatever is kept is a number: one that is not would spread to the
    // bodies on the next warm start.
    let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
    JointImpulses {
        linear: Vec2::new(finite(kept.linear.x), finite(kept.linear.y)),
        angular: finite(kept.angular),
        axial: finite(kept.axial),
        perp: finite(kept.perp),
        spring: finite(kept.spring),
        motor: finite(kept.motor),
        lower: finite(kept.lower),
        upper: finite(kept.upper),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::{Motor, TranslationLimit};

    fn every(value: f32) -> JointImpulses {
        JointImpulses {
            linear: Vec2::splat(value),
            angular: value,
            axial: value,
            perp: value,
            spring: value,
            motor: value,
            lower: value,
            upper: value,
        }
    }

    #[test]
    fn a_wheel_keeps_only_the_parts_it_has() {
        let bare = JointKind::Wheel {
            axis: Vec2::Y,
            spring: None,
            limit: None,
            motor: None,
        };
        let kept = active(&bare, &every(1.0), 0.01);
        assert_eq!(
            kept,
            JointImpulses {
                perp: 1.0,
                ..JointImpulses::default()
            }
        );
        let full = JointKind::Wheel {
            axis: Vec2::Y,
            spring: Some(RestSpring::default()),
            limit: Some(TranslationLimit::default()),
            motor: Some(Motor {
                speed: 1.0,
                max_torque: 50.0,
            }),
        };
        let kept = active(&full, &every(1.0), 0.01);
        assert_eq!(kept.motor, 0.5, "capped at the torque over the sub-step");
        assert_eq!((kept.spring, kept.lower, kept.upper), (1.0, 1.0, 1.0));
        assert_eq!((kept.angular, kept.axial), (0.0, 0.0), "not a wheel's");
    }

    #[test]
    fn a_spring_with_no_frequency_and_a_mouse_past_its_force_are_reset() {
        let slack = JointKind::Revolute {
            reference_angle: 0.0,
            limit: None,
            motor: None,
            spring: Some(RestSpring {
                hertz: 0.0,
                ..RestSpring::default()
            }),
        };
        assert_eq!(active(&slack, &every(1.0), 0.01).spring, 0.0);
        let mouse = JointKind::Mouse {
            max_force: 10.0,
            hertz: 5.0,
            damping_ratio: 0.7,
        };
        let kept = active(&mouse, &every(1.0), 0.01);
        assert!((kept.linear.length() - 0.1).abs() < 1e-6, "{}", kept.linear);
        let broken = active(&JointKind::default(), &every(f32::NAN), 0.01);
        assert_eq!(broken, JointImpulses::default());
    }
}
