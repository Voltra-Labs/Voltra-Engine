//! What surrounds a 3D view: the sky drawn where nothing else is, and the
//! fog between the eye and what it sees — `environment3d.wgsl`'s
//! `sky_color` and `fog_amount` on the CPU, number for number.

use glam::Vec3;

/// A sky of three colours by elevation: `top` straight up, `horizon`
/// level, `bottom` straight down, blended linearly in the elevation angle
/// between them. Linear RGB in the light units' energy (a luminance over
/// K), so past 1 is brighter than an EV100 15 exposure's white.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyGradient {
    /// Straight up.
    pub top: [f32; 3],
    /// Level with the eye, every way round.
    pub horizon: [f32; 3],
    /// Straight down.
    pub bottom: [f32; 3],
}

impl SkyGradient {
    /// One colour every way: what a flat background is to the fog.
    pub fn flat(color: [f32; 3]) -> Self {
        Self {
            top: color,
            horizon: color,
            bottom: color,
        }
    }

    /// The sky along `direction` (unit length): the gradient at its
    /// elevation, `asin(y) / 90°` from −1 down to 1 up.
    pub fn color(&self, direction: Vec3) -> Vec3 {
        let elevation = direction.y.clamp(-1.0, 1.0).asin() / std::f32::consts::FRAC_PI_2;
        let horizon = Vec3::from(self.horizon);
        if elevation >= 0.0 {
            horizon.lerp(Vec3::from(self.top), elevation.clamp(0.0, 1.0))
        } else {
            horizon.lerp(Vec3::from(self.bottom), (-elevation).clamp(0.0, 1.0))
        }
    }

    /// Its colours with any number that is not finite or is negative made
    /// 0: what the uniform holds.
    pub fn sanitized(&self) -> Self {
        let sane = |c: [f32; 3]| c.map(|c| if c.is_finite() { c.max(0.0) } else { 0.0 });
        Self {
            top: sane(self.top),
            horizon: sane(self.horizon),
            bottom: sane(self.bottom),
        }
    }
}

/// Fog: one medium filling the world, denser below `height` — Filament's
/// height fog. Its density at height `y` is `density · e^(−height_falloff ·
/// (y − height))`, so `height_falloff` 0 is plain distance fog, the same
/// everywhere; the light it takes away along a ray is that density
/// integrated from the eye to the point, `1 − e^(−∫)`, made exactly
/// rather than marched.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    /// Linear RGB in the light units' energy: what a distant thing fades
    /// to.
    pub color: [f32; 3],
    /// How thick it is at `height`, per metre: 0 is clear air; at 0.01 half
    /// the light is gone in some 69 m.
    pub density: f32,
    /// The height, in metres, at which it is `density` thick.
    pub height: f32,
    /// How fast it thins with height, per metre: 0 the same everywhere.
    pub height_falloff: f32,
    /// How much of the sky's colour, along the way the eye looks, replaces
    /// `color`: 0 none, 1 all — Godot's aerial perspective, so distant
    /// things fade into the sky behind them.
    pub sky_tint: f32,
}

impl Fog {
    /// Its numbers with what cannot be drawn made harmless: a density or a
    /// falloff that is not finite or negative 0, a colour likewise, the
    /// tint kept in `0..=1`.
    pub fn sanitized(&self) -> Self {
        let sane = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        Self {
            color: self.color.map(sane),
            density: sane(self.density),
            height: if self.height.is_finite() {
                self.height
            } else {
                0.0
            },
            height_falloff: sane(self.height_falloff),
            sky_tint: sane(self.sky_tint).min(1.0),
        }
    }

    /// How much of what is at `point` the fog hides from `eye`: 0 to 1.
    pub fn amount(&self, eye: Vec3, point: Vec3) -> f32 {
        let fog = self.sanitized();
        let ray = point - eye;
        let distance = ray.length();
        // The density at the eye, the exponent kept where an f32 holds it.
        let at_eye = fog.density
            * (-fog.height_falloff * (eye.y - fog.height))
                .clamp(-80.0, 80.0)
                .exp();
        // The mean of e^(−falloff · Δy) along the ray, relative to the eye.
        let k = (fog.height_falloff * ray.y).clamp(-80.0, 80.0);
        let along = if k.abs() < 1.0e-4 {
            1.0
        } else {
            (1.0 - (-k).exp()) / k
        };
        let optical = (at_eye * along).min(1.0e30) * distance;
        (1.0 - (-optical).exp()).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sky_is_its_three_colours_up_level_and_down() {
        let sky = SkyGradient {
            top: [0.0, 0.0, 1.0],
            horizon: [1.0, 1.0, 1.0],
            bottom: [0.0, 1.0, 0.0],
        };
        assert!(sky.color(Vec3::Y).abs_diff_eq(Vec3::Z, 1e-6));
        assert!(sky.color(Vec3::X).abs_diff_eq(Vec3::ONE, 1e-6));
        assert!(sky.color(Vec3::NEG_Y).abs_diff_eq(Vec3::Y, 1e-6));
        // Halfway up, 45°: halfway between horizon and top.
        let half = sky.color(Vec3::new(1.0, 1.0, 0.0).normalize());
        assert!(half.abs_diff_eq(Vec3::new(0.5, 0.5, 1.0), 1e-5), "{half}");
        let flat = SkyGradient::flat([0.2, 0.3, 0.4]);
        assert!(flat
            .color(Vec3::new(0.3, -0.8, 0.1).normalize())
            .abs_diff_eq(Vec3::new(0.2, 0.3, 0.4), 1e-6));
    }

    #[test]
    fn distance_fog_takes_the_light_exponentially_and_height_fog_thins_upward() {
        let fog = Fog {
            color: [1.0; 3],
            density: 0.01,
            height: 0.0,
            height_falloff: 0.0,
            sky_tint: 0.0,
        };
        let eye = Vec3::ZERO;
        let half = std::f32::consts::LN_2 / 0.01;
        assert!((fog.amount(eye, Vec3::new(0.0, 0.0, -half)) - 0.5).abs() < 1e-5);
        assert_eq!(fog.amount(eye, eye), 0.0);
        // Thinning upward: a point above is less fogged than one as far
        // below.
        let height = Fog {
            height_falloff: 0.2,
            ..fog
        };
        let up = height.amount(eye, Vec3::new(0.0, 20.0, -20.0));
        let down = height.amount(eye, Vec3::new(0.0, -20.0, -20.0));
        assert!(up < fog.amount(eye, Vec3::new(0.0, 20.0, -20.0)));
        assert!(down > up, "{down} {up}");
        // Level with the eye it is the plain fog at the eye's density.
        let level = height.amount(eye, Vec3::new(0.0, 0.0, -half));
        assert!((level - 0.5).abs() < 1e-5, "{level}");
        // Far below a dense layer: hidden, finite.
        let deep = Fog {
            height_falloff: 10.0,
            ..fog
        };
        let hidden = deep.amount(Vec3::new(0.0, 100.0, 0.0), Vec3::new(0.0, -100.0, -1.0));
        assert!(hidden.is_finite() && hidden <= 1.0);
        let broken = Fog {
            density: f32::NAN,
            height: f32::INFINITY,
            ..fog
        };
        assert_eq!(broken.amount(eye, Vec3::new(0.0, 0.0, -100.0)), 0.0);
    }
}
