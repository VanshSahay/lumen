# Lumen

**Trustless Ethereum light client that runs in any browser — no extension, no server, no API key.**

Lumen verifies Ethereum account state directly in the browser using cryptographic proofs. It fetches the finalized state root from the beacon chain via multi-source consensus, retrieves Merkle proofs from any execution node (untrusted), and verifies every keccak256 hash locally from state root to account leaf. If the math checks out, the data is correct — regardless of where it came from.

## The Problem

Almost every "decentralized" application today secretly trusts a centralized RPC provider — Infura, Alchemy, QuickNode — to tell it what the blockchain state is. These providers can lie about account balances, censor transactions, return fake contract state, or go down entirely.

The blockchain is decentralized. The apps built on it are not.

## The Solution

```typescript
// Before (trusting Infura):
const provider = new ethers.JsonRpcProvider("https://mainnet.infura.io/v3/YOUR_KEY")

// After (trustless):
const provider = await createLumenProvider()
```

Same interface. Drop-in replacement. Zero config.

## How It Works

1. **Fetches finalized state root from the beacon chain** — queries multiple independent beacon APIs (ChainSafe Lodestar, PublicNode) and requires consensus before proceeding
2. **Fetches Merkle proofs from any execution node** — the execution RPC is an untrusted data transport; it cannot forge a valid proof
3. **Verifies every proof locally via keccak256** — walks the Merkle-Patricia trie from state root to account leaf, checking every hash
4. **Exposes a standard EIP-1193 provider** — works with ethers.js, viem, wagmi, web3.js

## Trust Model

| Layer | What Lumen Trusts | Why It's OK |
|-------|-------------------|-------------|
| Beacon chain finality | Multiple independent beacon APIs agree | Same as checkpoint sync; collusion requires compromising N operators |
| Sync committee | 512 validators signed the finalized header | Same trust as Ethereum consensus itself (2/3 honest) |
| Merkle proofs | keccak256 collision resistance | Standard cryptographic assumption; computationally infeasible to break |
| Execution RPC | Nothing — it's an untrusted data pipe | Proof either verifies against the state root or gets rejected |
| `eth_call` | Fallback RPC (the one exception) | EVM execution isn't provable without zk-proofs (yet) |

See [docs/trust-model.md](docs/trust-model.md) for the complete, brutally honest breakdown.

## Architecture

```
crates/
├── lumen-core/     # Pure Rust: BLS verification, Merkle proofs, RLP/SSZ
├── lumen-wasm/     # WASM bindings: bridges Rust to JS via wasm-bindgen
└── lumen-p2p/      # P2P types: libp2p transport, gossipsub, peer scoring

packages/
├── lumen-js/       # TypeScript npm package (EIP-1193 provider)
└── lumen-react/    # React hook (useLumen)

demo/               # Live demo: real verification against Ethereum mainnet
├── beacon.ts       # Beacon chain light client sync (multi-source consensus)
├── rpc.ts          # Execution RPC data transport (untrusted)
├── verify.ts       # keccak256 + RLP + Merkle-Patricia trie verification
└── main.ts         # UI and verification flow

docs/               # Architecture, trust model, API reference
```

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

const balance = await provider.getBalance("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")
// ^ This balance was cryptographically verified via Merkle proof
```

### Use with viem

```typescript
import { createPublicClient, custom } from 'viem'
import { mainnet } from 'viem/chains'
import { createLumenProvider } from 'lumen-eth'

const lumen = await createLumenProvider()
const client = createPublicClient({
  chain: mainnet,
  transport: custom(lumen),
})
```

### Use with React

```tsx
import { useLumen } from 'lumen-react'

function App() {
  const { provider, syncState, isReady } = useLumen()

  if (!isReady) return <div>Syncing: {syncState.status}...</div>
  return <div>Connected! Head: {syncState.headSlot}</div>
}
```

## Demo

The demo app performs real trustless verification against Ethereum mainnet:

```bash
cd demo && pnpm dev
```

Enter any Ethereum address and the demo will:
1. Fetch the finalized state root from 2 independent beacon chain APIs
2. Verify multi-source consensus (both must agree)
3. Fetch `eth_getProof` from an untrusted execution RPC
4. Verify the Merkle-Patricia trie proof locally (keccak256 hash chain)
5. Decode the RLP account state and display the verified balance
6. Cross-check the RPC's claimed balance against the proof-verified balance

Verified balances match Etherscan exactly.

## Development

### Prerequisites

- Rust (stable) with `wasm32-unknown-unknown` target
- wasm-pack
- Node.js >= 18
- pnpm >= 8
- LLVM with wasm32 support (for `blst` C compilation — `brew install llvm` on macOS)

### Setup

```bash
# Install Rust WASM target
rustup target add wasm32-unknown-unknown

# Install wasm-pack
cargo install wasm-pack

# Install dependencies
pnpm install

# Run Rust tests (37 tests across all crates)
cargo test --workspace

# Build everything (Rust + WASM + TypeScript + demo)
./build.sh

# Run the demo
cd demo && pnpm dev
```

### Build Output

| Artifact | Size |
|----------|------|
| WASM binary (raw) | ~298 KB |
| WASM binary (gzipped) | ~115 KB |
| Demo JS bundle (gzipped) | ~9 KB |

### Project Structure

| Crate/Package | Purpose | Key Dependencies |
|--------------|---------|-----------------|
| `lumen-core` | BLS verification, Merkle proofs, RLP/SSZ | blst, alloy-primitives, sha2, tiny-keccak |
| `lumen-wasm` | WASM bindings for browser | lumen-core, wasm-bindgen, web-sys |
| `lumen-p2p` | P2P transport and gossip types | libp2p |
| `lumen-js` | TypeScript EIP-1193 provider | lumen-wasm (compiled) |
| `lumen-react` | React hook | lumen-js |
| `demo` | Live mainnet demo | js-sha3 (keccak256), vite |

## Documentation

- [Architecture](docs/architecture.md) — System design, data flow, and verification pipeline
- [Trust Model](docs/trust-model.md) — Precise, honest security analysis of every component
- [API Reference](docs/api.md) — Complete TypeScript API docs

## Non-Negotiables

1. Never silently falls back to unverified data — throws instead
2. Trust state is always visible (demo trust log, console logging)
3. The execution RPC is an untrusted data transport, not a trust anchor
4. Every balance shown has been cryptographically verified via Merkle proof
5. Works on mainnet with real data, not simulations

## License

MIT OR Apache-2.0
