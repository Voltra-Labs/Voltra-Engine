//! A skeleton's root, a bone and a limb's IK.

use voltra_core::egui::{DragValue, RichText, TextEdit, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::skeleton::ik::{problem, IkProblem};
use voltra_scene::skeleton::{bones_of, reset_to_rest};
use voltra_scene::{Bone2D, Skeleton2D, Transform, TwoBoneIk};

use super::active;

/// What the bone section asked for that needs the editor: a child bone at
/// its tip, spawned once the world is no longer borrowed, and the bone's
/// rest set from its pose — once the timeline has put the scene back as
/// authored, since a rest copies the transform and no clip keys a rest.
#[derive(Debug, Default)]
pub(super) struct Asked {
    pub claim: Option<&'static str>,
    pub child_bone: bool,
    pub set_rest: bool,
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
                reset_to_rest(world, entity);
                asked.claim = Some("Reset skeleton pose");
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
        let mut reset = false;
        ui.horizontal(|ui| {
            if ui
                .button("Set rest")
                .on_hover_text("this pose is the one a skinned mesh is bound in")
                .clicked()
            {
                asked.set_rest = true;
            }
            reset = ui.button("Reset to rest").clicked();
            asked.child_bone = ui
                .button("Add child bone")
                .on_hover_text("a bone hung at this one's tip")
                .clicked();
        });
        if reset {
            let rest = bone.rest;
            if let Some(transform) = world.get_mut::<Transform>(entity) {
                *transform = rest;
                asked.claim = Some("Reset bone pose");
            }
        }
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
