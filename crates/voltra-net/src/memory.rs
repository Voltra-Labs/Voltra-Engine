//! A network inside one process: what tests, and the editor's several
//! clients (stage 117), connect through without a socket.
//!
//! Perfect on its own — every datagram arrives, in order, at once. Wrap an
//! endpoint in [`Conditioned`](crate::Conditioned) for loss, latency,
//! jitter and duplication; the wrapper is the same one any other transport
//! takes, so what a test simulates is what the editor can apply to UDP.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::transport::{check_size, Datagram, Transport, TransportError};
use crate::Address;

/// The most datagrams one endpoint holds unread; past it the newest are
/// dropped, as a full socket buffer drops them.
const MAX_QUEUED: usize = 65_536;

#[derive(Default)]
struct Hub {
    next: u32,
    queues: HashMap<u32, VecDeque<Datagram>>,
}

/// The shared medium: endpoints made from one network reach each other.
///
/// Cheap to clone — every clone is the same network — and `Send`, so a
/// test may run its server on another thread.
#[derive(Clone, Default)]
pub struct MemoryNetwork {
    hub: Arc<Mutex<Hub>>,
}

impl MemoryNetwork {
    /// An empty network.
    pub fn new() -> Self {
        Self::default()
    }

    /// A new endpoint, at the next free [`Address::Memory`].
    pub fn endpoint(&self) -> MemoryTransport {
        let mut hub = lock(&self.hub);
        let id = hub.next;
        hub.next = hub.next.wrapping_add(1);
        hub.queues.insert(id, VecDeque::new());
        MemoryTransport {
            hub: self.hub.clone(),
            id,
        }
    }
}

/// The hub, even if a thread panicked holding it: a test's other threads
/// should report their own failure, not a poisoned lock.
fn lock(hub: &Mutex<Hub>) -> MutexGuard<'_, Hub> {
    hub.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One endpoint of a [`MemoryNetwork`]. Dropping it unbinds its address;
/// datagrams sent there afterwards are lost, as UDP's would be.
pub struct MemoryTransport {
    hub: Arc<Mutex<Hub>>,
    id: u32,
}

impl MemoryTransport {
    /// This endpoint's address.
    pub fn address(&self) -> Address {
        Address::Memory(self.id)
    }
}

impl Transport for MemoryTransport {
    fn send(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError> {
        check_size(payload)?;
        let Address::Memory(id) = to else {
            return Err(TransportError::Unreachable(to));
        };
        let mut hub = lock(&self.hub);
        if let Some(queue) = hub.queues.get_mut(&id) {
            if queue.len() < MAX_QUEUED {
                queue.push_back(Datagram {
                    from: Address::Memory(self.id),
                    payload: payload.to_vec(),
                });
            }
        }
        Ok(())
    }

    fn recv(&mut self) -> Result<Option<Datagram>, TransportError> {
        let mut hub = lock(&self.hub);
        Ok(hub
            .queues
            .get_mut(&self.id)
            .and_then(|queue| queue.pop_front()))
    }

    fn reaches(&self, to: Address) -> bool {
        matches!(to, Address::Memory(_))
    }

    fn local_addr(&self) -> Option<Address> {
        Some(self.address())
    }
}

impl Drop for MemoryTransport {
    fn drop(&mut self) {
        lock(&self.hub).queues.remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_DATAGRAM;

    #[test]
    fn two_endpoints_exchange_datagrams_in_order() {
        let network = MemoryNetwork::new();
        let mut a = network.endpoint();
        let mut b = network.endpoint();

        a.send(b.address(), b"one").expect("sends");
        a.send(b.address(), b"two").expect("sends");

        let first = b.recv().expect("reads").expect("arrived");
        assert_eq!(first.from, a.address());
        assert_eq!(first.payload, b"one");
        assert_eq!(b.recv().expect("reads").expect("arrived").payload, b"two");
        assert!(b.recv().expect("reads").is_none());
    }

    #[test]
    fn a_dropped_endpoint_is_gone_and_sending_to_it_is_lost() {
        let network = MemoryNetwork::new();
        let mut a = network.endpoint();
        let b = network.endpoint();
        let gone = b.address();
        drop(b);

        a.send(gone, b"hello?").expect("lost, not an error");
        let c = network.endpoint();
        assert_ne!(c.address(), gone, "addresses are not reused");
    }

    #[test]
    fn the_size_limit_holds_here_too() {
        let network = MemoryNetwork::new();
        let mut a = network.endpoint();
        let to = a.address();

        assert!(matches!(
            a.send(to, &[0; MAX_DATAGRAM + 1]),
            Err(TransportError::TooLarge(_))
        ));
    }
}
