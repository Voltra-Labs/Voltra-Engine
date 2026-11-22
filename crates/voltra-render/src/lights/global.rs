//! A light every pixel on its layers gets, wherever it is.
//!
//! Unity URP's Global `Light2D`, as an addition: the camera's ambient lights
//! every pixel, and each global light adds its colour on top for the pixels
//! of its light layers — so one layer can be lit brighter than the rest
//! without a lamp. Drawn by `lights_global.wgsl` as a full-screen triangle
//! per light onto the light map, testing the scene's light-layer buffer as
//! the point lights do.

use bytemuck::{Pod, Zeroable};

use super::ALL_LIGHT_LAYERS;

/// A light with no place: every pixel on one of its layers gets it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalLight {
    /// Linear colour, multiplied by `intensity`.
    pub color: [f32; 3],
    pub intensity: f32,
    /// The light layers it lights. [`ALL_LIGHT_LAYERS`] is more ambient.
    pub layers: u32,
}

/// One global light as `lights_global.wgsl` reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(super) struct GlobalInstance {
    /// colour × intensity, unused
    color: [f32; 4],
    /// the light layers it lights
    layers: u32,
}

impl GlobalLight {
    /// Whether it gives anything: an intensity and a colour, both numbers,
    /// and a layer to give it to.
    pub fn gives_light(&self) -> bool {
        self.intensity.is_finite()
            && self.color.iter().all(|c| c.is_finite())
            && self.intensity > 0.0
            && self.color.iter().any(|&c| c > 0.0)
            && self.layers != 0
    }

    /// Whether it lights only some layers: when the scene's light layers
    /// must be drawn for it.
    pub(super) fn picks_layers(&self) -> bool {
        self.gives_light() && self.layers != ALL_LIGHT_LAYERS
    }

    /// What the shader draws, or `None` for a light that gives nothing.
    pub(super) fn instance(&self) -> Option<GlobalInstance> {
        self.gives_light().then(|| {
            let [r, g, b] = self.color.map(|c| c.max(0.0) * self.intensity);
            GlobalInstance {
                color: [r, g, b, 0.0],
                layers: self.layers,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glow() -> GlobalLight {
        GlobalLight {
            color: [1.0, 0.5, 0.0],
            intensity: 0.5,
            layers: 0b10,
        }
    }

    #[test]
    fn a_global_light_is_its_colour_times_its_intensity_on_its_layers() {
        let i = glow().instance().expect("gives light");
        assert_eq!(i.color, [0.5, 0.25, 0.0, 0.0]);
        assert_eq!(i.layers, 0b10);
        assert!(glow().picks_layers());
        let everywhere = GlobalLight {
            layers: ALL_LIGHT_LAYERS,
            ..glow()
        };
        assert!(
            !everywhere.picks_layers(),
            "more ambient, no layers to read"
        );
    }

    #[test]
    fn a_global_light_that_gives_nothing_is_not_drawn() {
        for dark in [
            GlobalLight {
                intensity: 0.0,
                ..glow()
            },
            GlobalLight {
                color: [0.0; 3],
                ..glow()
            },
            GlobalLight {
                intensity: f32::NAN,
                ..glow()
            },
            GlobalLight {
                layers: 0,
                ..glow()
            },
        ] {
            assert_eq!(dark.instance(), None, "{dark:?}");
            assert!(!dark.picks_layers());
        }
    }
}
