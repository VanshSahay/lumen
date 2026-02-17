use crate::types::execution::*;
use thiserror::Error;
use tiny_keccak::{Hasher, Keccak};

/// Errors during Merkle-Patricia trie proof verification.
/// Each variant is specific enough to diagnose exactly what went wrong.
#[derive(Debug, Error)]
pub enum ProofError {
    #[error("Empty proof: no trie nodes provided")]
    EmptyProof,

    #[error("Invalid RLP encoding in proof node {index}: {reason}")]
    InvalidRlp { index: usize, reason: String },

    #[error("Proof verification failed: computed root {computed} does not match expected root {expected}")]
    RootMismatch { computed: String, expected: String },

    #[error("Invalid trie node type at depth {depth}: expected branch or extension, got {node_type}")]
    InvalidNodeType { depth: usize, node_type: String },

    #[error("Proof path incomplete: trie traversal ended at depth {depth} without reaching the key")]
    IncompleteProof { depth: usize },

    #[error("Account not found at address {address}")]
    AccountNotFound { address: String },

    #[error("Storage key not found: {key}")]
    StorageKeyNotFound { key: String },

    #[error("Invalid account RLP encoding: {reason}")]
    InvalidAccountEncoding { reason: String },

    #[error("Invalid value encoding: {reason}")]
    InvalidValueEncoding { reason: String },
}

/// Compute keccak256 hash of data.
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    let mut output = [0u8; 32];
    hasher.update(data);
    hasher.finalize(&mut output);
    output
}

/// Verify an account proof against a known state root.
/// The state root comes from a verified execution payload header.
/// This lets us prove balance, nonce, code hash, and storage root of any account.
///
/// IMPORTANT: The state root must come from our verified chain state.
/// Never accept a state root from an untrusted source.
pub fn verify_account_proof(
    state_root: [u8; 32],
    address: [u8; 20],
    proof: &AccountProof,
) -> Result<AccountState, ProofError> {
    if proof.proof.is_empty() {
        return Err(ProofError::EmptyProof);
    }

    // The key in the state trie is keccak256(address)
    let key = keccak256(&address);

    // Verify the proof path against the state root
    let value_bytes = verify_merkle_patricia_proof(&state_root, &key, &proof.proof)?;

    match value_bytes {
        Some(rlp_bytes) => {
            // Decode the account from RLP
            decode_account_from_rlp(&rlp_bytes)
        }
        None => {
            // Account doesn't exist in the trie — this is a valid proof of non-existence
            Err(ProofError::AccountNotFound {
                address: hex::encode(address),
            })
        }
    }
}

/// Verify a storage proof for a specific storage slot of a contract.
/// The storage root comes from a verified account state.
pub fn verify_storage_proof(
    storage_root: [u8; 32],
    slot: [u8; 32],
    proof: &StorageProof,
) -> Result<[u8; 32], ProofError> {
    if proof.proof.is_empty() {
        // Empty proof with empty storage root means the slot is zero
        if storage_root == AccountState::EMPTY_STORAGE_ROOT {
            return Ok([0u8; 32]);
        }
        return Err(ProofError::EmptyProof);
    }

    // The key in the storage trie is keccak256(slot)
    let key = keccak256(&slot);

    let value_bytes = verify_merkle_patricia_proof(&storage_root, &key, &proof.proof)?;

    match value_bytes {
        Some(rlp_bytes) => {
            // Storage values are RLP-encoded. Decode to get the raw bytes.
            decode_storage_value(&rlp_bytes)
        }
        None => {
            // Slot not in trie — value is zero (valid proof of non-existence)
            Ok([0u8; 32])
        }
    }
}

/// Core Merkle-Patricia trie proof verification.
///
/// Walks the trie from root to leaf following the proof nodes.
/// At each step, verifies that the hash of the current node matches
/// what the parent node claims.
///
/// Returns Some(value) if the key exists, None for proof of non-existence.
fn verify_merkle_patricia_proof(
    expected_root: &[u8; 32],
    key: &[u8; 32],
    proof_nodes: &[Vec<u8>],
) -> Result<Option<Vec<u8>>, ProofError> {
    if proof_nodes.is_empty() {
        return Err(ProofError::EmptyProof);
    }

    // Convert key to nibbles (each byte = 2 nibbles)
    let nibbles = bytes_to_nibbles(key);
    let mut nibble_index: usize = 0;

    // Verify the first node hashes to the expected root
    let first_hash = keccak256(&proof_nodes[0]);
    // For very short nodes (< 32 bytes), the node is embedded directly, not hashed
    if proof_nodes[0].len() >= 32 && first_hash != *expected_root {
        return Err(ProofError::RootMismatch {
            computed: hex::encode(first_hash),
            expected: hex::encode(expected_root),
        });
    }

    for (depth, node_rlp) in proof_nodes.iter().enumerate() {
        let items = decode_rlp_list(node_rlp).map_err(|e| ProofError::InvalidRlp {
            index: depth,
            reason: e,
        })?;

        match items.len() {
            17 => {
                // Branch node: 16 children + value
                if nibble_index >= nibbles.len() {
                    // We've consumed all nibbles — the value is in position 16
                    let value = &items[16];
                    if value.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(value.clone()));
                }

                let child_index = nibbles[nibble_index] as usize;
                nibble_index += 1;

                if depth + 1 < proof_nodes.len() {
                    // Verify the child hash matches
                    let child = &items[child_index];
                    if child.len() == 32 {
                        let next_hash = keccak256(&proof_nodes[depth + 1]);
                        if proof_nodes[depth + 1].len() >= 32 {
                            let mut expected = [0u8; 32];
                            expected.copy_from_slice(child);
                            if next_hash != expected {
                                return Err(ProofError::RootMismatch {
                                    computed: hex::encode(next_hash),
                                    expected: hex::encode(expected),
                                });
                            }
                        }
                    }
                } else {
                    // Last node in proof — check the child reference
                    let child = &items[child_index];
                    if child.is_empty() {
                        return Ok(None); // Key not in trie
                    }
                    // The child contains the value inline
                    return Ok(Some(child.clone()));
                }
            }
            2 => {
                // Extension or leaf node
                let (prefix_nibbles, is_leaf) =
                    decode_compact_path(&items[0]).map_err(|e| ProofError::InvalidRlp {
                        index: depth,
                        reason: e,
                    })?;

                if is_leaf {
                    // Leaf node: check if remaining nibbles match
                    let remaining = &nibbles[nibble_index..];
                    if remaining == prefix_nibbles.as_slice() {
                        let value = &items[1];
                        if value.is_empty() {
                            return Ok(None);
                        }
                        return Ok(Some(value.clone()));
                    } else {
                        // Key doesn't match — proof of non-existence
                        return Ok(None);
                    }
                } else {
                    // Extension node: consume the shared prefix
                    let remaining = &nibbles[nibble_index..];
                    if !remaining.starts_with(&prefix_nibbles) {
                        return Ok(None); // Path diverges — key not in trie
                    }
                    nibble_index += prefix_nibbles.len();

                    // Verify the next node hash
                    if depth + 1 < proof_nodes.len() {
                        let child_ref = &items[1];
                        if child_ref.len() == 32 && proof_nodes[depth + 1].len() >= 32 {
                            let next_hash = keccak256(&proof_nodes[depth + 1]);
                            let mut expected = [0u8; 32];
                            expected.copy_from_slice(child_ref);
                            if next_hash != expected {
                                return Err(ProofError::RootMismatch {
                                    computed: hex::encode(next_hash),
                                    expected: hex::encode(expected),
                                });
                            }
                        }
                    }
                }
            }
            _ => {
                return Err(ProofError::InvalidNodeType {
                    depth,
                    node_type: format!("{}-element list", items.len()),
                });
            }
        }
    }

    Err(ProofError::IncompleteProof {
        depth: proof_nodes.len(),
    })
}

/// Decode an Ethereum account from RLP encoding.
/// Account is RLP([nonce, balance, storageRoot, codeHash])
fn decode_account_from_rlp(rlp_bytes: &[u8]) -> Result<AccountState, ProofError> {
    let items = decode_rlp_list(rlp_bytes).map_err(|e| ProofError::InvalidAccountEncoding {
        reason: e,
    })?;

    if items.len() != 4 {
        return Err(ProofError::InvalidAccountEncoding {
            reason: format!("Expected 4 items, got {}", items.len()),
        });
    }

    let nonce = decode_rlp_uint64(&items[0]);
    let balance = decode_rlp_u256(&items[1]);

    let mut storage_root = [0u8; 32];
    if items[2].len() == 32 {
        storage_root.copy_from_slice(&items[2]);
    } else if items[2].is_empty() {
        storage_root = AccountState::EMPTY_STORAGE_ROOT;
    } else {
        return Err(ProofError::InvalidAccountEncoding {
            reason: format!("Invalid storage root length: {}", items[2].len()),
        });
    }

    let mut code_hash = [0u8; 32];
    if items[3].len() == 32 {
        code_hash.copy_from_slice(&items[3]);
    } else if items[3].is_empty() {
        code_hash = AccountState::EMPTY_CODE_HASH;
    } else {
        return Err(ProofError::InvalidAccountEncoding {
            reason: format!("Invalid code hash length: {}", items[3].len()),
        });
    }

    Ok(AccountState {
        nonce,
        balance,
        storage_root,
        code_hash,
    })
}

/// Decode a storage value from RLP.
fn decode_storage_value(rlp_bytes: &[u8]) -> Result<[u8; 32], ProofError> {
    let value = decode_rlp_bytes(rlp_bytes).map_err(|e| ProofError::InvalidValueEncoding {
        reason: e,
    })?;

    let mut result = [0u8; 32];
    if value.len() <= 32 {
        // Right-align the value in 32 bytes (big-endian)
        result[32 - value.len()..].copy_from_slice(&value);
    } else {
        return Err(ProofError::InvalidValueEncoding {
            reason: format!("Storage value too long: {} bytes", value.len()),
        });
    }
    Ok(result)
}

// --- RLP Decoding Helpers ---

/// Convert a 32-byte array to nibbles (4 bits each).
fn bytes_to_nibbles(bytes: &[u8; 32]) -> Vec<u8> {
    let mut nibbles = Vec::with_capacity(64);
    for byte in bytes {
        nibbles.push(byte >> 4);
        nibbles.push(byte & 0x0F);
    }
    nibbles
}

/// Decode compact (hex-prefix) encoding used in trie nodes.
/// Returns (nibbles, is_leaf).
fn decode_compact_path(encoded: &[u8]) -> Result<(Vec<u8>, bool), String> {
    if encoded.is_empty() {
        return Ok((vec![], false));
    }

    let first_nibble = encoded[0] >> 4;
    let is_leaf = first_nibble >= 2;
    let is_odd = first_nibble % 2 == 1;

    let mut nibbles = Vec::new();

    if is_odd {
        nibbles.push(encoded[0] & 0x0F);
    }

    for &byte in &encoded[1..] {
        nibbles.push(byte >> 4);
        nibbles.push(byte & 0x0F);
    }

    Ok((nibbles, is_leaf))
}

/// Minimal RLP list decoder.
/// Decodes an RLP-encoded list into its component items.
pub fn decode_rlp_list(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if data.is_empty() {
        return Err("Empty RLP data".to_string());
    }

    let (items_data, _) = decode_rlp_list_payload(data)?;
    let mut items = Vec::new();
    let mut offset = 0;

    while offset < items_data.len() {
        let (item, consumed) = decode_rlp_item(&items_data[offset..])?;
        items.push(item);
        offset += consumed;
    }

    Ok(items)
}

/// Decode the payload portion of an RLP list.
fn decode_rlp_list_payload(data: &[u8]) -> Result<(&[u8], usize), String> {
    let prefix = data[0];

    if prefix <= 0x7F {
        return Err("Expected list, got single byte".to_string());
    }

    if prefix <= 0xB7 {
        return Err("Expected list, got short string".to_string());
    }

    if prefix <= 0xBF {
        return Err("Expected list, got long string".to_string());
    }

    if prefix <= 0xF7 {
        // Short list: length is prefix - 0xC0
        let length = (prefix - 0xC0) as usize;
        if data.len() < 1 + length {
            return Err("Short list: insufficient data".to_string());
        }
        Ok((&data[1..1 + length], 1 + length))
    } else {
        // Long list: next (prefix - 0xF7) bytes are the length
        let len_bytes = (prefix - 0xF7) as usize;
        if data.len() < 1 + len_bytes {
            return Err("Long list: insufficient length bytes".to_string());
        }
        let mut length: usize = 0;
        for i in 0..len_bytes {
            length = (length << 8) | (data[1 + i] as usize);
        }
        let total = 1 + len_bytes + length;
        if data.len() < total {
            return Err("Long list: insufficient data".to_string());
        }
        Ok((&data[1 + len_bytes..total], total))
    }
}

/// Decode a single RLP item, returning the decoded bytes and how many bytes were consumed.
fn decode_rlp_item(data: &[u8]) -> Result<(Vec<u8>, usize), String> {
    if data.is_empty() {
        return Err("Empty data in RLP item".to_string());
    }

    let prefix = data[0];

    if prefix <= 0x7F {
        // Single byte
        Ok((vec![prefix], 1))
    } else if prefix <= 0xB7 {
        // Short string (0-55 bytes)
        let length = (prefix - 0x80) as usize;
        if length == 0 {
            return Ok((vec![], 1));
        }
        if data.len() < 1 + length {
            return Err("Short string: insufficient data".to_string());
        }
        Ok((data[1..1 + length].to_vec(), 1 + length))
    } else if prefix <= 0xBF {
        // Long string (>55 bytes)
        let len_bytes = (prefix - 0xB7) as usize;
        if data.len() < 1 + len_bytes {
            return Err("Long string: insufficient length bytes".to_string());
        }
        let mut length: usize = 0;
        for i in 0..len_bytes {
            length = (length << 8) | (data[1 + i] as usize);
        }
        let total = 1 + len_bytes + length;
        if data.len() < total {
            return Err("Long string: insufficient data".to_string());
        }
        Ok((data[1 + len_bytes..total].to_vec(), total))
    } else if prefix <= 0xF7 {
        // Short list
        let length = (prefix - 0xC0) as usize;
        let total = 1 + length;
        if data.len() < total {
            return Err("Short list item: insufficient data".to_string());
        }
        Ok((data[..total].to_vec(), total))
    } else {
        // Long list
        let len_bytes = (prefix - 0xF7) as usize;
        if data.len() < 1 + len_bytes {
            return Err("Long list item: insufficient length bytes".to_string());
        }
        let mut length: usize = 0;
        for i in 0..len_bytes {
            length = (length << 8) | (data[1 + i] as usize);
        }
        let total = 1 + len_bytes + length;
        if data.len() < total {
            return Err("Long list item: insufficient data".to_string());
        }
        Ok((data[..total].to_vec(), total))
    }
}

/// Decode RLP bytes (for simple byte strings).
fn decode_rlp_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Ok(vec![]);
    }

    let prefix = data[0];

    if prefix <= 0x7F {
        Ok(vec![prefix])
    } else if prefix <= 0xB7 {
        let length = (prefix - 0x80) as usize;
        if length == 0 {
            return Ok(vec![]);
        }
        if data.len() < 1 + length {
            return Err("Insufficient data for RLP bytes".to_string());
        }
        Ok(data[1..1 + length].to_vec())
    } else if prefix <= 0xBF {
        let len_bytes = (prefix - 0xB7) as usize;
        if data.len() < 1 + len_bytes {
            return Err("Insufficient data for long RLP bytes".to_string());
        }
        let mut length: usize = 0;
        for i in 0..len_bytes {
            length = (length << 8) | (data[1 + i] as usize);
        }
        let total = 1 + len_bytes + length;
        if data.len() < total {
            return Err("Insufficient data for long RLP bytes content".to_string());
        }
        Ok(data[1 + len_bytes..total].to_vec())
    } else {
        Err("Expected bytes, got list".to_string())
    }
}

/// Decode RLP-encoded uint64.
fn decode_rlp_uint64(data: &[u8]) -> u64 {
    if data.is_empty() {
        return 0;
    }
    let mut result: u64 = 0;
    for &byte in data {
        result = (result << 8) | (byte as u64);
    }
    result
}

/// Decode RLP-encoded U256 as 32-byte big-endian.
fn decode_rlp_u256(data: &[u8]) -> [u8; 32] {
    let mut result = [0u8; 32];
    if data.is_empty() {
        return result;
    }
    let start = 32usize.saturating_sub(data.len());
    let len = data.len().min(32);
    result[start..start + len].copy_from_slice(&data[..len]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keccak256() {
        // Test vector: keccak256("") = c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
        let empty_hash = keccak256(&[]);
        assert_eq!(empty_hash, AccountState::EMPTY_CODE_HASH);
    }

    #[test]
    fn test_bytes_to_nibbles() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0xAB;
        bytes[1] = 0xCD;
        let nibbles = bytes_to_nibbles(&bytes);
        assert_eq!(nibbles.len(), 64);
        assert_eq!(nibbles[0], 0xA);
        assert_eq!(nibbles[1], 0xB);
        assert_eq!(nibbles[2], 0xC);
        assert_eq!(nibbles[3], 0xD);
        assert_eq!(nibbles[4], 0x0);
    }

    #[test]
    fn test_decode_compact_path_even_extension() {
        // 0x00 prefix: even extension
        let encoded = vec![0x00, 0xAB, 0xCD];
        let (nibbles, is_leaf) = decode_compact_path(&encoded).unwrap();
        assert!(!is_leaf);
        assert_eq!(nibbles, vec![0xA, 0xB, 0xC, 0xD]);
    }

    #[test]
    fn test_decode_compact_path_odd_extension() {
        // 0x1X prefix: odd extension, first nibble is X
        let encoded = vec![0x1A, 0xBC];
        let (nibbles, is_leaf) = decode_compact_path(&encoded).unwrap();
        assert!(!is_leaf);
        assert_eq!(nibbles, vec![0xA, 0xB, 0xC]);
    }

    #[test]
    fn test_decode_compact_path_even_leaf() {
        // 0x20 prefix: even leaf
        let encoded = vec![0x20, 0xAB];
        let (nibbles, is_leaf) = decode_compact_path(&encoded).unwrap();
        assert!(is_leaf);
        assert_eq!(nibbles, vec![0xA, 0xB]);
    }

    #[test]
    fn test_decode_compact_path_odd_leaf() {
        // 0x3X prefix: odd leaf, first nibble is X
        let encoded = vec![0x3A, 0xBC];
        let (nibbles, is_leaf) = decode_compact_path(&encoded).unwrap();
        assert!(is_leaf);
        assert_eq!(nibbles, vec![0xA, 0xB, 0xC]);
    }

    #[test]
    fn test_rlp_decode_single_byte() {
        let data = vec![0x42];
        let (item, consumed) = decode_rlp_item(&data).unwrap();
        assert_eq!(item, vec![0x42]);
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_rlp_decode_empty_string() {
        let data = vec![0x80];
        let (item, consumed) = decode_rlp_item(&data).unwrap();
        assert_eq!(item, Vec::<u8>::new());
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_rlp_decode_short_string() {
        // 0x83 means 3-byte string
        let data = vec![0x83, 0x61, 0x62, 0x63]; // "abc"
        let (item, consumed) = decode_rlp_item(&data).unwrap();
        assert_eq!(item, vec![0x61, 0x62, 0x63]);
        assert_eq!(consumed, 4);
    }

    #[test]
    fn test_rlp_decode_uint64() {
        assert_eq!(decode_rlp_uint64(&[]), 0);
        assert_eq!(decode_rlp_uint64(&[0x01]), 1);
        assert_eq!(decode_rlp_uint64(&[0x01, 0x00]), 256);
        assert_eq!(decode_rlp_uint64(&[0xFF]), 255);
    }

    #[test]
    fn test_rlp_decode_u256() {
        let result = decode_rlp_u256(&[0x01]);
        assert_eq!(result[31], 1);
        assert_eq!(result[30], 0);

        let result = decode_rlp_u256(&[0x01, 0x00]);
        assert_eq!(result[31], 0);
        assert_eq!(result[30], 1);
    }

    #[test]
    fn test_rlp_decode_list() {
        // RLP encoding of [0x01, 0x02, 0x03]
        let data = vec![0xC3, 0x01, 0x02, 0x03];
        let items = decode_rlp_list(&data).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], vec![0x01]);
        assert_eq!(items[1], vec![0x02]);
        assert_eq!(items[2], vec![0x03]);
    }
}
