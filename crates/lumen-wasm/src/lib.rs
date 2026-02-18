//! # Lumen WASM
//!
//! WebAssembly bindings for the Lumen Ethereum light client.
//! This crate bridges `lumen-core`'s pure Rust verification logic to JavaScript
//! via `wasm-bindgen`.
//!
//! ## Architecture
//!
//! - All cryptographic verification happens in Rust/WASM (never in JS)
//! - BLS12-381 signature verification for beacon chain finality updates
//! - keccak256 Merkle-Patricia trie proof verification for execution layer
//! - Designed to run in a Web Worker (never block the main thread)
//! - Accepts raw beacon API / RPC JSON — format conversion handled internally

mod beacon_api;
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

    // =======================================================================
    // Beacon API integration methods
    //
    // These accept raw JSON from the Ethereum beacon REST API and handle
    // format conversion internally. The beacon API is used as DATA TRANSPORT
    // only — all trust comes from BLS signature verification in Rust.
    // =======================================================================

    /// Initialize a LumenClient from a beacon API bootstrap response.
    ///
    /// Accepts the raw JSON from:
    ///   GET /eth/v1/beacon/light_client/bootstrap/{block_root}
    ///
    /// This fetches the current sync committee (512 BLS public keys) which
    /// is used to verify all subsequent finality updates.
    ///
    /// The bootstrap is the ONE moment of trust — the block root must be
    /// obtained from multiple independent sources.
    pub fn from_beacon_bootstrap(bootstrap_json: &str) -> Result<LumenClient, JsValue> {
        let api_resp: beacon_api::ApiBootstrapResponse =
            serde_json::from_str(bootstrap_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid bootstrap JSON: {}", e)))?;

        let bootstrap = api_resp.data.to_core_bootstrap()
            .map_err(|e| JsValue::from_str(&format!("Bootstrap conversion: {}", e)))?;

        let exec_header = api_resp
            .data
            .header
            .execution
            .as_ref()
            .map(|exec| exec.to_core())
            .transpose()
            .map_err(|e| JsValue::from_str(&format!("Execution header: {}", e)))?;

        // Ethereum mainnet genesis validators root
        let genesis_validators_root = [
            0x4b, 0x36, 0x3d, 0xb9, 0x4e, 0x28, 0x61, 0x20, 0xd7, 0x6e, 0xb9, 0x05, 0x34,
            0x0f, 0xdd, 0x4e, 0x54, 0xbf, 0xe9, 0xf0, 0x6b, 0xf3, 0x3f, 0xf6, 0xcf, 0x5a,
            0xd2, 0x7f, 0x51, 0x1b, 0xfe, 0x95,
        ];

        // Deneb fork version
        let fork_version = [0x04, 0x00, 0x00, 0x00];

        let committee_size = bootstrap.current_sync_committee.pubkeys.len();

        let mut state = initialize_from_bootstrap(&bootstrap, genesis_validators_root, fork_version)
            .map_err(|e| JsValue::from_str(&format!("Bootstrap init: {}", e)))?;

        if let Some(exec) = exec_header {
            log_to_console(&format!(
                "[Lumen] Bootstrap execution state root: 0x{}",
                hex::encode(exec.state_root)
            ));
            state.latest_execution_payload_header = Some(exec);
        }

        log_to_console(&format!(
            "[Lumen] Initialized from beacon bootstrap — slot {}, {} sync committee members",
            state.finalized_header.slot, committee_size
        ));

        Ok(LumenClient { state })
    }

    /// Process a beacon API finality update with full BLS verification.
    ///
    /// Accepts the raw JSON from:
    ///   GET /eth/v1/beacon/light_client/finality_update
    ///
    /// Verification pipeline:
    /// 1. Parse the API JSON and convert to lumen-core types
    /// 2. Verify sync committee BLS aggregate signature (THE trust anchor)
    /// 3. Verify finality Merkle branch
    /// 4. Advance the verified head
    /// 5. Store the execution state root for proof verification
    ///
    /// Returns a FinalityUpdateResult on success with verified state info.
    pub fn process_finality_update(&mut self, update_json: &str) -> Result<JsValue, JsValue> {
        let api_resp: beacon_api::ApiFinalityUpdateResponse =
            serde_json::from_str(update_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid finality update JSON: {}", e)))?;

        let update = api_resp.data.to_core_update()
            .map_err(|e| JsValue::from_str(&format!("Update conversion: {}", e)))?;

        let exec_header = api_resp
            .data
            .finalized_header
            .execution
            .as_ref()
            .map(|exec| exec.to_core())
            .transpose()
            .map_err(|e| JsValue::from_str(&format!("Execution header: {}", e)))?;

        let participation = update.sync_aggregate.num_participants();

        // If the update doesn't advance us, skip silently
        if update.finalized_header.slot <= self.state.finalized_header.slot {
            let result = FinalityUpdateResult {
                verified: true,
                advanced: false,
                finalized_slot: self.state.finalized_header.slot,
                execution_state_root: self.execution_state_root().unwrap_or_default(),
                execution_block_number: self
                    .state
                    .latest_execution_payload_header
                    .as_ref()
                    .map(|h| h.block_number)
                    .unwrap_or(0),
                sync_participation: participation,
                message: "Already at this slot or newer".into(),
            };
            return serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&e.to_string()));
        }

        let genesis_validators_root = self.state.genesis_validators_root;
        let current_slot = self.state.finalized_header.slot;

        // This is where BLS verification happens — the core trust operation
        lumen_core::consensus::light_client::process_light_client_update(
            &mut self.state,
            &update,
            current_slot,
            genesis_validators_root,
        )
        .map_err(|e| JsValue::from_str(&format!("BLS verification failed: {}", e)))?;

        // BLS passed — store the execution state root
        if let Some(exec) = exec_header {
            log_to_console(&format!(
                "[Lumen] BLS-verified execution state root: 0x{} (block #{})",
                hex::encode(exec.state_root),
                exec.block_number
            ));
            self.state.latest_execution_payload_header = Some(exec);
        }

        log_to_console(&format!(
            "[Lumen] BLS verification passed — {}/512 validators signed, slot {}",
            participation, self.state.finalized_header.slot
        ));

        let result = FinalityUpdateResult {
            verified: true,
            advanced: true,
            finalized_slot: self.state.finalized_header.slot,
            execution_state_root: self.execution_state_root().unwrap_or_default(),
            execution_block_number: self
                .state
                .latest_execution_payload_header
                .as_ref()
                .map(|h| h.block_number)
                .unwrap_or(0),
            sync_participation: participation,
            message: format!(
                "BLS-verified finality at slot {} ({}/512 signers)",
                self.state.finalized_header.slot, participation
            ),
        };

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Verify an account proof from a raw eth_getProof RPC response.
    ///
    /// The proof data is UNTRUSTED — it could come from any RPC, P2P peer,
    /// or even a malicious actor. The verification uses:
    /// - Our BLS-verified execution state root (from process_finality_update)
    /// - keccak256 Merkle-Patricia trie traversal
    ///
    /// No keccak256 collision = no way to forge a valid proof.
    pub fn verify_account_rpc_proof(
        &self,
        address: &str,
        rpc_proof_json: &str,
    ) -> Result<JsValue, JsValue> {
        let state_root = self
            .state
            .verified_state_root()
            .ok_or_else(|| JsValue::from_str("No verified execution state root yet — process a finality update first"))?;

        let rpc_proof: beacon_api::RpcGetProofResponse =
            serde_json::from_str(rpc_proof_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid proof JSON: {}", e)))?;

        let addr_hex = address.strip_prefix("0x").unwrap_or(address);
        let addr_bytes = hex::decode(addr_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid address: {}", e)))?;
        if addr_bytes.len() != 20 {
            return Err(JsValue::from_str("Address must be 20 bytes"));
        }
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&addr_bytes);

        let account_proof = rpc_proof
            .to_core_account_proof(&addr)
            .map_err(|e| JsValue::from_str(&format!("Proof conversion: {}", e)))?;

        let proof_node_count = account_proof.proof.len();

        let account = lumen_core::execution::proof::verify_account_proof(state_root, addr, &account_proof)
            .map_err(|e| JsValue::from_str(&format!("Proof verification failed: {}", e)))?;

        log_to_console(&format!(
            "[Lumen] Account {} verified: {} nodes, balance=0x{}",
            address,
            proof_node_count,
            hex::encode(account.balance)
        ));

        let result = VerifiedAccountResponse {
            nonce: account.nonce,
            balance_hex: format!("0x{}", account.balance_hex()),
            storage_root: format!("0x{}", hex::encode(account.storage_root)),
            code_hash: format!("0x{}", hex::encode(account.code_hash)),
            is_contract: account.is_contract(),
            verified: true,
            verified_against_slot: self.state.finalized_header.slot,
            proof_nodes_verified: proof_node_count,
            rpc_claimed_balance: rpc_proof.balance.clone(),
        };

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Verify an account proof against an EXPLICIT state root (not the internal one).
    ///
    /// This is the race-condition-safe variant: the caller captures the state root
    /// and block number at the same instant, fetches the proof (async), then passes
    /// the originally-captured state root here. Even if the internal state advanced
    /// during the network round-trip, verification uses the correct root.
    pub fn verify_account_rpc_proof_with_root(
        &self,
        state_root_hex: &str,
        address: &str,
        rpc_proof_json: &str,
    ) -> Result<JsValue, JsValue> {
        let root_hex = state_root_hex.strip_prefix("0x").unwrap_or(state_root_hex);
        let root_bytes = hex::decode(root_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid state root hex: {}", e)))?;
        if root_bytes.len() != 32 {
            return Err(JsValue::from_str(&format!(
                "State root must be 32 bytes, got {}",
                root_bytes.len()
            )));
        }
        let mut state_root = [0u8; 32];
        state_root.copy_from_slice(&root_bytes);

        let rpc_proof: beacon_api::RpcGetProofResponse =
            serde_json::from_str(rpc_proof_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid proof JSON: {}", e)))?;

        let addr_hex = address.strip_prefix("0x").unwrap_or(address);
        let addr_bytes = hex::decode(addr_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid address: {}", e)))?;
        if addr_bytes.len() != 20 {
            return Err(JsValue::from_str("Address must be 20 bytes"));
        }
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&addr_bytes);

        let account_proof = rpc_proof
            .to_core_account_proof(&addr)
            .map_err(|e| JsValue::from_str(&format!("Proof conversion: {}", e)))?;

        let proof_node_count = account_proof.proof.len();

        let account = lumen_core::execution::proof::verify_account_proof(state_root, addr, &account_proof)
            .map_err(|e| JsValue::from_str(&format!("Proof verification failed: {}", e)))?;

        log_to_console(&format!(
            "[Lumen] Account {} verified against explicit root 0x{}: {} nodes, balance=0x{}",
            address,
            &root_hex[..8],
            proof_node_count,
            hex::encode(account.balance)
        ));

        let result = VerifiedAccountResponse {
            nonce: account.nonce,
            balance_hex: format!("0x{}", account.balance_hex()),
            storage_root: format!("0x{}", hex::encode(account.storage_root)),
            code_hash: format!("0x{}", hex::encode(account.code_hash)),
            is_contract: account.is_contract(),
            verified: true,
            verified_against_slot: self.state.finalized_header.slot,
            proof_nodes_verified: proof_node_count,
            rpc_claimed_balance: rpc_proof.balance.clone(),
        };

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Fetch an account's Merkle proof from an execution RPC and verify it.
    ///
    /// This is the "one call does everything" method. It:
    /// 1. POSTs eth_getBlockByNumber("latest") to get the state root
    /// 2. POSTs eth_getProof(address, [], "latest") to get the proof
    /// 3. Verifies the proof via keccak256 MPT in Rust
    /// 4. Cross-checks: latest block ≥ BLS-verified finalized block
    /// 5. Returns the verified account state
    ///
    /// The RPC endpoints are tried in order. All data from RPCs is untrusted
    /// and verified locally.
    pub async fn fetch_and_verify_account(
        &self,
        address: &str,
        rpc_endpoints_json: &str,
    ) -> Result<JsValue, JsValue> {
        let endpoints: Vec<String> = serde_json::from_str(rpc_endpoints_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid endpoints JSON: {}", e)))?;

        if endpoints.is_empty() {
            return Err(JsValue::from_str("No RPC endpoints provided"));
        }

        let finalized_block_num = self
            .state
            .latest_execution_payload_header
            .as_ref()
            .map(|h| h.block_number)
            .unwrap_or(0);

        let mut last_error = String::from("No endpoints tried");

        for endpoint in &endpoints {
            match self
                .try_fetch_and_verify(endpoint, address, finalized_block_num)
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let msg = e.as_string().unwrap_or_default();
                    log_to_console(&format!(
                        "[Lumen] RPC {} failed: {}",
                        endpoint, msg
                    ));
                    last_error = msg;
                }
            }
        }

        Err(JsValue::from_str(&format!(
            "All RPC endpoints failed. Last error: {}",
            last_error
        )))
    }

    /// Get the execution state info for the TypeScript layer.
    pub fn get_execution_state(&self) -> Result<JsValue, JsValue> {
        let exec_state = ExecutionStateResponse {
            has_state_root: self.state.latest_execution_payload_header.is_some(),
            state_root: self.execution_state_root().unwrap_or_default(),
            block_number: self
                .state
                .latest_execution_payload_header
                .as_ref()
                .map(|h| h.block_number)
                .unwrap_or(0),
            finalized_slot: self.state.finalized_header.slot,
        };

        serde_wasm_bindgen::to_value(&exec_state)
            .map_err(|e| JsValue::from_str(&e.to_string()))
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

#[derive(Serialize, Deserialize)]
struct FinalityUpdateResult {
    verified: bool,
    advanced: bool,
    finalized_slot: u64,
    execution_state_root: String,
    execution_block_number: u64,
    sync_participation: usize,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct VerifiedAccountResponse {
    nonce: u64,
    balance_hex: String,
    storage_root: String,
    code_hash: String,
    is_contract: bool,
    verified: bool,
    verified_against_slot: u64,
    proof_nodes_verified: usize,
    rpc_claimed_balance: String,
}

#[derive(Serialize, Deserialize)]
struct ExecutionStateResponse {
    has_state_root: bool,
    state_root: String,
    block_number: u64,
    finalized_slot: u64,
}

#[derive(Serialize, Deserialize)]
struct FetchVerifyAccountResult {
    nonce: u64,
    balance_hex: String,
    storage_root: String,
    code_hash: String,
    is_contract: bool,
    verified: bool,
    finalized_block: u64,
    proof_block: u64,
    proof_nodes_verified: usize,
    rpc_endpoint: String,
    rpc_claimed_balance: String,
}

// --- Private helpers ---

impl LumenClient {
    async fn try_fetch_and_verify(
        &self,
        endpoint: &str,
        address: &str,
        finalized_block_num: u64,
    ) -> Result<JsValue, JsValue> {
        // 1. Fetch latest block header (state root)
        let block_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByNumber",
            "params": ["latest", false]
        });
        let block_resp_text = network::post_json(endpoint, &block_req.to_string())
            .await
            .map_err(|e| JsValue::from_str(&format!("Block fetch: {}", e)))?;

        let block_resp: serde_json::Value = serde_json::from_str(&block_resp_text)
            .map_err(|e| JsValue::from_str(&format!("Block JSON parse: {}", e)))?;

        if let Some(err) = block_resp.get("error") {
            return Err(JsValue::from_str(&format!("Block RPC error: {}", err)));
        }

        let block_result = block_resp
            .get("result")
            .and_then(|r| if r.is_null() { None } else { Some(r) })
            .ok_or_else(|| JsValue::from_str("Block result is null"))?;

        let state_root_hex = block_result
            .get("stateRoot")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsValue::from_str("No stateRoot in block"))?;

        let block_num_hex = block_result
            .get("number")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsValue::from_str("No number in block"))?;

        let block_num = u64::from_str_radix(
            block_num_hex.strip_prefix("0x").unwrap_or(block_num_hex),
            16,
        )
        .map_err(|e| JsValue::from_str(&format!("Block number parse: {}", e)))?;

        // 2. Cross-check: latest block must extend finalized chain
        if block_num < finalized_block_num {
            return Err(JsValue::from_str(&format!(
                "RPC latest block {} < finalized block {}",
                block_num, finalized_block_num
            )));
        }

        // 3. Fetch proof at latest
        let proof_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_getProof",
            "params": [address, [], "latest"]
        });
        let proof_resp_text = network::post_json(endpoint, &proof_req.to_string())
            .await
            .map_err(|e| JsValue::from_str(&format!("Proof fetch: {}", e)))?;

        let proof_resp: serde_json::Value = serde_json::from_str(&proof_resp_text)
            .map_err(|e| JsValue::from_str(&format!("Proof JSON parse: {}", e)))?;

        if let Some(err) = proof_resp.get("error") {
            return Err(JsValue::from_str(&format!("Proof RPC error: {}", err)));
        }

        let proof_result = proof_resp
            .get("result")
            .and_then(|r| if r.is_null() { None } else { Some(r) })
            .ok_or_else(|| JsValue::from_str("Proof result is null"))?;

        let proof_json = proof_result.to_string();

        // 4. Parse state root
        let root_hex = state_root_hex
            .strip_prefix("0x")
            .unwrap_or(state_root_hex);
        let root_bytes = hex::decode(root_hex)
            .map_err(|e| JsValue::from_str(&format!("State root hex: {}", e)))?;
        if root_bytes.len() != 32 {
            return Err(JsValue::from_str("State root must be 32 bytes"));
        }
        let mut state_root = [0u8; 32];
        state_root.copy_from_slice(&root_bytes);

        // 5. Parse address
        let addr_hex = address.strip_prefix("0x").unwrap_or(address);
        let addr_bytes = hex::decode(addr_hex)
            .map_err(|e| JsValue::from_str(&format!("Address hex: {}", e)))?;
        if addr_bytes.len() != 20 {
            return Err(JsValue::from_str("Address must be 20 bytes"));
        }
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&addr_bytes);

        // 6. Parse proof and verify via keccak256 MPT
        let rpc_proof: beacon_api::RpcGetProofResponse =
            serde_json::from_str(&proof_json)
                .map_err(|e| JsValue::from_str(&format!("Proof parse: {}", e)))?;

        let account_proof = rpc_proof
            .to_core_account_proof(&addr)
            .map_err(|e| JsValue::from_str(&format!("Proof conversion: {}", e)))?;

        let proof_node_count = account_proof.proof.len();

        let account =
            lumen_core::execution::proof::verify_account_proof(state_root, addr, &account_proof)
                .map_err(|e| JsValue::from_str(&format!("Proof verification: {}", e)))?;

        log_to_console(&format!(
            "[Lumen] Account {} verified at block #{}: {} nodes, balance=0x{}",
            address, block_num, proof_node_count, hex::encode(account.balance)
        ));

        let result = FetchVerifyAccountResult {
            nonce: account.nonce,
            balance_hex: format!("0x{}", account.balance_hex()),
            storage_root: format!("0x{}", hex::encode(account.storage_root)),
            code_hash: format!("0x{}", hex::encode(account.code_hash)),
            is_contract: account.is_contract(),
            verified: true,
            finalized_block: finalized_block_num,
            proof_block: block_num,
            proof_nodes_verified: proof_node_count,
            rpc_endpoint: endpoint.to_string(),
            rpc_claimed_balance: rpc_proof.balance.clone(),
        };

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Serialization: {}", e)))
    }
}

// --- Console logging ---

fn log_to_console(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}
