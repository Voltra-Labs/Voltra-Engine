//! How the inspector shows a field that differs across the selection.
//!
//! Every section is drawn on the primary's value, so a field the
//! selection disagrees on would otherwise look settled. Two levels, both
//! read from [`MultiEdit`]:
//!
//! - **Every section** gets [`marker`]: a line naming its fields that
//!   differ, for built-in and game components alike, whatever widgets the
//!   section uses.
//! - **The shared widgets** here show the mixed state in the row itself
//!   — a number as `—` ([`drag`]), a checkbox half-ticked ([`checkbox`]) —
//!   and where a widget has no such state (a colour button, a dropdown, a
//!   path), the row's label says so ([`label`]) next to the primary's
//!   value. A section opts in by taking a [`Fields`].

use voltra_core::egui::{Checkbox, DragValue, Response, RichText, Ui};
use voltra_scene::format::serial_tree::{describe, Step};

use crate::multi_edit::MultiEdit;

/// What a mixed field's widget or label says on hover.
const MIXED: &str = "differs across the selection: the last picked's value is shown, \
                     and an edit writes this field to all of them";

/// One component's mixed fields, for its section's widgets to ask.
#[derive(Clone, Copy)]
pub(super) struct Fields<'a> {
    multi: &'a MultiEdit,
    name: &'static str,
}

impl<'a> Fields<'a> {
    /// `name`'s fields as `multi` found them.
    pub(super) fn of(multi: &'a MultiEdit, name: &'static str) -> Self {
        Self { multi, name }
    }

    /// Whether anything at or under `path` differs.
    pub(super) fn at(&self, path: &[Step]) -> bool {
        self.multi.is_mixed(self.name, path)
    }

    /// Whether the top-level field `field` differs.
    pub(super) fn field(&self, field: &'static str) -> bool {
        self.at(&[Step::Field(field)])
    }

    /// Whether element `index` of the top-level field `field` differs —
    /// one axis of a vector, one channel of a colour.
    pub(super) fn element(&self, field: &'static str, index: usize) -> bool {
        self.at(&[Step::Field(field), Step::Index(index)])
    }
}

/// `value`, showing `—` instead of the primary's number when `mixed`.
/// Dragged or typed into, it writes a number like any other, and the field
/// stops being mixed.
pub(super) fn drag(value: DragValue<'_>, mixed: bool) -> DragValue<'_> {
    if mixed {
        value.custom_formatter(|_, _| "—".to_owned())
    } else {
        value
    }
}

/// A checkbox, half-ticked when `mixed`.
pub(super) fn checkbox(ui: &mut Ui, value: &mut bool, text: &str, mixed: bool) -> Response {
    let response = ui.add(Checkbox::new(value, text).indeterminate(mixed));
    if mixed {
        response.on_hover_text(MIXED)
    } else {
        response
    }
}

/// A row's label, marked when its field is `mixed` — for a widget that
/// has no mixed state of its own and shows the primary's value.
pub(super) fn label(ui: &mut Ui, text: &str, mixed: bool) -> Response {
    if mixed {
        ui.label(RichText::new(format!("{text} ≠")).italics())
            .on_hover_text(MIXED)
    } else {
        ui.label(text)
    }
}

/// A line naming the fields of `names` that differ across the selection;
/// nothing when none does.
pub(super) fn marker(ui: &mut Ui, multi: &MultiEdit, names: &[&str]) {
    for name in names {
        let places = multi.mixed(name);
        if places.is_empty() {
            continue;
        }
        let mut fields: Vec<String> = Vec::new();
        for place in places {
            let field = top_field(place).unwrap_or(name).to_owned();
            if !fields.contains(&field) {
                fields.push(field);
            }
        }
        let every: Vec<String> = places
            .iter()
            .map(|place| match describe(place) {
                path if path.is_empty() => format!("{name} itself"),
                path => path,
            })
            .collect();
        ui.label(
            RichText::new(format!("≠ {name}: {}", fields.join(", ")))
                .italics()
                .weak()
                .small(),
        )
        .on_hover_text(format!("{MIXED}.\n{}", every.join("\n")));
    }
}

/// The first named field on `place`, the one its section has a row for.
fn top_field(place: &[Step]) -> Option<&'static str> {
    place.iter().find_map(|step| match step {
        Step::Field(name) => Some(*name),
        _ => None,
    })
}
