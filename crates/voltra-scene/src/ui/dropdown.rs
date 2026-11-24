//! A dropdown: one choice of a list, the list opening over everything.

use super::rect::Rect;

/// A control showing one of its [`options`](Self::options); a click, or
/// accept on the focus, opens the list over every other element, where the
/// pointer or the up and down navigation picks another.
///
/// Turns the [`UiButton`](super::UiButton) on the same entity into a
/// dropdown, as a [`UiToggle`](super::UiToggle) turns one into a checkbox:
/// the box keeps the button's hover, press, focus and colours (and a
/// theme's button style). Godot's `OptionButton` is a `Button` too; Unity's
/// `Dropdown` is a `Selectable`. The entity's `Text` is the
/// style of the shown choice and of the list's items — font, size, colour —
/// and the control writes the chosen option into its content; its
/// `Sprite`, if any, is the box. The list is drawn by the UI pass itself,
/// after every element, rather than spawned as entities: it is on top of
/// everything without a sort order to manage, and nothing is left behind
/// when a scene is saved with it open.
///
/// While the list is open it has the navigation: up and down move the
/// highlight, accept picks it, cancel (`ui_cancel`) or a click outside
/// closes the list unchanged. A pick that changes the choice is reported
/// as a change, as a slider's move is.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiDropdown {
    /// The choices, in the order the list shows them.
    #[serde(default)]
    pub options: Vec<String>,
    /// The chosen option's index.
    #[serde(default)]
    pub selected: usize,
    /// One item's height in the list, in points; `None` — the default, and
    /// not written — is the dropdown's own height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_height: Option<f32>,
    /// The most items the list shows at once; the highlight scrolls it.
    #[serde(default = "eight")]
    pub max_visible: usize,
    /// Behind the list's items.
    #[serde(default = "list_color")]
    pub list_color: [f32; 4],
    /// Behind the highlighted item.
    #[serde(default = "highlight_color")]
    pub highlight_color: [f32; 4],
    /// Whether the list is open, as the last update left it.
    #[serde(skip)]
    pub open: bool,
    /// The item the pointer or the navigation is on while open.
    #[serde(skip)]
    pub highlighted: usize,
    /// The first item shown while open, when there are more than fit.
    #[serde(skip)]
    pub first: usize,
}

fn eight() -> usize {
    8
}

fn list_color() -> [f32; 4] {
    [0.1, 0.11, 0.15, 0.98]
}

fn highlight_color() -> [f32; 4] {
    [0.25, 0.45, 0.9, 1.0]
}

impl Default for UiDropdown {
    fn default() -> Self {
        Self {
            options: vec!["One".to_owned(), "Two".to_owned(), "Three".to_owned()],
            selected: 0,
            item_height: None,
            max_visible: eight(),
            list_color: list_color(),
            highlight_color: highlight_color(),
            open: false,
            highlighted: 0,
            first: 0,
        }
    }
}

/// Where an open list lands, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListBox {
    /// The whole list.
    pub rect: Rect,
    /// One item's height.
    pub item: f32,
    /// How many items show.
    pub shown: usize,
}

impl ListBox {
    /// Item `row` (counted from the first shown) of the list.
    pub fn row(&self, row: usize) -> Rect {
        let top = self.rect.min.y + row as f32 * self.item;
        Rect::new(
            voltra_render::glam::Vec2::new(self.rect.min.x, top),
            voltra_render::glam::Vec2::new(self.rect.max.x, top + self.item),
        )
    }

    /// The row `point` is on, if it is on the list.
    pub fn row_at(&self, point: voltra_render::glam::Vec2) -> Option<usize> {
        if !self.rect.contains(point) || self.item <= 0.0 {
            return None;
        }
        let row = ((point.y - self.rect.min.y) / self.item) as usize;
        (row < self.shown).then_some(row)
    }
}

impl UiDropdown {
    /// The chosen option's text; empty with no options.
    pub fn selected_text(&self) -> &str {
        self.options.get(self.selected).map_or("", String::as_str)
    }

    /// Where the list opens for a dropdown at `rect` on `screen`: under it,
    /// or over it when there is more room above. `None` with no options.
    pub fn list(&self, rect: Rect, screen: Rect) -> Option<ListBox> {
        if self.options.is_empty() {
            return None;
        }
        let item = self
            .item_height
            .filter(|h| h.is_finite() && *h > 0.0)
            .unwrap_or(rect.size().y)
            .max(1.0);
        let shown = self.options.len().min(self.max_visible.max(1));
        let height = item * shown as f32;
        let below = screen.max.y - rect.max.y;
        let above = rect.min.y - screen.min.y;
        let top = if below >= height || below >= above {
            rect.max.y
        } else {
            rect.min.y - height
        };
        let min = voltra_render::glam::Vec2::new(rect.min.x, top);
        let max = voltra_render::glam::Vec2::new(rect.max.x, top + height);
        Some(ListBox {
            rect: Rect::new(min, max),
            item,
            shown,
        })
    }

    /// Opens the list on the chosen option.
    pub fn open_list(&mut self) {
        self.open = !self.options.is_empty();
        self.highlighted = self.selected.min(self.options.len().saturating_sub(1));
        self.first = 0;
        self.reveal();
    }

    /// Moves the highlight `by` items, kept on the list.
    pub fn step(&mut self, by: i32) {
        let last = self.options.len().saturating_sub(1) as i64;
        self.highlighted = (self.highlighted as i64 + i64::from(by)).clamp(0, last) as usize;
        self.reveal();
    }

    /// Picks the highlighted option and closes the list; whether the choice
    /// changed.
    pub fn pick(&mut self) -> bool {
        self.open = false;
        let changed = self.highlighted != self.selected && self.highlighted < self.options.len();
        if changed {
            self.selected = self.highlighted;
        }
        changed
    }

    /// Scrolls the shown window so the highlight is in it.
    fn reveal(&mut self) {
        let shown = self.max_visible.max(1);
        if self.highlighted < self.first {
            self.first = self.highlighted;
        } else if self.highlighted >= self.first + shown {
            self.first = self.highlighted + 1 - shown;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;

    fn screen() -> Rect {
        Rect::new(Vec2::ZERO, Vec2::new(400.0, 300.0))
    }

    #[test]
    fn the_list_opens_below_and_flips_above_near_the_bottom() {
        let dropdown = UiDropdown::default();
        let high = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(110.0, 30.0));
        let list = dropdown.list(high, screen()).expect("options");
        assert_eq!(list.rect.min.y, 30.0);
        assert_eq!(list.rect.size().y, 60.0, "three items of the box's height");

        let low = Rect::new(Vec2::new(10.0, 270.0), Vec2::new(110.0, 290.0));
        let list = dropdown.list(low, screen()).expect("options");
        assert_eq!(list.rect.max.y, 270.0, "over it");
    }

    #[test]
    fn rows_are_found_under_the_pointer() {
        let dropdown = UiDropdown::default();
        let rect = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(110.0, 30.0));
        let list = dropdown.list(rect, screen()).expect("options");
        assert_eq!(list.row_at(Vec2::new(50.0, 35.0)), Some(0));
        assert_eq!(list.row_at(Vec2::new(50.0, 75.0)), Some(2));
        assert_eq!(list.row_at(Vec2::new(50.0, 95.0)), None);
    }

    #[test]
    fn the_highlight_stays_on_the_list_and_scrolls_it() {
        let mut dropdown = UiDropdown {
            options: (0..20).map(|i| i.to_string()).collect(),
            max_visible: 5,
            ..UiDropdown::default()
        };
        dropdown.open_list();
        dropdown.step(-3);
        assert_eq!(dropdown.highlighted, 0);
        dropdown.step(7);
        assert_eq!((dropdown.highlighted, dropdown.first), (7, 3));
        dropdown.step(100);
        assert_eq!(dropdown.highlighted, 19);
        assert!(dropdown.pick());
        assert_eq!(dropdown.selected, 19);
        assert!(!dropdown.open);
    }

    #[test]
    fn no_options_never_opens() {
        let mut dropdown = UiDropdown {
            options: Vec::new(),
            ..UiDropdown::default()
        };
        dropdown.open_list();
        assert!(!dropdown.open);
        assert_eq!(dropdown.selected_text(), "");
        assert!(!dropdown.pick());
    }
}
