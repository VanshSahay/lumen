/**
 * Execution Layer Data Transport
 *
 * This module fetches RAW DATA from Ethereum execution layer nodes.
 * It is NOT a trusted source of truth — ALL data fetched here is
 * cryptographically verified locally before being used.
 *
 * The execution RPC is used for exactly two things:
 * 1. Fetching the latest block header (to get a recent state root)
 * 2. Fetching Merkle proofs via eth_getProof (untrusted proof bytes)
 *
 * Both are verified:
 * - The block's state root is cross-checked against beacon chain finality
 * - The Merkle proof is verified via keccak256 hash chain from root to leaf
 *
 * The RPC could be replaced by any data source — P2P peers, Portal
 * Network, a different RPC, or even a local node. The security model
 * does not depend on the RPC being honest.
 */

const RPC_ENDPOINTS = [
  'https://ethereum-rpc.publicnode.com',
  'https://eth.llamarpc.com',
];

let currentEndpointIndex = 0;

interface JsonRpcResponse {
  jsonrpc: string;
  id: number;
  result?: unknown;
  error?: { code: number; message: string };
}

async function rpcCall(method: string, params: unknown[]): Promise<unknown> {
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

      if (!response.ok) throw new Error(`HTTP ${response.status}`);

      const data: JsonRpcResponse = await response.json();
      if (data.error) {
        throw new Error(`RPC error ${data.error.code}: ${data.error.message}`);
      }

      currentEndpointIndex =
        (currentEndpointIndex + attempt) % RPC_ENDPOINTS.length;
      return data.result;
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
      console.warn(`[Lumen RPC] ${endpoint} failed: ${lastError.message}`);
    }
  }

  throw new Error(
    `All RPC endpoints failed. Last error: ${lastError?.message}`,
  );
}

// --- Types ---

export interface BlockHeader {
  number: string;
  hash: string;
  stateRoot: string;
  timestamp: string;
}

export interface EthGetProofResponse {
  address: string;
  accountProof: string[];
  balance: string;
  codeHash: string;
  nonce: string;
  storageHash: string;
  storageProof: unknown[];
}

// --- Public API ---

/**
 * Fetch the latest block header from an execution RPC.
 *
 * The state root from this block is used for Merkle proof verification.
 * We use "latest" because free public RPCs only serve eth_getProof for
 * very recent blocks (the proof window is typically <128 blocks).
 *
 * The block's relationship to the beacon chain finalized head is
 * cross-checked to ensure it extends the canonical chain.
 */
export async function getLatestBlock(): Promise<BlockHeader> {
  const block = (await rpcCall('eth_getBlockByNumber', [
    'latest',
    false,
  ])) as BlockHeader | null;
  if (!block) throw new Error('Failed to fetch latest block');
  return block;
}

/**
 * Fetch an account's Merkle proof via eth_getProof.
 *
 * The proof data is UNTRUSTED — it will be cryptographically verified
 * against a known state root using keccak256 + Merkle-Patricia trie
 * walking. The RPC cannot forge a valid proof without finding a
 * keccak256 preimage collision (computationally infeasible).
 */
export async function getProof(
  address: string,
  blockTag: string = 'latest',
): Promise<EthGetProofResponse> {
  const result = (await rpcCall('eth_getProof', [
    address,
    [],
    blockTag,
  ])) as EthGetProofResponse | null;

  if (!result) throw new Error(`Failed to get proof for ${address}`);
  return result;
}

export function getCurrentEndpoint(): string {
  return RPC_ENDPOINTS[currentEndpointIndex];
}
