# lumen-eth

> Trustless Ethereum light client for the browser — no extension, no server, no trust required.

## Install

```bash
npm install lumen-eth
```

## Quick Start

```typescript
import { createLumenProvider } from 'lumen-eth'

// One line — replaces Infura/Alchemy with trustless verification
const provider = await createLumenProvider()

// Works with ethers.js v6
import { BrowserProvider } from 'ethers'
const ethersProvider = new BrowserProvider(provider)
const balance = await ethersProvider.getBalance("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")
// ^ Verified via BLS sync committee signatures + Merkle proof. No trust required.
```

## What Lumen Does

Lumen runs an Ethereum light client entirely in your browser as WebAssembly:

1. **Connects to Ethereum's P2P network** via WebRTC/WebTransport
2. **Verifies every piece of data** using BLS signatures and Merkle proofs
3. **Exposes a standard provider interface** (EIP-1193) — drop-in replacement
4. **No API keys, no accounts, no extensions** — works on page load

## Trust Model

| What | Trust Level | How |
|------|------------|-----|
| Account balances | **Fully trustless** | Merkle-Patricia trie proof against verified state root |
| Storage values | **Fully trustless** | Storage proof against verified account storage root |
| Transaction counts | **Fully trustless** | Same as account balances |
| Block numbers | **Fully trustless** | From verified beacon chain head |
| `eth_call` | ⚠ **Trusted** | Requires fallback RPC (EVM execution not provable without zk) |

## API

### `createLumenProvider(options?)`

Create a trustless Ethereum provider.

```typescript
const provider = await createLumenProvider({
  // All options are optional
  fallbackRpc: 'https://eth.llamarpc.com',  // For eth_call only
  verbose: true,                              // Log trust state to console
})
```

### `provider.request({ method, params })`

Standard EIP-1193 request method.

### `provider.getSyncState()`

Returns the current sync state for UI display.

### `provider.onSyncStateChange(callback)`

Subscribe to sync state updates.

## License

MIT OR Apache-2.0
