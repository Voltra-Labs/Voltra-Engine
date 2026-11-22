//! Property animation: clips of keyed values, played onto components.
//!
//! A clip ([`AnimationClip`], a `.anim.ron` file) is tracks, each one
//! property ([`Property`]) of one entity — named by the path of names from
//! the player down ([`target::resolve`]) — keyed over time with an ease per
//! segment. An [`AnimationPlayer`] on an entity plays one clip onto it and
//! its children. Unity's `AnimationClip` and Godot's `Animation`, cut to
//! what a 2D game keys: transforms, colours, a sprite's frame, a light.
//!
//! Beside, not instead of, [`SpriteAnimation`](crate::SpriteAnimation): a
//! flipbook at one rate is a list of frames, and making it a clip would make
//! every walk cycle a file.

pub mod clip;
pub mod ease;
pub mod player;
pub mod property;
pub mod store;
pub mod target;
pub mod track;
pub mod value;

pub use clip::AnimationClip;
pub use ease::Ease;
pub use player::AnimationPlayer;
pub use property::Property;
pub use store::{AnimationClips, ClipError};
pub use track::{Key, Track};
pub use value::{Value, ValueKind};

use voltra_ecs::World;

/// Runs every playing [`AnimationPlayer`] `delta` seconds and writes its
/// clip's values.
///
/// Per frame, as `SpriteAnimation` is: what is animated is what is drawn.
/// A player applies nothing while it is not playing, so an entity whose clip
/// has ended — or was paused — can be moved by other code without the clip
/// putting it back. A dynamic body's transform is the simulation's: the
/// step after this overwrites it, so animate a kinematic body, or a child.
pub fn advance(world: &mut World, clips: &mut AnimationClips, delta: f32) {
    let mut due = Vec::new();
    for (entity, player) in world.query_mut::<AnimationPlayer>() {
        let Some(path) = player.clip.as_ref() else {
            continue;
        };
        let handle = clips.load(path);
        let clip = clips.get(handle);
        if clip.is_empty() {
            continue;
        }
        if player.step(clip.duration(), clip.looping, delta) {
            due.push((entity, handle, player.time()));
        }
    }

    for (entity, handle, time) in due {
        let missed: Vec<String> = clips
            .get(handle)
            .apply(world, entity, time)
            .into_iter()
            .map(|track| format!("{:?} on {:?}", track.property, track.target))
            .collect();
        let Some(player) = world.get_mut::<AnimationPlayer>(entity) else {
            continue;
        };
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

#[cfg(test)]
mod tests;
