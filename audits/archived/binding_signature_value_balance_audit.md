# Audit Findings

Audit target: Sapling and Orchard value-balance computation and binding signature verification.

Focus:

- sign mistakes
- omitted pools
- duplicated values
- negative / overflowing balances
- builder paths where computed value balance could differ from what the binding signature authenticates

High-level result: I did not confirm a value-balance or binding-signature soundness bug in the audited Sapling, Orchard, or transaction-builder paths. The code consistently:

- computes pool-local value balances by adding spends and subtracting outputs;
- range-checks those balances before converting to the transaction-level signed integer form;
- derives the binding signing key from the sum of trapdoors;
- independently derives the binding validating key from the committed values plus the claimed value balance;
- and rejects or panics on mismatch before producing an authorized bundle.

## No confirmed findings

### Sapling: builder and verifier use the same sign convention and cross-check trapdoor-sum vs commitment-sum

Sapling computes the bundle-local value balance as:

- `input_total = sum(note spends)`
- `value_balance = input_total - sum(note outputs)`

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/builder.rs:911-920`

The builder derives:

- `bsk` from `sum(rcv_spends) - sum(rcv_outputs)`
- `bvk` from `sum(cv_spends) - sum(cv_outputs) - ValueGenerator * value_balance`

and checks they match:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/builder.rs:751-798`

Consensus verification accumulates the same sign convention:

- add each spend `cv`
- subtract each output `cv`
- derive `bvk` from the supplied `value_balance`
- verify the binding signature against that key

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/verifier.rs:53-55`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/verifier.rs:138-147`

I did not find a sign inversion, duplicated contribution, or omitted contribution in these paths.

### Orchard: builder and binding-validating-key logic are consistent

Orchard computes the bundle-local value balance as the sum of per-action `value_sum()` values:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/builder.rs:844-849`

The Orchard builder derives:

- `bsk` from `sum(rcv_action)`
- `bvk` from `sum(cv_net_action) - ValueCommitment::derive(value_balance, 0)`

and asserts they match:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/builder.rs:725-747`

The public `binding_validating_key()` method uses the same formula:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/bundle.rs:407-420`

I did not find a path where the authenticated Orchard value balance differs from the builder’s own accounting.

### Range / overflow handling appears intentional and checked

Sapling:

- uses `ValueSum(i128)` for accumulation;
- checked-adds / checked-subtracts note values;
- errors on overflow;
- only later converts to `i64` for bundle `valueBalance`.

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/value/sums.rs:42-103`

Orchard:

- uses `ValueSum(i128)` constrained to `VALUE_SUM_RANGE`;
- checked-adds totals and rejects overflow;
- converts to `i64` only when constructing the bundle’s public `value_balance`.

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/value.rs:164-245`

I did not confirm a path where a negative or overflowing value balance is accepted and then authenticated as some different value.

### PCZT IO finalizers recheck internal consistency before creating binding signing keys

For Sapling PCZT:

- recompute `bsk` from stored `rcv`s
- recompute `bvk` from stored `cv`s and `value_sum`
- reject on mismatch

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt/io_finalizer.rs:17-58`

For Orchard PCZT:

- recompute `bsk` from stored `rcv`s
- recompute `bvk` from `sum(cv_net)` and `value_sum`
- reject on mismatch

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/io_finalizer.rs:22-39`

So I did not find a PCZT-specific path where mutable bundle metadata can silently desynchronize the value balance from the binding signature key material.

### Global transaction builder checks cross-pool balance before signatures

The top-level transaction builder sums:

- transparent value balance
- Sapling value balance
- Orchard value balance
- ZIP-233 amount where applicable
- TZE value balance where applicable

Relevant code:

- `zcash_primitives/src/transaction/builder.rs:678-703`

Then, before building an authorized transaction or a PCZT, it enforces:

- `global value_balance - fee == 0`

with explicit `InsufficientFunds` / `ChangeRequired` failure on mismatch:

- `zcash_primitives/src/transaction/builder.rs:1020-1033`
- `zcash_primitives/src/transaction/builder.rs:1210-1219`

This means the builder does not rely on the Sapling or Orchard binding signatures alone to enforce whole-transaction economic soundness; it checks cross-pool balance first.

I did not find an omitted-pool path in the standard builder logic.

## Residual notes

I did not classify the following as findings:

- Sapling and Orchard use different algebraic presentations of the binding key relation:
  - Sapling derives `bvk` from a commitment sum and a signed scalar multiple of the value generator.
  - Orchard derives `bvk` from a commitment sum minus `ValueCommitment::derive(value_balance, 0)`.

These are expected protocol differences, and I did not find an inconsistency between their builders and verifiers.

- The unstable / extensible transaction builder also tracks ZIP-233 and TZE balances in the global balance check. I did not find evidence that those paths desynchronize what the shielded binding signatures authenticate versus what the builder accepts.

## Conclusion

I did not confirm a binding-signature / value-balance soundness bug in the audited Sapling, Orchard, or transaction-builder code. The strongest assurance points are:

- checked range handling for value sums,
- independent `bsk` vs `bvk` consistency checks in builders and PCZT IO finalizers,
- matching verifier formulas,
- and the top-level builder’s explicit cross-pool zero-balance-after-fee requirement.
