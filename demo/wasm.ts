/**
 * WASM Loader & Typed Interface
 *
 * Loads the lumen-wasm module and provides a typed TypeScript interface
 * to the Rust verification engine. All cryptographic operations happen
 * in Rust/WASM — this module is just the bridge.
 */

import wasmInit, {
  LumenClient,
} from '../packages/lumen-js/wasm/lumen_wasm.js';

let wasmReady = false;
let client: LumenClient | null = null;

export interface FinalityUpdateResult {
  verified: boolean;
  advanced: boolean;
  finalized_slot: number;
  execution_state_root: string;
  execution_block_number: number;
  sync_participation: number;
  message: string;
}

export interface VerifiedAccountResult {
  nonce: number;
  balance_hex: string;
  storage_root: string;
  code_hash: string;
  is_contract: boolean;
  verified: boolean;
  verified_against_slot: number;
  proof_nodes_verified: number;
  rpc_claimed_balance: string;
}

export interface ExecutionState {
  has_state_root: boolean;
  state_root: string;
  block_number: number;
  finalized_slot: number;
}

/**
 * Initialize the WASM module. Must be called before any other function.
 */
export async function initWasm(): Promise<void> {
  if (wasmReady) return;

  // The WASM file is served relative to the JS glue code.
  // Vite's dev server handles the path resolution.
  await wasmInit();
  wasmReady = true;
}

/**
 * Initialize the LumenClient from a beacon API bootstrap response.
 *
 * This is the ONE moment of trust: the bootstrap data (sync committee
 * public keys) is fetched from the beacon chain API. After this, all
 * verification is pure BLS cryptography.
 */
export function initClientFromBootstrap(bootstrapJson: string): void {
  if (!wasmReady) throw new Error('WASM not initialized');
  client = LumenClient.from_beacon_bootstrap(bootstrapJson);
}

/**
 * Process a beacon finality update through BLS verification.
 *
 * This is the core trust operation:
 * - Verifies the aggregate BLS12-381 signature from 512 sync committee members
 * - Verifies the finality Merkle branch
 * - Extracts the BLS-verified execution state root
 *
 * After this succeeds, we have a cryptographically verified state root
 * to verify all Merkle-Patricia trie proofs against.
 */
export function processFinalityUpdate(
  updateJson: string,
): FinalityUpdateResult {
  if (!client) throw new Error('Client not initialized');
  return client.process_finality_update(updateJson) as FinalityUpdateResult;
}

/**
 * Verify an account proof from a raw eth_getProof RPC response.
 *
 * The proof data is UNTRUSTED. It is verified against the BLS-verified
 * execution state root using keccak256 Merkle-Patricia trie traversal
 * in Rust/WASM.
 */
export function verifyAccountProof(
  address: string,
  rpcProofJson: string,
): VerifiedAccountResult {
  if (!client) throw new Error('Client not initialized');
  return client.verify_account_rpc_proof(
    address,
    rpcProofJson,
  ) as VerifiedAccountResult;
}

/**
 * Verify an account proof against an EXPLICIT state root.
 *
 * Race-condition-safe: pass the state root captured at the same moment
 * as the block number. Even if the background worker advances the WASM
 * state during the async proof fetch, this uses the originally-captured root.
 */
export function verifyAccountProofWithRoot(
  stateRootHex: string,
  address: string,
  rpcProofJson: string,
): VerifiedAccountResult {
  if (!client) throw new Error('Client not initialized');
  return client.verify_account_rpc_proof_with_root(
    stateRootHex,
    address,
    rpcProofJson,
  ) as VerifiedAccountResult;
}

/**
 * Get the current execution state (state root, block number, etc.)
 */
export function getExecutionState(): ExecutionState {
  if (!client) throw new Error('Client not initialized');
  return client.get_execution_state() as ExecutionState;
}

/**
 * Get the current verified head slot.
 */
export function getHeadSlot(): number {
  if (!client) throw new Error('Client not initialized');
  return Number(client.head_slot());
}

/**
 * Check if the client is synced (has processed at least one update).
 */
export function isSynced(): boolean {
  if (!client) return false;
  return client.is_synced();
}

/**
 * Check if WASM is loaded and client is initialized.
 */
export function isReady(): boolean {
  return wasmReady && client !== null;
}
