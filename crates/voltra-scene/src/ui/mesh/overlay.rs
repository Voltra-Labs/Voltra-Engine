//! The overlays drawn after every element, unmasked: open lists and
//! tooltips over everything.

use voltra_ecs::World;
use voltra_render::glam::Vec2;

use super::{push_text, Target};
use crate::batch::SpriteBatch;
use crate::sprite::quad::WHOLE_UVS;
use crate::sprite::sheets::Sheets;
use crate::ui::layout::UiLayout;
use crate::ui::overlay::{overlays, Overlay};

pub(super) fn push(
    batch: &mut SpriteBatch,
    world: &World,
    sheets: Sheets<'_>,
    layout: &UiLayout,
    scale: f32,
) {
    let Some(fonts) = sheets.fonts else {
        return;
    };
    for overlay in overlays(world, layout, fonts) {
        let target = Target { batch, mask: None };
        match overlay {
            Overlay::Box { rect, color } => {
                let (min, max) = (rect.min * scale, rect.max * scale);
                let corners = [
                    Vec2::new(min.x, min.y),
                    Vec2::new(min.x, max.y),
                    Vec2::new(max.x, max.y),
                    Vec2::new(max.x, min.y),
                ];
                let mut target = target;
                target.push(corners, WHOLE_UVS, color, None);
            }
            Overlay::Text { rect, text } => push_text(target, rect, &text, sheets, scale),
        }
    }
}
