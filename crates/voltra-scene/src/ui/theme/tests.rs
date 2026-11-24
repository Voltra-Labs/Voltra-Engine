use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::*;
use crate::ui::{
    UiButton, UiControls, UiDropdown, UiInput, UiLayout, UiRect, UiSlider, UiSliderPart, UiStack,
    UiTab, UiTabs, UiTextField, UiToggle, UiTooltip,
};
use crate::{hierarchy, Name, SceneId, Sprite, Text};

/// A theme whose every colour is `shade` grey, with a font and a skin.
fn theme(shade: f32, font: &str) -> UiTheme {
    let c = format!("({shade}, {shade}, {shade}, 1.0)");
    let text = format!(
        r#"#![enable(implicit_some)]
        (
            version: 1,
            font: "{font}",
            text_color: {c},
            button: (normal: {c}, hovered: {c}, pressed: {c}, disabled: {c},
                skin: (texture: "ui/{font}.png", borders: (left: 4.0, top: 4.0, right: 4.0, bottom: 4.0))),
            selected_tint: {c},
            panel: (color: {c}),
            field: (color: {c}, selection: {c}),
            slider: (track: {c}, fill: {c}, handle: {c}),
            dropdown: (background: {c}, highlight: {c}),
            tooltip: (background: {c}, color: {c}, size: 11.0),
            spacing: {shade},
            padding: {shade},
        )"#
    );
    UiTheme::parse(&text).unwrap_or_else(|e| panic!("the theme parses: {e}"))
}

fn element(world: &mut World, name: &str, parent: Option<Entity>) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Name::new(name));
    world.insert(
        e,
        UiRect::pinned(Vec2::ZERO, Vec2::new(10.0, 10.0), Vec2::new(100.0, 30.0)),
    );
    if let Some(parent) = parent {
        hierarchy::set_parent(world, e, parent).expect("parents");
    }
    e
}

/// One of every control on a panel, and a label.
struct Screen {
    panel: Entity,
    button: Entity,
    label: Entity,
    toggle: Entity,
    slider: Entity,
    handle: Entity,
    field: Entity,
    dropdown: Entity,
    tab: Entity,
    own: Entity,
}

fn screen(world: &mut World) -> Screen {
    let panel = element(world, "Panel", None);
    world.insert(panel, Sprite::default());
    world.insert(panel, UiStack::default());
    world.insert(panel, UiTabs::default());
    world.insert(panel, UiTooltip::default());
    let button = element(world, "Button", Some(panel));
    world.insert(button, Sprite::default());
    world.insert(button, UiButton::default());
    let label = element(world, "Label", Some(button));
    world.insert(label, Text::new("Play"));
    let toggle = element(world, "Toggle", Some(panel));
    world.insert(toggle, Sprite::default());
    world.insert(toggle, UiButton::default());
    world.insert(toggle, UiToggle::default());
    let slider = element(world, "Slider", Some(panel));
    world.insert(slider, Sprite::default());
    world.insert(slider, UiSlider::default());
    let handle = element(world, "Handle", Some(slider));
    world.insert(handle, Sprite::default());
    world.insert(handle, UiSliderPart::Handle);
    let field = element(world, "Field", Some(panel));
    world.insert(field, Sprite::default());
    world.insert(field, Text::new(""));
    world.insert(field, UiTextField::default());
    let dropdown = element(world, "Dropdown", Some(panel));
    world.insert(dropdown, Sprite::default());
    world.insert(dropdown, UiButton::default());
    world.insert(dropdown, UiDropdown::default());
    world.insert(dropdown, Text::new(""));
    let tab = element(world, "Tab", Some(panel));
    world.insert(tab, Sprite::default());
    world.insert(tab, UiButton::default());
    world.insert(tab, UiToggle::default());
    world.insert(
        tab,
        UiTab {
            page: "Page".to_owned(),
        },
    );
    let own = element(world, "Own", Some(panel));
    world.insert(own, Sprite::default());
    world.insert(own, UiButton::default());
    world.insert(own, UiOwnStyle);
    Screen {
        panel,
        button,
        label,
        toggle,
        slider,
        handle,
        field,
        dropdown,
        tab,
        own,
    }
}

fn frame(world: &mut World, controls: &mut UiControls) {
    let layout = UiLayout::from_world(world, Vec2::new(800.0, 600.0));
    controls.update(world, &layout, None, &UiInput::default());
}

/// Every themed part of `s` shows `shade`, and the own-styled button not.
fn styled_as(world: &World, s: &Screen, shade: f32, font: &str) {
    let c = [shade, shade, shade, 1.0];
    let button = world.get::<UiButton>(s.button).expect("a button");
    assert_eq!(button.normal, c);
    assert_eq!(
        world
            .get::<Sprite>(s.button)
            .and_then(|sp| sp.texture.as_ref())
            .map(|p| p.as_str().to_owned()),
        Some(format!("ui/{font}.png")),
        "the skin's texture"
    );
    assert!(world.get::<crate::UiSlice>(s.button).is_some(), "sliced");
    let label = world.get::<Text>(s.label).expect("a label");
    assert_eq!(label.color, c);
    assert_eq!(label.font.as_ref().map(|p| p.as_str()), Some(font));
    assert_eq!(world.get::<UiToggle>(s.toggle).map(|t| t.on_tint), Some(c));
    assert_eq!(world.get::<UiToggle>(s.tab).map(|t| t.on_tint), Some(c));
    assert_eq!(world.get::<Sprite>(s.slider).map(|sp| sp.color), Some(c));
    assert_eq!(world.get::<Sprite>(s.handle).map(|sp| sp.color), Some(c));
    assert_eq!(world.get::<Sprite>(s.field).map(|sp| sp.color), Some(c));
    assert_eq!(
        world.get::<UiTextField>(s.field).map(|f| f.selection_color),
        Some(c)
    );
    let dropdown = world.get::<UiDropdown>(s.dropdown).expect("a dropdown");
    assert_eq!((dropdown.list_color, dropdown.highlight_color), (c, c));
    let tooltip = world.get::<UiTooltip>(s.panel).expect("a tooltip");
    assert_eq!(
        (tooltip.background, tooltip.color, tooltip.size),
        (c, c, 11.0)
    );
    assert_eq!(world.get::<Sprite>(s.panel).map(|sp| sp.color), Some(c));
    let stack = world.get::<UiStack>(s.panel).expect("a stack");
    assert_eq!(stack.spacing, shade);
    assert_eq!(stack.padding.left, shade);
    let own = world.get::<UiButton>(s.own).expect("its own button");
    assert_eq!(
        own.normal,
        UiButton::default().normal,
        "UiOwnStyle keeps its look"
    );
}

#[test]
fn a_theme_file_reads_and_a_newer_one_is_refused() {
    let dark = theme(0.25, "fonts/A.ttf");
    assert_eq!(dark.spacing, Some(0.25));
    assert!(matches!(
        UiTheme::parse("(version: 99)"),
        Err(ThemeError::Version(99))
    ));
    assert_eq!(
        UiTheme::parse("(version: 1)").expect("an empty theme"),
        UiTheme {
            version: 1,
            ..UiTheme::default()
        }
    );
}

#[test]
fn swapping_the_theme_restyles_every_control() {
    let mut world = World::new();
    let s = screen(&mut world);
    let mut controls = UiControls::default();

    controls.set_theme(Some(theme(0.25, "fonts/A.ttf")));
    frame(&mut world, &mut controls);
    styled_as(&world, &s, 0.25, "fonts/A.ttf");

    controls.set_theme(Some(theme(0.75, "fonts/B.ttf")));
    frame(&mut world, &mut controls);
    styled_as(&world, &s, 0.75, "fonts/B.ttf");
}

#[test]
fn a_colour_set_after_the_theme_stands() {
    let mut world = World::new();
    let s = screen(&mut world);
    let mut controls = UiControls::default();
    controls.set_theme(Some(theme(0.25, "fonts/A.ttf")));
    frame(&mut world, &mut controls);
    if let Some(label) = world.get_mut::<Text>(s.label) {
        label.color = [1.0, 0.0, 0.0, 1.0];
    }
    frame(&mut world, &mut controls);
    assert_eq!(
        world.get::<Text>(s.label).map(|t| t.color),
        Some([1.0, 0.0, 0.0, 1.0])
    );

    // An element spawned later is themed on its first frame.
    let late = element(&mut world, "Late", None);
    world.insert(late, Sprite::default());
    world.insert(late, UiButton::default());
    frame(&mut world, &mut controls);
    assert_eq!(
        world.get::<UiButton>(late).map(|b| b.normal),
        Some([0.25, 0.25, 0.25, 1.0])
    );
}

#[test]
fn the_shipped_themes_read() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/ui");
    for name in ["dark.theme.ron", "light.theme.ron"] {
        let theme = UiTheme::load(&root.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(theme.button.is_some() && theme.tooltip.is_some(), "{name}");
    }
}
