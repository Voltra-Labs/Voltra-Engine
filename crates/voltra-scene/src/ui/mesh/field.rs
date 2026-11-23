//! A text field drawn: its selection, its text scrolled and cut to the box,
//! its caret and the IME composition's underline.

use voltra_render::glam::Vec2;

use super::{push_text, Target};
use crate::sprite::sheets::Sheets;
use crate::text::{shape, ui_px, Text};
use crate::ui::text_field::view::aligned_x;
use crate::ui::{Rect, UiTextField};

/// How much fainter the placeholder is than typed text.
const PLACEHOLDER_ALPHA: f32 = 0.5;

pub(super) fn push(
    mut target: Target<'_>,
    rect: Rect,
    text: &Text,
    field: &UiTextField,
    sheets: Sheets<'_>,
    scale: f32,
) {
    let boxed = Rect::new(rect.min * scale, rect.max * scale);
    target.mask = Some(target.mask.map_or(boxed, |mask| mask.intersection(boxed)));
    let (_, placeholder) = field.display();
    let scroll = if field.scroll.is_finite() {
        field.scroll
    } else {
        0.0
    };

    let measured = sheets.fonts.and_then(|fonts| {
        let px = ui_px(text, scale);
        Some((
            shape::caret_stops_at(text, fonts, px)?,
            shape::bounds_at(text, fonts, px)?,
        ))
    });
    let origin = aligned_x(rect, text.align) - scroll;
    let line = measured.as_ref().map(|(_, bounds)| {
        let top = rect.min.y;
        (top, top + bounds.max.y - bounds.min.y, top + bounds.max.y)
    });
    let x_of = |index: usize| -> Option<f32> {
        let (stops, _) = measured.as_ref()?;
        if placeholder {
            return Some(origin);
        }
        stops
            .get(index.min(stops.len().saturating_sub(1)))
            .map(|x| origin + x)
    };

    // The selection, under the text, while focused and not composing.
    let selection = field.selection();
    if field.focused && !placeholder && field.preedit.is_none() && !selection.is_empty() {
        if let (Some(a), Some(b), Some((top, bottom, _))) =
            (x_of(selection.start), x_of(selection.end), line)
        {
            solid(
                &mut target,
                Vec2::new(a, top) * scale,
                Vec2::new(b, bottom) * scale,
                field.selection_color,
            );
        }
    }

    let mut shown = text.clone();
    if placeholder {
        shown.color[3] *= PLACEHOLDER_ALPHA;
    }
    push_text(
        Target {
            batch: &mut *target.batch,
            mask: target.mask,
        },
        rect.translated(Vec2::new(-scroll, 0.0)),
        &shown,
        sheets,
        scale,
    );

    if !field.focused {
        return;
    }
    let Some((top, bottom, baseline)) = line else {
        return;
    };
    // One whole pixel wide, or more on a dense screen.
    let thin = scale.round().max(1.0);
    if let Some((from, to)) = field.display_preedit() {
        if let (Some(a), Some(b)) = (x_of(from), x_of(to)) {
            let y = (baseline * scale).round() + thin;
            solid(
                &mut target,
                Vec2::new(a * scale, y),
                Vec2::new(b * scale, y + thin),
                text.color,
            );
        }
    }
    if let Some(x) = x_of(field.display_caret()) {
        let x = (x * scale).round();
        solid(
            &mut target,
            Vec2::new(x, top * scale),
            Vec2::new(x + thin, bottom * scale),
            text.color,
        );
    }
}

/// A flat box from `min` to `max`, in pixels.
fn solid(target: &mut Target<'_>, min: Vec2, max: Vec2, color: [f32; 4]) {
    let corners = [
        Vec2::new(min.x, min.y),
        Vec2::new(min.x, max.y),
        Vec2::new(max.x, max.y),
        Vec2::new(max.x, min.y),
    ];
    target.push(corners, crate::sprite::quad::WHOLE_UVS, color, None);
}
