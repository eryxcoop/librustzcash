# Audit Findings

Audit target: Sapling/Orchard note trial decryption and wallet scanning logic, with a focus on malformed outputs, wrong OVK/IVK, crafted ciphertexts, note attribution, value handling, and spendability.

High-level result: I did not confirm a clean wrong-IVK or wrong-OVK false-positive path that causes a note to be attributed to the wrong wallet account or receiver. The note-encryption code performs strong note-validity checks tying decrypted plaintext back to the published commitment and ephemeral key. I did find two wallet-facing issues: one concrete value-handling bug that can panic on decryptable-but-wallet-invalid note values, and one light-client scanning trust gap that can make outputs look spendable without local proof/signature validation.

## Medium: decryptable shielded outputs with note values outside the wallet’s `Zatoshis` range can panic instead of being rejected cleanly

Affected code:

- `zcash_client_backend/src/decrypt.rs:89-100`
- `zcash_client_backend/src/data_api/wallet.rs:205-229`
- `zcash_client_backend/src/data_api/ll/wallet.rs:663-750`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/value.rs:52-76`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.1/src/value.rs:100-122`

### What happens

Sapling and Orchard note plaintext parsing both admit any `u64` note value locally:

- Sapling `NoteValue::from_raw` / `from_bytes`
- Orchard `NoteValue::from_raw` / `from_bytes`

Later, wallet-side decrypted-output helpers convert that raw value into `Zatoshis` with:

- `Zatoshis::from_u64(...).expect(...)`

So if a transaction is decryptable but carries a shielded note value outside the wallet’s valid `Zatoshis` range, the wallet-side path panics instead of returning an error.

### Why this matters

This is not a wrong-IVK false positive. The note is still required to decrypt successfully. The issue is that once decryption succeeds, the wallet assumes the note value has already been filtered by stronger validity checks and uses `expect`.

That makes malformed or invalid-but-decryptable shielded data a denial-of-service risk at the wallet boundary.

### Impact

I rate this `Medium` because:

- the panic path is concrete
- it sits on a high-level wallet API (`decrypt_and_store_transaction`)
- the API naturally looks like something callers could apply to untrusted transaction data

I did not confirm silent acceptance of an incorrect value; the confirmed behavior is crash-on-use rather than wrong persisted balance.

## Low: compact-output scanning can treat decrypted notes as spendable without any local proof/signature validity check

Affected code:

- `zcash_client_backend/src/scanning/compact.rs:128-149`
- `zcash_client_backend/src/scanning/compact.rs:282-316`
- `zcash_client_backend/src/scanning.rs:279-316`
- `zcash_client_sqlite/src/wallet/common.rs:554-693`

### What happens

The compact scanning path:

- parses compact Sapling/Orchard outputs/actions
- trial-decrypts them against wallet IVKs
- assigns note commitment tree positions
- can later treat them as spendable once scan-state and confirmation rules are satisfied

This path does not locally verify:

- Sapling Groth proofs
- Orchard proofs
- spend authorization signatures
- binding signatures
- full transaction-level consensus validity

So a malicious or compromised compact-data source can create notes that pass wallet decryption and later look spendable to the wallet even though the wallet itself has not checked that the underlying transaction would pass consensus validation.

### Why this matters

This is a misleading-spendability issue rather than a direct decryption false positive:

- the wallet has genuinely decrypted something consistent with the published compact data
- but local spendability is inferred without local consensus validation

This is part of the light-client trust model, so I rate it `Low`, but it is still a real gap between:

- "decrypted and scan-positioned"
- "validated against consensus rules"

## No confirmed wrong IVK / wrong OVK false positives

I specifically looked for cases where:

- a wrong IVK decrypts a crafted ciphertext and attributes it to the wrong account
- a wrong OVK recovers an output as outgoing for the wrong sender
- malformed ciphertext produces a note with a mismatched recipient or value that still passes note validation

I did not confirm such a path.

The main reasons are:

- `zcash_note_encryption` rechecks decrypted notes against the published commitment bytes
- for post-ZIP-212 notes it also checks that the derived ephemeral public key matches the published `ephemeral_key`
- sender recovery checks consistency of `esk` against the recovered note
- batch decryption only returns outputs that pass the same inner validity checks

Relevant note-validity code reviewed:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_note_encryption-0.4.1/src/lib.rs:515-556`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_note_encryption-0.4.1/src/lib.rs:666-694`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_note_encryption-0.4.1/src/batch.rs:43-81`

## Residual risk / caution

I did not build a custom fuzz harness in this turn. This was a code audit plus targeted reasoning over the decryption and scanning pipelines. The strongest confirmed outcomes were:

- crash-on-value-conversion after successful decryption
- misleading spendability in the compact light-client trust model
