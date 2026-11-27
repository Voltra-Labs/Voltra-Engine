//! One established connection: its channels, its acks, its clock.
//!
//! What both ends run once the handshake is done. The endpoint owns the
//! socket and the handshake; this owns everything about one peer that
//! survives from packet to packet.

use std::time::Duration;

use crate::acks::{Carried, ReceivedPackets, SentPackets};
use crate::channel::assembly::Piece;
use crate::channel::receive::ReceiveChannel;
use crate::channel::send::{Refused, SendChannel};
use crate::packet::{frames, Acks, PayloadWriter};
use crate::wire::Malformed;
use crate::{Address, ChannelId, NetConfig, SendError};

/// The round trip assumed before the first is measured: resends wait
/// this long times [`RESEND_FACTOR`].
const INITIAL_RTT: Duration = Duration::from_millis(100);

/// A fragment is resent once its packet has had this many round trips to
/// be acknowledged.
const RESEND_FACTOR: f64 = 1.5;

/// The resend delay never goes below or above these, whatever the RTT.
const MIN_RESEND: Duration = Duration::from_millis(20);
const MAX_RESEND: Duration = Duration::from_secs(1);

/// What a connection has done, for a profiler or a stats overlay.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ConnectionStats {
    /// The smoothed round-trip time; [`INITIAL_RTT`]'s 100 ms until the
    /// first is measured.
    pub rtt: Duration,
    /// How much the round trip varies: RFC 6298's `RTTVAR`, the smoothed
    /// distance of each sample from the smoothed round trip. Zero until
    /// the first is measured. `rtt + 4 × rtt_variation` is TCP's bound on
    /// a round trip this link plausibly takes.
    pub rtt_variation: Duration,
    /// Datagrams sent.
    pub packets_sent: u64,
    /// Datagrams received (duplicates included).
    pub packets_received: u64,
    /// Bytes sent, packet headers included.
    pub bytes_sent: u64,
    /// Bytes received, packet headers included.
    pub bytes_received: u64,
    /// Fragments sent again because no ack came in time.
    pub resent: u64,
}

pub(crate) struct Connection {
    pub(crate) address: Address,
    /// What the client said its handshake was: a new handshake from the
    /// same address with another nonce is a new client.
    pub(crate) nonce: u64,
    pub(crate) session: u64,
    send: Vec<SendChannel>,
    receive: Vec<ReceiveChannel>,
    reliable: Vec<bool>,
    sent: SentPackets,
    received: ReceivedPackets,
    next_sequence: u16,
    last_received: Duration,
    last_sent: Option<Duration>,
    ack_owed: bool,
    /// Smoothed RTT and its variation in seconds; `None` until measured.
    rtt: Option<(f64, f64)>,
    stats: ConnectionStats,
}

impl Connection {
    pub(crate) fn new(
        address: Address,
        nonce: u64,
        session: u64,
        config: &NetConfig,
        now: Duration,
    ) -> Self {
        let max_fragments = config.max_fragments();
        Self {
            address,
            nonce,
            session,
            send: config
                .channels
                .iter()
                .map(|&kind| SendChannel::new(kind, config.channel_budget))
                .collect(),
            receive: config
                .channels
                .iter()
                .map(|&kind| ReceiveChannel::new(kind, max_fragments, config.channel_budget))
                .collect(),
            reliable: config.channels.iter().map(|k| k.is_reliable()).collect(),
            sent: SentPackets::new(),
            received: ReceivedPackets::default(),
            // From 1: an ack of 0 with no bits is what a peer that has
            // received nothing sends, and must not acknowledge anything.
            next_sequence: 1,
            last_received: now,
            last_sent: None,
            ack_owed: false,
            rtt: None,
            stats: ConnectionStats {
                rtt: INITIAL_RTT,
                ..Default::default()
            },
        }
    }

    pub(crate) fn stats(&self) -> ConnectionStats {
        self.stats
    }

    /// Queues `bytes`, already checked against the largest message.
    pub(crate) fn queue(&mut self, channel: ChannelId, bytes: &[u8]) -> Result<(), SendError> {
        let send = self
            .send
            .get_mut(usize::from(channel.0))
            .ok_or(SendError::NoChannel(channel))?;
        send.push(bytes)
            .map_err(|Refused::Full| SendError::ChannelFull(channel))
    }

    pub(crate) fn timed_out(&self, now: Duration, timeout: Duration) -> bool {
        now.saturating_sub(self.last_received) > timeout
    }

    /// Reads one payload packet from the peer: its acks, then its frames,
    /// each whole message handed to `deliver`. A frame that does not parse,
    /// or that breaks a channel's bounds, is a [`Malformed`] the endpoint
    /// ends the connection for.
    pub(crate) fn receive(
        &mut self,
        now: Duration,
        sequence: u16,
        acks: Acks,
        body: &[u8],
        size: usize,
        deliver: &mut impl FnMut(ChannelId, Vec<u8>),
    ) -> Result<(), Malformed> {
        self.last_received = now;
        self.stats.packets_received += 1;
        self.stats.bytes_received += size as u64;
        if !self.received.note(sequence) {
            return Ok(());
        }
        self.ack_owed = true;
        let send = &mut self.send;
        if let Some(at) = self.sent.apply(acks, |carried| {
            if let Some(channel) = send.get_mut(usize::from(carried.channel)) {
                channel.ack(carried.message, carried.index);
            }
        }) {
            let sample = now.saturating_sub(at).as_secs_f64();
            let (rtt, variation) = match self.rtt {
                None => (sample, sample / 2.0),
                // RFC 6298: the variation a quarter of the way toward each
                // sample's distance (beta), from the RTT before it moves; the
                // RTT an eighth of the way toward the sample (alpha).
                Some((rtt, variation)) => (
                    rtt + (sample - rtt) / 8.0,
                    variation + ((sample - rtt).abs() - variation) / 4.0,
                ),
            };
            self.rtt = Some((rtt, variation));
            self.stats.rtt = Duration::from_secs_f64(rtt.max(0.0));
            self.stats.rtt_variation = Duration::from_secs_f64(variation.max(0.0));
        }
        for frame in frames(body) {
            let frame = frame?;
            let channel = self
                .receive
                .get_mut(usize::from(frame.channel))
                .ok_or(Malformed)?;
            let id = ChannelId(frame.channel);
            channel.receive(&frame.piece, &mut |message| deliver(id, message))?;
        }
        Ok(())
    }

    /// The packets to send this tick: every fragment due, as few packets as
    /// hold them, at most `max_packets_per_flush`; or one empty packet if
    /// an ack is owed or the line has been quiet for `keepalive`.
    pub(crate) fn flush(&mut self, now: Duration, config: &NetConfig) -> Vec<Vec<u8>> {
        let resend_after = Duration::from_secs_f64(
            self.rtt.map_or(INITIAL_RTT.as_secs_f64(), |(rtt, _)| rtt) * RESEND_FACTOR,
        )
        .clamp(MIN_RESEND, MAX_RESEND);
        let acks = self.received.acks().unwrap_or_default();
        let mut packets = Vec::new();
        let mut open: Option<(PayloadWriter, Vec<Carried>, u16)> = None;
        let Self {
            send,
            sent,
            next_sequence,
            reliable,
            session,
            stats,
            ..
        } = self;
        let mut close = |open: &mut Option<(PayloadWriter, Vec<Carried>, u16)>,
                         packets: &mut Vec<Vec<u8>>| {
            if let Some((writer, carried, sequence)) = open.take() {
                sent.record(sequence, now, carried);
                packets.push(writer.finish());
            }
        };
        for (index, channel) in send.iter_mut().enumerate() {
            let reliable = reliable[index];
            stats.resent += channel.drain_due(now, resend_after, |due| {
                let bytes = due.bytes();
                if open.as_ref().is_some_and(|(w, _, _)| !w.fits(bytes.len())) {
                    close(&mut open, &mut packets);
                }
                if open.is_none() {
                    if packets.len() >= config.max_packets_per_flush {
                        return false;
                    }
                    let sequence = *next_sequence;
                    *next_sequence = next_sequence.wrapping_add(1);
                    open = Some((
                        PayloadWriter::new(*session, sequence, acks),
                        Vec::new(),
                        sequence,
                    ));
                }
                let Some((writer, carried, _)) = open.as_mut() else {
                    return false;
                };
                let piece = Piece {
                    message: due.message,
                    index: due.index,
                    count: due.count,
                    bytes,
                };
                writer.frame(index as u8, &piece);
                if reliable {
                    carried.push(Carried {
                        channel: index as u8,
                        message: due.message,
                        index: due.index,
                    });
                }
                true
            });
        }
        close(&mut open, &mut packets);
        let quiet = self
            .last_sent
            .is_none_or(|at| now.saturating_sub(at) >= config.keepalive);
        if packets.is_empty() && (self.ack_owed || quiet) {
            let sequence = self.next_sequence;
            self.next_sequence = self.next_sequence.wrapping_add(1);
            self.sent.record(sequence, now, Vec::new());
            packets.push(PayloadWriter::new(self.session, sequence, acks).finish());
        }
        if !packets.is_empty() {
            self.ack_owed = false;
            self.last_sent = Some(now);
            self.stats.packets_sent += packets.len() as u64;
            self.stats.bytes_sent += packets.iter().map(|p| p.len() as u64).sum::<u64>();
        }
        packets
    }
}
