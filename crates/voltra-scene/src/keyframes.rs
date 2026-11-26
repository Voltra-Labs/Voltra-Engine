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

mod advance;
pub mod blend;
pub mod clip;
mod clock;
pub mod ease;
mod fade;
mod frame;
pub mod player;
pub mod pose;
pub mod property;
pub mod store;
pub mod synced;
pub mod target;
pub mod track;
pub mod value;

pub use advance::advance;
pub use blend::{blend, blend_into, Mixing};
pub use clip::AnimationClip;
pub use ease::Ease;
pub use player::AnimationPlayer;
pub use pose::{Pose, Sample};
pub use property::Property;
pub use store::{AnimationClips, ClipError};
pub use synced::SyncedClips;
pub use track::{Key, Track};
pub use value::{Value, ValueKind};

#[cfg(test)]
mod tests;
