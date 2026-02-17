# Lumen API Reference

Complete API reference for the `lumen-eth` npm package.

## Installation

```bash
npm install lumen-eth
# or
pnpm add lumen-eth
# or
yarn add lumen-eth
```

---

## Quick Start

```typescript
import { createLumenProvider } from 'lumen-eth'

const provider = await createLumenProvider()
```

That's it. `provider` is a standard EIP-1193 Ethereum provider.

---

## `createLumenProvider(options?)`

Create a fully initialized, trustless Ethereum provider.

**Parameters:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `checkpoint` | `string` | Auto-fetched | Override the initial checkpoint hash (hex with 0x prefix) |
| `fallbackRpc` | `string` | None | RPC URL for `eth_call` and proof data (untrusted, verified where possible) |
| `checkpointSources` | `string[]` | 5 default sources | URLs to fetch checkpoint from |
| `requiredCheckpointAgreement` | `number` | 3 | How many sources must agree on the checkpoint |
| `maxPeers` | `number` | 10 | Maximum P2P peer connections (for future P2P integration) |
| `verbose` | `boolean` | true | Log trust state to console |

**Returns:** `Promise<LumenProvider>`

**Example:**

```typescript
const provider = await createLumenProvider({
  fallbackRpc: 'https://eth.llamarpc.com',  // Used for eth_call and proof data
  verbose: true,
})
```

---

## `LumenProvider`

The main provider class. Implements the EIP-1193 standard interface.

### `provider.request(args)`

Standard EIP-1193 request method.

**Supported Methods:**

#### `eth_chainId` — No Network Required
Returns the chain ID. Always `"0x1"` (mainnet).

```typescript
const chainId = await provider.request({ method: 'eth_chainId' })
// "0x1"
```

#### `eth_blockNumber` — Verified Head
Returns the latest verified head slot.

```typescript
const blockNumber = await provider.request({ method: 'eth_blockNumber' })
// "0x96b3a1" (hex-encoded block number)
```

#### `eth_getBalance` — Locally Verified

Fetches an account's Merkle proof from the configured data source and verifies it locally via keccak256 hash chain against the verified state root. The data source (RPC or P2P) is untrusted — only the local verification matters.

```typescript
const balance = await provider.request({
  method: 'eth_getBalance',
  params: ['0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045', 'latest']
})
// "0x1234..." (balance in wei, cryptographically verified)
```

**Verification pipeline:**
1. Fetch `eth_getProof` from data source (untrusted)
2. Walk the Merkle-Patricia trie proof, verifying `keccak256(node) == expected_hash` at each step
3. Decode the RLP account data at the leaf: `[nonce, balance, storageRoot, codeHash]`
4. Return the balance only if the full proof checks out

#### `eth_getTransactionCount` — Locally Verified

Fetches and verifies an account's nonce via the same Merkle proof pipeline.

```typescript
const nonce = await provider.request({
  method: 'eth_getTransactionCount',
  params: ['0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045', 'latest']
})
```

#### `eth_getStorageAt` — Locally Verified

Fetches and verifies a contract storage slot via a two-level Merkle proof (account proof + storage proof).

```typescript
const value = await provider.request({
  method: 'eth_getStorageAt',
  params: ['0xContractAddress', '0x0', 'latest']
})
```

#### `eth_getCode` — Locally Verified

Verifies the code hash of a contract via account proof.

```typescript
const codeHash = await provider.request({
  method: 'eth_getCode',
  params: ['0xContractAddress', 'latest']
})
```

#### `eth_call` — Trusted Execution (The One Exception)

Executes a call via the fallback RPC. **NOT verified.** Requires `fallbackRpc` in options.

EVM execution cannot be proven without zero-knowledge proofs. This is the one operation where Lumen must trust an external source. A console warning is logged every time.

```typescript
const result = await provider.request({
  method: 'eth_call',
  params: [{
    to: '0xContractAddress',
    data: '0x...'
  }, 'latest']
})
// ⚠ Result is from the fallback RPC and is NOT cryptographically verified
```

#### `eth_sendRawTransaction` — Broadcast

Broadcasts a signed transaction via the fallback RPC (or P2P network when available).

```typescript
const txHash = await provider.request({
  method: 'eth_sendRawTransaction',
  params: ['0xSignedTxData']
})
```

### `provider.getSyncState()`

Returns the current sync state.

```typescript
const state = provider.getSyncState()
```

**Return Type:**

```typescript
type SyncState =
  | { status: 'bootstrapping' }
  | { status: 'syncing'; headSlot: number; targetSlot: number }
  | { status: 'synced'; headSlot: number; connectionMode: ConnectionMode }
  | { status: 'error'; message: string }
```

### `provider.onSyncStateChange(callback)`

Subscribe to sync state changes. Returns an unsubscribe function.

```typescript
const unsubscribe = provider.onSyncStateChange((state) => {
  console.log('New state:', state.status)
})

// Later:
unsubscribe()
```

### `provider.on(event, listener)` / `provider.removeListener(event, listener)`

Standard EIP-1193 event subscription.

### `provider.destroy()`

Clean up resources (stop P2P, terminate WASM worker).

---

## Framework Integration

### ethers.js v6

```typescript
import { BrowserProvider } from 'ethers'
import { createLumenProvider } from 'lumen-eth'

const lumen = await createLumenProvider()
const provider = new BrowserProvider(lumen)

// Everything works as normal, but with local Merkle proof verification
const balance = await provider.getBalance('0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045')
console.log('Balance:', balance.toString(), 'wei')
```

### viem

```typescript
import { createPublicClient, custom } from 'viem'
import { mainnet } from 'viem/chains'
import { createLumenProvider } from 'lumen-eth'

const lumen = await createLumenProvider()
const client = createPublicClient({
  chain: mainnet,
  transport: custom(lumen),
})

const balance = await client.getBalance({
  address: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
})
```

### wagmi

```typescript
import { createConfig, custom } from 'wagmi'
import { mainnet } from 'wagmi/chains'
import { createLumenProvider } from 'lumen-eth'

const lumen = await createLumenProvider()

const config = createConfig({
  chains: [mainnet],
  transports: {
    [mainnet.id]: custom(lumen),
  },
})
```

### Vanilla JavaScript (no framework)

```html
<script type="module">
import { createLumenProvider } from 'https://unpkg.com/lumen-eth/dist/index.js'

const provider = await createLumenProvider()

const balance = await provider.request({
  method: 'eth_getBalance',
  params: ['0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045', 'latest']
})

console.log('Verified balance:', balance)
</script>
```

---

## React Hook

```bash
npm install lumen-react
```

```tsx
import { useLumen } from 'lumen-react'

function App() {
  const { provider, syncState, isReady, error, reconnect } = useLumen({
    verbose: true,
  })

  if (error) {
    return (
      <div>
        <p>Error: {error.message}</p>
        <button onClick={reconnect}>Retry</button>
      </div>
    )
  }

  if (!isReady) {
    return <p>Syncing: {syncState.status}...</p>
  }

  return <p>Connected! Head: {syncState.headSlot}</p>
}
```

---

## Types

All types are exported from `lumen-eth`:

```typescript
import type {
  EIP1193Provider,
  RequestArguments,
  SyncState,
  ConnectionMode,
  LumenOptions,
  VerifiedAccountState,
  VerifiedStorageValue,
  VerificationDetails,
  VerificationStep,
  LumenEvents,
  CheckpointHash,
} from 'lumen-eth'
```

---

## Error Handling

Lumen **never silently falls back to unverified data**. If verification fails:

```typescript
try {
  const balance = await provider.request({
    method: 'eth_getBalance',
    params: ['0x...', 'latest']
  })
} catch (error) {
  // error.code: -32000 (server error) for verification failures
  // error.message: Descriptive message explaining what failed
  console.error('Verification failed:', error.message)
}
```

Error codes:
- `-32601`: Method not supported
- `-32000`: Verification failed or no data source available
- `-32602`: Invalid parameters

---

## Console Logging

When `verbose: true` (default), Lumen logs its trust state to the browser console:

```
[Lumen] Initializing trustless Ethereum light client...
[Lumen] Trust model: All data cryptographically verified via sync committee
[Lumen] Step 1/3: Fetching checkpoint from multiple sources...
[Lumen] Checkpoint verified: 3/5 sources agree
[Lumen] Step 2/3: Initializing WASM verification module...
[Lumen] Step 3/3: Starting P2P layer...
[Lumen] ✓ Initialization complete. Ready to serve trustless queries.
[Lumen] ⚠ eth_call uses trusted execution via fallback RPC
```

The demo app (`demo/main.ts`) uses a visual trust log that shows each verification step in the UI with pass/fail indicators, timing, and details about which beacon APIs were consulted and how many trie nodes were verified.
