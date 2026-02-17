/**
 * # Lumen — Trustless Ethereum Light Client for the Browser
 *
 * The first fully trustless Ethereum light client that runs in any browser —
 * no extension, no server, no trust required.
 *
 * ## Quick Start
 *
 * ```typescript
 * import { createLumenProvider } from 'lumen-eth'
 *
 * // Create a trustless provider (one-liner replacement for Infura/Alchemy)
 * const provider = await createLumenProvider()
 *
 * // Use with ethers.js
 * const ethersProvider = new ethers.BrowserProvider(provider)
 * const balance = await ethersProvider.getBalance("0x...")
 * // ^ This balance was cryptographically verified. No trust required.
 * ```
 *
 * ## Trust Guarantees
 *
 * | Method | Trust Level |
 * |--------|------------|
 * | eth_getBalance | Fully trustless (Merkle proof verified) |
 * | eth_getCode | Fully trustless (Merkle proof verified) |
 * | eth_getStorageAt | Fully trustless (storage proof verified) |
 * | eth_getTransactionCount | Fully trustless (Merkle proof verified) |
 * | eth_call | ⚠ Trusted execution (requires fallback RPC) |
 *
 * @module
 */

// Main provider API
export { LumenProvider, createLumenProvider } from './provider';

// Checkpoint management
export {
  fetchConsensusCheckpoint,
  DEFAULT_CHECKPOINT_SOURCES,
} from './checkpoint';
export type { CheckpointHash } from './checkpoint';

// P2P bridge
export { P2PBridge } from './p2p-bridge';
export type { P2PBridgeConfig, P2PStats } from './p2p-bridge';

// WASM loader
export {
  initWasmWorker,
  sendToWorker,
  terminateWasmWorker,
  isWasmWorkerReady,
} from './wasm-loader';

// Types
export type {
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
  WorkerRequest,
  WorkerResponse,
} from './types';
