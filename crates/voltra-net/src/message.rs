//! Messages: bytes a peer sent on a channel, and the typed values they
//! carry (`serde` over `postcard`).

use std::fmt;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{ChannelId, PeerId};

/// One whole message, reassembled and in its channel's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Who sent it.
    pub from: PeerId,
    /// The channel it came on.
    pub channel: ChannelId,
    /// What it holds; at most
    /// [`NetConfig::max_message_size`](crate::NetConfig::max_message_size).
    pub bytes: Vec<u8>,
}

impl Message {
    /// The value it holds, as [`encode`] wrote it.
    ///
    /// The bytes came off the wire: a peer may send anything, so a game
    /// handles the error (and may [`kick`](crate::Endpoint::kick) the
    /// sender) rather than assuming. `postcard` gives serde no size hint
    /// when a length claims more than the bytes left, so a lying length
    /// allocates nothing up front and fails at the end of the message.
    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, CodecError> {
        decode(&self.bytes)
    }
}

/// A value as bytes to send: `postcard`, compact and stable — varint
/// integers, no field names.
pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, CodecError> {
    postcard::to_allocvec(value).map_err(CodecError)
}

/// Bytes as the value [`encode`] made them from. Trailing bytes are an
/// error: a message is one value.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    let (value, rest) = postcard::take_from_bytes(bytes).map_err(CodecError)?;
    if rest.is_empty() {
        Ok(value)
    } else {
        Err(CodecError(postcard::Error::DeserializeBadEncoding))
    }
}

/// A value that would not encode, or bytes that do not decode as the type
/// asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecError(postcard::Error);

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CodecError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum Chat {
        Say { who: u32, text: String },
        Leave(u32),
    }

    #[test]
    fn a_value_round_trips() {
        let said = Chat::Say {
            who: 3,
            text: "hello".into(),
        };
        let bytes = encode(&said).expect("encodes");
        let message = Message {
            from: PeerId(3),
            channel: ChannelId::RELIABLE_ORDERED,
            bytes,
        };

        assert_eq!(message.decode::<Chat>().expect("decodes"), said);
    }

    #[test]
    fn junk_is_an_error_not_a_panic_or_a_huge_allocation() {
        // A variant past the end, a string length of four billion, trailing
        // bytes, nothing at all.
        for junk in [
            &[9u8][..],
            &[0, 1, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F][..],
            &[1, 4, 0][..],
            &[][..],
        ] {
            assert!(decode::<Chat>(junk).is_err(), "{junk:?}");
        }
        let mut rng = crate::rng::Rng::new(5);
        for _ in 0..10_000 {
            let len = (rng.next_u64() % 64) as usize;
            let bytes: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
            let _ = decode::<Chat>(&bytes);
            let _ = decode::<Vec<String>>(&bytes);
        }
    }
}
