# Local Wallet Composed PoC Plan

## Goal

Build the next PoC stage directly on top of the routes already demonstrated in this branch, instead of starting from fresh unproven hypotheses.

The composed local-wallet attack already has two solid footholds:

- `PCZT mutable metadata -> wrong sent-history recipient`
- `missing chain context -> wrong ZIP-212 decryption semantics`

The plan below extends those proven routes into a stronger staged demonstration of local wallet corruption.

## Already demonstrated

### Backend

- `data_api::wallet::tests::extract_and_store_transaction_from_pczt_can_store_metadata_recipient_distinct_from_committed_sapling_recipient`
- `data_api::wallet::tests::extract_and_store_transaction_from_pczt_can_store_metadata_recipient_distinct_from_committed_orchard_recipient`
- `data_api::wallet::tests::decrypt_transaction_without_chain_context_can_accept_legacy_sapling_plaintext_that_current_context_rejects`

### Memory wallet

- `testing::pool::sapling::pczt_sent_history_can_be_misled_by_user_address_and_output_metadata`
- `testing::pool::orchard::pczt_sent_history_can_be_misled_by_user_address_and_output_metadata`
- `testing::tests::decrypt_and_store_transaction_without_chain_context_can_surface_legacy_sapling_state_through_sent_history`
- `testing::pool::sapling::pczt_sent_history_can_reclassify_external_output_as_internal_account`
- `testing::pool::orchard::pczt_sent_history_can_reclassify_external_output_as_internal_account`
- `testing::pool::sapling::pczt_tx_history_can_reuse_internal_account_reclassification_for_external_output`
- `testing::pool::orchard::pczt_tx_history_can_reuse_internal_account_reclassification_for_external_output`
- `testing::pool::sapling::local_wallet_can_simultaneously_surface_legacy_sapling_sent_history_and_pczt_internal_reclassification`
- `testing::pool::orchard::local_wallet_can_simultaneously_surface_legacy_sapling_sent_history_and_pczt_internal_reclassification`
- `testing::pool::sapling::local_wallet_composed_state_can_panic_on_malformed_compact_block_during_followup_scan`
- `testing::pool::orchard::local_wallet_composed_state_can_panic_on_malformed_compact_block_during_followup_scan`

## Not yet demonstrated

- a persistence-chain PoC where wrong-context shielded state is admitted and then shown to survive into later wallet-visible state
- a reorg-amplification PoC on `zcash_client_memory`
- a compact-block malformed-input PoC in the in-memory wallet harness

The first bullet above is now satisfied as a composed same-wallet harness. What remains
undemonstrated is mainly reorg amplification and the stronger rewind-repair variant of the DoS.

## Constraint

`zcash_client_memory` still does not implement `rewind_to_chain_state`, so the reorg-amplification segment cannot currently be completed there without adding missing target functionality first.

That means the next PoC stage should focus on:

1. composition of already-demonstrated admission + persistence + history corruption paths
2. backend-only preparation for later reorg amplification

## Next staged PoC

## Stage A: backend composition of wrong-context admission plus wallet persistence

### Objective

Demonstrate that a transaction accepted under wrong ZIP-212 context is not just decryptable in isolation, but can be fed through wallet persistence APIs and leave meaningful local state behind.

### Status

Implemented in this branch as:

- `decrypt_and_store_transaction_without_chain_context_can_persist_legacy_sapling_state_under_current_semantics_gap`

### Proposed test

- file: `zcash_client_backend/src/data_api/wallet.rs`

### Shape

1. Build a legacy-style Sapling output at a pre-ZIP-212 height.
2. Confirm `decrypt_transaction(..., None, None, ...)` accepts it.
3. Confirm `decrypt_transaction(..., None, Some(current_tip), ...)` rejects it.
4. Feed the same transaction through a wallet-write test double that records stored decrypted state.
5. Assert that the no-context path stores wallet-relevant state that the current-context path would not have admitted.

### Why this matters

This turns the already-proven context mismatch into the first half of a composed wallet-state corruption PoC.

## Stage B: compose admitted state with wallet-visible interpretation

### Objective

Show that once wallet-side state is admitted under the wrong semantic context, the system exposes it through higher-level wallet surfaces rather than keeping it quarantined as an opaque decrypt result.

### Status

Implemented in this branch as:

- `decrypt_and_store_transaction_without_chain_context_can_surface_legacy_sapling_state_through_sent_history`

### Proposed effect to observe

- received-output count
- memo visibility
- account attribution
- any history/summarization field that becomes non-empty only because the no-context decrypt path accepted the output

### Realized effect

The implemented test currently uses:

- `zcash_client_memory`
- a tracked sender UFVK
- `decrypt_and_store_transaction`
- and the wallet-visible `get_sent_outputs` API

to show that:

- with no chain context, the wallet surfaces the legacy Sapling output through sent history;
- with current chain context, the same transaction no longer surfaces through that API.

## Stage C: enrich the PCZT PoC from wrong recipient to stronger semantic drift

### Objective

Start from the already passing PCZT PoCs and attempt to increase the semantic mismatch beyond only the displayed recipient.

### Candidate extensions

- stronger user-facing address mismatch for full UA wrappers
- output-side value mismatch if a non-committed value field can still influence wallet-visible interpretation
- account-classification mismatch if the stored path can be pushed toward a different internal/external classification

### Status

Implemented in this branch via:

- `extract_and_store_transaction_from_pczt_can_store_internal_account_classification_distinct_from_committed_external_sapling_output`
- `extract_and_store_transaction_from_pczt_can_store_internal_account_classification_distinct_from_committed_external_orchard_output`
- `pczt_sent_history_can_reclassify_external_output_as_internal_account`

### Realized effect

The implemented Stage C route shows that:

- the committed shielded output remains externally addressed and sender-recoverable;
- only wallet-bound PCZT metadata is changed;
- backend extraction can then store the same output as `InternalAccount`;
- and the in-memory wallet's sent-history API stops surfacing any external recipient for it.

### Important caution

The value-mismatch branch remains intentionally unimplemented. The current code lets us prove a
recipient-type drift honestly; that is stronger than overclaiming a value drift that the actual
extraction path may not permit.

## Stage D: compose PCZT corruption with later wallet reads

### Objective

Show that the corrupted sent-history state is not only stored once, but reused by later wallet-facing consumers.

### Target

- `zcash_client_memory`

### Status

Implemented in this branch via:

- `pczt_tx_history_can_reuse_internal_account_reclassification_for_external_output`

### Proposed tests

- already realized by querying the wallet's `get_tx_history()` summary after the Stage C
  reclassification

### Realized effect

The Stage D test now shows that the semantic corruption is reused by a broader historical view:

- the same finalized transaction still commits to a real external recipient;
- `get_sent_outputs()` no longer exposes that external recipient after reclassification;
- `get_tx_history()` correspondingly stops counting that payment as a sent external note and
  treats the transaction as having only change-like internal outputs.

## Stage F: same-wallet composed harness

### Objective

Demonstrate both previously proven bug families inside the same local wallet instance rather
than as isolated tests:

- wrong-context legacy Sapling admission through `decrypt_and_store_transaction`
- later PCZT output reclassification through mutable wallet-bound metadata

### Status

Implemented in this branch via:

- `local_wallet_can_simultaneously_surface_legacy_sapling_sent_history_and_pczt_internal_reclassification`

for both the Sapling and Orchard pool variants in `zcash_client_memory`.

### Realized effect

The composed harness now proves that one in-memory wallet can simultaneously contain:

- a legacy Sapling transaction that still surfaces an external recipient through
  `get_sent_outputs()` only because it was first accepted without chain context;
- and a later PCZT-derived transaction whose real external payment is reclassified so that
  `get_sent_outputs()` no longer exposes any external recipient for it.

The strongest wallet-wide effect observed in the composed test is that, once both corruptions
coexist in the same wallet, `get_tx_history()` no longer yields a globally consistent summary
and instead fails while identifying the legacy transaction admitted through the missing-context
path.

## Stage G: executable High-candidate via normal scan DoS

### Objective

Turn the already-running same-wallet composed corruption PoC into a `High` candidate by proving
that the same wallet can then be crashed by one malformed compact block during its next ordinary
scan.

### Status

Implemented in this branch via:

- `local_wallet_composed_state_can_panic_on_malformed_compact_block_during_followup_scan`

for both the Sapling and Orchard pool variants in `zcash_client_memory`.

### Realized effect

The Stage G harness now proves this concrete chain:

- a wallet first enters the same composed corrupt state already demonstrated in Stage F;
- the wallet is still active enough to continue syncing / scanning;
- a single malformed `CompactBlock` with an invalid `prev_hash` length is inserted at the next
  height;
- the next ordinary `scan_cached_blocks()` call then panics while checking block continuity.

This is the current best executable `High` candidate because it upgrades the prior
integrity-only corruption into a deterministic non-distributed DoS against one wallet.

## Stage E: reorg amplification preparation

### Current blocker

- `zcash_client_memory::rewind_to_chain_state` is not implemented

### Additional blocker

- the stronger `rewind + historic repair scan -> panic` variant exists as a backend testing
  helper shape, but it is not currently executable on the in-memory target for the same reason.

### Short-term plan

- do not attempt to fake the reorg PoC in memory
- instead, document and preserve the existing backend-level reorg evidence
- optionally add a small explicit note in test comments or docs that the memory target is blocked by missing rewind support

### Medium-term plan

If we choose to expand scope later:

1. implement the minimum `rewind_to_chain_state` behavior in `zcash_client_memory`
2. port the existing backend test scenarios:
   - `rewind_to_chain_state_deep`
   - `newly_discovered_notes_become_stabilized`
3. then compose that with previously admitted/corrupted local state

## Comment quality requirements

All new tests should keep the exact same quality bar as the existing PCZT PoCs.

Every test should include:

- an opening comment that states what the test demonstrates
- a comment describing why the setup isolates the intended bug
- a comment before the attacker-controlled mutation or context skew
- a comment before the cryptographic ground-truth recovery step
- a comment before the wallet-visible observation step
- a final comment explaining precisely what mismatch proves the bug

Avoid:

- opaque helpers without commentary
- bundling multiple independent bug classes into one test without narrative
- “magic” assertions that are not explained immediately beforehand

## Recommended implementation order

1. keep the new Stage F/Stage G same-wallet harnesses green while extending adjacent PoCs
2. Stage E: revisit reorg only after the target supports rewind

## Success condition

The current composed PoC round is successful because we can now truthfully say:

- one proven route corrupts wallet-visible outgoing history after signing/extraction
- another proven route admits shielded state under the wrong consensus-era semantics
- the admitted state becomes durable wallet-visible state rather than a transient decrypt artifact
- and a same-wallet harness shows both families coexisting in one local wallet instance
- and a follow-up same-wallet harness shows that one malformed compact block can then crash the
  next normal scan attempt
