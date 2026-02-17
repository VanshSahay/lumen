# Lumen Architecture

## Why This Exists

Almost every "decentralized" application today has a dirty secret: it trusts a centralized RPC provider to tell it what the blockchain state is. Infura, Alchemy, QuickNode — these services sit between your dApp and the actual Ethereum network. They can:

- **Lie about balances** — report a different number than what's on-chain
- **Censor transactions** — refuse to include your transaction in a block
- **Return fake contract state** — make your dApp think a contract has different storage
- **Go down** — take every dApp that depends on them offline simultaneously

This isn't a theoretical problem. This is the current state of Web3's architecture. The blockchain itself is decentralized. The applications reading from it are not.

Lumen fixes this by moving the verification into the browser itself.

---

## How Beacon Chain Light Clients Work

Ethereum's consensus layer (the beacon chain) has a built-in mechanism for light clients: the **sync committee**.

### The Sync Committee

Every ~27 hours (256 epochs), Ethereum randomly selects 512 validators to form the **sync committee**. These 512 validators are responsible for signing every block header during their duty period. This signature is specifically designed for light client verification.

The key insight: **you don't need to verify every validator's signature**. You only need to verify that 2/3+ of the 512 sync committee members signed off on a block header. This is the same trust assumption as trusting Ethereum itself.

### BLS Signature Aggregation

The sync committee uses BLS12-381 signatures, which have a critical property: **signatures can be aggregated**. Instead of verifying 512 individual signatures (which would be slow), we verify a single aggregate signature against the aggregate of participating public keys.

The verification pipeline:
1. Receive a `LightClientUpdate` containing the attested header and sync committee signature
2. Extract the bitvector indicating which committee members signed (must be ≥ 342/512)
3. Aggregate the public keys of participating members
4. Verify the aggregate signature against the signing root
5. If it passes, the header is legitimate (with the same trust as Ethereum consensus)

### Merkle-Patricia Trie Proofs

Once we have a verified block header (which contains a `stateRoot`), we can verify any piece of execution layer data using Merkle-Patricia trie proofs:

- **Account proof**: Proves an account's nonce, balance, storage root, and code hash against the state root
- **Storage proof**: Proves a specific storage slot value against an account's storage root
- **Receipt proof**: Proves a transaction receipt against the receipts root

These proofs are purely mathematical — given a root hash and a proof path, either the data is correct or it isn't. No trust required.

---

## Why WASM in the Browser

### The Problem with JavaScript Crypto

JavaScript's `BigInt` and floating-point math are not suitable for consensus-critical cryptography:

- **No constant-time guarantees** — timing attacks are possible
- **Floating point** — rounding errors in any financial calculation are unacceptable
- **Performance** — BLS verification in JS is 10-50x slower than native/WASM

### Why Not a Browser Extension?

Extensions require installation, create a trust boundary (you trust the extension author), and are not available on mobile browsers. Lumen runs as a regular script — no permissions, no installation, no trust in a third party.

### The WASM Approach

Lumen compiles the entire verification pipeline (BLS signatures, Merkle proofs, RLP decoding) to WebAssembly:

- **Correctness**: Rust's type system and memory safety prevent entire classes of bugs
- **Performance**: WASM runs at near-native speed, ~10x faster than JS for crypto
- **Portability**: Works in every modern browser (Chrome, Firefox, Safari, Edge)
- **Size**: The compiled WASM binary is under 2MB gzipped
- **Isolation**: Runs in a Web Worker, never blocking the main thread

---

## The P2P Strategy

### Why Not Just Use HTTP?

HTTP requests go to a single server. That server is a single point of failure and trust. Even if you verify the response, the server can:

- Refuse to respond (censorship)
- Respond with stale data (omission)
- Track which addresses you query (privacy)

### WebRTC and WebTransport

Browsers can't open raw TCP connections. But they can use:

1. **WebTransport** (preferred) — a new browser API built on HTTP/3 and QUIC. Lower latency than WebRTC, supports both reliable and unreliable streams. Used by Ethereum nodes that support it.

2. **WebRTC** (fallback) — the same technology that powers video calls. Can establish direct peer-to-peer connections through NATs using STUN/TURN. More widely supported.

Both are encrypted and provide direct connections to Ethereum nodes.

### The Circuit Relay Bootstrap Problem

There's a chicken-and-egg problem: to find peers, you need to be connected to peers. The solution is circuit relays:

1. On startup, try to connect directly to known Ethereum bootnodes via WebTransport
2. If no direct connection within 3 seconds, connect via a circuit relay
3. The relay forwards traffic between us and Ethereum peers
4. Once connected, ask peers for more peer addresses (peer exchange)
5. Establish direct connections to discovered peers
6. Drop the relay connection once we have direct peers

**Trust implication**: The circuit relay can see metadata (who's connecting to whom) but cannot read or modify data (encrypted with Noise protocol). This is acceptable for bootstrapping but should be upgraded to direct connections ASAP.

### GossipSub

Once connected to Ethereum peers, Lumen subscribes to beacon chain gossip topics:

- `light_client_finality_update` — new finalized chain heads (strongest guarantee)
- `light_client_optimistic_update` — new attested blocks (lower latency)

Messages arrive as SSZ-encoded bytes, are deserialized, and passed through the full BLS verification pipeline. Invalid messages are discarded and the sending peer's score is reduced.

---

## The One Trust Exception: `eth_call`

There is one thing Lumen cannot verify trustlessly: **EVM execution**.

When you call `eth_call`, you're asking "what would this transaction return if executed right now?" To answer this, you need to actually run the EVM against the current state. This is computationally expensive and, more importantly, **not provable** without zero-knowledge proofs.

Lumen's approach:
1. `eth_call` requests are forwarded to a configurable fallback RPC
2. The result is clearly marked as **unverified**
3. The console logs a warning every time this happens
4. Documentation explicitly states this is the one trust exception

The long-term solution is to embed a zk-EVM prover in the browser, which would make `eth_call` provable. This is an active area of research but not yet practical for production use.

---

## System Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                        Browser                               │
│                                                              │
│  ┌──────────────────┐     ┌─────────────────────────────┐   │
│  │    Main Thread    │     │        Web Worker            │   │
│  │                   │     │                              │   │
│  │  ┌─────────────┐ │ msg │  ┌──────────────────────┐   │   │
│  │  │ LumenProvider│◄├─────├──┤  lumen-wasm (WASM)   │   │   │
│  │  │  (EIP-1193) │ │     │  │  ┌────────────────┐  │   │   │
│  │  └─────────────┘ │     │  │  │  lumen-core    │  │   │   │
│  │        │         │     │  │  │  (Rust)        │  │   │   │
│  │        │         │     │  │  │  - BLS verify  │  │   │   │
│  │  ┌─────────────┐ │     │  │  │  - MPT proofs  │  │   │   │
│  │  │  Your dApp  │ │     │  │  │  - Checkpoint  │  │   │   │
│  │  │  (ethers.js │ │     │  │  └────────────────┘  │   │   │
│  │  │   / viem)   │ │     │  └──────────────────────┘   │   │
│  │  └─────────────┘ │     │             │               │   │
│  └──────────────────┘     │  ┌──────────────────────┐   │   │
│                           │  │  lumen-p2p (WASM)    │   │   │
│                           │  │  - libp2p            │   │   │
│                           │  │  - WebRTC/WebTransp  │   │   │
│                           │  │  - GossipSub         │   │   │
│                           │  └──────────┬───────────┘   │   │
│                           └─────────────┼───────────────┘   │
│                                         │                    │
└─────────────────────────────────────────┼────────────────────┘
                                          │
                              ┌───────────┴───────────┐
                              │  Ethereum P2P Network  │
                              │  (WebRTC/WebTransport) │
                              └────────────────────────┘
```

---

## Data Flow: Verifying a Balance

1. dApp calls `provider.request({ method: 'eth_getBalance', params: ['0x...', 'latest'] })`
2. LumenProvider sends request to WASM worker
3. Worker requests `eth_getProof` from connected P2P peer (or fallback RPC)
4. Proof response arrives (untrusted data from network)
5. Worker passes proof to `lumen-core::verify_account_proof()`
   - Computes `keccak256(address)` to get the trie key
   - Walks the Merkle-Patricia trie proof from root to leaf
   - At each step, verifies `keccak256(node) == expected_hash`
   - Checks the root matches our verified `stateRoot`
   - Decodes the RLP-encoded account data
6. If verification passes, returns the balance to the dApp
7. If verification fails, throws an error (never returns unverified data)

Total time: ~50-200ms (dominated by network, not verification).
