use super::*;

const W: KeyCode = KeyCode::KeyW;
const S: KeyCode = KeyCode::KeyS;

#[test]
fn press_registers_as_held_and_pressed() {
    let mut input = Input::new();
    input.on_key(W, true, false);

    assert!(input.is_key_down(W));
    assert!(input.was_key_pressed(W));
    assert!(!input.was_key_released(W));
}

#[test]
fn end_frame_clears_pressed_but_keeps_held() {
    let mut input = Input::new();
    input.on_key(W, true, false);
    input.end_frame();

    assert!(input.is_key_down(W), "the key is still physically down");
    assert!(!input.was_key_pressed(W), "but it is no longer a new press");
}

#[test]
fn release_clears_held_and_reports_released() {
    let mut input = Input::new();
    input.on_key(W, true, false);
    input.end_frame();
    input.on_key(W, false, false);

    assert!(!input.is_key_down(W));
    assert!(input.was_key_released(W));
}

#[test]
fn auto_repeat_does_not_re_trigger_a_press() {
    let mut input = Input::new();
    input.on_key(W, true, false);
    input.end_frame();

    // The OS keeps sending presses while the key is held.
    input.on_key(W, true, true);

    assert!(input.is_key_down(W), "still held");
    assert!(
        !input.was_key_pressed(W),
        "a repeat must not read as a fresh press"
    );
}

#[test]
fn losing_focus_releases_everything_held() {
    let mut input = Input::new();
    input.on_key(W, true, false);
    input.on_mouse_button(MouseButton::Left, true);
    input.end_frame();

    input.on_focus_lost();

    assert!(!input.is_key_down(W), "a held key must not survive alt-tab");
    assert!(!input.is_mouse_button_down(MouseButton::Left));
    assert!(input.was_key_released(W));
    assert!(input.was_mouse_button_released(MouseButton::Left));
}

#[test]
fn first_cursor_event_reports_no_delta() {
    let mut input = Input::new();
    input.on_cursor_moved(Vec2::new(400.0, 300.0));

    assert_eq!(input.cursor_position(), Vec2::new(400.0, 300.0));
    assert_eq!(
        input.mouse_delta(),
        Vec2::ZERO,
        "the jump from the origin is not movement"
    );
}

#[test]
fn cursor_deltas_accumulate_within_a_frame() {
    let mut input = Input::new();
    input.on_cursor_moved(Vec2::new(100.0, 100.0));
    input.on_cursor_moved(Vec2::new(110.0, 100.0));
    input.on_cursor_moved(Vec2::new(110.0, 130.0));

    assert_eq!(input.mouse_delta(), Vec2::new(10.0, 30.0));

    input.end_frame();
    assert_eq!(input.mouse_delta(), Vec2::ZERO);
    assert_eq!(
        input.cursor_position(),
        Vec2::new(110.0, 130.0),
        "position is absolute and survives the frame boundary"
    );
}

#[test]
fn scroll_accumulates_and_resets() {
    let mut input = Input::new();
    input.on_scroll(1.0);
    input.on_scroll(-0.5);

    assert_eq!(input.scroll_delta(), 0.5);
    input.end_frame();
    assert_eq!(input.scroll_delta(), 0.0);
}

#[test]
fn axis_cancels_when_both_are_held() {
    let mut input = Input::new();
    assert_eq!(input.axis(S, W), 0.0);

    input.on_key(W, true, false);
    assert_eq!(input.axis(S, W), 1.0);

    input.on_key(S, true, false);
    assert_eq!(input.axis(S, W), 0.0, "opposing keys cancel");

    input.on_key(W, false, false);
    assert_eq!(input.axis(S, W), -1.0);
}

mod actions {
    use super::*;

    fn map() -> ActionMap {
        let mut map = ActionMap::new();
        map.bind("jump", Binding::Key(KeyCode::Space));
        map.bind("left", Binding::Key(KeyCode::KeyA));
        map.bind("right", Binding::Key(KeyCode::KeyD));
        map.bind("fire", Binding::Mouse(MouseButton::Left));
        map
    }

    #[test]
    fn a_bound_key_drives_its_action() {
        let mut input = Input::new();
        input.on_key(KeyCode::Space, true, false);
        input.update_actions(&map());
        assert!(input.action("jump").pressed());

        input.end_frame();
        input.update_actions(&map());
        assert!(input.action("jump").is_held());
        assert!(!input.action("jump").pressed());
    }

    #[test]
    fn a_mouse_button_drives_its_action() {
        let mut input = Input::new();
        input.on_mouse_button(MouseButton::Left, true);
        input.update_actions(&map());
        assert!(input.action("fire").pressed());
    }

    #[test]
    fn an_unknown_action_is_never_held() {
        let mut input = Input::new();
        input.on_key(KeyCode::Space, true, false);
        input.update_actions(&map());
        assert_eq!(input.action("jmup"), ActionState::default());
    }

    #[test]
    fn action_axis_is_the_difference_of_two_actions() {
        let mut input = Input::new();
        input.on_key(KeyCode::KeyD, true, false);
        input.update_actions(&map());
        assert_eq!(input.action_axis("left", "right"), 1.0);
        input.on_key(KeyCode::KeyA, true, false);
        input.update_actions(&map());
        assert_eq!(input.action_axis("left", "right"), 0.0);
    }

    #[test]
    fn focus_loss_releases_actions_too() {
        let mut input = Input::new();
        input.on_key(KeyCode::Space, true, false);
        input.update_actions(&map());
        input.end_frame();
        input.on_focus_lost();
        input.update_actions(&map());
        assert!(input.action("jump").released());
    }

    #[test]
    fn pressed_binding_reports_what_went_down_this_frame() {
        let mut input = Input::new();
        assert_eq!(input.pressed_binding(), None);
        input.on_mouse_button(MouseButton::Right, true);
        assert_eq!(
            input.pressed_binding(),
            Some(Binding::Mouse(MouseButton::Right))
        );
        input.on_key(KeyCode::KeyQ, true, false);
        assert_eq!(
            input.pressed_binding(),
            Some(Binding::Key(KeyCode::KeyQ)),
            "a key before a button"
        );
        input.end_frame();
        assert_eq!(input.pressed_binding(), None);
    }

    #[test]
    fn a_rebinding_is_seen_from_the_next_update() {
        let mut map = map();
        let mut input = Input::new();
        input.on_key(KeyCode::KeyW, true, false);
        input.update_actions(&map);
        assert!(!input.action("jump").is_held());

        map.rebind("jump", 0, Binding::Key(KeyCode::KeyW));
        input.update_actions(&map);
        assert!(input.action("jump").is_held());
    }
}

mod pads {
    use super::*;
    use crate::input::{Direction, PadAxis, PadButton};

    fn map() -> ActionMap {
        let mut map = ActionMap::new();
        map.bind("jump", Binding::Pad(PadButton::South));
        map.bind("jump", Binding::Key(KeyCode::Space));
        map.bind(
            "left",
            Binding::PadAxis(PadAxis::LeftStickX, Direction::Negative),
        );
        map.bind(
            "right",
            Binding::PadAxis(PadAxis::LeftStickX, Direction::Positive),
        );
        map
    }

    #[test]
    fn a_pad_button_drives_the_same_action_as_a_key() {
        let mut input = Input::new();
        input.on_pad_button(0, PadButton::South, true);
        input.update_actions(&map());
        assert!(input.action("jump").pressed());
    }

    #[test]
    fn a_stick_gives_an_analog_axis() {
        let mut input = Input::new();
        input.on_pad_axis(0, PadAxis::LeftStickX, -0.625);
        input.update_actions(&map());
        let axis = input.action_axis("left", "right");
        assert!((axis + 0.5).abs() < 1e-6, "{axis}");
    }

    #[test]
    fn pads_are_ignored_while_unfocused_and_count_again_after() {
        let mut input = Input::new();
        input.on_focus_lost();
        input.on_pad_button(0, PadButton::South, true);
        input.update_actions(&map());
        assert!(!input.action("jump").is_held());

        input.on_focus_gained();
        input.on_pad_button(0, PadButton::South, true);
        input.update_actions(&map());
        assert!(input.action("jump").is_held());
    }

    #[test]
    fn losing_focus_releases_a_held_pad_button() {
        let mut input = Input::new();
        input.on_pad_button(0, PadButton::South, true);
        input.update_actions(&map());
        input.end_frame();
        input.on_focus_lost();
        input.update_actions(&map());
        assert!(input.action("jump").released());
    }

    #[test]
    fn pressed_binding_sees_pad_buttons_and_pushed_sticks() {
        let mut input = Input::new();
        input.on_pad_button(0, PadButton::North, true);
        assert_eq!(
            input.pressed_binding(),
            Some(Binding::Pad(PadButton::North))
        );
        input.end_frame();
        input.on_pad_axis(0, PadAxis::RightStickY, 0.9);
        assert_eq!(
            input.pressed_binding(),
            Some(Binding::PadAxis(PadAxis::RightStickY, Direction::Positive))
        );
    }

    #[test]
    fn a_stick_past_half_way_is_down_as_a_raw_binding() {
        let mut input = Input::new();
        input.on_pad_axis(0, PadAxis::LeftStickX, 0.6);
        assert!(input.is_binding_down(Binding::PadAxis(PadAxis::LeftStickX, Direction::Positive)));
        assert!(!input.is_binding_down(Binding::PadAxis(PadAxis::LeftStickX, Direction::Negative)));
    }
}

mod touch_bindings {
    use voltra_render::glam::Vec2;

    use crate::input::{ActionMap, Binding, Input, TouchDir, TouchPhase, TouchZone};

    fn map() -> ActionMap {
        let mut map = ActionMap::new();
        map.bind(
            "right",
            Binding::Touch(TouchZone::BottomLeft, TouchDir::Right),
        );
        map.bind(
            "jump",
            Binding::Touch(TouchZone::BottomRight, TouchDir::Hold),
        );
        map
    }

    fn input() -> Input {
        let mut input = Input::new();
        input.set_screen(Vec2::new(1200.0, 600.0));
        input
    }

    #[test]
    fn a_thumb_slid_right_in_the_bottom_left_walks_right_by_how_far() {
        let mut input = input();
        // The radius is 600 / 12 = 50 pixels.
        input.on_touch(1, TouchPhase::Started, Vec2::new(200.0, 500.0));
        input.on_touch(1, TouchPhase::Moved, Vec2::new(225.0, 490.0));
        input.update_actions(&map());
        // Half the radius; past the default dead zone of a quarter, a third.
        assert!(
            (input.binding_value(Binding::Touch(TouchZone::BottomLeft, TouchDir::Right)) - 0.5)
                .abs()
                < 1e-4
        );
        assert!((input.action("right").strength() - 1.0 / 3.0).abs() < 1e-4);
        input.end_frame();
        input.on_touch(1, TouchPhase::Moved, Vec2::new(400.0, 500.0));
        input.update_actions(&map());
        assert_eq!(
            input.action("right").strength(),
            1.0,
            "fully at the radius and past it"
        );
        input.on_touch(1, TouchPhase::Ended, Vec2::new(400.0, 500.0));
        input.update_actions(&map());
        assert_eq!(
            input.action("right").strength(),
            0.0,
            "a lifted finger drives nothing"
        );
    }

    #[test]
    fn a_finger_down_elsewhere_drives_nothing_and_a_hold_presses() {
        let mut input = input();
        input.on_touch(1, TouchPhase::Started, Vec2::new(200.0, 100.0));
        input.on_touch(1, TouchPhase::Moved, Vec2::new(400.0, 100.0));
        input.update_actions(&map());
        assert_eq!(
            input.action("right").strength(),
            0.0,
            "it went down in the top half"
        );
        input.on_touch(2, TouchPhase::Started, Vec2::new(1000.0, 500.0));
        input.update_actions(&map());
        assert!(input.action("jump").pressed());
    }

    #[test]
    fn touch_bindings_read_from_a_file() {
        let text = r#"(version: 1, actions: {"right": (bindings: [Touch(BottomLeft, Right)])})"#;
        let map = ActionMap::from_ron(text, std::path::Path::new("x.input.ron")).expect("reads");
        assert_eq!(
            map.bindings("right"),
            &[Binding::Touch(TouchZone::BottomLeft, TouchDir::Right)]
        );
    }
}
