//! In-memory verified chain state management.
//!
//! This module manages the verified chain state that the WASM client maintains.
//! All state transitions are verified cryptographically before being applied.

use lumen_core::types::beacon::*;
use lumen_core::types::execution::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cache of recently verified account states.
/// Keyed by (address, slot) to avoid re-verifying the same proof multiple times.
///
/// This cache is purely a performance optimization — every entry was verified
/// cryptographically before being cached, and the cache is invalidated whenever
/// the verified head advances.
pub struct VerifiedStateCache {
    /// Verified account states: address -> (slot, AccountState)
    accounts: HashMap<[u8; 20], (u64, AccountState)>,
    /// Verified storage values: (address, key) -> (slot, value)
    storage: HashMap<([u8; 20], [u8; 32]), (u64, [u8; 32])>,
    /// The slot these cached entries are verified against.
    verified_slot: u64,
}

impl VerifiedStateCache {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            storage: HashMap::new(),
            verified_slot: 0,
        }
    }

    /// Invalidate the cache when the verified head advances.
    /// We could be smarter here (only invalidate accounts that might have changed),
    /// but correctness over cleverness: just clear everything.
    pub fn on_head_advance(&mut self, new_slot: u64) {
        if new_slot > self.verified_slot {
            self.accounts.clear();
            self.storage.clear();
            self.verified_slot = new_slot;
        }
    }

    /// Cache a verified account state.
    pub fn cache_account(&mut self, address: [u8; 20], slot: u64, state: AccountState) {
        self.accounts.insert(address, (slot, state));
    }

    /// Look up a cached account state.
    /// Returns None if not cached or if the cache is stale.
    pub fn get_account(&self, address: &[u8; 20], current_slot: u64) -> Option<&AccountState> {
        self.accounts.get(address).and_then(|(slot, state)| {
            if *slot == current_slot {
                Some(state)
            } else {
                None
            }
        })
    }

    /// Cache a verified storage value.
    pub fn cache_storage(
        &mut self,
        address: [u8; 20],
        key: [u8; 32],
        slot: u64,
        value: [u8; 32],
    ) {
        self.storage.insert((address, key), (slot, value));
    }

    /// Look up a cached storage value.
    pub fn get_storage(
        &self,
        address: &[u8; 20],
        key: &[u8; 32],
        current_slot: u64,
    ) -> Option<&[u8; 32]> {
        self.storage
            .get(&(*address, *key))
            .and_then(|(slot, value)| {
                if *slot == current_slot {
                    Some(value)
                } else {
                    None
                }
            })
    }

    /// Get the number of cached entries (for diagnostics).
    pub fn size(&self) -> (usize, usize) {
        (self.accounts.len(), self.storage.len())
    }
}

/// Sync progress tracking for the TypeScript layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncProgress {
    /// Current verified head slot.
    pub head_slot: u64,
    /// Target slot we're syncing toward (from P2P peers).
    pub target_slot: Option<u64>,
    /// Whether we've completed initial sync.
    pub is_initial_sync_complete: bool,
    /// Number of updates successfully processed.
    pub updates_processed: u64,
    /// Number of updates rejected (failed verification).
    pub updates_rejected: u64,
    /// Number of proofs verified successfully.
    pub proofs_verified: u64,
    /// Number of proofs that failed verification.
    pub proofs_rejected: u64,
}

impl SyncProgress {
    pub fn new() -> Self {
        Self {
            head_slot: 0,
            target_slot: None,
            is_initial_sync_complete: false,
            updates_processed: 0,
            updates_rejected: 0,
            proofs_verified: 0,
            proofs_rejected: 0,
        }
    }

    /// Calculate sync percentage (0.0 - 1.0).
    pub fn sync_percentage(&self) -> f64 {
        match self.target_slot {
            Some(target) if target > 0 => {
                (self.head_slot as f64 / target as f64).min(1.0)
            }
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_invalidation() {
        let mut cache = VerifiedStateCache::new();
        let addr = [0xAA; 20];
        let state = AccountState {
            nonce: 1,
            balance: [0; 32],
            storage_root: AccountState::EMPTY_STORAGE_ROOT,
            code_hash: AccountState::EMPTY_CODE_HASH,
        };

        cache.cache_account(addr, 100, state.clone());
        assert!(cache.get_account(&addr, 100).is_some());

        // Cache should be stale for a different slot
        assert!(cache.get_account(&addr, 101).is_none());

        // Advance head — cache should be cleared
        cache.on_head_advance(101);
        assert!(cache.get_account(&addr, 100).is_none());
    }

    #[test]
    fn test_sync_progress() {
        let mut progress = SyncProgress::new();
        assert_eq!(progress.sync_percentage(), 0.0);

        progress.head_slot = 50;
        progress.target_slot = Some(100);
        assert_eq!(progress.sync_percentage(), 0.5);

        progress.head_slot = 100;
        assert_eq!(progress.sync_percentage(), 1.0);
    }
}
