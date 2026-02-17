use thiserror::Error;

/// Errors that can occur during checkpoint operations.
#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("Insufficient checkpoint source agreement: {agreeing}/{total} sources agree (need {required})")]
    InsufficientAgreement {
        agreeing: usize,
        total: usize,
        required: usize,
    },

    #[error("No checkpoint sources available")]
    NoSources,

    #[error("Checkpoint hash format invalid: {reason}")]
    InvalidFormat { reason: String },

    #[error("Network error fetching checkpoint: {reason}")]
    NetworkError { reason: String },
}

/// A verified checkpoint — the starting point for light client sync.
/// This is the one moment of "soft trust" in Lumen's lifecycle.
/// Once past this point, all verification is purely cryptographic.
#[derive(Clone, Debug)]
pub struct VerifiedCheckpoint {
    /// The block root hash that multiple sources agreed on.
    pub block_root: [u8; 32],
    /// How many sources agreed on this checkpoint.
    pub source_agreement: usize,
    /// Total number of sources consulted.
    pub total_sources: usize,
    /// The slot this checkpoint corresponds to.
    pub slot: u64,
}

/// Verify that multiple checkpoint sources agree on the same block root.
/// This is the only "social consensus" step in Lumen — we trust that
/// N independent operators won't all collude to give us a fake checkpoint.
///
/// After this point, all verification is purely mathematical.
pub fn verify_checkpoint_consensus(
    checkpoint_hashes: &[([u8; 32], u64)], // (block_root, slot) from each source
    required_agreement: usize,
) -> Result<VerifiedCheckpoint, CheckpointError> {
    if checkpoint_hashes.is_empty() {
        return Err(CheckpointError::NoSources);
    }

    if required_agreement == 0 {
        return Err(CheckpointError::InsufficientAgreement {
            agreeing: 0,
            total: checkpoint_hashes.len(),
            required: required_agreement,
        });
    }

    // Count how many sources agree on each block root
    let mut agreement_counts: Vec<(([u8; 32], u64), usize)> = Vec::new();

    for &(hash, slot) in checkpoint_hashes {
        let found = agreement_counts
            .iter_mut()
            .find(|((h, _), _)| *h == hash);
        match found {
            Some((_, count)) => *count += 1,
            None => agreement_counts.push(((hash, slot), 1)),
        }
    }

    // Find the hash with the most agreement
    let best = agreement_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .unwrap(); // Safe: we checked non-empty above

    let ((block_root, slot), agreeing) = *best;

    if agreeing < required_agreement {
        return Err(CheckpointError::InsufficientAgreement {
            agreeing,
            total: checkpoint_hashes.len(),
            required: required_agreement,
        });
    }

    Ok(VerifiedCheckpoint {
        block_root,
        source_agreement: agreeing,
        total_sources: checkpoint_hashes.len(),
        slot,
    })
}

/// Parse a hex-encoded checkpoint hash string.
pub fn parse_checkpoint_hash(hex_str: &str) -> Result<[u8; 32], CheckpointError> {
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);

    if hex_str.len() != 64 {
        return Err(CheckpointError::InvalidFormat {
            reason: format!("Expected 64 hex characters, got {}", hex_str.len()),
        });
    }

    let bytes = hex::decode(hex_str).map_err(|e| CheckpointError::InvalidFormat {
        reason: format!("Invalid hex: {}", e),
    })?;

    let mut result = [0u8; 32];
    result.copy_from_slice(&bytes);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_consensus_succeeds() {
        let hash_a = [0xAA; 32];
        let hash_b = [0xBB; 32];

        let sources = vec![
            (hash_a, 1000),
            (hash_a, 1000),
            (hash_a, 1000),
            (hash_b, 999),
        ];

        let result = verify_checkpoint_consensus(&sources, 3).unwrap();
        assert_eq!(result.block_root, hash_a);
        assert_eq!(result.source_agreement, 3);
        assert_eq!(result.total_sources, 4);
        assert_eq!(result.slot, 1000);
    }

    #[test]
    fn test_checkpoint_consensus_fails_insufficient() {
        let hash_a = [0xAA; 32];
        let hash_b = [0xBB; 32];

        let sources = vec![
            (hash_a, 1000),
            (hash_a, 1000),
            (hash_b, 999),
            (hash_b, 999),
        ];

        let result = verify_checkpoint_consensus(&sources, 3);
        assert!(matches!(
            result,
            Err(CheckpointError::InsufficientAgreement { agreeing: 2, .. })
        ));
    }

    #[test]
    fn test_checkpoint_consensus_fails_empty() {
        let result = verify_checkpoint_consensus(&[], 3);
        assert!(matches!(result, Err(CheckpointError::NoSources)));
    }

    #[test]
    fn test_parse_checkpoint_hash() {
        let hash = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let result = parse_checkpoint_hash(hash).unwrap();
        assert_eq!(result, [0xAA; 32]);
    }

    #[test]
    fn test_parse_checkpoint_hash_no_prefix() {
        let hash = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let result = parse_checkpoint_hash(hash).unwrap();
        assert_eq!(result, [0xBB; 32]);
    }

    #[test]
    fn test_parse_checkpoint_hash_invalid_length() {
        let result = parse_checkpoint_hash("0xaabb");
        assert!(matches!(
            result,
            Err(CheckpointError::InvalidFormat { .. })
        ));
    }
}
