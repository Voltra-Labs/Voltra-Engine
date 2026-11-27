//! A collider's shape: which kind, its sizes, a polygon's points, and a
//! polygon fitted to the sprite.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider2D, ConvexPolygon};

mod kind;

use super::active;
use kind::{converted, KINDS};

/// What the panel asks of the inspector once it has drawn.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Outcome {
    /// The undo label of an edit made, or under way.
    pub claim: Option<&'static str>,
    /// The fit button was pressed: shape the collider as its sprite's
    /// silhouette, which only the inspector can trace.
    pub fit_to_sprite: bool,
    /// A kind picked from the dropdown, changed or not: the inspector
    /// turns every selected collider of another kind into it, each from
    /// its own size ([`reshape`]), rather than copy the primary's.
    pub kind: Option<&'static str>,
}

/// `collider` turned into `kind` at about the size it had; whether it was
/// another kind.
pub(super) fn reshape(collider: &mut Collider2D, kind: &str) -> bool {
    if collider.kind() == kind {
        return false;
    }
    *collider = converted(collider, kind);
    true
}

/// The collider's kind and sizes. `textured_sprite` offers the fit.
pub(super) fn show(ui: &mut Ui, collider: &mut Collider2D, textured_sprite: bool) -> Outcome {
    let mut outcome = Outcome::default();
    ui.label(RichText::new(Collider2D::NAME).strong())
        .on_hover_text("the shape it collides as, in the entity's space");

    let mut kind = collider.kind();
    egui::ComboBox::from_id_salt("collider_kind")
        .selected_text(kind)
        .show_ui(ui, |ui| {
            for option in KINDS {
                if ui.selectable_value(&mut kind, option, option).clicked() {
                    outcome.kind = Some(option);
                }
            }
        });
    if kind != collider.kind() {
        *collider = converted(collider, kind);
        outcome.claim = Some("Change collider shape");
    }

    let claim = match collider {
        Collider2D::Box { half_extents } => pair(
            ui,
            "half size",
            half_extents,
            "the box's half width and height",
        ),
        Collider2D::Circle { radius } => length(ui, "radius", radius, "the circle's radius"),
        Collider2D::Capsule {
            half_height,
            radius,
        } => {
            let claim = length(
                ui,
                "half height",
                half_height,
                "half the whole height, caps included; below the radius it is a circle",
            );
            claim.or(length(
                ui,
                "radius",
                radius,
                "the caps' radius, and half the width",
            ))
        }
        Collider2D::Polygon { points, radius } => {
            let claim = length(
                ui,
                "rounding",
                radius,
                "how far the surface stands off the points: zero is sharp corners",
            );
            claim.or(polygon(ui, points))
        }
    };
    outcome.claim = outcome.claim.or(claim);

    if textured_sprite
        && ui
            .button("Fit to sprite")
            .on_hover_text("a polygon around the sprite's opaque pixels, at most eight corners")
            .clicked()
    {
        outcome.fit_to_sprite = true;
    }
    outcome
}

fn length(ui: &mut Ui, label: &str, value: &mut f32, hint: &str) -> Option<&'static str> {
    ui.horizontal(|ui| {
        ui.label(label).on_hover_text(hint);
        active(
            &ui.add(DragValue::new(value).speed(0.01).range(0.0..=f32::MAX)),
            "Edit collider",
        )
    })
    .inner
}

fn pair(ui: &mut Ui, label: &str, value: &mut Vec2, hint: &str) -> Option<&'static str> {
    ui.horizontal(|ui| {
        ui.label(label).on_hover_text(hint);
        let x = ui.add(
            DragValue::new(&mut value.x)
                .speed(0.01)
                .range(0.0..=f32::MAX)
                .prefix("x "),
        );
        let y = ui.add(
            DragValue::new(&mut value.y)
                .speed(0.01)
                .range(0.0..=f32::MAX)
                .prefix("y "),
        );
        active(&x, "Edit collider").or(active(&y, "Edit collider"))
    })
    .inner
}

/// One row per point, a button to drop one and one to add one. An edit
/// that would leave the outline concave — a point dragged inside — stops at
/// the last convex one, so a point never vanishes from under the pointer.
fn polygon(ui: &mut Ui, points: &mut ConvexPolygon) -> Option<&'static str> {
    let mut edited: Vec<Vec2> = points.points().to_vec();
    let mut claim = None;
    let mut remove = None;
    egui::Grid::new("collider_points")
        .num_columns(3)
        .show(ui, |ui| {
            let can_remove = edited.len() > 3;
            for (i, point) in edited.iter_mut().enumerate() {
                ui.label(i.to_string());
                ui.horizontal(|ui| {
                    let x = ui.add(DragValue::new(&mut point.x).speed(0.01).prefix("x "));
                    let y = ui.add(DragValue::new(&mut point.y).speed(0.01).prefix("y "));
                    claim = claim
                        .or(active(&x, "Edit collider point"))
                        .or(active(&y, "Edit collider point"));
                });
                if ui
                    .add_enabled(can_remove, egui::Button::new("✕"))
                    .on_hover_text("remove this point; a polygon keeps at least three")
                    .clicked()
                {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = remove {
        edited.remove(i);
        claim = Some("Remove collider point");
    }
    if edited.len() < ConvexPolygon::MAX_POINTS
        && ui
            .button("Add point")
            .on_hover_text("splits the longest edge, its middle pushed out")
            .clicked()
    {
        split_longest_edge(&mut edited);
        claim = Some("Add collider point");
    }
    if edited.as_slice() != points.points() {
        let keeps_every_point = remove.is_some() || edited.len() >= points.points().len();
        match ConvexPolygon::new(&edited) {
            Ok(polygon) if polygon.points().len() == edited.len() || !keeps_every_point => {
                *points = polygon;
            }
            // Concave, or flattened: the last convex outline stands.
            _ => {}
        }
    }
    claim
}

/// Inserts a point in the middle of the longest edge, pushed out by a
/// tenth of its length so the outline stays convex and grows a corner.
fn split_longest_edge(points: &mut Vec<Vec2>) {
    let n = points.len();
    let Some(i) = (0..n).max_by(|&a, &b| {
        let length = |i: usize| points[i].distance_squared(points[(i + 1) % n]);
        length(a).total_cmp(&length(b))
    }) else {
        return;
    };
    let (a, b) = (points[i], points[(i + 1) % n]);
    let edge = b - a;
    let outward = Vec2::new(edge.y, -edge.x) * 0.1;
    points.insert(i + 1, (a + b) * 0.5 + outward);
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_core::egui::Context;

    #[test]
    fn drawing_every_kind_changes_nothing_by_itself() {
        let kinds = KINDS.map(|kind| converted(&Collider2D::default(), kind));
        for textured in [false, true] {
            for kind in kinds {
                let mut collider = kind;
                let mut outcome = Outcome {
                    claim: Some("unset"),
                    fit_to_sprite: true,
                    kind: Some("unset"),
                };
                let ctx = Context::default();
                let _ = ctx.run_ui(Default::default(), |ui| {
                    outcome = show(ui, &mut collider, textured);
                });

                assert_eq!(outcome, Outcome::default(), "{}", kind.kind());
                assert_eq!(collider, kind);
            }
        }
    }

    #[test]
    fn a_kind_picked_again_keeps_each_colliders_own_size() {
        use crate::multi_edit::MultiEdit;
        use voltra_ecs::World;
        use voltra_scene::ComponentRegistry;

        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let boxed = |x: f32| Collider2D::Box {
            half_extents: Vec2::splat(x),
        };
        let entities: Vec<_> = [boxed(1.0), Collider2D::Circle { radius: 2.0 }, boxed(3.0)]
            .into_iter()
            .map(|collider| {
                let e = world.spawn();
                world.insert(e, collider);
                e
            })
            .collect();
        let mut multi = MultiEdit::begin(&world, &registry, entities[2], entities.clone());
        // "Box" picked again on the primary, as the inspector does it.
        for &each in &entities {
            if let Some(collider) = world.get_mut::<Collider2D>(each) {
                reshape(collider, "Box");
            }
        }
        multi.done_for_each("Collider");
        assert!(multi.finish(&mut world, &registry).is_empty());
        let sizes: Vec<_> = entities
            .iter()
            .map(|&e| world.get::<Collider2D>(e).copied())
            .collect();
        assert_eq!(
            sizes,
            [Some(boxed(1.0)), Some(boxed(2.0)), Some(boxed(3.0))]
        );
    }

    #[test]
    fn splitting_an_edge_keeps_the_outline_convex_with_one_more_corner() {
        let mut points = ConvexPolygon::rectangle(Vec2::new(2.0, 0.5))
            .expect("a box")
            .points()
            .to_vec();
        split_longest_edge(&mut points);
        let polygon = ConvexPolygon::new(&points).expect("still convex");

        assert_eq!(polygon.points().len(), 5);
    }
}
