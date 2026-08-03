//! Window description, kept separate from the event loop that consumes it.

use winit::dpi::LogicalSize;
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
    pub(crate) fn to_attributes(&self) -> WindowAttributes {
        winit::window::Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(LogicalSize::new(self.width, self.height))
    }
}
