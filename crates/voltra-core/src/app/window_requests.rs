//! What a UI asks of the window it cannot reach: its title, and whether a
//! close the person asked for may go ahead.
//!
//! The window and the event loop stay `App`'s. A UI writes its wishes here
//! through `UiFrame`, and `App` carries them out after the layout — the
//! same shape as egui's viewport commands (`Title`, `CancelClose`), and for
//! the same reason: a callback inside the frame must not hold the window.

/// The UI's pending requests, and the close it is being asked about.
#[derive(Debug, Default)]
pub(crate) struct WindowRequests {
    /// The title last asked for, until `App` applies it.
    title: Option<String>,
    /// The title the window shows, so an unchanged one is not set again.
    applied: String,
    /// Whether a close goes to the UI first: set by `App::with_close_guard`.
    guarded: bool,
    /// The person asked to close, and the UI has not answered yet.
    asked: bool,
    /// The UI agreed to close — or closed of its own accord.
    confirmed: bool,
}

impl WindowRequests {
    pub(crate) fn guard(&mut self) {
        self.guarded = true;
    }

    /// The person asked to close. Answers whether to close now: unguarded,
    /// or with no UI to ask, at once; otherwise once the UI agrees.
    pub(crate) fn on_close_requested(&mut self, has_ui: bool) -> bool {
        if !self.guarded || !has_ui {
            return true;
        }
        self.asked = true;
        false
    }

    pub(crate) fn set_title(&mut self, title: String) {
        if title != self.applied {
            self.title = Some(title);
        }
    }

    /// The title to give the window, once.
    pub(crate) fn take_title(&mut self) -> Option<String> {
        let title = self.title.take()?;
        self.applied.clone_from(&title);
        Some(title)
    }

    pub(crate) fn close_requested(&self) -> bool {
        self.asked
    }

    pub(crate) fn close(&mut self) {
        self.confirmed = true;
        self.asked = false;
    }

    pub(crate) fn cancel_close(&mut self) {
        self.asked = false;
    }

    /// Whether the loop should end now.
    pub(crate) fn should_exit(&self) -> bool {
        self.confirmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unguarded_close_goes_at_once_and_a_guarded_one_waits_for_the_ui() {
        let mut open = WindowRequests::default();
        assert!(open.on_close_requested(true));

        let mut guarded = WindowRequests::default();
        guarded.guard();
        assert!(!guarded.on_close_requested(true));
        assert!(guarded.close_requested());
        guarded.cancel_close();
        assert!(!guarded.close_requested() && !guarded.should_exit());

        guarded.on_close_requested(true);
        guarded.close();
        assert!(guarded.should_exit());

        // Nothing to ask: a guard with no UI must not trap the window.
        let mut headless = WindowRequests::default();
        headless.guard();
        assert!(headless.on_close_requested(false));
    }

    #[test]
    fn a_title_is_handed_over_once_and_not_again_while_unchanged() {
        let mut requests = WindowRequests::default();
        requests.set_title("Editor — a.ron".into());
        assert_eq!(requests.take_title().as_deref(), Some("Editor — a.ron"));
        assert_eq!(requests.take_title(), None);
        requests.set_title("Editor — a.ron".into());
        assert_eq!(requests.take_title(), None, "unchanged");
        requests.set_title("Editor — a.ron*".into());
        assert!(requests.take_title().is_some());
    }
}
