//! Who the server replicates to: every connected client, or only those the
//! game names — a lobby's players, so a peer that never joined (or was
//! refused, and waits out its grace) is sent nothing of the match.

use std::collections::BTreeSet;

use voltra_net::PeerId;

use super::ReplicationServer;

impl ReplicationServer {
    /// Replicates only to the connected clients among `peers`, from the
    /// next [`send`](Self::send): one leaving them is forgotten (sent the
    /// schema and everything again if named later), one joining them is
    /// sent the schema first, as a new client is. Call it every frame the
    /// set may have changed; an unchanged set costs a comparison.
    pub fn set_audience<I>(&mut self, peers: I)
    where
        I: IntoIterator<Item = PeerId>,
        I::IntoIter: Clone,
    {
        let peers = peers.into_iter();
        if let Some(current) = &self.audience {
            let mut count = 0;
            let same = peers.clone().all(|peer| {
                count += 1;
                current.contains(&peer)
            });
            if same && count == current.len() {
                return;
            }
        }
        self.audience = Some(peers.filter(|&peer| peer != PeerId::SERVER).collect());
        self.seat_audience();
    }

    /// Back to replicating to every connected client.
    pub fn clear_audience(&mut self) {
        if self.audience.take().is_some() {
            self.seat_audience();
        }
    }

    /// The clients named by [`set_audience`](Self::set_audience); `None`
    /// when every connected one is replicated to.
    pub fn audience(&self) -> Option<&BTreeSet<PeerId>> {
        self.audience.as_ref()
    }

    /// Whether `peer`, connected, is one to replicate to.
    pub(super) fn admits(&self, peer: PeerId) -> bool {
        self.audience
            .as_ref()
            .is_none_or(|audience| audience.contains(&peer))
    }

    /// A view for each connected client admitted, and none for any other.
    fn seat_audience(&mut self) {
        let admitted: Vec<PeerId> = self
            .connected
            .iter()
            .copied()
            .filter(|&peer| self.admits(peer))
            .collect();
        self.clients.retain(|peer, _| admitted.contains(peer));
        for peer in admitted {
            self.clients.entry(peer).or_default();
        }
    }
}
