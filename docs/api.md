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
| `fallbackRpc` | `string` | None | RPC URL for `eth_call` (untrusted, verified where possible) |
| `checkpointSources` | `string[]` | 5 default sources | URLs to fetch checkpoint from |
| `requiredCheckpointAgreement` | `number` | 3 | How many sources must agree |
| `maxPeers` | `number` | 10 | Maximum P2P peer connections |
| `verbose` | `boolean` | true | Log trust state to console |

**Returns:** `Promise<LumenProvider>`

**Example:**

```typescript
const provider = await createLumenProvider({
  fallbackRpc: 'https://eth.llamarpc.com',  // For eth_call only
  verbose: true,
})
```

---

## `LumenProvider`

The main provider class. Implements EIP-1193.

### `provider.request(args)`

Standard EIP-1193 request method.

**Supported Methods:**

#### `eth_chainId` — Fully Trustless
Returns the chain ID. No network required.

```typescript
const chainId = await provider.request({ method: 'eth_chainId' })
// "0x1" (Ethereum mainnet)
```

#### `eth_blockNumber` — Fully Trustless
Returns the latest verified head slot.

```typescript
const blockNumber = await provider.request({ method: 'eth_blockNumber' })
// "0x96b3a1" (hex-encoded slot number)
```

#### `eth_getBalance` — Fully Trustless
Fetches and cryptographically verifies an account balance via Merkle proof.

```typescript
const balance = await provider.request({
  method: 'eth_getBalance',
  params: ['0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045', 'latest']
})
// "0x1234..." (balance in wei, verified)
```

**Trust:** The balance is verified via a Merkle-Patricia trie proof against the beacon chain sync committee's verified state root. No trust in any third party.

#### `eth_getTransactionCount` — Fully Trustless
Fetches and verifies an account's nonce.

```typescript
const nonce = await provider.request({
  method: 'eth_getTransactionCount',
  params: ['0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045', 'latest']
})
```

#### `eth_getStorageAt` — Fully Trustless
Fetches and verifies a contract storage slot.

```typescript
const value = await provider.request({
  method: 'eth_getStorageAt',
  params: ['0xContractAddress', '0x0', 'latest']
})
```

#### `eth_getCode` — Fully Trustless
Verifies the code hash of a contract via account proof.

```typescript
const codeHash = await provider.request({
  method: 'eth_getCode',
  params: ['0xContractAddress', 'latest']
})
```

#### `eth_call` — Trusted Execution
Executes a call via the fallback RPC. **NOT verified.** Requires `fallbackRpc` in options.

```typescript
const result = await provider.request({
  method: 'eth_call',
  params: [{
    to: '0xContractAddress',
    data: '0x...'
  }, 'latest']
})
// ⚠ This result is from the fallback RPC and is NOT cryptographically verified
```

#### `eth_sendRawTransaction` — Trustless Broadcast
Broadcasts a signed transaction to the P2P network.

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
// { status: 'synced', headSlot: 9876543, connectionMode: 'direct-webtransport' }
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

// Everything works as normal, but trustlessly verified
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
import { createConfig, http } from 'wagmi'
import { mainnet } from 'wagmi/chains'
import { createLumenProvider } from 'lumen-eth'

// In your wagmi config:
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

// Direct JSON-RPC
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

## Checkpoint Management

### `fetchConsensusCheckpoint(sources?, requiredAgreement?)`

Fetch and verify a checkpoint from multiple independent sources.

```typescript
import { fetchConsensusCheckpoint, DEFAULT_CHECKPOINT_SOURCES } from 'lumen-eth'

const checkpoint = await fetchConsensusCheckpoint(
  DEFAULT_CHECKPOINT_SOURCES,
  3  // At least 3 must agree
)

console.log(checkpoint)
// {
//   blockRoot: '0xabcd...',
//   slot: 9876543,
//   sourceAgreement: 4,
//   totalSources: 5
// }
```

### `DEFAULT_CHECKPOINT_SOURCES`

The default list of checkpoint source URLs.

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
[Lumen] Step 1/3: Fetching checkpoint from 5 sources...
[Lumen] ✓ beaconcha.in: slot 9876543
[Lumen] ✓ beaconstate.info: slot 9876543
[Lumen] ✓ checkpoint.sigp.io: slot 9876543
[Lumen] Checkpoint consensus: 3/5 sources agree
[Lumen] Step 2/3: Loading WASM module (1.4MB)...
[Lumen] Step 3/3: Starting P2P layer...
[Lumen] Connected to 3 peers via WebTransport
[Lumen] ✓ Ready. Connection: Direct WebTransport | 3 peers
[Lumen] ⚠ eth_call uses trusted execution via fallback RPC
```

This is intentional. Developers should never have to guess what trust mode Lumen is operating in.
