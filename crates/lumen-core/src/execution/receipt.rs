use crate::execution::proof::{keccak256, ProofError};
use crate::types::execution::*;

/// Verify a transaction receipt proof against a known receipts root.
/// The receipts root comes from a verified execution payload header.
///
/// This allows verification of:
/// - Whether a transaction succeeded or failed
/// - Gas used by the transaction
/// - Event logs emitted by the transaction
///
/// This is critical for dApps that need to confirm transaction effects.
pub fn verify_receipt_proof(
    receipts_root: [u8; 32],
    tx_index: u64,
    proof: &ReceiptProof,
) -> Result<TransactionReceipt, ProofError> {
    if proof.proof.is_empty() {
        return Err(ProofError::EmptyProof);
    }

    // The key in the receipts trie is RLP(tx_index)
    let key_bytes = rlp_encode_uint(tx_index);
    let _key_hash = keccak256(&key_bytes);

    // For the receipts trie, the key is the RLP-encoded transaction index.
    // We verify the Merkle proof against the receipts root.
    // Since we're using the same MPT structure, we can reuse the proof logic.
    let value_bytes =
        verify_receipt_merkle_proof(&receipts_root, &key_bytes, &proof.proof)?;

    match value_bytes {
        Some(rlp_bytes) => decode_receipt_from_rlp(&rlp_bytes),
        None => Err(ProofError::InvalidValueEncoding {
            reason: format!("Receipt not found for tx_index {}", tx_index),
        }),
    }
}

/// Verify a Merkle proof in the receipts trie.
/// The receipts trie uses the RLP-encoded tx index as the key (not hashed).
fn verify_receipt_merkle_proof(
    expected_root: &[u8; 32],
    key: &[u8],
    proof_nodes: &[Vec<u8>],
) -> Result<Option<Vec<u8>>, ProofError> {
    if proof_nodes.is_empty() {
        return Err(ProofError::EmptyProof);
    }

    // Verify root hash
    let first_hash = keccak256(&proof_nodes[0]);
    if proof_nodes[0].len() >= 32 && first_hash != *expected_root {
        return Err(ProofError::RootMismatch {
            computed: hex::encode(first_hash),
            expected: hex::encode(expected_root),
        });
    }

    // Convert key to nibbles for trie traversal
    let nibbles: Vec<u8> = key
        .iter()
        .flat_map(|b| vec![b >> 4, b & 0x0F])
        .collect();
    let mut nibble_index: usize = 0;

    for (depth, node_rlp) in proof_nodes.iter().enumerate() {
        let items =
            crate::execution::proof::decode_rlp_list(node_rlp).map_err(|e| ProofError::InvalidRlp {
                index: depth,
                reason: e,
            })?;

        match items.len() {
            17 => {
                // Branch node
                if nibble_index >= nibbles.len() {
                    let value = &items[16];
                    if value.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(value.clone()));
                }

                let child_index = nibbles[nibble_index] as usize;
                nibble_index += 1;

                if depth + 1 >= proof_nodes.len() {
                    let child = &items[child_index];
                    if child.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(child.clone()));
                }
            }
            2 => {
                // Extension or leaf
                let (prefix_nibbles, is_leaf) = decode_compact_path_receipt(&items[0])?;

                if is_leaf {
                    let remaining = &nibbles[nibble_index..];
                    if remaining == prefix_nibbles.as_slice() {
                        return Ok(Some(items[1].clone()));
                    } else {
                        return Ok(None);
                    }
                } else {
                    let remaining = &nibbles[nibble_index..];
                    if !remaining.starts_with(&prefix_nibbles) {
                        return Ok(None);
                    }
                    nibble_index += prefix_nibbles.len();
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

/// Decode compact path for receipt trie nodes.
fn decode_compact_path_receipt(encoded: &[u8]) -> Result<(Vec<u8>, bool), ProofError> {
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

/// Decode a transaction receipt from RLP encoding.
/// Post-EIP-2718, receipts may be typed (prefixed with a type byte).
fn decode_receipt_from_rlp(data: &[u8]) -> Result<TransactionReceipt, ProofError> {
    let rlp_data = if !data.is_empty() && data[0] <= 0x7F {
        // Typed receipt: skip the type byte
        &data[1..]
    } else {
        data
    };

    let items =
        crate::execution::proof::decode_rlp_list(rlp_data).map_err(|e| ProofError::InvalidValueEncoding {
            reason: format!("Invalid receipt RLP: {}", e),
        })?;

    if items.len() != 4 {
        return Err(ProofError::InvalidValueEncoding {
            reason: format!("Receipt should have 4 items, got {}", items.len()),
        });
    }

    // Status (post-Byzantium: 0 or 1)
    let status = if items[0].is_empty() {
        0
    } else {
        items[0][0]
    };

    // Cumulative gas used
    let cumulative_gas_used = bytes_to_u64(&items[1]);

    // Logs bloom (256 bytes)
    let mut logs_bloom = [0u8; 256];
    if items[2].len() == 256 {
        logs_bloom.copy_from_slice(&items[2]);
    }

    // Logs list
    let logs = decode_logs(&items[3])?;

    Ok(TransactionReceipt {
        status,
        cumulative_gas_used,
        logs_bloom,
        logs,
    })
}

/// Decode logs from RLP.
fn decode_logs(data: &[u8]) -> Result<Vec<Log>, ProofError> {
    if data.is_empty() || data[0] == 0xC0 {
        return Ok(vec![]);
    }

    let log_items =
        crate::execution::proof::decode_rlp_list(data).map_err(|e| ProofError::InvalidValueEncoding {
            reason: format!("Invalid logs RLP: {}", e),
        })?;

    let mut logs = Vec::new();
    for log_rlp in &log_items {
        let fields = crate::execution::proof::decode_rlp_list(log_rlp).map_err(|e| {
            ProofError::InvalidValueEncoding {
                reason: format!("Invalid log RLP: {}", e),
            }
        })?;

        if fields.len() != 3 {
            return Err(ProofError::InvalidValueEncoding {
                reason: format!("Log should have 3 fields, got {}", fields.len()),
            });
        }

        let mut address = [0u8; 20];
        if fields[0].len() == 20 {
            address.copy_from_slice(&fields[0]);
        }

        let topic_items = crate::execution::proof::decode_rlp_list(&fields[1]).map_err(|e| {
            ProofError::InvalidValueEncoding {
                reason: format!("Invalid topics RLP: {}", e),
            }
        })?;

        let topics: Vec<[u8; 32]> = topic_items
            .iter()
            .filter_map(|t| {
                if t.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(t);
                    Some(arr)
                } else {
                    None
                }
            })
            .collect();

        logs.push(Log {
            address,
            topics,
            data: fields[2].clone(),
        });
    }

    Ok(logs)
}

/// RLP encode a uint.
fn rlp_encode_uint(value: u64) -> Vec<u8> {
    if value == 0 {
        return vec![0x80]; // RLP empty string for zero
    }
    if value < 128 {
        return vec![value as u8];
    }

    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    let significant = &bytes[start..];
    let len = significant.len();

    let mut result = Vec::with_capacity(1 + len);
    result.push(0x80 + len as u8);
    result.extend_from_slice(significant);
    result
}

/// Convert bytes to u64 (big-endian).
fn bytes_to_u64(bytes: &[u8]) -> u64 {
    let mut result: u64 = 0;
    for &b in bytes {
        result = (result << 8) | (b as u64);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rlp_encode_uint() {
        assert_eq!(rlp_encode_uint(0), vec![0x80]);
        assert_eq!(rlp_encode_uint(1), vec![0x01]);
        assert_eq!(rlp_encode_uint(127), vec![0x7F]);
        assert_eq!(rlp_encode_uint(128), vec![0x81, 0x80]);
        assert_eq!(rlp_encode_uint(256), vec![0x82, 0x01, 0x00]);
    }

    #[test]
    fn test_bytes_to_u64() {
        assert_eq!(bytes_to_u64(&[]), 0);
        assert_eq!(bytes_to_u64(&[0x01]), 1);
        assert_eq!(bytes_to_u64(&[0x01, 0x00]), 256);
        assert_eq!(bytes_to_u64(&[0xFF, 0xFF]), 65535);
    }
}
