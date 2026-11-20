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
