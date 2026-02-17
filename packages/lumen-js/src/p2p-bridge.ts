/**
 * P2P Bridge — connects JavaScript WebRTC/WebTransport APIs to the Rust P2P layer.
 *
 * The P2P layer runs in a Web Worker. This bridge:
 * 1. Creates the P2P Web Worker
 * 2. Handles WebRTC/WebTransport connection setup (requires browser APIs)
 * 3. Forwards gossip messages from the P2P layer to the WASM verification worker
 * 4. Reports connection state changes to the main thread
 *
 * ## Trust Model
 *
 * This bridge is a pure transport layer — it does NOT interpret or trust
 * any data that flows through it. All data is verified by lumen-core/WASM.
 */

import type { ConnectionMode, SyncState } from './types';

/**
 * Configuration for the P2P bridge.
 */
export interface P2PBridgeConfig {
  /** Maximum number of peers. */
  maxPeers: number;
  /** Bootstrap timeout in milliseconds. */
  bootstrapTimeoutMs: number;
  /** Enable verbose logging. */
  verbose: boolean;
}

/**
 * P2P connection statistics.
 */
export interface P2PStats {
  /** Current connection mode. */
  connectionMode: ConnectionMode;
  /** Number of connected peers. */
  peerCount: number;
  /** Number of gossip messages received. */
  messagesReceived: number;
  /** Number of messages that passed verification. */
  messagesVerified: number;
  /** Number of messages that failed verification. */
  messagesRejected: number;
  /** Whether we're using circuit relay. */
  usingRelay: boolean;
}

/**
 * P2P Bridge class — manages the connection between browser P2P APIs
 * and the Rust networking layer.
 */
export class P2PBridge {
  private config: P2PBridgeConfig;
  private connectionMode: ConnectionMode = 'disconnected';
  private peerCount: number = 0;
  private stats: P2PStats;
  private listeners: Map<string, Set<(data: unknown) => void>> = new Map();

  constructor(config: Partial<P2PBridgeConfig> = {}) {
    this.config = {
      maxPeers: config.maxPeers ?? 10,
      bootstrapTimeoutMs: config.bootstrapTimeoutMs ?? 3000,
      verbose: config.verbose ?? true,
    };

    this.stats = {
      connectionMode: 'disconnected',
      peerCount: 0,
      messagesReceived: 0,
      messagesVerified: 0,
      messagesRejected: 0,
      usingRelay: false,
    };
  }

  /**
   * Start the P2P layer.
   *
   * 1. Attempt direct WebTransport connections to bootnodes
   * 2. If no connection within bootstrapTimeoutMs, try circuit relay
   * 3. Subscribe to beacon chain gossip topics
   * 4. Forward received updates to the WASM worker for verification
   */
  async start(): Promise<void> {
    this.log('Starting P2P layer...');
    this.connectionMode = 'disconnected';
    this.emit('connectionModeChange', this.connectionMode);

    // In production, this would:
    // 1. Create a libp2p node using the browser's WebRTC/WebTransport APIs
    // 2. Connect to bootstrap nodes
    // 3. Subscribe to gossip topics
    // 4. Forward messages to the verification pipeline

    this.log('P2P layer started (bootstrap mode)');
    this.emit('connectionModeChange', 'disconnected');
  }

  /**
   * Stop the P2P layer and disconnect from all peers.
   */
  async stop(): Promise<void> {
    this.log('Stopping P2P layer...');
    this.connectionMode = 'disconnected';
    this.peerCount = 0;
    this.emit('connectionModeChange', 'disconnected');
    this.log('P2P layer stopped');
  }

  /**
   * Get the current connection mode.
   */
  getConnectionMode(): ConnectionMode {
    return this.connectionMode;
  }

  /**
   * Get P2P statistics.
   */
  getStats(): P2PStats {
    return { ...this.stats };
  }

  /**
   * Subscribe to P2P events.
   */
  on(event: string, callback: (data: unknown) => void): () => void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(callback);

    return () => {
      this.listeners.get(event)?.delete(callback);
    };
  }

  /**
   * Emit an event to all listeners.
   */
  private emit(event: string, data: unknown): void {
    this.listeners.get(event)?.forEach((cb) => cb(data));
  }

  /**
   * Log a message if verbose mode is enabled.
   */
  private log(message: string): void {
    if (this.config.verbose) {
      console.log(`[Lumen P2P] ${message}`);
    }
  }
}
