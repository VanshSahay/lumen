use serde::{Deserialize, Serialize};

/// An Ethereum account as stored in the state trie.
/// Verified via Merkle-Patricia trie proofs against a known state root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountState {
    /// Number of transactions sent from this account.
    pub nonce: u64,
    /// Balance in wei (stored as big-endian bytes to avoid precision loss).
    pub balance: [u8; 32],
    /// Root hash of the account's storage trie.
    /// For externally owned accounts (EOAs), this is the empty trie root.
    pub storage_root: [u8; 32],
    /// Keccak256 hash of the account's code.
    /// For EOAs, this is the hash of the empty string.
    pub code_hash: [u8; 32],
}

impl AccountState {
    /// The keccak256 hash of empty bytes — the code hash for EOAs.
    pub const EMPTY_CODE_HASH: [u8; 32] = [
        0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7,
        0x03, 0xc0, 0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04,
        0x5d, 0x85, 0xa4, 0x70,
    ];

    /// The root of an empty Merkle-Patricia trie.
    pub const EMPTY_STORAGE_ROOT: [u8; 32] = [
        0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0,
        0xf8, 0x6e, 0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5,
        0xe3, 0x63, 0xb4, 0x21,
    ];

    /// Check if this is a contract account (has code deployed).
    pub fn is_contract(&self) -> bool {
        self.code_hash != Self::EMPTY_CODE_HASH
    }

    /// Check if this account has a non-empty storage trie.
    pub fn has_storage(&self) -> bool {
        self.storage_root != Self::EMPTY_STORAGE_ROOT
    }

    /// Get balance as a hex string (no 0x prefix, leading zeros stripped).
    pub fn balance_hex(&self) -> String {
        let hex_str = hex::encode(self.balance);
        let trimmed = hex_str.trim_start_matches('0');
        if trimmed.is_empty() {
            "0".to_string()
        } else {
            trimmed.to_string()
        }
    }
}

/// A Merkle-Patricia trie proof for an account.
/// Obtained from eth_getProof RPC call, but verified locally.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountProof {
    /// The address this proof is for.
    pub address: [u8; 20],
    /// RLP-encoded trie nodes forming the proof path.
    pub proof: Vec<Vec<u8>>,
    /// The account state (if the account exists).
    pub account: Option<AccountState>,
}

/// A Merkle-Patricia trie proof for a storage slot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageProof {
    /// The storage key (slot) this proof is for.
    pub key: [u8; 32],
    /// The storage value at this key.
    pub value: [u8; 32],
    /// RLP-encoded trie nodes forming the proof path.
    pub proof: Vec<Vec<u8>>,
}

/// A proof for a transaction receipt in the receipts trie.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReceiptProof {
    /// The transaction index in the block.
    pub tx_index: u64,
    /// RLP-encoded trie nodes forming the proof path.
    pub proof: Vec<Vec<u8>>,
}

/// A verified transaction receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionReceipt {
    /// Whether the transaction succeeded (1) or failed (0).
    pub status: u8,
    /// Cumulative gas used in the block up to and including this transaction.
    pub cumulative_gas_used: u64,
    /// Bloom filter for quick log searching (256 bytes, hex-encoded for serde).
    #[serde(with = "bloom_serde")]
    pub logs_bloom: [u8; 256],
    /// The logs emitted by this transaction.
    pub logs: Vec<Log>,
}

mod bloom_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bloom: &[u8; 256], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(bloom))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 256], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
        if bytes.len() != 256 {
            return Err(serde::de::Error::custom("bloom must be 256 bytes"));
        }
        let mut arr = [0u8; 256];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}

/// A log entry emitted by a smart contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Log {
    /// Address of the contract that emitted the log.
    pub address: [u8; 20],
    /// Indexed topics (up to 4, first is usually the event signature hash).
    pub topics: Vec<[u8; 32]>,
    /// Non-indexed data.
    pub data: Vec<u8>,
}

/// Full proof response from eth_getProof — contains account proof and storage proofs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EthGetProofResponse {
    /// The account proof.
    pub account_proof: AccountProof,
    /// Storage proofs for requested slots.
    pub storage_proofs: Vec<StorageProof>,
}
