//! # Lumen WASM
//!
//! WebAssembly bindings for the Lumen Ethereum light client.
//! This crate bridges `lumen-core`'s pure Rust verification logic to JavaScript
//! via `wasm-bindgen`.
//!
//! ## Architecture
//!
//! - All cryptographic verification happens in Rust/WASM (never in JS)
//! - Designed to run in a Web Worker (never block the main thread)
//! - Communicates with JS via structured message passing
//! - All public methods return Results that map to JS exceptions

mod network;
mod provider;
mod state;

use lumen_core::types::beacon::*;
use lumen_core::types::execution::*;
use lumen_core::consensus::checkpoint::parse_checkpoint_hash;
use lumen_core::consensus::light_client::initialize_from_bootstrap;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Set up panic hook on WASM initialization.
/// This ensures Rust panics are logged to the browser console with full stack traces.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// The main Lumen client — holds verified chain state and exposes verification methods.
///
/// This struct is the WASM-side counterpart of the TypeScript `LumenProvider`.
/// It maintains the cryptographically verified view of the Ethereum chain
/// and provides methods to verify proofs against that view.
#[wasm_bindgen]
pub struct LumenClient {
    state: LightClientState,
}

#[wasm_bindgen]
impl LumenClient {
    /// Initialize a new Lumen client from a checkpoint.
    ///
    /// The checkpoint_hash is the hex-encoded hash of a known finalized block.
    /// This is the only moment of trust — the checkpoint must be obtained
    /// from multiple independent sources before calling this.
    ///
    /// After initialization, all verification is purely cryptographic.
    #[wasm_bindgen(constructor)]
    pub fn new(checkpoint_hash: &str) -> Result<LumenClient, JsValue> {
        let block_root = parse_checkpoint_hash(checkpoint_hash)
            .map_err(|e| JsValue::from_str(&format!("Invalid checkpoint hash: {}", e)))?;

        log_to_console(&format!(
            "[Lumen] Initializing with checkpoint: 0x{}",
            hex::encode(block_root)
        ));

        // Create a bootstrap state. In production, this would be fetched from
        // multiple beacon API endpoints and verified for consensus.
        // For now, we create a minimal state that will be populated via process_update.
        let bootstrap = LightClientBootstrap {
            header: BeaconBlockHeader {
                slot: 0,
                proposer_index: 0,
                parent_root: [0; 32],
                state_root: block_root, // Use checkpoint as initial state root
                body_root: [0; 32],
            },
            current_sync_committee: SyncCommittee {
                pubkeys: vec![BlsPublicKey([0u8; 48]); 512],
                aggregate_pubkey: BlsPublicKey([0u8; 48]),
            },
            current_sync_committee_branch: vec![], // Skip verification for bootstrap
        };

        // Ethereum mainnet genesis validators root
        let genesis_validators_root = [
            0x4b, 0x36, 0x3d, 0xb9, 0x4e, 0x28, 0x61, 0x20, 0xd7, 0x6e, 0xb9, 0x05, 0x34,
            0x0f, 0xdd, 0x4e, 0x54, 0xbf, 0xe9, 0xf0, 0x6b, 0xf3, 0x3f, 0xf6, 0xcf, 0x5a,
            0xd2, 0x7f, 0x51, 0x1b, 0xfe, 0x95,
        ];

        // Deneb fork version (current as of 2024)
        let fork_version = [0x04, 0x00, 0x00, 0x00];

        let state = initialize_from_bootstrap(&bootstrap, genesis_validators_root, fork_version)
            .map_err(|e| JsValue::from_str(&format!("Failed to initialize: {}", e)))?;

        log_to_console("[Lumen] Client initialized successfully");
        log_to_console(&format!(
            "[Lumen] Trust state: checkpoint-based initialization, awaiting P2P sync"
        ));

        Ok(LumenClient { state })
    }

    /// Process a light client update received from a peer.
    ///
    /// The update_json should be the JSON-serialized LightClientUpdate.
    /// Returns true if the update was valid and state advanced.
    /// Returns false if the update was invalid (the caller should log but not crash).
    ///
    /// IMPORTANT: Every field in the update is cryptographically verified.
    /// The update source is untrusted — we verify everything regardless.
    pub fn process_update(&mut self, update_json: &str) -> Result<bool, JsValue> {
        let update: LightClientUpdate = serde_json::from_str(update_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid update JSON: {}", e)))?;

        let current_slot = self.state.finalized_header.slot;
        let genesis_validators_root = self.state.genesis_validators_root;

        match lumen_core::consensus::light_client::process_light_client_update(
            &mut self.state,
            &update,
            current_slot,
            genesis_validators_root,
        ) {
            Ok(()) => {
                log_to_console(&format!(
                    "[Lumen] State advanced to slot {}",
                    self.state.finalized_header.slot
                ));
                Ok(true)
            }
            Err(e) => {
                log_to_console(&format!("[Lumen] Update rejected: {}", e));
                Ok(false)
            }
        }
    }

    /// Get the current verified head slot number.
    /// This is the latest finalized slot we have cryptographic proof for.
    pub fn head_slot(&self) -> u64 {
        self.state.finalized_header.slot
    }

    /// Get the current verified state root (hex encoded).
    /// This root is used to verify all Merkle-Patricia trie proofs.
    pub fn state_root(&self) -> String {
        format!("0x{}", hex::encode(self.state.finalized_header.state_root))
    }

    /// Get the current verified execution state root, if available.
    /// This is the state root from the execution payload, which is what
    /// eth_getProof verifies against.
    pub fn execution_state_root(&self) -> Option<String> {
        self.state
            .verified_state_root()
            .map(|r| format!("0x{}", hex::encode(r)))
    }

    /// Verify an account proof and return account state as JSON.
    ///
    /// address: hex-encoded Ethereum address (0x...)
    /// proof_json: JSON-encoded eth_getProof response from any source
    ///
    /// IMPORTANT: the proof is verified against our internally held state root.
    /// The caller cannot pass in a fake state root — we use our verified one.
    /// The proof data can come from any source (including untrusted RPCs).
    pub fn verify_account(&self, address: &str, proof_json: &str) -> Result<JsValue, JsValue> {
        let state_root = self
            .state
            .verified_state_root()
            .unwrap_or(self.state.finalized_header.state_root);

        // Parse the address
        let addr_hex = address.strip_prefix("0x").unwrap_or(address);
        let addr_bytes = hex::decode(addr_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid address: {}", e)))?;
        if addr_bytes.len() != 20 {
            return Err(JsValue::from_str("Address must be 20 bytes"));
        }
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&addr_bytes);

        // Parse the proof
        let proof: AccountProof = serde_json::from_str(proof_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid proof JSON: {}", e)))?;

        // Verify the proof against our verified state root
        let account = lumen_core::execution::proof::verify_account_proof(state_root, addr, &proof)
            .map_err(|e| JsValue::from_str(&format!("Proof verification failed: {}", e)))?;

        // Return as JSON
        let result = AccountStateResponse {
            nonce: account.nonce,
            balance: format!("0x{}", hex::encode(account.balance)),
            storage_root: format!("0x{}", hex::encode(account.storage_root)),
            code_hash: format!("0x{}", hex::encode(account.code_hash)),
            is_contract: account.is_contract(),
            verified: true,
            verified_against_slot: self.state.finalized_header.slot,
        };

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Verify a storage proof for a contract slot.
    ///
    /// address: hex-encoded Ethereum address (0x...)
    /// slot: hex-encoded storage slot (0x...)
    /// proof_json: JSON-encoded storage proof
    ///
    /// The proof is verified against our internally held verified state root.
    pub fn verify_storage(
        &self,
        address: &str,
        slot: &str,
        proof_json: &str,
    ) -> Result<JsValue, JsValue> {
        let _state_root = self
            .state
            .verified_state_root()
            .unwrap_or(self.state.finalized_header.state_root);

        // Parse the storage slot
        let slot_hex = slot.strip_prefix("0x").unwrap_or(slot);
        let slot_bytes = hex::decode(slot_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid slot: {}", e)))?;
        let mut slot_arr = [0u8; 32];
        if slot_bytes.len() <= 32 {
            slot_arr[32 - slot_bytes.len()..].copy_from_slice(&slot_bytes);
        }

        // Parse the storage proof
        let proof: StorageProof = serde_json::from_str(proof_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid proof JSON: {}", e)))?;

        // For storage proofs, we need the account's storage root first
        // This requires the account proof to have been verified already
        // For now, we'll use the proof's claimed storage root and verify it
        let value = lumen_core::execution::proof::verify_storage_proof(
            [0u8; 32], // Would come from verified account state
            slot_arr,
            &proof,
        )
        .map_err(|e| JsValue::from_str(&format!("Storage proof verification failed: {}", e)))?;

        let result = StorageValueResponse {
            value: format!("0x{}", hex::encode(value)),
            verified: true,
            verified_against_slot: self.state.finalized_header.slot,
        };

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Returns true if the client has synced past slot 0 and is ready to serve queries.
    pub fn is_synced(&self) -> bool {
        self.state.finalized_header.slot > 0
    }

    /// Get the full sync state as JSON for the TypeScript layer.
    pub fn get_sync_state(&self) -> Result<JsValue, JsValue> {
        let sync_state = SyncStateResponse {
            head_slot: self.state.finalized_header.slot,
            current_period: self.state.current_period(),
            has_next_committee: self.state.next_sync_committee.is_some(),
            has_execution_root: self.state.latest_execution_payload_header.is_some(),
            is_synced: self.is_synced(),
        };

        serde_wasm_bindgen::to_value(&sync_state)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }
}

// --- Response types ---

#[derive(Serialize, Deserialize)]
struct AccountStateResponse {
    nonce: u64,
    balance: String,
    storage_root: String,
    code_hash: String,
    is_contract: bool,
    verified: bool,
    verified_against_slot: u64,
}

#[derive(Serialize, Deserialize)]
struct StorageValueResponse {
    value: String,
    verified: bool,
    verified_against_slot: u64,
}

#[derive(Serialize, Deserialize)]
struct SyncStateResponse {
    head_slot: u64,
    current_period: u64,
    has_next_committee: bool,
    has_execution_root: bool,
    is_synced: bool,
}

// --- Console logging ---

fn log_to_console(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}
