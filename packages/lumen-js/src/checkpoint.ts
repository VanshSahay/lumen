/**
 * Checkpoint management and consensus for the Lumen light client.
 *
 * Fetches the latest finalized checkpoint from multiple independent sources
 * and requires agreement before trusting it.
 *
 * This is the only moment of "soft trust" in Lumen's lifecycle.
 * Once past this point, all verification is purely cryptographic.
 *
 * ## Trust Model
 *
 * - We fetch the latest finalized checkpoint from N independent sources
 * - We require that at least M of them agree on the same block root
 * - The sources are a diverse set of independent operators
 * - An attacker would need to compromise M of N sources simultaneously
 * - After checkpoint, ALL further verification is purely cryptographic
 */

/**
 * A verified checkpoint hash.
 */
export interface CheckpointHash {
  /** The block root hash (hex with 0x prefix). */
  blockRoot: string;
  /** The finalized slot number. */
  slot: number;
  /** How many sources agreed on this checkpoint. */
  sourceAgreement: number;
  /** Total number of sources consulted. */
  totalSources: number;
}

/**
 * Default checkpoint sources — a diverse set of independent operators.
 *
 * IMPORTANT: These are trusted only for providing the initial checkpoint.
 * They cannot forge the checkpoint itself (it must match beacon chain state),
 * but they could potentially collude to point us to an old/wrong checkpoint.
 * That's why we require agreement from multiple independent sources.
 */
export const DEFAULT_CHECKPOINT_SOURCES: string[] = [
  'https://beaconcha.in/api/v1/slot/finalized',
  'https://beaconstate.info',
  'https://mainnet.checkpoint.sigp.io',
  'https://checkpointz.pietjepuk.net',
  'https://mainnet-checkpoint-sync.attestant.io',
];

/**
 * Fetch the latest finalized checkpoint from multiple independent sources
 * and require that N of them agree before trusting it.
 *
 * This is the only moment of "soft trust" in Lumen's lifecycle.
 * Once past this point, all verification is purely cryptographic.
 *
 * @param sources - URLs to fetch checkpoints from. Defaults to DEFAULT_CHECKPOINT_SOURCES.
 * @param requiredAgreement - How many sources must agree. Default: 3.
 * @returns The consensus checkpoint hash.
 * @throws If fewer than requiredAgreement sources agree.
 */
export async function fetchConsensusCheckpoint(
  sources: string[] = DEFAULT_CHECKPOINT_SOURCES,
  requiredAgreement: number = 3,
): Promise<CheckpointHash> {
  console.log(
    `[Lumen] Fetching checkpoint from ${sources.length} sources (need ${requiredAgreement} to agree)`,
  );

  // Fetch from all sources in parallel
  const results = await Promise.allSettled(
    sources.map((source) => fetchCheckpointFromSource(source)),
  );

  // Collect successful results
  const checkpoints: { blockRoot: string; slot: number; source: string }[] = [];
  const failures: string[] = [];

  results.forEach((result, i) => {
    if (result.status === 'fulfilled') {
      checkpoints.push({ ...result.value, source: sources[i] });
      console.log(
        `[Lumen] ✓ Source ${i + 1}/${sources.length}: ${sources[i]} → slot ${result.value.slot}`,
      );
    } else {
      failures.push(`${sources[i]}: ${result.reason}`);
      console.warn(
        `[Lumen] ✗ Source ${i + 1}/${sources.length}: ${sources[i]} → ${result.reason}`,
      );
    }
  });

  if (checkpoints.length < requiredAgreement) {
    throw new Error(
      `Only ${checkpoints.length} checkpoint sources responded (need ${requiredAgreement}). ` +
        `Failures: ${failures.join('; ')}`,
    );
  }

  // Find the block root with the most agreement
  const agreement = new Map<string, { slot: number; count: number }>();

  for (const cp of checkpoints) {
    const existing = agreement.get(cp.blockRoot);
    if (existing) {
      existing.count += 1;
    } else {
      agreement.set(cp.blockRoot, { slot: cp.slot, count: 1 });
    }
  }

  // Find the best consensus
  let bestRoot = '';
  let bestSlot = 0;
  let bestCount = 0;

  for (const [root, { slot, count }] of agreement) {
    if (count > bestCount) {
      bestRoot = root;
      bestSlot = slot;
      bestCount = count;
    }
  }

  if (bestCount < requiredAgreement) {
    throw new Error(
      `Checkpoint consensus failed: best agreement is ${bestCount}/${checkpoints.length} ` +
        `(need ${requiredAgreement}). Sources may be returning different finalized blocks. ` +
        `This could indicate a chain split or source compromise.`,
    );
  }

  const result: CheckpointHash = {
    blockRoot: bestRoot,
    slot: bestSlot,
    sourceAgreement: bestCount,
    totalSources: sources.length,
  };

  console.log(
    `[Lumen] Checkpoint consensus achieved: ${bestCount}/${sources.length} sources agree on slot ${bestSlot}`,
  );
  console.log(`[Lumen] Checkpoint root: ${bestRoot}`);

  return result;
}

/**
 * Fetch a checkpoint from a single source.
 */
async function fetchCheckpointFromSource(
  source: string,
): Promise<{ blockRoot: string; slot: number }> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10_000);

  try {
    const response = await fetch(source, {
      signal: controller.signal,
      headers: { Accept: 'application/json' },
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const data = await response.json();

    // Different APIs return different formats — normalize
    return normalizeCheckpointResponse(data, source);
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') {
      throw new Error('Timeout (10s)');
    }
    throw error;
  } finally {
    clearTimeout(timeout);
  }
}

/**
 * Normalize different checkpoint API response formats to a common shape.
 */
function normalizeCheckpointResponse(
  data: Record<string, unknown>,
  source: string,
): { blockRoot: string; slot: number } {
  // beaconcha.in format
  if (data.data && typeof data.data === 'object') {
    const d = data.data as Record<string, unknown>;
    if (d.blockroot && d.slot) {
      return {
        blockRoot: String(d.blockroot),
        slot: Number(d.slot),
      };
    }
  }

  // Checkpoint sync endpoint format
  if (data.block_root && data.slot) {
    return {
      blockRoot: String(data.block_root),
      slot: Number(data.slot),
    };
  }

  // Lodestar format
  if (data.root && data.epoch) {
    return {
      blockRoot: String(data.root),
      slot: Number(data.epoch) * 32, // epoch * slots_per_epoch
    };
  }

  // Generic format — try common field names
  const root = data.block_root || data.blockRoot || data.root || data.hash;
  const slot = data.slot || data.epoch;

  if (root && slot) {
    return {
      blockRoot: String(root),
      slot: Number(slot),
    };
  }

  throw new Error(`Unrecognized checkpoint format from ${source}`);
}
