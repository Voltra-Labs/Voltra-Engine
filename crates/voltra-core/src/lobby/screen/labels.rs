//! What the screen's rows and status line say.
//!
//! A row is two texts: a name someone else chose — a server's, a
//! player's — and what the screen says of it, in a column of its own. A
//! name is never one string with the words after it, so "Eve host" is a
//! name and nothing more, wherever it is drawn.

use voltra_net::{LobbyEntry, PeerId, ServerInfo};

/// One row of a list: the name, the screen's words about it, and whether
/// the row is greyed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Row {
    /// Someone else's words: shown as typed, never as markup.
    pub(super) name: String,
    /// The screen's own: "1/4 · full", "host · ready".
    pub(super) detail: String,
    /// Whether it cannot be picked.
    pub(super) greyed: bool,
}

/// Between the words of a row's detail.
const SEPARATOR: &str = " · ";

/// A server found on the LAN, greyed when it cannot be joined: one this
/// build cannot join, one with no room, one with no address for it.
pub(super) fn server(info: &ServerInfo) -> Row {
    let mut words = vec![format!("{}/{}", info.players, info.max_players)];
    if info.in_match {
        words.push("playing".into());
    }
    if !info.compatible {
        words.push("another version".into());
    } else if !info.has_room() {
        words.push("full".into());
    }
    Row {
        name: info.name.clone(),
        detail: words.join(SEPARATOR),
        greyed: !info.compatible || !info.has_room() || info.address().is_none(),
    }
}

/// A player in the lobby: their name, and whether they are this player,
/// host, are ready.
pub(super) fn player(entry: &LobbyEntry, host: Option<PeerId>, me: Option<PeerId>) -> Row {
    let mut words: Vec<&str> = Vec::new();
    if Some(entry.peer) == me {
        words.push("you");
    }
    if Some(entry.peer) == host {
        words.push("host");
    }
    words.push(if entry.ready { "ready" } else { "not ready" });
    Row {
        name: entry.name.clone(),
        detail: words.join(SEPARATOR),
        greyed: false,
    }
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

    fn row(name: &str, detail: &str, greyed: bool) -> Row {
        Row {
            name: name.into(),
            detail: detail.into(),
            greyed,
        }
    }

    #[test]
    fn a_server_row_says_who_and_how_many_and_greys_what_cannot_be_joined() {
        assert_eq!(server(&info()), row("Ada's game", "1/4", false));
        let other = ServerInfo {
            compatible: false,
            ..info()
        };
        assert_eq!(
            server(&other),
            row("Ada's game", "1/4 · another version", true)
        );
        let full = ServerInfo {
            players: 4,
            in_match: true,
            ..info()
        };
        assert_eq!(
            server(&full),
            row("Ada's game", "4/4 · playing · full", true)
        );
        let unreachable = ServerInfo {
            udp: None,
            ..info()
        };
        assert!(server(&unreachable).greyed);
    }

    #[test]
    fn a_player_row_marks_the_host_and_this_player_beside_the_name() {
        let entry = LobbyEntry {
            peer: PeerId(2),
            name: "Bob".into(),
            ready: true,
        };
        assert_eq!(
            player(&entry, Some(PeerId(1)), None),
            row("Bob", "ready", false)
        );
        assert_eq!(
            player(&entry, Some(PeerId(2)), Some(PeerId(2))),
            row("Bob", "you · host · ready", false)
        );
        let waiting = LobbyEntry {
            ready: false,
            ..entry
        };
        assert_eq!(player(&waiting, None, None).detail, "not ready");

        // A name that says "host" is still only a name.
        let eve = LobbyEntry {
            peer: PeerId(3),
            name: "Eve host ready".into(),
            ready: false,
        };
        let shown = player(&eve, Some(PeerId(1)), None);
        assert_eq!(shown.name, "Eve host ready");
        assert_eq!(shown.detail, "not ready");
    }
}
