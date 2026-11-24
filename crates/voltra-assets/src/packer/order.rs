//! Which index each region gets, so a re-pack keeps what scenes point at.
//!
//! A sprite names its frame by index. A re-pack that sorted the files afresh
//! would renumber every region after a new file's name and silently swap
//! what half a level draws. So a region keeps the index it had: the previous
//! atlas's names come first, in their order, and new files after them.

use std::collections::BTreeSet;

/// One index of the packed atlas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot {
    /// The region cut from the file called this.
    Region(String),
    /// A region the previous atlas had and no file supplies any more. Kept
    /// as an empty frame, so the regions after it keep their indices, and
    /// what still names it draws the missing-frame checker instead of
    /// whichever region slid into its place.
    Gone(Option<String>),
}

/// The slots for regions called `names`, given the previous atlas's frame
/// names in index order (`None` for an unnamed frame).
///
/// Gone slots at the end are dropped: nothing after them can shift.
pub fn slots(names: &BTreeSet<String>, previous: &[Option<String>]) -> Vec<Slot> {
    let mut placed = BTreeSet::new();
    let mut slots: Vec<Slot> = previous
        .iter()
        .map(|name| match name {
            Some(name) if names.contains(name) && placed.insert(name.clone()) => {
                Slot::Region(name.clone())
            }
            other => Slot::Gone(other.clone()),
        })
        .collect();
    while matches!(slots.last(), Some(Slot::Gone(_))) {
        slots.pop();
    }
    slots.extend(
        names
            .iter()
            .filter(|name| !placed.contains(*name))
            .map(|name| Slot::Region(name.clone())),
    );
    slots
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    fn previous(list: &[&str]) -> Vec<Option<String>> {
        list.iter().map(|name| Some((*name).to_owned())).collect()
    }

    fn region(name: &str) -> Slot {
        Slot::Region(name.to_owned())
    }

    #[test]
    fn a_first_pack_is_in_name_order() {
        assert_eq!(
            slots(&names(&["b", "a"]), &[]),
            vec![region("a"), region("b")]
        );
    }

    #[test]
    fn a_new_file_goes_after_the_regions_already_there() {
        assert_eq!(
            slots(&names(&["a", "b", "c"]), &previous(&["c", "b"])),
            vec![region("c"), region("b"), region("a")]
        );
    }

    #[test]
    fn a_removed_file_keeps_its_index_empty() {
        assert_eq!(
            slots(&names(&["a", "c"]), &previous(&["a", "b", "c"])),
            vec![region("a"), Slot::Gone(Some("b".into())), region("c")]
        );
    }

    #[test]
    fn removed_files_at_the_end_shift_nothing_and_go() {
        assert_eq!(
            slots(&names(&["a"]), &previous(&["a", "b", "c"])),
            vec![region("a")]
        );
    }

    #[test]
    fn a_name_listed_twice_before_keeps_its_first_index() {
        assert_eq!(
            slots(&names(&["a", "b"]), &previous(&["a", "a", "b"])),
            vec![region("a"), Slot::Gone(Some("a".into())), region("b")]
        );
    }
}
