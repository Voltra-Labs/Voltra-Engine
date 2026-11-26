//! A frame of every player: clocks run, poses sampled, mixed and written.

use voltra_ecs::{Entity, World};

use super::frame::{step, Frame};
use super::layered;
use super::player::AnimationPlayer;
use super::report::report;
use super::sample::{keep, sample_frame, Buffers};
use super::store::AnimationClips;
use super::synced::Pick;
use super::target::Children;

/// Runs every playing [`AnimationPlayer`] `delta` seconds and writes its
/// clip's values — or its synced clips' mix, or, while it fades, the mix of
/// what it plays and what it fades out — each property once, its layers
/// composited over it.
///
/// Per frame, as `SpriteAnimation` is: what is animated is what is drawn.
/// A player applies nothing while it is not playing, so an entity whose clip
/// has ended — or was paused — can be moved by other code without the clip
/// putting it back. A dynamic body's transform is the simulation's: the
/// step after this overwrites it, so animate a kinematic body, or a child.
///
/// A frame samples into a few buffers reused across every player, and finds
/// targets through one index of the hierarchy, so a crowd of players
/// blending several clips each allocates little and scans the world once.
/// A player with no layers takes exactly the path it took before there
/// were layers.
pub fn advance(world: &mut World, clips: &mut AnimationClips, delta: f32) {
    let mut due = Vec::new();
    let mut stacked = Vec::new();
    let mut picks = Vec::new();
    for (entity, player) in world.query_mut::<AnimationPlayer>() {
        if player.is_layered() {
            stacked.push((entity, layered::step(player, clips, delta, &mut picks)));
        } else if let Some(frame) = step(player, clips, delta, &mut picks) {
            due.push((entity, frame));
        }
    }
    if due.is_empty() && stacked.is_empty() {
        return;
    }
    // Writing values moves no parent link: one index serves the frame.
    let children = Children::of(world);
    let clips = &*clips;
    let mut buffers = Buffers::default();
    for (entity, frame) in due {
        write(world, clips, &children, &picks, &mut buffers, entity, frame);
    }
    if stacked.is_empty() {
        return;
    }
    let mut more = layered::Buffers::default();
    for (entity, frames) in stacked {
        layered::write(
            world,
            clips,
            &children,
            &picks,
            &mut buffers,
            &mut more,
            entity,
            frames,
        );
    }
}

/// Samples, mixes and writes one player's frame, and warns of what it
/// found nothing to write to.
fn write<'c>(
    world: &mut World,
    clips: &'c AnimationClips,
    children: &Children,
    picks: &[Pick],
    buffers: &mut Buffers<'c>,
    entity: Entity,
    frame: Frame,
) {
    let mut sampled = sample_frame(clips, picks, buffers, frame, false);
    let pose = buffers.result(&sampled);
    let missed = pose.apply_through(world, entity, children);
    let Some(player) = world.get_mut::<AnimationPlayer>(entity) else {
        return;
    };
    keep(player, &mut sampled, &buffers.mixed);
    report(player, sampled.fading, &buffers.incoming, &missed);
}
