//! libp2p network behaviour for the Lumen light client.
//!
//! Defines the composite behaviour that combines:
//! - GossipSub for beacon chain topic subscription
//! - Identify for peer identification
//! - Ping for connection keep-alive
//!
//! The behaviour handles peer scoring to deprioritize peers that
//! send invalid data (as determined by lumen-core verification).

use libp2p::{
    gossipsub, identify, ping,
    swarm::NetworkBehaviour,
};
use serde::{Deserialize, Serialize};

/// The composite network behaviour for Lumen.
///
/// This bundles all the libp2p protocols we need into one behaviour
/// that the Swarm drives.
#[derive(NetworkBehaviour)]
pub struct LumenBehaviour {
    /// GossipSub for subscribing to beacon chain gossip topics.
    /// This is how we receive new light client updates from the network.
    pub gossipsub: gossipsub::Behaviour,

    /// Identify protocol for exchanging peer information.
    /// Lets us discover what protocols peers support.
    pub identify: identify::Behaviour,

    /// Ping for keeping connections alive and measuring latency.
    pub ping: ping::Behaviour,
}

/// Peer scoring — track which peers give us valid vs invalid data.
/// Peers that consistently send invalid updates get lower priority.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerScore {
    /// Total updates received from this peer.
    pub updates_received: u64,
    /// Updates that passed verification.
    pub updates_valid: u64,
    /// Updates that failed verification.
    pub updates_invalid: u64,
    /// Average latency in milliseconds.
    pub avg_latency_ms: f64,
}

impl PeerScore {
    pub fn new() -> Self {
        Self {
            updates_received: 0,
            updates_valid: 0,
            updates_invalid: 0,
            avg_latency_ms: 0.0,
        }
    }

    /// Calculate a reputation score (0.0 - 1.0).
    /// Higher is better. Peers with many invalid updates get scored down.
    pub fn reputation(&self) -> f64 {
        if self.updates_received == 0 {
            return 0.5; // Neutral for new peers
        }
        self.updates_valid as f64 / self.updates_received as f64
    }

    /// Whether this peer should be disconnected due to bad behavior.
    pub fn should_disconnect(&self) -> bool {
        // Disconnect if more than 50% of updates are invalid and we have enough data
        self.updates_received >= 10 && self.reputation() < 0.5
    }
}

/// Create a GossipSub configuration tuned for Ethereum beacon chain topics.
pub fn create_gossipsub_config() -> gossipsub::Config {
    gossipsub::ConfigBuilder::default()
        .heartbeat_interval(std::time::Duration::from_secs(1))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .max_transmit_size(10 * 1024 * 1024) // 10MB — beacon blocks can be large
        .build()
        .expect("Valid gossipsub config")
}

/// Create identify configuration for Lumen.
pub fn create_identify_config(local_public_key: libp2p::identity::PublicKey) -> identify::Config {
    identify::Config::new(
        "/lumen/0.1.0".to_string(),
        local_public_key,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_score_reputation() {
        let mut score = PeerScore::new();
        assert_eq!(score.reputation(), 0.5); // Neutral

        score.updates_received = 10;
        score.updates_valid = 10;
        assert_eq!(score.reputation(), 1.0);

        score.updates_invalid = 5;
        score.updates_received = 15;
        assert!((score.reputation() - 0.667).abs() < 0.01);
    }

    #[test]
    fn test_peer_disconnect_threshold() {
        let mut score = PeerScore::new();
        score.updates_received = 10;
        score.updates_valid = 4;
        score.updates_invalid = 6;

        assert!(score.should_disconnect()); // 40% valid < 50% threshold
    }
}
