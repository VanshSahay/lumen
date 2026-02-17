use crate::types::beacon::*;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors that can occur during sync committee signature verification.
/// Each variant represents a specific, actionable failure — never a generic "invalid" error.
#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("Insufficient sync committee participation: {participants}/512 (need at least {required})")]
    InsufficientParticipation {
        participants: usize,
        required: usize,
    },

    #[error("Invalid BLS signature: the aggregate signature does not verify against the participating committee members")]
    InvalidSignature,

    #[error("Invalid BLS public key at index {index}: {reason}")]
    InvalidPublicKey { index: usize, reason: String },

    #[error("Signature slot {signature_slot} is not after attested header slot {attested_slot}")]
    InvalidSlotOrder {
        signature_slot: u64,
        attested_slot: u64,
    },

    #[error("Attested header slot {attested_slot} is not after finalized header slot {finalized_slot}")]
    InvalidFinalityOrder {
        attested_slot: u64,
        finalized_slot: u64,
    },

    #[error("Invalid Merkle branch for finalized header: branch verification failed")]
    InvalidFinalityBranch,

    #[error("Invalid Merkle branch for next sync committee: branch verification failed")]
    InvalidNextSyncCommitteeBranch,

    #[error("Update slot {update_slot} is not newer than current state slot {current_slot}")]
    UpdateNotNewer {
        update_slot: u64,
        current_slot: u64,
    },

    #[error("Sync committee bits length mismatch: expected 64 bytes, got {got}")]
    InvalidSyncCommitteeBitsLength { got: usize },

    #[error("BLS aggregation error: {0}")]
    BlsError(String),
}

/// Compute the signing root for a beacon block header.
/// This is what the sync committee actually signs — not the header directly,
/// but the hash_tree_root(header) wrapped in a signing domain.
pub fn compute_signing_root(header: &BeaconBlockHeader, domain: &[u8; 32]) -> [u8; 32] {
    let header_root = hash_beacon_block_header(header);
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(&header_root);
    data[32..].copy_from_slice(domain);
    sha256_hash(&data)
}

/// Compute the domain for sync committee signing.
/// domain = domain_type + fork_data_root[:28]
pub fn compute_domain(
    domain_type: &[u8; 4],
    fork_version: &[u8; 4],
    genesis_validators_root: &[u8; 32],
) -> [u8; 32] {
    let fork_data_root = compute_fork_data_root(fork_version, genesis_validators_root);
    let mut domain = [0u8; 32];
    domain[..4].copy_from_slice(domain_type);
    domain[4..].copy_from_slice(&fork_data_root[..28]);
    domain
}

/// Compute the fork data root from fork version and genesis validators root.
fn compute_fork_data_root(
    fork_version: &[u8; 4],
    genesis_validators_root: &[u8; 32],
) -> [u8; 32] {
    let mut data = [0u8; 64];
    // SSZ encode: fork_version padded to 32 bytes, then genesis_validators_root
    data[..4].copy_from_slice(fork_version);
    // bytes 4..32 are zero (padding)
    data[32..].copy_from_slice(genesis_validators_root);
    sha256_hash(&data)
}

/// Hash a beacon block header using SHA256 hash_tree_root (simplified SSZ).
/// The header has 5 fields, each 32 bytes when SSZ-encoded.
pub fn hash_beacon_block_header(header: &BeaconBlockHeader) -> [u8; 32] {
    // SSZ hash_tree_root for a container with 5 fields:
    // 1. slot (uint64, padded to 32 bytes)
    // 2. proposer_index (uint64, padded to 32 bytes)
    // 3. parent_root (bytes32)
    // 4. state_root (bytes32)
    // 5. body_root (bytes32)

    let slot_leaf = uint64_to_leaf(header.slot);
    let proposer_leaf = uint64_to_leaf(header.proposer_index);
    let parent_leaf = header.parent_root;
    let state_leaf = header.state_root;
    let body_leaf = header.body_root;

    // Merkleize: 5 leaves -> pad to 8 (next power of 2)
    let zero = [0u8; 32];

    // Layer 0 (leaves): [slot, proposer, parent, state, body, 0, 0, 0]
    let h01 = sha256_pair(&slot_leaf, &proposer_leaf);
    let h23 = sha256_pair(&parent_leaf, &state_leaf);
    let h45 = sha256_pair(&body_leaf, &zero);
    let h67 = sha256_pair(&zero, &zero);

    // Layer 1
    let h0123 = sha256_pair(&h01, &h23);
    let h4567 = sha256_pair(&h45, &h67);

    // Root
    sha256_pair(&h0123, &h4567)
}

/// Verify a sync committee signature against a beacon block header.
/// This is the core trust anchor — if this passes, the header is legitimate.
///
/// Requires >= 2/3 of the 512 sync committee members to have signed.
/// Uses BLS signature aggregation — we verify one aggregate sig, not 512 individual ones.
pub fn verify_sync_committee_signature(
    update: &LightClientUpdate,
    current_sync_committee: &SyncCommittee,
    genesis_validators_root: [u8; 32],
    fork_version: [u8; 4],
) -> Result<(), VerificationError> {
    // Validate sync committee bits length
    if update.sync_aggregate.sync_committee_bits.len() != SYNC_COMMITTEE_SIZE / 8 {
        return Err(VerificationError::InvalidSyncCommitteeBitsLength {
            got: update.sync_aggregate.sync_committee_bits.len(),
        });
    }

    // Check participation threshold — need at least 2/3 of committee
    let num_participants = update.sync_aggregate.num_participants();
    if num_participants < MIN_SYNC_COMMITTEE_PARTICIPANTS {
        return Err(VerificationError::InsufficientParticipation {
            participants: num_participants,
            required: MIN_SYNC_COMMITTEE_PARTICIPANTS,
        });
    }

    // Verify slot ordering: signature_slot > attested_header.slot >= finalized_header.slot
    if update.signature_slot <= update.attested_header.slot {
        return Err(VerificationError::InvalidSlotOrder {
            signature_slot: update.signature_slot,
            attested_slot: update.attested_header.slot,
        });
    }

    if update.attested_header.slot < update.finalized_header.slot {
        return Err(VerificationError::InvalidFinalityOrder {
            attested_slot: update.attested_header.slot,
            finalized_slot: update.finalized_header.slot,
        });
    }

    // Compute the signing domain
    let domain = compute_domain(
        &DOMAIN_SYNC_COMMITTEE,
        &fork_version,
        &genesis_validators_root,
    );

    // Compute the signing root (what the committee actually signed)
    let signing_root = compute_signing_root(&update.attested_header, &domain);

    // Collect the public keys of participating committee members
    let participant_indices = update.sync_aggregate.participant_indices();
    let participant_pubkeys: Vec<&BlsPublicKey> = participant_indices
        .iter()
        .map(|&i| &current_sync_committee.pubkeys[i])
        .collect();

    // Verify the aggregate BLS signature
    verify_aggregate_bls_signature(
        &participant_pubkeys,
        &signing_root,
        &update.sync_aggregate.sync_committee_signature,
    )?;

    Ok(())
}

/// Verify an aggregate BLS12-381 signature.
/// Uses the blst library for actual cryptographic verification.
fn verify_aggregate_bls_signature(
    pubkeys: &[&BlsPublicKey],
    message: &[u8; 32],
    signature: &BlsSignature,
) -> Result<(), VerificationError> {
    use blst::min_pk::{AggregatePublicKey, PublicKey, Signature};
    use blst::BLST_ERROR;

    if pubkeys.is_empty() {
        return Err(VerificationError::InsufficientParticipation {
            participants: 0,
            required: MIN_SYNC_COMMITTEE_PARTICIPANTS,
        });
    }

    // Deserialize the signature
    let sig = Signature::from_bytes(&signature.0).map_err(|e| {
        VerificationError::BlsError(format!("Failed to deserialize signature: {:?}", e))
    })?;

    // Deserialize all public keys
    let pks: Vec<PublicKey> = pubkeys
        .iter()
        .enumerate()
        .map(|(i, pk)| {
            PublicKey::from_bytes(&pk.0).map_err(|e| VerificationError::InvalidPublicKey {
                index: i,
                reason: format!("{:?}", e),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Aggregate the public keys
    let pk_refs: Vec<&PublicKey> = pks.iter().collect();
    let agg_pk = AggregatePublicKey::aggregate(&pk_refs, false).map_err(|e| {
        VerificationError::BlsError(format!("Failed to aggregate public keys: {:?}", e))
    })?;

    let agg_pk_final = agg_pk.to_public_key();

    // DST (domain separation tag) for Ethereum BLS signatures
    let dst = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";

    // Verify the signature
    let result = sig.verify(false, message, dst, &[], &agg_pk_final, false);
    if result != BLST_ERROR::BLST_SUCCESS {
        return Err(VerificationError::InvalidSignature);
    }

    Ok(())
}

/// Verify a Merkle branch (SSZ proof) against an expected root.
/// Used to verify finality proofs and sync committee proofs within beacon state.
pub fn verify_merkle_branch(
    leaf: &[u8; 32],
    branch: &[[u8; 32]],
    depth: usize,
    index: u64,
    root: &[u8; 32],
) -> bool {
    if branch.len() != depth {
        return false;
    }

    let mut current = *leaf;
    for (i, node) in branch.iter().enumerate() {
        if (index >> i) & 1 == 1 {
            current = sha256_pair(node, &current);
        } else {
            current = sha256_pair(&current, node);
        }
    }

    current == *root
}

// --- Helper functions ---

/// SHA256 hash of arbitrary data.
fn sha256_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    output
}

/// SHA256 hash of two 32-byte values concatenated.
fn sha256_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(a);
    data[32..].copy_from_slice(b);
    sha256_hash(&data)
}

/// Encode a u64 as a 32-byte SSZ leaf (little-endian, zero-padded).
fn uint64_to_leaf(value: u64) -> [u8; 32] {
    let mut leaf = [0u8; 32];
    leaf[..8].copy_from_slice(&value.to_le_bytes());
    leaf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uint64_to_leaf() {
        let leaf = uint64_to_leaf(42);
        assert_eq!(leaf[0], 42);
        assert_eq!(leaf[1..8], [0; 7]);
        assert_eq!(leaf[8..32], [0; 24]);
    }

    #[test]
    fn test_sha256_pair_deterministic() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let h1 = sha256_pair(&a, &b);
        let h2 = sha256_pair(&a, &b);
        assert_eq!(h1, h2);

        // Order matters
        let h3 = sha256_pair(&b, &a);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_compute_domain() {
        let domain = compute_domain(
            &DOMAIN_SYNC_COMMITTEE,
            &[0x04, 0x00, 0x00, 0x00], // Deneb fork version
            &[0xaa; 32],
        );
        // Domain should start with the domain type
        assert_eq!(&domain[..4], &DOMAIN_SYNC_COMMITTEE);
        // And be deterministic
        let domain2 = compute_domain(
            &DOMAIN_SYNC_COMMITTEE,
            &[0x04, 0x00, 0x00, 0x00],
            &[0xaa; 32],
        );
        assert_eq!(domain, domain2);
    }

    #[test]
    fn test_verify_merkle_branch_trivial() {
        // Single-depth branch: leaf with one sibling
        let leaf = sha256_hash(b"leaf");
        let sibling = sha256_hash(b"sibling");
        let root = sha256_pair(&leaf, &sibling);

        assert!(verify_merkle_branch(&leaf, &[sibling], 1, 0, &root));
        // Wrong index should fail
        assert!(!verify_merkle_branch(&leaf, &[sibling], 1, 1, &root));
    }

    #[test]
    fn test_sync_aggregate_participation() {
        let mut bits = vec![0u8; 64]; // 512 bits
        bits[0] = 0b11111111; // First 8 members participated
        bits[1] = 0b00000001; // 9th member

        let aggregate = SyncAggregate {
            sync_committee_bits: bits,
            sync_committee_signature: BlsSignature([0u8; 96]),
        };

        assert_eq!(aggregate.num_participants(), 9);
        assert!(aggregate.has_participant(0));
        assert!(aggregate.has_participant(7));
        assert!(aggregate.has_participant(8));
        assert!(!aggregate.has_participant(9));
    }

    #[test]
    fn test_insufficient_participation_rejected() {
        // Create an update with only 100 participants (less than 342 required)
        let mut bits = vec![0u8; 64];
        // Set first 100 bits
        for i in 0..12 {
            bits[i] = 0xFF; // 12 * 8 = 96 participants
        }
        bits[12] = 0x0F; // 4 more = 100

        let sync_aggregate = SyncAggregate {
            sync_committee_bits: bits,
            sync_committee_signature: BlsSignature([0u8; 96]),
        };

        assert_eq!(sync_aggregate.num_participants(), 100);

        let update = LightClientUpdate {
            attested_header: BeaconBlockHeader {
                slot: 100,
                proposer_index: 1,
                parent_root: [0; 32],
                state_root: [0; 32],
                body_root: [0; 32],
            },
            next_sync_committee: None,
            next_sync_committee_branch: vec![],
            finalized_header: BeaconBlockHeader {
                slot: 90,
                proposer_index: 1,
                parent_root: [0; 32],
                state_root: [0; 32],
                body_root: [0; 32],
            },
            finality_branch: vec![],
            sync_aggregate,
            signature_slot: 101,
        };

        let committee = SyncCommittee {
            pubkeys: vec![BlsPublicKey([0u8; 48]); 512],
            aggregate_pubkey: BlsPublicKey([0u8; 48]),
        };

        let result = verify_sync_committee_signature(
            &update,
            &committee,
            [0; 32],
            [0x04, 0x00, 0x00, 0x00],
        );

        assert!(matches!(
            result,
            Err(VerificationError::InsufficientParticipation { .. })
        ));
    }
}
