//! Property animation: clips of keyed values, played onto components.
//!
//! A clip ([`AnimationClip`], a `.anim.ron` file) is tracks, each one
//! property ([`Property`]) of one entity — named by the path of names from
//! the player down ([`target::resolve`]) — keyed over time with an ease per
//! segment. An [`AnimationPlayer`] on an entity plays one clip onto it and
//! its children. Unity's `AnimationClip` and Godot's `Animation`, cut to
//! what a 2D game keys: transforms, colours, a sprite's frame, a light.
//!
//! While a player fades from one clip to the next, both are sampled
//! without writing ([`AnimationClip::sample`], a [`Pose`]), mixed by weight
//! ([`blend`]) and written once — the mechanism anything playing several
//! clips at once goes through.
//!
//! Beside, not instead of, [`SpriteAnimation`](crate::SpriteAnimation): a
//! flipbook at one rate is a list of frames, and making it a clip would make
//! every walk cycle a file.

pub mod blend;
pub mod clip;
pub mod ease;
mod fade;
pub mod player;
pub mod pose;
pub mod property;
pub mod store;
pub mod target;
pub mod track;
pub mod value;

pub use blend::{blend, Mixing};
pub use clip::AnimationClip;
pub use ease::Ease;
pub use player::AnimationPlayer;
pub use pose::{Pose, Sample};
pub use property::Property;
pub use store::{AnimationClips, ClipError};
pub use track::{Key, Track};
pub use value::{Value, ValueKind};

use voltra_assets::Handle;
use voltra_ecs::World;

use fade::Outgoing;

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
        let (pose, faded) = match frame.fade {
            None => (incoming, None),
            Some((from, weight)) => {
                let (outgoing, frozen) = match from {
                    Source::Clip(handle, time) => (clips.get(handle).sample(time), false),
                    Source::Pose(pose) => (pose, true),
                };
                let mixed = blend(&[(&outgoing, 1.0 - weight), (&incoming, weight)]);
                (mixed, Some((outgoing, frozen)))
            }
        };
        let missed: Vec<String> = pose
            .apply(world, entity)
            .into_iter()
            .map(|s| format!("{:?} on {:?}", s.property, s.target))
            .collect();
        let Some(player) = world.get_mut::<AnimationPlayer>(entity) else {
            continue;
        };
        if let (Some(fade), Some((outgoing, frozen))) = (player.fade.as_mut(), faded) {
            fade.last = Some(pose.into_owned());
            if frozen {
                fade.from = Outgoing::Pose(outgoing.into_owned());
            }
        }
        if missed.is_empty() || player.reported == player.clip {
            continue;
        }
        player.reported = player.clip.clone();
        let clip = player.clip.as_ref().map_or("", |p| p.as_str());
        log::warn!(
            "animation {clip}: nothing to animate for {}",
            missed.join(", ")
        );
    }
}

/// What one player writes this frame, found while its clocks run.
struct Frame {
    /// Its clip, and the time to sample it at; `None` for one that will not
    /// read.
    clip: Option<(Handle<AnimationClip>, f32)>,
    /// While it fades: what it fades out, and its clip's weight.
    fade: Option<(Source, f32)>,
}

/// The outgoing side of a fade, ready to sample.
enum Source {
    /// A clip, at a time.
    Clip(Handle<AnimationClip>, f32),
    /// A held pose, lent by the fade for the frame and given back.
    Pose(Pose<'static>),
}

/// Runs `player`'s clocks — its clip's and, fading, the outgoing one's —
/// and answers what to write, if anything.
fn step(player: &mut AnimationPlayer, clips: &mut AnimationClips, delta: f32) -> Option<Frame> {
    let incoming = player
        .clip
        .as_ref()
        .map(|path| clips.load(path))
        .filter(|&handle| !clips.get(handle).is_empty());
    let Some(mut fade) = player.fade.take() else {
        let handle = incoming?;
        let clip = clips.get(handle);
        return player
            .step(clip.duration(), clip.looping, delta)
            .then(|| Frame {
                clip: Some((handle, player.time())),
                fade: None,
            });
    };
    // Paused mid-fade: everything holds, the fade too.
    if !player.playing && !fade.held {
        player.fade = Some(fade);
        return None;
    }
    if let Some(handle) = incoming {
        let clip = clips.get(handle);
        let was = player.playing;
        player.step(clip.duration(), clip.looping, delta);
        fade.held |= was && !player.playing;
    }
    fade.tick(delta);
    let clip = incoming.map(|handle| (handle, player.time()));
    if fade.done() {
        // The last frame of the fade is the clip alone, and the fade ends.
        return clip.map(|clip| Frame {
            clip: Some(clip),
            fade: None,
        });
    }
    let from = match &mut fade.from {
        Outgoing::Clip(out) => {
            let handle = clips.load(&out.clip);
            let source = clips.get(handle);
            player::run_clock(
                &mut out.time,
                &mut out.playing,
                out.speed,
                source.duration(),
                source.looping,
                delta,
            );
            Source::Clip(handle, out.time)
        }
        Outgoing::Pose(pose) => Source::Pose(std::mem::take(pose)),
    };
    let weight = fade.weight();
    player.fade = Some(fade);
    Some(Frame {
        clip,
        fade: Some((from, weight)),
    })
}

#[cfg(test)]
mod tests;
