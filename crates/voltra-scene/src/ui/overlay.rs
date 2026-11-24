//! What the UI draws over every element: open dropdown lists and shown
//! tooltips — boxes and texts, placed on the screen.

use voltra_assets::Fonts;
use voltra_ecs::World;
use voltra_render::glam::Vec2;

use super::layout::UiLayout;
use super::rect::Rect;
use super::{UiDropdown, UiTooltip};
use crate::text::{shape, ui_px, Align, Text};

/// Points between a tooltip's text and its box's edge, and between a
/// list item's text and the list's side.
pub const PAD: f32 = 6.0;

/// Points between an element and its tooltip.
const GAP: f32 = 4.0;

/// One thing drawn over the elements, in points.
#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    /// A flat box.
    Box {
        /// Where.
        rect: Rect,
        /// Its colour.
        color: [f32; 4],
    },
    /// A text laid out in a box, as an element's text is in its rect.
    Text {
        /// Where.
        rect: Rect,
        /// What, styled.
        text: Text,
    },
}

/// The texts the overlays draw, without their places: what the prepare
/// rasterizes. Each is drawn in a `UiRect`, so at `ui_px`.
pub fn texts(world: &World, fonts: &Fonts) -> Vec<Text> {
    let mut out = Vec::new();
    for (entity, dropdown) in world.query::<UiDropdown>() {
        if !dropdown.open {
            continue;
        }
        let Some(style) = world.get::<Text>(entity) else {
            continue;
        };
        let shown = dropdown.options.len().min(dropdown.max_visible.max(1));
        for option in dropdown.options.iter().skip(dropdown.first).take(shown) {
            out.push(item(style, option));
        }
    }
    for (_, tooltip) in world.query::<UiTooltip>() {
        if tooltip.shown {
            out.push(tooltip.as_text(fonts));
        }
    }
    out
}

/// Every overlay, in draw order: lists, then tooltips over them.
pub fn overlays(world: &World, layout: &UiLayout, fonts: &Fonts) -> Vec<Overlay> {
    let screen = layout.screen();
    let mut out = Vec::new();
    for (entity, dropdown) in world.query::<UiDropdown>() {
        if !dropdown.open {
            continue;
        }
        let (Some(rect), Some(style)) = (layout.rect(entity), world.get::<Text>(entity)) else {
            continue;
        };
        let Some(list) = dropdown.list(rect, screen) else {
            continue;
        };
        out.push(Overlay::Box {
            rect: list.rect,
            color: dropdown.list_color,
        });
        for row in 0..list.shown {
            let index = dropdown.first + row;
            let Some(option) = dropdown.options.get(index) else {
                break;
            };
            let at = list.row(row);
            if index == dropdown.highlighted {
                out.push(Overlay::Box {
                    rect: at,
                    color: dropdown.highlight_color,
                });
            }
            let text = item(style, option);
            let height = line_height(&text, fonts);
            let top = at.min.y + ((at.size().y - height) / 2.0).max(0.0);
            out.push(Overlay::Text {
                rect: Rect::new(
                    Vec2::new(at.min.x + PAD, top),
                    Vec2::new(at.max.x - PAD, at.max.y),
                ),
                text,
            });
        }
    }
    for (entity, tooltip) in world.query::<UiTooltip>() {
        if !tooltip.shown {
            continue;
        }
        let Some(rect) = layout.rect(entity) else {
            continue;
        };
        let text = tooltip.as_text(fonts);
        let px = ui_px(&text, 1.0);
        let Some(bounds) = shape::bounds_at(&text, fonts, px, text.max_width) else {
            continue;
        };
        let size = Vec2::new(
            bounds.max.x - bounds.min.x + 2.0 * PAD,
            bounds.max.y - bounds.min.y + 2.0 * PAD,
        );
        let below = rect.max.y + GAP;
        let y = if below + size.y <= screen.max.y {
            below
        } else {
            (rect.min.y - GAP - size.y).max(screen.min.y)
        };
        let x = rect.min.x.min(screen.max.x - size.x).max(screen.min.x);
        let boxed = Rect::new(Vec2::new(x, y), Vec2::new(x, y) + size);
        out.push(Overlay::Box {
            rect: boxed,
            color: tooltip.background,
        });
        out.push(Overlay::Text {
            rect: Rect::new(boxed.min + Vec2::splat(PAD), boxed.max - Vec2::splat(PAD)),
            text,
        });
    }
    out
}

/// A list item: the dropdown's text style saying `option`, on one line.
fn item(style: &Text, option: &str) -> Text {
    Text {
        content: option.to_owned(),
        key: None,
        max_width: None,
        align: Align::Left,
        ..style.clone()
    }
}

/// Ascent to descent of `text`'s first line, in its own units.
fn line_height(text: &Text, fonts: &Fonts) -> f32 {
    shape::bounds_at(text, fonts, ui_px(text, 1.0), None).map_or(text.size, |b| b.max.y - b.min.y)
}
