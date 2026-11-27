//! Tabs: pages of which one shows, picked by a row of buttons.

use voltra_ecs::{Entity, World};

use super::hidden::set_hidden;
use super::toggle::UiToggle;
use super::UiRect;
use crate::{hierarchy, Name};

/// A container showing one of its pages: the elements under it named by
/// its [`UiTab`] buttons. The others are [`UiHidden`](super::UiHidden), so
/// they keep every value the player set, take no room and no focus.
///
/// Godot's `TabContainer` pairs pages with titles by child order; here a
/// tab names its page, so the tab row and the pages can be laid out
/// anywhere under the container — a row above, a column beside — and a
/// page is found the way the options screen finds its parts, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UiTabs {
    /// The name of the page showing. Empty — or a name no tab gives — is
    /// the first tab's page.
    #[serde(default)]
    pub current: String,
}

impl UiTabs {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "UiTabs";
}

/// A button that shows its [`UiTabs`] container's page named
/// [`page`](Self::page) when clicked, or accepted on the focus. With a
/// [`UiToggle`] beside it, the toggle is on while its page shows — the
/// tab drawn selected.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UiTab {
    /// The [`Name`] of the page it shows, under the same container.
    #[serde(default)]
    pub page: String,
}

impl UiTab {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "UiTab";
}

/// The [`UiTabs`] container `entity` belongs to: the nearest one above it.
pub fn container_of(world: &World, entity: Entity) -> Option<Entity> {
    hierarchy::ancestors(world, entity)
        .into_iter()
        .find(|&e| world.get::<UiTabs>(e).is_some())
}

/// Shows `tab`'s page in its container; the container if that changed
/// which page shows.
pub fn select(world: &mut World, tab: Entity) -> Option<Entity> {
    let page = world.get::<UiTab>(tab)?.page.clone();
    let container = container_of(world, tab)?;
    let tabs = world.get_mut::<UiTabs>(container)?;
    if tabs.current == page {
        return None;
    }
    tabs.current = page;
    Some(container)
}

/// Brings every container's pages and tabs in line with its `current`:
/// the page showing, the others hidden, each tab's toggle on for its page.
/// Every frame, so a page spawned, a tab renamed or `current` set from a
/// tick is the next frame's.
pub fn apply(world: &mut World) {
    let containers: Vec<Entity> = world.query::<UiTabs>().map(|(e, _)| e).collect();
    for container in containers {
        let under = hierarchy::descendants(world, container);
        let own = |world: &World, e: Entity| container_of(world, e) == Some(container);
        let mut tabs: Vec<(i32, u32, Entity, String)> = under
            .iter()
            .filter(|&&e| own(world, e))
            .filter_map(|&e| {
                let tab = world.get::<UiTab>(e)?;
                let slot = world.get::<UiRect>(e).map_or(0, |r| r.slot);
                Some((slot, e.index(), e, tab.page.clone()))
            })
            .collect();
        tabs.sort();
        let Some(first) = tabs.first().map(|t| t.3.clone()) else {
            continue;
        };
        let current = world.get::<UiTabs>(container).map(|t| t.current.clone());
        let current = match current {
            Some(name) if tabs.iter().any(|t| t.3 == name) => name,
            _ => first,
        };
        let pages: Vec<Entity> = under
            .iter()
            .copied()
            .filter(|&e| own(world, e))
            .filter(|&e| {
                world
                    .get::<Name>(e)
                    .is_some_and(|name| tabs.iter().any(|t| t.3 == name.0))
            })
            .collect();
        for page in pages {
            let showing = world.get::<Name>(page).is_some_and(|n| n.0 == current);
            set_hidden(world, page, !showing);
        }
        for (_, _, tab, page) in tabs {
            if let Some(toggle) = world.get_mut::<UiToggle>(tab) {
                toggle.on = page == current;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::UiHidden;
    use crate::SceneId;

    fn element(world: &mut World, name: &str, parent: Option<Entity>) -> Entity {
        let e = world.spawn();
        world.insert(e, SceneId::new());
        world.insert(e, Name::new(name));
        world.insert(e, UiRect::default());
        if let Some(parent) = parent {
            hierarchy::set_parent(world, e, parent).expect("parents");
        }
        e
    }

    fn tab(world: &mut World, page: &str, parent: Entity, slot: i32) -> Entity {
        let e = element(world, &format!("{page} tab"), Some(parent));
        world.insert(
            e,
            UiTab {
                page: page.to_owned(),
            },
        );
        world.insert(e, UiToggle::default());
        if let Some(rect) = world.get_mut::<UiRect>(e) {
            rect.slot = slot;
        }
        e
    }

    #[test]
    fn one_page_shows_and_the_first_by_default() {
        let mut world = World::new();
        let root = element(&mut world, "Tabs", None);
        world.insert(root, UiTabs::default());
        let bar = element(&mut world, "Bar", Some(root));
        let a_tab = tab(&mut world, "A", bar, 0);
        let b_tab = tab(&mut world, "B", bar, 1);
        let a = element(&mut world, "A", Some(root));
        let b = element(&mut world, "B", Some(root));

        apply(&mut world);
        assert!(world.get::<UiHidden>(a).is_none());
        assert!(world.get::<UiHidden>(b).is_some());
        assert!(world.get::<UiToggle>(a_tab).is_some_and(|t| t.on));
        assert!(world.get::<UiToggle>(b_tab).is_some_and(|t| !t.on));

        assert_eq!(select(&mut world, b_tab), Some(root));
        assert_eq!(select(&mut world, b_tab), None, "already showing");
        apply(&mut world);
        assert!(world.get::<UiHidden>(a).is_some());
        assert!(world.get::<UiHidden>(b).is_none());
        assert!(world.get::<UiHidden>(bar).is_none(), "the bar is no page");
    }

    #[test]
    fn nested_tabs_keep_to_their_own_container() {
        let mut world = World::new();
        let outer = element(&mut world, "Outer", None);
        world.insert(outer, UiTabs::default());
        tab(&mut world, "P", outer, 0);
        let page = element(&mut world, "P", Some(outer));
        let inner = element(&mut world, "Inner", Some(page));
        world.insert(inner, UiTabs::default());
        tab(&mut world, "Q", inner, 0);
        tab(&mut world, "R", inner, 1);
        let q = element(&mut world, "Q", Some(inner));
        let r = element(&mut world, "R", Some(inner));
        apply(&mut world);
        assert!(world.get::<UiHidden>(page).is_none());
        assert!(world.get::<UiHidden>(q).is_none());
        assert!(world.get::<UiHidden>(r).is_some());
    }
}
