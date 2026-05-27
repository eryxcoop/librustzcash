# Audit Findings

Audit target: Sapling and Orchard memo encryption/decryption and wallet storage, with a focus on whether memos are only accepted when bound to the same note, recipient, `epk`, ciphertext, and output index.

High-level result: I did not confirm a cryptographic memo-binding bug in the core Sapling / Orchard note-encryption paths. Recipient decryption and sender recovery both extract the memo only after decrypting the same note plaintext and rechecking note validity against the published commitment and `epk`. The confirmed issues are at the wallet storage boundary:

- a `Medium` issue where low-level storage APIs accept caller-supplied `memo` and `output_index` / `action_index` without cryptographic rebinding to the ciphertext-bearing output;
- a `Low` issue where SQLite upsert semantics make memo associations sticky, allowing stale memo reuse if a row is ever initially populated with wrong memo data.

## Medium: wallet storage accepts memo and output index as independent caller-controlled metadata

Affected code:

- `zcash_client_backend/src/decrypt.rs:152-180`
- `zcash_client_backend/src/decrypt.rs:205-235`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_note_encryption-0.4.1/src/lib.rs:503-555`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_note_encryption-0.4.1/src/lib.rs:697-712`
- `zcash_client_sqlite/src/wallet/sapling.rs:349-379`
- `zcash_client_sqlite/src/wallet/orchard.rs:313-336`
- `zcash_client_sqlite/src/wallet.rs:2785-2816`
- `zcash_client_sqlite/src/wallet.rs:2905-2920`

### What happens

In the normal decrypt/recovery paths, the memo is correctly bound to the same decrypted note:

- the wallet enumerates actual transaction outputs by index,
- decrypts the note plaintext from that output,
- extracts the memo from the same plaintext bytes,
- and only accepts the result after `check_note_validity(...)` rebinds the note to the published commitment and `epk`.

Relevant code:

- `zcash_client_backend/src/decrypt.rs:152-180`
- `zcash_client_backend/src/decrypt.rs:205-235`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_note_encryption-0.4.1/src/lib.rs:503-555`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_note_encryption-0.4.1/src/lib.rs:697-712`

That is the good path.

The problem is at the low-level wallet storage boundary. The SQLite received-note ingestion paths persist:

- `transaction_id`
- `output_index` / `action_index`
- `memo`

directly from the caller-supplied `Received*Output` object:

- `zcash_client_sqlite/src/wallet/sapling.rs:349-379`
- `zcash_client_sqlite/src/wallet/orchard.rs:313-336`

Later, memo lookup APIs trust only `(txid, pool, output_index)` to retrieve the memo:

- received memo:
  - `zcash_client_sqlite/src/wallet.rs:2785-2816`
- sent memo:
  - `zcash_client_sqlite/src/wallet.rs:2905-2920`

There is no storage-layer recomputation that proves:

- this memo came from the same ciphertext as the stored note,
- or that the supplied output index is the one for that ciphertext-bearing output.

### Why this matters

This means the cryptographic memo binding established by decryption is not re-established at the persistence boundary.

A buggy or malicious caller using the low-level wallet write API can provide:

- a real note object from one output,
- a memo from another output,
- and an index selecting whichever output row they want the memo attached to,

and the storage layer will accept that tuple as authoritative.

I did not confirm that the built-in decrypt / scan paths do this incorrectly. The issue is that the storage contract is weaker than the cryptographic boundary it appears to sit behind.

### Impact

I rate this `Medium` because:

- it can cause the wallet to persist a memo for the wrong output index;
- later `get_received_memo` / `get_sent_memo` calls will surface that memo as if it belonged to that output;
- and memo contents are user-visible, semantically significant data that may carry invoices, references, or application-layer instructions.

This is a wallet-state integrity issue, not a consensus or fund-redirection issue.

## Low: SQLite upsert semantics allow stale memo reuse once a memo has been associated with an output row

Affected code:

- `zcash_client_sqlite/src/wallet/sapling.rs:370-379`
- `zcash_client_sqlite/src/wallet/orchard.rs:325-335`
- `zcash_client_sqlite/src/wallet.rs:4716-4721`

### What happens

For both received notes and sent outputs, SQLite uses `IFNULL(:memo, memo)` on conflict:

- Sapling received notes:
  - `zcash_client_sqlite/src/wallet/sapling.rs:370-379`
- Orchard received notes:
  - `zcash_client_sqlite/src/wallet/orchard.rs:325-335`
- sent outputs:
  - `zcash_client_sqlite/src/wallet.rs:4716-4721`

This means:

- if a row already has a memo,
- and a later reprocessing path for the same `(txid, pool, output_index)` has `memo = NULL`,
- the existing memo is preserved.

That behavior is helpful for legitimate cases such as:

- memo learned from full-transaction decryption,
- later block-scan update that lacks memo bytes.

But it also means that if a memo is ever initially populated incorrectly, later processing will not clear it automatically.

### Why this matters

This creates a stale-memo-reuse hazard:

- a bad initial memo association can become sticky;
- later processing that no longer has memo material, or does not revalidate the original memo association, will preserve the stale value.

I did not confirm a built-in path that injects a wrong memo first and a `NULL` memo second under normal operation. The issue is that once the boundary above is crossed incorrectly, the database schema and upsert logic do not self-heal.

### Impact

I rate this `Low` because:

- it depends on an initial bad memo association;
- but after that, later storage updates will preserve the stale memo rather than correcting or clearing it.

## No confirmed cryptographic memo / ciphertext binding issue

I specifically looked for cases where a memo plaintext could be accepted:

- from the wrong note,
- under the wrong recipient,
- with the wrong `epk`,
- or from the wrong ciphertext blob.

I did not confirm such a bug in the note-encryption layer.

The core paths:

- decrypt the note plaintext from the output’s `enc_ciphertext`,
- parse the note and recipient from that same plaintext,
- extract the memo from that same plaintext,
- and only then accept the result if the note matches the published commitment and `epk`.

So the strongest issues here are in how memos are persisted and keyed by output index after decryption, not in the cryptographic decryption itself.
