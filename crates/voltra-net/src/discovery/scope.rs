//! Whom a server answers: its own network, or anyone.
//!
//! A broadcast probe never crosses a router, so every probe a LAN game
//! wants comes from a private, link-local or loopback address. One from
//! a public address is a stranger asking directly — a scanner, or a
//! forged source that would turn the answers on someone else — and a
//! responder that answers it serves only them.

use std::net::IpAddr;

/// Which sources a [`DiscoveryResponder`](super::DiscoveryResponder)
/// answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiscoveryScope {
    /// Only private, link-local and loopback sources: IPv4 `10/8`,
    /// `172.16/12`, `192.168/16`, `169.254/16`, `127/8`; IPv6 `fc00::/7`,
    /// `fe80::/10`, `::1` (and those IPv4 ones as IPv6-mapped).
    #[default]
    Local,
    /// Any source: a server probed by address across networks
    /// (`--discovery-public` for `voltra-server`).
    Anyone,
}

impl DiscoveryScope {
    /// Whether a probe from `ip` is answered.
    pub fn admits(self, ip: IpAddr) -> bool {
        match self {
            Self::Anyone => true,
            Self::Local => is_local(ip),
        }
    }
}

/// Whether `ip` is a private, link-local or loopback address.
pub fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_loopback(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_local(IpAddr::V4(v4)),
            None => {
                let first = v6.segments()[0];
                v6.is_loopback() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().expect("an address")
    }

    #[test]
    fn only_the_local_ranges_are_local() {
        for local in [
            "10.0.0.1",
            "10.255.255.255",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.1.20",
            "169.254.3.4",
            "127.0.0.1",
            "127.8.9.10",
            "::1",
            "fc00::1",
            "fd12:3456::7",
            "fe80::1",
            "febf::1",
            "::ffff:192.168.0.5",
        ] {
            assert!(is_local(ip(local)), "{local}");
            assert!(DiscoveryScope::Local.admits(ip(local)), "{local}");
        }
        for public in [
            "8.8.8.8",
            "172.15.255.255",
            "172.32.0.1",
            "192.169.0.1",
            "169.255.0.1",
            "100.64.0.1",
            "1.1.1.1",
            "2001:db8::1",
            "fec0::1",
            "::ffff:8.8.8.8",
            "0.0.0.0",
        ] {
            assert!(!is_local(ip(public)), "{public}");
            assert!(!DiscoveryScope::Local.admits(ip(public)), "{public}");
            assert!(DiscoveryScope::Anyone.admits(ip(public)), "{public}");
        }
    }
}
