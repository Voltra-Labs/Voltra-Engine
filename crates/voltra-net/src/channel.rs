//! Channels: what a message is promised on its way.
//!
//! One connection carries several, each with its own numbering, so a lost
//! packet holding up an ordered channel's chat line does not hold up the
//! position updates on an unreliable one — head-of-line blocking stays
//! inside a channel. renet, Unity's Netcode and Godot's ENet peers all draw
//! the line in the same place.

pub(crate) mod assembly;
pub(crate) mod receive;
pub(crate) mod send;

/// What a channel promises its messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelKind {
    /// Every message arrives, once, in the order it was sent. A lost one
    /// holds back those behind it until it is resent: chat, commands,
    /// anything that reads as a sequence.
    ReliableOrdered,
    /// Every message arrives, once, as soon as it is whole: events that
    /// stand alone — a pickup, a hit.
    ReliableUnordered,
    /// Messages may be lost, and one older than the newest already handed
    /// out is dropped rather than delivered late: state that the next
    /// message replaces — a position, an input.
    UnreliableSequenced,
}

impl ChannelKind {
    pub(crate) fn is_reliable(self) -> bool {
        !matches!(self, Self::UnreliableSequenced)
    }

    pub(crate) fn code(self) -> u8 {
        match self {
            Self::ReliableOrdered => 0,
            Self::ReliableUnordered => 1,
            Self::UnreliableSequenced => 2,
        }
    }
}

/// A channel, by its place in [`NetConfig::channels`](crate::NetConfig::channels).
///
/// The constants name the default layout; a game (or replication, stage
/// 114) that adds channels names them by index after these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChannelId(pub u8);

impl ChannelId {
    /// The default layout's [`ChannelKind::ReliableOrdered`] channel.
    pub const RELIABLE_ORDERED: Self = Self(0);
    /// The default layout's [`ChannelKind::ReliableUnordered`] channel.
    pub const RELIABLE_UNORDERED: Self = Self(1);
    /// The default layout's [`ChannelKind::UnreliableSequenced`] channel.
    pub const UNRELIABLE_SEQUENCED: Self = Self(2);
}

/// The default layout: one channel of each kind, in [`ChannelId`]'s order.
pub const DEFAULT_CHANNELS: [ChannelKind; 3] = [
    ChannelKind::ReliableOrdered,
    ChannelKind::ReliableUnordered,
    ChannelKind::UnreliableSequenced,
];

/// The most channels one connection may have: a frame's channel is a byte,
/// and every channel costs its window on both ends.
pub const MAX_CHANNELS: usize = 32;

/// The bytes of one fragment. Every fragment of a message but its last is
/// exactly this long; a message no longer than this is one fragment.
/// Leaves room in a datagram for the packet's header, the frame's, and a
/// few small frames beside it.
pub const FRAGMENT_SIZE: usize = 1024;

/// How many messages a reliable channel may have sent and not yet had
/// acknowledged. Both ends size their windows by it; a sender past it is
/// told [`SendError::ChannelFull`](crate::SendError::ChannelFull).
pub const RELIABLE_WINDOW: u16 = 512;

/// The bytes a message of `fragments` fragments is counted as, on both
/// ends — the sender's queue and the receiver's buffers agree on it, which
/// is what lets the receiver treat going over as a violation.
pub(crate) fn reserve(fragments: u16) -> usize {
    usize::from(fragments.max(1)) * FRAGMENT_SIZE
}

/// How many fragments a message of `len` bytes is sent as.
pub(crate) fn fragments_for(len: usize) -> usize {
    len.div_ceil(FRAGMENT_SIZE).max(1)
}
