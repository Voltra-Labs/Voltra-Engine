//! The shadow map: per shadowed light, how far its light gets in every
//! direction before an occluder stops it.
//!
//! Godot's 2D shadow atlas. Each shadowed light owns four rows of one
//! [`SHADOW_FORMAT`] texture, a quarter turn each (right, up, left, down).
//! A texel across a row is a direction, through a 90° perspective
//! projection centred on the light; its value is the distance to the
//! nearest occluding edge that way, over the light's radius — `1`, the
//! clear, where nothing is in reach. Every casting edge is drawn as a quad
//! filling the light's row in each quarter, clipped by the projection to the
//! directions it covers, and the nearest wins by a `Min` blend: no depth
//! buffer, the engine has none.
//!
//! The accumulate pass then looks each lit pixel's direction up in its
//! light's rows and keeps the light only where the pixel is nearer than
//! what was stored, sampled either side for a soft edge.

mod pass;
mod plan;

pub(super) use pass::ShadowPass;
pub(super) use plan::{instance_count, ShadowPlan};

/// One distance per texel, a fraction of the light's radius. Half floats
/// are blendable everywhere — `R32Float` needs a feature for `Min` — and
/// keep the fraction to within 1/2048 near the radius.
pub const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// Quarter turns per light, one row each.
const QUARTERS: u32 = 4;
