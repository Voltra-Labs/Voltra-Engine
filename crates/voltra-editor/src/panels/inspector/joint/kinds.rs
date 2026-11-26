//! The rows of each joint kind's own parameters.

use voltra_core::egui::{RichText, Ui};
use voltra_scene::{JointKind, Motor, RestSpring};

use super::rows::{axis, degrees, number, toggle};

/// The rows of `kind`'s own parameters, in the joint's grid.
pub(super) fn show(ui: &mut Ui, kind: &mut JointKind) -> Option<&'static str> {
    let mut claim = None;
    match kind {
        JointKind::Revolute {
            reference_angle,
            limit,
            motor,
            spring,
        } => {
            claim = claim.or(degrees(ui, "reference", reference_angle));
            claim = claim.or(toggle(
                ui,
                "limit",
                "how far a may turn against b",
                "joint_limit",
                limit,
            ));
            if let Some(limit) = limit {
                claim = claim.or(degrees(ui, "lower", &mut limit.lower));
                claim = claim.or(degrees(ui, "upper", &mut limit.upper));
            }
            claim = claim.or(turning_motor(ui, motor));
            claim = claim.or(toggle(
                ui,
                "spring",
                "turns a back towards a rest angle, from the reference",
                "joint_rest_spring",
                spring,
            ));
            if let Some(spring) = spring {
                claim = claim.or(rest_spring(ui, spring, true));
            }
        }
        JointKind::Distance {
            length,
            min,
            max,
            spring,
        } => {
            claim = claim.or(number(ui, "length", length, 0.0..=1e4));
            claim = claim.or(number(ui, "min", min, 0.0..=1e4));
            claim = claim.or(number(ui, "max", max, 0.0..=1e4));
            if *min >= *max {
                ui.label("");
                ui.label(RichText::new("held at its length").weak());
                ui.end_row();
            }
            claim = claim.or(toggle(
                ui,
                "spring",
                "pulls towards the length instead of holding it; needs min < max",
                "joint_spring",
                spring,
            ));
            if let Some(spring) = spring {
                claim = claim.or(number(ui, "hertz", &mut spring.hertz, 0.0..=60.0));
                claim = claim.or(number(ui, "damping", &mut spring.damping_ratio, 0.0..=10.0));
            }
        }
        JointKind::Weld { reference_angle } => {
            claim = claim.or(degrees(ui, "reference", reference_angle));
        }
        JointKind::Wheel {
            axis: line,
            spring,
            limit,
            motor,
        } => {
            claim = claim.or(axis(
                ui,
                "axis",
                "the line the suspension travels along, in b's space",
                line,
            ));
            claim = claim.or(toggle(
                ui,
                "suspension",
                "springs the wheel along the axis; off, it slides freely",
                "joint_rest_spring",
                spring,
            ));
            if let Some(spring) = spring {
                claim = claim.or(rest_spring(ui, spring, false));
            }
            claim = claim.or(slide_limit(ui, limit));
            claim = claim.or(turning_motor(ui, motor));
        }
        JointKind::Prismatic {
            axis: line,
            reference_angle,
            limit,
            motor,
            spring,
        } => {
            claim = claim.or(axis(
                ui,
                "axis",
                "the line a slides along, in b's space",
                line,
            ));
            claim = claim.or(degrees(ui, "reference", reference_angle));
            claim = claim.or(slide_limit(ui, limit));
            claim = claim.or(toggle(
                ui,
                "motor",
                "slides a along the axis at a speed",
                "joint_linear_motor",
                motor,
            ));
            if let Some(motor) = motor {
                claim = claim.or(number(ui, "speed /s", &mut motor.speed, -1e4..=1e4));
                claim = claim.or(number(ui, "max force", &mut motor.max_force, 0.0..=1e7));
            }
            claim = claim.or(toggle(
                ui,
                "spring",
                "pulls a along the axis towards a rest",
                "joint_rest_spring",
                spring,
            ));
            if let Some(spring) = spring {
                claim = claim.or(rest_spring(ui, spring, false));
            }
        }
        JointKind::Mouse {
            max_force,
            hertz,
            damping_ratio,
        } => {
            ui.label("");
            ui.label(RichText::new("pulls a's anchor to b's — the target").weak());
            ui.end_row();
            claim = claim.or(number(ui, "max force", max_force, 0.0..=1e7));
            claim = claim.or(number(ui, "hertz", hertz, 0.0..=60.0));
            claim = claim.or(number(ui, "damping", damping_ratio, 0.0..=10.0));
        }
    }
    claim
}

/// A turning motor's rows: a revolute's or a wheel's.
fn turning_motor(ui: &mut Ui, motor: &mut Option<Motor>) -> Option<&'static str> {
    let mut claim = toggle(
        ui,
        "motor",
        "turns a against b at a speed",
        "joint_motor",
        motor,
    );
    if let Some(motor) = motor {
        claim = claim.or(degrees(ui, "speed /s", &mut motor.speed));
        claim = claim.or(number(ui, "max torque", &mut motor.max_torque, 0.0..=1e6));
    }
    claim
}

/// How far a wheel or a slider may travel along its axis.
fn slide_limit(
    ui: &mut Ui,
    limit: &mut Option<voltra_scene::TranslationLimit>,
) -> Option<&'static str> {
    let mut claim = toggle(
        ui,
        "limit",
        "how far a may travel along the axis",
        "joint_slide_limit",
        limit,
    );
    if let Some(limit) = limit {
        claim = claim.or(number(ui, "lower", &mut limit.lower, -1e4..=1e4));
        claim = claim.or(number(ui, "upper", &mut limit.upper, -1e4..=1e4));
    }
    claim
}

/// A spring's rows, its rest an angle (`turning`) or a distance.
fn rest_spring(ui: &mut Ui, spring: &mut RestSpring, turning: bool) -> Option<&'static str> {
    let mut claim = number(ui, "hertz", &mut spring.hertz, 0.0..=60.0);
    claim = claim.or(number(ui, "damping", &mut spring.damping_ratio, 0.0..=10.0));
    if turning {
        claim.or(degrees(ui, "rest", &mut spring.rest))
    } else {
        claim.or(number(ui, "rest", &mut spring.rest, -1e4..=1e4))
    }
}
