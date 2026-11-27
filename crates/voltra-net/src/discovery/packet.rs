//! The probe and the answer, byte by byte. Hand-written, every read
//! bounds-checked, like the session's packets (`wire.rs`).

use super::{Advert, MAX_GAME_ID, MAX_SERVER_NAME, PROBE_SIZE};
use crate::wire::{Malformed, Reader, Writer};
use crate::PROTOCOL_VERSION;

/// What every discovery datagram starts with: not a session packet, not
/// another program's.
const MAGIC: [u8; 4] = *b"VLDS";

/// The version of this layout. Another is dropped unread, both ways.
const VERSION: u8 = 1;

const PROBE: u8 = 1;
const ANSWER: u8 = 2;

/// The bytes before an answer's two strings: magic, version, kind, nonce,
/// protocol version, fingerprint, players, slots, two ports, flags.
const ANSWER_FIXED: usize = 4 + 1 + 1 + 8 + 2 + 8 + 2 + 2 + 2 + 2 + 1;

/// The largest answer: the fixed part, then the game id and the name,
/// each a length byte and at most its cap.
pub(super) const MAX_ANSWER: usize = ANSWER_FIXED + 1 + MAX_GAME_ID + 1 + MAX_SERVER_NAME;

// No amplification: an answer is never larger than the probe it answers.
const _: () = assert!(MAX_ANSWER <= PROBE_SIZE);

const IN_MATCH: u8 = 1;

/// A client asking who is there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Probe {
    /// Echoed in the answer, so a client takes only answers to its own.
    pub(super) nonce: u64,
    /// Only servers of this game answer; empty for any.
    pub(super) game: String,
}

impl Probe {
    pub(super) fn encode(&self) -> Vec<u8> {
        let mut w = Writer::with_capacity(PROBE_SIZE);
        w.bytes(&MAGIC);
        w.u8(VERSION);
        w.u8(PROBE);
        w.u64(self.nonce);
        short_string(&mut w, &self.game, MAX_GAME_ID);
        w.pad_to(PROBE_SIZE);
        w.finish()
    }

    /// A probe, if `bytes` is one: exactly [`PROBE_SIZE`] long, ours.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, Malformed> {
        if bytes.len() != PROBE_SIZE {
            return Err(Malformed);
        }
        let mut r = Reader::new(bytes);
        header(&mut r, PROBE)?;
        let nonce = r.u64()?;
        let game = read_string(&mut r, MAX_GAME_ID)?;
        Ok(Self { nonce, game })
    }
}

/// A server's answer to a probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Answer {
    pub(super) nonce: u64,
    pub(super) protocol_version: u16,
    pub(super) advert: Advert,
}

impl Answer {
    pub(super) fn new(nonce: u64, advert: &Advert) -> Self {
        Self {
            nonce,
            protocol_version: PROTOCOL_VERSION,
            advert: advert.clone(),
        }
    }

    pub(super) fn encode(&self) -> Vec<u8> {
        let advert = &self.advert;
        let mut w = Writer::with_capacity(MAX_ANSWER);
        w.bytes(&MAGIC);
        w.u8(VERSION);
        w.u8(ANSWER);
        w.u64(self.nonce);
        w.u16(self.protocol_version);
        w.u64(advert.fingerprint);
        w.u16(clamp_u16(advert.players));
        w.u16(clamp_u16(advert.max_players));
        w.u16(advert.udp_port.unwrap_or(0));
        w.u16(advert.websocket_port.unwrap_or(0));
        w.u8(if advert.in_match { IN_MATCH } else { 0 });
        short_string(&mut w, &advert.game, MAX_GAME_ID);
        short_string(&mut w, &advert.name, MAX_SERVER_NAME);
        w.finish()
    }

    /// An answer, if `bytes` is one: ours, no longer than
    /// [`MAX_ANSWER`], nothing after its last field.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, Malformed> {
        if bytes.len() > MAX_ANSWER {
            return Err(Malformed);
        }
        let mut r = Reader::new(bytes);
        header(&mut r, ANSWER)?;
        let nonce = r.u64()?;
        let protocol_version = r.u16()?;
        let fingerprint = r.u64()?;
        let players = usize::from(r.u16()?);
        let max_players = usize::from(r.u16()?);
        let port = |p: u16| (p != 0).then_some(p);
        let udp_port = port(r.u16()?);
        let websocket_port = port(r.u16()?);
        let flags = r.u8()?;
        let game = read_string(&mut r, MAX_GAME_ID)?;
        let name = read_string(&mut r, MAX_SERVER_NAME)?;
        if !r.is_empty() {
            return Err(Malformed);
        }
        Ok(Self {
            nonce,
            protocol_version,
            advert: Advert {
                name,
                game,
                players,
                max_players,
                in_match: flags & IN_MATCH != 0,
                udp_port,
                websocket_port,
                fingerprint,
            },
        })
    }
}

fn header(r: &mut Reader<'_>, kind: u8) -> Result<(), Malformed> {
    if r.bytes(MAGIC.len())? != MAGIC || r.u8()? != VERSION || r.u8()? != kind {
        return Err(Malformed);
    }
    Ok(())
}

fn clamp_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// `text`'s first `cap` bytes, cut on a character boundary.
pub(super) fn cut(text: &str, cap: usize) -> &str {
    let mut end = text.len().min(cap);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// A length byte and at most `cap` bytes of `text`.
fn short_string(w: &mut Writer, text: &str, cap: usize) {
    let text = cut(text, cap.min(usize::from(u8::MAX)));
    w.u8(text.len() as u8);
    w.bytes(text.as_bytes());
}

fn read_string(r: &mut Reader<'_>, cap: usize) -> Result<String, Malformed> {
    let len = usize::from(r.u8()?);
    if len > cap {
        return Err(Malformed);
    }
    let bytes = r.bytes(len)?;
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advert() -> Advert {
        Advert {
            name: "Ada's game".into(),
            game: "arena".into(),
            players: 2,
            max_players: 4,
            in_match: true,
            udp_port: Some(7777),
            websocket_port: None,
            fingerprint: 0xDEAD_BEEF_F00D,
        }
    }

    #[test]
    fn a_probe_and_an_answer_round_trip() {
        let probe = Probe {
            nonce: 42,
            game: "arena".into(),
        };
        let bytes = probe.encode();
        assert_eq!(bytes.len(), PROBE_SIZE);
        assert_eq!(Probe::decode(&bytes), Ok(probe));

        let answer = Answer::new(42, &advert());
        assert_eq!(Answer::decode(&answer.encode()), Ok(answer));
    }

    #[test]
    fn no_answer_is_larger_than_a_probe() {
        let longest = Advert {
            name: "n".repeat(500),
            game: "g".repeat(500),
            players: usize::MAX,
            max_players: usize::MAX,
            ..advert()
        };
        let bytes = Answer::new(1, &longest).encode();
        assert_eq!(bytes.len(), MAX_ANSWER);
        assert!(bytes.len() <= PROBE_SIZE);
        let back = Answer::decode(&bytes).expect("decodes");
        assert_eq!(back.advert.name.len(), MAX_SERVER_NAME);
        assert_eq!(back.advert.players, usize::from(u16::MAX));
    }

    #[test]
    fn a_name_is_cut_on_a_character_boundary() {
        let wide = Advert {
            name: "é".repeat(40),
            ..advert()
        };
        let back = Answer::decode(&Answer::new(1, &wide).encode()).expect("decodes");
        assert_eq!(back.advert.name, "é".repeat(32));
    }

    #[test]
    fn a_probe_of_another_size_or_kind_is_dropped() {
        let bytes = Probe {
            nonce: 1,
            game: String::new(),
        }
        .encode();
        assert!(Probe::decode(&bytes[..PROBE_SIZE - 1]).is_err(), "short");
        let mut long = bytes.clone();
        long.push(0);
        assert!(Probe::decode(&long).is_err(), "long");
        let mut magic = bytes.clone();
        magic[0] ^= 1;
        assert!(Probe::decode(&magic).is_err(), "not ours");
        let mut version = bytes.clone();
        version[4] = VERSION + 1;
        assert!(Probe::decode(&version).is_err(), "another version");
        let answer = Answer::new(1, &advert()).encode();
        assert!(Probe::decode(&answer).is_err(), "an answer is no probe");
        assert!(Answer::decode(&bytes).is_err(), "a probe is no answer");
    }

    #[test]
    fn random_bytes_never_panic() {
        let mut state = 0x9E37_79B9_u64;
        for len in 0..5000 {
            let mut bytes: Vec<u8> = (0..len % (PROBE_SIZE + 8))
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                })
                .collect();
            // Half of them past the header, to reach the fields.
            if len % 2 == 0 && bytes.len() >= 6 {
                bytes[..4].copy_from_slice(&MAGIC);
                bytes[4] = VERSION;
                bytes[5] = if len % 4 == 0 { PROBE } else { ANSWER };
            }
            let _ = Probe::decode(&bytes);
            let _ = Answer::decode(&bytes);
        }
    }
}
