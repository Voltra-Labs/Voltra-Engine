//! A skeleton's root, a bone and a limb's IK.

use voltra_core::egui::{DragValue, RichText, TextEdit, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::skeleton::ik::{problem, IkProblem};
use voltra_scene::skeleton::{bones_of, reset_to_rest};
use voltra_scene::{Bone2D, Skeleton2D, Transform2D, TwoBoneIk};

use super::active;

/// What the bone section asked for that needs the editor: a child bone at
/// its tip, spawned once the world is no longer borrowed, and the bone's
/// rest set from its pose — once the timeline has put the scene back as
/// authored, since a rest copies the transform and no clip keys a rest.
/// The resets too: each is done by the caller to every selected entity
/// from its own rest, where copying the primary's pose would put them all
/// in one place.
#[derive(Debug, Default)]
pub(super) struct Asked {
    pub claim: Option<&'static str>,
    pub child_bone: bool,
    pub set_rest: bool,
    /// "Reset pose" on a skeleton: its bones back to their rests.
    pub reset_pose: bool,
    /// "Reset to rest" on a bone: its transform back to its rest.
    pub reset_bone: bool,
}

pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Asked {
    let mut asked = Asked::default();
    if world.get::<Skeleton2D>(entity).is_some() {
        ui.separator();
        ui.label(RichText::new("Skeleton").strong());
        let bones = bones_of(world, entity).len();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{bones} bones")).weak());
            if ui
                .button("Reset pose")
                .on_hover_text("every bone back to its rest pose")
                .clicked()
            {
                asked.reset_pose = true;
            }
        });
    }
    if let Some(bone) = world.get_mut::<Bone2D>(entity) {
        ui.separator();
        ui.label(RichText::new("Bone").strong());
        ui.horizontal(|ui| {
            ui.label("length");
            asked.claim = asked.claim.or(active(
                &ui.add(
                    DragValue::new(&mut bone.length)
                        .speed(0.01)
                        .range(0.0..=100.0),
                ),
                "Edit bone",
            ));
        });
        ui.horizontal(|ui| {
            if ui
                .button("Set rest")
                .on_hover_text("this pose is the one a skinned mesh is bound in")
                .clicked()
            {
                asked.set_rest = true;
            }
            asked.reset_bone = ui.button("Reset to rest").clicked();
            asked.child_bone = ui
                .button("Add child bone")
                .on_hover_text("a bone hung at this one's tip")
                .clicked();
        });
    }
    if let Some(ik) = world.get_mut::<TwoBoneIk>(entity) {
        ui.separator();
        ui.label(RichText::new("Two-bone IK").strong())
            .on_hover_text("bends this bone and its parent so this one's tip reaches the target");
        ui.horizontal(|ui| {
            ui.label("target");
            let response = ui.add(TextEdit::singleline(&mut ik.target).hint_text("name/path"));
            asked.claim = asked.claim.or(active(&response, "Edit IK"));
        });
        ui.horizontal(|ui| {
            if ui.checkbox(&mut ik.enabled, "on").changed() {
                asked.claim = Some("Edit IK");
            }
            if ui.checkbox(&mut ik.flip, "bend the other way").changed() {
                asked.claim = Some("Edit IK");
            }
        });
        if let Some(problem) = problem(world, entity) {
            let why = match problem {
                IkProblem::NotABone => "this entity is not a bone",
                IkProblem::NoUpperBone => "its parent is not a bone",
                IkProblem::NoSkeleton => "it is under no skeleton",
                IkProblem::NoTarget => "no entity has that path under the skeleton",
            };
            ui.label(RichText::new(format!("not solving: {why}")).weak());
        }
    }
    asked
}

/// Puts `entity`'s skeleton's bones back to their rests; whether it is one.
pub(super) fn reset_pose(world: &mut World, entity: Entity) -> bool {
    if world.get::<Skeleton2D>(entity).is_none() {
        return false;
    }
    reset_to_rest(world, entity);
    true
}

/// Puts `entity`'s transform back to its bone's rest; whether it is a bone.
pub(super) fn reset_bone(world: &mut World, entity: Entity) -> bool {
    let Some(rest) = world.get::<Bone2D>(entity).map(|bone| bone.rest) else {
        return false;
    };
    match world.get_mut::<Transform2D>(entity) {
        Some(transform) => {
            *transform = rest;
            true
        }
        None => false,
    }
}
