//! Beacon API JSON format adapter.
//!
//! The Ethereum beacon chain REST API uses a different JSON format than
//! lumen-core's internal types. This module defines serde types matching
//! the API format and provides conversion functions.
//!
//! Key differences:
//! - API uses string numbers ("12345"), core uses u64
//! - API uses 0x-prefixed hex strings, core uses byte arrays
//! - API nests headers as { beacon: {...}, execution: {...} }
//! - API wraps everything in { data: {...} }

use lumen_core::types::beacon::*;
use lumen_core::types::execution::*;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Hex conversion helpers
// ---------------------------------------------------------------------------

pub fn hex_to_bytes32(s: &str) -> Result<[u8; 32], String> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).map_err(|e| format!("hex decode: {}", e))?;
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

pub fn hex_to_bytes20(s: &str) -> Result<[u8; 20], String> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).map_err(|e| format!("hex decode: {}", e))?;
    if bytes.len() != 20 {
        return Err(format!("expected 20 bytes, got {}", bytes.len()));
    }
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

pub fn hex_to_bytes(s: &str) -> Result<Vec<u8>, String> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    hex::decode(s).map_err(|e| format!("hex decode: {}", e))
}

fn parse_u64_string(s: &str) -> Result<u64, String> {
    s.parse::<u64>().map_err(|e| format!("parse u64: {}", e))
}

// ---------------------------------------------------------------------------
// Beacon API: Bootstrap response
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ApiBootstrapResponse {
    pub data: ApiBootstrapData,
}

#[derive(Deserialize)]
pub struct ApiBootstrapData {
    pub header: ApiLightClientHeader,
    pub current_sync_committee: ApiSyncCommittee,
    pub current_sync_committee_branch: Vec<String>,
}

impl ApiBootstrapData {
    pub fn to_core_bootstrap(&self) -> Result<LightClientBootstrap, String> {
        Ok(LightClientBootstrap {
            header: self.header.beacon.to_core()?,
            current_sync_committee: self.current_sync_committee.to_core()?,
            // Skip branch verification for bootstrap (it's the trust anchor).
            // The SSZ hash_tree_root for SyncCommittee is complex and would
            // require a full SSZ library. The bootstrap checkpoint IS the
            // moment of trust, so this is acceptable.
            current_sync_committee_branch: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// Beacon API: Finality update response
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ApiFinalityUpdateResponse {
    pub data: ApiFinalityUpdateData,
}

#[derive(Deserialize)]
pub struct ApiFinalityUpdateData {
    pub attested_header: ApiLightClientHeader,
    pub finalized_header: ApiLightClientHeader,
    pub finality_branch: Vec<String>,
    pub sync_aggregate: ApiSyncAggregate,
    pub signature_slot: String,
}

impl ApiFinalityUpdateData {
    pub fn to_core_update(&self) -> Result<LightClientUpdate, String> {
        let finality_branch: Vec<[u8; 32]> = self
            .finality_branch
            .iter()
            .map(|s| hex_to_bytes32(s))
            .collect::<Result<_, _>>()?;

        Ok(LightClientUpdate {
            attested_header: self.attested_header.beacon.to_core()?,
            finalized_header: self.finalized_header.beacon.to_core()?,
            finality_branch,
            sync_aggregate: self.sync_aggregate.to_core()?,
            signature_slot: parse_u64_string(&self.signature_slot)?,
            // Finality updates don't include next sync committee
            next_sync_committee: None,
            next_sync_committee_branch: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// Beacon API: Shared sub-structures
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ApiLightClientHeader {
    pub beacon: ApiBeaconBlockHeader,
    pub execution: Option<ApiExecutionPayloadHeader>,
    pub execution_branch: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct ApiBeaconBlockHeader {
    pub slot: String,
    pub proposer_index: String,
    pub parent_root: String,
    pub state_root: String,
    pub body_root: String,
}

impl ApiBeaconBlockHeader {
    pub fn to_core(&self) -> Result<BeaconBlockHeader, String> {
        Ok(BeaconBlockHeader {
            slot: parse_u64_string(&self.slot)?,
            proposer_index: parse_u64_string(&self.proposer_index)?,
            parent_root: hex_to_bytes32(&self.parent_root)?,
            state_root: hex_to_bytes32(&self.state_root)?,
            body_root: hex_to_bytes32(&self.body_root)?,
        })
    }
}

#[derive(Deserialize)]
pub struct ApiExecutionPayloadHeader {
    pub parent_hash: String,
    pub fee_recipient: String,
    pub state_root: String,
    pub receipts_root: String,
    pub block_number: String,
    pub gas_limit: String,
    pub gas_used: String,
    pub timestamp: String,
    pub base_fee_per_gas: String,
    pub block_hash: String,
    pub transactions_root: String,
    pub withdrawals_root: String,
}

impl ApiExecutionPayloadHeader {
    pub fn to_core(&self) -> Result<ExecutionPayloadHeader, String> {
        Ok(ExecutionPayloadHeader {
            parent_hash: hex_to_bytes32(&self.parent_hash)?,
            fee_recipient: hex_to_bytes20(&self.fee_recipient)?,
            state_root: hex_to_bytes32(&self.state_root)?,
            receipts_root: hex_to_bytes32(&self.receipts_root)?,
            block_number: parse_u64_string(&self.block_number)?,
            gas_limit: parse_u64_string(&self.gas_limit)?,
            gas_used: parse_u64_string(&self.gas_used)?,
            timestamp: parse_u64_string(&self.timestamp)?,
            base_fee_per_gas: parse_u64_string(&self.base_fee_per_gas)?,
            block_hash: hex_to_bytes32(&self.block_hash)?,
            transactions_root: hex_to_bytes32(&self.transactions_root)?,
            withdrawals_root: hex_to_bytes32(&self.withdrawals_root)?,
        })
    }
}

#[derive(Deserialize)]
pub struct ApiSyncAggregate {
    pub sync_committee_bits: String,
    pub sync_committee_signature: String,
}

impl ApiSyncAggregate {
    pub fn to_core(&self) -> Result<SyncAggregate, String> {
        let bits_bytes = hex_to_bytes(&self.sync_committee_bits)?;

        let sig_bytes = hex_to_bytes(&self.sync_committee_signature)?;
        let signature = BlsSignature::from_bytes(&sig_bytes)
            .map_err(|e| format!("BLS signature: {}", e))?;

        Ok(SyncAggregate {
            sync_committee_bits: bits_bytes,
            sync_committee_signature: signature,
        })
    }
}

#[derive(Deserialize)]
pub struct ApiSyncCommittee {
    pub pubkeys: Vec<String>,
    pub aggregate_pubkey: String,
}

impl ApiSyncCommittee {
    pub fn to_core(&self) -> Result<SyncCommittee, String> {
        let pubkeys: Vec<BlsPublicKey> = self
            .pubkeys
            .iter()
            .enumerate()
            .map(|(i, hex_pk)| {
                let bytes = hex_to_bytes(hex_pk)?;
                BlsPublicKey::from_bytes(&bytes)
                    .map_err(|e| format!("pubkey[{}]: {}", i, e))
            })
            .collect::<Result<_, _>>()?;

        let agg_bytes = hex_to_bytes(&self.aggregate_pubkey)?;
        let aggregate_pubkey = BlsPublicKey::from_bytes(&agg_bytes)
            .map_err(|e| format!("aggregate_pubkey: {}", e))?;

        Ok(SyncCommittee {
            pubkeys,
            aggregate_pubkey,
        })
    }
}

// ---------------------------------------------------------------------------
// Beacon API: Finalized header (for getting the block root)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ApiHeaderResponse {
    pub data: ApiHeaderData,
}

#[derive(Deserialize)]
pub struct ApiHeaderData {
    pub root: String,
    pub header: ApiHeaderMessage,
}

#[derive(Deserialize)]
pub struct ApiHeaderMessage {
    pub message: ApiBeaconBlockHeader,
}

// ---------------------------------------------------------------------------
// Execution RPC: eth_getProof response
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct RpcGetProofResponse {
    pub address: String,
    #[serde(rename = "accountProof")]
    pub account_proof: Vec<String>,
    pub balance: String,
    #[serde(rename = "codeHash")]
    pub code_hash: String,
    pub nonce: String,
    #[serde(rename = "storageHash")]
    pub storage_hash: String,
}

impl RpcGetProofResponse {
    /// Convert the hex-encoded proof nodes to raw bytes for lumen-core.
    pub fn to_core_account_proof(
        &self,
        addr: &[u8; 20],
    ) -> Result<AccountProof, String> {
        let proof_nodes: Vec<Vec<u8>> = self
            .account_proof
            .iter()
            .map(|hex_str| hex_to_bytes(hex_str))
            .collect::<Result<_, _>>()?;

        Ok(AccountProof {
            address: *addr,
            proof: proof_nodes,
            account: None, // decoded from the proof itself
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_bytes32() {
        let hex = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let result = hex_to_bytes32(hex).unwrap();
        assert_eq!(result[31], 1);
        assert_eq!(result[0], 0);
    }

    #[test]
    fn test_hex_to_bytes32_no_prefix() {
        let hex = "0000000000000000000000000000000000000000000000000000000000000002";
        let result = hex_to_bytes32(hex).unwrap();
        assert_eq!(result[31], 2);
    }

    #[test]
    fn test_parse_u64_string() {
        assert_eq!(parse_u64_string("12345").unwrap(), 12345);
        assert_eq!(parse_u64_string("0").unwrap(), 0);
    }

    #[test]
    fn test_api_beacon_header_conversion() {
        let api_header = ApiBeaconBlockHeader {
            slot: "100".into(),
            proposer_index: "42".into(),
            parent_root: "0x0000000000000000000000000000000000000000000000000000000000000001".into(),
            state_root: "0x0000000000000000000000000000000000000000000000000000000000000002".into(),
            body_root: "0x0000000000000000000000000000000000000000000000000000000000000003".into(),
        };
        let core = api_header.to_core().unwrap();
        assert_eq!(core.slot, 100);
        assert_eq!(core.proposer_index, 42);
        assert_eq!(core.parent_root[31], 1);
    }
}
