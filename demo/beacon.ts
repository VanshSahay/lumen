/**
 * Beacon Chain Data Transport
 *
 * Fetches data from Ethereum beacon chain REST APIs.
 * These APIs are used as UNTRUSTED DATA TRANSPORT only.
 *
 * All trust comes from:
 * - BLS12-381 signature verification (in Rust/WASM via lumen-core)
 * - Merkle branch verification (in Rust/WASM)
 *
 * Even if every beacon API is compromised, they cannot forge a valid
 * BLS aggregate signature from the sync committee.
 *
 * Data flow:
 * 1. Fetch finalized block root → GET /eth/v1/beacon/headers/finalized
 * 2. Fetch bootstrap (sync committee) → GET /eth/v1/beacon/light_client/bootstrap/{root}
 * 3. Fetch finality update → GET /eth/v1/beacon/light_client/finality_update
 * 4. Pass raw JSON to WASM for cryptographic verification
 */

/** Independent beacon chain API endpoints (data transport, NOT trusted). */
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

// ---------------------------------------------------------------------------
// Bootstrap: fetch the sync committee for BLS verification
// ---------------------------------------------------------------------------

/**
 * Fetch the finalized block root from a beacon API.
 * This root is used to fetch the bootstrap (sync committee).
 */
export async function fetchFinalizedBlockRoot(): Promise<{
  root: string;
  slot: number;
  source: string;
}> {
  for (const api of BEACON_APIS) {
    try {
      const resp = await fetch(
        `${api.url}/eth/v1/beacon/headers/finalized`,
        {
          headers: { Accept: 'application/json' },
          signal: AbortSignal.timeout(10_000),
        },
      );
      if (!resp.ok) continue;
      const json = await resp.json();
      return {
        root: json.data.root,
        slot: parseInt(json.data.header.message.slot, 10),
        source: api.name,
      };
    } catch {
      continue;
    }
  }
  throw new Error('All beacon APIs failed to return finalized block root');
}

/**
 * Fetch the light client bootstrap data for a given block root.
 * Returns the RAW JSON string — it goes straight to WASM for verification.
 *
 * The bootstrap contains:
 * - The beacon block header at the checkpoint
 * - The current sync committee (512 BLS public keys)
 * - The execution payload header (with state root)
 */
export async function fetchBootstrapJson(
  blockRoot: string,
): Promise<{ json: string; source: string }> {
  for (const api of BEACON_APIS) {
    try {
      const resp = await fetch(
        `${api.url}/eth/v1/beacon/light_client/bootstrap/${blockRoot}`,
        {
          headers: { Accept: 'application/json' },
          signal: AbortSignal.timeout(15_000),
        },
      );
      if (!resp.ok) continue;
      const json = await resp.text();
      return { json, source: api.name };
    } catch {
      continue;
    }
  }
  throw new Error('All beacon APIs failed to return bootstrap data');
}

// ---------------------------------------------------------------------------
// Finality updates: the data that gets BLS-verified
// ---------------------------------------------------------------------------

export interface RawFinalityUpdate {
  /** Raw JSON string — passed directly to WASM for BLS verification */
  json: string;
  /** Finalized beacon slot (parsed for display, NOT trusted until BLS-verified) */
  claimedSlot: number;
  /** Execution block number (parsed for display, NOT trusted until BLS-verified) */
  claimedBlockNumber: number;
  /** Execution state root (parsed for display, NOT trusted until BLS-verified) */
  claimedStateRoot: string;
  /** Sync committee participation count (parsed for display) */
  claimedParticipation: number;
  /** Which beacon API returned this data */
  source: string;
}

/**
 * Fetch a finality update from beacon APIs.
 * Returns the raw JSON for WASM BLS verification, plus parsed display data.
 *
 * The "claimed" fields are parsed for UI display but are NOT trusted
 * until the WASM module verifies the BLS signature.
 */
export async function fetchFinalityUpdateRaw(): Promise<RawFinalityUpdate> {
  for (const api of BEACON_APIS) {
    try {
      const resp = await fetch(
        `${api.url}/eth/v1/beacon/light_client/finality_update`,
        {
          headers: { Accept: 'application/json' },
          signal: AbortSignal.timeout(10_000),
        },
      );
      if (!resp.ok) continue;

      const text = await resp.text();
      const parsed = JSON.parse(text);
      const fin = parsed.data.finalized_header;

      return {
        json: text,
        claimedSlot: parseInt(fin.beacon.slot, 10),
        claimedBlockNumber: parseInt(fin.execution?.block_number || '0', 10),
        claimedStateRoot: fin.execution?.state_root || '0x',
        claimedParticipation: countParticipation(
          parsed.data.sync_aggregate.sync_committee_bits,
        ),
        source: api.name,
      };
    } catch {
      continue;
    }
  }
  throw new Error('All beacon APIs failed to return finality update');
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Count set bits in a hex-encoded bitvector (sync committee participation).
 */
function countParticipation(hexBits: string): number {
  const bits = hexBits.startsWith('0x') ? hexBits.slice(2) : hexBits;
  let count = 0;
  for (const char of bits) {
    const nibble = parseInt(char, 16);
    count +=
      ((nibble >> 0) & 1) +
      ((nibble >> 1) & 1) +
      ((nibble >> 2) & 1) +
      ((nibble >> 3) & 1);
  }
  return count;
}

/** Get all configured beacon API names. */
export function getBeaconSources(): string[] {
  return BEACON_APIS.map((api) => api.name);
}
