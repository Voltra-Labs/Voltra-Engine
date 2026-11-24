use super::edit::Edited;
use super::*;

fn key(edit: Edit) -> EditKey {
    EditKey::plain(edit)
}

fn shift(edit: Edit) -> EditKey {
    EditKey {
        extend: true,
        ..EditKey::plain(edit)
    }
}

fn ctrl(edit: Edit) -> EditKey {
    EditKey {
        word: true,
        ..EditKey::plain(edit)
    }
}

#[test]
fn typing_goes_in_at_the_caret() {
    let mut field = UiTextField::with_text("held");
    field.apply(key(Edit::Left));
    field.apply(key(Edit::Left));
    assert!(field.insert("l"));
    assert_eq!(field.text, "helld");
    assert_eq!(field.caret, 3);
}

#[test]
fn characters_not_bytes() {
    let mut field = UiTextField::with_text("añb");
    field.apply(key(Edit::Left));
    field.apply(key(Edit::Backspace));
    assert_eq!(field.text, "ab");
    field.insert("日本");
    assert_eq!(field.text, "a日本b");
    assert_eq!(field.caret, 3);
}

#[test]
fn a_selection_is_replaced_by_what_is_typed() {
    let mut field = UiTextField::with_text("hello world");
    field.apply(ctrl(Edit::Left));
    field.apply(EditKey {
        extend: true,
        word: true,
        edit: Edit::Right,
    });
    assert_eq!(field.selected(), "world");
    field.insert("there");
    assert_eq!(field.text, "hello there");
}

#[test]
fn shift_selects_and_an_arrow_collapses_it() {
    let mut field = UiTextField::with_text("abcd");
    field.apply(shift(Edit::Left));
    field.apply(shift(Edit::Left));
    assert_eq!(field.selected(), "cd");
    field.apply(key(Edit::Left));
    assert_eq!((field.caret, field.anchor), (2, 2));
    field.apply(shift(Edit::Home));
    assert_eq!(field.selected(), "ab");
}

#[test]
fn select_all_then_delete_empties_it() {
    let mut field = UiTextField::with_text("save one");
    field.apply(key(Edit::SelectAll));
    let edited = field.apply(key(Edit::Delete));
    assert!(edited.changed);
    assert_eq!(field.text, "");
}

#[test]
fn ctrl_backspace_takes_a_word() {
    let mut field = UiTextField::with_text("the quick fox");
    field.apply(ctrl(Edit::Backspace));
    assert_eq!(field.text, "the quick ");
    field.apply(ctrl(Edit::Backspace));
    assert_eq!(field.text, "the ");
}

#[test]
fn nothing_to_delete_is_no_change() {
    let mut field = UiTextField::default();
    assert_eq!(field.apply(key(Edit::Backspace)), Edited::default());
    assert_eq!(field.apply(key(Edit::Delete)), Edited::default());
}

#[test]
fn control_characters_and_the_limit_are_kept_out() {
    let mut field = UiTextField {
        max_chars: Some(5),
        ..UiTextField::default()
    };
    field.insert("ab\ncd\r\tefgh");
    assert_eq!(field.text, "abcde");
    assert!(!field.insert("x"), "full");
}

#[test]
fn a_caret_left_past_the_end_is_brought_back() {
    let mut field = UiTextField::with_text("long name");
    field.text = "ab".to_owned();
    field.insert("c");
    assert_eq!(field.text, "abc");
}

#[test]
fn the_composition_shows_at_the_caret_until_it_commits() {
    let mut field = UiTextField::with_text("ab");
    field.caret = 1;
    field.anchor = 1;
    field.take(&TextInput {
        preedit: Some(Preedit {
            text: "にほ".to_owned(),
            cursor: Some((6, 6)),
        }),
        ..TextInput::default()
    });
    assert_eq!(field.display(), ("aにほb".to_owned(), false));
    assert_eq!(field.display_caret(), 3);
    assert_eq!(field.display_preedit(), Some((1, 3)));
    assert_eq!(field.text, "ab", "not typed yet");

    let edited = field.take(&TextInput {
        typed: "日本".to_owned(),
        preedit: Some(Preedit::default()),
        ..TextInput::default()
    });
    assert!(edited.changed);
    assert_eq!(field.text, "a日本b");
    assert_eq!(field.preedit, None);
}

#[test]
fn the_placeholder_shows_while_empty() {
    let field = UiTextField {
        placeholder: "Your name".to_owned(),
        ..UiTextField::default()
    };
    assert_eq!(field.display(), ("Your name".to_owned(), true));
}

#[test]
fn enter_submits() {
    let mut field = UiTextField::with_text("x");
    let edited = field.take(&TextInput {
        keys: vec![key(Edit::Submit)],
        ..TextInput::default()
    });
    assert!(edited.submitted && !edited.changed);
}

mod in_visual_order {
    use super::*;
    use crate::text::Text;
    use voltra_assets::{AssetPath, Fonts, DEFAULT_FONT};
    use voltra_testkit::{copy_workspace_asset, scratch_root, NOTO_HEBREW};

    fn fonts() -> Fonts {
        let root = scratch_root();
        copy_workspace_asset(&root, DEFAULT_FONT);
        copy_workspace_asset(&root, NOTO_HEBREW);
        let mut fonts = Fonts::new(root);
        fonts.set_fallbacks(&[AssetPath::new(NOTO_HEBREW).expect("a path")]);
        fonts
    }

    fn style(fonts: &Fonts) -> Text {
        Text {
            size: 20.0,
            font_handle: Some(fonts.default_font()),
            ..Text::new("")
        }
    }

    fn press(field: &mut UiTextField, edit: Edit, fonts: &Fonts, style: &Text) {
        let measure = |content: &str| view::stops(style, content, fonts);
        field.take_measured(
            &TextInput {
                keys: vec![key(edit)],
                ..TextInput::default()
            },
            &measure,
        );
    }

    #[test]
    fn the_right_arrow_walks_a_mixed_line_rightward_on_screen() {
        let fonts = fonts();
        let style = style(&fonts);
        let mut field = UiTextField::with_text("ab שלום");
        field.place_caret(0, false);
        let stops = view::stops(&style, &field.text, &fonts).expect("measured");

        let mut visited = vec![field.caret];
        for _ in 0..20 {
            press(&mut field, Edit::Right, &fonts, &style);
            if visited.last() == Some(&field.caret) {
                break;
            }
            visited.push(field.caret);
        }
        let xs: Vec<f32> = visited.iter().map(|i| stops[*i]).collect();
        assert!(xs.windows(2).all(|w| w[1] >= w[0]), "{visited:?} at {xs:?}");
        assert_eq!(visited.len(), stops.len(), "every position: {visited:?}");
        // Inside the Hebrew word, a right arrow moves back through it.
        assert!(visited.windows(2).any(|w| w[1] < w[0]), "{visited:?}");

        // And the left arrow walks it back.
        for _ in 0..20 {
            press(&mut field, Edit::Left, &fonts, &style);
        }
        assert_eq!(stops[field.caret], xs[0]);
    }

    #[test]
    fn without_a_font_the_arrows_stay_logical() {
        let mut field = UiTextField::with_text("שלום");
        field.place_caret(0, false);
        field.take_measured(
            &TextInput {
                keys: vec![key(Edit::Right)],
                ..TextInput::default()
            },
            &|_| None,
        );
        assert_eq!(field.caret, 1);
    }
}
