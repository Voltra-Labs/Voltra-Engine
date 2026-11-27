//! A theme written into the elements that take it.

use std::collections::HashSet;

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use super::{Skin, UiOwnStyle, UiTheme};
use crate::sprite::Sprite2D;
use crate::text::Text;
use crate::ui::controls::is_control;
use crate::ui::edges::Edges;
use crate::ui::{
    UiButton, UiDropdown, UiRect, UiSlider, UiSliderPart, UiStack, UiTextField, UiToggle, UiTooltip,
};

/// Styles every UI element `styled` does not hold yet with `theme`, and
/// adds it: an element is themed once, when it first appears, so a colour
/// code sets on it later stands. Clearing `styled` themes everything again
/// — what a new theme does. Elements marked [`UiOwnStyle`] are passed over;
/// despawned ones are forgotten.
pub fn apply(world: &mut World, theme: &UiTheme, styled: &mut HashSet<Entity>) {
    styled.retain(|&e| world.is_alive(e));
    let fresh: Vec<Entity> = world
        .query::<UiRect>()
        .map(|(e, _)| e)
        .filter(|e| !styled.contains(e))
        .collect();
    let font = theme
        .font
        .as_deref()
        .and_then(|raw| match AssetPath::new(raw) {
            Ok(path) => Some(path),
            Err(e) => {
                log::warn!("the theme's font {raw:?}: {e}");
                None
            }
        });
    for entity in fresh {
        styled.insert(entity);
        if world.get::<UiOwnStyle>(entity).is_some() {
            continue;
        }
        style(world, entity, theme, font.as_ref());
    }
}

fn style(world: &mut World, entity: Entity, theme: &UiTheme, font: Option<&AssetPath>) {
    let control = is_control(world, entity);
    if let (Some(style), Some(button)) = (&theme.button, world.get_mut::<UiButton>(entity)) {
        button.normal = style.normal;
        button.hovered = style.hovered;
        button.pressed = style.pressed;
        button.disabled_color = style.disabled;
        let skin = style.skin.clone();
        skin_sprite(world, entity, skin.as_ref());
    }
    if let (Some(tint), Some(toggle)) = (theme.selected_tint, world.get_mut::<UiToggle>(entity)) {
        toggle.on_tint = tint;
    }
    // A dropdown is a button: its box took the button style above.
    if let (Some(list), Some(dropdown)) = (&theme.dropdown, world.get_mut::<UiDropdown>(entity)) {
        dropdown.list_color = list.background;
        dropdown.highlight_color = list.highlight;
    }
    if let Some(style) = &theme.field {
        if let Some(field) = world.get_mut::<UiTextField>(entity) {
            field.selection_color = style.selection;
            if let Some(sprite) = world.get_mut::<Sprite2D>(entity) {
                sprite.color = style.color;
            }
            let skin = style.skin.clone();
            skin_sprite(world, entity, skin.as_ref());
        }
    }
    if let Some(style) = &theme.slider {
        if world.get::<UiSlider>(entity).is_some() {
            if let Some(sprite) = world.get_mut::<Sprite2D>(entity) {
                sprite.color = style.track;
            }
        }
        if let Some(part) = world.get::<UiSliderPart>(entity).copied() {
            if let Some(sprite) = world.get_mut::<Sprite2D>(entity) {
                sprite.color = match part {
                    UiSliderPart::Fill => style.fill,
                    UiSliderPart::Handle => style.handle,
                };
            }
        }
    }
    if let Some(tooltip) = world.get_mut::<UiTooltip>(entity) {
        if let Some(style) = &theme.tooltip {
            tooltip.background = style.background;
            tooltip.color = style.color;
            if let Some(size) = style.size.filter(|s| s.is_finite() && *s > 0.0) {
                tooltip.size = size;
            }
        }
        if let Some(font) = font {
            tooltip.font = Some(font.clone());
        }
    }
    if let Some(text) = world.get_mut::<Text>(entity) {
        if let Some(color) = theme.text_color {
            text.color = color;
        }
        if let Some(font) = font {
            if text.font.as_ref() != Some(font) {
                text.font = Some(font.clone());
                // Resolved by the loop's next pass over unresolved texts.
                text.font_handle = None;
            }
        }
    }
    let panel = !control
        && world.get::<UiStack>(entity).is_some()
        && world.get::<Sprite2D>(entity).is_some()
        && world.get::<UiSliderPart>(entity).is_none();
    if panel {
        if let Some(style) = &theme.panel {
            if let Some(sprite) = world.get_mut::<Sprite2D>(entity) {
                sprite.color = style.color;
            }
            let skin = style.skin.clone();
            skin_sprite(world, entity, skin.as_ref());
        }
    }
    if let Some(stack) = world.get_mut::<UiStack>(entity) {
        if let Some(spacing) = theme.spacing.filter(|s| s.is_finite() && *s >= 0.0) {
            stack.spacing = spacing;
        }
        if panel {
            if let Some(padding) = theme.padding.filter(|p| p.is_finite() && *p >= 0.0) {
                stack.padding = Edges::uniform(padding);
            }
        }
    }
}

/// Gives `entity`'s sprite the skin's texture and slice. The texture's
/// handle is left for the loop to resolve — it needs the GPU.
fn skin_sprite(world: &mut World, entity: Entity, skin: Option<&Skin>) {
    let Some(skin) = skin else {
        return;
    };
    let path = match AssetPath::new(&skin.texture) {
        Ok(path) => path,
        Err(e) => {
            log::warn!("the theme's skin {:?}: {e}", skin.texture);
            return;
        }
    };
    let Some(sprite) = world.get_mut::<Sprite2D>(entity) else {
        return;
    };
    if sprite.texture.as_ref() != Some(&path) {
        sprite.texture = Some(path);
        sprite.texture_handle = None;
        sprite.atlas = None;
        sprite.atlas_handle = None;
    }
    world.insert(entity, skin.slice());
}
