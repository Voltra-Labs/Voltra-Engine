use super::*;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../voltra-scene/tests/fixtures"
);

/// A copy of `fixtures/<version>` in a folder of its own.
fn copy_of(version: &str, name: &str) -> PathBuf {
    let to = std::env::temp_dir().join(format!("voltra-xtask-migrate-{name}"));
    let _ = std::fs::remove_dir_all(&to);
    copy(&Path::new(FIXTURES).join(version), &to);
    to
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a folder");
    for entry in std::fs::read_dir(from).expect("the fixtures").flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copied");
        }
    }
}

#[test]
fn check_fails_on_version_one_and_passes_after_a_migration() {
    let registry = ComponentRegistry::with_defaults();
    let folder = copy_of("v1", "check");
    let (seen, outdated) = survey(&folder, &registry).expect("reads");
    assert_eq!(seen, 4, "a scene, two prefabs and a clip");
    assert_eq!(outdated.len(), 4, "every one is out of date: {outdated:#?}");
    for file in &outdated {
        std::fs::write(&file.path, &file.text).expect("written");
    }
    let (seen, outdated) = survey(&folder, &registry).expect("reads");
    assert_eq!((seen, outdated.len()), (4, 0), "{outdated:#?}");
}

#[test]
fn a_migration_writes_the_version_two_twin_byte_for_byte() {
    let registry = ComponentRegistry::with_defaults();
    let folder = copy_of("v1", "twin");
    let (_, outdated) = survey(&folder, &registry).expect("reads");
    for file in outdated {
        let relative = file.path.strip_prefix(&folder).expect("under the copy");
        let twin = std::fs::read_to_string(Path::new(FIXTURES).join("v2").join(relative))
            .expect("every v1 fixture has a v2 twin");
        // The header says which version it is; the rest is the engine's.
        assert_eq!(
            body(&file.text),
            body(&twin),
            "{} is not its twin",
            relative.display()
        );
    }
    // And the twins are what this build writes already.
    let (seen, outdated) = survey(&Path::new(FIXTURES).join("v2"), &registry).expect("reads");
    assert_eq!((seen, outdated.len()), (4, 0), "{outdated:#?}");
}

fn body(text: &str) -> &str {
    &text[header(text).len()..]
}

#[test]
fn other_ron_files_are_left_alone() {
    let registry = ComponentRegistry::with_defaults();
    let folder = std::env::temp_dir().join("voltra-xtask-migrate-others");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("a folder");
    for (name, text) in [
        ("dark.theme.ron", "(version: 1)"),
        ("build.ron", "(version: 1, name: \"x\", assets: \"assets\")"),
        ("hero.animator.ron", "(start: \"idle\")"),
        ("default.layers.ron", "([\"Default\"])"),
    ] {
        std::fs::write(folder.join(name), text).expect("written");
    }
    let (seen, outdated) = survey(&folder, &registry).expect("reads");
    assert_eq!((seen, outdated.len()), (0, 0));
}

#[test]
fn a_broken_scene_is_an_error_not_a_skip() {
    let registry = ComponentRegistry::with_defaults();
    let folder = std::env::temp_dir().join("voltra-xtask-migrate-broken");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("a folder");
    std::fs::write(
        folder.join("level.ron"),
        "(version: 1, entities: [(id: \"019fe900-0000-7000-8000-000000000001\", \
         components: {\"Transform\": (oops)})])",
    )
    .expect("written");
    let error = survey(&folder, &registry).expect_err("a scene that will not read");
    assert!(error.contains("level.ron"), "{error}");
    std::fs::write(folder.join("level.ron"), "(version: 3, entities: [])").expect("written");
    let error = survey(&folder, &registry).expect_err("a newer version");
    assert!(error.contains("version 3"), "{error}");
}

#[test]
fn a_header_is_the_leading_comments_only() {
    assert_eq!(header("// a\n// b\n\n(\n// c\n)"), "// a\n// b\n\n");
    assert_eq!(header("(version: 2)"), "");
    assert_eq!(header("\n\n(version: 2)"), "");
}
