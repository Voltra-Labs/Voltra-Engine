//! Whether an entity blocks the light of lights that cast shadows, and the
//! shape it blocks with.

use voltra_core::egui::{Button, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{CasterShape, ShadowCaster2D};

use crate::shape_edit::ShapeEdit;

/// What the section knows about the entity beyond its caster.
pub(super) struct Context<'a> {
    /// It has a collider or is a solid layer: a caster can take that shape.
    pub has_collider: bool,
    /// The polygon it would start from (its collider in its own space, else
    /// its sprite's quad), if it has either.
    pub starting: Option<Vec<Vec2>>,
    pub shape: &'a mut ShapeEdit,
}

/// The caster toggle and its settings, for an entity that has a collider or
/// a sprite, is a solid layer, or already casts.
///
/// Takes the world rather than a `&mut` component, as the collision filter
/// does: the caster is absent by default, and ticking the box is what adds
/// it — shaped by the collider when there is one, else by the sprite's quad
/// as a polygon. A polygon's points are edited in the viewport: "Edit
/// shape" turns that mode on (see [`crate::shape_edit`]).
pub(super) fn show(
    ui: &mut Ui,
    world: &mut World,
    entity: Entity,
    context: Context<'_>,
) -> Option<&'static str> {
    let Context {
        has_collider,
        starting,
        shape,
    } = context;
    ui.label(RichText::new("Shadows").strong())
        .on_hover_text("blocks the light of lights that cast shadows");
    let mut claim = None;

    let mut casts = world.get::<ShadowCaster2D>(entity).is_some();
    // A checkbox is one click, so `changed` is the whole edit.
    if ui.checkbox(&mut casts, "casts shadows").changed() {
        if casts {
            let caster = match (has_collider, starting.clone()) {
                (false, Some(points)) => ShadowCaster2D {
                    shape: CasterShape::Polygon {
                        points,
                        closed: true,
                    },
                    self_shadow: false,
                },
                _ => ShadowCaster2D::default(),
            };
            world.insert(entity, caster);
        } else {
            world.remove::<ShadowCaster2D>(entity);
            shape.set_editing(None);
        }
        claim = Some("Toggle shadow caster");
    }
    let Some(caster) = world.get_mut::<ShadowCaster2D>(entity) else {
        return claim;
    };
    if ui
        .checkbox(&mut caster.self_shadow, "self shadow")
        .on_hover_text("shadowed by its own near side; off, light reaches it")
        .changed()
    {
        claim = claim.or(Some("Toggle self shadow"));
    }

    let mut replace = None;
    match &mut caster.shape {
        CasterShape::Collider => {
            let said = if has_collider {
                "shaped by its collider"
            } else {
                "shaped by its collider, but it has none: it casts nothing"
            };
            ui.label(RichText::new(said).weak());
            if let Some(points) = starting {
                let from = if has_collider {
                    "start from the collider's outline and edit it in the viewport"
                } else {
                    "start from the sprite's quad and edit it in the viewport"
                };
                if ui.button("Use own polygon").on_hover_text(from).clicked() {
                    replace = Some(CasterShape::Polygon {
                        points,
                        closed: true,
                    });
                    shape.set_editing(Some(entity));
                    claim = claim.or(Some("Use own shadow shape"));
                }
            }
        }
        CasterShape::Polygon { points, closed } => {
            ui.label(RichText::new(format!("own polygon, {} points", points.len())).weak());
            if ui
                .checkbox(closed, "closed")
                .on_hover_text("an outline; open, a line that blocks both sides")
                .changed()
            {
                claim = claim.or(Some("Toggle closed outline"));
            }
            let editing = shape.is_editing(entity);
            ui.horizontal(|ui| {
                if ui
                    .add(Button::selectable(editing, "Edit shape"))
                    .on_hover_text("move, add and remove its points in the viewport")
                    .clicked()
                {
                    shape.set_editing((!editing).then_some(entity));
                }
                if has_collider && ui.button("Use collider").clicked() {
                    replace = Some(CasterShape::Collider);
                    shape.set_editing(None);
                    claim = claim.or(Some("Use collider shadow shape"));
                }
            });
            if editing {
                ui.label(
                    RichText::new(
                        "drag a point to move it; press a midpoint to add one; \
                         right-click a point to remove it; Esc to finish",
                    )
                    .weak()
                    .small(),
                );
            }
        }
    }
    if let Some(new) = replace {
        caster.shape = new;
    }
    claim
}
