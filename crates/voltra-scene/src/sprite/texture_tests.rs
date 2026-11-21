use super::*;
use voltra_assets::AssetPath;

#[test]
fn default_sprite_has_no_texture() {
    let sprite = Sprite::default();
    assert!(sprite.texture.is_none());
    assert!(sprite.texture_handle.is_none());
}

#[test]
fn texture_path_round_trips_through_ron_without_the_handle() {
    // No handle is forged here: `Handle::new` is `pub(crate)` to
    // `voltra-assets`, and a handle only ever comes from `Textures::load`.
    // Setting just `texture` and checking `texture_handle` stays `None`
    // through a round trip is the same claim without an API hole.
    let sprite = Sprite {
        texture: Some(AssetPath::new("sprites/hero.png").expect("valid")),
        ..Default::default()
    };

    let text = ron::to_string(&sprite).expect("serialize");
    assert!(
        !text.contains("texture_handle"),
        "handle leaked into RON: {text}"
    );
    let back: Sprite = ron::from_str(&text).expect("deserialize");
    assert_eq!(
        back.texture.as_ref().map(AssetPath::as_str),
        Some("sprites/hero.png")
    );
    assert!(back.texture_handle.is_none(), "handle must not deserialize");
}

#[test]
fn old_ron_without_texture_field_still_loads() {
    let text = "(color:(1.0,1.0,1.0,1.0),sort_order:0)";
    let sprite: Sprite = ron::from_str(text).expect("old scene shape");
    assert!(sprite.texture.is_none());
    assert!(sprite.texture_handle.is_none());
}

#[test]
fn hostile_texture_path_is_rejected_on_deserialize() {
    let hostile =
        r#"(color:(1.0,1.0,1.0,1.0),sort_order:0,texture:Some(Path("../../etc/passwd")))"#;
    assert!(ron::from_str::<Sprite>(hostile).is_err());
}

#[test]
fn a_normal_map_round_trips_without_its_handle_and_is_left_out_when_absent() {
    let text = ron::to_string(&Sprite::default()).expect("writes");
    assert!(!text.contains("normal_map"), "{text}");
    let sprite = Sprite {
        normal_map: Some(AssetPath::new("sprites/brick_n.png").expect("a valid path")),
        ..Sprite::default()
    };
    let text = ron::to_string(&sprite).expect("writes");
    let read: Sprite = ron::from_str(&text).expect("reads");
    assert_eq!(read.normal_map, sprite.normal_map);
    assert_eq!(read.normal_handle, None, "a handle never comes off disk");
}
