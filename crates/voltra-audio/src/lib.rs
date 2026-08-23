//! Mixing and playing the sounds a scene asks for.
//!
//! The engine's mixer is its own: voices, resampling, panning and the
//! summation all live here, and the platform is used only as a device to hand
//! finished buffers to. That is the shape Unity, Unreal and Godot all have —
//! each writes its mixer and drives WASAPI, CoreAudio and ALSA underneath —
//! and the reason to copy it is not symmetry. A wrapped mixer decides what
//! panning means, what happens when a voice runs out, and how a sound is tied
//! to the fixed tick; those are engine decisions, and they are also the ones
//! that have to be tested without a sound card.
//!
//! **This crate owns the audio backend.** Only `voltra-audio` may depend on
//! `cpal` or `symphonia`, exactly as only `voltra-render` may depend on `wgpu`
//! and only `voltra-core` on `winit`. Everything above it deals in
//! [`Clip`]s, [`VoiceId`]s and [`PlayParams`].
//!
//! The layering inside is the same rule again: [`mixer`] and [`voice`] are
//! pure arithmetic with no thread and no device, [`device`] is the thin shell
//! that owns the `cpal` stream, and [`audio`] is the seam the engine holds.

pub mod audio;
pub mod clip;
pub mod command;
pub mod decode;
pub mod device;
pub mod error;
pub mod mixer;
pub mod params;
pub mod spatial;
pub mod voice;

pub use audio::Audio;
pub use clip::Clip;
pub use command::Command;
pub use decode::decode;
pub use device::Output;
pub use error::AudioError;
pub use mixer::{Mixer, DEFAULT_CAPACITY};
pub use params::PlayParams;
pub use voice::{Voice, VoiceId};
