//! The frame's light array, as `surface3d_light.wgsl`'s `Lights3D` reads
//! it: group 0, binding 1.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use super::{Lighting3D, MAX_DIRECTIONAL, MAX_PUNCTUAL};

/// One directional light: toward it (unit length) and its colour times
/// its energy.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
struct DirectionalGpu {
    toward: [f32; 4],
    color: [f32; 4],
}

/// One point or spot light, 48 bytes (the plan allows 64): where it is
/// and `1 / range²` in `w`, its colour times its energy and the cone's
/// scale in `w`, the way a spot shines and the cone's offset in `w` — a
/// point light's cone is scale 0, offset 1, so the shader needs no branch
/// between the two.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
struct PunctualGpu {
    position: [f32; 4],
    color: [f32; 4],
    spot: [f32; 4],
}

/// The uniform holding every light of a frame: how many of each, then up
/// to [`MAX_DIRECTIONAL`] directional and [`MAX_PUNCTUAL`] point and spot
/// lights. 6 288 bytes — under WebGL2's 16 KiB block with stage 140's
/// cascades and 141's sky coefficients to come (a test holds it there).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct LightsUniform3D {
    counts: [u32; 4],
    directional: [DirectionalGpu; MAX_DIRECTIONAL],
    punctual: [PunctualGpu; MAX_PUNCTUAL],
}

impl LightsUniform3D {
    /// The array of `lighting`'s lights, the first of each kind up to its
    /// cap — the scene side chooses which and says what it drops.
    /// A light with no finite direction, or a broken number, lights
    /// nothing, but keeps its slot so an instance's indices still point at
    /// the light they were chosen for.
    pub fn new(lighting: &Lighting3D) -> Self {
        let mut out = Self::zeroed();
        let finite3 = |c: [f32; 3]| c.map(|c| if c.is_finite() { c.max(0.0) } else { 0.0 });
        let energy = |e: f32| if e.is_finite() { e.max(0.0) } else { 0.0 };
        let directional = lighting.directional.iter().take(MAX_DIRECTIONAL);
        for (slot, light) in out.directional.iter_mut().zip(directional) {
            let toward = -light.direction.normalize_or_zero();
            let on = toward != Vec3::ZERO && toward.is_finite();
            let color = finite3(light.color).map(|c| c * energy(light.energy));
            *slot = DirectionalGpu {
                toward: toward.extend(0.0).to_array(),
                color: if on {
                    [color[0], color[1], color[2], 0.0]
                } else {
                    [0.0; 4]
                },
            };
        }
        let punctual = lighting.punctual.iter().take(MAX_PUNCTUAL);
        for (slot, light) in out.punctual.iter_mut().zip(punctual) {
            if light.is_broken() || light.range <= 0.0 {
                continue;
            }
            let color = light.color.map(|c| c * light.energy);
            let (scale, offset, direction) = match light.spot {
                Some(spot) => {
                    let (scale, offset) = spot.scale_offset();
                    let direction = spot.direction.normalize_or_zero();
                    if direction == Vec3::ZERO || !direction.is_finite() {
                        continue;
                    }
                    (scale, offset, direction)
                }
                None => (0.0, 1.0, Vec3::ZERO),
            };
            *slot = PunctualGpu {
                position: light
                    .position
                    .extend(1.0 / (light.range * light.range))
                    .to_array(),
                color: [color[0], color[1], color[2], scale],
                spot: direction.extend(offset).to_array(),
            };
        }
        out.counts = [
            lighting.directional.len().min(MAX_DIRECTIONAL) as u32,
            lighting.punctual.len().min(MAX_PUNCTUAL) as u32,
            0,
            0,
        ];
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render3d::{DirectionalLight, PunctualLight, SpotCone};

    /// What later stages put beside the lights in the same 16 KiB block:
    /// 140's four cascades (a matrix each, their splits and the bias) and
    /// 141's nine spherical-harmonic coefficients.
    const CASCADES_140: usize = 4 * 64 + 16 + 16;
    const SKY_SH_141: usize = 9 * 16;

    #[test]
    fn the_array_fits_webgl2_s_block_with_what_is_to_come() {
        assert_eq!(size_of::<PunctualGpu>(), 48);
        assert!(size_of::<PunctualGpu>() <= 64, "the plan's budget a light");
        assert_eq!(size_of::<LightsUniform3D>(), 16 + 4 * 32 + 128 * 48);
        let limits = wgpu::Limits::downlevel_webgl2_defaults();
        assert_eq!(limits.max_uniform_buffer_binding_size, 16 << 10);
        let planned = size_of::<LightsUniform3D>() + CASCADES_140 + SKY_SH_141;
        assert!(
            planned as u64 <= limits.max_uniform_buffer_binding_size,
            "{planned}"
        );
        assert_eq!(size_of::<LightsUniform3D>() % 16, 0);
    }

    #[test]
    fn lights_keep_their_slots_and_a_broken_one_lights_nothing() {
        let lamp = PunctualLight {
            position: Vec3::new(1.0, 2.0, 3.0),
            color: [1.0, 0.5, 0.25],
            energy: 2.0,
            range: 4.0,
            spot: None,
        };
        let spot = PunctualLight {
            spot: Some(SpotCone {
                direction: Vec3::NEG_Y * 3.0,
                inner: 0.2,
                outer: 0.4,
            }),
            ..lamp
        };
        let broken = PunctualLight {
            energy: f32::NAN,
            ..lamp
        };
        let lighting = Lighting3D {
            directional: vec![DirectionalLight {
                direction: Vec3::NEG_Y * 2.0,
                color: [1.0; 3],
                energy: 3.0,
            }],
            punctual: vec![broken, lamp, spot],
            ..Lighting3D::DARK
        };
        let u = LightsUniform3D::new(&lighting);
        assert_eq!(u.counts, [1, 3, 0, 0]);
        assert_eq!(u.directional[0].toward, [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(u.directional[0].color, [3.0, 3.0, 3.0, 0.0]);
        assert_eq!(
            u.punctual[0],
            PunctualGpu::zeroed(),
            "broken: dark, in place"
        );
        assert_eq!(u.punctual[1].position, [1.0, 2.0, 3.0, 1.0 / 16.0]);
        assert_eq!(u.punctual[1].color, [2.0, 1.0, 0.5, 0.0]);
        assert_eq!(u.punctual[1].spot, [0.0, 0.0, 0.0, 1.0], "a point: no cone");
        assert_eq!(u.punctual[2].spot[..3], [0.0, -1.0, 0.0]);
        assert!(u.punctual[2].color[3] > 0.0);
        let many = Lighting3D {
            punctual: vec![lamp; 200],
            ..Lighting3D::DARK
        };
        assert_eq!(LightsUniform3D::new(&many).counts[1], 128);
    }
}
