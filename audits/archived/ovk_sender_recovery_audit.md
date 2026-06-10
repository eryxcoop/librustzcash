# Audit Findings

Audit target: Sapling and Orchard sender-side output recovery using OVKs, with a focus on whether recovered note/plaintext data is fully cross-checked against the output commitment and ephemeral key before being used to attribute recipient, value, memo, or account.

High-level result: I did not confirm a cryptographic sender-recovery bug in the core Sapling or Orchard note-encryption implementations. The generic recovery path derives `ock` from `ovk + cv + commitment + epk`, decrypts the outgoing plaintext to `pk_d` and `esk`, and then rechecks note commitment validity and the `esk`/`epk` relationship before accepting the recovered note. The main confirmed issue is higher-layer: sent-transaction output extraction from PCZT uses mutable metadata to attribute recipient/value/account and uses sender recovery only for the memo, without first rebinding the reconstructed note fields to the tx-bound `cmu` / `cmx`.

## Medium: PCZT sent-output extraction can attribute wrong recipient, wrong value, or wrong account even when sender-recovery memo validation fails

Affected code:

- `zcash_client_backend/src/data_api/wallet.rs:2245-2320`
- `zcash_client_backend/src/data_api/wallet.rs:2401-2435`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt/tx_extractor.rs:69-90`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/tx_extractor.rs:69-80`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt/verify.rs:146-157`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/verify.rs:128-143`

### What happens

When the wallet reconstructs `SentTransactionOutput`s from a finalized PCZT, it does the following for shielded outputs:

- rebuilds a Sapling or Orchard note from optional PCZT metadata fields such as:
  - recipient
  - value
  - `rseed`
  - and for Orchard, `rho` derived from the spend nullifier
- reconstructs recipient/account display metadata from:
  - `user_address`
  - proprietary `PcztRecipient` metadata
- then calls `try_output_recovery_with_pkd_esk(...)` only to recover the memo

Crucially:

- the displayed `recipient`
- the displayed `value`
- and the displayed `account`

come from reconstructed metadata before any `verify_note_commitment()`-style recheck is applied.

At the same time, the Sapling and Orchard PCZT transaction extractors ignore those optional note metadata fields when producing the final transaction effects:

- Sapling extraction uses only `cv`, `cmu`, `ephemeral_key`, `enc_ciphertext`, `out_ciphertext`, and proof/signature fields.
- Orchard extraction uses only `nullifier`, `rk`, `cmx`, encrypted note bytes, `cv_net`, and authorization/proof fields.

So a participant who can mutate the PCZT after those metadata fields are no longer needed for proof/signature generation can change displayed recipient/value/account metadata without changing the extracted signed transaction.

The libraries even provide explicit `verify_note_commitment()` helpers for these optional PCZT note fields, but the sent-output extraction path does not call them.

### Why this matters

This is a real sender-recovery attribution gap:

- memo recovery is cryptographically checked against the actual output;
- but recipient/value/account attribution is not rebound to the actual output before being surfaced to the wallet.

As a result, sent transaction history can reflect:

- the wrong recipient address,
- the wrong output value,
- or the wrong internal receiving account classification

even if memo recovery correctly fails because the reconstructed note metadata does not actually match the tx-bound `cmu` / `cmx` and `epk`.

This does not redirect funds on chain. The underlying transaction remains the same. The issue is integrity of sender-visible wallet history and attribution.

### Impact

I rate this `Medium` because:

- the final transaction is unaffected, so this is not a consensus or theft issue;
- but a safe wallet API can display materially wrong sent-output information from mutable PCZT-side metadata;
- and the code already has the primitives required to prevent this, but does not use them at this boundary.

## No confirmed finding in the core Sapling and Orchard OVK recovery cryptography

I specifically looked for a case where OVK recovery would accept:

- the wrong recipient,
- the wrong value,
- the wrong memo,
- or the wrong note/account

because `pk_d`, `esk`, note plaintext, commitment, or `epk` were insufficiently cross-checked.

I did not confirm such a bug in the cryptographic recovery path.

### Shared recovery checks

The generic `zcash_note_encryption` recovery flow does all of the right high-level binding:

- derives `ock` from `ovk`, `cv`, output commitment bytes, and `epk`;
- decrypts outgoing plaintext to obtain `pk_d` and `esk`;
- decrypts the note plaintext using that key material;
- rechecks derived `esk` against the note where the domain supports deterministic `esk`;
- rechecks the note commitment and `epk` via `check_note_validity(...)`.

Relevant code reviewed:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_note_encryption-0.4.1/src/lib.rs:531-555`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_note_encryption-0.4.1/src/lib.rs:613-709`

### Sapling

Sapling additionally has extensive negative tests showing OVK recovery rejects mutated:

- `ovk`
- `cv`
- `cmu`
- `epk`
- note plaintext version
- diversifier
- `pk_d`
- ciphertext authentication tags

Relevant test coverage reviewed:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/note_encryption.rs:1035-1318`

### Orchard

Orchard binds recovery to the action's `rho` through the domain:

- note plaintext parsing uses `RandomSeed::from_bytes(..., &domain.rho)`;
- `OrchardDomain::for_action` gets `rho` from the action itself;
- the generic recovery layer then rechecks commitment and `esk`/`epk`.

Relevant code reviewed:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/note_encryption.rs:52-73`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/note_encryption.rs:98-107`

## Conclusion

The core OVK recovery implementations for Sapling and Orchard appear correctly bound. The confirmed problem is one layer up: wallet sent-output extraction from PCZT does not use those bindings to validate the recipient/value/account metadata it surfaces, and can therefore report the wrong sent-output semantics for an otherwise unchanged transaction.
