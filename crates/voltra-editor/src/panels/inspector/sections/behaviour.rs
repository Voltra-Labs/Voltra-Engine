//! The sections of how an entity moves, lights, frames and sounds: its
//! skin and bones, parallax, light, joint, animation, camera, sound and
//! replication.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::{AudioSource, Camera, Parallax, Replicated, Transform};

use super::super::mixed;
use super::super::{
    animation_player, animator, audio_source, camera, camera_rig, camera_view, global_light, joint,
    light, parallax, replicated, skeleton, skinned,
};
use super::gate;
use crate::editor::Editor;
use crate::multi_edit::MultiEdit;

/// Every section of these `entity` has, top to bottom; what to call the
/// edit one of them is in the middle of, if any.
pub(in crate::panels::inspector) fn show(
    editor: &mut Editor,
    ui: &mut Ui,
    frame: &mut UiFrame<'_>,
    entity: Entity,
    multi: &mut MultiEdit,
) -> Option<&'static str> {
    let mut claim = None;
    if multi.editable(&["SkinnedSprite"]) {
        mixed::marker(ui, multi, &["SkinnedSprite"]);
        let (edited, action) = skinned::show(
            ui,
            frame.world,
            skinned::Context {
                entity,
                paint: &mut editor.weight_paint,
                textures: frame.textures,
                silhouettes: frame.silhouettes.as_deref_mut(),
                device: frame.device,
                queue: frame.queue,
            },
        );
        claim = claim.or(edited);
        if let Some(action) = action {
            // Each from its own texture and bones.
            for each in multi.all().collect::<Vec<_>>() {
                let silhouettes = frame.silhouettes.as_deref_mut();
                if skinned::apply(action, frame.world, each, frame.textures, silhouettes) {
                    claim = Some(action.label());
                }
            }
            multi.done_for_each("SkinnedSprite");
        }
    }
    let bones = ["Skeleton2D", "Bone2D", "TwoBoneIk"];
    let asked = if multi.editable(&bones) {
        mixed::marker(ui, multi, &bones);
        skeleton::show(ui, frame.world, entity)
    } else {
        skeleton::Asked::default()
    };
    claim = claim.or(asked.claim);
    if asked.set_rest {
        // A rest copies the transform: the authored one, never
        // a recorded pose.
        editor
            .timeline
            .end_record(frame.world, frame.animation_clips);
        // Each bone's from its own transform, not the primary's copied.
        let bones: Vec<Entity> = multi.all().collect();
        for bone in bones {
            let pose = frame.world.get::<Transform>(bone).copied();
            if let (Some(pose), Some(bone)) =
                (pose, frame.world.get_mut::<voltra_scene::Bone2D>(bone))
            {
                bone.rest = pose;
                claim = Some("Set bone rest pose");
            }
        }
        multi.done_for_each("Bone2D");
    }
    if asked.reset_pose || asked.reset_bone {
        // Each from its own rest: the bones of every selected skeleton,
        // every selected bone's own transform.
        for each in multi.all().collect::<Vec<_>>() {
            if asked.reset_pose && skeleton::reset_pose(frame.world, each) {
                claim = Some("Reset skeleton pose");
            }
            if asked.reset_bone && skeleton::reset_bone(frame.world, each) {
                claim = Some("Reset bone pose");
            }
        }
        multi.done_for_each("Transform");
    }
    if asked.child_bone {
        crate::spawn::record(editor, frame, "Add child bone", |frame| {
            crate::spawn::child_bone(frame, entity)
        });
    }
    if let Some(layer) = gate(frame.world.get_mut::<Parallax>(entity), multi, "Parallax") {
        ui.separator();
        mixed::marker(ui, multi, &["Parallax"]);
        claim = claim.or(parallax::show(ui, layer));
    }
    if let Some(lamp) = gate(
        frame.world.get_mut::<voltra_scene::PointLight2D>(entity),
        multi,
        "PointLight2D",
    ) {
        ui.separator();
        mixed::marker(ui, multi, &["PointLight2D"]);
        claim = claim.or(light::show(
            ui,
            lamp,
            frame.textures,
            frame.device,
            frame.queue,
        ));
    }
    if frame.world.get::<voltra_scene::Joint2D>(entity).is_some() && multi.editable(&["Joint2D"]) {
        let ends = joint::ends(frame.world, entity);
        if let Some(j) = frame.world.get_mut::<voltra_scene::Joint2D>(entity) {
            ui.separator();
            mixed::marker(ui, multi, &["Joint2D"]);
            claim = claim.or(joint::show(ui, j, &ends));
        }
    }
    if multi.editable(&["AnimationPlayer"]) {
        mixed::marker(ui, multi, &["AnimationPlayer"]);
        claim = claim.or(animation_player::show(ui, frame.world, entity));
    }
    if multi.editable(&["Animator", "AnimationPlayer"]) {
        mixed::marker(ui, multi, &["Animator"]);
        claim = claim.or(animator::show(
            ui,
            frame.world,
            frame.state_machines,
            entity,
        ));
    }
    if let Some(glow) = gate(
        frame.world.get_mut::<voltra_scene::GlobalLight2D>(entity),
        multi,
        "GlobalLight2D",
    ) {
        ui.separator();
        mixed::marker(ui, multi, &["GlobalLight2D"]);
        claim = claim.or(global_light::show(ui, glow));
    }
    if let Some(camera) = gate(frame.world.get_mut::<Camera>(entity), multi, "Camera") {
        ui.separator();
        mixed::marker(ui, multi, &["Camera"]);
        claim = claim.or(camera::show(ui, camera));
    }
    if frame.world.get::<Camera>(entity).is_some() && multi.editable(&["Camera", "CameraTarget"]) {
        mixed::marker(ui, multi, &["CameraTarget"]);
        claim = claim.or(camera_view::show(ui, frame.world, entity));
    }
    let rig = ["CameraFollow2D", "CameraShake"];
    if multi.editable(&rig) {
        mixed::marker(ui, multi, &rig);
        claim = claim.or(camera_rig::show(ui, frame.world, entity));
    }
    if let Some(source) = gate(
        frame.world.get_mut::<AudioSource>(entity),
        multi,
        "AudioSource",
    ) {
        ui.separator();
        mixed::marker(ui, multi, &["AudioSource"]);
        claim = claim.or(audio_source::show(
            ui,
            source,
            frame.clips,
            frame.audio,
            &mut editor.audition,
        ));
    }
    if let Some(sent) = gate(
        frame.world.get_mut::<Replicated>(entity),
        multi,
        Replicated::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[Replicated::NAME]);
        claim = claim.or(replicated::show(ui, sent));
    }
    claim
}
