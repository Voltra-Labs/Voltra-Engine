//! A collider's shape: which kind, its sizes, a polygon's points, and a
//! polygon fitted to the sprite.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, ConvexPolygon};

use super::active;

/// The kinds a collider can be switched between, in the order offered.
const KINDS: [&str; 4] = ["Box", "Circle", "Capsule", "Polygon"];

/// What the panel asks of the inspector once it has drawn.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Outcome {
    /// The undo label of an edit made, or under way.
    pub claim: Option<&'static str>,
    /// The fit button was pressed: shape the collider as its sprite's
    /// silhouette, which only the inspector can trace.
    pub fit_to_sprite: bool,
}

/// The collider's kind and sizes. `textured_sprite` offers the fit.
pub(super) fn show(ui: &mut Ui, collider: &mut Collider, textured_sprite: bool) -> Outcome {
    let mut outcome = Outcome::default();
    ui.label(RichText::new("Collider").strong())
        .on_hover_text("the shape it collides as, in the entity's space");

    let mut kind = collider.kind();
    egui::ComboBox::from_id_salt("collider_kind")
        .selected_text(kind)
        .show_ui(ui, |ui| {
            for option in KINDS {
                ui.selectable_value(&mut kind, option, option);
            }
        });
    if kind != collider.kind() {
        *collider = converted(collider, kind);
        outcome.claim = Some("Change collider shape");
    }

    let claim = match collider {
        Collider::Box { half_extents } => pair(
            ui,
            "half size",
            half_extents,
            "the box's half width and height",
        ),
        Collider::Circle { radius } => length(ui, "radius", radius, "the circle's radius"),
        Collider::Capsule {
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
        Collider::Polygon { points, radius } => {
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

/// `collider` as a shape of `kind`, covering about what it covered.
fn converted(collider: &Collider, kind: &str) -> Collider {
    let extent = match collider {
        Collider::Box { half_extents } => *half_extents,
        Collider::Circle { radius } => Vec2::splat(*radius),
        Collider::Capsule {
            half_height,
            radius,
        } => Vec2::new(*radius, half_height.max(*radius)),
        Collider::Polygon { points, radius } => {
            points
                .points()
                .iter()
                .fold(Vec2::ZERO, |most, p| most.max(p.abs()))
                + Vec2::splat(*radius)
        }
    };
    let extent = if extent.x > 0.0 && extent.y > 0.0 && extent.is_finite() {
        extent
    } else {
        Vec2::splat(voltra_scene::Sprite::HALF_EXTENT)
    };
    match kind {
        "Circle" => Collider::Circle {
            radius: extent.max_element(),
        },
        "Capsule" => Collider::Capsule {
            half_height: extent.y.max(extent.x),
            radius: extent.x,
        },
        "Polygon" => match ConvexPolygon::rectangle(extent) {
            Ok(points) => Collider::Polygon {
                points,
                radius: 0.0,
            },
            Err(_) => Collider::default(),
        },
        _ => Collider::Box {
            half_extents: extent,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_core::egui::Context;

    #[test]
    fn drawing_every_kind_changes_nothing_by_itself() {
        let kinds = KINDS.map(|kind| converted(&Collider::default(), kind));
        for textured in [false, true] {
            for kind in kinds {
                let mut collider = kind;
                let mut outcome = Outcome {
                    claim: Some("unset"),
                    fit_to_sprite: true,
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
    fn switching_kind_keeps_about_the_same_size() {
        let wide = Collider::Box {
            half_extents: Vec2::new(1.0, 2.0),
        };
        assert_eq!(
            converted(&wide, "Capsule"),
            Collider::Capsule {
                half_height: 2.0,
                radius: 1.0
            }
        );
        assert_eq!(converted(&wide, "Circle"), Collider::Circle { radius: 2.0 });
        let polygon = converted(&wide, "Polygon");
        assert_eq!(converted(&polygon, "Box"), wide);
        assert_eq!(polygon.kind(), "Polygon");
    }

    #[test]
    fn a_collider_of_no_size_converts_to_a_sprites_size() {
        let nothing = Collider::Circle { radius: 0.0 };
        assert_eq!(converted(&nothing, "Box"), Collider::default());
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
