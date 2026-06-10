# Audit Findings

Audit target: pre-ZIP-212 and post-ZIP-212 Sapling note handling, focusing on `rseed` / `rcm` semantics, `esk` derivation, note plaintext parsing, and encryption/decryption acceptance under the correct version rules.

High-level result: I did not find a clean consensus bug in the core Sapling note-encryption implementation when the caller supplies the correct contextual height. The main confirmed issues are wallet/API-side version confusion:

- a `Medium` issue where `decrypt_transaction` can apply `Zip212Enforcement::Off` to present-day unmined transactions if no chain tip is available, causing acceptance of v1 note plaintexts that consensus would now reject;
- a `Low` issue where the SQLite wallet backend erases post-ZIP-212 Sapling note version semantics by reconstructing all stored notes as `Rseed::BeforeZip212(rcm)`.

## Medium: `decrypt_transaction` falls back to Sapling activation height and can accept consensus-invalid pre-ZIP-212 note plaintexts for unmined transactions

Affected code:

- `zcash_client_backend/src/decrypt.rs:117-145`
- `zcash_client_backend/src/data_api/wallet.rs:207-226`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/note_encryption.rs:71-105`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/note_encryption.rs:399-404`

### What happens

Sapling note plaintext parsing is explicitly contextual:

- under `Zip212Enforcement::Off`, only plaintext lead byte `0x01` is accepted;
- under `GracePeriod`, `0x01` and `0x02` are both accepted;
- under `On`, only `0x02` is accepted.

That is correct.

The problem is in `zcash_client_backend::decrypt_transaction`. For unmined transactions it computes a "mempool height" from `chain_tip_height + 1`, but if the caller supplies neither a mined height nor a chain tip, it falls back all the way to Sapling activation height:

- `mined_height.unwrap_or_else(|| chain_tip_height.map(|h| h + 1).or_else(|| params.activation_height(NetworkUpgrade::Sapling)) ...)`

On main networks, that fallback implies `Zip212Enforcement::Off`, not the current post-Canopy rule set.

As a result, the high-level decryption API can be invoked on a present-day unmined transaction and evaluate its Sapling outputs using pre-ZIP-212 parsing rules, accepting version-1 note plaintexts that would not be valid if mined under current consensus.

`decrypt_and_store_transaction` inherits this behavior when:

- the caller passes `mined_height = None`,
- the wallet has no stored height for the tx,
- and `data.chain_height()` is also `None`.

### Why this matters

This is a genuine version-confusion acceptance gap:

- the note plaintext parser itself is correct;
- the contextual enforcement passed into it can be too old;
- and the resulting `DecryptedTransaction` / wallet state can reflect Sapling outputs from a transaction whose note plaintext version would be consensus-invalid today.

This does not let an attacker forge a valid on-chain transaction. It does let a safe, high-level wallet API accept and potentially persist Sapling notes under the wrong version semantics when chain context is absent.

### Impact

I rate this `Medium` because:

- it is reachable through safe public wallet/decryption APIs;
- it can cause acceptance and storage of Sapling outputs that current consensus would reject;
- and it is a concrete instance of wallet-side semantic validation drifting from consensus due to wrong ZIP-212 context.

## Low: SQLite spendable-note reconstruction erases post-ZIP-212 Sapling note semantics and rehydrates all notes as `BeforeZip212(rcm)`

Affected code:

- `zcash_client_sqlite/src/wallet/sapling.rs:54-66`
- `zcash_client_sqlite/src/wallet/sapling.rs:105-123`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/note.rs:129-152`

### What happens

The SQLite backend stores only `rcm` for Sapling notes. When it reconstructs a note for spend selection, it does this unconditionally:

- parse stored `rcm`;
- construct `Rseed::BeforeZip212(rcm)`;
- rebuild the note with `sapling::Note::from_parts(...)`.

The file comment is explicit:

- "We store `rcm` directly in the data DB, regardless of whether the note used a v1 or v2 note plaintext, so for the purposes of spending let's pretend this is a pre-ZIP-212 note."

This means a note originally decrypted from a valid version-2 post-ZIP-212 plaintext is later surfaced by a safe wallet API as a `sapling::Note` whose internal randomness variant is now `BeforeZip212`.

That semantic change is observable downstream because Sapling note behavior depends on the `Rseed` variant:

- `derive_esk()` returns `None` for `BeforeZip212`;
- `generate_or_derive_esk()` will synthesize random `esk` for `BeforeZip212`, but deterministically derive it for `AfterZip212`.

### Why this matters

For ordinary spend construction, this usually does not break consensus because Sapling spends only need the note commitment randomness `rcm`, not the original note plaintext version. So I am not claiming a spend failure or theft bug here.

But it is still a real version-confusion issue:

- the wallet accepted a post-ZIP-212 note;
- later safe APIs no longer preserve that note's post-ZIP-212 semantics;
- and any downstream code that assumes a retrieved Sapling note still preserves `rseed`/`esk` semantics can get the wrong behavior.

This is especially relevant because other code in the workspace does make explicit post-ZIP-212 assumptions, for example:

- sent-output recovery in `zcash_client_backend/src/data_api/wallet.rs:2402-2404` expects `derive_esk(&note)` to succeed for notes it handles there;
- Sapling PCZT output handling similarly assumes post-ZIP-212 output notes and rejects pre-ZIP-212 output semantics.

### Impact

I rate this `Low` because:

- I did not confirm a consensus break or a direct misuse in current spend construction;
- but the wallet does return notes under the wrong semantic version, which is exactly the kind of confusion this audit was looking for.

## No confirmed finding in the core ZIP-212-aware decrypt / recovery logic when the caller provides correct height context

I specifically checked for cases where Sapling note parsing or decryption would accept:

- a v1 plaintext when `Zip212Enforcement::On` should require v2,
- a v2 plaintext when `Zip212Enforcement::Off` should require v1,
- a post-ZIP-212 note with inconsistent derived `esk` / published `epk`,
- or sender-recovery under the wrong randomness semantics.

I did not confirm such a bug in the core implementation.

Relevant checked behavior:

- `sapling_parse_note_plaintext_without_memo` gates the lead byte through `plaintext_version_is_valid(...)`.
- Recipient trial decryption rejects notes whose derived note commitment does not match the published `cmu`.
- For post-ZIP-212 notes, recipient and sender paths additionally enforce the `esk` / `epk` relationship.
- Modern Sapling PCZT output construction explicitly forbids pre-ZIP-212 outputs.

So the strongest issues here are not in the cryptographic decryption core, but in wallet-layer context selection and long-term note representation.
