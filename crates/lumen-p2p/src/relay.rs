//! Circuit relay client logic for Lumen.
//!
//! Circuit relay is used ONLY as a bootstrap mechanism:
//! 1. Try to connect directly to WebTransport bootnodes
//! 2. If none reachable within 3 seconds, fall back to circuit relay
//! 3. Once connected via relay, do peer exchange to find direct peers
//! 4. Upgrade to direct connections, drop relay dependency
//! 5. Log clearly to console which mode we're in
//!
//! ## Trust Model
//!
//! Circuit relays can see:
//! - Who is connecting to whom (metadata)
//! - When connections happen (timing)
//!
//! Circuit relays CANNOT:
//! - Read the data (encrypted with Noise protocol)
//! - Modify the data (integrity checked by Noise)
//! - Forge light client updates (requires BLS signatures from sync committee)

use serde::{Deserialize, Serialize};

/// The current connection mode — indicates trust level clearly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionMode {
    /// Connected directly via WebTransport — fully P2P, no intermediary.
    /// This is the best possible trust state.
    DirectWebTransport {
        /// Number of direct WebTransport peers.
        peer_count: usize,
    },

    /// Connected directly via WebRTC — fully P2P, no intermediary.
    DirectWebRTC {
        /// Number of direct WebRTC peers.
        peer_count: usize,
    },

    /// Connected via circuit relay — relay sees metadata, not data.
    /// This is acceptable for bootstrapping but should be upgraded ASAP.
    ViaRelay {
        /// PeerId of the relay being used (for diagnostics).
        relay_peer: String,
        /// Number of direct peers (should be increasing as we discover them).
        direct_peers: usize,
    },

    /// Bootstrapping — not yet connected to any peers.
    Bootstrapping,

    /// Disconnected — no peers available.
    Disconnected {
        /// Reason for disconnection.
        reason: String,
    },
}

impl ConnectionMode {
    /// Human-readable description for console logging.
    pub fn description(&self) -> String {
        match self {
            ConnectionMode::DirectWebTransport { peer_count } => format!(
                "Direct WebTransport: {} peers | Fully P2P, no intermediary | Trust: MAXIMUM",
                peer_count
            ),
            ConnectionMode::DirectWebRTC { peer_count } => format!(
                "Direct WebRTC: {} peers | Fully P2P, no intermediary | Trust: MAXIMUM",
                peer_count
            ),
            ConnectionMode::ViaRelay {
                relay_peer,
                direct_peers,
            } => format!(
                "Circuit Relay via {} | {} direct peers discovered | Trust: relay sees metadata only, not data",
                &relay_peer[..8.min(relay_peer.len())],
                direct_peers
            ),
            ConnectionMode::Bootstrapping => {
                "Bootstrapping — connecting to Ethereum P2P network...".to_string()
            }
            ConnectionMode::Disconnected { reason } => {
                format!("Disconnected: {}", reason)
            }
        }
    }

    /// Whether we have any active connections.
    pub fn is_connected(&self) -> bool {
        matches!(
            self,
            ConnectionMode::DirectWebTransport { .. }
                | ConnectionMode::DirectWebRTC { .. }
                | ConnectionMode::ViaRelay { .. }
        )
    }

    /// Whether we're using a relay (less ideal trust state).
    pub fn is_relayed(&self) -> bool {
        matches!(self, ConnectionMode::ViaRelay { .. })
    }

    /// Whether we have direct connections (ideal trust state).
    pub fn is_direct(&self) -> bool {
        matches!(
            self,
            ConnectionMode::DirectWebTransport { .. } | ConnectionMode::DirectWebRTC { .. }
        )
    }
}

/// Strategy for upgrading from relay to direct connections.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelayUpgradeStrategy {
    /// How often to attempt finding direct peers (milliseconds).
    pub discovery_interval_ms: u64,
    /// Maximum time to stay on relay before warning (milliseconds).
    pub max_relay_duration_ms: u64,
    /// Whether to aggressively seek direct connections.
    pub aggressive_discovery: bool,
}

impl Default for RelayUpgradeStrategy {
    fn default() -> Self {
        Self {
            discovery_interval_ms: 5_000,
            max_relay_duration_ms: 60_000,
            aggressive_discovery: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_mode_trust_state() {
        let direct = ConnectionMode::DirectWebTransport { peer_count: 5 };
        assert!(direct.is_connected());
        assert!(direct.is_direct());
        assert!(!direct.is_relayed());

        let relay = ConnectionMode::ViaRelay {
            relay_peer: "12D3KooW...".to_string(),
            direct_peers: 0,
        };
        assert!(relay.is_connected());
        assert!(!relay.is_direct());
        assert!(relay.is_relayed());

        let bootstrap = ConnectionMode::Bootstrapping;
        assert!(!bootstrap.is_connected());
    }
}
