//! A camera drawing into a render texture instead of the screen.

use voltra_assets::{AssetPath, Handle};
use voltra_render::Texture;

/// Makes this entity's [`Camera`](super::Camera) draw into a render texture
/// — a `.rendertexture.ron` — instead of the screen: a minimap, a security
/// monitor, a mirror. A sprite or a UI image naming the same file as its
/// texture shows what it sees, the same frame.
///
/// Unity's `Camera.targetTexture`, Godot's camera inside a `SubViewport`.
/// A separate component, not a field of the camera, because a path is not
/// `Copy` and the camera is read by value everywhere.
///
/// What the texture shows is drawn before the screen's cameras; a sprite
/// showing it is left out of its own camera's picture, since sampling a
/// texture while drawing into it is not allowed.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CameraTarget {
    pub texture: AssetPath,
    /// What `texture` resolves to in this session's textures. Never
    /// written.
    #[serde(skip)]
    pub handle: Option<Handle<Texture>>,
}

impl Default for CameraTarget {
    /// A template to fill in: the file it names is the one to make.
    fn default() -> Self {
        Self::new(AssetPath::new("textures/view.rendertexture.ron").expect("a valid literal path"))
    }
}

impl CameraTarget {
    pub fn new(texture: AssetPath) -> Self {
        Self {
            texture,
            handle: None,
        }
    }
}

/// Resolves every camera's render texture in `world`: what a draw needs
/// before it can tell which cameras draw into what.
pub fn resolve_world(
    world: &mut voltra_ecs::World,
    textures: &mut voltra_assets::Textures,
    device: &voltra_render::wgpu::Device,
    queue: &voltra_render::wgpu::Queue,
) {
    for (_, target) in world.query_mut::<CameraTarget>() {
        target.handle = Some(textures.load(device, queue, &target.texture));
    }
}
