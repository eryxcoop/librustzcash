# Audit Findings

Audit target: composed local-wallet impact obtained by chaining the strongest confirmed wallet-boundary findings together.

High-level result: I did not confirm an end-to-end chain that steals funds on chain or bypasses consensus. I did confirm enough compatible local-wallet weaknesses to describe a stronger composite issue:

- a local wallet can ingest untrusted or consensus-unvalidated shielded state;
- persist and later reuse note-adjacent metadata without rebinding it to the original tx-bound commitment/output context;
- trust mutable PCZT-side recipient intent after signing/extraction;
- preserve inconsistent spendability/witness/nullifier state across deep rewinds;
- and in some branches crash before it can self-repair.

Taken together, this supports a `High`-severity local-wallet impact chain: persistent wallet-state corruption, false sent/received history, wrong spendability decisions, late spend/proving failures, and reliable DoS against recovery or rescan workflows.

This report intentionally composes the strongest confirmed findings from:

- `consensus_wallet_validation_gap_audit.md`
- `note_consistency_recheck_audit.md`
- `pczt_cryptographic_intent_binding_audit.md`
- `reorg_witness_invalidation_audit.md`
- `rseed_randomness_version_confusion_audit.md`

## Current PoC status

The composed chain is no longer only a paper composition. The following pieces are already
demonstrated as executable tests in this branch:

- `zcash_client_backend::data_api::wallet::tests::extract_and_store_transaction_from_pczt_can_store_metadata_recipient_distinct_from_committed_sapling_recipient`
- `zcash_client_backend::data_api::wallet::tests::extract_and_store_transaction_from_pczt_can_store_metadata_recipient_distinct_from_committed_orchard_recipient`
- `zcash_client_backend::data_api::wallet::tests::extract_and_store_transaction_from_pczt_can_store_internal_account_classification_distinct_from_committed_external_sapling_output`
- `zcash_client_backend::data_api::wallet::tests::extract_and_store_transaction_from_pczt_can_store_internal_account_classification_distinct_from_committed_external_orchard_output`
- `zcash_client_backend::data_api::wallet::tests::decrypt_transaction_without_chain_context_can_accept_legacy_sapling_plaintext_that_current_context_rejects`
- `zcash_client_backend::data_api::wallet::tests::decrypt_and_store_transaction_without_chain_context_can_persist_legacy_sapling_state_under_current_semantics_gap`
- `zcash_client_memory::testing::tests::decrypt_and_store_transaction_without_chain_context_can_surface_legacy_sapling_state_through_sent_history`
- `zcash_client_memory::testing::pool::sapling::pczt_sent_history_can_be_misled_by_user_address_and_output_metadata`
- `zcash_client_memory::testing::pool::orchard::pczt_sent_history_can_be_misled_by_user_address_and_output_metadata`
- `zcash_client_memory::testing::pool::sapling::pczt_sent_history_can_reclassify_external_output_as_internal_account`
- `zcash_client_memory::testing::pool::orchard::pczt_sent_history_can_reclassify_external_output_as_internal_account`
- `zcash_client_memory::testing::pool::sapling::pczt_tx_history_can_reuse_internal_account_reclassification_for_external_output`
- `zcash_client_memory::testing::pool::orchard::pczt_tx_history_can_reuse_internal_account_reclassification_for_external_output`
- `zcash_client_memory::testing::pool::sapling::local_wallet_can_simultaneously_surface_legacy_sapling_sent_history_and_pczt_internal_reclassification`
- `zcash_client_memory::testing::pool::orchard::local_wallet_can_simultaneously_surface_legacy_sapling_sent_history_and_pczt_internal_reclassification`
- `zcash_client_memory::testing::pool::sapling::local_wallet_composed_state_can_panic_on_malformed_compact_block_during_followup_scan`
- `zcash_client_memory::testing::pool::orchard::local_wallet_composed_state_can_panic_on_malformed_compact_block_during_followup_scan`

These tests already prove two important composed routes:

- post-signing recipient-intent corruption survives through extraction and into wallet-visible
  sent history;
- post-signing recipient-type corruption can reclassify a committed external output as
  wallet-internal in stored wallet semantics;
- that same recipient-type corruption is then reused by higher-level wallet history summaries,
  not just by the narrow sent-output listing API;
- wrong-chain-context Sapling decryption semantics can change whether the same transaction is
  treated as wallet-owned;
- and that same context mismatch can survive through `decrypt_and_store_transaction` into
  recorded wallet-relevant persisted state;
- and from there into a real wallet-visible API surface in the in-memory wallet
  (`get_sent_outputs`) when the transaction is treated as outgoing under the wrong context.
- and now a same-wallet harness shows that both bug families can coexist inside one local
  wallet instance: one tx still exposes an external recipient only because it was first
  admitted under missing-context legacy semantics, while a later PCZT-derived tx in the same
  wallet hides its real external recipient after output reclassification.
- in that same composed harness, `get_tx_history()` no longer yields a globally consistent
  history summary and instead fails while identifying the legacy transaction admitted through
  the missing-context path.
- and the current strongest executable `High` candidate now extends that same wallet-harness one
  step further: after the wallet has already entered the composed corrupt state, a single
  malformed compact block can deterministically crash the next normal scan attempt.

What remains undemonstrated as an executable composed PoC in this branch is mainly the
reorg-amplification half on the in-memory wallet target. That is blocked today because
`zcash_client_memory` still leaves `rewind_to_chain_state` unimplemented.

## High: composed local-wallet corruption plus scan-time DoS chain

### Attacker model

The chain is local-wallet-facing rather than consensus-facing. The attacker needs one or more ways to feed the wallet untrusted state, for example:

- a malicious or corrupted compact-block / transaction data source;
- a recovery/import flow that calls decryption or storage APIs on unvalidated transactions;
- a malicious PCZT coparticipant or tooling step that mutates wallet-bound output metadata before extraction;
- or a workflow that triggers deep rewind / historic rescan while corrupted wallet state is already present.

I am not claiming all of these capabilities are simultaneously available in every deployment. The point is that the confirmed bugs compose cleanly once untrusted state crosses the wallet boundary.

### Phase 1: admission of unvalidated or wrong-context shielded state

The first stage is getting the wallet to accept shielded data that has not been fully validated under the right consensus semantics.

Confirmed building blocks:

- `decrypt_and_store_transaction` can process transactions that have not first passed local consensus validation.
- compact scanning can surface decrypted outputs as wallet-owned/spendable without local proof/signature verification.
- malformed compact metadata can also panic the scanner.
- if chain context is absent, Sapling ZIP-212 enforcement can fall back too far back in time and accept note plaintext semantics that current consensus would reject.

Effect:

- the wallet can admit note-like state under weaker guarantees than “this is a consensus-valid current-chain output”.

### Phase 2: persistence without rebinding

Once note-like state crosses into low-level wallet storage, several later checks are missing.

Confirmed building blocks:

- persisted note internals are not rebound to the original published `cmu` / `cmx`;
- caller-supplied nullifier and commitment-tree position metadata can be stored without canonical rebinding;
- memo and output-index metadata can be stored independently of the ciphertext-bearing output;
- SQLite upsert behavior can make some incorrect memo associations sticky;
- post-ZIP-212 Sapling note semantics can be degraded when notes are reconstructed from storage.

Effect:

- corrupted or semantically stale wallet state can survive persistence;
- later wallet logic can reconstruct internally plausible notes and metadata even though the original tx-bound anchors are no longer the final source of truth.

### Phase 3: intent-layer corruption for sent outputs

For outgoing flows, the wallet has a second independent trust problem after transaction effects are already fixed.

Confirmed building blocks:

- PCZT output-intent metadata is wallet-bound, not consensus-bound or signature-bound;
- recipient/value/account attribution for shielded sent outputs can come from mutable metadata before final rebinding;
- full Unified Address metadata can be changed after pool selection without changing the actual extracted transaction.

Effect:

- the wallet can persistently display the wrong recipient, wrong full UA, wrong value, or wrong internal/external classification for sent outputs even though the extracted transaction itself is unchanged.

### Phase 4: amplification into spendability and spend construction

The earlier phases are not just cosmetic. They can influence later wallet behavior.

Confirmed building blocks:

- persisted nullifier/position metadata is later trusted by spend-selection / witness-generation logic until failure surfaces late;
- reconstructed note state can be treated as spendable without being rebound to the original commitment;
- deep rewind can preserve stabilized witness and nullifier-derived spentness above the requested target height.

Effect:

- the wallet can continue operating on mixed or corrupted cryptographic state;
- balances and spendability can look valid locally;
- later proposal/build/proving stages fail only when a stronger anchor/proving invariant is finally checked.

In other words, the corruption can survive long enough to influence user-visible “can I spend this?” answers and transaction-construction attempts.

### Phase 5: crash on the next normal scan

The chain can be hardened further by panic surfaces that stop cleanup or corrective rescans.

Confirmed building blocks:

- decryptable out-of-range shielded note values can panic wallet code instead of being rejected;
- malformed compact identifiers, commitments, heights, or nullifiers can panic the scanner.

Effect:

- once the wallet is already in a semantically corrupted local state, a malicious server-fed
  compact block can turn that corruption into an operational DoS during the very next ordinary
  scan attempt;
- the wallet does not need to reach a special “repair mode” first for the crash to become
  user-visible;
- in the currently demonstrated variant, the crash happens while checking block continuity
  against a malformed `prev_hash` field.

## Why the combined impact is higher than the individual reports

Individually, many of the findings were “only”:

- acceptance of unvalidated shielded state;
- metadata rebinding after persistence;
- recipient-intent corruption;
- stale reorg state;
- or panic/DoS.

Together, they form a stronger local-wallet failure mode:

1. admit untrusted shielded state,
2. persist it without enough rebinding,
3. let it influence history/spendability/building,
4. preserve some of that state across rewind boundaries,
5. and optionally crash before the wallet can self-correct.

That combination is materially worse than any one report in isolation because it turns transient trust-boundary mistakes into durable wallet corruption.

## Concrete local-wallet impact

The highest confirmed combined impact is now:

- persistent wrong sent/received history;
- wrong recipient / full-UA / account attribution for outputs;
- higher-level transaction summaries that inherit the same false internal/external classification;
- corrupted memo associations;
- spendable-looking notes or balances that are inconsistent with fully validated chain state;
- late failures during witness generation / proving / extraction;
- wallet crash during scan / recovery / decrypt-and-store processing.

The strongest executable path in the branch today is:

1. admit a legacy Sapling transaction under missing chain context;
2. later store a second outgoing transaction with PCZT-side internal/external misclassification;
3. observe that the same wallet now has contradictory sent-output semantics and a broken
   `get_tx_history()` view;
4. then feed it one malformed compact block and crash the next ordinary scan pass.

I did not confirm:

- direct theft of current-transaction funds;
- a valid on-chain spend created from incorrectly bound nullifiers/positions;
- or a consensus-invalid transaction becoming valid on chain via this chain.

That is why the current disclosure framing can now defend `High` as a
`non-distributed denial of service against an individual wallet`, in addition to the already
confirmed local-wallet integrity corruption. It is still not `Critical`, because I did not
confirm theft, destruction of funds, or consensus breakage.

## Strongest practical composition paths

Two compositions now look especially realistic:

- `malicious lightwalletd or untrusted recovery input`
  - feeds malformed or consensus-unvalidated shielded data
  - wallet stores it
  - note/nullifier/position/memo state persists without rebinding
  - later ordinary scan or repair path misbehaves or crashes

- `malicious PCZT coparticipant or workflow step`
  - leaves tx effects intact
  - mutates wallet-bound output intent metadata
  - wallet stores wrong recipient/value/account history
  - later persistence and reorg semantics make cleanup harder

## PoC guidance

The most promising PoC plan is not “one giant exploit” first. It is to build a staged local-wallet demonstration:

1. inject unvalidated or wrong-context shielded state into a local wallet;
2. show it gets persisted;
3. show later wallet reads / spend selection / history reflect corrupted state;
4. then crash a normal wallet operation such as scanning, rescan, or recovery with a single
   malformed follow-up input.

That staged PoC would demonstrate the composite impact more convincingly than isolated unit behaviors.

## Recommended next PoC increments

Given the tests already implemented in this branch, the most productive next increments are:

1. Revisit the stronger `repair-path DoS` / `rewind + historic rescan` variant once either:
   - `zcash_client_memory::rewind_to_chain_state` is implemented, or
   - we decide to accept a backend-only reorg PoC as sufficient evidence for that stage.
2. If needed for disclosure packaging, split the same-wallet composed harness into a
   presentation-friendly sequence of intermediate assertions or screenshots, while preserving
   the current executable test as the canonical proof.
