# Audit Findings

Audit target: transaction parsing, sighash, builder, and verifier behavior around network-upgrade activation heights, with emphasis on one-block-before / at / one-block-after boundary behavior.

High-level result: I did not confirm a current consensus bug where the same transaction is silently signed or verified under inconsistent upgrade rules in normal block-driven flows. The branch-selection logic at activation boundaries is mostly coherent:

- `BranchId::for_height` flips exactly at the activation height.
- the builder derives its default transaction version from that branch and rejects incompatible manual overrides.
- block parsing supplies the expected branch for non-coinbase transactions based on the claimed block height.
- v5+ and v6 transactions carry their own consensus branch ID in the serialized header and do not depend on caller-supplied epoch context for parsing.

The reportable issue is a pre-v5 contextual-parse hazard:

- `Low`: the same v3/v4 transaction bytes can be parsed and later sighashed under different consensus branches depending on the caller-supplied context, so safe-looking parsing APIs still permit boundary-dependent reinterpretation for pre-v5 transactions.

Affected code:

- `components/zcash_protocol/src/consensus.rs:772-856`
- `zcash_primitives/src/transaction/mod.rs:228-274`
- `zcash_primitives/src/transaction/mod.rs:603-610`
- `zcash_primitives/src/transaction/mod.rs:816-905`
- `zcash_primitives/src/transaction/sighash.rs:43-63`
- `zcash_primitives/src/transaction/sighash_v4.rs:130-225`
- `zcash_primitives/src/transaction/builder.rs:403-448`
- `zcash_primitives/src/transaction/builder.rs:497-504`
- `zcash_primitives/src/block.rs:253-274`

## Low: pre-v5 parsing and sighash remain caller-contextual across activation boundaries

### What happens

For v3/v4 transactions, `Transaction::read(reader, consensus_branch_id)` does not recover the branch ID from serialized bytes. Instead, it trusts the caller-provided `consensus_branch_id` and stores that into the parsed `TransactionData`.

That stored branch ID then directly affects signature hashing:

- `sighash_v4` personalizes the digest with `tx.consensus_branch_id`;
- Sprout JoinSplit hashing also changes behavior through `consensus_branch_id.sprout_uses_groth_proofs()`.

So the same serialized pre-v5 transaction bytes can be reinterpreted under different epochs if a caller changes only the contextual branch ID passed to `Transaction::read`.

This is not hypothetical in the API design; `TransactionData::fix_consensus_branch_id(...)` exists specifically to rewrite the branch ID of pre-v5 transactions after parsing, and its doc comment says it can be used to fix an incorrect value passed to `Transaction::read`.

### Boundary cases checked

On mainnet, the relevant exact edges currently include:

- Sapling activation: June 26, 2018 at height `419200`
  - `419199` -> `BranchId::Overwinter`
  - `419200` -> `BranchId::Sapling`
  - `419201` -> `BranchId::Sapling`
- NU5 activation: May 31, 2022 at height `1687104`
  - `1687103` -> `BranchId::Canopy`
  - `1687104` -> `BranchId::Nu5`
  - `1687105` -> `BranchId::Nu5`
- NU6 activation: November 23, 2024 at height `2726400`
  - `2726399` -> `BranchId::Nu5`
  - `2726400` -> `BranchId::Nu6`
  - `2726401` -> `BranchId::Nu6`
- NU6.1 activation: height `3146400`
  - `3146399` -> `BranchId::Nu6`
  - `3146400` -> `BranchId::Nu6_1`
  - `3146401` -> `BranchId::Nu6_1`

For v5+ parsing, this contextual hazard is closed because `read_v5` / `read_v6` parse the branch ID from the transaction header itself and ignore the caller-supplied `consensus_branch_id` argument.

### Why this matters

This creates a safe-API mismatch for pre-v5 transactions:

- parsing succeeds under multiple caller-chosen consensus branches;
- the resulting object carries a branch ID that is not committed by the bytes;
- later sighash / signature verification behavior depends on that chosen branch.

At an upgrade boundary, this means the same pre-v5 byte stream can be:

- parsed under the “before” branch and produce one signature hash;
- parsed under the “at/after” branch and produce a different signature hash.

I did not confirm an acceptance bug where an actually invalid transaction becomes valid. The observed failure mode is inconsistent interpretation or later rejection if the wrong contextual branch is supplied.

### Why this is only `Low`

I rate this `Low` because:

- normal block parsing is fail-closed and supplies the correct contextual branch from the block height;
- v5+ transactions commit to their branch ID directly and are not affected;
- I did not find an in-repo path that silently verifies a pre-v5 transaction under the wrong branch and accepts it as valid consensus data.

The risk is that external callers, wallet recovery code, or storage/reparse layers can treat `Transaction::read` as if it were self-contained for pre-v5 bytes when it is not.

## No confirmed boundary mismatch in builder or normal block verification flow

I also checked the default builder and the normal block-driven parse path.

The builder side looked coherent:

- `Builder::new` derives `consensus_branch_id = BranchId::for_height(params, target_height)`;
- it then picks `TxVersion::suggested_for_branch(consensus_branch_id)`;
- `check_version_compatibility` rejects manual version overrides that are not valid in the selected branch, and separately rejects Sapling/Orchard use when the chosen branch/version combination does not support those pools.

The block side also looked coherent:

- `Block::read` extracts the claimed coinbase height;
- computes the expected branch with `BranchId::for_height(params, claimed_height)`;
- reparses/fixes pre-v5 coinbase branch context if needed;
- and parses all non-coinbase transactions with that same expected branch.

So I did not confirm a stronger issue where one-block-before / at / after upgrade causes the builder, parser, or bundled proof/signature verifiers to disagree in the normal on-chain path.

## Residual note

This audit overlaps slightly with the earlier `cross_network_domain_confusion` report: for pre-v5 transactions, any storage or recovery layer that infers a branch heuristically for unmined transactions inherits this same contextual hazard. I did not duplicate that as a separate finding here because it is the same underlying pre-v5 “branch not committed by bytes” property rather than a distinct activation-boundary bug.
