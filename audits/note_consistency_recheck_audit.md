# Audit Findings

Audit target: wallet persistence and reconstruction of shielded outputs, with a focus on whether note components, note commitments, nullifiers, commitment-tree positions, memos, and output indices remain mutually bound after crossing the decryption boundary.

High-level result: several earlier reports were describing the same underlying family of `Medium` wallet-boundary issues: once a note-like object crosses into low-level wallet storage, multiple pieces of caller-supplied metadata are persisted and later reused without being rebound to the original tx-bound note commitment and ciphertext-bearing output.

This consolidated report covers the overlapping findings previously split across:

- note consistency recheck;
- nullifier derivation binding;
- commitment tree position binding;
- memo / ciphertext binding at the storage boundary.

Affected code:

- `zcash_client_backend/src/data_api/ll.rs:293-404`
- `zcash_client_backend/src/data_api/ll/wallet.rs:725-750`
- `zcash_client_backend/src/scanning.rs:154-155`
- `zcash_client_backend/src/scanning.rs:195-200`
- `zcash_client_backend/src/scanning.rs:801-810`
- `zcash_client_sqlite/src/wallet/sapling.rs:40-127`
- `zcash_client_sqlite/src/wallet/sapling.rs:329-414`
- `zcash_client_sqlite/src/wallet/orchard.rs:40-130`
- `zcash_client_sqlite/src/wallet/orchard.rs:299-371`
- `zcash_client_sqlite/src/wallet/common.rs:225-245`
- `zcash_client_sqlite/src/wallet/common.rs:382-410`
- `zcash_client_sqlite/src/wallet/common.rs:554-693`
- `zcash_client_sqlite/src/wallet.rs:2785-2816`
- `zcash_client_sqlite/src/wallet.rs:2905-2920`
- `zcash_client_backend/src/data_api/wallet.rs:1170-1229`

## Medium: wallet note storage and reconstruction do not rebind persisted note components to the original published commitment

### What happens

The normal decryption layer is relatively strong: Sapling / Orchard note decryption checks note/plaintext consistency against the published commitment and ephemeral key before returning a note object.

After that point, the low-level wallet storage boundary becomes weaker:

- note internals such as value, diversifier, `rcm` / `rseed` / `rho`, and key scope are persisted from `Received*Output`;
- the storage layer does not recompute the note commitment and compare it against the originally published `cmu` / `cmx`;
- later SQLite spendable-note reconstruction rebuilds notes from stored components but still does not compare the reconstructed note’s commitment back to the original published commitment.

So if inconsistent note components ever cross the boundary into storage, the commitment binding is not restored later.

### Why this matters

This means the wallet persistence layer is not acting as a defensive rebinding boundary. Persisted inconsistency is not self-healing:

- a malformed or incorrectly constructed `Received*Output` can be stored;
- later reconstruction can still produce a note object that looks internally valid;
- and the original on-chain commitment is no longer the final binding anchor for wallet-side state.

### Impact

I rate this `Medium` because persisted note inconsistency can survive into later wallet logic even though I did not confirm a clean false-positive decryption bug in the core cryptography.

## Medium: caller-supplied nullifier and commitment-tree position metadata are trusted without canonical rebinding

### What happens

The same storage boundary also trusts metadata that should be derived or cross-checked:

- optional nullifier fields can be accepted from caller-supplied `Received*Output` state;
- optional commitment-tree positions can be persisted without rebinding them to the note commitment in the wallet’s tree view;
- later spend-selection / witness-generation logic treats those persisted fields as authoritative until a later anchor or spendability failure surfaces.

This is the same root bug as above, specialized to note-adjacent metadata rather than to note internals themselves.

### Why this matters

The effect is wallet-state corruption rather than consensus bypass:

- spentness can be computed from untrusted persisted nullifier metadata;
- witness generation and spend creation can trust the wrong tree position;
- failures show up late, often only when proving or anchor checks happen.

### Impact

I rate this `Medium` because it directly affects spendability and local wallet state, even though I did not confirm a valid on-chain spend with a wrongly bound nullifier or position.

## Medium: memo and output-index metadata are stored independently of the ciphertext-bearing output

### What happens

At the storage layer, memo text and output / action index metadata are also accepted as separate caller-controlled fields:

- the decrypt/recovery path itself obtains memos from the correct ciphertext-bearing output;
- but the persistence layer can store memo and output-index metadata without re-establishing that they still refer to the same underlying encrypted output.

This is not a core note-encryption flaw. It is another instance of the same wallet-boundary rebinding gap.

### Why this matters

If incorrect metadata is persisted once, later wallet reads can surface:

- a memo associated with the wrong logical output;
- or the right memo attached to the wrong output index / action index.

### Impact

I rate this `Medium` because it affects wallet-visible output semantics and shares the same root cause as the stronger note/nullifier/position trust issues.

## Low: SQLite upsert semantics can make stale memo associations sticky

### What happens

SQLite upserts in the memo storage path use `IFNULL(:memo, memo)`-style behavior, so once a memo association is populated incorrectly, later reprocessing with absent memo data can leave the old association in place.

### Why this matters

This is weaker than the issues above, but it means metadata corruption is not always corrected by replay or rescanning.

### Impact

I rate this `Low` because it requires a prior bad association and does not create a new cryptographic mismatch by itself.

## No confirmed finding in the actual trial-decryption consistency checks

I did not confirm a clean bug in the core Sapling / Orchard trial-decryption logic itself.

The confirmed problems all begin after a note object has already crossed into wallet persistence, where the system stops rebinding several note-adjacent fields back to the original tx-bound commitment and ciphertext context.
