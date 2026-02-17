use serde::{Deserialize, Serialize};
use hex;

/// Number of validators in the Ethereum beacon chain sync committee.
pub const SYNC_COMMITTEE_SIZE: usize = 512;

/// Number of bytes in a BLS12-381 public key (compressed).
pub const BLS_PUBKEY_LEN: usize = 48;

/// Number of bytes in a BLS12-381 signature (compressed).
pub const BLS_SIGNATURE_LEN: usize = 96;

/// Slots per sync committee period (256 epochs * 32 slots/epoch = 8192).
pub const SLOTS_PER_SYNC_COMMITTEE_PERIOD: u64 = 8192;

/// Epochs per sync committee period.
pub const EPOCHS_PER_SYNC_COMMITTEE_PERIOD: u64 = 256;

/// Slots per epoch.
pub const SLOTS_PER_EPOCH: u64 = 32;

/// Domain type for sync committee signatures.
pub const DOMAIN_SYNC_COMMITTEE: [u8; 4] = [0x07, 0x00, 0x00, 0x00];

/// Minimum number of sync committee participants required (2/3 of 512).
pub const MIN_SYNC_COMMITTEE_PARTICIPANTS: usize = 342;

/// A BLS12-381 public key (48 bytes, compressed G1 point).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlsPublicKey(pub [u8; BLS_PUBKEY_LEN]);

impl Serialize for BlsPublicKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for BlsPublicKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
        Self::from_bytes(&bytes).map_err(serde::de::Error::custom)
    }
}

impl BlsPublicKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != BLS_PUBKEY_LEN {
            return Err("Invalid BLS public key length");
        }
        let mut arr = [0u8; BLS_PUBKEY_LEN];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }
}

/// A BLS12-381 signature (96 bytes, compressed G2 point).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlsSignature(pub [u8; BLS_SIGNATURE_LEN]);

impl Serialize for BlsSignature {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for BlsSignature {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
        Self::from_bytes(&bytes).map_err(serde::de::Error::custom)
    }
}

impl BlsSignature {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != BLS_SIGNATURE_LEN {
            return Err("Invalid BLS signature length");
        }
        let mut arr = [0u8; BLS_SIGNATURE_LEN];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }
}

/// A beacon chain block header.
/// This is the minimal header — enough to verify the chain without storing full blocks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeaconBlockHeader {
    /// Slot number of this block.
    pub slot: u64,
    /// Index of the validator who proposed this block.
    pub proposer_index: u64,
    /// Root hash of the parent beacon block.
    pub parent_root: [u8; 32],
    /// Root hash of the beacon state after processing this block.
    pub state_root: [u8; 32],
    /// Root hash of the block body.
    pub body_root: [u8; 32],
}

/// The sync committee — 512 validators that sign off on the chain head.
/// Rotates every ~27 hours (256 epochs).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncCommittee {
    /// 512 BLS public keys of committee members.
    pub pubkeys: Vec<BlsPublicKey>,
    /// Aggregated public key for fast signature verification.
    pub aggregate_pubkey: BlsPublicKey,
}

impl SyncCommittee {
    /// Validate the sync committee has the correct number of members.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.pubkeys.len() != SYNC_COMMITTEE_SIZE {
            return Err("Sync committee must have exactly 512 members");
        }
        Ok(())
    }
}

/// The aggregate BLS signature from the sync committee.
/// Contains a bitvector indicating which of the 512 members signed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncAggregate {
    /// Bitvector indicating which of the 512 committee members signed.
    /// Each bit corresponds to a committee member at the same index.
    pub sync_committee_bits: Vec<u8>,
    /// The aggregated BLS signature from all participating members.
    pub sync_committee_signature: BlsSignature,
}

impl SyncAggregate {
    /// Count how many sync committee members participated (set bits).
    pub fn num_participants(&self) -> usize {
        self.sync_committee_bits
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum()
    }

    /// Check if a specific committee member (by index) participated.
    pub fn has_participant(&self, index: usize) -> bool {
        if index >= SYNC_COMMITTEE_SIZE {
            return false;
        }
        let byte_index = index / 8;
        let bit_index = index % 8;
        if byte_index >= self.sync_committee_bits.len() {
            return false;
        }
        (self.sync_committee_bits[byte_index] >> bit_index) & 1 == 1
    }

    /// Get the indices of all participating committee members.
    pub fn participant_indices(&self) -> Vec<usize> {
        (0..SYNC_COMMITTEE_SIZE)
            .filter(|&i| self.has_participant(i))
            .collect()
    }
}

/// A light client update from the beacon chain.
/// This is what peers send us to update our view of the chain head.
/// Every field must be cryptographically verified before accepting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightClientUpdate {
    /// The header that the sync committee is attesting to.
    pub attested_header: BeaconBlockHeader,
    /// The next sync committee (present only during committee rotations).
    pub next_sync_committee: Option<SyncCommittee>,
    /// Merkle branch proving next_sync_committee against the beacon state.
    pub next_sync_committee_branch: Vec<[u8; 32]>,
    /// The latest finalized header that this update references.
    pub finalized_header: BeaconBlockHeader,
    /// Merkle branch proving finalized_header against the beacon state.
    pub finality_branch: Vec<[u8; 32]>,
    /// The aggregate signature from the sync committee.
    pub sync_aggregate: SyncAggregate,
    /// The slot at which the signature was produced.
    pub signature_slot: u64,
}

/// A light client bootstrap — the initial data needed to start syncing.
/// Contains the trusted checkpoint header and the current sync committee.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightClientBootstrap {
    /// The beacon block header at the checkpoint.
    pub header: BeaconBlockHeader,
    /// The current sync committee at the checkpoint.
    pub current_sync_committee: SyncCommittee,
    /// Merkle branch proving current_sync_committee against the beacon state.
    pub current_sync_committee_branch: Vec<[u8; 32]>,
}

/// Execution payload header — the link between beacon and execution layers.
/// Contains the state root we use for Merkle proof verification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPayloadHeader {
    /// Hash of the parent execution block.
    pub parent_hash: [u8; 32],
    /// Address of the fee recipient (coinbase).
    pub fee_recipient: [u8; 20],
    /// Root of the execution layer state trie — THIS is what we verify proofs against.
    pub state_root: [u8; 32],
    /// Root of the receipts trie.
    pub receipts_root: [u8; 32],
    /// Block number in the execution layer.
    pub block_number: u64,
    /// Gas limit.
    pub gas_limit: u64,
    /// Gas used.
    pub gas_used: u64,
    /// Block timestamp.
    pub timestamp: u64,
    /// Base fee per gas.
    pub base_fee_per_gas: u64,
    /// Hash of the execution block.
    pub block_hash: [u8; 32],
    /// Root of the transactions trie.
    pub transactions_root: [u8; 32],
    /// Root of the withdrawals trie.
    pub withdrawals_root: [u8; 32],
}

/// The verified state of the light client.
/// This is our accumulated knowledge about the chain, built from verified updates.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightClientState {
    /// The latest finalized beacon block header we have verified.
    pub finalized_header: BeaconBlockHeader,
    /// The current sync committee (used to verify signatures in the current period).
    pub current_sync_committee: SyncCommittee,
    /// The next sync committee (if known, used after the current period ends).
    pub next_sync_committee: Option<SyncCommittee>,
    /// The latest known execution payload header (for state root proofs).
    pub latest_execution_payload_header: Option<ExecutionPayloadHeader>,
    /// Genesis validators root — needed for domain computation.
    pub genesis_validators_root: [u8; 32],
    /// Current fork version — changes with hard forks.
    pub fork_version: [u8; 4],
    /// The slot at which this state was last updated.
    pub last_updated_slot: u64,
}

impl LightClientState {
    /// Get the current sync committee period based on the finalized header slot.
    pub fn current_period(&self) -> u64 {
        self.finalized_header.slot / SLOTS_PER_SYNC_COMMITTEE_PERIOD
    }

    /// Check if the client has synced to at least the given slot.
    pub fn is_synced_to(&self, slot: u64) -> bool {
        self.finalized_header.slot >= slot
    }

    /// Get the verified state root for Merkle proof verification.
    /// Returns None if we don't have an execution payload header yet.
    pub fn verified_state_root(&self) -> Option<[u8; 32]> {
        self.latest_execution_payload_header
            .as_ref()
            .map(|h| h.state_root)
    }
}

/// Fork data used for computing signing domains.
#[derive(Clone, Debug)]
pub struct ForkData {
    pub current_version: [u8; 4],
    pub genesis_validators_root: [u8; 32],
}

/// Signing domain — computed from fork version and genesis validators root.
/// Used to prevent cross-chain replay attacks.
#[derive(Clone, Debug)]
pub struct SigningDomain(pub [u8; 32]);
