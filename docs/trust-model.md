# Lumen Trust Model

This document is intentionally blunt. For every component of Lumen, we answer:
1. What does it trust?
2. Under what attack scenario would that trust be exploited?
3. What is the probability and impact?
4. What would it take to eliminate that trust assumption entirely?

If you find something we're being evasive about, file an issue. Honesty about trust is the entire point.

---

## Component: BLS Signature Verification

**What it trusts:** That the BLS12-381 cryptographic scheme is sound. Specifically, that the Discrete Logarithm Problem on the BLS12-381 curve is hard.

**Current status:** BLS verification is implemented in the `lumen-core` Rust crate and compiled to WASM. The demo currently uses multi-source beacon API consensus as an approximation — BLS signature verification will be activated when the full light client sync pipeline is connected end-to-end.

**Attack scenario:** A breakthrough in discrete logarithm algorithms allows forging BLS signatures. An attacker could create fake sync committee signatures for arbitrary block headers.

**Probability:** Extremely low. BLS12-381 is a well-studied curve with a 128-bit security level. No known attacks reduce this below ~100 bits. A quantum computer with thousands of logical qubits could break it, but such hardware doesn't exist.

**Impact:** Total — an attacker could make Lumen accept any block header.

**What would eliminate this:** Post-quantum signature schemes. Ethereum itself would need to migrate, which is an active research area.

---

## Component: Sync Committee Trust (2/3 Honest Assumption)

**What it trusts:** That at least 342 of the 512 sync committee members are honest during their ~27-hour duty period.

**Current status:** The demo fetches finality updates from beacon APIs that include the sync committee participation bitvector. Typically 500+ of 512 validators sign (the demo displays the exact count). The BLS signature is present in the data and available for verification.

**Attack scenario:** An attacker controls 171+ of the 512 sync committee positions. They could sign a fake block header that Lumen would accept. This requires the attacker to control a massive amount of staked ETH (each validator needs 32 ETH).

**Probability:** Very low. Sync committee members are randomly sampled from all active validators (~900,000+). Controlling 33% of them requires controlling ~33% of all staked ETH ($30B+ at current prices). This is the same assumption that secures Ethereum's consensus itself.

**Impact:** Total — the attacker could make Lumen accept a fake chain state.

**Mitigation already in place:** This is literally the same trust assumption as running an Ethereum full node. If this assumption breaks, Ethereum itself is compromised, not just Lumen.

**What would eliminate this:** Nothing — this is fundamental to Ethereum's security model.

---

## Component: Beacon Chain API Consensus (State Root)

**What it trusts:** That at least N of the configured beacon API providers are honest and serve the real finalized state root.

**Current status:** The demo fetches the light client finality update from 2 independent beacon APIs (ChainSafe Lodestar and PublicNode Beacon) and requires both to agree on the same finalized execution state root. If they disagree, Lumen halts and reports the discrepancy.

**Attack scenario:** An attacker compromises all configured beacon API providers and makes them return a fake finalized state root. Lumen would verify proofs against a wrong root, accepting incorrect account states.

**Probability:** Low. The beacon APIs are run by different organizations (ChainSafe and PublicNode) with different infrastructure. Compromising both simultaneously is difficult. Adding more independent sources further reduces the probability.

**Impact:** High — if the state root is wrong, all proof verifications produce wrong results.

**Mitigation already in place:**
- Multiple independent sources required to agree
- The finality update includes the full sync aggregate (committee bits + BLS signature) which can be verified locally
- Sources are operated by different organizations in different jurisdictions
- In production, BLS verification of the sync committee signature makes this fully trustless — the beacon API becomes a mere data transport

**What would eliminate this:**
- Enable BLS signature verification on the finality update (code exists in `lumen-core`, needs end-to-end integration)
- Use P2P gossipsub to receive finality updates directly from the Ethereum network
- Increase the number of independent beacon API sources

**Key difference from RPC trust:** Traditional RPC providers (Infura, Alchemy) are trusted for both the state root AND the data. Lumen separates these: even with beacon API consensus for the state root, all proof data is verified locally via keccak256. The beacon API consensus can be eliminated entirely once BLS verification is connected.

---

## Component: Merkle-Patricia Trie Proofs

**What it trusts:** That keccak256 is collision-resistant. A proof is valid if and only if the chain of hashes from leaf to root is consistent.

**Current status:** Fully implemented and working. The demo verifies real Merkle proofs from Ethereum mainnet in the browser, producing balances that match Etherscan exactly.

**Attack scenario:** An attacker finds a keccak256 collision — two different account states that produce the same hash. They could construct a proof that appears valid for a different account state.

**Probability:** Effectively zero. Keccak256 has a 256-bit output with no known collision attacks below the birthday bound (~2^128 operations). This is computationally infeasible.

**Impact:** High — the attacker could make Lumen report incorrect account balances.

**What would eliminate this:** This is already as trust-minimized as mathematically possible. The proof verification is purely computational with no social or economic assumptions.

---

## Component: Execution RPC (Data Transport)

**What it trusts:** Nothing. The execution RPC is an untrusted data pipe.

**Current status:** The demo fetches `eth_getProof` responses from public execution RPCs (PublicNode, LlamaRPC). The proof bytes are then verified locally against the beacon-chain-sourced state root.

**Attack scenario 1 (forged proof):** The RPC returns a forged `eth_getProof` response with incorrect account data.

**Result:** The Merkle proof verification fails because `keccak256(forged_node) != expected_hash`. The proof is rejected. The RPC cannot forge a valid proof without finding a keccak256 collision.

**Attack scenario 2 (stale data):** The RPC returns a valid proof but from an old block, showing an outdated balance.

**Mitigation:** The demo cross-checks that the proof block number is >= the beacon-finalized block number. A proof from a very old block would fail this check.

**Attack scenario 3 (omission):** The RPC refuses to serve proofs for certain addresses.

**Impact:** Availability only — Lumen cannot verify the address but also cannot be tricked. Multiple RPC endpoints are tried as fallback.

**What would eliminate this entirely:**
- Portal Network state network (in development) for P2P proof retrieval
- Running your own execution node
- Any data source works — the verification is source-independent

---

## Component: Fallback RPC (for eth_call only)

**What it trusts:** The RPC's EVM execution results. This is the ONE exception.

**Attack scenario:** The RPC returns a wrong `eth_call` result. For example, it says a token balance is X when it's actually Y.

**Probability:** The RPC operator could do this trivially and silently.

**Impact:** Application-dependent. If your dApp uses `eth_call` to display a token balance, it could show the wrong number. If it uses `eth_call` to determine whether to approve a transaction, it could be manipulated.

**Mitigation already in place:**
- `eth_call` results are clearly marked as unverified in the API
- Console warning on every `eth_call`
- Documentation explicitly states this is not trustless

**What would eliminate this:**
- Embed a zk-EVM prover in the browser that generates proofs of EVM execution
- This is technically feasible (projects like SP1 and RISC Zero are working on it) but not yet practical for browser deployment due to proving time and prover size
- This is Lumen's roadmap item — when zk-EVM provers are small and fast enough, `eth_call` becomes fully trustless

---

## Component: WASM Runtime

**What it trusts:** That the browser's WASM runtime correctly executes the Lumen WASM binary.

**Current status:** The `lumen-wasm` crate compiles to a 298 KB WASM binary (115 KB gzipped). It contains BLS verification, Merkle proof verification, and RLP/SSZ decoding. The demo currently runs MPT verification in TypeScript for simplicity; the WASM module is available for integration.

**Attack scenario:** A browser bug causes the WASM module to compute incorrect BLS verification results, accepting invalid signatures.

**Probability:** Very low. Browser WASM runtimes (V8, SpiderMonkey, JavaScriptCore) are among the most tested and fuzzed software in the world.

**Impact:** Total — incorrect verification means incorrect chain state.

**Mitigation:** Use multiple browsers independently. If they disagree, something is wrong.

**What would eliminate this:** Run Lumen natively instead of in WASM. But then you lose the "works in any browser" property.

---

## Component: RLP and SSZ Decoding

**What it trusts:** That our implementation correctly decodes RLP (Recursive Length Prefix) and SSZ (Simple Serialize) encoded data.

**Current status:** RLP decoding is implemented in both Rust (`lumen-core`) and TypeScript (`demo/verify.ts`). The TypeScript implementation is used in the demo and has been tested against real Ethereum mainnet data.

**Attack scenario:** A parsing bug causes Lumen to misinterpret a valid proof, either accepting invalid data or rejecting valid data.

**Probability:** Low-medium. Parsing bugs are a common vulnerability class. Our implementations are tested against real Ethereum data, but edge cases may exist.

**Impact:** Variable — could cause false accepts or false rejects.

**Mitigation:** Extensive test suite with real Ethereum mainnet data. The Rust crate has 30+ tests. The demo's TypeScript verification has been validated against real `eth_getProof` responses, producing balances that exactly match Etherscan.

---

## Summary Table

| Component | Trusts | Attack Probability | Impact | Status |
|-----------|--------|-------------------|--------|--------|
| BLS12-381 | Crypto hardness | Near-zero | Total | Implemented in Rust, pending end-to-end integration |
| Sync committee | 2/3 honest validators | Very low ($30B+ attack) | Total | Finality data fetched; BLS verification ready |
| Beacon API consensus | N/N sources honest | Low | High | Working — 2 sources required to agree |
| Merkle proofs | keccak256 collision resistance | Near-zero | High | **Working** — real mainnet verification |
| Execution RPC | Nothing (untrusted) | N/A | None (verified locally) | **Working** — proof data verified via keccak256 |
| Fallback RPC | EVM execution (eth_call) | Trivial for RPC | App-dependent | Documented as sole trust exception |
| WASM runtime | Browser correctness | Very low | Total | Built and compiled (115 KB gzip) |
| RLP/SSZ parsing | Our implementation | Low-medium | Variable | Tested against real mainnet data |

---

## What Lumen Does NOT Protect Against

For completeness, things explicitly out of scope:

1. **Browser compromise** — if your browser is malware, nothing helps
2. **OS compromise** — if the OS is compromised, all bets are off
3. **Social engineering** — if someone tricks you into using a fake Lumen, that's not a crypto problem
4. **Network censorship** — if your ISP blocks all connections, Lumen can't fetch data (but also can't be fed fake data)
5. **Key management** — Lumen is a read-only light client, not a wallet. It doesn't hold keys.

---

## What's Working Now vs Roadmap

**Working now (demo):**
- Beacon chain finality consensus from 2 independent APIs
- Real `eth_getProof` fetching from execution RPCs
- Full Merkle-Patricia trie proof verification (keccak256 + RLP)
- Cross-check: proof block extends beacon-finalized chain
- Cross-check: RPC-claimed balance matches proof-verified balance
- Verified balances match Etherscan exactly

**Implemented but not yet connected end-to-end:**
- BLS12-381 signature verification (in `lumen-core` Rust crate)
- WASM bindings for all verification functions (in `lumen-wasm`)
- P2P transport types and gossipsub configuration (in `lumen-p2p`)
- EIP-1193 provider implementation (in `lumen-js`)

**Roadmap:**
- Connect BLS verification to finality updates (eliminates beacon API trust)
- P2P gossipsub for receiving finality updates directly
- Portal Network integration for P2P proof retrieval
- zk-EVM prover for trustless `eth_call`
