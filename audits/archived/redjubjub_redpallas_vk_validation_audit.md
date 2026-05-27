# Audit Findings

Audit target: RedJubjub and RedPallas verification-key parsing and signature verification paths, with a focus on identity, small-order, non-canonical, and wrong-domain acceptance.

High-level result: I did not confirm a case where an invalid RedJubjub / RedPallas verification key or signature can reach successful consensus verification. The strongest confirmed issues are delayed-validation mismatches:

- Sapling transaction / PCZT parsing accepts some `rk` values that are only rejected later by the Sapling verifier.
- Orchard’s plain RedPallas verification-key type accepts the identity encoding, and Orchard PCZT parsing preserves that value until later rejection in proving / extraction.

I did not confirm a stronger “wrong-basepoint” or cross-domain signature confusion bug.

## Low: Sapling parsing accepts `rk` values that are only rejected later by consensus verification

Affected code:

- `zcash_primitives/src/transaction/components/sapling.rs:142-150`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/verifier.rs:45-50`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/keys.rs:193-210`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt/parse.rs:71-80`

### What happens

Sapling transaction deserialization parses `rk` using plain `redjubjub::VerificationKey::try_from(bytes)`:

- `zcash_primitives/src/transaction/components/sapling.rs:142-150`

The inline comment is explicit that:

- canonical encoding is enforced there,
- but “not small order” is enforced later in `SaplingVerificationContext::check_spend()`.

Relevant verifier code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/verifier.rs:45-50`

The same pattern appears in Sapling PCZT spend parsing:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt/parse.rs:71-80`

This is notably weaker than Sapling’s own `SpendValidatingKey::from_bytes`, which explicitly narrows the accepted set to non-identity prime-order subgroup points:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/keys.rs:193-210`

### Why this matters

This is a classic delayed-semantic-validation issue:

- the ergonomic parsing API accepts an `rk` into transaction / PCZT state,
- later consensus verification rejects it if it is small-order.

I did not confirm a downstream use that turns this into theft or consensus bypass. The impact is that callers can hold or pass around Sapling transaction objects that look structurally parsed but are semantically unfit for consensus until a later stage.

### Impact

I rate this `Low` because:

- rejection still happens before successful consensus verification;
- but safe-looking parse paths expose a wider accepted set than the actual valid domain.

## Low: Orchard plain RedPallas verification keys accept the identity encoding, and PCZT can carry identity `rk` until prover/extractor rejection

Affected code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/primitives/redpallas.rs:81-84`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/action.rs:33-54`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/action.rs:232-236`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/parse.rs:100-122`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/prover.rs:88-100`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/tx_extractor.rs:72-84`

### What happens

The plain RedPallas verification-key type accepts whatever `reddsa::VerificationKey` accepts:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/primitives/redpallas.rs:81-84`

Orchard’s own action layer then adds the missing semantic restriction:

- `Action::from_parts` rejects identity `rk`
- the test code explicitly documents that the canonical identity encoding `[0u8; 32]` is accepted by plain redpallas

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/action.rs:33-54`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/action.rs:232-236`

Normal Orchard transaction parsing is fail-closed because it immediately wraps the parsed `rk` with `Action::from_parts(...)`, which rejects identity:

- `zcash_primitives/src/transaction/components/orchard.rs:123-127`
- `zcash_primitives/src/transaction/components/orchard.rs:153-165`

But Orchard PCZT parsing does not do that extra semantic check. It stores the plain parsed `rk` directly in `Spend`:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/parse.rs:100-122`

Identity `rk` is then only rejected later:

- during proof creation through `Instance::from_parts(...).ok_or(ProverError::IdentityRk)`
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/prover.rs:88-100`
- during tx extraction through `Action::from_parts(...).ok_or(TxExtractorError::IdentityRk)`
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/tx_extractor.rs:72-84`

### Why this matters

This is a real ergonomic delayed-rejection gap:

- “plain” RedPallas VK parsing succeeds,
- Orchard PCZT can carry identity `rk` as parsed state,
- and only later proving / extraction refuses it.

I did not confirm a successful inconsistent use beyond that delayed failure, and I did not confirm the same pattern in normal Orchard transaction parsing because `Action::from_parts` closes the gap there.

### Impact

I rate this `Low` because:

- final consensus-valid transaction creation still rejects identity `rk`;
- but Orchard PCZT parsing exposes an invalid intermediate state that later roles must remember to reject.

## No confirmed wrong-basepoint or signature-domain confusion

I specifically looked for cases where:

- a RedJubjub signature could be used as RedPallas or vice versa,
- a `Binding` signature could be accepted as `SpendAuth` or vice versa,
- or a verification key from the wrong basepoint/domain could pass a higher-level verifier.

I did not confirm such a bug.

The main reasons are:

- RedJubjub and RedPallas are distinct types at the API level.
- `SpendAuth` and `Binding` are tracked as separate type parameters.
- Signature verification sites use the appropriately typed keys/signatures.

So the reportable issue here is delayed validation, not cross-domain acceptance.
