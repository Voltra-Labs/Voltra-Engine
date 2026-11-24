//! What each row of the options screen says.

use crate::input::{ActionMap, BindingKind};
use crate::window::Fullscreen;

/// `move_left` as a person reads it: `Move left`.
pub(super) fn humanize(name: &str) -> String {
    let words = name.replace(['_', '-'], " ");
    let mut chars = words.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub(super) fn display(mode: Fullscreen) -> String {
    match mode {
        Fullscreen::Windowed => "Display: Windowed".to_owned(),
        Fullscreen::Borderless => "Display: Fullscreen".to_owned(),
        Fullscreen::Exclusive(mode) => format!(
            "Display: Exclusive {} × {} at {:.0} Hz",
            mode.width,
            mode.height,
            mode.refresh_hz()
        ),
    }
}

pub(super) fn resolution(size: Option<(u32, u32)>) -> String {
    match size {
        Some((width, height)) => format!("Resolution: {width} × {height}"),
        None => "Resolution: set by the page".to_owned(),
    }
}

pub(super) fn vsync(on: bool, can_change: bool) -> String {
    match (can_change, on) {
        (false, _) => "VSync: the browser's".to_owned(),
        (true, true) => "VSync: on".to_owned(),
        (true, false) => "VSync: off".to_owned(),
    }
}

pub(super) fn volume(bus: &str, volume: f32) -> String {
    format!("{} {:.0}%", humanize(bus), volume.clamp(0.0, 1.0) * 100.0)
}

/// `Jump: Space / Pad South`, and the titles of the other listed actions
/// sharing either of those, for the row to show the clash.
pub(super) fn binding<'a>(
    map: &ActionMap,
    action: &str,
    title: &str,
    listed: &[(&'a str, &'a str)],
) -> (String, Vec<&'a str>) {
    let names: Vec<&str> = listed.iter().map(|(name, _)| *name).collect();
    let mut clashes = Vec::new();
    let mut shown = Vec::new();
    for kind in [BindingKind::KeyboardMouse, BindingKind::Gamepad] {
        let Some(binding) = map.first_of_kind(action, kind) else {
            shown.push("—".to_owned());
            continue;
        };
        shown.push(binding.to_string());
        for other in map.conflicts(action, binding, names.iter().copied()) {
            if let Some((_, other_title)) = listed.iter().find(|(name, _)| *name == other) {
                if !clashes.contains(other_title) {
                    clashes.push(*other_title);
                }
            }
        }
    }
    let mut text = format!("{title}: {}", shown.join(" / "));
    if !clashes.is_empty() {
        text.push_str(&format!("  (also {})", clashes.join(", ")));
    }
    (text, clashes)
}

pub(super) fn waiting(title: &str) -> String {
    format!("{title}: press a key… (Esc cancels)")
}

#[cfg(test)]
mod tests {
    use winit::keyboard::KeyCode;

    use super::*;
    use crate::input::{Binding, PadButton};

    #[test]
    fn names_read_as_words() {
        assert_eq!(humanize("move_left"), "Move left");
        assert_eq!(humanize("sfx"), "Sfx");
        assert_eq!(humanize(""), "");
    }

    #[test]
    fn a_binding_row_shows_both_kinds_and_its_clashes() {
        let mut map = ActionMap::new();
        map.bind("jump", Binding::Key(KeyCode::Space));
        map.bind("jump", Binding::Pad(PadButton::South));
        map.bind("left", Binding::Key(KeyCode::KeyA));
        let listed = [("jump", "Jump"), ("left", "Move left")];
        let (text, clashes) = binding(&map, "jump", "Jump", &listed);
        assert_eq!(text, "Jump: Space / Pad South");
        assert!(clashes.is_empty());

        map.assign("jump", Binding::Key(KeyCode::KeyA));
        let (text, clashes) = binding(&map, "jump", "Jump", &listed);
        assert_eq!(text, "Jump: A / Pad South  (also Move left)");
        assert_eq!(clashes, vec!["Move left"]);
        let (text, _) = binding(&map, "left", "Move left", &listed);
        assert_eq!(text, "Move left: A / —  (also Jump)");
    }
}
