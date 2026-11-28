//! What surrounds the 3D world: its ambient light.

use serde::{Deserialize, Serialize};

/// The 3D world's surroundings: Godot's `WorldEnvironment`. On a camera's
/// entity it is that camera's; on any other entity it is the world's, used
/// by every camera without its own — with several, the one on the lowest
/// entity index, the rest warned about once. With none, the default.
///
/// Stage 137 gives it the ambient alone; stage 139 adds the background,
/// an ambient energy and fog, and 141 a sky that lights.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Environment3D {
    /// A uniform sky's light, linear RGB in the light units' energy: the
    /// diffuse it gives a white surface, before occlusion. Lights a surface
    /// facing away from every light. The default, a dim grey of 0.1, is not
    /// written.
    #[serde(default = "dim", skip_serializing_if = "is_dim")]
    pub ambient: [f32; 3],
}

fn dim() -> [f32; 3] {
    [Environment3D::DEFAULT_AMBIENT; 3]
}

fn is_dim(ambient: &[f32; 3]) -> bool {
    *ambient == dim()
}

impl Default for Environment3D {
    fn default() -> Self {
        Self { ambient: dim() }
    }
}

impl Environment3D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Environment3D";

    /// The ambient of a world with no environment: a dim grey, so a mesh
    /// with no light is still seen (Godot's default environment lights by
    /// its clear colour, Bevy's `AmbientLight` by a dim white); an
    /// `Environment3D` with black ambient makes the dark.
    pub const DEFAULT_AMBIENT: f32 = 0.1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_environment_writes_nothing_and_reads_back() {
        let environment = Environment3D::default();
        assert_eq!(ron::to_string(&environment).expect("written"), "()");
        let dark = Environment3D { ambient: [0.0; 3] };
        let text = ron::to_string(&dark).expect("written");
        assert_eq!(ron::from_str::<Environment3D>(&text).expect("read"), dark);
    }
}
