/**
 * Lumen P2P Worker
 *
 * Runs in a Web Worker to keep the main thread responsive.
 * Responsibilities:
 * 1. Poll beacon chain APIs for finality updates (current transport)
 * 2. Subscribe to P2P gossip topics when available (future transport)
 * 3. Post raw update JSON to the main thread for BLS verification
 *
 * Trust model:
 * - This worker is a DATA TRANSPORT layer only
 * - It fetches raw JSON from beacon APIs / P2P peers
 * - ALL trust comes from BLS verification in the WASM module (main thread)
 * - A compromised worker cannot forge a valid BLS signature
 *
 * P2P Architecture (lumen-p2p integration):
 * - Topic: /eth2/{fork_digest}/light_client_finality_update/ssz_snappy
 * - Topic: /eth2/{fork_digest}/light_client_optimistic_update/ssz_snappy
 * - Transport: WebRTC / WebTransport (when browser libp2p matures)
 * - Current fallback: HTTP polling of beacon chain REST APIs
 */

// Beacon API endpoints (data transport — NOT trusted)
const BEACON_APIS = [
  {
    name: 'ChainSafe Lodestar',
    url: 'https://lodestar-mainnet.chainsafe.io',
  },
  {
    name: 'PublicNode Beacon',
    url: 'https://ethereum-beacon-api.publicnode.com',
  },
];

// P2P gossip topics (from lumen-p2p::beacon_gossip)
const GOSSIP_TOPICS = {
  FINALITY_UPDATE:
    '/eth2/b5303f2a/light_client_finality_update/ssz_snappy',
  OPTIMISTIC_UPDATE:
    '/eth2/b5303f2a/light_client_optimistic_update/ssz_snappy',
};

// Message types between worker and main thread
interface WorkerMessage {
  type:
    | 'bootstrap_ready'
    | 'finality_update'
    | 'error'
    | 'status'
    | 'gossip_connected'
    | 'gossip_disconnected';
  payload: unknown;
}

interface MainThreadCommand {
  type: 'start' | 'stop' | 'fetch_bootstrap' | 'set_poll_interval';
  payload?: unknown;
}

let pollInterval: ReturnType<typeof setInterval> | null = null;
let running = false;
let lastFinalizedSlot = 0;

// -----------------------------------------------------------------------
// Beacon API Data Transport (current)
// -----------------------------------------------------------------------

async function fetchFinalizedBlockRoot(): Promise<string> {
  for (const api of BEACON_APIS) {
    try {
      const resp = await fetch(
        `${api.url}/eth/v1/beacon/headers/finalized`,
        { headers: { Accept: 'application/json' } },
      );
      if (!resp.ok) continue;
      const json = await resp.json();
      return json.data.root;
    } catch {
      continue;
    }
  }
  throw new Error('All beacon APIs failed to return finalized block root');
}

async function fetchBootstrapJson(blockRoot: string): Promise<string> {
  for (const api of BEACON_APIS) {
    try {
      const resp = await fetch(
        `${api.url}/eth/v1/beacon/light_client/bootstrap/${blockRoot}`,
        { headers: { Accept: 'application/json' } },
      );
      if (!resp.ok) continue;
      return await resp.text();
    } catch {
      continue;
    }
  }
  throw new Error('All beacon APIs failed to return bootstrap');
}

async function fetchFinalityUpdateJson(): Promise<{
  json: string;
  slot: number;
  source: string;
}> {
  for (const api of BEACON_APIS) {
    try {
      const resp = await fetch(
        `${api.url}/eth/v1/beacon/light_client/finality_update`,
        { headers: { Accept: 'application/json' } },
      );
      if (!resp.ok) continue;
      const text = await resp.text();
      const parsed = JSON.parse(text);
      const slot = parseInt(
        parsed.data.finalized_header.beacon.slot,
        10,
      );
      return { json: text, slot, source: api.name };
    } catch {
      continue;
    }
  }
  throw new Error('All beacon APIs failed to return finality update');
}

// -----------------------------------------------------------------------
// Polling loop
// -----------------------------------------------------------------------

async function pollForUpdates(): Promise<void> {
  try {
    const update = await fetchFinalityUpdateJson();

    if (update.slot > lastFinalizedSlot) {
      lastFinalizedSlot = update.slot;
      postMessage({
        type: 'finality_update',
        payload: {
          json: update.json,
          slot: update.slot,
          source: update.source,
          transport: 'http', // will be 'p2p_gossip' when available
        },
      } satisfies WorkerMessage);
    }
  } catch (err) {
    postMessage({
      type: 'error',
      payload: {
        message:
          err instanceof Error ? err.message : String(err),
        context: 'polling',
      },
    } satisfies WorkerMessage);
  }
}

// -----------------------------------------------------------------------
// P2P Gossip Integration (infrastructure — ready for lumen-p2p WASM)
// -----------------------------------------------------------------------

/**
 * When lumen-p2p is compiled to WASM and loaded in this worker,
 * it will subscribe to these gossip topics and call this handler
 * with raw SSZ+snappy bytes. Those bytes will be decoded and
 * forwarded to the main thread for BLS verification.
 *
 * For now, this is a placeholder that documents the intended flow.
 */
function _onGossipMessage(topic: string, _data: Uint8Array): void {
  const msgType =
    topic === GOSSIP_TOPICS.FINALITY_UPDATE
      ? 'finality_update'
      : topic === GOSSIP_TOPICS.OPTIMISTIC_UPDATE
        ? 'optimistic_update'
        : 'unknown';

  // When P2P is live, decode SSZ, convert to JSON, post to main thread
  postMessage({
    type: 'status',
    payload: {
      message: `P2P gossip: received ${msgType} from topic`,
      transport: 'p2p_gossip',
    },
  } satisfies WorkerMessage);
}

// -----------------------------------------------------------------------
// Message handler
// -----------------------------------------------------------------------

self.onmessage = async (event: MessageEvent<MainThreadCommand>) => {
  const { type, payload } = event.data;

  switch (type) {
    case 'start': {
      if (running) return;
      running = true;

      postMessage({
        type: 'status',
        payload: { message: 'Worker started — fetching bootstrap...' },
      } satisfies WorkerMessage);

      try {
        // Step 1: Get finalized block root
        const blockRoot = await fetchFinalizedBlockRoot();

        // Step 2: Fetch bootstrap (sync committee + finalized header)
        const bootstrapJson = await fetchBootstrapJson(blockRoot);

        postMessage({
          type: 'bootstrap_ready',
          payload: {
            json: bootstrapJson,
            blockRoot,
          },
        } satisfies WorkerMessage);

        // Step 3: Fetch initial finality update
        await pollForUpdates();

        // Step 4: Start polling (every 12 seconds — one slot)
        const intervalMs =
          typeof payload === 'number' ? payload : 12_000;
        pollInterval = setInterval(pollForUpdates, intervalMs);

        postMessage({
          type: 'status',
          payload: {
            message: `Polling every ${intervalMs / 1000}s for finality updates`,
          },
        } satisfies WorkerMessage);
      } catch (err) {
        postMessage({
          type: 'error',
          payload: {
            message:
              err instanceof Error ? err.message : String(err),
            context: 'startup',
          },
        } satisfies WorkerMessage);
      }
      break;
    }

    case 'stop': {
      running = false;
      if (pollInterval) {
        clearInterval(pollInterval);
        pollInterval = null;
      }
      postMessage({
        type: 'status',
        payload: { message: 'Worker stopped' },
      } satisfies WorkerMessage);
      break;
    }

    case 'fetch_bootstrap': {
      try {
        const blockRoot = await fetchFinalizedBlockRoot();
        const bootstrapJson = await fetchBootstrapJson(blockRoot);
        postMessage({
          type: 'bootstrap_ready',
          payload: { json: bootstrapJson, blockRoot },
        } satisfies WorkerMessage);
      } catch (err) {
        postMessage({
          type: 'error',
          payload: {
            message:
              err instanceof Error ? err.message : String(err),
            context: 'fetch_bootstrap',
          },
        } satisfies WorkerMessage);
      }
      break;
    }

    case 'set_poll_interval': {
      if (pollInterval) clearInterval(pollInterval);
      const ms = typeof payload === 'number' ? payload : 12_000;
      pollInterval = setInterval(pollForUpdates, ms);
      break;
    }
  }
};
