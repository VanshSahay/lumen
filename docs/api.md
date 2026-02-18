# Lumen API Reference

## Installation

```bash
npm install lumen-eth
```

---

## Quick Start

```typescript
import { createLumenProvider } from 'lumen-eth'

const provider = await createLumenProvider()
// provider is a standard EIP-1193 Ethereum provider
// Every eth_getBalance, eth_getTransactionCount, eth_getStorageAt call
// is cryptographically verified via BLS + keccak256 in Rust/WASM
```

---

## `createLumenProvider(options?)`

Create a fully initialized trustless Ethereum provider.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `checkpoint` | `string` | Auto-fetched | Initial checkpoint hash (hex with 0x prefix) |
| `fallbackRpc` | `string` | None | RPC URL for `eth_call` (the one unverified operation) |
| `beaconApis` | `string[]` | ChainSafe + PublicNode | Beacon API endpoints (untrusted data transport) |
| `executionRpcs` | `string[]` | PublicNode + LlamaRPC | Execution RPC endpoints (untrusted data transport) |
| `verbose` | `boolean` | true | Log trust state to console |

**Returns:** `Promise<LumenProvider>`

---

## Verification Pipeline (What Happens Under the Hood)

Every verified query follows this pipeline:

1. **WASM module loaded** — Rust BLS12-381 + keccak256 verification engine (~115 KB gzipped)
2. **Beacon bootstrap** — 512 sync committee BLS public keys fetched from beacon API
3. **BLS finality verification** — sync committee aggregate BLS12-381 signature verified in Rust/WASM → proves finalized state root
4. **Proof fetched** — `eth_getProof` from any execution RPC (untrusted bytes)
5. **keccak256 MPT verification** — Merkle-Patricia trie walked in Rust/WASM → proves account state
6. **Cross-check** — latest block extends BLS-verified finalized chain

---

## Supported Methods

### `eth_getBalance` — Cryptographically Verified

```typescript
const balance = await provider.request({
  method: 'eth_getBalance',
  params: ['0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045', 'latest']
})
// balance is verified: BLS-proven finality + keccak256 Merkle proof in Rust/WASM
```

Pipeline: fetch `eth_getProof` → verify keccak256 MPT in WASM → extract balance from RLP account state.

### `eth_getTransactionCount` — Cryptographically Verified

```typescript
const nonce = await provider.request({
  method: 'eth_getTransactionCount',
  params: ['0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045', 'latest']
})
```

Uses the same Merkle proof as `eth_getBalance`. The nonce is extracted from the same RLP-encoded account leaf.

### `eth_getStorageAt` — Cryptographically Verified

```typescript
const value = await provider.request({
  method: 'eth_getStorageAt',
  params: ['0xContractAddress', '0x0', 'latest']
})
```

Two-level verification: account proof (state root → account) + storage proof (storage root → slot value).

### `eth_getCode` — Cryptographically Verified

```typescript
const code = await provider.request({
  method: 'eth_getCode',
  params: ['0xContractAddress', 'latest']
})
```

The code hash is verified via account proof. The actual code bytes come from the RPC (the code hash can be cross-checked).

### `eth_chainId` — No Network Required

```typescript
const chainId = await provider.request({ method: 'eth_chainId' })
// "0x1" (mainnet)
```

### `eth_blockNumber` — Verified Head

```typescript
const blockNumber = await provider.request({ method: 'eth_blockNumber' })
// Returns the BLS-verified finalized block number
```

### `eth_call` — **NOT Verified** (The One Exception)

```typescript
const result = await provider.request({
  method: 'eth_call',
  params: [{ to: '0x...', data: '0x...' }, 'latest']
})
// ⚠ This result is from the fallback RPC and is NOT cryptographically verified
```

EVM execution cannot be proven without zero-knowledge proofs. This is forwarded to the fallback RPC with a console warning.

---

## WASM Module Direct API

For advanced usage, the WASM module can be used directly:

```typescript
import wasmInit, { LumenClient } from 'lumen-wasm'

await wasmInit()

// Initialize from beacon bootstrap
const client = LumenClient.from_beacon_bootstrap(bootstrapJson)

// BLS-verify a finality update
const result = client.process_finality_update(finalityUpdateJson)
// result.verified, result.finalized_slot, result.execution_state_root, etc.

// Get BLS-verified execution state
const state = client.get_execution_state()
// state.state_root, state.block_number, state.finalized_slot

// Verify a proof against the internal BLS-verified state root
const account = client.verify_account_rpc_proof(address, proofJson)

// Verify a proof against an explicit state root (race-condition safe)
const account2 = client.verify_account_rpc_proof_with_root(stateRootHex, address, proofJson)
// account.balance_hex, account.nonce, account.is_contract, account.proof_nodes_verified
```

---

## Framework Integration

### ethers.js v6

```typescript
import { BrowserProvider } from 'ethers'
import { createLumenProvider } from 'lumen-eth'

const lumen = await createLumenProvider()
const provider = new BrowserProvider(lumen)

const balance = await provider.getBalance('vitalik.eth')
```

### viem

```typescript
import { createPublicClient, custom } from 'viem'
import { mainnet } from 'viem/chains'
import { createLumenProvider } from 'lumen-eth'

const lumen = await createLumenProvider()
const client = createPublicClient({ chain: mainnet, transport: custom(lumen) })
```

### wagmi

```typescript
import { createConfig, custom } from 'wagmi'
import { mainnet } from 'wagmi/chains'
import { createLumenProvider } from 'lumen-eth'

const lumen = await createLumenProvider()
const config = createConfig({
  chains: [mainnet],
  transports: { [mainnet.id]: custom(lumen) },
})
```

---

## Error Handling

Lumen never silently falls back to unverified data. If verification fails, it throws:

```typescript
try {
  const balance = await provider.request({
    method: 'eth_getBalance',
    params: ['0x...', 'latest']
  })
} catch (error) {
  // error.code: -32000 for verification failures
  // error.message describes exactly what failed
}
```

| Code | Meaning |
|------|---------|
| `-32601` | Method not supported |
| `-32000` | Verification failed or no data source |
| `-32602` | Invalid parameters |

---

## Types

```typescript
import type {
  EIP1193Provider,
  SyncState,
  ConnectionMode,
  LumenOptions,
} from 'lumen-eth'
```
