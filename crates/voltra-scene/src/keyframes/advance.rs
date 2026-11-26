//! A frame of every player: clocks run, poses sampled, mixed and written.

use voltra_ecs::World;

use super::blend::blend;
use super::fade::Outgoing;
use super::frame::{step, Source};
use super::player::AnimationPlayer;
use super::store::AnimationClips;

/// Runs every playing [`AnimationPlayer`] `delta` seconds and writes its
/// clip's values — or, while it fades, the mix of its clip and what it
/// fades out, each property once.
///
/// Per frame, as `SpriteAnimation` is: what is animated is what is drawn.
/// A player applies nothing while it is not playing, so an entity whose clip
/// has ended — or was paused — can be moved by other code without the clip
/// putting it back. A dynamic body's transform is the simulation's: the
/// step after this overwrites it, so animate a kinematic body, or a child.
pub fn advance(world: &mut World, clips: &mut AnimationClips, delta: f32) {
    let mut due = Vec::new();
    for (entity, player) in world.query_mut::<AnimationPlayer>() {
        if let Some(frame) = step(player, clips, delta) {
            due.push((entity, frame));
        }
    }

    let clips = &*clips;
    for (entity, frame) in due {
        let incoming = frame
            .clip
            .map(|(handle, time)| clips.get(handle).sample(time))
            .unwrap_or_default();
        let faded = frame.fade.map(|(from, weight)| {
            let (outgoing, frozen) = match from {
                Source::Clip(handle, time) => (clips.get(handle).sample(time), false),
                Source::Pose(pose) => (pose, true),
            };
            let mixed = blend(&[(&outgoing, 1.0 - weight), (&incoming, weight)]);
            (mixed, outgoing, frozen)
        });
        let pose = faded.as_ref().map_or(&incoming, |(mixed, ..)| mixed);
        // A miss is the clip faded in's when it animates that property, and
        // otherwise the side faded out's — each warned of under its own name.
        let (mut missed, mut missed_out) = (Vec::new(), Vec::new());
        for sample in pose.apply(world, entity) {
            let said = format!("{:?} on {:?}", sample.property, sample.target);
            let theirs = faded.is_some() && incoming.get(&sample.target, sample.property).is_none();
            if theirs {
                missed_out.push(said);
            } else {
                missed.push(said);
            }
        }
        let Some(player) = world.get_mut::<AnimationPlayer>(entity) else {
            continue;
        };
        let clip = player.clip.as_ref().map_or("", |p| p.as_str()).to_owned();
        if let (Some(fade), Some((mixed, outgoing, frozen))) = (player.fade.as_mut(), faded) {
            fade.last = Some(mixed.into_owned());
            if frozen {
                fade.from = Outgoing::Pose(outgoing.into_owned());
            }
            if !missed_out.is_empty() && !fade.reported {
                fade.reported = true;
                let from = match &fade.from {
                    Outgoing::Clip(out) => format!("animation {}", out.clip.as_str()),
                    Outgoing::Pose(_) => "an interrupted fade".to_owned(),
                };
                log::warn!(
                    "{from}, fading into {clip}: nothing to animate for {}",
                    missed_out.join(", ")
                );
            }
        }
        if missed.is_empty() || player.reported == player.clip {
            continue;
        }
        player.reported = player.clip.clone();
        log::warn!(
            "animation {clip}: nothing to animate for {}",
            missed.join(", ")
        );
    }
}
