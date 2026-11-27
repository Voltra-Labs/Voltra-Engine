//! The sections of how an entity moves, lights, frames and sounds: its
//! skin and bones, parallax, light, joint, animation, camera, sound and
//! replication (with lag compensation).

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::{AudioSource, Camera, LagCompensated, Parallax, Replicated, Transform};

use super::super::mixed;
use super::super::{
    animation_player, animator, audio_source, camera, camera_rig, camera_view, global_light, joint,
    lag_compensated, light, parallax, replicated, skeleton, skinned,
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
    if multi.editable(&[voltra_scene::SkinnedSprite::NAME]) {
        mixed::marker(ui, multi, &[voltra_scene::SkinnedSprite::NAME]);
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
            multi.done_for_each(voltra_scene::SkinnedSprite::NAME);
        }
    }
    let bones = [
        voltra_scene::Skeleton2D::NAME,
        voltra_scene::Bone2D::NAME,
        voltra_scene::TwoBoneIk::NAME,
    ];
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
        multi.done_for_each(voltra_scene::Bone2D::NAME);
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
        multi.done_for_each(Transform::NAME);
    }
    if asked.child_bone {
        crate::spawn::record(editor, frame, "Add child bone", |frame| {
            crate::spawn::child_bone(frame, entity)
        });
    }
    if let Some(layer) = gate(
        frame.world.get_mut::<Parallax>(entity),
        multi,
        Parallax::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[Parallax::NAME]);
        claim = claim.or(parallax::show(ui, layer));
    }
    if let Some(lamp) = gate(
        frame.world.get_mut::<voltra_scene::PointLight2D>(entity),
        multi,
        voltra_scene::PointLight2D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[voltra_scene::PointLight2D::NAME]);
        claim = claim.or(light::show(
            ui,
            lamp,
            frame.textures,
            frame.device,
            frame.queue,
        ));
    }
    if frame.world.get::<voltra_scene::Joint2D>(entity).is_some()
        && multi.editable(&[voltra_scene::Joint2D::NAME])
    {
        let ends = joint::ends(frame.world, entity);
        if let Some(j) = frame.world.get_mut::<voltra_scene::Joint2D>(entity) {
            ui.separator();
            mixed::marker(ui, multi, &[voltra_scene::Joint2D::NAME]);
            claim = claim.or(joint::show(ui, j, &ends));
        }
    }
    if multi.editable(&[voltra_scene::AnimationPlayer::NAME]) {
        mixed::marker(ui, multi, &[voltra_scene::AnimationPlayer::NAME]);
        claim = claim.or(animation_player::show(ui, frame.world, entity));
    }
    if multi.editable(&[
        voltra_scene::Animator::NAME,
        voltra_scene::AnimationPlayer::NAME,
    ]) {
        mixed::marker(ui, multi, &[voltra_scene::Animator::NAME]);
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
        voltra_scene::GlobalLight2D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[voltra_scene::GlobalLight2D::NAME]);
        claim = claim.or(global_light::show(ui, glow));
    }
    if let Some(camera) = gate(frame.world.get_mut::<Camera>(entity), multi, Camera::NAME) {
        ui.separator();
        mixed::marker(ui, multi, &[Camera::NAME]);
        claim = claim.or(camera::show(ui, camera));
    }
    if frame.world.get::<Camera>(entity).is_some()
        && multi.editable(&[Camera::NAME, voltra_scene::CameraTarget::NAME])
    {
        mixed::marker(ui, multi, &[voltra_scene::CameraTarget::NAME]);
        claim = claim.or(camera_view::show(ui, frame.world, entity));
    }
    let rig = [
        voltra_scene::CameraFollow2D::NAME,
        voltra_scene::CameraShake::NAME,
    ];
    if multi.editable(&rig) {
        mixed::marker(ui, multi, &rig);
        claim = claim.or(camera_rig::show(ui, frame.world, entity));
    }
    if let Some(source) = gate(
        frame.world.get_mut::<AudioSource>(entity),
        multi,
        AudioSource::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[AudioSource::NAME]);
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
    // Under replication, where a networked entity's settings are: and shown
    // on one marked without being replicated, so the mark is never hidden.
    let networked = frame.world.get::<Replicated>(entity).is_some()
        || frame.world.get::<LagCompensated>(entity).is_some();
    if networked && multi.editable(&[LagCompensated::NAME]) {
        mixed::marker(ui, multi, &[LagCompensated::NAME]);
        claim = claim.or(lag_compensated::show(ui, frame.world, entity));
    }
    claim
}
