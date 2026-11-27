//! Putting a dropdown or a set of tabs into the scene, whole, as one undo
//! entry.

use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{
    hierarchy, Align, Name, Sprite2D, Text, UiButton, UiDropdown, UiHidden, UiRect, UiStack, UiTab,
    UiTabs, UiToggle, UiTooltip,
};

use super::hud::spawn;

/// Size a control's text is, in points.
const TEXT_SIZE: f32 = 18.0;

fn child(frame: &mut UiFrame<'_>, parent: Entity, entity: Entity) {
    if let Err(e) = hierarchy::set_parent(frame.world, entity, parent) {
        log::error!("could not put a control's part under it: {e}");
    }
}

fn text(frame: &mut UiFrame<'_>, content: &str) -> Text {
    let mut text = Text {
        size: TEXT_SIZE,
        align: Align::Center,
        ..Text::new(content)
    };
    text.set_font(None, frame.fonts);
    text
}

/// A dropdown: a button whose text shows the chosen option, with a tooltip
/// saying what it is for.
pub(crate) fn hud_dropdown(frame: &mut UiFrame<'_>, name: &str) -> Entity {
    let dropdown = spawn(frame, name);
    frame.world.insert(
        dropdown,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(16.0), Vec2::new(200.0, 30.0)),
    );
    frame.world.insert(dropdown, Sprite2D::default());
    frame.world.insert(dropdown, UiButton::default());
    frame.world.insert(dropdown, UiDropdown::default());
    let style = text(frame, "One");
    frame.world.insert(dropdown, style);
    frame
        .world
        .insert(dropdown, UiTooltip::new("Pick one of the options"));
    dropdown
}

/// Tabs: a container with a row of two tab buttons and two pages, the
/// second hidden.
pub(crate) fn hud_tabs(frame: &mut UiFrame<'_>, name: &str) -> (Vec<Entity>, Option<Entity>) {
    let tabs = spawn(frame, name);
    frame.world.insert(
        tabs,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(16.0), Vec2::new(320.0, 240.0)),
    );
    frame.world.insert(tabs, UiTabs::default());
    let bar = spawn(frame, "Tab bar");
    frame.world.insert(
        bar,
        UiRect {
            anchor_min: Vec2::ZERO,
            anchor_max: Vec2::new(1.0, 0.0),
            offset_min: Vec2::ZERO,
            offset_max: Vec2::new(0.0, 30.0),
            slot: 0,
        },
    );
    frame.world.insert(
        bar,
        UiStack {
            direction: voltra_scene::ui::stack::Direction::Row,
            spacing: 4.0,
            padding: voltra_scene::ui::Edges::uniform(0.0),
            ..UiStack::default()
        },
    );
    child(frame, tabs, bar);
    let mut spawned = vec![tabs, bar];
    for (slot, page) in ["Page 1", "Page 2"].into_iter().enumerate() {
        let tab = spawn(frame, &format!("{page} tab"));
        frame.world.insert(
            tab,
            UiRect {
                offset_max: Vec2::new(100.0, 0.0),
                slot: slot as i32,
                ..UiRect::default()
            },
        );
        frame.world.insert(tab, Sprite2D::default());
        frame.world.insert(tab, UiButton::default());
        frame.world.insert(
            tab,
            UiToggle {
                on: slot == 0,
                ..UiToggle::default()
            },
        );
        frame.world.insert(
            tab,
            UiTab {
                page: page.to_owned(),
            },
        );
        let words = text(frame, page);
        frame.world.insert(tab, words);
        child(frame, bar, tab);

        let body = spawn(frame, page);
        frame.world.insert(body, Name::new(page));
        frame.world.insert(
            body,
            UiRect {
                anchor_min: Vec2::ZERO,
                anchor_max: Vec2::ONE,
                offset_min: Vec2::new(0.0, 36.0),
                offset_max: Vec2::ZERO,
                slot: 0,
            },
        );
        frame
            .world
            .insert(body, Sprite2D::new([0.09, 0.1, 0.14, 1.0]));
        if slot > 0 {
            frame.world.insert(body, UiHidden);
        }
        child(frame, tabs, body);
        spawned.extend([tab, body]);
    }
    (spawned, Some(tabs))
}
