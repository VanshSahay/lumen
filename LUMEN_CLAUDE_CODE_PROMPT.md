# Lumen — Trustless Ethereum Light Client in the Browser
## Claude Code Build Prompt

---

> **Project Name:** Lumen
> **Tagline:** The first fully trustless Ethereum light client that runs in any browser — no extension, no server, no trust required.
> **Stack:** Rust (WASM core) + TypeScript (JS layer) + libp2p (P2P networking)
> **Starting point:** Empty directory

---

## Who You Are

You are a senior Rust/WebAssembly engineer and co-founder of Lumen. You are building this from scratch in an empty directory. You care deeply about:

- **Correctness over speed** — every cryptographic operation must be verified, never assumed
- **Developer experience** — a dApp developer should be able to drop Lumen in with one import and zero config
- **Honest trust model** — document exactly what is and isn't trusted at every layer, no marketing speak
- **Minimal footprint** — the WASM binary must be small enough to be embedded in a web page without destroying load time

You are not building a demo. You are building something production-ready, well-tested, and documented well enough that another engineer could pick it up on day one.

---

## Project Overview

### The Problem

Almost every "decentralized" application today secretly trusts a centralized RPC provider — Infura, Alchemy, QuickNode — to tell it what the Ethereum blockchain state is. These providers can:

- Lie about account balances
- Censor transactions
- Return fake contract state
- Go down, taking the dApp with them

This is Web3's biggest architectural lie. The blockchain is decentralized. The apps built on it are not.

### The Solution

Lumen is a light client that runs entirely inside the browser as a WebAssembly module. It:

1. **Connects directly to Ethereum's P2P network** via WebRTC and WebTransport (no RPC middleman)
2. **Verifies every piece of data cryptographically** — block headers via BLS signature verification against Ethereum's sync committee, account state and transaction data via Merkle-Patricia trie proofs
3. **Exposes a standard EIP-1193 provider interface** so any existing dApp can swap it in with one line
4. **Requires no login, no extension, no account** — it initializes on page load and is ready in under 10 seconds

### Trust Model (be precise about this)

| Layer | What Lumen trusts | Attack surface |
|---|---|---|
| Cryptography | Standard assumptions (BLS12-381, keccak256) | Theoretical only |
| Sync committee | That 2/3+ of Ethereum's 512-member sync committee is honest | Same assumption as trusting Ethereum itself |
| P2P peers | Nothing — all data is verified against committee signatures | Peers can lie, Lumen will reject it |
| Circuit relay (bootstrap only) | For peer *introductions* only, not data | Can introduce bad peers, cannot forge proofs |
| Checkpoint | Multi-peer consensus on initial checkpoint hash | Requires collusion of N bootstrap peers |

---

## Repository Structure

Build the following directory structure from scratch:

```
lumen/
├── crates/
│   ├── lumen-core/          # Pure Rust: verification logic, no networking
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── consensus/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── sync_committee.rs   # BLS verification of sync committee signatures
│   │   │   │   ├── light_client.rs     # Light client update processing
│   │   │   │   └── checkpoint.rs       # Checkpoint sync logic
│   │   │   ├── execution/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── proof.rs            # Merkle-Patricia trie proof verification
│   │   │   │   ├── account.rs          # Account state verification
│   │   │   │   └── receipt.rs          # Transaction receipt verification
│   │   │   └── types/
│   │   │       ├── mod.rs
│   │   │       ├── beacon.rs           # Beacon chain types (LightClientUpdate, etc.)
│   │   │       └── execution.rs        # Execution layer types
│   │   └── Cargo.toml
│   │
│   ├── lumen-wasm/          # WASM bindings: bridges core to JS
│   │   ├── src/
│   │   │   ├── lib.rs               # wasm-bindgen entrypoints
│   │   │   ├── provider.rs          # EIP-1193 provider implementation
│   │   │   ├── network.rs           # Async fetch/WebSocket abstraction (WASM-safe)
│   │   │   └── state.rs             # In-memory verified chain state
│   │   └── Cargo.toml
│   │
│   └── lumen-p2p/           # P2P layer: libp2p WebRTC + WebTransport
│       ├── src/
│       │   ├── lib.rs
│       │   ├── transport.rs         # WebRTC + WebTransport transport setup
│       │   ├── behaviour.rs         # libp2p network behaviour (gossipsub, identify, etc.)
│       │   ├── bootstrap.rs         # Hardcoded bootnode multiaddrs + peer discovery
│       │   ├── relay.rs             # Circuit relay client logic
│       │   └── beacon_gossip.rs     # Subscribe to beacon chain gossip topics
│       └── Cargo.toml
│
├── packages/
│   ├── lumen-js/            # TypeScript npm package
│   │   ├── src/
│   │   │   ├── index.ts             # Main entrypoint, re-exports everything
│   │   │   ├── provider.ts          # EIP-1193 LumenProvider class
│   │   │   ├── wasm-loader.ts       # Lazy WASM initialization
│   │   │   ├── p2p-bridge.ts        # Bridges JS WebRTC APIs to Rust P2P layer
│   │   │   ├── checkpoint.ts        # Checkpoint management and consensus
│   │   │   └── types.ts             # TypeScript type definitions
│   │   ├── package.json
│   │   ├── tsconfig.json
│   │   └── README.md
│   │
│   └── lumen-react/         # Optional React hook package
│       ├── src/
│       │   ├── index.ts
│       │   └── useLumen.ts          # useLumen() React hook
│       └── package.json
│
├── demo/                    # Demo web app showing Lumen in action
│   ├── index.html
│   ├── main.ts
│   └── vite.config.ts
│
├── tests/
│   ├── integration/         # Integration tests running against real Ethereum data
│   └── fixtures/            # Known-good block headers, proofs, checkpoints for tests
│
├── docs/
│   ├── architecture.md      # Deep dive on the trust model and system design
│   ├── trust-model.md       # Precise documentation of what is and isn't trusted
│   └── api.md               # JS/TS API reference
│
├── Cargo.toml               # Workspace root
├── package.json             # Monorepo root (pnpm workspaces)
├── pnpm-workspace.yaml
└── README.md
```

---

## Phase 1: Rust Core (`lumen-core`)

### Goal
A pure Rust crate with **zero networking** that can:
- Verify Ethereum beacon chain light client updates (BLS signature verification)
- Process and store sync committee state
- Verify Merkle-Patricia trie proofs for execution layer data (accounts, storage, receipts)
- Checkpoint sync from a trusted initial block hash

### Dependencies to use in `lumen-core/Cargo.toml`

```toml
[dependencies]
# BLS signature verification for sync committee
blst = "0.3"                    # Fast BLS12-381, compiles to WASM

# Ethereum types and RLP encoding
alloy-primitives = "0.7"        # H256, Address, U256, Bloom etc.
alloy-rlp = "0.3"               # RLP encoding/decoding

# Merkle Patricia Trie verification
eth-trie = "0.4"                # MPT proof verification

# SSZ encoding (beacon chain uses SSZ, not RLP)
ssz_rs = "0.9"                  # SSZ serialization for beacon types

# SHA256 / keccak for hashing
sha2 = { version = "0.10", features = ["asm"] }
tiny-keccak = { version = "2.0", features = ["keccak"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

[dev-dependencies]
hex-literal = "0.4"
```

### Key types to implement in `lumen-core/src/types/beacon.rs`

```rust
/// A light client update from the beacon chain.
/// This is what peers send us to update our view of the chain head.
pub struct LightClientUpdate {
    pub attested_header: BeaconBlockHeader,
    pub next_sync_committee: Option<SyncCommittee>,
    pub next_sync_committee_branch: Vec<[u8; 32]>,
    pub finalized_header: BeaconBlockHeader,
    pub finality_branch: Vec<[u8; 32]>,
    pub sync_aggregate: SyncAggregate,
    pub signature_slot: u64,
}

/// The sync committee — 512 validators that sign off on the chain head
pub struct SyncCommittee {
    pub pubkeys: Vec<BlsPublicKey>,       // 512 BLS public keys
    pub aggregate_pubkey: BlsPublicKey,   // Aggregated for fast verification
}

/// The aggregate BLS signature from the sync committee
pub struct SyncAggregate {
    pub sync_committee_bits: Bitvector<512>,  // Which of 512 signed
    pub sync_committee_signature: BlsSignature,
}
```

### Key verification functions to implement

```rust
// In lumen-core/src/consensus/sync_committee.rs

/// Verify a sync committee signature against a beacon block header.
/// This is the core trust anchor — if this passes, the header is legitimate.
/// 
/// Requires >= 2/3 of the 512 sync committee members to have signed.
/// Uses BLS signature aggregation — we verify one aggregate sig, not 512 individual ones.
pub fn verify_sync_committee_signature(
    update: &LightClientUpdate,
    current_sync_committee: &SyncCommittee,
    genesis_validators_root: [u8; 32],
    fork_version: [u8; 4],
) -> Result<(), VerificationError>

/// Process a light client update, verifying all proofs and advancing state.
/// Returns the new verified head if valid, error if anything doesn't check out.
pub fn process_light_client_update(
    state: &mut LightClientState,
    update: &LightClientUpdate,
    current_slot: u64,
    genesis_validators_root: [u8; 32],
) -> Result<LightClientState, VerificationError>
```

```rust
// In lumen-core/src/execution/proof.rs

/// Verify an account proof against a known state root.
/// The state root comes from a verified execution payload header.
/// This lets us prove balance, nonce, code hash, and storage root of any account.
pub fn verify_account_proof(
    state_root: [u8; 32],
    address: [u8; 20],
    proof: &AccountProof,  // RLP-encoded Merkle-Patricia trie proof nodes
) -> Result<AccountState, ProofError>

/// Verify a storage proof for a specific storage slot of a contract.
pub fn verify_storage_proof(
    storage_root: [u8; 32],
    slot: [u8; 32],
    proof: &StorageProof,
) -> Result<[u8; 32], ProofError>

/// Verify a transaction receipt proof.
pub fn verify_receipt_proof(
    receipts_root: [u8; 32],
    tx_index: u64,
    proof: &ReceiptProof,
) -> Result<TransactionReceipt, ProofError>
```

### What to test in Phase 1

Write integration tests in `tests/integration/` using **real Ethereum mainnet data**:

- Use a known finalized block (hardcode the slot number and expected state root)
- Download the actual light client update for that slot from a public beacon API
- Run it through `verify_sync_committee_signature` — it must pass
- Mutate one byte of the signature — it must fail
- Use a known account address (e.g. the Ethereum Foundation's address) at a known block
- Verify their balance using a real Merkle proof — it must match what Etherscan shows
- Mutate one node in the proof — it must fail

**This phase is complete when all tests pass against real mainnet data. Do not move to Phase 2 until this is solid.**

---

## Phase 2: WASM Bindings (`lumen-wasm`)

### Goal
Compile `lumen-core` to WebAssembly and expose it to JavaScript via `wasm-bindgen`. The WASM module should be:
- Under 2MB gzipped (profile carefully, this matters)
- Fully async-compatible (use `wasm-bindgen-futures`)
- Safe to run in a Web Worker (no main thread blocking)

### Dependencies

```toml
[dependencies]
lumen-core = { path = "../lumen-core" }

wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
    "Window",
    "WorkerGlobalScope", 
    "Request",
    "Response",
    "Headers",
    "ReadableStream",
    "WebSocket",
    "MessageEvent",
    "console",
] }

serde-wasm-bindgen = "0.6"
serde_json = "1.0"
getrandom = { version = "0.2", features = ["js"] }  # WASM-safe RNG
console_error_panic_hook = "0.1"

[lib]
crate-type = ["cdylib"]
```

### Key WASM-bindgen exports

```rust
// In lumen-wasm/src/lib.rs

#[wasm_bindgen(start)]
pub fn init() {
    // Set up panic hook so panics show up in browser console
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct LumenClient {
    state: LightClientState,
    // Internal state, not exposed to JS
}

#[wasm_bindgen]
impl LumenClient {
    /// Initialize a new Lumen client from a checkpoint.
    /// checkpoint_hash: the hex-encoded hash of a known finalized block
    /// This is the only moment of trust — the checkpoint must be obtained
    /// from multiple independent sources before calling this.
    #[wasm_bindgen(constructor)]
    pub fn new(checkpoint_hash: &str) -> Result<LumenClient, JsValue>

    /// Process a light client update received from a peer.
    /// Returns true if the update was valid and state advanced.
    /// Returns false if the update was invalid (don't disconnect peer, just ignore).
    pub fn process_update(&mut self, update_ssz: &[u8]) -> Result<bool, JsValue>

    /// Get the current verified head slot number
    pub fn head_slot(&self) -> u64

    /// Get the current verified state root (hex encoded)
    pub fn state_root(&self) -> String

    /// Verify an account proof and return account state as JSON.
    /// address: hex-encoded Ethereum address (0x...)
    /// proof_json: JSON-encoded eth_getProof response from any source
    /// 
    /// IMPORTANT: the proof is verified against our internally held state root.
    /// The caller cannot pass in a fake state root — we use our verified one.
    pub fn verify_account(&self, address: &str, proof_json: &str) -> Result<JsValue, JsValue>

    /// Verify a storage proof for a contract slot
    pub fn verify_storage(
        &self, 
        address: &str, 
        slot: &str, 
        proof_json: &str
    ) -> Result<JsValue, JsValue>

    /// Returns true if the client has synced past a given slot and is ready to serve queries
    pub fn is_synced(&self) -> bool
}
```

### Network abstraction for WASM

The core networking in the browser must use the Web Fetch API and WebSockets, not TCP. Create an abstraction:

```rust
// In lumen-wasm/src/network.rs

/// Fetch a URL and return bytes. Uses the browser Fetch API via web-sys.
/// This is used ONLY for initial checkpoint fetching from multiple sources.
/// After P2P is established, this is no longer used.
pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, NetworkError>

/// Open a WebSocket connection. Used for libp2p WebSocket transport.
pub fn open_websocket(url: &str) -> Result<WebSocketStream, NetworkError>
```

### Build configuration

Create `lumen-wasm/.cargo/config.toml`:

```toml
[build]
target = "wasm32-unknown-unknown"

[profile.release]
opt-level = "z"      # Optimize for size, not speed
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization at cost of compile time
panic = "abort"      # Smaller binary, no unwinding
strip = true         # Strip debug symbols
```

Create a `build.sh` that:
1. Runs `wasm-pack build --target web --release`
2. Runs `wasm-opt -Oz` (Binaryen optimizer) on the output
3. Reports the final gzipped size
4. Fails the build if the gzipped WASM exceeds 2MB

---

## Phase 3: P2P Layer (`lumen-p2p`)

### Goal
Connect the browser to Ethereum's actual P2P network using libp2p's browser-compatible transports (WebRTC and WebTransport). This layer:

- Discovers and connects to Ethereum full nodes that support WebTransport
- Subscribes to beacon chain gossip topics to receive new light client updates
- Passes verified updates to `lumen-wasm` for processing
- Handles peer discovery, scoring, and reconnection

### Important architecture note

The P2P layer runs in a **Web Worker**, not the main thread. All crypto verification (in `lumen-wasm`) also runs in a Web Worker. The main thread only receives final verified results. This is non-negotiable for performance.

### Dependencies

```toml
[dependencies]
# libp2p with browser-compatible transports
libp2p = { version = "0.54", features = [
    "webrtc-websys",     # WebRTC using browser WebRTC APIs
    "webtransport-websys", # WebTransport using browser WebTransport API
    "gossipsub",         # For beacon chain gossip
    "identify",          # Peer identification
    "ping",              # Keep-alive
    "noise",             # Encrypted transport
    "yamux",             # Stream multiplexing
    "dns",               # DNS resolution for bootnode addresses
] }

wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = { version = "0.3", features = ["RtcPeerConnection", ...] }

# Async runtime for WASM
tokio = { version = "1", features = ["sync", "time"], default-features = false }
futures = "0.3"

serde = { version = "1.0", features = ["derive"] }
```

### Hardcoded Ethereum bootnodes that support WebTransport

Research and hardcode real Ethereum mainnet bootnode multiaddrs that speak WebTransport. At minimum include:

```rust
// In lumen-p2p/src/bootstrap.rs

/// Ethereum mainnet bootnodes with WebTransport support.
/// These are used only for initial peer discovery — once we have peers,
/// we use the libp2p DHT and peer exchange to find more.
/// 
/// These nodes are trusted ONLY for peer introductions, not for data.
/// All data received from any peer is cryptographically verified independently.
pub const ETHEREUM_BOOTNODES_WEBTRANSPORT: &[&str] = &[
    // Research and fill in real multiaddrs here from:
    // https://github.com/eth-clients/mainnet/blob/main/metadata/bootstrap_nodes.yaml
    // Filter for nodes that advertise /webtransport in their multiaddr
];

/// Public libp2p circuit relays (from the IPFS/libp2p public infrastructure)
/// Used as fallback if no direct WebTransport peers are immediately reachable.
/// These are operated by Protocol Labs and the libp2p community.
pub const PUBLIC_CIRCUIT_RELAYS: &[&str] = &[
    // Research and fill in from: https://github.com/libp2p/js-libp2p/blob/main/doc/CONFIGURATION.md
];
```

### Beacon chain gossip topics to subscribe to

```rust
// In lumen-p2p/src/beacon_gossip.rs

/// The gossip topic for light client finality updates.
/// This is the main feed of new verified chain heads.
pub const LIGHT_CLIENT_FINALITY_UPDATE_TOPIC: &str = 
    "/eth2/b5303f2a/light_client_finality_update/ssz_snappy";

/// Optimistic updates arrive faster (before finality) — useful for lower latency
pub const LIGHT_CLIENT_OPTIMISTIC_UPDATE_TOPIC: &str = 
    "/eth2/b5303f2a/light_client_optimistic_update/ssz_snappy";

/// Subscribe to these topics and forward decoded SSZ bytes to the WASM core for verification
pub async fn subscribe_to_beacon_gossip(
    swarm: &mut Swarm<LumenBehaviour>,
    on_update: impl Fn(Vec<u8>),  // Raw SSZ bytes — WASM core verifies them
)
```

### Circuit relay strategy

```rust
// In lumen-p2p/src/relay.rs

/// Bootstrap strategy:
/// 1. Try to connect directly to WebTransport bootnodes
/// 2. If none reachable within 3 seconds, fall back to circuit relay
/// 3. Once connected via relay, do peer exchange to find WebTransport-capable peers
/// 4. Upgrade to direct connections, drop relay dependency
/// 5. Log clearly to console which mode we're in so developers can see the trust state

pub enum ConnectionMode {
    /// Connected directly via WebTransport — fully P2P, no intermediary
    DirectWebTransport { peer_count: usize },
    /// Connected via circuit relay — relay sees metadata, not data
    ViaRelay { relay_peer: PeerId, direct_peers: usize },
    /// Bootstrapping — not yet connected
    Bootstrapping,
}
```

---

## Phase 4: TypeScript Package (`lumen-js`)

### Goal
A clean, well-typed TypeScript package that any dApp developer can use. The API must be so simple that adding trustless verification requires changing **one line of code** in an existing dApp.

### The one-line promise

**Before Lumen (trusting Infura):**
```typescript
const provider = new ethers.JsonRpcProvider("https://mainnet.infura.io/v3/YOUR_KEY")
```

**After Lumen (trustless):**
```typescript
const provider = await createLumenProvider()
```

That's it. Same `ethers.Provider` interface. Drop-in replacement. This is the core product promise and every API decision should serve it.

### `packages/lumen-js/src/provider.ts`

```typescript
import { EIP1193Provider, RequestArguments } from './types'

/**
 * LumenProvider implements EIP-1193 — the standard Ethereum provider interface.
 * 
 * It is compatible with ethers.js, viem, wagmi, and web3.js.
 * All data returned has been cryptographically verified against the Ethereum
 * beacon chain sync committee. No trust in any third party is required.
 */
export class LumenProvider implements EIP1193Provider {
  private wasmClient: LumenWasmClient  // The Rust WASM core
  private p2pWorker: Worker            // Web Worker running the P2P layer
  private syncState: SyncState

  /**
   * Create and initialize a Lumen provider.
   * 
   * @param options.checkpoint - Optional: override the default checkpoint.
   *   If omitted, Lumen fetches the latest finalized checkpoint from multiple
   *   independent sources and requires consensus before trusting it.
   *   
   * @param options.fallbackRpc - Optional: an RPC URL to use for fetching
   *   proof data while P2P is bootstrapping. Lumen will ALWAYS verify proofs
   *   cryptographically regardless of source. The fallback is untrusted for
   *   correctness but speeds up initial load.
   */
  static async create(options?: LumenOptions): Promise<LumenProvider>

  /**
   * EIP-1193 request method — the standard Ethereum provider interface.
   * 
   * Supported methods and their trust guarantees:
   * 
   * eth_blockNumber          → Returns verified head slot. Fully trustless.
   * eth_getBalance           → Fetches + verifies MPT proof. Fully trustless.
   * eth_getCode              → Fetches + verifies MPT proof. Fully trustless.
   * eth_getStorageAt         → Fetches + verifies storage proof. Fully trustless.
   * eth_getTransactionCount  → Fetches + verifies MPT proof. Fully trustless.
   * eth_call                 → WARNING: requires trusted execution. See docs.
   * eth_sendRawTransaction   → Broadcasts to P2P network. Trustless broadcast.
   * 
   * Note on eth_call: executing EVM code in-browser requires an EVM interpreter.
   * This version fetches eth_call results from a configurable RPC endpoint.
   * Results are NOT verified (EVM execution is not provable without zk-proofs).
   * This is documented clearly and is the only trust exception in Lumen.
   */
  async request(args: RequestArguments): Promise<unknown>

  /**
   * Returns the current sync state so dApps can show users what's happening
   */
  getSyncState(): SyncState

  /**
   * Subscribe to sync state changes
   */
  onSyncStateChange(callback: (state: SyncState) => void): () => void
}

export type SyncState = 
  | { status: 'bootstrapping' }
  | { status: 'syncing'; headSlot: number; targetSlot: number }
  | { status: 'synced'; headSlot: number; connectionMode: 'direct' | 'relay' }
  | { status: 'error'; message: string }

/**
 * Create a LumenProvider with sensible defaults.
 * This is the one-liner API that most developers should use.
 */
export async function createLumenProvider(options?: LumenOptions): Promise<LumenProvider>
```

### Checkpoint consensus in `packages/lumen-js/src/checkpoint.ts`

```typescript
/**
 * Fetch the latest finalized checkpoint from multiple independent sources
 * and require that N of them agree before trusting it.
 * 
 * Sources are a mix of:
 * - Public beacon chain checkpoint APIs (beaconcha.in, etc.)
 * - P2P peers (once connected)
 * 
 * This is the only moment of "soft trust" in Lumen's lifecycle.
 * Once past this point, all verification is purely cryptographic.
 */
export async function fetchConsensusCheckpoint(
  sources: string[],
  requiredAgreement: number = 3,  // At least 3 sources must agree
): Promise<CheckpointHash>

// Default checkpoint sources — a diverse set of independent operators
export const DEFAULT_CHECKPOINT_SOURCES = [
  'https://beaconcha.in/api/v1/slot/finalized',
  'https://beaconstate.info',
  'https://mainnet.checkpoint.sigp.io',
  // Add more — diversity of operators matters here
]
```

### Package.json for `lumen-js`

```json
{
  "name": "lumen-eth",
  "version": "0.1.0",
  "description": "Trustless Ethereum light client for the browser",
  "type": "module",
  "main": "./dist/index.js",
  "types": "./dist/index.d.ts",
  "exports": {
    ".": {
      "import": "./dist/index.js",
      "types": "./dist/index.d.ts"
    }
  },
  "files": ["dist", "wasm"],
  "scripts": {
    "build:wasm": "cd ../../crates/lumen-wasm && wasm-pack build --target web --release",
    "build:ts": "tsc",
    "build": "npm run build:wasm && npm run build:ts",
    "test": "vitest"
  }
}
```

---

## Phase 5: Demo App

Build a demo at `/demo` that shows Lumen working. It must demonstrate:

1. **Live sync status** — show the current head slot updating in real time as new blocks arrive via P2P gossip
2. **Account verification** — let the user type any Ethereum address and see their balance, fetched and cryptographically verified, with a step-by-step breakdown of the proof verification shown in the UI
3. **Connection mode indicator** — clearly show whether Lumen is connected via direct WebTransport or via circuit relay, with an explanation of what each means for the trust model
4. **No RPC key required** — the demo must work with zero configuration, no API keys, no sign-up

The demo should be buildable with:
```bash
cd demo && pnpm install && pnpm dev
```

And it should open in a browser and work, end to end, on Ethereum mainnet.

---

## Phase 6: Documentation

Write the following docs. These are real docs, not placeholder text:

### `docs/architecture.md`

Cover:
- Why existing dApps have a centralization problem
- How beacon chain light clients work (sync committee, BLS aggregation, Merkle proofs)
- Why WASM in the browser is the right deployment target
- The WebRTC/WebTransport P2P strategy and why TCP isn't available in browsers
- The circuit relay bootstrap tradeoff and why it's acceptable
- The one trust exception (eth_call) and why it exists

### `docs/trust-model.md`

This is the most important document. Be brutally honest. For every component, answer:
- What does this component trust?
- Under what attack scenario would that trust be exploited?
- What is the probability and impact of that attack?
- What would it take to eliminate that trust assumption entirely?

Do not soften this. Sophisticated developers will read it and will know if you're being evasive.

### `docs/api.md`

Full API reference for `lumen-eth`. Every exported function, class, and type. Include working code examples for:
- ethers.js v6 integration
- viem integration
- wagmi integration
- Vanilla JS with no framework

---

## Non-Negotiables

These are hard requirements. Do not skip them:

1. **All cryptographic verification must happen in Rust/WASM, never in JavaScript.** JavaScript BigInt and floating point are not suitable for consensus-critical math.

2. **The WASM module must run in a Web Worker.** Never block the main thread. The provider API communicates with the worker via `postMessage`.

3. **Every public API must have JSDoc comments** that explain the trust guarantees of that specific method.

4. **The build must be reproducible.** Pin all dependency versions. Document the exact toolchain (rustup toolchain version, wasm-pack version, node version).

5. **The demo must work on mainnet, not just a testnet.** Testnets are fine for development but the demo must prove the real thing works.

6. **Never silently fall back to an unverified data source.** If verification fails or P2P is unavailable, throw a clear error. Let the developer decide how to handle it — don't quietly return unverified data.

7. **Log the trust state clearly.** On initialization, log to console exactly what mode Lumen is running in, what checkpoint was used, how many sources agreed on it, and how many peers are connected. Developers should never have to guess.

---

## Getting Started — First Commands to Run

```bash
# Create the workspace
mkdir lumen && cd lumen
git init

# Set up Rust workspace
cargo init --name lumen-workspace

# Install toolchain
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
cargo install wasm-opt

# Set up Node monorepo
pnpm init
# configure pnpm-workspace.yaml

# Create crate directories
cargo new --lib crates/lumen-core
cargo new --lib crates/lumen-wasm  
cargo new --lib crates/lumen-p2p

# Start building lumen-core first — no WASM, no networking, just pure Rust
# Get the verification logic right before adding complexity
```

---

## Definition of Done

The project is complete when:

- [ ] `cargo test` passes for `lumen-core` with real mainnet data
- [ ] `wasm-pack build` succeeds and produces a WASM binary under 2MB gzipped
- [ ] The demo app opens in Chrome/Firefox with no config and displays a live verified head slot
- [ ] The demo can verify an account balance end-to-end, showing the proof
- [ ] The P2P layer connects to at least one real Ethereum peer via WebTransport or WebRTC
- [ ] `npm install lumen-eth` works and the README one-liner example runs
- [ ] All docs are written (not placeholder text)
- [ ] `pnpm build` from the root builds everything cleanly

---

*Good luck. This is genuinely hard and genuinely important. The browser light client is the missing piece that would make Web3's trust model honest. Build it right.*
