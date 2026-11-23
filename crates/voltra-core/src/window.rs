//! Window description, kept separate from the event loop that consumes it.

use winit::window::WindowAttributes;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Voltra Engine".into(),
            width: 1280,
            height: 720,
        }
    }
}

impl WindowConfig {
    /// A desktop window of `width` × `height` logical pixels.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn to_attributes(&self) -> WindowAttributes {
        winit::window::Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(self.width, self.height))
    }

    /// A canvas appended to the page, sized by the page.
    ///
    /// The size is left out on purpose: winit writes a requested size into
    /// the canvas's style, which would pin it against the page's CSS. The
    /// page lays the canvas out, winit's resize observer reports the size it
    /// got, and the frame follows — the window size a manifest names is a
    /// desktop setting. The title is the page's, since a canvas has none.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn to_attributes(&self) -> WindowAttributes {
        use winit::platform::web::WindowAttributesExtWebSys;

        winit::window::Window::default_attributes()
            .with_title(self.title.clone())
            .with_append(true)
    }
}
