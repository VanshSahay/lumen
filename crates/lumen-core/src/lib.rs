//! # Lumen Core
//!
//! Pure Rust Ethereum light client verification logic.
//!
//! This crate contains **no networking code** and **no WASM dependencies**.
//! It is the cryptographic heart of Lumen — every piece of Ethereum data
//! passes through these verification functions before being trusted.
//!
//! ## Trust Model
//!
//! - **Sync committee verification** (`consensus` module): Verifies BLS12-381
//!   aggregate signatures from Ethereum's 512-member sync committee. Trusts
//!   that 2/3+ of the committee is honest (same assumption as Ethereum itself).
//!
//! - **Execution proof verification** (`execution` module): Verifies
//!   Merkle-Patricia trie proofs for account state, storage, and receipts.
//!   Zero trust assumptions beyond the verified state root.
//!
//! ## Usage
//!
//! ```ignore
//! use lumen_core::consensus::{initialize_from_bootstrap, process_light_client_update};
//! use lumen_core::execution::proof::verify_account_proof;
//! ```

pub mod consensus;
pub mod execution;
pub mod types;

// Re-export commonly used types for convenience
pub use consensus::{
    checkpoint::{verify_checkpoint_consensus, CheckpointError, VerifiedCheckpoint},
    light_client::{initialize_from_bootstrap, process_light_client_update},
    sync_committee::{verify_sync_committee_signature, VerificationError},
};
pub use execution::{
    account::{verify_full_account_state, VerifiedAccountState},
    proof::{keccak256, verify_account_proof, verify_storage_proof, ProofError},
    receipt::verify_receipt_proof,
};
pub use types::{beacon::*, execution::*};
