# Integration Tests

These tests run against real Ethereum mainnet data to verify that Lumen's
verification pipeline produces correct results on actual chain data.

## Test Categories

### Sync Committee Verification
- Use a known finalized block header and the corresponding sync committee signature
- Verify the BLS aggregate signature passes
- Mutate one byte — verify it fails

### Merkle Proof Verification
- Use a known account (e.g., Ethereum Foundation) at a known block
- Fetch the actual eth_getProof response from a public API
- Verify the proof against the known state root
- Mutate one proof node — verify it fails

### Checkpoint Consensus
- Fetch from multiple checkpoint sources
- Verify agreement logic works with both matching and conflicting hashes

## Running

```bash
cargo test --test integration -- --ignored
```

Integration tests are `#[ignore]` by default since they require network access.
