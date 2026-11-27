//! Native UDP: the transport a desktop game and its server talk over.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use crate::transport::{check_size, Datagram, Transport, TransportError, MAX_DATAGRAM};
use crate::Address;

/// How many datagrams one [`recv`](Transport::recv) call reads past and
/// throws away — over-long ones, ICMP errors — before it answers `None` for
/// this tick. A flood of junk then costs a bounded amount of work per tick
/// instead of spinning the loop.
const MAX_SKIPPED_PER_CALL: usize = 256;

/// A non-blocking UDP socket.
pub struct UdpTransport {
    socket: UdpSocket,
    local: SocketAddr,
    /// One byte longer than any datagram we accept: a read that fills it
    /// was truncated, which is how an over-long datagram is told apart.
    buffer: Box<[u8]>,
}

impl UdpTransport {
    /// Binds a socket at `addr` — `"0.0.0.0:7777"` for a server, `"0.0.0.0:0"`
    /// (any free port) for a client.
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        let local = socket.local_addr()?;
        Ok(Self {
            socket,
            local,
            buffer: vec![0; MAX_DATAGRAM + 1].into_boxed_slice(),
        })
    }

    /// The socket address bound — with the real port when `0` was asked for.
    pub fn local_socket(&self) -> SocketAddr {
        self.local
    }
}

impl Transport for UdpTransport {
    fn send(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError> {
        check_size(payload)?;
        let Address::Udp(addr) = to else {
            return Err(TransportError::Unreachable(to));
        };
        match self.socket.send_to(payload, addr) {
            Ok(_) => Ok(()),
            // The send buffer is full: dropped, as a router would.
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                log::debug!("udp send buffer full; dropped a datagram to {addr}");
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    fn recv(&mut self) -> Result<Option<Datagram>, TransportError> {
        for _ in 0..MAX_SKIPPED_PER_CALL {
            match self.socket.recv_from(&mut self.buffer) {
                Ok((len, from)) if len > MAX_DATAGRAM => {
                    log::debug!("dropped an over-long datagram from {from}");
                }
                Ok((len, from)) => {
                    return Ok(Some(Datagram {
                        from: Address::Udp(from),
                        payload: self.buffer[..len].to_vec(),
                    }));
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                Err(e) if is_transient(&e) => {
                    log::debug!("udp receive skipped: {e}");
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(None)
    }

    fn reaches(&self, to: Address) -> bool {
        matches!(to, Address::Udp(_))
    }

    fn local_addr(&self) -> Option<Address> {
        Some(Address::Udp(self.local))
    }
}

/// Errors a UDP read reports about *some other* datagram, not about the
/// socket: an ICMP "port unreachable" for an earlier send (Windows hands it
/// to the next `recv_from` as a reset), an interrupted call, or Windows'
/// "message too long" for a truncated datagram.
pub(crate) fn is_transient(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionRefused
            | io::ErrorKind::Interrupted
    ) || is_too_long(error)
}

/// Whether `error` is Windows' "message too long": a datagram longer than
/// the buffer arrived, and was cut. Elsewhere the read succeeds with the
/// buffer filled, so a reader that sizes its buffer one past the largest
/// message it takes treats both the same: a datagram it does not want.
pub(crate) fn is_too_long(error: &io::Error) -> bool {
    const WSAEMSGSIZE: i32 = 10040;
    cfg!(windows) && error.raw_os_error() == Some(WSAEMSGSIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_datagram_over_the_limit_is_refused_before_it_is_sent() {
        let mut transport = UdpTransport::bind("127.0.0.1:0").expect("binds");
        let to = Address::Udp(transport.local_socket());

        let error = transport
            .send(to, &[0; MAX_DATAGRAM + 1])
            .expect_err("too large");

        assert!(matches!(error, TransportError::TooLarge(len) if len == MAX_DATAGRAM + 1));
    }

    #[test]
    fn another_kind_of_address_is_unreachable() {
        let mut transport = UdpTransport::bind("127.0.0.1:0").expect("binds");

        assert!(!transport.reaches(Address::Memory(1)));
        assert!(matches!(
            transport.send(Address::Memory(1), b"hi"),
            Err(TransportError::Unreachable(_))
        ));
    }

    #[test]
    fn nothing_waiting_is_none_not_a_block() {
        let mut transport = UdpTransport::bind("127.0.0.1:0").expect("binds");

        assert!(transport.recv().expect("reads").is_none());
    }
}
