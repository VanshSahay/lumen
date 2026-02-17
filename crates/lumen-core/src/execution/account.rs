use crate::execution::proof::{verify_account_proof, verify_storage_proof, ProofError};
use crate::types::execution::*;

/// Verify an account's full state including selected storage slots.
/// This is the high-level API that combines account proof and storage proof verification.
///
/// The state_root must come from a verified execution payload header in our light client state.
/// The proof data can come from any source (including untrusted RPCs) — we verify it
/// cryptographically regardless of source.
pub fn verify_full_account_state(
    state_root: [u8; 32],
    proof_response: &EthGetProofResponse,
) -> Result<VerifiedAccountState, ProofError> {
    // First, verify the account proof to get the account state
    let account = verify_account_proof(
        state_root,
        proof_response.account_proof.address,
        &proof_response.account_proof,
    )?;

    // Then verify each storage proof against the account's storage root
    let mut verified_storage: Vec<VerifiedStorageSlot> = Vec::new();

    for storage_proof in &proof_response.storage_proofs {
        let value = verify_storage_proof(account.storage_root, storage_proof.key, storage_proof)?;

        verified_storage.push(VerifiedStorageSlot {
            key: storage_proof.key,
            value,
        });
    }

    Ok(VerifiedAccountState {
        address: proof_response.account_proof.address,
        account,
        storage_slots: verified_storage,
    })
}

/// A fully verified account state with verified storage slots.
/// Every field in this struct has been cryptographically verified against
/// the beacon chain sync committee consensus.
#[derive(Clone, Debug)]
pub struct VerifiedAccountState {
    /// The Ethereum address.
    pub address: [u8; 20],
    /// The verified account state (nonce, balance, storage root, code hash).
    pub account: AccountState,
    /// Verified storage slot values.
    pub storage_slots: Vec<VerifiedStorageSlot>,
}

/// A single verified storage slot.
#[derive(Clone, Debug)]
pub struct VerifiedStorageSlot {
    /// The storage key (slot number).
    pub key: [u8; 32],
    /// The verified storage value.
    pub value: [u8; 32],
}

impl VerifiedAccountState {
    /// Get the balance as a hex string (without 0x prefix).
    pub fn balance_hex(&self) -> String {
        hex::encode(self.account.balance)
            .trim_start_matches('0')
            .to_string()
    }

    /// Check if this is a contract account.
    pub fn is_contract(&self) -> bool {
        self.account.is_contract()
    }

    /// Look up a verified storage slot value by key.
    pub fn get_storage(&self, key: &[u8; 32]) -> Option<&[u8; 32]> {
        self.storage_slots
            .iter()
            .find(|s| &s.key == key)
            .map(|s| &s.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verified_account_state_balance_hex() {
        let state = VerifiedAccountState {
            address: [0; 20],
            account: AccountState {
                nonce: 0,
                balance: {
                    let mut b = [0u8; 32];
                    b[31] = 100; // 100 wei
                    b
                },
                storage_root: AccountState::EMPTY_STORAGE_ROOT,
                code_hash: AccountState::EMPTY_CODE_HASH,
            },
            storage_slots: vec![],
        };

        assert_eq!(state.balance_hex(), "64"); // 100 in hex
        assert!(!state.is_contract());
    }

    #[test]
    fn test_get_storage_lookup() {
        let key1 = [0x01; 32];
        let key2 = [0x02; 32];
        let value1 = [0xAA; 32];

        let state = VerifiedAccountState {
            address: [0; 20],
            account: AccountState {
                nonce: 0,
                balance: [0; 32],
                storage_root: [0; 32],
                code_hash: AccountState::EMPTY_CODE_HASH,
            },
            storage_slots: vec![VerifiedStorageSlot {
                key: key1,
                value: value1,
            }],
        };

        assert_eq!(state.get_storage(&key1), Some(&value1));
        assert_eq!(state.get_storage(&key2), None);
    }
}
