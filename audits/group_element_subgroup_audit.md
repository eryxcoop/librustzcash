# Audit Findings

Audit target: group-element decoding, subgroup validation, signature verification-key parsing, key agreement, and delayed semantic validation across Jubjub / Pallas-related code.

High-level result: the recurring pattern across the earlier curve-validation reports is delayed rejection, not successful consensus acceptance. Safe-looking parsing APIs sometimes admit canonical group-element encodings that are only rejected later by consensus or bundle-level verifiers.

This consolidated report covers the overlapping findings previously split across:

- subgroup / small-order acceptance in note-encryption-facing group elements;
- RedJubjub / RedPallas verification-key validation gaps.

Affected code:

- `zcash_client_backend/src/proto.rs:149-157`
- `zcash_client_backend/src/proto.rs:189-197`
- `zcash_primitives/src/transaction/components/sapling.rs:142-150`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/verifier.rs:45-50`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/bundle.rs:372-389`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt/parse.rs:71-80`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt/parse.rs:190-198`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/action.rs:33-54`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/action.rs:232-236`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/parse.rs:100-122`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/prover.rs:88-100`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/tx_extractor.rs:72-84`

## Low: Sapling `epk` subgroup validation is deferred

### What happens

Several Sapling-facing APIs accept output ephemeral keys as raw bytes or decode them only as generic Jubjub points before later consensus validation rejects small-order values.

That means small-order Jubjub `epk` values can get through some safe-looking parsing or key-agreement-facing APIs before the later verifier enforces the actual consensus rule.

### Why this matters

I did not confirm theft or consensus bypass. The problem is acceptance/rejection mismatch:

- an ergonomic parse/decrypt-facing API accepts the value;
- a later verifier rejects it.

### Impact

I rate this `Low` because rejection still happens before successful consensus verification.

## Low: Sapling / Orchard verification-key parsing is also delayed relative to later semantic validation

### What happens

The same delayed-validation pattern exists for RedJubjub / RedPallas verification keys:

- Sapling parsing can accept `rk` values that are only rejected later when the verifier checks for small order;
- Orchard plain RedPallas verification-key parsing accepts the identity encoding, and Orchard PCZT can carry that state until later prover / extractor rejection.

So both note-encryption-facing group elements and signature-verification keys exhibit the same basic problem class: canonical parsing first, stronger semantic rejection later.

### Why this matters

Again, I did not confirm a successful consensus acceptance bug. The risk is that intermediate APIs can hold semantically invalid curve values longer than callers may expect.

### Impact

I rate this `Low` because the confirmed issue is delayed rejection rather than wrong-final-verification.

## No confirmed broader subgroup-check failure

I did not confirm:

- an Orchard analogue to the Sapling `epk` small-order issue that reaches stronger impact;
- a wrong-basepoint signature confusion bug;
- or a case where these delayed-validation values actually pass final consensus verification.
