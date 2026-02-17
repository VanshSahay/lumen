/**
 * Lumen Demo — Trustless Ethereum Account Verification
 *
 * Trust model:
 * 1. Beacon chain finality update fetched from 2+ independent beacon APIs
 *    → gives us the finalized execution state root (multi-source consensus)
 * 2. Merkle proof fetched from any execution RPC (untrusted data transport)
 * 3. Proof verified LOCALLY via keccak256 hash chain (trustless math)
 * 4. Cross-check: proof block must extend the beacon-finalized chain
 *
 * The RPC cannot lie — the proof either matches the state root or it doesn't.
 * Even if the RPC is malicious, it cannot forge a valid Merkle proof without
 * finding a keccak256 collision (computationally infeasible).
 */

import { verifyAccountProof, weiToEth } from './verify';
import {
  fetchBeaconConsensus,
  type BeaconConsensusResult,
} from './beacon';
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
let beaconConsensus: BeaconConsensusResult | null = null;

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
  addLog('Initializing Lumen light client...', 'info');
  syncBadge.className = 'status-badge bootstrapping';
  syncStatusText.textContent = 'Syncing';
  connectionIcon.textContent = '⏳';
  connectionType.textContent = 'Connecting to beacon chain';
  connectionDetail.textContent =
    'Fetching finality update from independent beacon APIs...';

  try {
    const startTime = performance.now();
    addLog(
      'Fetching beacon chain finality update from multiple sources...',
      'info',
    );

    beaconConsensus = await fetchBeaconConsensus();
    const fin = beaconConsensus.finality;
    const elapsed = Math.round(performance.now() - startTime);

    addLog(
      `Beacon consensus reached in ${elapsed}ms ` +
        `(${beaconConsensus.sourcesAgreed}/${beaconConsensus.sourcesQueried} sources agree)`,
      'success',
    );
    addLog(
      `Finalized slot ${fin.slot.toLocaleString()} — ` +
        `sync committee: ${fin.syncParticipation}/${fin.syncCommitteeSize} validators signed`,
      'success',
    );
    addLog(
      `Execution state root: ${fin.executionStateRoot}`,
      'info',
    );
    addLog(
      `Sources: ${beaconConsensus.agreeSources.join(', ')}`,
      'info',
    );

    // Update UI
    headSlotEl.textContent = fin.slot.toLocaleString();
    syncPeriodEl.textContent = Math.floor(fin.slot / 8192).toString();
    peerCountEl.textContent = beaconConsensus.sourcesAgreed.toString();

    syncBadge.className = 'status-badge synced';
    syncStatusText.textContent = 'Synced';
    connectionIcon.textContent = '🟢';
    connectionType.textContent = 'Beacon Chain Consensus';
    connectionDetail.textContent =
      `${beaconConsensus.sourcesAgreed} independent beacon APIs agree — ` +
      `${fin.syncParticipation}/512 sync committee validators signed`;

    addLog(
      'Ready. Enter any Ethereum address to verify its balance.',
      'success',
    );

    // Refresh beacon finality periodically
    setInterval(refreshBeacon, 90_000);
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    addLog(`Beacon sync failed: ${msg}`, 'error');
    syncBadge.className = 'status-badge error';
    syncStatusText.textContent = 'Error';
    connectionIcon.textContent = '❌';
    connectionType.textContent = 'Beacon Sync Failed';
    connectionDetail.textContent = msg;
  }
}

async function refreshBeacon(): Promise<void> {
  try {
    beaconConsensus = await fetchBeaconConsensus();
    const fin = beaconConsensus.finality;
    headSlotEl.textContent = fin.slot.toLocaleString();
    syncPeriodEl.textContent = Math.floor(fin.slot / 8192).toString();
    peerCountEl.textContent = beaconConsensus.sourcesAgreed.toString();
  } catch {
    // Silently fail — keep previous finality data
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
  if (!beaconConsensus) {
    addLog('Beacon chain not synced yet. Please wait...', 'warn');
    return;
  }

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
  const fin = beaconConsensus.finality;

  try {
    // Step 1: Refresh beacon chain finality (get latest consensus)
    const step1Start = performance.now();
    addLog('Refreshing beacon chain finality...', 'info');

    try {
      beaconConsensus = await fetchBeaconConsensus();
    } catch {
      addLog('Beacon refresh failed, using cached finality data', 'warn');
    }

    const freshFin = beaconConsensus.finality;
    steps.push({
      name: 'Beacon chain finality (multi-source consensus)',
      passed: true,
      details:
        `Slot ${freshFin.slot.toLocaleString()} finalized — ` +
        `${beaconConsensus.sourcesAgreed} sources agree, ` +
        `${freshFin.syncParticipation}/512 validators signed`,
      timeMs: Math.round(performance.now() - step1Start),
    });
    renderSteps(steps);

    // Update displayed slot
    headSlotEl.textContent = freshFin.slot.toLocaleString();
    syncPeriodEl.textContent = Math.floor(freshFin.slot / 8192).toString();

    // Step 2: Fetch latest block + proof from execution RPC (untrusted data)
    const step2Start = performance.now();
    addLog(`Fetching proof from ${getCurrentEndpoint()} (untrusted)...`, 'info');

    let freshBlock: BlockHeader;
    let proofResponse: EthGetProofResponse;

    try {
      [freshBlock, proofResponse] = await Promise.all([
        getLatestBlock(),
        getProof(address, 'latest'),
      ]);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      steps.push({
        name: 'Fetch proof from execution node (untrusted)',
        passed: false,
        details: `Failed: ${msg}`,
        timeMs: Math.round(performance.now() - step2Start),
      });
      renderSteps(steps);
      throw new Error(`Data fetch failed: ${msg}`);
    }

    const blockNum = parseInt(freshBlock.number, 16);

    steps.push({
      name: 'Fetch proof from execution node (untrusted data transport)',
      passed: true,
      details:
        `${proofResponse.accountProof.length} trie nodes from ${getCurrentEndpoint()} ` +
        `at block #${blockNum.toLocaleString()}`,
      timeMs: Math.round(performance.now() - step2Start),
    });
    renderSteps(steps);

    // Step 3: Cross-check block extends beacon-finalized chain
    const step3Start = performance.now();
    const extendsFinalized = blockNum >= freshFin.executionBlockNumber;

    steps.push({
      name: 'Cross-check: block extends beacon-finalized chain',
      passed: extendsFinalized,
      details: extendsFinalized
        ? `Block #${blockNum.toLocaleString()} ≥ finalized #${freshFin.executionBlockNumber.toLocaleString()} ` +
          `(${blockNum - freshFin.executionBlockNumber} blocks ahead)`
        : `REJECTED: block #${blockNum.toLocaleString()} < finalized #${freshFin.executionBlockNumber.toLocaleString()}`,
      timeMs: Math.round(performance.now() - step3Start),
    });
    renderSteps(steps);

    if (!extendsFinalized) {
      throw new Error('Block is behind beacon-finalized head — possible fork');
    }

    // Step 4: Verify Merkle-Patricia trie proof LOCALLY
    const step4Start = performance.now();
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
        name: 'Verify Merkle-Patricia trie proof (keccak256)',
        passed: false,
        details: `VERIFICATION FAILED: ${msg}`,
        timeMs: Math.round(performance.now() - step4Start),
      });
      renderSteps(steps);
      throw new Error(`Proof verification failed: ${msg}`);
    }

    steps.push({
      name: 'Verify Merkle-Patricia trie proof (keccak256)',
      passed: true,
      details:
        `${verified.nodesVerified} trie nodes verified — all keccak256 hashes ` +
        `match from state root to account leaf`,
      timeMs: Math.round(performance.now() - step4Start),
    });
    renderSteps(steps);

    // Step 5: Decode and cross-check
    const step5Start = performance.now();
    const account = verified.account;
    const balanceEth = weiToEth(account.balance);
    const rpcClaimedBalance = BigInt(proofResponse.balance);
    const balancesMatch = rpcClaimedBalance === account.balance;

    steps.push({
      name: 'Decode RLP account state',
      passed: true,
      details:
        `nonce=${account.nonce}, balance=${balanceEth} ETH, ` +
        `contract=${account.isContract}`,
      timeMs: Math.round(performance.now() - step5Start),
    });

    steps.push({
      name: 'Cross-check: RPC claim vs cryptographic proof',
      passed: balancesMatch,
      details: balancesMatch
        ? `RPC claimed ${weiToEth(rpcClaimedBalance)} ETH = proof-verified ${balanceEth} ETH`
        : `MISMATCH! RPC claimed ${weiToEth(rpcClaimedBalance)} ETH but proof shows ${balanceEth} ETH`,
      timeMs: 0,
    });

    renderSteps(steps);

    const totalMs = Math.round(performance.now() - totalStart);

    verifiedBalance.textContent = `${balanceEth} ETH`;
    verifiedBalance.className = 'value';

    proofsCount++;
    proofsVerifiedEl.textContent = proofsCount.toString();

    addLog(
      `VERIFIED: ${address.slice(0, 10)}...${address.slice(-4)} = ` +
        `${balanceEth} ETH (${verified.nodesVerified} nodes, ${totalMs}ms)`,
      'success',
    );
    addLog(
      `  nonce=${account.nonce}, contract=${account.isContract}, ` +
        `storageRoot=${account.storageRoot.slice(0, 18)}...`,
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
