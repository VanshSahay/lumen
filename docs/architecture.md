# Lumen Architecture

## Overview

Lumen is a trustless Ethereum light client that runs in the browser. All cryptographic verification — BLS12-381 signature verification and keccak256 Merkle-Patricia trie proofs — happens in Rust compiled to WebAssembly. External data sources (beacon APIs, execution RPCs) are untrusted data transport.

---

## Verification Pipeline

### Stage 1: BLS-Verified Finality (Rust/WASM)

The beacon chain finalizes a block every ~6.4 minutes (1 epoch). A **sync committee** of 512 randomly selected validators signs each block header with BLS12-381 signatures, which can be aggregated into a single verifiable signature.

Lumen's flow:

1. Fetch the **light client bootstrap** from a beacon API — this contains the current sync committee's 512 BLS public keys and the finalized header
2. Fetch a **finality update** — contains the latest finalized header, execution payload (state root + block number), sync aggregate (participation bits + BLS signature)
3. **BLS verification in Rust/WASM** (`lumen-core`):
   - Reconstruct the signing root from the attested header
   - Compute the signing domain (fork version + genesis validators root)
   - Aggregate the participating public keys (identified by the bitvector)
   - Verify the BLS12-381 aggregate signature via the `blst` crate
   - Verify the finality Merkle branch (attested header → finalized header)
4. Store the **BLS-verified execution state root** and block number

After this stage, the beacon API is no longer trusted. It delivered raw bytes; Lumen verified the cryptographic proof.

### Stage 2: Merkle Proof Fetch (Untrusted Data Transport)

With a BLS-verified finalized state root, Lumen needs a Merkle proof for a specific account. This is fetched from any execution RPC via `eth_getProof`.

The execution RPC is **completely untrusted**. It could return fabricated data — it doesn't matter. The proof is verified locally in Stage 3. Forging a valid proof requires finding a keccak256 collision (~2^128 operations — computationally infeasible).

**Practical constraint:** Free public RPCs (PublicNode, LlamaRPC) are pruned nodes that only serve `eth_getProof` at the `latest` block, not at specific historical blocks. The proof is fetched at `latest` and cross-checked: the latest block number must be ≥ the BLS-verified finalized block number, confirming it extends the canonical finalized chain.

### Stage 3: keccak256 MPT Verification (Rust/WASM)

The proof is verified entirely in Rust/WASM (`lumen-core`):

1. Compute `keccak256(address)` → 32-byte trie key
2. Start at the state root from the latest block header
3. For each node in the proof:
   - Verify `keccak256(rlp_encoded_node) == expected_hash`
   - Decode the RLP node (branch: 17 items, extension/leaf: 2 items)
   - Follow the path using the key nibbles
4. At the leaf, decode the RLP account: `[nonce, balance, storageRoot, codeHash]`
5. Cross-check the proof-extracted balance against the RPC's claimed balance

If any hash in the chain doesn't match, the proof is rejected.

---

## System Diagram

```
                    ┌─────────────────────────────────┐
                    │           Browser                │
                    │                                  │
                    │  ┌──────────────────────────┐   │
                    │  │    demo/main.ts (UI)      │   │
                    │  │    ~500 lines, no crypto   │   │
                    │  └──────────┬───────────────┘   │
                    │             │                    │
                    │  ┌──────────▼───────────────┐   │
                    │  │    demo/wasm.ts           │   │
                    │  │    ~130 lines typed bridge │   │
                    │  └──────────┬───────────────┘   │
                    │             │                    │
                    │  ┌──────────▼───────────────────────────────┐
                    │  │     Rust/WASM  (lumen-core + lumen-wasm)  │
                    │  │                                           │
                    │  │  BLS12-381 verification (blst crate)      │
                    │  │  keccak256 MPT proof verification         │
                    │  │  RLP decoding (account state)             │
                    │  │  SSZ types (beacon chain structures)      │
                    │  │  Beacon API JSON adapter                  │
                    │  │                                           │
                    │  │  Size: ~360 KB raw, ~115 KB gzipped       │
                    │  └────────┬────────────────┬────────────────┘
                    │           │                │                 │
                    └───────────┼────────────────┼─────────────────┘
                                │                │
                    ┌───────────▼──────┐  ┌──────▼──────────────┐
                    │  Beacon APIs      │  │  Execution RPCs      │
                    │  (data transport) │  │  (data transport)    │
                    │                   │  │                      │
                    │  ChainSafe        │  │  PublicNode           │
                    │  PublicNode Beacon │  │  LlamaRPC             │
                    └───────────────────┘  └──────────────────────┘
```

---

## Crate Structure

### `lumen-core` — Pure Rust Verification

No networking, no WASM dependencies. Pure verification logic.

| Module | Purpose |
|--------|---------|
| `consensus::light_client` | Sync committee BLS verification, finality branch verification, state advancement |
| `consensus::checkpoint` | Checkpoint hash parsing and validation |
| `execution::proof` | Merkle-Patricia trie proof verification (keccak256) |
| `execution::rlp` | RLP decoding for Ethereum account state |
| `types::beacon` | BeaconBlockHeader, SyncCommittee, LightClientUpdate, etc. |
| `types::execution` | ExecutionPayloadHeader, AccountProof, AccountState |
| `crypto::bls` | BLS12-381 aggregate signature verification via `blst` |
| `crypto::merkle` | SSZ Merkle branch verification (generalized indices) |
| `crypto::signing` | Signing domain computation (fork version + genesis root) |

Key constants (Electra fork):
- `FINALIZED_ROOT_GINDEX = 169`, depth 7
- `NEXT_SYNC_COMMITTEE_GINDEX = 87`, depth 6
- `CURRENT_SYNC_COMMITTEE_GINDEX = 86`, depth 6

### `lumen-wasm` — WASM Bindings

Bridges `lumen-core` to JavaScript via `wasm-bindgen`.

| File | Purpose |
|------|---------|
| `lib.rs` | `LumenClient` struct: `from_beacon_bootstrap`, `process_finality_update`, `verify_account_rpc_proof`, `verify_account_rpc_proof_with_root` |
| `beacon_api.rs` | JSON adapter: converts beacon REST API response formats to `lumen-core` types |
| `network.rs` | Browser Fetch API wrappers (for future RPC-in-WASM) |
| `provider.rs` | JSON-RPC provider utilities |
| `state.rs` | Verified state cache and sync progress |

### `lumen-p2p` — P2P Network Types

**Status: not yet compiled to WASM or integrated into the demo.**

Contains libp2p configuration for direct P2P connections:

| File | Purpose |
|------|---------|
| `transport.rs` | WebRTC + WebTransport transport config |
| `behaviour.rs` | GossipSub + Identify + Ping network behaviour |
| `beacon_gossip.rs` | Beacon chain gossip topics (finality_update, optimistic_update) |
| `bootstrap.rs` | Bootstrap peer discovery with hardcoded bootnodes |
| `relay.rs` | Circuit relay client for NAT traversal |

When compiled to WASM and loaded in the Web Worker, this would replace HTTP polling with direct P2P gossip for receiving finality updates. The verification pipeline is unchanged — P2P delivers the same raw bytes that get BLS-verified in WASM.

---

## Demo Architecture

The demo's TypeScript is deliberately minimal — orchestration and UI only.

```
demo/main.ts        → UI + flow orchestration (no crypto)
demo/wasm.ts        → typed bridge to LumenClient WASM methods
demo/beacon.ts      → HTTP fetch from beacon APIs (raw JSON strings)
demo/rpc.ts         → HTTP fetch from execution RPCs (raw JSON strings)
demo/lumen-worker.ts → Web Worker: polls beacon APIs for background finality updates
```

### Data flow: verifying a balance

1. `main.ts` calls `initWasm()` → loads the WASM binary
2. `beacon.ts` fetches bootstrap JSON → `wasm.ts` calls `LumenClient.from_beacon_bootstrap(json)`
3. `beacon.ts` fetches finality update JSON → `wasm.ts` calls `LumenClient.process_finality_update(json)` → BLS verification in Rust
4. User enters an address → `main.ts` calls verification flow:
   - `rpc.ts` fetches `eth_getProof` at `latest` → raw JSON
   - `rpc.ts` fetches block header at `latest` → state root
   - `wasm.ts` calls `LumenClient.verify_account_rpc_proof_with_root(stateRoot, address, proofJson)` → keccak256 verification in Rust
   - Cross-check: latest block ≥ BLS-verified finalized block
5. `lumen-worker.ts` runs in a Web Worker, polling beacon APIs every 12 seconds for new finality updates, which are BLS-verified on arrival

### What the TypeScript does NOT do

- No keccak256 hashing
- No BLS signature verification
- No RLP decoding
- No Merkle-Patricia trie walking
- No SSZ parsing

All of this is in Rust/WASM.

---

## The One Trust Exception: `eth_call`

EVM execution cannot be proven without zero-knowledge proofs. When `eth_call` is invoked, it is forwarded to a fallback RPC and the result is marked as **unverified**. This is the one operation where Lumen trusts an external source. A console warning is logged every time.

The long-term path is a zk-EVM prover in the browser.

---

## Build Pipeline

1. Compile `lumen-core` + `lumen-wasm` to WASM via `wasm-pack` (requires LLVM with wasm32 target for `blst` C cross-compilation)
2. Output: `lumen_wasm.js` (glue), `lumen_wasm_bg.wasm` (binary), `lumen_wasm.d.ts` (types)
3. Vite serves the demo with the WASM module loaded at runtime

The `blst` crate (BLS12-381) contains C code that must be cross-compiled to wasm32. This requires Homebrew LLVM on macOS:

```bash
CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang \
AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar \
wasm-pack build crates/lumen-wasm --target web --out-dir ../../packages/lumen-js/wasm
```
