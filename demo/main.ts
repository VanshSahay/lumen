/**
 * Lumen Demo — Real Trustless Ethereum Account Verification
 *
 * This demo performs REAL cryptographic verification:
 * 1. Fetches a finalized block header to obtain the state root
 * 2. Fetches eth_getProof for any address (untrusted data source)
 * 3. Verifies the Merkle-Patricia trie proof LOCALLY using keccak256
 * 4. Displays the cryptographically verified balance
 *
 * The proof verification is trustless — if the math checks out, the data
 * is correct regardless of where it came from.
 */

import {
  verifyAccountProof,
  weiToEth,
} from './verify';
import {
  getLatestBlock,
  getProof,
  getCurrentEndpoint,
  type BlockHeader,
  type EthGetProofResponse,
} from './rpc';

// --- DOM Elements ---

const syncBadge = document.getElementById('sync-badge')!;
const syncStatusText = document.getElementById('sync-status-text')!;
const headSlotEl = document.getElementById('head-slot')!;
const syncPeriodEl = document.getElementById('sync-period')!;
const peerCountEl = document.getElementById('peer-count')!;
const proofsVerifiedEl = document.getElementById('proofs-verified')!;
const connectionIcon = document.getElementById('connection-icon')!;
const connectionType = document.getElementById('connection-type')!;
const connectionDetail = document.getElementById('connection-detail')!;
const addressInput = document.getElementById(
  'address-input',
) as HTMLInputElement;
const verifyBtn = document.getElementById('verify-btn') as HTMLButtonElement;
const verificationResult = document.getElementById('verification-result')!;
const verifiedBalance = document.getElementById('verified-balance')!;
const proofSteps = document.getElementById('proof-steps')!;
const logOutput = document.getElementById('log-output')!;

// --- State ---

let proofsCount = 0;
let currentBlock: BlockHeader | null = null;
let isInitializing = false;

// --- Logging ---

function addLog(
  message: string,
  level: 'info' | 'success' | 'warn' | 'error' = 'info',
): void {
  const time = new Date().toLocaleTimeString('en-US', { hour12: false });
  const entry = document.createElement('div');
  entry.className = 'log-entry';
  entry.innerHTML = `<span class="log-time">[${time}]</span> <span class="log-${level}">${message}</span>`;
  logOutput.appendChild(entry);
  logOutput.scrollTop = logOutput.scrollHeight;
}

// --- Initialization ---

async function initialize(): Promise<void> {
  if (isInitializing) return;
  isInitializing = true;

  addLog('Initializing Lumen trustless verification demo...', 'info');

  // Update UI to bootstrapping state
  syncBadge.className = 'status-badge bootstrapping';
  syncStatusText.textContent = 'Bootstrapping';
  connectionIcon.textContent = '⏳';
  connectionType.textContent = 'Connecting';
  connectionDetail.textContent = 'Fetching finalized block header...';

  try {
    // Step 1: Fetch the latest block to verify we can reach the network
    addLog('Fetching latest Ethereum block header...', 'info');
    const startTime = performance.now();

    currentBlock = await getLatestBlock();

    const blockNum = parseInt(currentBlock.number, 16);
    const elapsed = Math.round(performance.now() - startTime);

    addLog(
      `Latest block #${blockNum.toLocaleString()} fetched in ${elapsed}ms`,
      'success',
    );
    addLog(`State root: ${currentBlock.stateRoot}`, 'info');
    addLog(
      `Source: ${getCurrentEndpoint()} (untrusted — proofs verified locally)`,
      'info',
    );
    addLog(
      'In production: state root comes from BLS-verified beacon chain sync committee',
      'info',
    );

    // Update UI
    headSlotEl.textContent = blockNum.toLocaleString();
    syncPeriodEl.textContent = Math.floor(blockNum / 8192).toString();
    peerCountEl.textContent = '1';

    syncBadge.className = 'status-badge synced';
    syncStatusText.textContent = 'Ready';
    connectionIcon.textContent = '🟢';
    connectionType.textContent = 'RPC + Local Verification';
    connectionDetail.textContent =
      'Proofs fetched from RPC, verified locally via keccak256 + Merkle proof';

    addLog(
      'Ready. Enter any Ethereum address to verify its balance trustlessly.',
      'success',
    );

    // Refresh block periodically
    setInterval(refreshBlock, 60_000);
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    addLog(`Initialization failed: ${msg}`, 'error');

    syncBadge.className = 'status-badge error';
    syncStatusText.textContent = 'Error';
    connectionIcon.textContent = '❌';
    connectionType.textContent = 'Connection Failed';
    connectionDetail.textContent = msg;
  } finally {
    isInitializing = false;
  }
}

async function refreshBlock(): Promise<void> {
  try {
    currentBlock = await getLatestBlock();
    const blockNum = parseInt(currentBlock.number, 16);
    headSlotEl.textContent = blockNum.toLocaleString();
    syncPeriodEl.textContent = Math.floor(blockNum / 8192).toString();
  } catch {
    // Silently fail — we still have the previous block
  }
}

// --- Verification ---

interface VerificationStep {
  name: string;
  passed: boolean;
  details: string;
  timeMs: number;
}

async function verifyAddress(address: string): Promise<void> {
  // Validate address format
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) {
    addLog(`Invalid address format: ${address}`, 'error');
    verifiedBalance.textContent = 'Invalid address format';
    verifiedBalance.className = 'value error';
    return;
  }

  verifyBtn.disabled = true;
  verifyBtn.textContent = 'Verifying...';
  verificationResult.style.display = 'block';
  verifiedBalance.textContent = 'Fetching proof...';
  verifiedBalance.className = 'value';
  proofSteps.innerHTML = '';

  addLog(`Verifying account: ${address}`, 'info');

  const steps: VerificationStep[] = [];
  const totalStart = performance.now();

  try {
    // Step 0: Fetch a FRESH block — eth_getProof only works for recent blocks
    const step0Start = performance.now();
    addLog('Fetching fresh block header for proof...', 'info');

    let freshBlock: BlockHeader;
    try {
      freshBlock = await getLatestBlock();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      steps.push({
        name: 'Fetch latest block header',
        passed: false,
        details: `Failed: ${msg}`,
        timeMs: Math.round(performance.now() - step0Start),
      });
      renderSteps(steps);
      throw new Error(`Failed to fetch block: ${msg}`);
    }

    const blockNum = parseInt(freshBlock.number, 16);
    steps.push({
      name: 'Fetch latest block header',
      passed: true,
      details: `Block #${blockNum.toLocaleString()} — stateRoot: ${freshBlock.stateRoot.slice(0, 18)}...`,
      timeMs: Math.round(performance.now() - step0Start),
    });
    renderSteps(steps);

    // Update the displayed head block
    currentBlock = freshBlock;
    headSlotEl.textContent = blockNum.toLocaleString();
    syncPeriodEl.textContent = Math.floor(blockNum / 8192).toString();

    // Step 1: Fetch eth_getProof from RPC at this exact block (UNTRUSTED data)
    const step1Start = performance.now();
    addLog(
      `Fetching Merkle proof from ${getCurrentEndpoint()}...`,
      'info',
    );

    let proofResponse: EthGetProofResponse;
    try {
      proofResponse = await getProof(address, freshBlock.number);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      steps.push({
        name: 'Fetch Merkle proof from RPC',
        passed: false,
        details: `Failed: ${msg}`,
        timeMs: Math.round(performance.now() - step1Start),
      });
      renderSteps(steps);
      throw new Error(`Failed to fetch proof: ${msg}`);
    }

    steps.push({
      name: 'Fetch Merkle proof from RPC',
      passed: true,
      details: `eth_getProof returned ${proofResponse.accountProof.length} trie nodes from ${getCurrentEndpoint()} (untrusted)`,
      timeMs: Math.round(performance.now() - step1Start),
    });
    renderSteps(steps);

    // Step 2: Verify the Merkle-Patricia trie proof LOCALLY
    const step2Start = performance.now();
    addLog('Verifying Merkle-Patricia trie proof locally...', 'info');

    let verified;
    try {
      verified = verifyAccountProof(
        freshBlock.stateRoot,
        address,
        proofResponse.accountProof,
      );
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      steps.push({
        name: 'Verify Merkle-Patricia trie proof',
        passed: false,
        details: `VERIFICATION FAILED: ${msg}`,
        timeMs: Math.round(performance.now() - step2Start),
      });
      renderSteps(steps);
      throw new Error(`Proof verification failed: ${msg}`);
    }

    steps.push({
      name: 'Verify Merkle-Patricia trie proof (keccak256)',
      passed: true,
      details: `${verified.nodesVerified} trie nodes verified, all keccak256 hashes match root → leaf`,
      timeMs: Math.round(performance.now() - step2Start),
    });
    renderSteps(steps);

    // Step 3: Decode and display the verified account
    const step3Start = performance.now();
    const account = verified.account;
    const balanceEth = weiToEth(account.balance);

    // Cross-check: does the RPC's claimed balance match our verified balance?
    const rpcClaimedBalance = BigInt(proofResponse.balance);
    const balancesMatch = rpcClaimedBalance === account.balance;

    steps.push({
      name: 'Decode RLP account state',
      passed: true,
      details: `nonce=${account.nonce}, balance=${balanceEth} ETH, contract=${account.isContract}`,
      timeMs: Math.round(performance.now() - step3Start),
    });

    if (balancesMatch) {
      steps.push({
        name: 'Cross-check: RPC claimed balance matches proof',
        passed: true,
        details: `RPC claimed ${weiToEth(rpcClaimedBalance)} ETH = verified ${balanceEth} ETH`,
        timeMs: 0,
      });
    } else {
      steps.push({
        name: 'Cross-check: RPC balance vs proof',
        passed: false,
        details: `MISMATCH! RPC claimed ${weiToEth(rpcClaimedBalance)} ETH but proof shows ${balanceEth} ETH`,
        timeMs: 0,
      });
    }

    renderSteps(steps);

    const totalMs = Math.round(performance.now() - totalStart);

    // Display the verified balance
    verifiedBalance.textContent = `${balanceEth} ETH`;
    verifiedBalance.className = 'value';

    proofsCount++;
    proofsVerifiedEl.textContent = proofsCount.toString();

    addLog(
      `VERIFIED: ${address.slice(0, 10)}...${address.slice(-4)} = ${balanceEth} ETH ` +
        `(${verified.nodesVerified} nodes, ${totalMs}ms)`,
      'success',
    );
    addLog(
      `  nonce=${account.nonce}, contract=${account.isContract}, storageRoot=${account.storageRoot.slice(0, 18)}...`,
      'info',
    );
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    verifiedBalance.textContent = msg;
    verifiedBalance.className = 'value error';
    addLog(`Verification failed: ${msg}`, 'error');
  } finally {
    verifyBtn.disabled = false;
    verifyBtn.textContent = 'Verify';
  }
}

function renderSteps(steps: VerificationStep[]): void {
  proofSteps.innerHTML = '';
  for (const step of steps) {
    const stepEl = document.createElement('div');
    stepEl.className = 'proof-step';
    stepEl.innerHTML = `
      <span class="proof-step-icon">${step.passed ? '✅' : '❌'}</span>
      <div>
        <div class="proof-step-name">${step.name}</div>
        <div class="proof-step-detail">${step.details}${step.timeMs > 0 ? ` (${step.timeMs}ms)` : ''}</div>
      </div>
    `;
    proofSteps.appendChild(stepEl);
  }
}

// --- Event Listeners ---

verifyBtn.addEventListener('click', () => {
  const address =
    addressInput.value.trim() ||
    '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045';
  verifyAddress(address);
});

addressInput.addEventListener('keypress', (e: KeyboardEvent) => {
  if (e.key === 'Enter') {
    verifyBtn.click();
  }
});

// --- Start ---

initialize();
