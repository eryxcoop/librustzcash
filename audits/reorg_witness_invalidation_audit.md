# Audit Findings

Audit target: wallet reorg handling for note witnesses, nullifier state, commitment tree shards, scan ranges, and spendability.

Focus:

- whether cryptographic witness state is invalidated when rewinding to an earlier chain state
- whether nullifier-derived spentness survives in a way inconsistent with the rewind target
- whether scan-queue / shard / mined-transaction state stays mutually consistent during and after reorg handling

High-level result: I confirmed one `Medium` wallet-state integrity issue in the deep-rewind path. When `rewind_to_chain_state` targets a height below the pruning floor, the wallet intentionally preserves stabilized witness state, preserved mined transaction state, and `tx_locator_map` / nullifier state up to the pruning floor rather than to the requested rewind target. Before the forced historic rescan has repaired that state, wallet APIs can continue to treat notes and spentness as if the preserved post-target cryptographic state were still valid.

## Medium: deep rewind preserves stabilized witnesses and nullifier-derived spentness beyond the requested chain state

Affected code:

- `zcash_client_sqlite/src/wallet.rs:3872-3893`
- `zcash_client_sqlite/src/wallet.rs:3961-4014`
- `zcash_client_sqlite/src/wallet.rs:4018-4039`
- `zcash_client_sqlite/src/wallet.rs:3694-3744`
- `zcash_client_sqlite/src/wallet/common.rs:111-120`
- `zcash_client_sqlite/src/wallet/common.rs:438-487`
- `zcash_client_sqlite/src/wallet/common.rs:646-688`
- `zcash_client_sqlite/src/wallet/common.rs:163-180`
- `zcash_client_backend/src/data_api/testing/pool.rs:4620-4725`
- `zcash_client_backend/src/data_api/testing/pool.rs:4947-5145`

### What happens

`rewind_to_chain_state` explicitly does **not** rewind all cryptographic wallet state to the requested `ChainState` height.

Its own documentation says that on a deep rewind it preserves:

- blocks
- note commitment trees
- transactions
- transparent UTXO observations
- nullifier-map entries

down to the pruning floor, not the requested target height:

- `zcash_client_sqlite/src/wallet.rs:3872-3893`

The implementation follows that contract:

- if `target_height < max_scanned_height`, it computes `pruning_floor`
- clamps truncation to a checkpoint at or above that floor
- and calls `truncate_to_height_internal(truncation_height)` instead of truncating all the way to `target_height`

Relevant code:

- `zcash_client_sqlite/src/wallet.rs:3961-4014`

`truncate_to_height_internal` then:

- only un-mines transactions with `mined_height > truncation_height`
- only deletes block rows with `height > truncation_height`
- only deletes nullifier locators with `block_height > truncation_height`

Relevant code:

- `zcash_client_sqlite/src/wallet.rs:3694-3744`

After that, `rewind_to_chain_state` overwrites the scan queue above the **requested** target with a `Historic` rescan range, but preserves the wallet’s pre-rewind view of the chain tip:

- `zcash_client_sqlite/src/wallet.rs:4018-4039`

So there is a period where:

- the requested chain state says “rewind to height `target_height`”
- but wallet cryptographic state below the pruning floor still reflects a later chain

### Why this matters

This retained state is not inert.

First, stabilized notes bypass ordinary shard-scan / confirmation gating:

- `witness_stabilized` lets notes pass spendability checks directly
- `witness_stabilized` also short-circuits confirmation checks

Relevant code:

- `zcash_client_sqlite/src/wallet/common.rs:438-487`
- `zcash_client_sqlite/src/wallet/common.rs:646-688`

Second, note spentness and tracked nullifiers are also derived from preserved transaction / locator state:

- `spent_notes_clause(...)` excludes notes spent by transactions that remain mined or unexpired
- `get_nullifiers(NullifierQuery::Unspent)` excludes notes with spending transactions that remain mined

Relevant code:

- `zcash_client_sqlite/src/wallet/common.rs:111-120`
- `zcash_client_sqlite/src/wallet/common.rs:163-180`

Because deep rewind preserves mined transactions and locator data up to the truncation floor rather than the target height, nullifier-derived spentness can likewise survive above the requested rewind target.

### Concrete evidence in the test suite

The repo’s own tests codify this behavior.

One deep-rewind test explicitly asserts that:

- blocks, transactions, `tx_locator_map` entries, and note commitment trees are only rewound to `tip - (PRUNING_DEPTH - 1)`, not to the rewind target

Relevant test:

- `zcash_client_backend/src/data_api/testing/pool.rs:4620-4725`

Another test explicitly asserts that after `rewind_to_chain_state` to a height **below** the note height:

- notes with surviving `witness_stabilized = 1` remain spendable
- `get_spendable_balance` still returns their full value
- the wallet can immediately propose and build a real spend end-to-end after the rewind

Relevant test:

- `zcash_client_backend/src/data_api/testing/pool.rs:4947-5145`

That is exactly the cryptographic-state-survival pattern this audit prompt asked about.

### Security impact

I rate this `Medium` because:

- I did not confirm theft or a consensus bypass;
- but after a deep rewind, the wallet can continue to use witnesses and nullifier-derived spentness that are inconsistent with the caller-provided target chain state;
- and that inconsistency is strong enough that the wallet can still propose / build spends before the historic rescan has reconciled the state.

Practical consequences include:

- balances may temporarily reflect notes whose spendability is only justified by preserved later-chain shard state;
- notes may remain marked spent based on preserved mined spending transactions above the requested rewind target;
- transaction construction may use witnesses derived from preserved post-target tree state rather than from the rewound chain state the caller intended to restore.

### Why this is not just “eventual consistency”

This is more than a benign rescan lag.

The preserved state is cryptographically meaningful:

- Merkle witness availability is carried by `witness_stabilized`
- note spentness is carried by mined spend rows and nullifier locators
- and both can survive a deep rewind in a way that is intentionally inconsistent with the requested target height

The later historic rescan may repair the state, but until that happens, the wallet is operating on a mixed view of two chain states.

## No confirmed issue in shallow truncation

I did not confirm the same class of inconsistency for the simple / shallow truncation path that fully truncates scanned data to the target checkpoint. The strongest confirmed issue is specifically the deep-rewind preservation behavior below the pruning floor.

## Conclusion

I confirmed a `Medium` reorg-handling issue: on deep rewinds, cryptographic witness state and nullifier-derived spentness can survive above the requested rewind target into a mixed, locally inconsistent wallet state, and that state is strong enough to influence spendability and spend creation before rescan completes.
