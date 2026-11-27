//! A lobby's change, as the events a game reads.

use super::super::event::LobbyEvent;
use super::super::state::{Lobby, LobbyPhase};

/// The events that take `before` to `after`: who left, who joined or
/// changed, who hosts, whether the match began or ended (or both, in
/// order, the lobby's [`matches`](Lobby::matches) counting the starts) —
/// in that order, so a game reading `Started` already has every `Joined`.
pub(super) fn changes(before: &Lobby, after: &Lobby, events: &mut Vec<LobbyEvent>) {
    // A match may have started — and finished — between the two.
    let started = after.matches() != before.matches();
    for gone in before.entries() {
        if !after.contains(gone.peer) {
            events.push(LobbyEvent::Left { peer: gone.peer });
        }
    }
    for entry in after.entries() {
        match before.entry(entry.peer) {
            None => events.push(LobbyEvent::Joined {
                peer: entry.peer,
                name: entry.name.clone(),
            }),
            Some(was) => {
                if was.name != entry.name {
                    events.push(LobbyEvent::Renamed {
                        peer: entry.peer,
                        name: entry.name.clone(),
                    });
                }
                if was.ready != entry.ready && !started && after.phase() == before.phase() {
                    events.push(LobbyEvent::ReadyChanged {
                        peer: entry.peer,
                        ready: entry.ready,
                    });
                }
            }
        }
    }
    if before.host() != after.host() {
        events.push(LobbyEvent::HostChanged(after.host()));
    }
    let was_playing = before.phase() == LobbyPhase::Playing;
    let playing = after.phase() == LobbyPhase::Playing;
    if was_playing && (started || !playing) {
        events.push(LobbyEvent::Finished);
    }
    if started {
        events.push(LobbyEvent::Started);
        if !playing {
            events.push(LobbyEvent::Finished);
        }
    }
}
