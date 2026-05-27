# Audit Findings

Audit target: Sapling and Orchard nullifier derivation paths, with a focus on whether account key, note position, `rho`, `nk`/`nivk`, and note commitment are bound exactly as specified, and whether any caller-supplied field can override derived nullifier data.

High-level result: I did not find a consensus-level nullifier-binding failure in the canonical scan, builder, prover, or PCZT-signing paths. Sapling and Orchard nullifiers are rederived from the expected inputs on the main construction paths. The main confirmed gap is at the wallet persistence boundary: the low-level wallet ingestion/storage APIs accept caller-supplied nullifier and note-position metadata without rederiving or rebinding them to the stored note, which can corrupt wallet spentness and local spend construction state.

## Medium: low-level wallet ingestion trusts caller-supplied nullifier and note position instead of rederiving canonical nullifier binding

Affected code:

- `zcash_client_backend/src/scanning.rs:154-155`
- `zcash_client_backend/src/scanning.rs:195-200`
- `zcash_client_backend/src/scanning.rs:801-810`
- `zcash_client_backend/src/data_api/ll.rs:343-365`
- `zcash_client_backend/src/data_api/ll.rs:393-410`
- `zcash_client_backend/src/data_api/ll.rs:616-623`
- `zcash_client_sqlite/src/wallet/sapling.rs:336-400`
- `zcash_client_sqlite/src/wallet/orchard.rs:303-357`
- `zcash_client_sqlite/src/wallet/common.rs:224-245`
- `zcash_client_sqlite/src/wallet/common.rs:380-410`
- `zcash_client_backend/src/data_api/wallet.rs:1170-1187`
- `zcash_client_backend/src/data_api/wallet.rs:1214-1230`
- `zcash_client_sqlite/src/wallet.rs:3328-3339`
- `zcash_client_memory/src/types/memory_wallet/mod.rs:339-346`
- `zcash_client_memory/src/types/memory_wallet/mod.rs:429-434`

### What happens

The canonical scan path derives owned-note nullifiers from note data plus the expected binding inputs:

- Sapling scanning derives `nf` as `note.nf(nk, position)`.
- Orchard scanning derives `nf` as `note.nullifier(fvk)`.

Those are the right bindings:

- Sapling note nullifiers are derived from `nk`, the note commitment, and the note position.
- Orchard note nullifiers are derived from `nk`, `rho`, `psi`, and the note commitment.

However, once a shielded output crosses into the low-level wallet write interface, that canonical derivation is no longer enforced. The public-ish `ReceivedShieldedOutput` abstraction exposes:

- `note()`
- `nullifier()`
- `note_commitment_tree_position()`

as separately supplied fields. The SQLite and memory backends then persist and trust:

- the note internals from `output.note()`
- the optional nullifier from `output.nullifier()`
- the note position from `output.note_commitment_tree_position()`

without recomputing the nullifier from the note and wallet key material, and without checking that the stored note position is consistent with the note commitment tree leaf for that note.

Downstream logic then treats these fields as authoritative:

- spendable-note queries require `nf IS NOT NULL`
- spend detection matches revealed on-chain nullifiers against the stored `nf`
- locally created sent transactions mark notes spent by matching spend nullifiers against the stored `nf`
- spend construction reloads the witness using the stored note position

### Why this matters

This is a real nullifier-binding gap at the wallet state boundary.

A buggy or malicious caller using the low-level wallet write APIs can store:

- a valid Sapling or Orchard note object
- with a mismatched stored nullifier
- and/or a mismatched stored note position

without the wallet rederiving and rejecting the inconsistency.

That can produce wallet-state corruption such as:

- a real spend on chain not being matched to the stored note because the wallet tracks the wrong nullifier
- a note being spuriously marked spent if the stored nullifier collides with a revealed nullifier from another tracked note
- a locally created transaction failing to mark its own input note spent, because sent-transaction bookkeeping matches the transaction's actual spend nullifier against the stored one
- a stored wrong position causing witness lookup for the wrong tree leaf during later spend construction

The last case does not let an attacker redirect funds or create a valid spend with a forged nullifier binding. It is still meaningful because note position is one of the canonical inputs to Sapling nullifier derivation, and the wallet lets that input be overridden in persisted state instead of re-establishing it from trusted scan data.

### Impact

I rate this `Medium` because:

- the canonical on-chain nullifier derivations are sound
- I did not find a way to make consensus accept a spend with the wrong nullifier binding
- but the wallet's accepted persisted state can diverge from the canonical nullifier binding, and later wallet behavior trusts that divergence for spentness and spendability

This is best understood as a wallet-state integrity issue, not a consensus failure.

## No confirmed finding in the canonical Sapling and Orchard nullifier derivation paths

I specifically checked the places where nullifiers are actually derived or rechecked for transaction construction and signing.

### Sapling

- The note API derives the nullifier from `nk`, the note commitment, and the note position:
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/note.rs:112-113`
- The wallet scanning path uses exactly that derivation:
  - `zcash_client_backend/src/scanning.rs:154-155`
  - `zcash_client_backend/src/scanning.rs:801-810`
- PCZT signing rederives the nullifier from the reconstructed note, the validating key, and the witness position:
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt/verify.rs:67-91`

I did not confirm a path where a Sapling nullifier can be rebound to a different note, different `nk`, or different witness position and still survive proof/signature generation as a valid transaction.

### Orchard

- The note API derives the nullifier from `nk`, `rho`, `psi`, and the note commitment:
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/note.rs:283-290`
- The wallet scanning path uses `note.nullifier(fvk)` and does not accept an IVK-only shortcut for nullifier computation:
  - `zcash_client_backend/src/scanning.rs:195-200`
  - IVK-only `DecryptedOutput` paths expose `nullifier() == None`
- PCZT signing rederives the nullifier from reconstructed note fields and rejects mismatch:
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/verify.rs:65-90`

I likewise did not confirm a path where an Orchard nullifier can be rebound to different `rho`, `nk`, or commitment data and still remain valid on chain.

## Conclusion

The strongest confirmed issue is not in nullifier derivation itself, but in the wallet boundary where nullifier-related metadata becomes caller-controlled state. The canonical derivation logic in Sapling and Orchard is appropriately bound; the missing hardening is that wallet persistence accepts a precomputed `nf` and note position as if they were already trusted, instead of rederiving or cross-checking them before later wallet logic depends on them.
