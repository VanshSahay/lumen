# Lumen

**The first fully trustless Ethereum light client that runs in any browser — no extension, no server, no trust required.**

Lumen runs an Ethereum light client entirely inside the browser as WebAssembly. It verifies every piece of data cryptographically — block headers via BLS signature verification against Ethereum's sync committee, account state and transaction data via Merkle-Patricia trie proofs.

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

Same interface. Drop-in replacement. Zero config. Zero trust.

## How It Works

1. **Connects directly to Ethereum's P2P network** via WebRTC and WebTransport
2. **Verifies every piece of data cryptographically** — BLS signatures from the sync committee, Merkle-Patricia trie proofs for all account data
3. **Exposes a standard EIP-1193 provider** — works with ethers.js, viem, wagmi, web3.js
4. **Requires no login, no extension, no account** — initializes on page load

## Trust Model

| Layer | What Lumen Trusts | Attack Surface |
|-------|-------------------|----------------|
| Cryptography | BLS12-381, keccak256 (standard assumptions) | Theoretical only |
| Sync committee | 2/3+ of 512 validators honest | Same as trusting Ethereum itself |
| P2P peers | Nothing — all data verified | Peers can lie, Lumen rejects it |
| Circuit relay | Peer introductions only | Cannot forge proofs |
| Checkpoint | Multi-source consensus | Requires collusion of N sources |

See [docs/trust-model.md](docs/trust-model.md) for the complete, brutally honest breakdown.

## Architecture

```
crates/
├── lumen-core/     # Pure Rust: BLS verification, Merkle proofs, no networking
├── lumen-wasm/     # WASM bindings: bridges Rust to JS via wasm-bindgen
└── lumen-p2p/      # P2P layer: libp2p WebRTC + WebTransport

packages/
├── lumen-js/       # TypeScript npm package (EIP-1193 provider)
└── lumen-react/    # React hook (useLumen)

demo/               # Demo web app
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

## Development

### Prerequisites

- Rust (latest stable) with `wasm32-unknown-unknown` target
- wasm-pack
- Node.js >= 18
- pnpm >= 8

### Setup

```bash
# Install Rust WASM target
rustup target add wasm32-unknown-unknown

# Install wasm-pack
cargo install wasm-pack

# Install dependencies
pnpm install

# Run Rust tests
cargo test --workspace

# Build everything
./build.sh

# Run the demo
cd demo && pnpm dev
```

### Project Structure

| Crate/Package | Purpose | Dependencies |
|--------------|---------|-------------|
| `lumen-core` | Pure Rust verification (BLS, Merkle proofs) | blst, sha2, tiny-keccak |
| `lumen-wasm` | WASM bindings via wasm-bindgen | lumen-core, wasm-bindgen |
| `lumen-p2p` | Browser P2P (WebRTC/WebTransport) | libp2p |
| `lumen-js` | TypeScript npm package | lumen-wasm (compiled) |
| `lumen-react` | React hook | lumen-js |

## Documentation

- [Architecture](docs/architecture.md) — System design and data flow
- [Trust Model](docs/trust-model.md) — Precise, honest security analysis
- [API Reference](docs/api.md) — Complete TypeScript API docs

## Non-Negotiables

1. All crypto verification happens in Rust/WASM, never in JavaScript
2. WASM runs in a Web Worker — never blocks the main thread
3. Never silently falls back to unverified data — throws instead
4. Trust state is always logged clearly to console
5. Works on mainnet, not just testnets

## Toolchain

Pin these versions for reproducible builds:

| Tool | Version |
|------|---------|
| Rust | 1.77+ (stable) |
| wasm-pack | 0.12+ |
| Node.js | 18+ |
| pnpm | 9+ |

## License

MIT OR Apache-2.0
