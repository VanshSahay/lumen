# Lumen

**Trustless Ethereum light client — BLS + keccak256 verification in Rust/WASM, runs in any browser.**

Lumen verifies Ethereum state directly in the browser with zero trusted intermediaries. The sync committee's BLS12-381 aggregate signature is verified in Rust/WASM to prove a finalized state root, then Merkle-Patricia trie proofs are verified via keccak256 in Rust/WASM to prove individual account balances. The beacon APIs and execution RPCs are untrusted data transport — they deliver bytes, Lumen verifies the math.

## The Problem

Every "decentralized" application today trusts a centralized RPC provider (Infura, Alchemy, QuickNode) to report blockchain state. These providers can lie about balances, censor queries, or go offline. The blockchain is decentralized. The applications reading from it are not.

## The Solution

```typescript
import { createLumenProvider } from 'lumen-eth'

const provider = await createLumenProvider()
// Every balance, nonce, and storage query is now cryptographically verified
```

Drop-in EIP-1193 provider. Works with ethers.js, viem, wagmi.

## How It Works

```
Beacon API (untrusted)           Execution RPC (untrusted)
       │                                  │
       ▼                                  ▼
┌─────────────────────────────────────────────────────────┐
│                  Rust / WASM (115 KB gzip)               │
│                                                          │
│  1. BLS12-381 signature verification (blst crate)        │
│     └─ Proves: 512 sync committee validators signed      │
│        the finalized beacon block header                 │
│                                                          │
│  2. Merkle-Patricia trie verification (keccak256)        │
│     └─ Proves: account balance at a specific state root  │
│                                                          │
│  Result: cryptographically verified account state        │
└─────────────────────────────────────────────────────────┘
```

1. **Beacon bootstrap** — fetches the current sync committee (512 BLS public keys) from a beacon API
2. **BLS verification** — verifies the sync committee's aggregate BLS12-381 signature on a finality update, proving a finalized execution state root
3. **Proof fetch** — fetches `eth_getProof` from any execution RPC (untrusted bytes)
4. **keccak256 MPT verification** — walks the Merkle-Patricia trie in Rust/WASM, verifying every hash from state root to account leaf
5. **Cross-check** — confirms the proof's block extends the BLS-verified finalized chain

Zero TypeScript crypto. All verification happens in Rust compiled to WebAssembly.

## Architecture

```
crates/
├── lumen-core/     # Pure Rust: BLS12-381, keccak256 MPT, RLP/SSZ
├── lumen-wasm/     # WASM bindings: LumenClient + beacon API adapter
└── lumen-p2p/      # P2P types: libp2p transport, gossipsub (not yet WASM)

packages/
├── lumen-js/       # TypeScript npm package (EIP-1193 provider)
└── lumen-react/    # React hook (useLumen)

demo/               # Live demo: real verification against Ethereum mainnet
├── main.ts         # Orchestrator — loads WASM, drives verification flow
├── wasm.ts         # Thin typed bridge to Rust/WASM LumenClient
├── beacon.ts       # Beacon API data transport (raw JSON, untrusted)
├── rpc.ts          # Execution RPC data transport (raw JSON, untrusted)
└── lumen-worker.ts # Web Worker: polls beacon APIs for finality updates
```

### What each component does

| Component | Language | Role | Trusted? |
|-----------|----------|------|----------|
| `lumen-core` | Rust | BLS verification, MPT proofs, RLP/SSZ | **Verification engine** |
| `lumen-wasm` | Rust → WASM | Bridges lumen-core to JavaScript | **Verification engine** |
| `lumen-p2p` | Rust | libp2p transport types (WebRTC, gossipsub) | Not integrated yet |
| `demo/beacon.ts` | TypeScript | Fetches raw JSON from beacon APIs | Untrusted transport |
| `demo/rpc.ts` | TypeScript | Fetches raw JSON from execution RPCs | Untrusted transport |
| `demo/wasm.ts` | TypeScript | ~130 lines — typed wrapper around WASM | Thin bridge |
| `demo/main.ts` | TypeScript | UI orchestration, no crypto | No crypto |

### lumen-p2p status

The `lumen-p2p` crate contains libp2p transport configuration (WebRTC, WebTransport), GossipSub behaviour for beacon chain topics, and peer scoring. It is **not yet compiled to WASM or integrated into the demo**. The demo's Web Worker currently uses HTTP polling of beacon REST APIs as the data transport layer. The architecture is designed so that swapping HTTP polling for P2P gossip requires no changes to the verification pipeline — both deliver raw bytes that get BLS-verified in WASM.

## Trust Model

| Layer | What Lumen Trusts | Current Status |
|-------|-------------------|----------------|
| BLS12-381 crypto | Discrete log hardness on BLS12-381 | **Active** — verified in Rust/WASM |
| Sync committee | 2/3 of 512 validators honest | **Active** — BLS aggregate sig verified |
| Merkle proofs | keccak256 collision resistance | **Active** — verified in Rust/WASM |
| Beacon APIs | Nothing — untrusted data transport | Raw JSON delivered, BLS-verified locally |
| Execution RPCs | Nothing — untrusted data transport | Raw proof bytes verified via keccak256 |
| `eth_call` | Fallback RPC (the one exception) | EVM execution not provable without zk-proofs |

See [docs/trust-model.md](docs/trust-model.md) for the complete breakdown.

## Demo

The demo performs real trustless verification against Ethereum mainnet:

```bash
# Prerequisites: Rust stable, wasm-pack, LLVM (brew install llvm), Node.js 18+, pnpm

# Build WASM (requires LLVM for blst cross-compilation)
cd crates/lumen-wasm
CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang \
AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar \
wasm-pack build --target web --out-dir ../../packages/lumen-js/wasm --out-name lumen_wasm

# Run the demo
cd demo && pnpm install && pnpm dev
```

Enter any Ethereum address and the demo will:
1. Load the Rust/WASM verification engine (BLS + keccak256)
2. Fetch beacon bootstrap (512 sync committee BLS public keys)
3. BLS-verify a finality update (aggregate signature from sync committee)
4. Fetch `eth_getProof` at `latest` from an untrusted execution RPC
5. Cross-check that `latest` block extends the BLS-verified finalized chain
6. Verify the Merkle-Patricia trie proof in Rust/WASM (keccak256)
7. Decode the RLP account state and display the verified balance

## Quick Start

### Install

```bash
npm install lumen-eth
```

### Use with ethers.js v6

```typescript
import { BrowserProvider } from 'ethers'
import { createLumenProvider } from 'lumen-eth'

const lumen = await createLumenProvider()
const provider = new BrowserProvider(lumen)

const balance = await provider.getBalance("vitalik.eth")
// This balance was cryptographically verified: BLS + keccak256 in Rust/WASM
```

### Use with viem

```typescript
import { createPublicClient, custom } from 'viem'
import { mainnet } from 'viem/chains'
import { createLumenProvider } from 'lumen-eth'

const lumen = await createLumenProvider()
const client = createPublicClient({ chain: mainnet, transport: custom(lumen) })
```

## Development

### Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Rust | stable | `rustup default stable` |
| wasm32 target | — | `rustup target add wasm32-unknown-unknown` |
| wasm-pack | 0.12+ | `cargo install wasm-pack` |
| LLVM | 15+ | `brew install llvm` (macOS, needed for `blst` → WASM cross-compilation) |
| Node.js | 18+ | — |
| pnpm | 8+ | — |

### Build

```bash
# Run Rust tests
cargo test --workspace

# Build WASM
CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang \
AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar \
wasm-pack build crates/lumen-wasm --target web --out-dir ../../packages/lumen-js/wasm --out-name lumen_wasm

# Run the demo
cd demo && pnpm dev
```

### Build Output

| Artifact | Size |
|----------|------|
| WASM binary (gzipped) | ~115 KB |
| WASM binary (raw) | ~360 KB |

## Documentation

- [Architecture](docs/architecture.md) — System design, data flow, verification pipeline
- [Trust Model](docs/trust-model.md) — Honest security analysis of every component
- [API Reference](docs/api.md) — TypeScript API docs

## Non-Negotiables

1. All cryptographic verification happens in Rust/WASM — zero TypeScript crypto
2. Never silently falls back to unverified data — throws instead
3. Trust state is always visible (UI trust log, console logging)
4. Beacon APIs and execution RPCs are untrusted data transport, not trust anchors
5. Works on mainnet with real data, not simulations

## License

MIT OR Apache-2.0
