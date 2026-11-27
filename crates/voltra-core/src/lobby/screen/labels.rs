//! What the screen's rows and status line say.

use voltra_net::{LobbyEntry, PeerId, ServerInfo};

/// A server found on the LAN, and whether its row is greyed: one this
/// build cannot join, one with no room, one with no address for it.
pub(super) fn server(info: &ServerInfo) -> (String, bool) {
    let mut words = format!("{}  {}/{}", info.name, info.players, info.max_players);
    if info.in_match {
        words.push_str("  playing");
    }
    if !info.compatible {
        words.push_str("  another version");
    } else if !info.has_room() {
        words.push_str("  full");
    }
    let greyed = !info.compatible || !info.has_room() || info.address().is_none();
    (words, greyed)
}

/// A player in the lobby: their name, and whether they host, are ready,
/// are this player.
pub(super) fn player(entry: &LobbyEntry, host: Option<PeerId>, me: Option<PeerId>) -> String {
    let mut words = entry.name.clone();
    if Some(entry.peer) == me {
        words.push_str(" (you)");
    }
    if Some(entry.peer) == host {
        words.push_str("  host");
    }
    words.push_str(if entry.ready {
        "  ready"
    } else {
        "  not ready"
    });
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> ServerInfo {
        ServerInfo {
            name: "Ada's game".into(),
            game: "arena".into(),
            players: 1,
            max_players: 4,
            in_match: false,
            compatible: true,
            from: ([127, 0, 0, 1], 7779).into(),
            udp: Some("udp://127.0.0.1:7777".into()),
            websocket: None,
        }
    }

    #[test]
    fn a_server_row_says_who_and_how_many_and_greys_what_cannot_be_joined() {
        assert_eq!(server(&info()), ("Ada's game  1/4".into(), false));
        let other = ServerInfo {
            compatible: false,
            ..info()
        };
        assert_eq!(
            server(&other),
            ("Ada's game  1/4  another version".into(), true)
        );
        let full = ServerInfo {
            players: 4,
            in_match: true,
            ..info()
        };
        assert_eq!(
            server(&full),
            ("Ada's game  4/4  playing  full".into(), true)
        );
        let unreachable = ServerInfo {
            udp: None,
            ..info()
        };
        assert!(server(&unreachable).1);
    }

    #[test]
    fn a_player_row_marks_the_host_and_this_player() {
        let entry = LobbyEntry {
            peer: PeerId(2),
            name: "Bob".into(),
            ready: true,
        };
        assert_eq!(player(&entry, Some(PeerId(1)), None), "Bob  ready");
        assert_eq!(
            player(&entry, Some(PeerId(2)), Some(PeerId(2))),
            "Bob (you)  host  ready"
        );
        let waiting = LobbyEntry {
            ready: false,
            ..entry
        };
        assert_eq!(player(&waiting, None, None), "Bob  not ready");
    }
}
