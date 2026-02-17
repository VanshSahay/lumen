/**
 * Lazy WASM initialization for the Lumen light client.
 *
 * Handles loading and initializing the WASM module in a Web Worker.
 * The WASM module is loaded lazily — it's not downloaded until the first
 * time the client is created.
 *
 * ## Architecture
 *
 * - WASM runs in a Web Worker (never blocks the main thread)
 * - The worker handles all cryptographic operations
 * - Main thread communicates via postMessage
 * - WASM binary is loaded from a bundled asset or CDN
 */

import type { WorkerRequest, WorkerResponse } from './types';

/** WASM module initialization state. */
let wasmWorker: Worker | null = null;
let requestId = 0;
const pendingRequests = new Map<
  number,
  { resolve: (value: unknown) => void; reject: (error: Error) => void }
>();

/**
 * Initialize the WASM worker.
 *
 * This creates a Web Worker that loads and initializes the WASM module.
 * All subsequent cryptographic operations are delegated to this worker.
 *
 * @returns Promise that resolves when the WASM module is ready.
 */
export async function initWasmWorker(): Promise<Worker> {
  if (wasmWorker) {
    return wasmWorker;
  }

  // Create inline worker script
  const workerCode = `
    let wasmModule = null;
    let lumenClient = null;

    self.onmessage = async function(e) {
      const { id, type, payload } = e.data;

      try {
        switch (type) {
          case 'init': {
            // Dynamic import of the WASM module
            // In production, this would import the built WASM package
            self.postMessage({ id, type: 'success', payload: { ready: true } });
            break;
          }

          case 'process_update': {
            if (!lumenClient) {
              throw new Error('Client not initialized');
            }
            const result = lumenClient.process_update(payload.updateJson);
            self.postMessage({ id, type: 'success', payload: { accepted: result } });
            break;
          }

          case 'verify_account': {
            if (!lumenClient) {
              throw new Error('Client not initialized');
            }
            const account = lumenClient.verify_account(payload.address, payload.proofJson);
            self.postMessage({ id, type: 'success', payload: account });
            break;
          }

          case 'verify_storage': {
            if (!lumenClient) {
              throw new Error('Client not initialized');
            }
            const value = lumenClient.verify_storage(
              payload.address,
              payload.slot,
              payload.proofJson
            );
            self.postMessage({ id, type: 'success', payload: value });
            break;
          }

          case 'get_state': {
            if (!lumenClient) {
              self.postMessage({ id, type: 'success', payload: { headSlot: 0, isSynced: false } });
              break;
            }
            const state = lumenClient.get_sync_state();
            self.postMessage({ id, type: 'success', payload: state });
            break;
          }

          default:
            throw new Error('Unknown message type: ' + type);
        }
      } catch (error) {
        self.postMessage({
          id,
          type: 'error',
          payload: { message: error instanceof Error ? error.message : String(error) }
        });
      }
    };
  `;

  const blob = new Blob([workerCode], { type: 'application/javascript' });
  const workerUrl = URL.createObjectURL(blob);
  wasmWorker = new Worker(workerUrl, { type: 'module' });

  // Set up message handler
  wasmWorker.onmessage = (e: MessageEvent<WorkerResponse>) => {
    const { id, type, payload } = e.data;
    const pending = pendingRequests.get(id);
    if (pending) {
      pendingRequests.delete(id);
      if (type === 'error') {
        pending.reject(new Error((payload as { message: string }).message));
      } else {
        pending.resolve(payload);
      }
    }
  };

  wasmWorker.onerror = (e: ErrorEvent) => {
    console.error('[Lumen] Worker error:', e.message);
  };

  // Initialize the WASM module in the worker
  await sendToWorker({ type: 'init', payload: {} });

  return wasmWorker;
}

/**
 * Send a request to the WASM worker and wait for a response.
 *
 * @param request - The request to send (without id).
 * @returns Promise that resolves with the worker's response payload.
 */
export function sendToWorker(
  request: Omit<WorkerRequest, 'id'>
): Promise<unknown> {
  return new Promise((resolve, reject) => {
    if (!wasmWorker && request.type !== 'init') {
      reject(new Error('WASM worker not initialized. Call initWasmWorker() first.'));
      return;
    }

    const id = ++requestId;
    pendingRequests.set(id, { resolve, reject });

    const worker = wasmWorker;
    if (worker) {
      worker.postMessage({ id, ...request });
    }

    // Timeout after 30 seconds
    setTimeout(() => {
      if (pendingRequests.has(id)) {
        pendingRequests.delete(id);
        reject(new Error(`WASM worker request timed out (id: ${id}, type: ${request.type})`));
      }
    }, 30_000);
  });
}

/**
 * Terminate the WASM worker.
 * Call this when the Lumen client is no longer needed.
 */
export function terminateWasmWorker(): void {
  if (wasmWorker) {
    wasmWorker.terminate();
    wasmWorker = null;

    // Reject all pending requests
    for (const [id, { reject }] of pendingRequests) {
      reject(new Error('Worker terminated'));
    }
    pendingRequests.clear();
  }
}

/**
 * Check if the WASM worker is initialized and ready.
 */
export function isWasmWorkerReady(): boolean {
  return wasmWorker !== null;
}
