use voltra_testkit::scratch_root;

use super::*;

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Progress {
    level: String,
    coins: u32,
}

fn progress() -> Progress {
    Progress {
        level: "scenes/level2.ron".to_owned(),
        coins: 7,
    }
}

#[test]
fn a_save_reads_back_as_it_was_written() {
    let saves = Saves::in_dir(scratch_root().join("saves"));
    saves.save("slot1", &progress()).expect("saves");
    assert_eq!(
        saves.load::<Progress>("slot1").expect("loads"),
        Some(progress())
    );
}

#[test]
fn an_empty_slot_is_none_not_an_error() {
    let saves = Saves::in_dir(scratch_root());
    assert_eq!(saves.load::<Progress>("never").expect("no error"), None);
}

#[test]
fn saving_again_replaces_and_leaves_no_temporary() {
    let dir = scratch_root();
    let saves = Saves::in_dir(&dir);
    saves.save("slot1", &progress()).expect("saves");
    let mut later = progress();
    later.coins = 9;
    saves.save("slot1", &later).expect("saves again");
    assert_eq!(saves.load::<Progress>("slot1").expect("loads"), Some(later));
    assert_eq!(std::fs::read_dir(&dir).expect("lists").count(), 1);
}

#[test]
fn a_slot_name_cannot_leave_the_folder() {
    let saves = Saves::in_dir(scratch_root());
    for bad in ["../escape", "a/b", "C:x", "", "dots.ron", &"x".repeat(65)] {
        assert!(
            matches!(saves.save(bad, &progress()), Err(SaveError::BadSlot(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn a_save_of_another_shape_is_an_error_not_a_fresh_start() {
    let dir = scratch_root();
    std::fs::write(dir.join("slot1.save.ron"), "(name: \"other\")").expect("writes");
    let saves = Saves::in_dir(&dir);
    assert!(matches!(
        saves.load::<Progress>("slot1"),
        Err(SaveError::Parse { .. })
    ));
}

#[test]
fn an_oversized_save_is_refused_before_it_is_read() {
    let dir = scratch_root();
    let file = std::fs::File::create(dir.join("big.save.ron")).expect("creates");
    file.set_len(MAX_SAVE_BYTES + 1).expect("grows");
    let saves = Saves::in_dir(&dir);
    assert!(matches!(
        saves.load::<Progress>("big"),
        Err(SaveError::TooLarge { .. })
    ));
}

#[test]
fn slots_lists_only_slots_sorted() {
    let dir = scratch_root();
    let saves = Saves::in_dir(&dir);
    saves.save("b", &progress()).expect("saves");
    saves.save("a", &progress()).expect("saves");
    std::fs::write(dir.join("a.save.ron.tmp"), "half").expect("writes");
    std::fs::write(dir.join("notes.txt"), "mine").expect("writes");
    assert_eq!(saves.slots(), vec!["a", "b"]);
}

#[test]
fn delete_reports_whether_there_was_a_slot() {
    let saves = Saves::in_dir(scratch_root());
    saves.save("slot1", &progress()).expect("saves");
    assert!(saves.delete("slot1").expect("deletes"));
    assert!(!saves.delete("slot1").expect("nothing to delete"));
    assert_eq!(saves.load::<Progress>("slot1").expect("loads"), None);
}

#[test]
fn with_no_folder_every_write_says_so() {
    let saves = Saves { dir: None };
    assert!(matches!(
        saves.save("slot1", &progress()),
        Err(SaveError::NoSaveDir)
    ));
    assert!(saves.slots().is_empty());
}
