/**
 * React hook for the Lumen trustless Ethereum light client.
 *
 * Provides a simple hook-based API for React applications to access
 * the Lumen provider with automatic lifecycle management.
 *
 * @example
 * ```tsx
 * import { useLumen } from 'lumen-react'
 *
 * function App() {
 *   const { provider, syncState, isReady, error } = useLumen()
 *
 *   if (!isReady) return <div>Syncing: {syncState.status}...</div>
 *   if (error) return <div>Error: {error.message}</div>
 *
 *   return <div>Connected! Head slot: {syncState.headSlot}</div>
 * }
 * ```
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import type { LumenProvider, SyncState, LumenOptions } from 'lumen-eth';
import { createLumenProvider } from 'lumen-eth';

/**
 * Return type of the useLumen hook.
 */
export interface UseLumenResult {
  /** The Lumen provider instance. Null until initialization completes. */
  provider: LumenProvider | null;
  /** Current sync state. */
  syncState: SyncState;
  /** Whether the provider is initialized and ready to serve requests. */
  isReady: boolean;
  /** Error if initialization or syncing failed. */
  error: Error | null;
  /** Manually reconnect (destroys and recreates the provider). */
  reconnect: () => void;
}

/**
 * React hook for the Lumen trustless Ethereum light client.
 *
 * Automatically initializes the Lumen provider on mount and cleans up on unmount.
 * Provides reactive sync state updates for UI display.
 *
 * @param options - Lumen provider options. Changes to this object will cause reinitialization.
 * @returns The current Lumen state and provider instance.
 */
export function useLumen(options?: LumenOptions): UseLumenResult {
  const [provider, setProvider] = useState<LumenProvider | null>(null);
  const [syncState, setSyncState] = useState<SyncState>({ status: 'bootstrapping' });
  const [isReady, setIsReady] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const providerRef = useRef<LumenProvider | null>(null);
  const mountedRef = useRef(true);

  const initProvider = useCallback(async () => {
    try {
      setError(null);
      setSyncState({ status: 'bootstrapping' });
      setIsReady(false);

      // Clean up previous provider
      if (providerRef.current) {
        await providerRef.current.destroy();
      }

      const newProvider = await createLumenProvider(options);

      if (!mountedRef.current) {
        await newProvider.destroy();
        return;
      }

      providerRef.current = newProvider;
      setProvider(newProvider);

      // Subscribe to sync state changes
      newProvider.onSyncStateChange((state) => {
        if (mountedRef.current) {
          setSyncState(state);
          setIsReady(state.status === 'synced');
        }
      });

      // Set initial sync state
      const initialState = newProvider.getSyncState();
      setSyncState(initialState);
      setIsReady(initialState.status === 'synced');
    } catch (err) {
      if (mountedRef.current) {
        const error = err instanceof Error ? err : new Error(String(err));
        setError(error);
        setSyncState({ status: 'error', message: error.message });
      }
    }
  }, [options]);

  useEffect(() => {
    mountedRef.current = true;
    initProvider();

    return () => {
      mountedRef.current = false;
      if (providerRef.current) {
        providerRef.current.destroy();
        providerRef.current = null;
      }
    };
  }, [initProvider]);

  const reconnect = useCallback(() => {
    initProvider();
  }, [initProvider]);

  return {
    provider,
    syncState,
    isReady,
    error,
    reconnect,
  };
}
