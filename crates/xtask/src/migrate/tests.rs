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
            .expect("every v1 fixture has a v2 twin")
            .replace("\r\n", "\n");
        let text = file.text.replace("\r\n", "\n");
        // The header says which version it is; the rest is the engine's.
        assert_eq!(
            body(&text),
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

/// Every file under `dir` rewritten with `ending` between lines.
fn with_endings(dir: &Path, ending: &str) {
    for entry in std::fs::read_dir(dir).expect("a folder").flatten() {
        let path = entry.path();
        if path.is_dir() {
            with_endings(&path, ending);
        } else {
            let text = std::fs::read_to_string(&path).expect("reads");
            let lf = text.replace("\r\n", "\n");
            std::fs::write(&path, lf.replace('\n', ending)).expect("written");
        }
    }
}

#[test]
fn a_crlf_checkout_is_neither_out_of_date_nor_mixed_by_a_migration() {
    let registry = ComponentRegistry::with_defaults();
    // Current files, as Git for Windows' `core.autocrlf` checks them out.
    let current = copy_of("v2", "crlf-current");
    with_endings(&current, "\r\n");
    let (seen, outdated) = survey(&current, &registry).expect("reads");
    assert_eq!((seen, outdated.len()), (4, 0), "{outdated:#?}");

    // Old files in CRLF are migrated in CRLF, every line of them.
    let old = copy_of("v1", "crlf-old");
    with_endings(&old, "\r\n");
    let (_, outdated) = survey(&old, &registry).expect("reads");
    assert_eq!(outdated.len(), 4);
    for file in &outdated {
        let lone = file.text.matches('\n').count() - file.text.matches("\r\n").count();
        assert_eq!(lone, 0, "{} has a bare LF", file.path.display());
        std::fs::write(&file.path, &file.text).expect("written");
    }
    let (_, outdated) = survey(&old, &registry).expect("reads");
    assert!(outdated.is_empty(), "{outdated:#?}");

    // A file of both is rewritten in one: here, mostly CRLF.
    let mixed = copy_of("v2", "crlf-mixed");
    let scene = mixed.join("scene.ron");
    let text = std::fs::read_to_string(&scene)
        .expect("reads")
        .replace("\r\n", "\n");
    let text = text.replace('\n', "\r\n").replacen("\r\n", "\n", 1);
    std::fs::write(&scene, text).expect("written");
    let (_, outdated) = survey(&mixed, &registry).expect("reads");
    assert_eq!(outdated.len(), 1, "{outdated:#?}");
    let fixed = &outdated[0].text;
    assert_eq!(fixed.matches('\n').count(), fixed.matches("\r\n").count());
}
