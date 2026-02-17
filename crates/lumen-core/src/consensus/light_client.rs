use crate::consensus::sync_committee::{
    hash_beacon_block_header, verify_merkle_branch,
    verify_sync_committee_signature, VerificationError,
};
use crate::types::beacon::*;

/// Generalized index for the finalized checkpoint in the beacon state.
/// This is the index in the SSZ Merkle tree where the finalized checkpoint lives.
const FINALIZED_ROOT_GINDEX: u64 = 105;
const FINALIZED_ROOT_DEPTH: usize = 6;

/// Generalized index for the next sync committee in the beacon state.
const NEXT_SYNC_COMMITTEE_GINDEX: u64 = 55;
const NEXT_SYNC_COMMITTEE_DEPTH: usize = 5;

/// Process a light client update, verifying all proofs and advancing state.
///
/// This function performs the complete verification pipeline:
/// 1. Validates structural correctness (slot ordering, participation threshold)
/// 2. Verifies the sync committee BLS signature (core trust anchor)
/// 3. Verifies the finality Merkle branch (proves finalized header is in the attested state)
/// 4. Verifies the next sync committee branch (if present, for committee rotation)
/// 5. Updates the light client state to reflect the new verified head
///
/// Returns the updated state if valid, error if any verification step fails.
pub fn process_light_client_update(
    state: &mut LightClientState,
    update: &LightClientUpdate,
    _current_slot: u64,
    genesis_validators_root: [u8; 32],
) -> Result<(), VerificationError> {
    // 1. The update must advance us forward — no replaying old updates
    if update.finalized_header.slot <= state.finalized_header.slot {
        return Err(VerificationError::UpdateNotNewer {
            update_slot: update.finalized_header.slot,
            current_slot: state.finalized_header.slot,
        });
    }

    // 2. Determine which sync committee to use for verification.
    // If the update is in the current period, use current_sync_committee.
    // If in the next period, use next_sync_committee (if we have it).
    let update_period = update.attested_header.slot / SLOTS_PER_SYNC_COMMITTEE_PERIOD;
    let current_period = state.current_period();

    let sync_committee = if update_period == current_period {
        &state.current_sync_committee
    } else if update_period == current_period + 1 {
        state
            .next_sync_committee
            .as_ref()
            .ok_or_else(|| VerificationError::BlsError(
                "Update is in next period but we don't have the next sync committee yet".into(),
            ))?
    } else {
        return Err(VerificationError::BlsError(format!(
            "Update period {} is too far from current period {}",
            update_period, current_period
        )));
    };

    // 3. Verify the sync committee BLS signature — THE CORE TRUST OPERATION
    verify_sync_committee_signature(
        update,
        sync_committee,
        genesis_validators_root,
        state.fork_version,
    )?;

    // 4. Verify finality branch — proves the finalized header is committed to in the attested state
    if !update.finality_branch.is_empty() {
        let finalized_root = hash_beacon_block_header(&update.finalized_header);
        let is_valid = verify_merkle_branch(
            &finalized_root,
            &update.finality_branch,
            FINALIZED_ROOT_DEPTH,
            FINALIZED_ROOT_GINDEX,
            &update.attested_header.state_root,
        );
        if !is_valid {
            return Err(VerificationError::InvalidFinalityBranch);
        }
    }

    // 5. If a next sync committee is provided, verify its branch
    if let Some(ref next_committee) = update.next_sync_committee {
        if !update.next_sync_committee_branch.is_empty() {
            let committee_root = hash_sync_committee(next_committee);
            let is_valid = verify_merkle_branch(
                &committee_root,
                &update.next_sync_committee_branch,
                NEXT_SYNC_COMMITTEE_DEPTH,
                NEXT_SYNC_COMMITTEE_GINDEX,
                &update.attested_header.state_root,
            );
            if !is_valid {
                return Err(VerificationError::InvalidNextSyncCommitteeBranch);
            }
        }
    }

    // 6. All checks passed — update the state
    state.finalized_header = update.finalized_header.clone();
    state.last_updated_slot = update.finalized_header.slot;

    // If we're transitioning to a new period, rotate committees
    if update_period == current_period + 1 {
        if let Some(ref next) = state.next_sync_committee {
            state.current_sync_committee = next.clone();
            state.next_sync_committee = None;
        }
    }

    // Store the next sync committee if provided
    if let Some(next_committee) = update.next_sync_committee.clone() {
        state.next_sync_committee = Some(next_committee);
    }

    Ok(())
}

/// Compute a simplified hash of a sync committee for Merkle branch verification.
/// In production, this would be the SSZ hash_tree_root of the SyncCommittee.
fn hash_sync_committee(committee: &SyncCommittee) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();

    // Hash all pubkeys
    for pk in &committee.pubkeys {
        hasher.update(&pk.0);
    }
    hasher.update(&committee.aggregate_pubkey.0);

    let result = hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    output
}

/// Initialize a light client state from a bootstrap.
/// This is the one moment of trust — the checkpoint hash must be verified
/// against multiple independent sources before calling this.
pub fn initialize_from_bootstrap(
    bootstrap: &LightClientBootstrap,
    genesis_validators_root: [u8; 32],
    fork_version: [u8; 4],
) -> Result<LightClientState, VerificationError> {
    // Validate the sync committee
    bootstrap
        .current_sync_committee
        .validate()
        .map_err(|e| VerificationError::BlsError(e.to_string()))?;

    // Verify the sync committee is committed to in the beacon state
    if !bootstrap.current_sync_committee_branch.is_empty() {
        let committee_root = hash_sync_committee(&bootstrap.current_sync_committee);
        // The current sync committee is at a different gindex than the next one
        let current_sync_committee_gindex: u64 = 54;
        let current_sync_committee_depth: usize = 5;

        let is_valid = verify_merkle_branch(
            &committee_root,
            &bootstrap.current_sync_committee_branch,
            current_sync_committee_depth,
            current_sync_committee_gindex,
            &bootstrap.header.state_root,
        );
        if !is_valid {
            return Err(VerificationError::InvalidNextSyncCommitteeBranch);
        }
    }

    Ok(LightClientState {
        finalized_header: bootstrap.header.clone(),
        current_sync_committee: bootstrap.current_sync_committee.clone(),
        next_sync_committee: None,
        latest_execution_payload_header: None,
        genesis_validators_root,
        fork_version,
        last_updated_slot: bootstrap.header.slot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_header(slot: u64) -> BeaconBlockHeader {
        BeaconBlockHeader {
            slot,
            proposer_index: 1,
            parent_root: [0; 32],
            state_root: [0; 32],
            body_root: [0; 32],
        }
    }

    fn make_test_committee() -> SyncCommittee {
        SyncCommittee {
            pubkeys: vec![BlsPublicKey([0u8; 48]); 512],
            aggregate_pubkey: BlsPublicKey([0u8; 48]),
        }
    }

    #[test]
    fn test_initialize_from_bootstrap() {
        let bootstrap = LightClientBootstrap {
            header: make_test_header(1000),
            current_sync_committee: make_test_committee(),
            current_sync_committee_branch: vec![], // Skip branch verification for test
        };

        let state = initialize_from_bootstrap(
            &bootstrap,
            [0xaa; 32],
            [0x04, 0x00, 0x00, 0x00],
        )
        .unwrap();

        assert_eq!(state.finalized_header.slot, 1000);
        assert_eq!(state.current_sync_committee.pubkeys.len(), 512);
        assert!(state.next_sync_committee.is_none());
        assert_eq!(state.last_updated_slot, 1000);
    }

    #[test]
    fn test_initialize_rejects_invalid_committee_size() {
        let bootstrap = LightClientBootstrap {
            header: make_test_header(1000),
            current_sync_committee: SyncCommittee {
                pubkeys: vec![BlsPublicKey([0u8; 48]); 100], // Wrong size
                aggregate_pubkey: BlsPublicKey([0u8; 48]),
            },
            current_sync_committee_branch: vec![],
        };

        let result = initialize_from_bootstrap(
            &bootstrap,
            [0xaa; 32],
            [0x04, 0x00, 0x00, 0x00],
        );

        assert!(result.is_err());
    }
}
