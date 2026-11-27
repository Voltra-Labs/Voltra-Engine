//! A light with no place: Unity URP's Global `Light2D`, as an addition to
//! the camera's ambient for the pixels of its light layers.
//!
//! The camera's [`ambient`](crate::Camera2D::ambient) lights every pixel of a
//! lit frame; a global light adds its colour on top, but only where what was
//! drawn shares one of its light layers — a background left dim while the
//! foreground's layer is lifted, without a lamp. Its entity's transform does
//! not matter. In an unlit frame it does nothing, as every light does.

use voltra_ecs::World;
use voltra_render::{GlobalLight, ALL_LIGHT_LAYERS};

/// Adds `color × intensity` to every lit pixel on one of `layers`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct GlobalLight2D {
    /// Linear colour.
    pub color: [f32; 3],
    /// What the colour is multiplied by.
    pub intensity: f32,
    /// The light layers it lights, as a
    /// [`PointLight2D`](crate::PointLight2D)'s are. Every bit — the default,
    /// and not written — is more ambient.
    #[serde(
        default = "crate::light::all_light_layers",
        skip_serializing_if = "crate::light::is_all_light_layers"
    )]
    pub layers: u32,
}

impl Default for GlobalLight2D {
    /// A faint white, on every layer.
    fn default() -> Self {
        Self {
            color: [1.0; 3],
            intensity: 0.25,
            layers: ALL_LIGHT_LAYERS,
        }
    }
}

impl GlobalLight2D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "GlobalLight2D";

    /// The light as the renderer takes it.
    pub fn light(&self) -> GlobalLight {
        GlobalLight {
            color: self.color,
            intensity: self.intensity,
            layers: self.layers,
        }
    }
}

/// Every global light in `world` that gives light.
pub fn gather(world: &World) -> Vec<GlobalLight> {
    world
        .query::<GlobalLight2D>()
        .map(|(_, light)| light.light())
        .filter(GlobalLight::gives_light)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_global_light_that_gives_light_is_gathered() {
        let mut world = World::new();
        let on = |world: &mut World, light: GlobalLight2D| {
            let entity = world.spawn();
            world.insert(entity, light);
        };
        on(&mut world, GlobalLight2D::default());
        on(
            &mut world,
            GlobalLight2D {
                layers: 0b10,
                ..GlobalLight2D::default()
            },
        );
        on(
            &mut world,
            GlobalLight2D {
                intensity: 0.0,
                ..GlobalLight2D::default()
            },
        );
        let mut layers: Vec<u32> = gather(&world).iter().map(|l| l.layers).collect();
        layers.sort_unstable();
        assert_eq!(layers, [0b10, ALL_LIGHT_LAYERS]);
    }

    #[test]
    fn a_global_light_round_trips_and_its_default_layers_are_not_written() {
        let plain = ron::to_string(&GlobalLight2D::default()).expect("writes");
        assert!(!plain.contains("layers"), "{plain}");
        let picky = GlobalLight2D {
            layers: 0b100,
            ..GlobalLight2D::default()
        };
        let text = ron::to_string(&picky).expect("writes");
        assert_eq!(ron::from_str::<GlobalLight2D>(&text).expect("reads"), picky);
        let filled: GlobalLight2D = ron::from_str("(intensity: 2.0)").expect("reads");
        assert_eq!(filled.layers, ALL_LIGHT_LAYERS);
    }
}
