# Audit Findings

Audit target: Mainnet / Testnet / Regtest / consensus-epoch handling across addresses, branch IDs, transaction parsing, sighash, and upgrade-activation boundaries.

High-level result: the stronger network and upgrade boundaries are mostly coherent:

- v5+ transactions commit to their consensus branch ID in the serialized header;
- the builder derives its default version from `BranchId::for_height` and rejects incompatible overrides;
- block parsing supplies the expected branch to non-coinbase transaction parsing based on the claimed block height.

The remaining confirmed issues are both `Low` and both contextual:

- some Testnet address encodings are intentionally accepted as Regtest;
- pre-v5 transactions remain caller-contextual, so the same byte stream can be materialized under a different consensus epoch depending on supplied or inferred branch context, including around one-block-before / at / after upgrade boundaries.

Affected code:

- `components/zcash_protocol/src/consensus.rs:195-199`
- `components/zcash_protocol/src/consensus.rs:772-856`
- `zcash_primitives/src/transaction/mod.rs:228-274`
- `zcash_primitives/src/transaction/mod.rs:603-610`
- `zcash_primitives/src/transaction/mod.rs:816-905`
- `zcash_primitives/src/transaction/sighash.rs:43-63`
- `zcash_primitives/src/transaction/sighash_v4.rs:130-225`
- `zcash_primitives/src/transaction/builder.rs:403-448`
- `zcash_primitives/src/transaction/builder.rs:497-504`
- `zcash_primitives/src/block.rs:253-274`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_address-0.11.0/src/lib.rs:254-274`
- `zcash_keys/src/address.rs:402-406`
- `zcash_client_backend/src/data_api/wallet/input_selection.rs:453-456`

## Low: Testnet Sprout and transparent addresses are intentionally accepted as Regtest

### What happens

For some address kinds, Regtest does not have distinct encodings from Testnet. The address-conversion layer therefore explicitly accepts Testnet Sprout and transparent addresses when Regtest is requested.

This is documented behavior rather than an accidental parser bug, but it still means some cross-network data is accepted under a weaker boundary than callers may expect.

### Why this matters

Callers that assume “Regtest only” implies strict byte-level separation from Testnet for every address kind can be surprised by this compatibility carveout.

### Impact

I rate this `Low` because it is explicit compatibility behavior and not a strong signing or theft issue.

## Low: pre-v5 transactions remain caller-contextual across epochs and activation boundaries

### What happens

For v3/v4 transactions, `Transaction::read(reader, consensus_branch_id)` does not recover the branch ID from the serialized bytes. It trusts the caller-provided branch and stores it into the parsed `TransactionData`.

That stored branch then affects later behavior:

- pre-v5 sighash uses `tx.consensus_branch_id`;
- Sprout JoinSplit hashing changes through `sprout_uses_groth_proofs()`;
- storage / recovery layers that reparse or heuristically infer branch context can materialize the same bytes under different epochs.

This is the same underlying issue that also appeared in the more activation-focused audit. The one-block-before / at / after upgrade boundaries matter because `BranchId::for_height` flips exactly there, so the same pre-v5 bytes can be interpreted under different contextual branches if the supplied height/branch changes.

Concrete mainnet examples:

- height `419199` -> `BranchId::Overwinter`
- height `419200` -> `BranchId::Sapling`
- height `1687103` -> `BranchId::Canopy`
- height `1687104` -> `BranchId::Nu5`
- height `2726399` -> `BranchId::Nu5`
- height `2726400` -> `BranchId::Nu6`

By contrast, v5+ parsing closes this gap because the branch ID is parsed from the transaction header itself.

### Why this matters

I did not confirm a normal block-validation acceptance bug. The problem is contextual reinterpretation:

- the same pre-v5 bytes can be parsed under different epochs;
- the resulting object can later be sighashed or stored under that caller-chosen epoch;
- safe-looking parsing is therefore weaker than it appears for legacy transaction formats.

### Impact

I rate this `Low` because normal block parsing is fail-closed and supplies the right branch from height, but external callers and wallet recovery/storage paths can still misuse the API surface.

## No confirmed stronger network / epoch boundary break

I did not confirm:

- a v5+ cross-epoch parsing ambiguity;
- a builder bug where one-block-before / at / after upgrade chooses an inconsistent version/branch pair;
- or a strong cross-network signature/sighash confusion beyond the two low-severity contextual issues above.
