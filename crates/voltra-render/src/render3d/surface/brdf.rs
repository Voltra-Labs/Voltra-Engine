//! The standard surface's lighting on the CPU: `surface3d.wgsl`'s
//! `shade`, number for number — what a test compares a picture's pixels
//! to, as [`Tonemap::apply`](crate::render3d::Tonemap::apply) is the tone
//! curve's twin.
//!
//! Filament's BRDF: GGX distribution, height-correlated Smith visibility,
//! Schlick Fresnel, Lambert diffuse; a uniform ambient through Karis's
//! analytic split-sum fit. Linear light in, linear light out.

use glam::Vec3;

/// Filament's floor on perceptual roughness.
pub const MIN_ROUGHNESS: f32 = 0.045;

/// The most light a surface sends back, per channel (`surface3d.wgsl`'s
/// `MAX_LIGHT`): under a half float's 65504 with room for bloom's levels
/// summed. A mirror-smooth highlight near grazing reaches some 10⁵ at
/// energy 1.
pub const MAX_LIGHT: f32 = 4096.0;

/// What reflects: the surface's numbers at one point, after textures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Linear RGB.
    pub base: Vec3,
    /// 0 a dielectric, 1 a metal.
    pub metallic: f32,
    /// Perceptual roughness.
    pub roughness: f32,
    /// 1 unoccluded.
    pub occlusion: f32,
    /// Linear RGB given off.
    pub emissive: Vec3,
}

/// What lights it, from where it is seen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    /// The shading normal, unit length, facing the eye's side.
    pub n: Vec3,
    /// Toward the eye, unit length.
    pub v: Vec3,
    /// Toward the light, unit length; with `color` zero for none.
    pub l: Vec3,
    /// The light's colour times its energy.
    pub color: Vec3,
    /// The ambient: a uniform sky's light.
    pub ambient: Vec3,
}

/// GGX / Trowbridge-Reitz, Filament's form.
pub fn d_ggx(n_dot_h: f32, n_cross_h: Vec3, a: f32) -> f32 {
    let one_minus = n_cross_h.dot(n_cross_h);
    let ah = n_dot_h * a;
    let k = a / (one_minus + ah * ah);
    k * k * std::f32::consts::FRAC_1_PI
}

/// Height-correlated Smith visibility, `G / (4 n·l n·v)`.
pub fn v_smith_ggx_correlated(n_dot_v: f32, n_dot_l: f32, a: f32) -> f32 {
    let a2 = a * a;
    let ggx_v = n_dot_l * (n_dot_v * n_dot_v * (1.0 - a2) + a2).sqrt();
    let ggx_l = n_dot_v * (n_dot_l * n_dot_l * (1.0 - a2) + a2).sqrt();
    0.5 / (ggx_v + ggx_l)
}

/// Schlick's Fresnel, toward white at grazing.
pub fn f_schlick(f0: Vec3, l_dot_h: f32) -> Vec3 {
    let f = (1.0 - l_dot_h).powi(5);
    Vec3::splat(f) + f0 * (1.0 - f)
}

/// What a uniform environment reflects, Karis's fit of the split sum.
pub fn env_brdf(f0: Vec3, roughness: f32, n_dot_v: f32) -> Vec3 {
    let c0 = [-1.0, -0.0275, -0.572, 0.022];
    let c1 = [1.0, 0.0425, 1.04, -0.04];
    let r: [f32; 4] = std::array::from_fn(|i| roughness * c0[i] + c1[i]);
    let a004 = (r[0] * r[0]).min((-9.28 * n_dot_v).exp2()) * r[0] + r[1];
    let (scale, bias) = (-1.04 * a004 + r[2], 1.04 * a004 + r[3]);
    f0 * scale + Vec3::splat(bias)
}

fn unit_or(v: Vec3, fallback: Vec3) -> Vec3 {
    let length_squared = v.dot(v);
    if length_squared > 1.0e-30 {
        v / length_squared.sqrt()
    } else {
        fallback
    }
}

/// The light `point` sends back toward the eye, lit as `light` says:
/// the shader's `shade`, before alpha.
pub fn shade(point: &Point, light: &Light) -> Vec3 {
    let (n, v, l) = (light.n, light.v, light.l);
    let metallic = point.metallic.clamp(0.0, 1.0);
    let roughness = point.roughness.clamp(MIN_ROUGHNESS, 1.0);
    let diffuse_color = point.base * (1.0 - metallic);
    let f0 = Vec3::splat(0.04).lerp(point.base, metallic);
    let n_dot_v = (n.dot(v).abs() + 1.0e-5).min(1.0);
    let a = roughness * roughness;
    let h = unit_or(v + l, n);
    let n_dot_l = n.dot(l).clamp(0.0, 1.0);
    let n_dot_h = n.dot(h).clamp(0.0, 1.0);
    let l_dot_h = l.dot(h).clamp(0.0, 1.0);
    let specular = d_ggx(n_dot_h, n.cross(h), a)
        * v_smith_ggx_correlated(n_dot_v, n_dot_l, a)
        * f_schlick(f0, l_dot_h);
    let direct = (diffuse_color * std::f32::consts::FRAC_1_PI + specular) * light.color * n_dot_l;
    // Specular E = f0·A + B, the diffuse taking what it leaves (Filament's).
    let reflected = env_brdf(f0, roughness, n_dot_v);
    let ambient =
        (diffuse_color * (Vec3::ONE - reflected) + reflected) * light.ambient * point.occlusion;
    (direct + ambient + point.emissive).min(Vec3::splat(MAX_LIGHT))
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI};

    use super::*;

    /// ∫ D(h) (n·h) dω over the hemisphere, by the midpoint rule in theta.
    fn projected_area(a: f32) -> f32 {
        let steps = 20_000;
        let dtheta = FRAC_PI_2 / steps as f32;
        (0..steps)
            .map(|i| {
                let theta = (i as f32 + 0.5) * dtheta;
                let (s, c) = theta.sin_cos();
                let n_cross_h = Vec3::new(s, 0.0, 0.0);
                d_ggx(c, n_cross_h, a) * c * s * dtheta * 2.0 * PI
            })
            .sum()
    }

    #[test]
    fn the_distribution_covers_the_surface_once_at_every_roughness() {
        for roughness in [0.2f32, 0.5, 0.8, 1.0] {
            let area = projected_area(roughness * roughness);
            assert!((area - 1.0).abs() < 2.0e-3, "{roughness}: {area}");
        }
    }

    #[test]
    fn fresnel_is_f0_head_on_and_white_at_grazing() {
        let f0 = Vec3::new(0.04, 0.5, 0.9);
        assert!(f_schlick(f0, 1.0).abs_diff_eq(f0, 1e-6));
        assert!(f_schlick(f0, 0.0).abs_diff_eq(Vec3::ONE, 1e-6));
    }

    #[test]
    fn visibility_never_reflects_more_than_comes_in() {
        // G = 4 n·l n·v V lies in 0..=1.
        for a in [0.002f32, 0.1, 0.5, 1.0] {
            for n_dot_v in [0.01f32, 0.3, 1.0] {
                for n_dot_l in [0.01f32, 0.5, 1.0] {
                    let g = 4.0 * n_dot_l * n_dot_v * v_smith_ggx_correlated(n_dot_v, n_dot_l, a);
                    assert!(
                        (0.0..=1.0 + 1e-5).contains(&g),
                        "{a} {n_dot_v} {n_dot_l}: {g}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_white_rough_dielectric_lit_head_on_is_lambert_and_a_little_specular() {
        let point = Point {
            base: Vec3::ONE,
            metallic: 0.0,
            roughness: 1.0,
            occlusion: 1.0,
            emissive: Vec3::ZERO,
        };
        let light = Light {
            n: Vec3::Z,
            v: Vec3::Z,
            l: Vec3::Z,
            color: Vec3::ONE,
            ambient: Vec3::ZERO,
        };
        let out = shade(&point, &light);
        // 1/π diffuse, plus D·V·F at a = 1: (1/π)·(1/4)·0.04.
        let expected = 1.0 / PI + 0.04 / (4.0 * PI);
        assert!((out.x - expected).abs() < 1e-5, "{out}");
        // A uniform white sky reflects what it is lit with: the specular
        // takes E and the diffuse the rest, (1 − E), so white sums to 1.
        let sky = shade(
            &point,
            &Light {
                color: Vec3::ZERO,
                ambient: Vec3::ONE,
                ..light
            },
        );
        assert!((sky.x - 1.0).abs() < 0.01, "{sky}");
    }

    #[test]
    fn a_mirror_highlight_near_grazing_stays_finite_under_the_cap() {
        let mirror = Point {
            base: Vec3::ONE,
            metallic: 1.0,
            roughness: 0.0,
            occlusion: 1.0,
            emissive: Vec3::ZERO,
        };
        // Lit from just past the rim, seen at the mirror angle.
        let l = Vec3::new(0.0, 0.999, 0.02).normalize();
        let v = Vec3::new(0.0, -0.999, 0.02).normalize();
        let lit = Light {
            n: Vec3::Z,
            v,
            l,
            color: Vec3::ONE,
            ambient: Vec3::ZERO,
        };
        let out = shade(&mirror, &lit);
        assert_eq!(out.max_element(), MAX_LIGHT, "capped: {out}");
        let glowing = shade(
            &Point {
                emissive: Vec3::splat(1.0e30),
                ..mirror
            },
            &lit,
        );
        assert!(glowing.is_finite() && glowing.max_element() <= MAX_LIGHT);
        for curve in crate::render3d::Tonemap::ALL {
            let toned = curve.apply([f32::INFINITY, f32::NAN, 1.0e30]);
            assert!(toned.iter().all(|c| c.is_finite()), "{curve:?}: {toned:?}");
        }
    }

    #[test]
    fn a_metal_has_no_diffuse_and_reflects_its_colour() {
        let gold = Point {
            base: Vec3::new(1.0, 0.77, 0.34),
            metallic: 1.0,
            roughness: 0.3,
            occlusion: 1.0,
            emissive: Vec3::ZERO,
        };
        // Lit from 60° off, seen from the mirror direction: the highlight.
        let l = Vec3::new(60f32.to_radians().sin(), 0.0, 60f32.to_radians().cos());
        let v = Vec3::new(-l.x, 0.0, l.z);
        let mirror = shade(
            &gold,
            &Light {
                n: Vec3::Z,
                v,
                l,
                color: Vec3::ONE,
                ambient: Vec3::ZERO,
            },
        );
        // Seen head on, far from the mirror direction: dark.
        let away = shade(
            &gold,
            &Light {
                n: Vec3::Z,
                v: Vec3::Z,
                l,
                color: Vec3::ONE,
                ambient: Vec3::ZERO,
            },
        );
        assert!(
            mirror.x > 1.0 && mirror.x > away.x * 20.0,
            "{mirror} {away}"
        );
        assert!(mirror.z < mirror.x, "tinted by its colour: {mirror}");
    }
}
