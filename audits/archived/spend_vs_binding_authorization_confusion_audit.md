# Audit Findings

Audit target: Sapling and Orchard spend authorization signatures versus binding signatures, with a focus on shared RedDSA machinery, basepoint selection, domain separation, and verification-key wrapper confusion.

High-level result: I did not confirm a case where a signature or verification key intended for `SpendAuth` can be accepted as `Binding`, or vice versa, in the audited Sapling or Orchard paths.

The main reasons are:

- the underlying `reddsa` library models `SpendAuth` and `Binding` as distinct `SigType`s rather than as an untyped “mode” flag;
- both Sapling and Orchard use distinct role-specific basepoints even where the hash personalization string is shared within a curve family;
- verifier sites consume role-typed keys and signatures, and binding verification keys are rederived from transaction value commitments instead of being caller-supplied.

Affected code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/reddsa-0.5.1/src/lib.rs:39-108`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/reddsa-0.5.1/src/orchard.rs:15-78`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/reddsa-0.5.1/src/batch.rs:53-127`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/reddsa-0.5.1/tests/batch.rs:8-98`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/builder.rs:1100-1186`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/verifier.rs:30-144`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/builder.rs:931-1104`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/bundle.rs:407-459`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/bundle/batch.rs:31-81`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/primitives/redpallas.rs:129-155`

## No confirmed finding: shared RedDSA machinery does not create SpendAuth/Binding confusion

### What I checked

I traced:

- the generic `reddsa` role model;
- Sapling and Orchard role-specific basepoint definitions;
- builder-side signature creation for spend auth and binding auth;
- verifier-side signature checking for both roles;
- batch verification code paths where mixed signature kinds are queued together;
- verification-key wrapper boundaries, especially whether a caller-supplied `rk` could be reused as a binding validating key.

### What I found

At the `reddsa` layer, `SpendAuth` and `Binding` are distinct traits over a generic `SigType`, and the internal `Sealed` implementation assigns each role its own generator/basepoint:

- Sapling uses separate `SPENDAUTHSIG_BASEPOINT_BYTES` and `BINDINGSIG_BASEPOINT_BYTES`.
- Orchard uses separate `ORCHARD_SPENDAUTHSIG_BASEPOINT_BYTES` and `ORCHARD_BINDINGSIG_BASEPOINT_BYTES`.

Within a curve family, the hash personalization is shared:

- Sapling uses `Zcash_RedJubjubH` for both roles.
- Orchard uses `Zcash_RedPallasH` for both roles.

But this is not enough to cause role confusion because the signature equation also depends on the role-specific generator. Reusing the same transcript hash with different basepoints still gives distinct verification equations.

The library APIs preserve that distinction:

- Sapling spend signatures are `redjubjub::Signature<SpendAuth>`, while binding signatures are `redjubjub::Signature<Binding>`.
- Orchard spend signatures are `redpallas::Signature<SpendAuth>`, while binding signatures are `redpallas::Signature<Binding>`.
- Orchard batch items are separately constructed from `VerificationKey<SpendAuth>` and `VerificationKey<Binding>`.

The verifier sites also keep the roles apart:

- Sapling `check_spend` verifies spend auth against a caller-visible `rk: VerificationKey<SpendAuth>`.
- Sapling `final_check` verifies the binding signature only against a fresh `bvk` derived from `cv_sum` and `value_balance`.
- Orchard per-action verification uses `action.rk()` for spend auth.
- Orchard binding verification uses `bundle.binding_validating_key()`, which is recomputed from the action value commitments and bundle `value_balance`.

So the most dangerous class of bug here, “attacker substitutes a spend-auth key/signature where a binding signature is expected,” is closed off by both type separation and verifier-side rederivation of the binding key.

### Why this is not a finding

I did not confirm any path where:

- a `SpendAuth` signature verifies as a `Binding` signature;
- a `Binding` signature verifies as a `SpendAuth` signature;
- an `rk` wrapper can be reused as the bundle binding validating key;
- a caller can override the derived binding verification key with externally supplied bytes;
- batch verification erases the role distinction and accepts mixed items under the wrong equation.

The checked-in `reddsa` batch tests also explicitly exercise mixed `SpendAuth` and `Binding` batches, which supports the intended role separation rather than contradicting it.

### Residual note

This audit does not change the earlier low-severity delayed-validation observations for plain RedJubjub / RedPallas verification-key parsing. Those are real ergonomic issues, but they are not cross-role authorization confusion and remain covered by `audits/redjubjub_redpallas_vk_validation_audit.md`.
