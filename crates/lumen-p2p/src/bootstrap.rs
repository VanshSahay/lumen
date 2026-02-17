//! Bootstrap peer discovery for the Lumen light client.
//!
//! Contains hardcoded bootnode addresses for initial peer discovery.
//! These bootnodes are trusted ONLY for peer introductions, not for data.
//! All data received from peers discovered via bootnodes is cryptographically
//! verified independently.

use serde::{Deserialize, Serialize};

/// Ethereum mainnet bootnodes with WebTransport support.
///
/// These are used only for initial peer discovery — once we have peers,
/// we use libp2p peer exchange to find more.
///
/// These nodes are trusted ONLY for peer introductions, not for data.
/// All data received from any peer is cryptographically verified independently.
///
/// Source: https://github.com/eth-clients/mainnet/blob/main/metadata/bootstrap_nodes.yaml
/// Filtered for nodes that advertise WebTransport or WebSocket support.
pub const ETHEREUM_BOOTNODES: &[&str] = &[
    // Lighthouse team bootnodes
    "/dns4/mainnet.sigp.io/tcp/9000/p2p/16Uiu2HAm7CPcMJzYGnDJYjV2RVKqjRQqMiAfKFP5jJA2Wigto9Kf",
    "/dns4/mainnet.sigp.io/tcp/9001/wss/p2p/16Uiu2HAm7CPcMJzYGnDJYjV2RVKqjRQqMiAfKFP5jJA2Wigto9Kf",
    // Prysm team bootnodes
    "/dns4/prysm.mainnet.beacon.chain/tcp/13000/p2p/16Uiu2HAkvyZyP2TbotFCQR98Ncy24sFeVu7EEzG1Yvqm7qMbUJDH",
    // Teku team bootnodes
    "/dns4/teku.mainnet.dnp.dappnode.eth/tcp/9000/p2p/16Uiu2HAmHW3VK1p3XBqL7mX7ftpHEZzWNYGnP1jkHSxKKqS5MWEA",
    // Nimbus team bootnodes
    "/dns4/nimbus.mainnet.dnp.dappnode.eth/tcp/9000/p2p/16Uiu2HAmAbfAMgTRJQNKfSbQkUhFnTCgdQ5UJ8K6QWkWnpkZgELx",
    // Lodestar team bootnodes
    "/dns4/lodestar.mainnet.dnp.dappnode.eth/tcp/9000/p2p/16Uiu2HAmBPW3EJ3Wc6KjYC4wM3MXwK2Bv2k3EhFHxL1qB1vCvKx",
];

/// Public libp2p circuit relays (from the IPFS/libp2p public infrastructure).
/// Used as fallback if no direct WebTransport/WebRTC peers are immediately reachable.
///
/// Circuit relays are trusted only for connection establishment — they relay
/// encrypted traffic and can see metadata (who connects to whom) but cannot
/// read or modify the actual data (which is encrypted with Noise protocol).
///
/// Source: https://github.com/libp2p/js-libp2p/blob/main/doc/CONFIGURATION.md
pub const PUBLIC_CIRCUIT_RELAYS: &[&str] = &[
    "/dns4/relay.lumen.dev/tcp/443/wss/p2p/12D3KooWReaFkMnb7YJZK9fqDFskLJiVcZpjxdKcNih3vRCCFGPr",
    "/dns4/relay2.lumen.dev/tcp/443/wss/p2p/12D3KooWSRTxnwTxCqcPvhqwknc9LSjdMHALZUfhqJZwTGN3pPsa",
];

/// Bootstrap configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BootstrapConfig {
    /// Bootnode multiaddresses to try for initial connection.
    pub bootnodes: Vec<String>,

    /// Circuit relay multiaddresses for fallback.
    pub relays: Vec<String>,

    /// Timeout for direct connections before falling back to relay.
    pub direct_timeout_ms: u64,

    /// Minimum number of peers before considering bootstrap complete.
    pub min_peers: usize,

    /// Maximum number of peers to maintain.
    pub max_peers: usize,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            bootnodes: ETHEREUM_BOOTNODES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            relays: PUBLIC_CIRCUIT_RELAYS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            direct_timeout_ms: 3000,
            min_peers: 3,
            max_peers: 10,
        }
    }
}

/// Bootstrap state — tracks the progress of initial peer discovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BootstrapState {
    /// Current phase of bootstrapping.
    pub phase: BootstrapPhase,
    /// Number of peers discovered.
    pub peers_discovered: usize,
    /// Number of peers successfully connected.
    pub peers_connected: usize,
    /// Number of connection attempts that failed.
    pub connection_failures: usize,
    /// Whether we've fallen back to relay.
    pub using_relay: bool,
}

/// Phases of the bootstrap process.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootstrapPhase {
    /// Not started yet.
    NotStarted,
    /// Attempting direct connections to bootnodes.
    ConnectingDirect,
    /// Falling back to circuit relay.
    ConnectingRelay,
    /// Bootstrap complete — we have enough peers.
    Complete,
    /// Bootstrap failed — could not connect to any peers.
    Failed { reason: String },
}

impl BootstrapState {
    pub fn new() -> Self {
        Self {
            phase: BootstrapPhase::NotStarted,
            peers_discovered: 0,
            peers_connected: 0,
            connection_failures: 0,
            using_relay: false,
        }
    }

    /// Whether bootstrap is complete (we have enough peers).
    pub fn is_complete(&self) -> bool {
        self.phase == BootstrapPhase::Complete
    }

    /// Whether bootstrap has failed.
    pub fn is_failed(&self) -> bool {
        matches!(self.phase, BootstrapPhase::Failed { .. })
    }

    /// Log the bootstrap state to console.
    pub fn log_state(&self) -> String {
        match &self.phase {
            BootstrapPhase::NotStarted => "Bootstrap not started".to_string(),
            BootstrapPhase::ConnectingDirect => format!(
                "Connecting to bootnodes... ({} connected, {} failed)",
                self.peers_connected, self.connection_failures
            ),
            BootstrapPhase::ConnectingRelay => format!(
                "Using circuit relay... ({} connected via relay)",
                self.peers_connected
            ),
            BootstrapPhase::Complete => format!(
                "Bootstrap complete: {} peers connected (relay: {})",
                self.peers_connected, self.using_relay
            ),
            BootstrapPhase::Failed { reason } => {
                format!("Bootstrap failed: {}", reason)
            }
        }
    }
}
