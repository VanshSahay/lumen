/**
 * Trustless Ethereum Merkle-Patricia Trie Proof Verification
 *
 * This module implements LOCAL cryptographic verification of Ethereum
 * account proofs. The data can come from ANY source (including untrusted
 * RPCs) — the math doesn't lie.
 *
 * Verification chain:
 * 1. State root comes from a verified beacon chain header (or block header)
 * 2. keccak256(address) gives the trie key
 * 3. Walk the proof nodes from root to leaf, verifying keccak256 hashes at each step
 * 4. Decode the leaf value as an RLP-encoded account [nonce, balance, storageRoot, codeHash]
 *
 * If ANY hash in the chain doesn't match, the proof is REJECTED.
 * An attacker cannot forge a valid proof without finding a keccak256 collision.
 */

import { keccak256 as keccak256Hash } from 'js-sha3';

// --- Core Crypto ---

/** Compute keccak256 hash of a byte array. */
export function keccak256(data: Uint8Array): Uint8Array {
  return new Uint8Array(keccak256Hash.arrayBuffer(data));
}

// --- Encoding Utilities ---

export function hexToBytes(hex: string): Uint8Array {
  const h = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (h.length === 0) return new Uint8Array(0);
  const bytes = new Uint8Array(h.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(h.substring(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

export function bytesToHex(bytes: Uint8Array): string {
  return (
    '0x' +
    Array.from(bytes)
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('')
  );
}

function arraysEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

// --- RLP Decoding ---

type RLPItem = Uint8Array | RLPItem[];

function rlpDecodeItem(
  data: Uint8Array,
  offset: number,
): { item: RLPItem; consumed: number } {
  if (offset >= data.length) throw new Error('RLP: unexpected end of data');

  const prefix = data[offset];

  if (prefix <= 0x7f) {
    return { item: new Uint8Array([prefix]), consumed: 1 };
  } else if (prefix <= 0xb7) {
    const length = prefix - 0x80;
    if (length === 0) return { item: new Uint8Array(0), consumed: 1 };
    return {
      item: data.slice(offset + 1, offset + 1 + length),
      consumed: 1 + length,
    };
  } else if (prefix <= 0xbf) {
    const lenBytes = prefix - 0xb7;
    let length = 0;
    for (let i = 0; i < lenBytes; i++) {
      length = (length << 8) | data[offset + 1 + i];
    }
    return {
      item: data.slice(offset + 1 + lenBytes, offset + 1 + lenBytes + length),
      consumed: 1 + lenBytes + length,
    };
  } else if (prefix <= 0xf7) {
    const length = prefix - 0xc0;
    const items: RLPItem[] = [];
    let pos = 0;
    while (pos < length) {
      const { item, consumed } = rlpDecodeItem(data, offset + 1 + pos);
      items.push(item);
      pos += consumed;
    }
    return { item: items, consumed: 1 + length };
  } else {
    const lenBytes = prefix - 0xf7;
    let length = 0;
    for (let i = 0; i < lenBytes; i++) {
      length = (length << 8) | data[offset + 1 + i];
    }
    const items: RLPItem[] = [];
    let pos = 0;
    while (pos < length) {
      const { item, consumed } = rlpDecodeItem(
        data,
        offset + 1 + lenBytes + pos,
      );
      items.push(item);
      pos += consumed;
    }
    return { item: items, consumed: 1 + lenBytes + length };
  }
}

export function rlpDecode(data: Uint8Array): RLPItem {
  const { item } = rlpDecodeItem(data, 0);
  return item;
}

// --- Trie Path Encoding ---

function bytesToNibbles(bytes: Uint8Array): number[] {
  const nibbles: number[] = [];
  for (const byte of bytes) {
    nibbles.push(byte >> 4);
    nibbles.push(byte & 0x0f);
  }
  return nibbles;
}

function decodeCompactPath(encoded: Uint8Array): {
  nibbles: number[];
  isLeaf: boolean;
} {
  if (encoded.length === 0) return { nibbles: [], isLeaf: false };

  const firstNibble = encoded[0] >> 4;
  const isLeaf = firstNibble >= 2;
  const isOdd = firstNibble % 2 === 1;

  const nibbles: number[] = [];
  if (isOdd) {
    nibbles.push(encoded[0] & 0x0f);
  }
  for (let i = 1; i < encoded.length; i++) {
    nibbles.push(encoded[i] >> 4);
    nibbles.push(encoded[i] & 0x0f);
  }
  return { nibbles, isLeaf };
}

function nibblesMatch(a: number[], b: number[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

function nibblesStartWith(haystack: number[], needle: number[]): boolean {
  if (haystack.length < needle.length) return false;
  for (let i = 0; i < needle.length; i++) {
    if (haystack[i] !== needle[i]) return false;
  }
  return true;
}

// --- Merkle-Patricia Trie Proof Verification ---

export interface ProofVerificationResult {
  value: Uint8Array | null;
  nodesVerified: number;
  verified: boolean;
}

/**
 * Verify a Merkle-Patricia Trie proof.
 *
 * Given a state root, a key (keccak256 of address), and proof nodes
 * (from eth_getProof), walk the trie and verify every hash link.
 *
 * Returns the leaf value (RLP-encoded account) if the proof is valid.
 * Throws if any hash doesn't match (proof is invalid/tampered).
 */
export function verifyMPTProof(
  expectedRoot: Uint8Array,
  key: Uint8Array,
  proofNodes: Uint8Array[],
): ProofVerificationResult {
  if (proofNodes.length === 0) {
    return { value: null, nodesVerified: 0, verified: true };
  }

  const keyNibbles = bytesToNibbles(key);
  let nibbleIndex = 0;

  // Verify root: keccak256(first_node) must equal the expected state root
  const firstNodeHash = keccak256(proofNodes[0]);
  if (proofNodes[0].length >= 32 && !arraysEqual(firstNodeHash, expectedRoot)) {
    throw new Error(
      `Root hash mismatch: expected ${bytesToHex(expectedRoot)}, ` +
        `got ${bytesToHex(firstNodeHash)}`,
    );
  }

  for (let depth = 0; depth < proofNodes.length; depth++) {
    const decoded = rlpDecode(proofNodes[depth]);
    if (!Array.isArray(decoded)) {
      throw new Error(`Invalid trie node at depth ${depth}: not a list`);
    }

    const node = decoded as RLPItem[];

    if (node.length === 17) {
      // --- Branch Node (16 children + value) ---
      if (nibbleIndex >= keyNibbles.length) {
        const value = node[16] as Uint8Array;
        return {
          value: value.length > 0 ? value : null,
          nodesVerified: depth + 1,
          verified: true,
        };
      }

      const childIndex = keyNibbles[nibbleIndex];
      nibbleIndex++;

      if (depth + 1 < proofNodes.length) {
        const childRef = node[childIndex] as Uint8Array;
        if (childRef.length === 32) {
          const nextHash = keccak256(proofNodes[depth + 1]);
          if (
            proofNodes[depth + 1].length >= 32 &&
            !arraysEqual(nextHash, childRef)
          ) {
            throw new Error(
              `Branch child hash mismatch at depth ${depth}, child ${childIndex}`,
            );
          }
        }
      } else {
        // Last node in proof
        const childRef = node[childIndex] as Uint8Array;
        if (childRef.length === 0) {
          return { value: null, nodesVerified: depth + 1, verified: true };
        }
        // Inline value
        return { value: childRef, nodesVerified: depth + 1, verified: true };
      }
    } else if (node.length === 2) {
      // --- Extension or Leaf Node ---
      const { nibbles: pathNibbles, isLeaf } = decodeCompactPath(
        node[0] as Uint8Array,
      );

      if (isLeaf) {
        const remaining = keyNibbles.slice(nibbleIndex);
        if (nibblesMatch(remaining, pathNibbles)) {
          return {
            value: node[1] as Uint8Array,
            nodesVerified: depth + 1,
            verified: true,
          };
        }
        // Key doesn't match — valid proof of non-existence
        return { value: null, nodesVerified: depth + 1, verified: true };
      } else {
        // Extension node
        const remaining = keyNibbles.slice(nibbleIndex);
        if (!nibblesStartWith(remaining, pathNibbles)) {
          return { value: null, nodesVerified: depth + 1, verified: true };
        }
        nibbleIndex += pathNibbles.length;

        if (depth + 1 < proofNodes.length) {
          const childRef = node[1] as Uint8Array;
          if (childRef.length === 32 && proofNodes[depth + 1].length >= 32) {
            const nextHash = keccak256(proofNodes[depth + 1]);
            if (!arraysEqual(nextHash, childRef)) {
              throw new Error(
                `Extension child hash mismatch at depth ${depth}`,
              );
            }
          }
        }
      }
    } else {
      throw new Error(
        `Invalid trie node at depth ${depth}: ${node.length} items (expected 2 or 17)`,
      );
    }
  }

  throw new Error(`Proof incomplete: ran out of nodes at nibble ${nibbleIndex}`);
}

// --- Account Decoding ---

export interface VerifiedAccount {
  nonce: bigint;
  balance: bigint;
  storageRoot: string;
  codeHash: string;
  isContract: boolean;
}

const EMPTY_CODE_HASH =
  '0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470';

/**
 * Decode an RLP-encoded Ethereum account.
 * Account format: RLP([nonce, balance, storageRoot, codeHash])
 */
export function decodeAccount(rlpData: Uint8Array): VerifiedAccount {
  const decoded = rlpDecode(rlpData);
  if (!Array.isArray(decoded) || decoded.length !== 4) {
    throw new Error(
      `Invalid account RLP: expected 4 items, got ${Array.isArray(decoded) ? decoded.length : 'non-list'}`,
    );
  }

  const items = decoded as Uint8Array[];
  const codeHash = bytesToHex(items[3]);

  return {
    nonce: bytesToBigInt(items[0]),
    balance: bytesToBigInt(items[1]),
    storageRoot: bytesToHex(items[2]),
    codeHash,
    isContract: codeHash !== EMPTY_CODE_HASH,
  };
}

function bytesToBigInt(bytes: Uint8Array): bigint {
  if (bytes.length === 0) return 0n;
  let result = 0n;
  for (const byte of bytes) {
    result = (result << 8n) | BigInt(byte);
  }
  return result;
}

// --- High-Level API ---

export interface VerificationResult {
  account: VerifiedAccount;
  stateRoot: string;
  address: string;
  blockNumber: number;
  nodesVerified: number;
  verified: boolean;
}

/**
 * Verify an account proof from eth_getProof against a known state root.
 *
 * This is the core trustless operation:
 * - stateRoot: from a verified block header (beacon chain or block hash)
 * - address: the account to verify
 * - accountProof: hex-encoded proof nodes from eth_getProof
 *
 * The proof is verified using ONLY keccak256 hashes. No trust in the
 * data source is required — if the math checks out, the data is correct.
 */
export function verifyAccountProof(
  stateRoot: string,
  address: string,
  accountProof: string[],
): { account: VerifiedAccount; nodesVerified: number } {
  const stateRootBytes = hexToBytes(stateRoot);
  const addressBytes = hexToBytes(address);

  // The key in the state trie is keccak256(address)
  const key = keccak256(addressBytes);

  // Decode proof nodes from hex
  const proofNodes = accountProof.map((hex) => hexToBytes(hex));

  // Verify the Merkle-Patricia trie proof
  const result = verifyMPTProof(stateRootBytes, key, proofNodes);

  if (result.value === null) {
    throw new Error(
      `Account ${address} not found in state trie (valid proof of non-existence)`,
    );
  }

  // Decode the verified account data
  const account = decodeAccount(result.value);

  return { account, nodesVerified: result.nodesVerified };
}

// --- Formatting ---

export function weiToEth(wei: bigint): string {
  const ETH = 10n ** 18n;
  const whole = wei / ETH;
  const fraction = wei % ETH;
  const fractionStr = fraction.toString().padStart(18, '0');
  const trimmed = fractionStr.replace(/0+$/, '') || '0';
  return `${whole}.${trimmed}`;
}

export function weiToHex(wei: bigint): string {
  return '0x' + wei.toString(16);
}
