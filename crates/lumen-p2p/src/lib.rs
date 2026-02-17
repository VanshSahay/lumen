//! # Lumen P2P
//!
//! Peer-to-peer networking layer for the Lumen Ethereum light client.
//! Connects the browser to Ethereum's P2P network using browser-compatible
//! transports (WebRTC and WebTransport).
//!
//! ## Architecture
//!
//! - Runs in a **Web Worker** (never blocks the main thread)
//! - Uses libp2p for peer discovery, connection management, and gossip
//! - Subscribes to beacon chain gossip topics for light client updates
//! - All received data is passed to `lumen-core` for cryptographic verification
//!
//! ## Trust Model
//!
//! The P2P layer trusts NOTHING. Peers can:
//! - Send invalid data → rejected by lumen-core verification
//! - Refuse to send data → we connect to multiple peers
//! - Attempt DoS → peer scoring and connection limits
//!
//! Circuit relays are trusted ONLY for peer introductions (metadata),
//! never for actual data.

pub mod transport;
pub mod behaviour;
pub mod bootstrap;
pub mod relay;
pub mod beacon_gossip;

pub use bootstrap::*;
pub use behaviour::*;
pub use relay::*;
pub use beacon_gossip::*;
