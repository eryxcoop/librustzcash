# Audit Findings

Audit target: Sapling and Orchard batch verification APIs, with a focus on whether invalid proofs/signatures can be hidden by batching, whether failures are attributable to the correct transaction or output, and whether callers treat partial batch results as success.

High-level result: I did not confirm a case where an invalid Sapling or Orchard proof/signature is accepted as valid because of batching, and I did not find an in-repo caller that treats partial batch work as a successful verification result.

I did confirm one API-level isolation issue in Sapling:

- `Low`: `sapling::BatchValidator::check_bundle` can enqueue part of an invalid bundle into the batch before returning `false`, so reusing that validator after a failed pre-check can poison later batch validation and break failure attribution.

Affected code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/verifier/batch.rs:35-74`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/verifier/batch.rs:118-163`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/bundle/batch.rs:40-91`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/redjubjub-0.8.0/src/batch.rs:54-67`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/reddsa-0.5.1/src/batch.rs:102-127`
- `pczt/src/roles/tx_extractor/sapling.rs:14-24`
- `pczt/src/roles/tx_extractor/orchard.rs:9-27`

## Low: Sapling `check_bundle` can partially enqueue an invalid bundle before reporting failure

### What happens

Sapling’s batch API splits verification into two phases:

- `check_bundle(...)` performs structural / consensus-side prechecks and queues proofs/signatures into the batch.
- `validate(...)` later batch-verifies everything that has been accumulated.

The important detail is that `check_bundle(...)` is not atomic. Its own doc comment says:

- the validator “can continue to be used regardless” after failure,
- but “some or all of the proofs and signatures from this bundle may have already been added to the batch even if it fails other consensus rules.”

That is exactly what the implementation does:

- spend auth signatures are queued during each successful `check_spend(...)`;
- spend proofs are queued at the same time;
- output proofs are queued during each successful `check_output(...)`;
- the method can still later return `false` on a later spend/output parse failure or later consensus-rule failure.

So an invalid bundle can leave partial verification items behind inside the `BatchValidator`.

### Why this matters

If a caller batches multiple untrusted bundles in one validator and follows the API documentation literally by reusing the validator after `check_bundle(...) == false`, the invalid bundle can contaminate the batch state for later bundles.

The next `validate(...)` result then no longer means:

- “one of the later queued bundles is invalid”

but instead:

- “something in the whole accumulated state is invalid, possibly including leftovers from a bundle that was already known-bad.”

That creates two concrete problems:

- failure attribution is lost even more strongly than the normal “batching can’t pinpoint the bad item” limitation;
- one malicious invalid bundle can cause later-valid bundles in the same reused batch context to be rejected together.

I did not find an in-repo caller that makes this mistake. The local Sapling PCZT extractor creates a fresh validator, aborts immediately on `check_bundle(...) == false`, and only calls `validate(...)` on a clean one-bundle batch:

- `pczt/src/roles/tx_extractor/sapling.rs:14-24`

So the confirmed issue is an API-level hazard for external callers, not a currently confirmed in-repo misuse.

### Impact

I rate this `Low` because:

- I did not confirm acceptance of an invalid proof or signature;
- I did not confirm any current in-repo caller misuse;
- but a public batch API explicitly allows a reuse pattern that can poison later validation and misattribute failures across bundles.

## No confirmed hidden-invalid or partial-success issue in audited callers

I specifically looked for:

- an invalid Sapling or Orchard proof/signature becoming accepted because it was batched with valid ones;
- a caller treating “queued successfully” as equivalent to “verified successfully”;
- a caller using partial batch outcomes as a success signal.

I did not confirm any of those.

The audited caller behavior is fail-closed:

- Sapling PCZT extraction requires both `check_bundle(...)` and `validate(...)` to succeed.
- Orchard PCZT extraction always runs `validate(...)` on the accumulated bundle before success.
- Orchard’s batch API has the usual “no per-bundle pinpointing” limitation, but I did not find a comparable partial-enqueue-after-failure path because it does not have a separate `check_bundle(...) -> bool` staging step.

At the signature-layer below them, both RedJubjub and RedPallas batch APIs document that batching loses easy pinpointing of the failing item, and they expose `verify_single()` on queued items for explicit fallback isolation. The higher-level Sapling/Orchard wrappers simply do not automate that isolation.
