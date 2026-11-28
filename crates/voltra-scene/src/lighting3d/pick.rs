//! Which point and spot lights matter most to one instance: each light's
//! light estimated at the instance's box, the eight largest kept.

use voltra_render::glam::Vec3;
use voltra_render::render3d::{
    distance_attenuation, Aabb3, LightList3D, PunctualLight, LIGHTS_PER_INSTANCE,
};

/// What choosing an instance's lights weighs a point or spot light by: its
/// place, reach, brightness, cone and layers, and its entity's index —
/// the tie-break, so two equal lights are chosen the same way every frame
/// whatever the camera does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightReach3D {
    /// The light's entity's index: equal estimates keep the lower first.
    pub entity: u32,
    /// Where it is.
    pub position: Vec3,
    /// Metres it reaches.
    pub range: f32,
    /// Its brightest channel times its energy.
    pub intensity: f32,
    /// A spot's axis and the cosine and sine of its outer angle.
    pub cone: Option<(Vec3, f32, f32)>,
    /// The layers it lights.
    pub layers: u32,
}

impl LightReach3D {
    /// The reach of `light`, of brightness `intensity`, on `layers`, its
    /// entity's index `entity`.
    pub fn new(entity: u32, light: &PunctualLight, intensity: f32, layers: u32) -> Self {
        Self {
            entity,
            position: light.position,
            range: light.range,
            intensity,
            cone: light.spot.map(|spot| {
                let outer = if spot.outer.is_finite() {
                    spot.outer.clamp(0.0, std::f32::consts::FRAC_PI_2)
                } else {
                    0.0
                };
                (spot.direction.normalize_or_zero(), outer.cos(), outer.sin())
            }),
            layers,
        }
    }

    /// How much light it is estimated to give `bounds` on `layers`: its
    /// brightness through the falloff at the box's nearest point to it,
    /// 0 when the box is out of its range or its cone, or off its layers.
    /// An estimate, so a light inside the box counts as a centimetre away.
    pub fn estimate(&self, bounds: &Aabb3, layers: u32) -> f32 {
        if self.layers & layers == 0 {
            return 0.0;
        }
        let nearest = self.position.clamp(bounds.min, bounds.max);
        let distance_squared = (nearest - self.position).length_squared();
        if distance_squared >= self.range * self.range {
            return 0.0;
        }
        if let Some((axis, cos_outer, sin_outer)) = self.cone {
            if !in_cone(self.position, axis, cos_outer, sin_outer, bounds) {
                return 0.0;
            }
        }
        self.intensity * distance_attenuation(distance_squared, self.range)
    }
}

/// Whether any of the ball around `bounds` may be inside the cone from
/// `apex` along `axis` of the outer angle whose cosine and sine are given:
/// conservative — a ball the apex is inside always is.
fn in_cone(apex: Vec3, axis: Vec3, cos_outer: f32, sin_outer: f32, bounds: &Aabb3) -> bool {
    let center = bounds.center();
    let radius = bounds.half_extents().length();
    let to = center - apex;
    let distance = to.length();
    if distance <= radius {
        return true;
    }
    // The ball is inside the cone widened by its angular radius: compare
    // the angle to the centre against outer + asin(radius / distance).
    let cos_to = to.dot(axis) / distance;
    let sin_r = radius / distance;
    let cos_r = (1.0 - sin_r * sin_r).max(0.0).sqrt();
    // cos(outer + r), with outer + r capped at a half-turn.
    let cos_reach = cos_outer * cos_r - sin_outer * sin_r;
    let past_half_turn = cos_outer * sin_r + sin_outer * cos_r < 0.0;
    past_half_turn || cos_to >= cos_reach
}

/// One instance's choice: its list and how many lights reached it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picked3D {
    /// The directional lights on its layers and the point and spot lights
    /// estimated to give it most, at most [`LIGHTS_PER_INSTANCE`].
    pub list: LightList3D,
    /// How many point and spot lights reach it at all: past
    /// [`LIGHTS_PER_INSTANCE`], the least are dropped.
    pub reached: usize,
}

/// The lights of an instance with world box `bounds` on `layers`: every
/// directional light whose `directional_layers` share a bit with them, and
/// of `lights` — the frame's point and spot lights, in the order the
/// uniform holds them — the [`LIGHTS_PER_INSTANCE`] estimated to give it
/// most ([`LightReach3D::estimate`]), the lower entity first between
/// equals. Nothing about the camera is read, so the choice holds still as
/// the camera moves.
pub fn pick(
    lights: &[LightReach3D],
    directional_layers: &[u32],
    bounds: &Aabb3,
    layers: u32,
) -> Picked3D {
    let directional = directional_layers
        .iter()
        .take(8)
        .enumerate()
        .filter(|(_, l)| *l & layers != 0)
        .fold(0u8, |bits, (i, _)| bits | 1 << i);
    // The best so far, the largest first: (estimate, entity, index).
    let mut best = [(0.0f32, u32::MAX, LightList3D::NONE); LIGHTS_PER_INSTANCE];
    let mut kept = 0;
    let mut reached = 0;
    let index_limit = usize::from(LightList3D::NONE);
    for (index, light) in lights.iter().enumerate().take(index_limit) {
        let estimate = light.estimate(bounds, layers);
        if estimate.is_nan() || estimate <= 0.0 {
            continue;
        }
        reached += 1;
        let better = |(e, entity, _): &(f32, u32, u8)| {
            estimate > *e || (estimate == *e && light.entity < *entity)
        };
        let open = (kept < LIGHTS_PER_INSTANCE).then_some(kept);
        let Some(at) = best[..kept].iter().position(better).or(open) else {
            continue;
        };
        let end = kept.min(LIGHTS_PER_INSTANCE - 1);
        best.copy_within(at..end, at + 1);
        best[at] = (estimate, light.entity, index as u8);
        kept = (kept + 1).min(LIGHTS_PER_INSTANCE);
    }
    let mut indices = [LightList3D::NONE; LIGHTS_PER_INSTANCE];
    for (slot, (_, _, index)) in indices.iter_mut().zip(&best[..kept]) {
        *slot = *index;
    }
    Picked3D {
        list: LightList3D::new(directional, &indices[..kept]),
        reached,
    }
}
