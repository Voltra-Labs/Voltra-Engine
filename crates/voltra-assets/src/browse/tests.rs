//! The asset browser's listing, and the kinds it gives each file.

mod listing {
    use super::super::*;

    /// A scratch tree: two directories, a PNG, a file with no loader, a
    /// dotfile.
    fn tree() -> voltra_testkit::Scratch {
        let root = voltra_testkit::scratch_root();
        std::fs::create_dir_all(root.join("sprites")).expect("sprites dir");
        std::fs::create_dir_all(root.join("Scenes")).expect("scenes dir");
        std::fs::write(root.join("sprites/checker.png"), b"not really a png").expect("png");
        std::fs::write(root.join("sprites/source.psd"), b"layers").expect("psd");
        std::fs::write(root.join("notes.txt"), b"hello").expect("txt");
        std::fs::write(root.join(".gitkeep"), b"").expect("dotfile");
        root
    }

    #[test]
    fn directories_come_first_then_names_case_insensitively() {
        let root = tree();
        let names: Vec<_> = list(&root, None)
            .expect("the root lists")
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        // `Scenes` before `sprites` despite the capital, and both before the
        // file.
        assert_eq!(names, ["Scenes", "sprites", "notes.txt"]);
    }

    #[test]
    fn a_dotfile_is_not_an_asset() {
        let root = tree();
        let entries = list(&root, None).expect("the root lists");
        assert!(entries.iter().all(|entry| entry.name != ".gitkeep"));
    }

    #[test]
    fn extension_decides_what_is_loadable() {
        let root = tree();
        let dir = AssetPath::new("sprites").expect("a valid path");
        let entries = list(&root, Some(&dir)).expect("the directory lists");

        let checker = &entries[0];
        assert_eq!(checker.kind, EntryKind::Texture);
        assert_eq!(checker.path.as_str(), "sprites/checker.png");

        let source = &entries[1];
        assert_eq!(source.kind, EntryKind::Other);
    }

    #[test]
    fn an_uppercase_extension_is_still_a_texture() {
        let root = voltra_testkit::scratch_root();
        std::fs::write(root.join("SHOUT.PNG"), b"").expect("png");
        let entries = list(&root, None).expect("the root lists");
        assert_eq!(entries[0].kind, EntryKind::Texture);
    }

    #[test]
    fn a_missing_directory_is_an_error_rather_than_an_empty_listing() {
        let root = voltra_testkit::scratch_root();
        let dir = AssetPath::new("nowhere").expect("a valid path");
        assert!(list(&root, Some(&dir)).is_err());
    }

    #[test]
    fn parent_walks_up_to_the_root_and_stops() {
        let deep = AssetPath::new("sprites/enemies/red.png").expect("a valid path");
        let up = parent(&deep).expect("a parent");
        assert_eq!(up.as_str(), "sprites/enemies");

        let up = parent(&up).expect("a parent");
        assert_eq!(up.as_str(), "sprites");

        assert!(parent(&up).is_none());
    }
}

mod atlas_tests {
    use super::super::*;

    #[test]
    fn a_slicing_is_its_own_kind_rather_than_unloadable() {
        let root = voltra_testkit::scratch_root();
        std::fs::write(root.join("coin.atlas.ron"), "(version: 1)").expect("the fixture writes");
        std::fs::write(root.join("demo.ron"), "(version: 1)").expect("the fixture writes");
        std::fs::write(root.join("label.ttf"), "").expect("the fixture writes");
        std::fs::write(root.join("default.input.ron"), "").expect("the fixture writes");
        std::fs::write(root.join("default.layers.ron"), "").expect("the fixture writes");
        std::fs::write(root.join("bob.anim.ron"), "").expect("the fixture writes");
        std::fs::write(root.join("hero.animator.ron"), "").expect("the fixture writes");
        std::fs::write(root.join("theme.clip.ron"), "").expect("the fixture writes");
        std::fs::write(root.join("default.buses.ron"), "").expect("the fixture writes");
        std::fs::write(root.join("default.render.ron"), "").expect("the fixture writes");
        std::fs::write(root.join("flash.material.ron"), "").expect("the fixture writes");

        let entries = list(&root, None).expect("the root lists");

        let kind = |name: &str| {
            entries
                .iter()
                .find(|entry| entry.name == name)
                .map(|entry| entry.kind)
        };
        assert_eq!(kind("coin.atlas.ron"), Some(EntryKind::Atlas));
        assert_eq!(kind("label.ttf"), Some(EntryKind::Font));
        assert_eq!(kind("default.input.ron"), Some(EntryKind::InputMap));
        assert_eq!(kind("default.layers.ron"), Some(EntryKind::SortingLayers));
        assert_eq!(kind("bob.anim.ron"), Some(EntryKind::Animation));
        assert_eq!(kind("hero.animator.ron"), Some(EntryKind::Animator));
        assert_eq!(
            kind("theme.clip.ron"),
            Some(EntryKind::Clip),
            "a clip's settings"
        );
        assert_eq!(kind("default.buses.ron"), Some(EntryKind::BusLayout));
        assert_eq!(kind("default.render.ron"), Some(EntryKind::RenderSettings));
        assert_eq!(kind("flash.material.ron"), Some(EntryKind::Material));
        assert_eq!(
            kind("demo.ron"),
            Some(EntryKind::Scene),
            "a scene is not a slicing"
        );
    }
}
