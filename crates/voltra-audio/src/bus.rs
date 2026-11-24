//! The named tracks a sound can be played on, and the table of their names.
//!
//! A bus is a group with one volume and a chain of [effects](crate::effect):
//! turn the music down, mute the effects, duck everything under a cutscene.
//! Every engine has them and every engine keeps the names somewhere
//! different — Unity in an `AudioMixer` asset, Godot in
//! `default_bus_layout.tres`, Unreal in `SoundClass` assets, Wwise in the
//! authoring tool. Here [`Buses::default`] is master, music, sfx and ui; a
//! game hands [`Audio::with_buses`](crate::Audio::with_buses) its own, and a project's
//! `default.buses.ron` adds to the table and sets each bus's volume and
//! effects ([`Audio::apply_layout`](crate::Audio::apply_layout)).
//!
//! **The name is what a scene file stores, and the [`BusId`] is what the mixer
//! sees.** A `.ron` file saying `bus: "music"` still means music after a bus
//! is inserted before it; an index would silently become something else. The
//! resolution happens once, on the main thread, in [`Buses::id`] — the audio
//! thread never compares a string.
//!
//! Routing is one level deep, which is Godot's shape: every bus feeds the
//! master, and a voice on the master feeds it directly. Buses feeding other
//! buses is a graph, and a graph wants a topology to validate and a cycle
//! check to fail; effects need none of it, since each bus runs its own
//! chain before it joins the master.

/// Which bus a voice is heard on.
///
/// An index, deliberately small and `Copy`: it is compared once per voice per
/// buffer on the audio thread, where a string would be an allocation and a
/// hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BusId(u8);

impl BusId {
    /// The bus everything else is summed into, and the one a voice with no
    /// group of its own plays on.
    ///
    /// Its volume is the master volume: applied to the finished sum, after
    /// every other bus has been added, which is what "routes into the master"
    /// means when the routing is written out.
    pub const MASTER: BusId = BusId(0);

    /// The id at `index`, which is only ever a slot [`Buses`] handed out.
    pub(crate) fn from_index(index: usize) -> Self {
        debug_assert!(index < MAX_BUSES, "Buses caps every id it issues");
        BusId(index as u8)
    }

    /// Where this bus sits in the mixer's tracks.
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

impl Default for BusId {
    /// The master, so a [`PlayParams`](crate::PlayParams) built from
    /// `Default` plays somewhere a volume slider the author never assigned
    /// cannot silence.
    fn default() -> Self {
        Self::MASTER
    }
}

/// How many buses can exist at once.
///
/// A ceiling on the mixer's per-buffer work rather than a design limit: every
/// bus is scanned once per buffer, and the number exists so a scene full of
/// misspelled names cannot grow that scan without bound. Thirty-two is more
/// groups than Unity ships in a default mixer and more than any 2D game has
/// wanted.
pub const MAX_BUSES: usize = 32;

/// The names this engine starts with.
///
/// Master first because it is [`BusId::MASTER`], then the three every mixing
/// desk in every engine has: music, effects, interface. They are a starting
/// table and not a rule — nothing in the engine treats `music` differently
/// from a bus a game invents.
pub const DEFAULT_BUSES: &[&str] = &["master", "music", "sfx", "ui"];

/// The names of the buses, in id order.
///
/// Lives on the main thread. The audio thread holds gains, not names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buses {
    /// Lowercase, always: see [`Buses::id`].
    names: Vec<String>,
}

impl Default for Buses {
    fn default() -> Self {
        Self::new(DEFAULT_BUSES.iter().copied())
    }
}

impl Buses {
    /// A table holding `names`, with the master in front of them.
    ///
    /// The master is inserted if the caller left it out rather than refused,
    /// because a table without one is not a table a mixer can use: id zero has
    /// to be the output. A caller that lists it keeps it where it is.
    pub fn new(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut table = Self {
            names: vec![DEFAULT_BUSES[0].to_owned()],
        };
        for name in names {
            table.id(&name.into());
        }
        table
    }

    /// The id `name` refers to, registering it if this is the first time.
    ///
    /// Registering rather than refusing, and this is the decision the stage
    /// turned on. A scene naming a bus nothing declared has to do *something*,
    /// and the alternatives are worse: refusing silences a sound the author
    /// can hear no reason for, and falling back to the master plays it at the
    /// right volume but leaves a slider that will never move it. Registering
    /// gives a working group at unity gain that the game can reach by the same
    /// name it was typed under — which is what a project with no settings file
    /// to declare buses in actually needs.
    ///
    /// Matching is case-insensitive and names are stored lowercase: `Music`
    /// and `music` are one bus, because a table where they are two is a bug
    /// that presents as a volume slider that works on some sounds.
    ///
    /// Past [`MAX_BUSES`] this warns once per name and answers
    /// [`BusId::MASTER`], which is audible at the right volume — the fallback
    /// that is wrong here is the silent one.
    pub fn id(&mut self, name: &str) -> BusId {
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return BusId::MASTER;
        }
        if let Some(found) = self.find(&name) {
            return found;
        }
        if self.names.len() >= MAX_BUSES {
            log::warn!("no room for the audio bus {name:?}; it will play on the master");
            return BusId::MASTER;
        }

        let id = BusId::from_index(self.names.len());
        log::debug!("audio bus {name:?} is {id:?}");
        self.names.push(name);
        id
    }

    /// The id `name` already has, without registering it.
    pub fn find(&self, name: &str) -> Option<BusId> {
        let name = name.trim().to_ascii_lowercase();
        self.names
            .iter()
            .position(|held| *held == name)
            .map(BusId::from_index)
    }

    /// What `id` is called, or `""` for an id from some other table.
    pub fn name(&self, id: BusId) -> &str {
        self.names.get(id.index()).map(String::as_str).unwrap_or("")
    }

    /// Every bus, in id order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// How many buses there are.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Never true: the master is always there.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_table_starts_with_the_master() {
        let buses = Buses::default();
        assert_eq!(buses.name(BusId::MASTER), "master");
        assert_eq!(buses.len(), DEFAULT_BUSES.len());
        assert_eq!(buses.find("sfx"), Some(BusId::from_index(2)));
    }

    #[test]
    fn a_game_s_own_table_still_gets_a_master() {
        // Id zero is the output. A table without one is not one the mixer can
        // route through, so the constructor supplies it.
        let buses = Buses::new(["dialogue", "footsteps"]);

        assert_eq!(buses.name(BusId::MASTER), "master");
        assert_eq!(buses.find("dialogue"), Some(BusId::from_index(1)));
        assert_eq!(buses.len(), 3);
    }

    #[test]
    fn a_table_that_lists_the_master_does_not_get_a_second_one() {
        let buses = Buses::new(["master", "music"]);
        assert_eq!(buses.len(), 2);
    }

    #[test]
    fn a_name_resolves_to_the_same_id_every_time() {
        let mut buses = Buses::default();
        assert_eq!(buses.id("music"), buses.id("music"));
    }

    #[test]
    fn case_and_padding_do_not_make_a_second_bus() {
        // The bug this prevents presents as a volume slider that works on some
        // of the music and not the rest.
        let mut buses = Buses::default();
        let before = buses.len();

        assert_eq!(buses.id("  Music "), buses.find("music").expect("declared"));
        assert_eq!(buses.len(), before);
    }

    #[test]
    fn a_name_nothing_declared_becomes_a_bus_of_its_own() {
        // The decision on `id`: a scene may name a bus this table has never
        // heard of, and it has to be audible at unity gain and reachable by
        // the same name afterwards.
        let mut buses = Buses::default();

        let dialogue = buses.id("dialogue");

        assert_ne!(dialogue, BusId::MASTER);
        assert_eq!(buses.name(dialogue), "dialogue");
        assert_eq!(buses.find("dialogue"), Some(dialogue));
    }

    #[test]
    fn an_empty_name_is_the_master_rather_than_a_bus_called_nothing() {
        let mut buses = Buses::default();
        let before = buses.len();

        assert_eq!(buses.id(""), BusId::MASTER);
        assert_eq!(buses.id("   "), BusId::MASTER);
        assert_eq!(buses.len(), before);
    }

    #[test]
    fn a_scene_full_of_misspelled_names_cannot_grow_the_table_without_bound() {
        let mut buses = Buses::default();

        for index in 0..MAX_BUSES * 2 {
            buses.id(&format!("bus{index}"));
        }

        assert_eq!(buses.len(), MAX_BUSES);
        assert_eq!(
            buses.id("one more"),
            BusId::MASTER,
            "and the overflow is audible, not silent"
        );
    }

    #[test]
    fn an_id_from_another_table_names_nothing_rather_than_panicking() {
        let buses = Buses::new(["music"]);
        assert_eq!(buses.name(BusId::from_index(9)), "");
    }
}
