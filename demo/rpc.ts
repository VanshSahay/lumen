/**
 * Ethereum JSON-RPC client for the Lumen demo.
 *
 * Uses multiple public RPC endpoints for redundancy. No API keys required.
 *
 * TRUST MODEL: The RPC endpoints are UNTRUSTED data sources. We fetch
 * data from them (block headers, Merkle proofs), but ALL data is
 * cryptographically verified locally before being displayed.
 *
 * The only thing we "trust" the RPC for is the state root of a finalized
 * block. In production, this would come from the BLS-verified beacon chain
 * sync committee. The demo makes this trust boundary explicit.
 */

/** Public RPC endpoints — no API keys, CORS-enabled. */
const RPC_ENDPOINTS = [
  'https://ethereum-rpc.publicnode.com',
  'https://rpc.ankr.com/eth',
  'https://eth.llamarpc.com',
];

let currentEndpointIndex = 0;

interface JsonRpcResponse {
  jsonrpc: string;
  id: number;
  result?: unknown;
  error?: { code: number; message: string };
}

/**
 * Make a JSON-RPC call, trying multiple endpoints on failure.
 */
async function rpcCall(
  method: string,
  params: unknown[],
): Promise<unknown> {
  let lastError: Error | null = null;

  for (let attempt = 0; attempt < RPC_ENDPOINTS.length; attempt++) {
    const endpoint =
      RPC_ENDPOINTS[(currentEndpointIndex + attempt) % RPC_ENDPOINTS.length];

    try {
      const response = await fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0',
          id: Date.now(),
          method,
          params,
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const data: JsonRpcResponse = await response.json();

      if (data.error) {
        throw new Error(`RPC error ${data.error.code}: ${data.error.message}`);
      }

      // This endpoint worked — prefer it next time
      currentEndpointIndex =
        (currentEndpointIndex + attempt) % RPC_ENDPOINTS.length;

      return data.result;
    } catch (error) {
      lastError =
        error instanceof Error ? error : new Error(String(error));
      console.warn(`[Lumen RPC] ${endpoint} failed: ${lastError.message}`);
    }
  }

  throw new Error(
    `All RPC endpoints failed. Last error: ${lastError?.message}`,
  );
}

// --- Public API ---

export interface BlockHeader {
  number: string;       // hex
  hash: string;
  stateRoot: string;
  timestamp: string;    // hex
}

export interface EthGetProofResponse {
  address: string;
  accountProof: string[];
  balance: string;         // claimed balance (we verify independently)
  codeHash: string;
  nonce: string;
  storageHash: string;
  storageProof: unknown[];
}

/**
 * Get the latest block header.
 * Returns the block number, hash, and state root.
 *
 * Uses "latest" because most public RPCs only serve eth_getProof for
 * very recent blocks (the "proof window" is typically <128 blocks).
 * Using "finalized" risks the block aging out of the window before
 * the user clicks Verify.
 *
 * TRUST NOTE: In production, the state root would be obtained from the
 * BLS-verified beacon chain sync committee. In this demo, we fetch it
 * from the RPC. The Merkle proof verification against this root IS
 * trustless — only the source of the root requires trust.
 */
export async function getLatestBlock(): Promise<BlockHeader> {
  const block = (await rpcCall('eth_getBlockByNumber', [
    'latest',
    false,
  ])) as BlockHeader | null;
  if (!block) throw new Error('Failed to fetch block');
  return block;
}

/**
 * Fetch an account proof via eth_getProof.
 *
 * The returned proof data is UNTRUSTED — it will be verified
 * cryptographically using verifyAccountProof().
 */
export async function getProof(
  address: string,
  blockNumber: string,
): Promise<EthGetProofResponse> {
  const result = (await rpcCall('eth_getProof', [
    address,
    [],
    blockNumber,
  ])) as EthGetProofResponse | null;

  if (!result) throw new Error(`Failed to get proof for ${address}`);
  return result;
}

/**
 * Get the current RPC endpoint being used.
 */
export function getCurrentEndpoint(): string {
  return RPC_ENDPOINTS[currentEndpointIndex];
}

/**
 * Get all available RPC endpoints.
 */
export function getAllEndpoints(): string[] {
  return [...RPC_ENDPOINTS];
}
