//! The tone curve a 3D shot's HDR picture is brought into range by.

use glam::{Mat3, Vec3};

/// How a 3D camera's linear HDR light becomes the `0..=1` a screen shows:
/// the curve its post pass applies after exposure and before the grade.
///
/// Godot's `Environment.tonemap_mode`, Unity URP's tonemapping mode, Bevy's
/// `Tonemapping`. **AgX** by default, as Blender 4 and Godot 4.4 moved to:
/// bright colours desaturate towards white as film does, rather than
/// clipping to a pure hue. 2D shots have no curve — their light is already
/// in range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Tonemap {
    /// No curve: light past 1 clips. What the scene's colours are, exactly
    /// — a flat-coloured mesh reads back the colour it was given.
    None,
    /// `c / (1 + c)` per channel: gentle, never quite reaching white.
    Reinhard,
    /// Stephen Hill's fit of ACES's reference rendering and output
    /// transforms: contrasty, saturated highlights.
    Aces,
    /// Benjamin Wrensch's fit of Troy Sobotka's AgX: the default.
    #[default]
    AgX,
}

const AGX_MIN_EV: f32 = -12.47393;
const AGX_MAX_EV: f32 = 4.026069;

impl Tonemap {
    /// The most light a curve is handed (`tonemap.wgsl`'s
    /// `MAX_TONE_INPUT`): Reinhard's and ACES's ratios are NaN for infinite
    /// light, and every curve is at its top long before.
    pub const MAX_INPUT: f32 = 65504.0;

    /// Every curve, in the order the shader numbers them.
    pub const ALL: [Self; 4] = [Self::None, Self::Reinhard, Self::Aces, Self::AgX];

    /// Whether it is the default, [`AgX`](Self::AgX).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The number `tonemap.wgsl` picks the curve by.
    pub fn index(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Reinhard => 1.0,
            Self::Aces => 2.0,
            Self::AgX => 3.0,
        }
    }

    /// The curve on the CPU: what `tonemap.wgsl` computes for linear
    /// `rgb`, number for number — a reference for tests and for tools that
    /// preview a colour without a GPU. Negative light counts as none, light
    /// past [`MAX_INPUT`](Self::MAX_INPUT) as that, and NaN as none.
    pub fn apply(self, rgb: [f32; 3]) -> [f32; 3] {
        let c = Vec3::from(rgb.map(|c| if c.is_nan() { 0.0 } else { c }))
            .clamp(Vec3::ZERO, Vec3::splat(Self::MAX_INPUT));
        let out = match self {
            Self::None => c,
            Self::Reinhard => c / (Vec3::ONE + c),
            Self::Aces => aces(c),
            Self::AgX => agx(c),
        };
        out.to_array()
    }
}

fn agx(c: Vec3) -> Vec3 {
    let inset = Mat3::from_cols(
        Vec3::new(0.842_479_06, 0.042_328_24, 0.042_375_655),
        Vec3::new(0.078_433_6, 0.878_468_6, 0.078_433_6),
        Vec3::new(0.079_223_745, 0.079_166_13, 0.879_143),
    );
    let outset = Mat3::from_cols(
        Vec3::new(1.196_879, -0.052_896_853, -0.052_971_635),
        Vec3::new(-0.098_020_88, 1.151_903_1, -0.098_043_45),
        Vec3::new(-0.099_029_74, -0.098_961_18, 1.151_073_7),
    );
    let log = (inset * c)
        .max(Vec3::splat(1e-10))
        .to_array()
        .map(f32::log2);
    let x = (Vec3::from(log).clamp(Vec3::splat(AGX_MIN_EV), Vec3::splat(AGX_MAX_EV)) - AGX_MIN_EV)
        / (AGX_MAX_EV - AGX_MIN_EV);
    let x2 = x * x;
    let x4 = x2 * x2;
    let sigmoid =
        15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x
            - Vec3::splat(0.00232);
    Vec3::from(
        (outset * sigmoid)
            .max(Vec3::ZERO)
            .to_array()
            .map(|v| v.powf(2.2)),
    )
}

fn aces(c: Vec3) -> Vec3 {
    let input = Mat3::from_cols(
        Vec3::new(0.59719, 0.07600, 0.02840),
        Vec3::new(0.35458, 0.90834, 0.13383),
        Vec3::new(0.04823, 0.01566, 0.83777),
    );
    let output = Mat3::from_cols(
        Vec3::new(1.60475, -0.10208, -0.00327),
        Vec3::new(-0.53108, 1.10813, -0.07276),
        Vec3::new(-0.07367, -0.00605, 1.07602),
    );
    let v = input * c;
    let a = v * (v + Vec3::splat(0.024_578_6)) - Vec3::splat(0.000_090_537);
    let b = v * (0.983_729 * v + Vec3::splat(0.432_951)) + Vec3::splat(0.238_081);
    (output * (a / b)).clamp(Vec3::ZERO, Vec3::ONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curve_starts_at_black_rises_and_stays_in_range() {
        for curve in Tonemap::ALL {
            let black = curve.apply([0.0; 3]);
            assert!(black.iter().all(|&v| v.abs() < 2e-3), "{curve:?} {black:?}");
            let mut last = -1.0;
            for step in 0..200 {
                let x = 2f32.powf(step as f32 / 10.0 - 10.0);
                let y = curve.apply([x; 3])[1];
                assert!(y.is_finite() && y >= last - 1e-6, "{curve:?} at {x}: {y}");
                if curve != Tonemap::None {
                    assert!(y <= 1.0 + 1e-6, "{curve:?} at {x}: {y}");
                }
                last = y;
            }
        }
    }

    #[test]
    fn the_curves_land_where_their_authors_put_them() {
        // Middle grey: kept by None, halved-ish by Reinhard, and AgX and
        // ACES both near their published 0.18 → ~0.17–0.18 linear output.
        let grey = |curve: Tonemap| curve.apply([0.18; 3])[0];
        assert_eq!(grey(Tonemap::None), 0.18);
        assert!((grey(Tonemap::Reinhard) - 0.18 / 1.18).abs() < 1e-6);
        assert!(
            (0.10..0.25).contains(&grey(Tonemap::Aces)),
            "{}",
            grey(Tonemap::Aces)
        );
        assert!(
            (0.10..0.25).contains(&grey(Tonemap::AgX)),
            "{}",
            grey(Tonemap::AgX)
        );
        // Bright light: ACES and AgX reach white; Reinhard stays under it.
        assert!(Tonemap::Aces.apply([16.0; 3])[0] > 0.98);
        assert!(Tonemap::AgX.apply([16.0; 3])[0] > 0.95);
        assert!(Tonemap::Reinhard.apply([16.0; 3])[0] < 0.95);
        // AgX desaturates a bright pure red towards white; None clips it.
        let red = Tonemap::AgX.apply([8.0, 0.0, 0.0]);
        assert!(red[1] > 0.1 && red[0] > red[1], "{red:?}");
        assert_eq!(Tonemap::None.apply([8.0, 0.0, 0.0]), [8.0, 0.0, 0.0]);
    }

    #[test]
    fn the_default_is_agx_and_each_curve_has_its_own_number() {
        assert_eq!(Tonemap::default(), Tonemap::AgX);
        assert!(Tonemap::AgX.is_default());
        for (i, curve) in Tonemap::ALL.into_iter().enumerate() {
            assert_eq!(curve.index(), i as f32);
        }
    }
}
