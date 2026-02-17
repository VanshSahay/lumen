# Lumen Architecture

## Why This Exists

Almost every "decentralized" application today has a dirty secret: it trusts a centralized RPC provider to tell it what the blockchain state is. Infura, Alchemy, QuickNode — these services sit between your dApp and the actual Ethereum network. They can:

- **Lie about balances** — report a different number than what's on-chain
- **Censor transactions** — refuse to include your transaction in a block
- **Return fake contract state** — make your dApp think a contract has different storage
- **Go down** — take every dApp that depends on them offline simultaneously

This isn't a theoretical problem. This is the current state of Web3's architecture. The blockchain itself is decentralized. The applications reading from it are not.

Lumen fixes this by verifying Ethereum data directly in the browser using cryptographic proofs.

---

## How It Works (The Real Data Flow)

Lumen's verification has three stages:

### Stage 1: Beacon Chain Consensus (State Root)

The beacon chain is Ethereum's source of truth. Every ~6.4 minutes, a block is finalized — meaning 2/3+ of all validators have attested to it. This finalized block contains an execution payload with a `stateRoot` — the root hash of the entire Ethereum state trie.

Lumen fetches the latest **light client finality update** from multiple independent beacon chain APIs:

- **ChainSafe Lodestar** (`lodestar-mainnet.chainsafe.io`)
- **PublicNode Beacon** (`ethereum-beacon-api.publicnode.com`)

The finality update includes:
- The finalized beacon slot number
- The finalized execution payload (state root, block number, block hash)
- The sync committee aggregate signature and participation bitvector

Lumen requires **all sources to agree** on the same finalized execution state root. If any disagree, it halts and reports the discrepancy. This is the "checkpoint sync" trust model — multi-source consensus prevents any single provider from lying.

The sync committee participation is also checked: typically 500+ of 512 validators sign each finalized block. This is reported in the UI so users can see the health of the consensus.

### Stage 2: Merkle Proof Fetching (Untrusted Data)

Once we have a trusted state root from the beacon chain, we need the actual Merkle proof for a specific account. This is fetched via `eth_getProof` from an execution RPC endpoint.

**The execution RPC is treated as an untrusted data transport.** It could be PublicNode, LlamaRPC, a local node, or a malicious server — it doesn't matter. The proof bytes are verified locally in Stage 3. The RPC cannot forge a valid proof without finding a keccak256 collision (computationally infeasible — would require ~2^128 operations).

A practical constraint: free public RPCs only serve `eth_getProof` for very recent blocks (the "proof window" is typically <128 blocks). Since the beacon-finalized block can be ~95 blocks behind the head, Lumen fetches the proof at the `latest` block and cross-checks that this block extends the beacon-finalized chain (block number >= finalized block number).

### Stage 3: Local Cryptographic Verification

The Merkle-Patricia trie proof is verified entirely in the browser:

1. Compute `keccak256(address)` to get the trie key (20-byte address → 32-byte key)
2. Start at the state root hash from the beacon chain
3. For each node in the proof:
   - Verify `keccak256(node_rlp) == expected_hash`
   - Decode the RLP node (branch with 17 items, or extension/leaf with 2 items)
   - Follow the path using the key nibbles
4. At the leaf, decode the RLP-encoded account: `[nonce, balance, storageRoot, codeHash]`
5. Cross-check: the proof-extracted balance should match what the RPC claimed

If ANY hash in the chain doesn't match, the proof is **rejected**. The verification is pure math — no trust required.

---

## The Sync Committee

Every ~27 hours (256 epochs), Ethereum randomly selects 512 validators to form the **sync committee**. These validators sign every block header during their duty period. The key insight: you don't need to verify every validator's signature — you only need to verify that 2/3+ of the 512 sync committee members signed off.

The sync committee uses BLS12-381 signatures, which can be aggregated — instead of verifying 512 individual signatures, you verify a single aggregate signature against the aggregate of participating public keys. This is what the Rust `lumen-core` crate implements.

In the current demo, BLS signature verification is not performed on each request. Instead, the demo uses multi-source consensus (multiple beacon APIs agreeing on the same finality update) as a practical approximation. The BLS verification code exists in `lumen-core` and is compiled to WASM, ready for integration when the full light client sync pipeline is connected.

---

## Why Merkle-Patricia Trie Proofs Work

Once you have a verified state root (from the beacon chain), you can prove any account's state:

- **Account proof**: Proves an account's nonce, balance, storage root, and code hash against the state root
- **Storage proof**: Proves a specific storage slot value against an account's storage root
- **Receipt proof**: Proves a transaction receipt against the receipts root

These proofs are purely mathematical — given a root hash and a proof path, either the data is correct or it isn't. The demo implements this verification in TypeScript using `js-sha3` for keccak256 hashing, with full RLP decoding and Merkle-Patricia trie walking.

The Rust `lumen-core` crate implements the same verification logic, along with BLS signature verification. The WASM module (`lumen-wasm`) exposes these functions to JavaScript via `wasm-bindgen`.

---

## The Rust/WASM Stack

### Why Rust?

- **Correctness**: Rust's type system and memory safety prevent entire classes of bugs
- **Performance**: BLS12-381 verification is computationally intensive — Rust/WASM is ~10x faster than JavaScript for this
- **Portability**: Compiles to WASM that runs in every modern browser

### Crate Structure

**`lumen-core`** — Pure Rust verification library (no networking, no WASM dependencies):
- BLS12-381 signature verification via the `blst` crate
- Merkle-Patricia trie proof verification
- RLP decoding for Ethereum account data
- SSZ types for beacon chain data
- Checkpoint consensus logic
- 30+ tests with real Ethereum data structures

**`lumen-wasm`** — WASM bindings via `wasm-bindgen`:
- `LumenClient` struct with methods for processing light client updates and verifying proofs
- Browser Fetch API wrappers for network requests
- JSON-RPC provider utilities
- Verified state cache and sync progress tracking
- Compiled size: 298 KB raw, **115 KB gzipped**

**`lumen-p2p`** — P2P layer types and configuration:
- libp2p transport configuration (WebRTC, WebTransport)
- GossipSub behaviour for beacon chain topics
- Peer scoring and bootstrap node configuration
- Circuit relay strategies

### Build Pipeline

The `build.sh` script orchestrates the full build:
1. Run all Rust tests (`cargo test --workspace` — 37 tests)
2. Compile WASM via `wasm-pack` with LLVM clang for `wasm32-unknown-unknown`
3. Check gzipped WASM size (must be <2 MB)
4. Install npm dependencies (`pnpm install`)
5. Build TypeScript packages (`lumen-js`, `lumen-react`)
6. Build the demo app (Vite)

On macOS, `blst` (the BLS library) requires Homebrew LLVM for cross-compilation to WASM. The build script auto-detects this at `/opt/homebrew/opt/llvm` (Apple Silicon) or `/usr/local/opt/llvm` (Intel).

---

## The One Trust Exception: `eth_call`

There is one thing Lumen cannot verify trustlessly: **EVM execution**.

When you call `eth_call`, you're asking "what would this transaction return if executed right now?" To answer this, you need to actually run the EVM against the current state. This is computationally expensive and, more importantly, **not provable** without zero-knowledge proofs.

Lumen's approach:
1. `eth_call` requests are forwarded to a configurable fallback RPC
2. The result is clearly marked as **unverified**
3. The console logs a warning every time this happens
4. Documentation explicitly states this is the one trust exception

The long-term solution is a zk-EVM prover in the browser, which would make `eth_call` provable. This is an active area of research but not yet practical for production use.

---

## System Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                          Browser                              │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                     Demo / dApp                          │ │
│  │                                                          │ │
│  │  beacon.ts          rpc.ts             verify.ts         │ │
│  │  ┌────────────┐     ┌──────────────┐   ┌─────────────┐  │ │
│  │  │ Beacon API │     │ Execution    │   │ keccak256 + │  │ │
│  │  │ Consensus  │     │ RPC (untrust)│   │ RLP + MPT   │  │ │
│  │  └─────┬──────┘     └──────┬───────┘   └──────┬──────┘  │ │
│  │        │                   │                   │         │ │
│  │        ▼                   ▼                   ▼         │ │
│  │  finalized            eth_getProof       local proof     │ │
│  │  state root           (raw bytes)        verification    │ │
│  └──┬──────────────────────────────────────────────┬────────┘ │
│     │                                              │          │
│  ┌──▼──────────────────────────────────────────────▼────────┐ │
│  │              Rust/WASM (lumen-core + lumen-wasm)          │ │
│  │  BLS12-381 verification │ MPT proofs │ RLP/SSZ decoding   │ │
│  │  (compiled, 115 KB gzip)                                  │ │
│  └───────────────────────────────────────────────────────────┘ │
└──────────────┬────────────────────────┬───────────────────────┘
               │                        │
    ┌──────────▼──────────┐  ┌──────────▼──────────┐
    │  Beacon Chain APIs   │  │  Execution RPCs      │
    │  (ChainSafe, Public  │  │  (PublicNode, Llama   │
    │   Node — consensus)  │  │   — untrusted data)   │
    └─────────────────────┘  └──────────────────────┘
```

---

## Data Flow: Verifying a Balance

1. User enters an Ethereum address in the demo
2. `beacon.ts` fetches the light client finality update from 2 independent beacon APIs
3. Both sources must agree on the finalized execution state root (consensus check)
4. `rpc.ts` fetches `eth_getProof` from an execution RPC at the `latest` block
5. `main.ts` cross-checks that the proof block extends the beacon-finalized chain
6. `verify.ts` walks the Merkle-Patricia trie proof:
   - Computes `keccak256(address)` to get the trie key
   - At each node: verifies `keccak256(node_rlp) == expected_hash`
   - Follows branch/extension/leaf nodes using key nibbles
   - Decodes the RLP-encoded account at the leaf: `[nonce, balance, storageRoot, codeHash]`
7. Cross-checks the proof-verified balance against the RPC's claimed balance
8. If all checks pass, displays the verified balance
9. If any check fails, shows an error (never displays unverified data)

Typical end-to-end time: ~500ms (dominated by beacon API fetch; local verification is <100ms).
