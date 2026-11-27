//! A light's cookie: its image resolved from a path, and the frame's table
//! of the cookies its lights wear. See
//! [`voltra_render::lights::Cookie`](voltra_render::Cookie) for what one
//! does.

use voltra_assets::{AssetPath, Handle, Textures};
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::{wgpu, Cookie, Texture};

use super::PointLight2D;

impl PointLight2D {
    /// Sets or clears the cookie's path and refreshes its handle, as
    /// [`Sprite2D::set_texture`](crate::Sprite2D::set_texture) does for a
    /// sprite: the same store, the image's colour read as colour.
    pub fn set_cookie(
        &mut self,
        path: Option<AssetPath>,
        textures: &mut Textures,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        self.cookie_handle = path.as_ref().map(|path| {
            let handle = textures.load(device, queue, path);
            if textures.is_broken(handle) {
                log::warn!(
                    "cookie {} did not load; the light shines plain until it does",
                    path.as_str()
                );
            }
            handle
        });
        self.cookie = path;
    }
}

/// The cookies the frame's lights wear, each image once, in the order first
/// worn.
pub(super) struct Cookies<'a> {
    /// `None` resolves nothing: every light shines plain, as a headless
    /// caller that never loads textures gets.
    textures: Option<&'a Textures>,
    worn: Vec<Handle<Texture>>,
}

impl<'a> Cookies<'a> {
    pub(super) fn new(textures: Option<&'a Textures>) -> Self {
        Self {
            textures,
            worn: Vec::new(),
        }
    }

    /// The cookie `light` wears, laid along its entity's axes by `matrix`
    /// (turned and mirrored with it); `None` without one, without textures,
    /// or for an image that would not load — drawn as the checker, it would
    /// stamp the checker on the floor.
    pub(super) fn wear(&mut self, light: &PointLight2D, matrix: Mat3) -> Option<Cookie> {
        let handle = light.cookie_handle?;
        if self.textures?.is_broken(handle) {
            return None;
        }
        let index = match self.worn.iter().position(|&worn| worn == handle) {
            Some(index) => index,
            None => {
                self.worn.push(handle);
                self.worn.len() - 1
            }
        };
        Some(Cookie {
            // As many as there are lights in view, far inside a `u32`.
            index: u32::try_from(index).unwrap_or(u32::MAX),
            x: matrix.transform_vector2(Vec2::X),
            y: matrix.transform_vector2(Vec2::Y),
        })
    }

    /// The images worn, by [`Cookie::index`], as the renderer binds them.
    pub(super) fn into_bind_groups(self) -> Vec<wgpu::BindGroup> {
        let Some(textures) = self.textures else {
            return Vec::new();
        };
        self.worn
            .into_iter()
            .map(|handle| textures.bind_group(handle).clone())
            .collect()
    }
}
