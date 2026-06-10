# Audit Findings

Audit target: note commitment tree position assignment and use across scanning, wallet storage, spend creation, witness generation, and nullifier computation.

Focus:

- whether note positions are trusted or rederived
- whether stored positions remain bound to the note and the commitment tree root used for proving
- whether a note can be stored or selected for spend with an inconsistent position

High-level result: I did not confirm a consensus-level bug where an inconsistent note position leads to a valid on-chain spend. The strongest confirmed issue is a wallet-state integrity gap: low-level wallet ingestion/storage accepts caller-supplied note commitment tree positions without rebinding them to the note commitment, and later spend-selection / witness-generation code trusts those stored positions until the builder’s anchor-consistency check fails.

## Medium: wallet storage accepts untrusted commitment tree positions and later spend creation trusts them until late anchor mismatch

Affected code:

- `zcash_client_backend/src/scanning.rs:801-810`
- `zcash_client_sqlite/src/wallet/sapling.rs:329-400`
- `zcash_client_sqlite/src/wallet/orchard.rs:300-357`
- `zcash_client_sqlite/src/wallet/common.rs:225-245`
- `zcash_client_sqlite/src/wallet/common.rs:382-410`
- `zcash_client_backend/src/data_api/wallet.rs:1170-1229`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/builder.rs:624-643`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/builder.rs:574-591`

### What happens

In the normal scan path, note positions are assigned canonically from block-relative output ordering:

- `zcash_client_backend/src/scanning.rs:801-810`

That is the good path.

However, the low-level wallet storage boundary does not require that the position it stores be rederived from trusted tree state. The SQLite note-ingestion paths persist:

- note internals from `output.note()`
- optional nullifier from `output.nullifier()`
- optional `commitment_tree_position` from `output.note_commitment_tree_position()`

without checking that the supplied position actually corresponds to the note commitment in the wallet’s view of the tree:

- `zcash_client_sqlite/src/wallet/sapling.rs:329-400`
- `zcash_client_sqlite/src/wallet/orchard.rs:300-357`

Later, spendable-note selection only requires that a stored note have a non-null position:

- `zcash_client_sqlite/src/wallet/common.rs:225-245`
- `zcash_client_sqlite/src/wallet/common.rs:382-410`

And spend creation uses that stored position to fetch a witness at the chosen checkpoint height:

- `zcash_client_backend/src/data_api/wallet.rs:1170-1229`

The note and witness are then passed into the Sapling / Orchard builders, which finally rebind them by checking that the note commitment plus that witness reproduces the anchor:

- Sapling: `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/builder.rs:624-643`
- Orchard: `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/builder.rs:574-591`

### Why this matters

This means an inconsistent position can be:

- stored in wallet state,
- considered “spendable enough” by intermediate wallet logic,
- and used to fetch a witness for the wrong leaf,

before being rejected only at spend-construction time by an anchor mismatch.

That is not a consensus bypass. It is still a real integrity and reliability issue because it lets untrusted or stale position data survive deep into later wallet operations.

Concrete consequences include:

- a note can be persisted with a wrong position relative to the wallet’s tree;
- the wallet can later attempt to spend it using a witness for that wrong position;
- nullifier computation paths that rely on position for Sapling are no longer tied to the canonical stored tree location if wallet state was poisoned earlier;
- spend creation fails late, after the note has already been selected and treated as a candidate spend.

### Impact

I rate this `Medium` because:

- I did not confirm theft or a valid inconsistent spend on chain;
- but the wallet does accept and retain untrusted position metadata across a security-sensitive boundary;
- and later proving / nullifier-related operations trust that metadata longer than they should.

This is best understood as wallet-state corruption / late-failure risk rather than a consensus flaw.

## No confirmed finding in the builder-side proving boundary

The final spend builders do appear to fail closed.

For both Sapling and Orchard, `add_spend(...)` checks that the note commitment, when combined with the supplied Merkle path, yields the requested anchor:

- Sapling: `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/builder.rs:624-643`
- Orchard: `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/builder.rs:574-591`

So I did not confirm a path where:

- the wallet fetches a witness for one position,
- proves spend authority for a different note,
- and still produces a valid transaction.

The anchor consistency check appears to block that.

## No confirmed issue in canonical scan-time position assignment

I also did not confirm a position-assignment bug in the standard compact-scanning path itself.

In the audited code, decrypted outputs receive positions from the block-local position tracker at the same time their note commitments are appended into the scanned tree representation:

- `zcash_client_backend/src/scanning.rs:801-810`

So the strongest issue here is not “scanning assigns the wrong position by default”, but “wallet storage will trust a supplied position without re-establishing that binding later until a much later stage.”

## Conclusion

I did not confirm a proving or consensus bypass based on inconsistent note commitment tree positions. The confirmed issue is a `Medium` wallet-boundary integrity gap: commitment tree positions can be stored and treated as authoritative before they are rebound to the note and checkpoint root, causing late spend failures and state corruption risk.
