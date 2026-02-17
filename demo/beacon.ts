/**
 * Beacon Chain Light Client Sync
 *
 * Fetches light client finality updates from multiple independent beacon
 * chain API endpoints. The finality update contains:
 * - The finalized execution state root (what we ultimately verify proofs against)
 * - Sync committee participation (how many of the 512 validators attested)
 * - The finalized slot and execution block number
 *
 * By fetching from MULTIPLE independent sources and comparing, we get
 * checkpoint-style consensus without trusting any single provider.
 *
 * In production, BLS12-381 signature verification (in WASM) would
 * cryptographically verify the sync committee signatures. The demo
 * uses multi-source consensus as an approximation.
 */

export interface BeaconFinalityUpdate {
  /** Beacon chain slot that was finalized */
  slot: number;
  /** Execution layer state root from the finalized block */
  executionStateRoot: string;
  /** Execution layer block number */
  executionBlockNumber: number;
  /** Execution layer block hash */
  executionBlockHash: string;
  /** Number of sync committee members that signed (out of 512) */
  syncParticipation: number;
  /** Total sync committee size (always 512 on mainnet) */
  syncCommitteeSize: number;
  /** Which beacon API source provided this data */
  source: string;
}

export interface BeaconConsensusResult {
  /** The finality update that all sources agreed on */
  finality: BeaconFinalityUpdate;
  /** Number of independent sources that agreed */
  sourcesAgreed: number;
  /** Total number of sources queried */
  sourcesQueried: number;
  /** Names of sources that agreed */
  agreeSources: string[];
}

/** Independent beacon chain API endpoints (not execution RPCs). */
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

interface RawFinalityResponse {
  version?: string;
  data: {
    finalized_header: {
      beacon: {
        slot: string;
        proposer_index: string;
        parent_root: string;
        state_root: string;
        body_root: string;
      };
      execution: {
        state_root: string;
        block_number: string;
        block_hash: string;
        [key: string]: unknown;
      };
    };
    sync_aggregate: {
      sync_committee_bits: string;
      sync_committee_signature: string;
    };
    signature_slot: string;
  };
}

/**
 * Count the number of set bits in a hex string (sync committee participation).
 * Each bit represents one validator's signature.
 */
function countParticipation(hexBits: string): {
  participating: number;
  total: number;
} {
  const bits = hexBits.startsWith('0x') ? hexBits.slice(2) : hexBits;
  let count = 0;
  for (const char of bits) {
    const nibble = parseInt(char, 16);
    count += ((nibble >> 0) & 1) + ((nibble >> 1) & 1) +
             ((nibble >> 2) & 1) + ((nibble >> 3) & 1);
  }
  return { participating: count, total: 512 };
}

/**
 * Fetch a light client finality update from a single beacon API.
 */
async function fetchFromBeaconAPI(
  api: { name: string; url: string },
  signal?: AbortSignal,
): Promise<BeaconFinalityUpdate> {
  const endpoint = `${api.url}/eth/v1/beacon/light_client/finality_update`;

  const response = await fetch(endpoint, {
    headers: { Accept: 'application/json' },
    signal,
  });

  if (!response.ok) {
    throw new Error(`${api.name}: HTTP ${response.status}`);
  }

  const raw: RawFinalityResponse = await response.json();
  const finalized = raw.data.finalized_header;
  const sync = raw.data.sync_aggregate;
  const { participating, total } = countParticipation(sync.sync_committee_bits);

  return {
    slot: parseInt(finalized.beacon.slot, 10),
    executionStateRoot: finalized.execution.state_root,
    executionBlockNumber: parseInt(finalized.execution.block_number, 10),
    executionBlockHash: finalized.execution.block_hash,
    syncParticipation: participating,
    syncCommitteeSize: total,
    source: api.name,
  };
}

/**
 * Fetch finality updates from ALL beacon APIs and verify consensus.
 *
 * All sources must agree on the same finalized execution state root.
 * If any source disagrees, we report the discrepancy.
 *
 * This is the "checkpoint sync" trust model: we trust that multiple
 * independent operators are not colluding. In production, BLS signature
 * verification would make this fully trustless.
 */
export async function fetchBeaconConsensus(): Promise<BeaconConsensusResult> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 15_000);

  try {
    const results = await Promise.allSettled(
      BEACON_APIS.map((api) => fetchFromBeaconAPI(api, controller.signal)),
    );

    const successes: BeaconFinalityUpdate[] = [];
    const failures: string[] = [];

    for (let i = 0; i < results.length; i++) {
      const result = results[i];
      if (result.status === 'fulfilled') {
        successes.push(result.value);
      } else {
        failures.push(
          `${BEACON_APIS[i].name}: ${result.reason?.message || 'unknown error'}`,
        );
      }
    }

    if (successes.length === 0) {
      throw new Error(
        `All beacon APIs failed: ${failures.join('; ')}`,
      );
    }

    // Check consensus: all successful sources must agree on the state root
    const referenceRoot = successes[0].executionStateRoot;
    const agreeSources: string[] = [];
    const disagreeSources: string[] = [];

    for (const update of successes) {
      if (update.executionStateRoot === referenceRoot) {
        agreeSources.push(update.source);
      } else {
        disagreeSources.push(update.source);
      }
    }

    if (disagreeSources.length > 0) {
      throw new Error(
        `Beacon chain consensus FAILED: sources disagree on finalized state root. ` +
          `Agree (${agreeSources.join(', ')}): ${referenceRoot}, ` +
          `Disagree (${disagreeSources.join(', ')}): different roots. ` +
          `This could indicate a chain split or compromised API.`,
      );
    }

    // Take the update with highest participation as the reference
    const best = successes.reduce((a, b) =>
      a.syncParticipation > b.syncParticipation ? a : b,
    );

    return {
      finality: best,
      sourcesAgreed: agreeSources.length,
      sourcesQueried: BEACON_APIS.length,
      agreeSources,
    };
  } finally {
    clearTimeout(timeout);
  }
}

/**
 * Get all configured beacon API names.
 */
export function getBeaconSources(): string[] {
  return BEACON_APIS.map((api) => api.name);
}
