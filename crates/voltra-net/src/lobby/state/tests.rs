//! The lobby's rules, one at a time.

use super::*;

fn config(max: usize, min: usize) -> LobbyConfig {
    LobbyConfig {
        max_players: max,
        min_players: min,
        ..LobbyConfig::default()
    }
}

fn peer(n: u32) -> PeerId {
    PeerId(n)
}

#[test]
fn the_first_in_hosts_and_the_host_passes_on_in_join_order() {
    let config = config(4, 1);
    let mut lobby = Lobby::new(&config);
    assert_eq!(lobby.host(), None);

    for n in 1..=3 {
        lobby.join(peer(n), "p", &config).expect("room");
    }
    assert_eq!(lobby.host(), Some(peer(1)));

    lobby.leave(peer(1)).expect("was in");
    assert_eq!(lobby.host(), Some(peer(2)));
    lobby.leave(peer(3)).expect("was in");
    assert_eq!(lobby.host(), Some(peer(2)));
    lobby.leave(peer(2)).expect("was in");
    assert_eq!(lobby.host(), None);
    assert!(lobby.leave(peer(2)).is_none(), "gone already");
}

#[test]
fn names_are_sanitised_and_made_unique() {
    let config = LobbyConfig {
        max_name_chars: 8,
        ..config(4, 1)
    };
    let mut lobby = Lobby::new(&config);

    assert_eq!(lobby.join(peer(1), "Ada\u{7}", &config), Ok("Ada".into()));
    assert_eq!(lobby.join(peer(2), "ada", &config), Ok("ada 2".into()));
    assert_eq!(
        lobby.join(peer(3), "a very long name", &config),
        Ok("a very l".into())
    );
    // A rename keeps a name of one's own, and takes no one else's.
    assert_eq!(lobby.join(peer(2), "Ada", &config), Ok("Ada 2".into()));
    assert_eq!(lobby.join(peer(1), "Ada", &config), Ok("Ada".into()));
    assert_eq!(lobby.len(), 3, "a rename is not a second entry");
}

#[test]
fn a_full_lobby_refuses_and_frees_a_slot_when_one_leaves() {
    let config = config(2, 1);
    let mut lobby = Lobby::new(&config);
    lobby.join(peer(1), "a", &config).expect("room");
    lobby.join(peer(2), "b", &config).expect("room");

    assert_eq!(lobby.join(peer(3), "c", &config), Err(LobbyRefusal::Full));
    assert!(lobby.is_full());
    lobby.leave(peer(1));
    assert!(lobby.join(peer(3), "c", &config).is_ok());
}

#[test]
fn the_host_starts_once_everyone_is_ready_and_there_are_enough() {
    let config = config(4, 2);
    let mut lobby = Lobby::new(&config);
    lobby.join(peer(1), "a", &config).expect("room");

    assert_eq!(lobby.set_ready(peer(1), true), Ok(true));
    assert_eq!(lobby.set_ready(peer(1), true), Ok(false), "no change");
    assert_eq!(
        lobby.start(Some(peer(1))),
        Err(LobbyError::TooFewPlayers { have: 1, need: 2 })
    );
    lobby.join(peer(2), "b", &config).expect("room");
    assert_eq!(
        lobby.start(Some(peer(1))),
        Err(LobbyError::NotEveryoneReady)
    );
    lobby.set_ready(peer(2), true).expect("in");
    assert_eq!(lobby.start(Some(peer(2))), Err(LobbyError::NotHost));
    assert_eq!(lobby.start(Some(peer(9))), Err(LobbyError::NotInLobby));
    assert_eq!(lobby.start(Some(peer(1))), Ok(()));
    assert_eq!(lobby.phase(), LobbyPhase::Playing);
    assert_eq!(lobby.start(Some(peer(1))), Err(LobbyError::AlreadyPlaying));
    assert_eq!(
        lobby.set_ready(peer(1), false),
        Err(LobbyError::AlreadyPlaying)
    );

    assert!(lobby.finish());
    assert!(!lobby.finish(), "finished already");
    assert_eq!(lobby.phase(), LobbyPhase::Waiting);
    assert!(lobby.entries().iter().all(|entry| !entry.ready));
}

#[test]
fn a_ready_player_leaving_can_make_the_rest_startable_or_not() {
    let config = config(4, 2);
    let mut lobby = Lobby::new(&config);
    for n in 1..=3 {
        lobby.join(peer(n), "p", &config).expect("room");
    }
    lobby.set_ready(peer(1), true).expect("in");
    lobby.set_ready(peer(2), true).expect("in");
    assert_eq!(lobby.ready_to_start(), Err(LobbyError::NotEveryoneReady));

    // The one not ready leaves: the rest can go.
    lobby.leave(peer(3));
    assert_eq!(lobby.ready_to_start(), Ok(()));
    // One more leaves: too few.
    lobby.leave(peer(2));
    assert_eq!(
        lobby.ready_to_start(),
        Err(LobbyError::TooFewPlayers { have: 1, need: 2 })
    );
}

#[test]
fn a_late_joiner_is_refused_or_plays_as_configured() {
    let refuse = config(4, 1);
    let mut lobby = Lobby::new(&refuse);
    lobby.join(peer(1), "a", &refuse).expect("room");
    lobby.set_ready(peer(1), true).expect("in");
    lobby.start(None).expect("the server may start it");

    assert_eq!(
        lobby.join(peer(2), "b", &refuse),
        Err(LobbyRefusal::MatchUnderway)
    );

    let play = LobbyConfig {
        late_join: LateJoin::Play,
        ..refuse
    };
    assert!(lobby.join(peer(2), "b", &play).is_ok());
    assert!(lobby.entry(peer(2)).is_some_and(|entry| entry.ready));
}

#[test]
fn a_player_not_in_cannot_be_ready() {
    let config = config(4, 1);
    let mut lobby = Lobby::new(&config);
    assert_eq!(lobby.set_ready(peer(1), true), Err(LobbyError::NotInLobby));
    assert!(!lobby.all_ready(), "an empty lobby is not ready");
}
