//! Naming a pasted copy apart from its new siblings: `Name (1)`,
//! `Name (2)`…
//!
//! Unity's rule (`GameObjectUtility.GetUniqueNameForSibling`, whose
//! `ObjectNames.GetUniqueName` takes "the next available increment"): a
//! copy keeps its name when none of its siblings has it — a paste into
//! another scene or under another parent is still `Player` — and otherwise
//! takes its name's base, any ` (n)` suffix stripped, with the first number
//! from 1 that no sibling has: `Coin` beside `Coin (1)` → `Coin (2)`, and
//! `Coin (1)` duplicated beside `Coin` and `Coin (3)` → `Coin (2)`. Only a
//! copy's tops are renamed; what is under them keeps its names, as its
//! siblings are copies too. A nameless entity stays nameless.

use voltra_ecs::{Entity, World};
use voltra_scene::{hierarchy, Name};

/// `name` split into its base and the number of a trailing ` (n)`, if it
/// has one: `Coin (3)` is `("Coin", Some(3))`; `Coin(3)` and `(3)` have
/// none, the first lacking Unity's space and the second a base.
pub(super) fn split(name: &str) -> (&str, Option<u64>) {
    let numbered = name
        .strip_suffix(')')
        .and_then(|rest| rest.rsplit_once(" ("))
        .filter(|(base, digits)| {
            !base.is_empty() && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
        })
        .and_then(|(base, digits)| digits.parse().ok().map(|n| (base, n)));
    match numbered {
        Some((base, n)) => (base, Some(n)),
        None => (name, None),
    }
}

/// The name a copy called `name` takes among siblings named `taken`.
pub(super) fn unique(name: &str, taken: &[&str]) -> String {
    if !taken.contains(&name) {
        return name.to_owned();
    }
    let (base, _) = split(name);
    // Each sibling holds at most one number, so one of the first
    // `taken.len() + 1` is free: the count ends there, whatever number the
    // name itself had.
    let mut n: usize = 1;
    loop {
        let candidate = format!("{base} ({n})");
        if !taken.contains(&candidate.as_str()) {
            return candidate;
        }
        n += 1;
    }
}

/// Renames each of `tops`, in order, apart from the siblings it now has.
/// Earlier tops count as siblings as they were named; later ones, not yet
/// named, do not — so two copies of `Coin` pasted where nothing is called
/// that are `Coin` and `Coin (1)`, and beside an original `Coin`,
/// `Coin (1)` and `Coin (2)`. Answers the tops renamed.
pub(super) fn number(world: &mut World, tops: &[Entity]) -> Vec<Entity> {
    let mut renamed = Vec::new();
    for (i, &top) in tops.iter().enumerate() {
        let later = &tops[i + 1..];
        let Some(name) = world.get::<Name>(top).map(|n| n.as_str().to_owned()) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let siblings = match hierarchy::parent_of(world, top) {
            Some(parent) => hierarchy::children_of(world, parent),
            None => hierarchy::roots(world),
        };
        let names: Vec<String> = siblings
            .into_iter()
            .filter(|&sibling| sibling != top && !later.contains(&sibling))
            .filter_map(|sibling| world.get::<Name>(sibling).map(|n| n.as_str().to_owned()))
            .collect();
        let taken: Vec<&str> = names.iter().map(String::as_str).collect();
        let fresh = unique(&name, &taken);
        if fresh != name {
            world.insert(top, Name::new(fresh));
            renamed.push(top);
        }
    }
    renamed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trailing_number_is_split_off_only_as_unity_writes_it() {
        assert_eq!(split("Coin (3)"), ("Coin", Some(3)));
        assert_eq!(split("Big Coin (12)"), ("Big Coin", Some(12)));
        assert_eq!(split("Coin"), ("Coin", None));
        assert_eq!(split("Coin(3)"), ("Coin(3)", None));
        assert_eq!(split("(3)"), ("(3)", None));
        assert_eq!(split("Coin (x)"), ("Coin (x)", None));
        assert_eq!(split("Coin ()"), ("Coin ()", None));
        assert_eq!(split("Coin (-1)"), ("Coin (-1)", None));
    }

    #[test]
    fn a_name_no_sibling_has_is_kept() {
        assert_eq!(unique("Player", &[]), "Player");
        assert_eq!(unique("Coin (3)", &["Coin"]), "Coin (3)");
    }

    #[test]
    fn a_taken_name_gets_the_first_free_number() {
        assert_eq!(unique("Coin", &["Coin"]), "Coin (1)");
        assert_eq!(
            unique("Coin", &["Coin", "Coin (1)", "Coin (2)"]),
            "Coin (3)"
        );
        // Skips what is taken, fills what is not.
        assert_eq!(
            unique("Coin", &["Coin", "Coin (1)", "Coin (3)"]),
            "Coin (2)"
        );
        // Its own suffix stripped first: the base is what is numbered.
        assert_eq!(unique("Coin (3)", &["Coin (3)"]), "Coin (1)");
        assert_eq!(
            unique("Coin (3)", &["Coin (1)", "Coin (2)", "Coin (3)"]),
            "Coin (4)"
        );
        assert_eq!(
            unique("Coin (1)", &["Coin", "Coin (1)", "Coin (2)"]),
            "Coin (3)"
        );
    }

    #[test]
    fn copies_side_by_side_number_apart_and_children_are_left_alone() {
        let mut world = World::new();
        let spawn = |world: &mut World, name: &str| {
            let e = world.spawn();
            world.insert(e, voltra_scene::SceneId::new());
            world.insert(e, Name::new(name));
            e
        };
        let original = spawn(&mut world, "Coin");
        let first = spawn(&mut world, "Coin");
        let second = spawn(&mut world, "Coin");
        let nameless = world.spawn();
        world.insert(nameless, voltra_scene::SceneId::new());
        let child = spawn(&mut world, "Coin");
        hierarchy::set_parent(&mut world, child, first).expect("no cycle");

        let renamed = number(&mut world, &[first, second, nameless]);

        assert_eq!(renamed, [first, second]);
        let name = |e| world.get::<Name>(e).map(|n| n.as_str().to_owned());
        assert_eq!(name(original).as_deref(), Some("Coin"));
        assert_eq!(name(first).as_deref(), Some("Coin (1)"));
        assert_eq!(name(second).as_deref(), Some("Coin (2)"));
        assert_eq!(name(nameless), None);
        assert_eq!(name(child).as_deref(), Some("Coin"), "no sibling has it");
    }

    #[test]
    fn copies_not_yet_numbered_are_not_in_the_way() {
        let mut world = World::new();
        let spawn = |world: &mut World| {
            let e = world.spawn();
            world.insert(e, voltra_scene::SceneId::new());
            world.insert(e, Name::new("Coin"));
            e
        };
        // Pasted where nothing is called that: the first keeps its name.
        let first = spawn(&mut world);
        let second = spawn(&mut world);
        assert_eq!(number(&mut world, &[first, second]), [second]);
        let name = |e| world.get::<Name>(e).map(|n| n.as_str().to_owned());
        assert_eq!(name(first).as_deref(), Some("Coin"));
        assert_eq!(name(second).as_deref(), Some("Coin (1)"));
    }

    #[test]
    fn a_huge_number_is_stripped_like_any_other() {
        let max = format!("Coin ({})", u64::MAX);
        let near = format!("Coin ({})", u64::MAX - 1);
        assert_eq!(split(&max), ("Coin", Some(u64::MAX)));
        assert_eq!(unique(&max, &[&max]), "Coin (1)");
        assert_eq!(unique(&near, &[&near, "Coin (1)"]), "Coin (2)");
        // Past what a number holds: no number, the whole name the base.
        let past = "Coin (99999999999999999999999)";
        assert_eq!(split(past), (past, None));
        assert_eq!(unique(past, &[past]), format!("{past} (1)"));
    }
}
