/**
 * Core type definitions for the Lumen Ethereum light client.
 *
 * These types define the public API surface of Lumen, including:
 * - EIP-1193 provider interface
 * - Sync state types
 * - Configuration options
 * - Verification result types
 */

// --- EIP-1193 Standard Interface ---

/**
 * EIP-1193 provider interface — the standard Ethereum provider API.
 * Compatible with ethers.js, viem, wagmi, and web3.js.
 */
export interface EIP1193Provider {
  /**
   * Send a JSON-RPC request to the Ethereum network.
   * All responses are cryptographically verified unless explicitly noted.
   */
  request(args: RequestArguments): Promise<unknown>;

  /**
   * Subscribe to provider events.
   */
  on(event: string, listener: (...args: unknown[]) => void): void;

  /**
   * Remove an event listener.
   */
  removeListener(event: string, listener: (...args: unknown[]) => void): void;
}

/**
 * EIP-1193 request arguments.
 */
export interface RequestArguments {
  /** The JSON-RPC method name (e.g., "eth_getBalance"). */
  readonly method: string;
  /** The method parameters. */
  readonly params?: readonly unknown[] | object;
}

// --- Sync State ---

/**
 * The current synchronization state of the Lumen client.
 * Use this to show users the current trust state in your UI.
 */
export type SyncState =
  | { status: 'bootstrapping' }
  | { status: 'syncing'; headSlot: number; targetSlot: number }
  | { status: 'synced'; headSlot: number; connectionMode: ConnectionMode }
  | { status: 'error'; message: string };

/**
 * How we're connected to the Ethereum P2P network.
 * This directly affects the trust model — log it clearly.
 */
export type ConnectionMode =
  | 'direct-webtransport'
  | 'direct-webrtc'
  | 'relay'
  | 'disconnected';

// --- Configuration ---

/**
 * Options for creating a Lumen provider.
 */
export interface LumenOptions {
  /**
   * Override the default checkpoint hash.
   * If omitted, Lumen fetches the latest finalized checkpoint from multiple
   * independent sources and requires consensus before trusting it.
   *
   * Format: hex string with 0x prefix (e.g., "0xabcd...")
   */
  checkpoint?: string;

  /**
   * Fallback RPC URL for fetching proof data while P2P is bootstrapping.
   *
   * IMPORTANT: Lumen will ALWAYS verify proofs cryptographically regardless
   * of the source. This fallback is untrusted for correctness but speeds up
   * initial queries before P2P peers are fully connected.
   */
  fallbackRpc?: string;

  /**
   * Checkpoint sources to use for multi-source consensus.
   * Defaults to a diverse set of independent operators.
   */
  checkpointSources?: string[];

  /**
   * Number of checkpoint sources that must agree.
   * Default: 3
   */
  requiredCheckpointAgreement?: number;

  /**
   * Maximum number of P2P peers to connect to.
   * Default: 10
   */
  maxPeers?: number;

  /**
   * Enable detailed console logging of trust state transitions.
   * Default: true (developers should see what mode Lumen is in)
   */
  verbose?: boolean;
}

// --- Verification Results ---

/**
 * A verified account state — every field has been cryptographically verified
 * against the Ethereum beacon chain sync committee.
 */
export interface VerifiedAccountState {
  /** Account nonce (transaction count). */
  nonce: number;
  /** Balance in wei (hex string with 0x prefix). */
  balance: string;
  /** Root hash of the account's storage trie (hex). */
  storageRoot: string;
  /** Keccak256 hash of the account's bytecode (hex). */
  codeHash: string;
  /** Whether this is a contract account. */
  isContract: boolean;
  /** Whether this result was cryptographically verified. Always true for Lumen. */
  verified: boolean;
  /** The beacon chain slot this was verified against. */
  verifiedAgainstSlot: number;
}

/**
 * A verified storage slot value.
 */
export interface VerifiedStorageValue {
  /** The storage value (hex string with 0x prefix). */
  value: string;
  /** Whether this result was cryptographically verified. */
  verified: boolean;
  /** The beacon chain slot this was verified against. */
  verifiedAgainstSlot: number;
}

/**
 * Detailed verification result that can be displayed in UI.
 */
export interface VerificationDetails {
  /** Steps of the verification process. */
  steps: VerificationStep[];
  /** Total time taken for verification (milliseconds). */
  totalTimeMs: number;
  /** Whether all steps passed. */
  allPassed: boolean;
}

/**
 * A single step in the verification process.
 * Useful for showing users exactly what was verified and how.
 */
export interface VerificationStep {
  /** Human-readable name of this verification step. */
  name: string;
  /** Whether this step passed. */
  passed: boolean;
  /** Details about what was checked. */
  details: string;
  /** Time taken for this step (milliseconds). */
  timeMs: number;
}

// --- Events ---

/**
 * Events emitted by the Lumen provider.
 */
export interface LumenEvents {
  /** Sync state changed. */
  syncStateChange: SyncState;
  /** New verified head received. */
  headUpdate: { slot: number; stateRoot: string };
  /** Connection mode changed. */
  connectionModeChange: ConnectionMode;
  /** A verification was completed (for UI display). */
  verificationComplete: VerificationDetails;
  /** Chain ID changed (shouldn't happen on mainnet). */
  chainChanged: string;
  /** Connected accounts changed (N/A for Lumen, but required by EIP-1193). */
  accountsChanged: string[];
}

// --- Worker Messages ---

/**
 * Messages sent to the WASM Web Worker.
 */
export interface WorkerRequest {
  id: number;
  type: 'init' | 'process_update' | 'verify_account' | 'verify_storage' | 'get_state';
  payload: unknown;
}

/**
 * Messages received from the WASM Web Worker.
 */
export interface WorkerResponse {
  id: number;
  type: 'success' | 'error';
  payload: unknown;
}
