//! What a field shows and where its caret falls: the parts that need the
//! font to measure.

use voltra_assets::Fonts;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::UiTextField;
use crate::text::{shape, ui_px, Align, Text};
use crate::ui::{Rect, UiLayout};

/// The x a text's aligned origin sits at in `rect`: its left edge, middle
/// or right edge — where the draw puts it.
pub(crate) fn aligned_x(rect: Rect, align: Align) -> f32 {
    match align {
        Align::Left => rect.min.x,
        Align::Center => rect.center().x,
        Align::Right => rect.max.x,
    }
}

/// The caret stops of `content` in `style`, in points from the aligned
/// origin.
pub(crate) fn stops(style: &Text, content: &str, fonts: &Fonts) -> Option<Vec<f32>> {
    let mut text = style.clone();
    text.content = content.to_owned();
    // Measured as drawn: brackets typed into a field are text.
    text.rich = false;
    shape::caret_stops_at(&text, fonts, ui_px(&text, 1.0))
}

/// Brings every field's shown text, focus and scroll up to date, after the
/// frame's input has edited them.
pub(crate) fn refresh(
    world: &mut World,
    layout: &UiLayout,
    fonts: Option<&Fonts>,
    focused: Option<Entity>,
) {
    let fields: Vec<Entity> = world.query::<UiTextField>().map(|(e, _)| e).collect();
    for entity in fields {
        let style = world.get::<Text>(entity).cloned();
        let Some(field) = world.get_mut::<UiTextField>(entity) else {
            continue;
        };
        field.focused = focused == Some(entity);
        if !field.focused {
            field.preedit = None;
        }
        let (shown, _) = field.display();
        if let (Some(style), Some(rect), Some(fonts)) = (&style, layout.rect(entity), fonts) {
            if let Some(stops) = stops(style, &shown, fonts) {
                field.scroll = scrolled(field, &stops, rect, style.align);
            }
        }
        if let Some(text) = world.get_mut::<Text>(entity) {
            if text.content != shown {
                text.content = shown;
            }
        }
    }
}

/// Room kept right of the caret, in points, so the box's own mask does not
/// cut the caret off at its right edge.
const CARET_ROOM: f32 = 2.0;

/// The scroll that keeps the caret inside `rect`, moving the least, and
/// never further than the text reaches.
fn scrolled(field: &UiTextField, stops: &[f32], rect: Rect, align: Align) -> f32 {
    let right = (rect.max.x - CARET_ROOM).max(rect.min.x);
    let origin = aligned_x(rect, align);
    let (Some(&first), Some(&last)) = (stops.first(), stops.last()) else {
        return 0.0;
    };
    let caret = origin + stops[field.display_caret().min(stops.len() - 1)];
    let mut scroll = if field.scroll.is_finite() {
        field.scroll
    } else {
        0.0
    };
    if caret - scroll > right {
        scroll = caret - right;
    }
    if caret - scroll < rect.min.x {
        scroll = caret - rect.min.x;
    }
    let most = (origin + last - right).max(0.0);
    let least = (origin + first - rect.min.x).min(0.0);
    scroll.clamp(least, most.max(least))
}

/// The character `point` falls nearest, in `entity`'s field as it is
/// scrolled; the end without a font to measure with.
pub(crate) fn index_at(
    world: &World,
    entity: Entity,
    layout: &UiLayout,
    fonts: Option<&Fonts>,
    point: Vec2,
) -> Option<usize> {
    let field = world.get::<UiTextField>(entity)?;
    let end = field.text.chars().count();
    let (Some(style), Some(rect), Some(fonts)) =
        (world.get::<Text>(entity), layout.rect(entity), fonts)
    else {
        return Some(end);
    };
    let Some(stops) = stops(style, &field.text, fonts) else {
        return Some(end);
    };
    let x = point.x + field.scroll - aligned_x(rect, style.align);
    stops
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (a.1 - x)
                .abs()
                .partial_cmp(&(b.1 - x).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_at(caret: usize) -> UiTextField {
        let mut field = UiTextField::with_text("abcdefghij");
        field.caret = caret;
        field.anchor = caret;
        field
    }

    /// Ten characters, ten points each.
    fn stops() -> Vec<f32> {
        (0..=10).map(|i| i as f32 * 10.0).collect()
    }

    fn rect() -> Rect {
        Rect::new(Vec2::new(100.0, 0.0), Vec2::new(150.0, 20.0))
    }

    #[test]
    fn a_caret_past_the_right_edge_scrolls_the_text_left() {
        let scroll = scrolled(&field_at(10), &stops(), rect(), Align::Left);
        assert_eq!(scroll, 50.0 + CARET_ROOM);
    }

    #[test]
    fn a_caret_back_at_the_start_scrolls_it_back() {
        let mut field = field_at(1);
        field.scroll = 50.0;
        assert_eq!(scrolled(&field, &stops(), rect(), Align::Left), 10.0);
    }

    #[test]
    fn short_text_does_not_scroll() {
        let mut field = UiTextField::with_text("ab");
        field.scroll = 30.0;
        let short: Vec<f32> = vec![0.0, 10.0, 20.0];
        assert_eq!(scrolled(&field, &short, rect(), Align::Left), 0.0);
    }
}
