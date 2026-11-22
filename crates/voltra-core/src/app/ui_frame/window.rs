//! The window, as far as a UI may reach it: its title, and closing.

use super::UiFrame;

impl UiFrame<'_> {
    /// Asks for the window's title; set after the layout, and only when it
    /// changed.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.window.set_title(title.into());
    }

    /// Whether the person asked to close the window and is waiting for an
    /// answer — only ever with [`App::with_close_guard`]: without it a close
    /// is immediate. Answer with [`close`](Self::close) or
    /// [`cancel_close`](Self::cancel_close).
    ///
    /// [`App::with_close_guard`]: crate::App::with_close_guard
    pub fn close_requested(&self) -> bool {
        self.window.close_requested()
    }

    /// Closes the window after this frame, asked or not.
    pub fn close(&mut self) {
        self.window.close();
    }

    /// Keeps the window open after all.
    pub fn cancel_close(&mut self) {
        self.window.cancel_close();
    }
}
