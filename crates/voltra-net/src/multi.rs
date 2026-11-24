//! Several transports as one: a server listening on UDP for desktop
//! players and on WebSockets for browser ones, in one session.

use std::time::Duration;

use crate::transport::{Datagram, Transport, TransportError};
use crate::Address;

/// Every transport it holds, read in turn and sent through by address kind.
#[derive(Default)]
pub struct MultiTransport {
    transports: Vec<Box<dyn Transport>>,
    /// Which transport the next `recv` starts at, so a busy one cannot
    /// starve the others.
    next: usize,
}

impl MultiTransport {
    /// None yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `transport`. A send goes to the first one that
    /// [`reaches`](Transport::reaches) its address.
    pub fn with(mut self, transport: impl Transport + 'static) -> Self {
        self.transports.push(Box::new(transport));
        self
    }

    /// How many transports there are.
    pub fn len(&self) -> usize {
        self.transports.len()
    }

    /// Whether there are none — a session that reaches nobody.
    pub fn is_empty(&self) -> bool {
        self.transports.is_empty()
    }
}

impl Transport for MultiTransport {
    fn update(&mut self, now: Duration) -> Result<(), TransportError> {
        // One broken transport must not silence the others: each is updated,
        // and the first error is reported after.
        let mut first = Ok(());
        for transport in &mut self.transports {
            if let Err(e) = transport.update(now) {
                if first.is_ok() {
                    first = Err(e);
                }
            }
        }
        first
    }

    fn send(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError> {
        match self.transports.iter_mut().find(|t| t.reaches(to)) {
            Some(transport) => transport.send(to, payload),
            None => Err(TransportError::Unreachable(to)),
        }
    }

    fn recv(&mut self) -> Result<Option<Datagram>, TransportError> {
        let count = self.transports.len();
        for offset in 0..count {
            let index = (self.next + offset) % count;
            if let Some(datagram) = self.transports[index].recv()? {
                self.next = (index + 1) % count;
                return Ok(Some(datagram));
            }
        }
        Ok(None)
    }

    fn reaches(&self, to: Address) -> bool {
        self.transports.iter().any(|t| t.reaches(to))
    }

    fn local_addr(&self) -> Option<Address> {
        self.transports.iter().find_map(|t| t.local_addr())
    }

    fn local_addrs(&self) -> Vec<Address> {
        self.transports
            .iter()
            .flat_map(|t| t.local_addrs())
            .collect()
    }

    fn take_closed(&mut self) -> Option<Address> {
        self.transports.iter_mut().find_map(|t| t.take_closed())
    }

    fn close(&mut self, peer: Address) {
        if let Some(transport) = self.transports.iter_mut().find(|t| t.reaches(peer)) {
            transport.close(peer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryNetwork;

    #[test]
    fn it_reads_every_transport_and_routes_by_kind() {
        let one = MemoryNetwork::new();
        let two = MemoryNetwork::new();
        let a = one.endpoint();
        let b = two.endpoint();
        let mut to_a = one.endpoint();
        let mut to_b = two.endpoint();
        to_a.send(a.address(), b"via one").expect("sends");
        to_b.send(b.address(), b"via two").expect("sends");
        let mut multi = MultiTransport::new().with(a).with(b);

        let mut got = Vec::new();
        while let Some(datagram) = multi.recv().expect("reads") {
            got.push(datagram.payload);
        }
        got.sort();

        assert_eq!(got, vec![b"via one".to_vec(), b"via two".to_vec()]);
        assert!(matches!(
            multi.send(Address::Server, b"x"),
            Err(TransportError::Unreachable(_))
        ));
    }
}
