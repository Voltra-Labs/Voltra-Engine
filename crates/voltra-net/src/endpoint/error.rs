//! Why a send was refused.

use crate::{ChannelId, CodecError, PeerId};

/// Why a message was not queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// No connection to that peer — never connected, or gone.
    NotConnected(PeerId),
    /// The configuration has no such channel.
    NoChannel(ChannelId),
    /// Longer than [`NetConfig::max_message_size`].
    TooLarge {
        /// The message's length.
        len: usize,
        /// The most allowed.
        max: usize,
    },
    /// The channel holds as much as it may for that peer — it has not
    /// acknowledged enough of what was sent. Try again next tick.
    ChannelFull(ChannelId),
    /// The value would not encode.
    Encode(CodecError),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConnected(peer) => write!(f, "{peer} is not connected"),
            Self::NoChannel(channel) => write!(f, "there is no channel {}", channel.0),
            Self::TooLarge { len, max } => {
                write!(f, "a message of {len} bytes is over the limit of {max}")
            }
            Self::ChannelFull(channel) => write!(f, "channel {} is full", channel.0),
            Self::Encode(e) => write!(f, "the message would not encode: {e}"),
        }
    }
}

impl std::error::Error for SendError {}
