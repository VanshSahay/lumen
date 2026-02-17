//! Transport configuration for browser-based P2P.
//!
//! In the browser, we cannot use TCP or QUIC directly.
//! We use WebRTC and WebTransport, which are browser-native.
//!
//! ## Transport Priority
//!
//! 1. **WebTransport** (preferred) — lower latency, better performance
//! 2. **WebRTC** (fallback) — wider peer support, works behind more NATs
//! 3. **Circuit relay** (bootstrap only) — for initial peer discovery

use serde::{Deserialize, Serialize};

/// Transport type used for a connection.
/// Logged clearly so developers can audit their trust state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportType {
    /// WebTransport — direct, encrypted, fully P2P.
    /// No intermediary. Best performance and trust model.
    WebTransport,

    /// WebRTC — direct, encrypted, fully P2P.
    /// Slightly higher latency than WebTransport but wider compatibility.
    WebRTC,

    /// WebSocket — direct connection.
    /// Used as a fallback when WebRTC/WebTransport are not available.
    WebSocket,

    /// Circuit relay — indirect, via relay node.
    /// The relay can see metadata (who's talking to whom) but NOT the data
    /// (which is encrypted with Noise). Used only for bootstrapping.
    CircuitRelay,
}

impl TransportType {
    /// Whether this transport is direct (no intermediary).
    pub fn is_direct(&self) -> bool {
        matches!(
            self,
            TransportType::WebTransport | TransportType::WebRTC | TransportType::WebSocket
        )
    }

    /// Human-readable description for logging.
    pub fn description(&self) -> &'static str {
        match self {
            TransportType::WebTransport => "WebTransport (direct, encrypted, fully P2P)",
            TransportType::WebRTC => "WebRTC (direct, encrypted, fully P2P)",
            TransportType::WebSocket => "WebSocket (direct, encrypted)",
            TransportType::CircuitRelay => {
                "Circuit Relay (indirect — relay sees metadata, not data)"
            }
        }
    }
}

/// Configuration for the P2P transport layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Enable WebTransport (preferred).
    pub enable_webtransport: bool,

    /// Enable WebRTC (fallback).
    pub enable_webrtc: bool,

    /// Enable WebSocket (last resort for direct connections).
    pub enable_websocket: bool,

    /// Enable circuit relay for bootstrapping.
    pub enable_relay: bool,

    /// Maximum number of concurrent peer connections.
    pub max_peers: usize,

    /// Connection timeout in milliseconds.
    pub connection_timeout_ms: u64,

    /// Timeout for initial bootstrap in milliseconds.
    /// If no direct connection is established within this time,
    /// fall back to circuit relay.
    pub bootstrap_timeout_ms: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            enable_webtransport: true,
            enable_webrtc: true,
            enable_websocket: true,
            enable_relay: true,
            max_peers: 10,
            connection_timeout_ms: 10_000,
            bootstrap_timeout_ms: 3_000,
        }
    }
}

/// Statistics about the current transport state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransportStats {
    /// Number of active connections by transport type.
    pub connections: Vec<(TransportType, usize)>,
    /// Total number of active peer connections.
    pub total_peers: usize,
    /// Whether we're currently using any relay connections.
    pub using_relay: bool,
    /// Whether we have at least one direct connection.
    pub has_direct_connection: bool,
}

impl TransportStats {
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
            total_peers: 0,
            using_relay: false,
            has_direct_connection: false,
        }
    }
}
