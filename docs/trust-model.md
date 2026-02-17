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

**Attack scenario:** A breakthrough in discrete logarithm algorithms allows forging BLS signatures. An attacker could create fake sync committee signatures for arbitrary block headers.

**Probability:** Extremely low. BLS12-381 is a well-studied curve with a 128-bit security level. No known attacks reduce this below ~100 bits. A quantum computer with thousands of logical qubits could break it, but such hardware doesn't exist.

**Impact:** Total — an attacker could make Lumen accept any block header.

**What would eliminate this:** Post-quantum signature schemes. Ethereum itself would need to migrate, which is an active research area.

---

## Component: Sync Committee Trust (2/3 Honest Assumption)

**What it trusts:** That at least 342 of the 512 sync committee members are honest during their ~27-hour duty period.

**Attack scenario:** An attacker controls 171+ of the 512 sync committee positions. They could sign a fake block header that Lumen would accept. This requires the attacker to control a massive amount of staked ETH (each validator needs 32 ETH).

**Probability:** Very low. Sync committee members are randomly sampled from all active validators (~900,000 as of 2024). Controlling 33% of them requires controlling ~33% of all staked ETH (~$30B+ at current prices). This is the same assumption that secures Ethereum's consensus itself.

**Impact:** Total — the attacker could make Lumen accept a fake chain state.

**Mitigation already in place:** This is literally the same trust assumption as running an Ethereum full node. If this assumption breaks, Ethereum itself is compromised, not just Lumen.

**What would eliminate this:** Nothing — this is fundamental to Ethereum's security model. You could reduce it to 1/2 honest with different protocols, but 2/3 is Ethereum's design choice.

---

## Component: Checkpoint Initialization

**What it trusts:** That at least N of the configured checkpoint sources agree on the current finalized block root, and that this agreement reflects the actual chain state.

**Attack scenario:** An attacker compromises N checkpoint sources (e.g., beaconcha.in, beaconstate.info, sigp.io) and makes them all return a fake checkpoint. Lumen would initialize from a fake starting point and could be served a valid-looking but incorrect chain history.

**Probability:** Low but non-zero. The checkpoint sources are run by different organizations in different jurisdictions. Compromising 3+ of them simultaneously is difficult but not impossible (e.g., a state-level actor or a compromised dependency in all of them).

**Impact:** High — if the checkpoint is wrong, Lumen's entire chain view is wrong from that point.

**Mitigation already in place:**
- Multiple independent sources (default: 5)
- Required agreement threshold (default: 3)
- Sources operated by diverse organizations
- The checkpoint can be manually verified by the user

**What would eliminate this:**
- Embed a known-good finalized checkpoint in the Lumen binary (but this is stale)
- Use P2P peers themselves as checkpoint sources (chicken-and-egg problem)
- Use social consensus (ask your friends what the current checkpoint is)
- Use a zkSNARK that proves the checkpoint is on the canonical chain (active research)

---

## Component: Merkle-Patricia Trie Proofs

**What it trusts:** That keccak256 is collision-resistant. A proof is valid if and only if the chain of hashes from leaf to root is consistent.

**Attack scenario:** An attacker finds a keccak256 collision — two different account states that produce the same hash. They could construct a proof that appears valid for a different account state.

**Probability:** Effectively zero. Keccak256 has a 256-bit output with no known collision attacks below the birthday bound (~2^128 operations). This is computationally infeasible.

**Impact:** High — the attacker could make Lumen report incorrect account balances.

**What would eliminate this:** This is already as trust-minimized as mathematically possible. The proof verification is purely computational with no social or economic assumptions.

---

## Component: Circuit Relay (Bootstrap)

**What it trusts:** The relay for connection metadata only — who is connecting to whom, and when.

**What it explicitly does NOT trust:** The relay for data integrity or confidentiality. All data is encrypted with the Noise protocol. The relay cannot read or modify it.

**Attack scenario 1 (metadata):** A malicious relay logs all connection patterns. They know your IP address and which Ethereum peers you connect to.

**Probability:** High — any relay operator can do this trivially.

**Impact:** Low — this reveals network patterns but not query data (which addresses you look up, etc.).

**Attack scenario 2 (selective relay):** A malicious relay only relays connections to peers it controls, which then feed Lumen manipulated (but invalid) data.

**Probability:** Medium — possible but the manipulated data would fail BLS/Merkle verification.

**Impact:** None on correctness (data is still verified). Impact is on availability (Lumen can't sync if all relay-introduced peers send garbage).

**Mitigation already in place:**
- Relay is used only for bootstrapping (3 second timeout)
- Once direct peers are found, relay is dropped
- All data from relay-introduced peers is verified identically to direct peers
- Multiple relays can be configured

**What would eliminate this:**
- Direct WebTransport connections (no relay needed) — this is already the preferred path
- If browsers supported raw QUIC, relay wouldn't be needed at all
- Pre-cached peer list in the binary

---

## Component: P2P Peers

**What it trusts:** Nothing. Peers are untrusted data sources.

**Attack scenario 1 (lies):** A peer sends a fake `LightClientUpdate` with an invalid BLS signature.

**Result:** `lumen-core` verification fails. The update is silently rejected. The peer's score is reduced.

**Attack scenario 2 (omission):** Peers refuse to send updates, keeping Lumen stuck on an old state.

**Mitigation:** Connect to multiple peers. If no peer provides updates for an unusual duration, log a warning. The user can manually provide a fallback.

**Attack scenario 3 (eclipse):** An attacker surrounds Lumen with malicious peers that all send the same invalid data.

**Result:** All their data fails BLS verification. Lumen cannot sync but also cannot be tricked.

**Impact:** Availability only — Lumen may not sync but will never accept bad data.

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

**Attack scenario:** A browser bug causes the WASM module to compute incorrect BLS verification results, accepting invalid signatures.

**Probability:** Very low. Browser WASM runtimes (V8, SpiderMonkey, JavaScriptCore) are among the most tested and fuzzed software in the world.

**Impact:** Total — incorrect verification means incorrect chain state.

**Mitigation:** Use multiple browsers independently. If they disagree, something is wrong.

**What would eliminate this:** Run Lumen natively instead of in WASM. But then you lose the "works in any browser" property.

---

## Component: RLP and SSZ Decoding

**What it trusts:** That our implementation correctly decodes RLP (Recursive Length Prefix) and SSZ (Simple Serialize) encoded data.

**Attack scenario:** A parsing bug causes Lumen to misinterpret a valid proof, either accepting invalid data or rejecting valid data.

**Probability:** Low-medium. Parsing bugs are a common vulnerability class. Our implementation is tested against known test vectors, but edge cases may exist.

**Impact:** Variable — could cause false accepts or false rejects.

**Mitigation:** Extensive test suite with real Ethereum mainnet data. Fuzzing. Using well-tested libraries where possible.

---

## Summary Table

| Component | Trusts | Attack Probability | Impact | Eliminable? |
|-----------|--------|-------------------|--------|-------------|
| BLS12-381 | Crypto hardness | Near-zero | Total | Post-quantum (future) |
| Sync committee | 2/3 honest validators | Very low ($30B+ attack) | Total | No (fundamental) |
| Checkpoint init | N/M sources honest | Low | High | zk proofs (research) |
| Merkle proofs | keccak256 collision resistance | Near-zero | High | Already minimal |
| Circuit relay | Metadata only | Relay can see metadata | Low | Direct connections |
| P2P peers | Nothing | N/A | Availability only | Already trustless |
| Fallback RPC | EVM execution (eth_call) | Trivial for RPC | App-dependent | zk-EVM (roadmap) |
| WASM runtime | Browser correctness | Very low | Total | Run natively |
| RLP/SSZ parsing | Our implementation | Low-medium | Variable | More testing/fuzzing |

---

## What Lumen Does NOT Protect Against

For completeness, things explicitly out of scope:

1. **Browser compromise** — if your browser is malware, nothing helps
2. **OS compromise** — if the OS is compromised, all bets are off
3. **Social engineering** — if someone tricks you into using a fake Lumen, that's not a crypto problem
4. **Network censorship** — if your ISP blocks all P2P traffic, Lumen can't connect (but also can't be fed fake data)
5. **Key management** — Lumen is a read-only light client, not a wallet. It doesn't hold keys.
