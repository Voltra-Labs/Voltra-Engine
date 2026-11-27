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
//!
//! **Choosing what the primary already has** moves nothing on it, so the
//! difference [`MultiEdit::finish`] looks for is not there. A widget that
//! commits a choice says so with [`touch`] — a number typed and entered, a
//! box ticked, a dropdown's entry picked — and the field is written to all
//! the same, as Unity does. The touches are kept in egui's memory for the
//! frame, so any section can leave one without being handed the edit.

use voltra_core::egui::{Checkbox, DragValue, Id, Key, Response, RichText, Ui};
use voltra_scene::format::serial_tree::{describe, Step, TreePath};

use crate::multi_edit::{MultiEdit, Touch};

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

    /// Says `path` of this component was committed this frame.
    pub(super) fn touch(&self, ui: &Ui, path: TreePath) {
        touch(ui, self.name, path);
    }

    /// Whether anything at or under `path` differs.
    pub(super) fn at(&self, path: &[Step]) -> bool {
        self.multi.is_mixed(self.name, path)
    }

    /// Whether the top-level field `field` differs.
    pub(super) fn field(&self, field: &'static str) -> bool {
        self.at(&[Step::Field(field)])
    }
}

/// Where this frame's touches wait for the inspector to collect them.
fn touched_id() -> Id {
    Id::new("inspector_multi_edit_touched")
}

/// One touch: the component, the place, and how it is written.
type Touched = (&'static str, TreePath, Touch);

/// Says the field at `path` of component `name` was committed this frame,
/// for [`MultiEdit::touch`]: written to the whole selection even if the
/// primary already had the value chosen.
pub(super) fn touch(ui: &Ui, name: &'static str, path: TreePath) {
    push(ui, (name, path, Touch::Value));
}

/// Says the enum at `path` of component `name` was picked this frame: each
/// selected entity of another variant becomes the primary's, and each
/// already of it keeps its own parameters — never the whole component.
pub(super) fn touch_variant(ui: &Ui, name: &'static str, path: TreePath) {
    push(ui, (name, path, Touch::Variant));
}

fn push(ui: &Ui, touched: Touched) {
    ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<Vec<Touched>>(touched_id())
            .push(touched);
    });
}

/// This frame's touches, taken: the inspector hands them to the edit.
pub(super) fn take_touched(ui: &Ui) -> Vec<Touched> {
    ui.ctx().data_mut(|data| {
        data.remove_temp::<Vec<Touched>>(touched_id())
            .unwrap_or_default()
    })
}

/// `response`, a dropdown entry or a selectable button for `path` of
/// component `name`, touched when clicked: picking what the primary
/// already has still sets it on all.
pub(super) fn chosen(ui: &Ui, response: Response, name: &'static str, path: TreePath) -> Response {
    if response.clicked() {
        touch(ui, name, path);
    }
    response
}

/// [`chosen`] for an entry that picks an enum's variant: see
/// [`touch_variant`].
pub(super) fn chosen_variant(
    ui: &Ui,
    response: Response,
    name: &'static str,
    path: TreePath,
) -> Response {
    if response.clicked() {
        touch_variant(ui, name, path);
    }
    response
}

/// A number at `path` of `fields`' component, showing `—` instead of the
/// primary's when it is mixed. Dragged or typed into, it writes a number
/// like any other; a number typed into the `—` is written to all once the
/// field is left — Enter, Tab or a click away — even when it is the
/// primary's own. Clicked into and left untouched (still `—`), or left
/// with Escape, it writes nothing.
pub(super) fn drag(
    ui: &mut Ui,
    value: DragValue<'_>,
    fields: Fields<'_>,
    path: TreePath,
) -> Response {
    let mixed = fields.at(&path);
    let value = if mixed {
        value.custom_formatter(|_, _| "—".to_owned())
    } else {
        value
    };
    // The id the `DragValue` takes: it keeps the text being typed under it
    // (egui-0.35 `widgets/drag_value.rs`, `next_auto_id` and the `String`
    // temp), and drops it the frame the field is left.
    let id = ui.next_auto_id();
    let response = ui.add(value);
    if mixed && typed_and_left(ui, id) {
        fields.touch(ui, path);
        return response;
    }
    if mixed {
        response.on_hover_text(MIXED)
    } else {
        response
    }
}

/// Whether the `DragValue` at `id` was just left with a number typed into
/// it. The text is copied aside while it is being typed, since egui drops
/// its own the frame focus goes; text aside and no focus is a field left.
fn typed_and_left(ui: &Ui, id: Id) -> bool {
    let aside = id.with("mixed_typed");
    if let Some(text) = ui.data(|data| data.get_temp::<String>(id)) {
        ui.data_mut(|data| data.insert_temp(aside, text));
    }
    if ui.memory(|memory| memory.has_focus(id)) {
        return false;
    }
    let typed = ui.data_mut(|data| data.remove_temp::<String>(aside));
    let escaped = ui.input(|i| i.key_pressed(Key::Escape));
    !escaped && typed.is_some_and(|text| text.trim().parse::<f64>().is_ok())
}

/// A checkbox at `path` of `fields`' component, half-ticked when mixed;
/// a click is a commit.
pub(super) fn checkbox(
    ui: &mut Ui,
    value: &mut bool,
    text: &str,
    fields: Fields<'_>,
    path: TreePath,
) -> Response {
    let mixed = fields.at(&path);
    let response = ui.add(Checkbox::new(value, text).indeterminate(mixed));
    if response.changed() {
        fields.touch(ui, path);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_core::egui::Context;
    use voltra_ecs::World;
    use voltra_render::glam::Vec2;
    use voltra_scene::{ComponentRegistry, Sprite2D, Transform2D};

    #[test]
    fn drawing_mixed_sections_changes_nothing_by_itself() {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let here = world.spawn();
        let there = world.spawn();
        world.insert(here, Transform2D::from_translation(Vec2::new(1.0, 2.0)));
        world.insert(there, Transform2D::from_translation(Vec2::new(3.0, 2.0)));
        world.insert(here, Sprite2D::new([1.0, 0.0, 0.0, 1.0]));
        let mut flipped = Sprite2D::new([0.0, 1.0, 0.0, 1.0]);
        flipped.flip_x = true;
        world.insert(there, flipped);
        let multi = MultiEdit::begin(&world, &registry, there, [here, there]);
        let fields = Fields::of(&multi, "Transform2D");
        assert!(fields.at(&[Step::Field("translation"), Step::Index(0)]));
        assert!(!fields.at(&[Step::Field("translation"), Step::Index(1)]));
        assert!(Fields::of(&multi, "Sprite2D").field("flip_x"));

        let ctx = Context::default();
        let mut transform = *world.get::<Transform2D>(there).expect("t");
        let mut flip = true;
        for _ in 0..2 {
            let _ = ctx.run_ui(Default::default(), |ui| {
                marker(ui, &multi, &["Transform2D", "Sprite2D"]);
                let _ = super::super::transform::show(ui, &mut transform, fields);
                let sprite = Fields::of(&multi, "Sprite2D");
                let _ = checkbox(ui, &mut flip, "x", sprite, vec![Step::Field("flip_x")]);
                let _ = label(ui, "colour", true);
            });
        }
        assert_eq!(Some(&transform), world.get::<Transform2D>(there));
        assert!(flip);
        let _ = ctx.run_ui(Default::default(), |ui| {
            assert!(take_touched(ui).is_empty(), "nothing committed");
            touch(ui, "Sprite2D", vec![Step::Field("flip_x")]);
            touch_variant(ui, "Collider2D", Vec::new());
            assert_eq!(
                take_touched(ui),
                [
                    ("Sprite2D", vec![Step::Field("flip_x")], Touch::Value),
                    ("Collider2D", Vec::new(), Touch::Variant),
                ]
            );
            assert!(take_touched(ui).is_empty(), "taken");
        });
        assert!(multi.finish(&mut world, &registry).is_empty());
    }
}
