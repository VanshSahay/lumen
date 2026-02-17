//! Beacon chain gossip topic subscription and message handling.
//!
//! Subscribes to Ethereum beacon chain gossip topics to receive
//! light client updates in real time.
//!
//! All received messages are raw SSZ bytes that must be passed to
//! lumen-core for cryptographic verification. This module does NOT
//! interpret or trust any data — it only handles transport.

use serde::{Deserialize, Serialize};

/// The gossip topic for light client finality updates.
/// This is the main feed of new verified chain heads.
///
/// Topic format: /eth2/{fork_digest}/light_client_finality_update/ssz_snappy
/// fork_digest for mainnet Deneb: b5303f2a
pub const LIGHT_CLIENT_FINALITY_UPDATE_TOPIC: &str =
    "/eth2/b5303f2a/light_client_finality_update/ssz_snappy";

/// Optimistic updates arrive faster (before finality) — useful for lower latency.
/// These are verified with the same sync committee signatures but represent
/// a less-certain view of the chain head.
pub const LIGHT_CLIENT_OPTIMISTIC_UPDATE_TOPIC: &str =
    "/eth2/b5303f2a/light_client_optimistic_update/ssz_snappy";

/// All beacon gossip topics that Lumen subscribes to.
pub const ALL_TOPICS: &[&str] = &[
    LIGHT_CLIENT_FINALITY_UPDATE_TOPIC,
    LIGHT_CLIENT_OPTIMISTIC_UPDATE_TOPIC,
];

/// A message received from a beacon chain gossip topic.
/// Contains raw SSZ bytes that need to be deserialized and verified.
#[derive(Clone, Debug)]
pub struct GossipMessage {
    /// The topic this message was received on.
    pub topic: String,
    /// The raw message bytes (SSZ + snappy compressed).
    pub data: Vec<u8>,
    /// The peer that propagated this message to us.
    pub source_peer: Option<String>,
    /// Message ID for deduplication.
    pub message_id: Vec<u8>,
}

/// The type of gossip message received.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GossipMessageType {
    /// A finality update — the chain head has been finalized.
    /// This is the strongest form of consensus and the most trusted.
    FinalityUpdate,
    /// An optimistic update — a new block has been attested but not finalized.
    /// Lower latency but slightly weaker guarantee.
    OptimisticUpdate,
    /// Unknown topic.
    Unknown(String),
}

impl GossipMessageType {
    /// Determine the message type from a topic string.
    pub fn from_topic(topic: &str) -> Self {
        if topic.contains("light_client_finality_update") {
            Self::FinalityUpdate
        } else if topic.contains("light_client_optimistic_update") {
            Self::OptimisticUpdate
        } else {
            Self::Unknown(topic.to_string())
        }
    }

    /// Whether this is a finality update (strongest guarantee).
    pub fn is_finality(&self) -> bool {
        matches!(self, Self::FinalityUpdate)
    }
}

/// Statistics about gossip message processing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GossipStats {
    /// Total messages received across all topics.
    pub messages_received: u64,
    /// Messages that passed verification.
    pub messages_valid: u64,
    /// Messages that failed verification (peer sent bad data).
    pub messages_invalid: u64,
    /// Messages that were duplicates (already processed).
    pub messages_duplicate: u64,
    /// Finality updates received.
    pub finality_updates: u64,
    /// Optimistic updates received.
    pub optimistic_updates: u64,
}

impl GossipStats {
    /// Log a summary of gossip statistics.
    pub fn summary(&self) -> String {
        format!(
            "Gossip: {} received ({} valid, {} invalid, {} duplicate) | {} finality, {} optimistic",
            self.messages_received,
            self.messages_valid,
            self.messages_invalid,
            self.messages_duplicate,
            self.finality_updates,
            self.optimistic_updates,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_from_topic() {
        assert_eq!(
            GossipMessageType::from_topic(LIGHT_CLIENT_FINALITY_UPDATE_TOPIC),
            GossipMessageType::FinalityUpdate
        );
        assert_eq!(
            GossipMessageType::from_topic(LIGHT_CLIENT_OPTIMISTIC_UPDATE_TOPIC),
            GossipMessageType::OptimisticUpdate
        );
        assert!(matches!(
            GossipMessageType::from_topic("/eth2/b5303f2a/unknown"),
            GossipMessageType::Unknown(_)
        ));
    }

    #[test]
    fn test_gossip_stats_summary() {
        let stats = GossipStats {
            messages_received: 100,
            messages_valid: 95,
            messages_invalid: 3,
            messages_duplicate: 2,
            finality_updates: 10,
            optimistic_updates: 85,
        };
        let summary = stats.summary();
        assert!(summary.contains("100 received"));
        assert!(summary.contains("95 valid"));
    }
}
