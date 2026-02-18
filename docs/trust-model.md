# Lumen Trust Model

This document is intentionally blunt. For every component, we answer:
1. What does it trust?
2. What's the attack scenario?
3. What's the current status?

If you find something evasive, file an issue. Honesty about trust is the entire point.

---

## Component: BLS12-381 Signature Verification

**What it trusts:** That the Discrete Logarithm Problem on the BLS12-381 curve is hard.

**Status:** **Active.** BLS12-381 aggregate signature verification runs in Rust/WASM via the `blst` crate. Every finality update from a beacon API is BLS-verified before the state root is accepted. The beacon API is untrusted data transport.

**Attack:** A breakthrough in discrete logarithm algorithms allows forging BLS signatures. An attacker creates fake sync committee signatures for arbitrary block headers.

**Probability:** Extremely low. BLS12-381 has a 128-bit security level. No known attacks reduce this meaningfully. A large quantum computer could break it, but such hardware doesn't exist.

**Impact:** Total — attacker could make Lumen accept any block header.

---

## Component: Sync Committee (2/3 Honest Assumption)

**What it trusts:** That at least 342 of the 512 sync committee members are honest during their ~27-hour duty period.

**Status:** **Active.** The demo BLS-verifies the aggregate signature and reports the exact participation count (typically 500+/512). The `lumen-core` crate checks both the signature validity and the finality Merkle branch.

**Attack:** An attacker controls 171+ sync committee positions and signs a fake block header. Requires controlling ~33% of all staked ETH ($30B+).

**Probability:** Very low. This is the same assumption that secures Ethereum's consensus itself. If this breaks, all of Ethereum is compromised, not just Lumen.

**Impact:** Total — attacker could make Lumen accept a fake chain state.

---

## Component: Beacon APIs (Data Transport)

**What it trusts:** Nothing. Beacon APIs are untrusted data transport.

**Status:** **Active.** The demo fetches raw JSON from beacon APIs (ChainSafe Lodestar, PublicNode Beacon). The JSON is passed directly to the Rust/WASM `LumenClient.process_finality_update()`, which performs full BLS signature verification. A beacon API returning forged data would fail BLS verification and be rejected.

**Attack 1 (forged finality update):** The beacon API returns a fake finality update with wrong execution state root.

**Result:** BLS verification fails. The fake aggregate signature doesn't match the sync committee public keys. Rejected.

**Attack 2 (stale data):** The beacon API returns a valid but old finality update.

**Result:** Lumen's state doesn't advance (the update's slot is ≤ the current finalized slot). No harm done; the user simply sees a slightly older finalized slot. Background polling will eventually receive a newer update.

**Attack 3 (omission):** The beacon API refuses to serve data.

**Result:** Availability-only impact. Multiple APIs are tried as fallback.

---

## Component: Merkle-Patricia Trie Proofs (keccak256)

**What it trusts:** That keccak256 is collision-resistant.

**Status:** **Active.** Full Merkle-Patricia trie proof verification runs in Rust/WASM (`lumen-core::execution::proof`). Every hash in the chain from state root to account leaf is verified via keccak256. RLP decoding happens in Rust.

**Attack:** An attacker finds a keccak256 collision — two different account states producing the same hash.

**Probability:** Effectively zero. keccak256 has a 256-bit output with no known collision attacks below the birthday bound (~2^128 operations).

**Impact:** High — attacker could forge a valid-looking proof for an incorrect balance.

---

## Component: Execution RPCs (Data Transport)

**What it trusts:** Nothing. The execution RPC is untrusted data transport.

**Status:** **Active.** The demo fetches `eth_getProof` from public RPCs (PublicNode, LlamaRPC). Proof bytes are verified in Rust/WASM via keccak256.

**Practical constraint:** Free public RPCs are pruned nodes. They only serve `eth_getProof` at the `latest` block, not at specific historical blocks (returns "old data not available due to pruning"). Lumen fetches the proof at `latest` and also fetches the latest block header to get its state root. The proof is verified against the block's own state root, and a cross-check confirms the block extends the BLS-verified finalized chain.

**Attack 1 (forged proof):** The RPC returns fabricated proof bytes.

**Result:** keccak256 verification fails. `keccak256(forged_node) != expected_hash`. Rejected.

**Attack 2 (stale proof):** The RPC returns a valid proof from an old block.

**Result:** The proof verifies against the old block's state root (which Lumen obtained from the same RPC). The cross-check confirms the block extends the finalized chain. The balance shown is correct for that block — it may differ slightly from the absolute latest state if the account was very recently active.

**Attack 3 (omission):** The RPC refuses to serve proofs.

**Result:** Availability-only impact. Multiple RPC endpoints are tried.

---

## Component: Latest Block State Root

**What it trusts:** That the execution RPC's latest block header is from the canonical chain.

**Status:** The proof is verified against the state root from the latest block header, which comes from the same RPC as the proof. The BLS-verified finalized state root is used as a cross-check anchor (block number comparison), but the proof itself is verified against the latest block's state root.

**Why not verify against the BLS-verified state root directly?** Free pruned RPCs cannot serve `eth_getProof` at historical blocks. The BLS-verified finalized block is typically 100+ blocks behind `latest`, and pruned RPCs return errors for it.

**Gap:** A sophisticated RPC could serve a valid proof from a non-canonical fork. The cross-check (latest block number ≥ finalized block number) catches simple cases but doesn't cryptographically bind the latest block to the finalized chain.

**Mitigation path:** When running against an archive node or using the Portal Network, the proof can be fetched at the exact BLS-verified block number, closing this gap entirely. The WASM module supports both flows (`verify_account_rpc_proof` for internal state root, `verify_account_rpc_proof_with_root` for explicit state root).

---

## Component: Fallback RPC (eth_call only)

**What it trusts:** The RPC's EVM execution results. This is the ONE exception.

**Status:** `eth_call` is forwarded to a fallback RPC. The result is marked as **unverified**. Console warning logged every time.

**Attack:** The RPC returns wrong `eth_call` results. Trivially exploitable.

**Mitigation path:** zk-EVM prover in the browser. Technically feasible (SP1, RISC Zero) but not yet practical for browser deployment.

---

## Component: WASM Runtime

**What it trusts:** That the browser's WASM runtime correctly executes the binary.

**Probability of failure:** Very low. V8, SpiderMonkey, and JavaScriptCore are among the most tested software in the world.

**Impact:** Total — incorrect execution means incorrect verification results.

---

## Component: lumen-p2p (Not Yet Integrated)

**Status:** The `lumen-p2p` Rust crate contains libp2p transport configuration (WebRTC, WebTransport), GossipSub behaviour for beacon chain topics, and peer scoring logic. It is **not compiled to WASM and not used in the demo**. The demo uses HTTP polling of beacon REST APIs as its data transport.

**When integrated:** P2P gossip would deliver finality updates directly from the Ethereum network without relying on beacon API providers. The verification pipeline is unchanged — raw bytes get BLS-verified regardless of transport.

---

## Summary

| Component | Trusts | Status | Attack Probability | Impact |
|-----------|--------|--------|-------------------|--------|
| BLS12-381 | Crypto hardness | **Active in WASM** | Near-zero | Total |
| Sync committee | 2/3 honest validators | **Active — BLS verified** | Very low ($30B+) | Total |
| Beacon APIs | Nothing (data transport) | **Active — BLS verified locally** | N/A | None |
| Merkle proofs | keccak256 collision resistance | **Active in WASM** | Near-zero | High |
| Execution RPCs | Nothing (data transport) | **Active — keccak256 verified** | N/A | None |
| Latest block root | RPC serves canonical chain | Active with finality cross-check | Low | Medium |
| Fallback RPC | EVM execution (eth_call) | Documented exception | Trivial | App-dependent |
| WASM runtime | Browser correctness | Active | Very low | Total |
| lumen-p2p | — | **Not integrated** | — | — |
