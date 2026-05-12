# PCZT Audit Findings

## Executive summary

This pass focused only on the PCZT subsystem and on exploit variant A:

> committed shielded recipient `B` vs displayed / stored recipient metadata `A`

I did **not** prove:
- a signature forgery,
- a spend-authorization bypass,
- or a consensus-valid transaction that can mean two different things on-chain.

I **did** prove a concrete end-to-end semantic mismatch within the librustzcash PCZT flow:

- a malicious coordinator can mutate only output metadata,
- leave the committed shielded output recipient unchanged,
- still obtain valid signatures,
- still extract and store the transaction successfully,
- and cause the wallet/history reconstruction path to store recipient `A` while the committed shielded output actually pays recipient `B`.

I now have passing backend executable PoCs for both Sapling and Orchard variants of this mismatch.

That makes the strongest candidate no longer “merely theoretical metadata divergence”.

## Is the mismatch actually exploitable?

**YES**

Within the scope of librustzcash’s PCZT + sent-history reconstruction flow, I now have a passing executable PoC that demonstrates:

- **Committed recipient:** `B`
- **Displayed/stored recipient:** `A`
- **Transaction accepted:** yes

What this proves:
- the sent-history reconstruction path can trust recipient metadata that is not cryptographically bound to the committed output recipient.
- this behavior is reproducible in backend tests for both Sapling and Orchard outputs.
- this behavior is also reproducible against the concrete `zcash_client_memory` wallet-store implementation for both Sapling and Orchard.
- a local Zallet harness can be made to surface the forged Orchard recipient in transaction-history RPC output once fed a transaction derived from a malicious PCZT.

What it does **not** prove:
- that a human signer UI in a deployed wallet definitely displays the forged metadata before approval.
- that current production Zallet send flows are themselves reachable via this vector.

So:
- **history / wallet summary deception:** demonstrated
- **Zallet history/RPC deception under local malicious-PCZT harness:** demonstrated
- **signer-confirmation deception:** still not directly demonstrated from this repo alone
- **consensus / authorization integrity failure:** not shown

## Threat model

This audit assumes:

1. A malicious coordinator constructs a misleading PCZT for a signer or wallet consumer.
2. A signer sees only a partial or semantically misleading transaction view.
3. Different participants validate different subsets of fields.
4. A pruned PCZT preserves enough cryptographic validity to pass low-level checks while violating higher-level semantic expectations.
5. A helper API exposes stronger guarantees than it actually enforces.
6. A signing role assumes another role already validated something important.
7. A transaction component is “valid enough for signing” but not semantically equivalent to what another participant thinks they are signing.

I distinguish:

- **cryptographic validity**
  - signatures verify,
  - nullifiers / note commitments / randomized keys are internally consistent,
  - extracted transaction passes verifier checks,

from:

- **semantic correctness**
  - displayed recipient matches committed recipient,
  - helper summaries describe the same transaction the signatures authorize,
  - role-local metadata does not mislead downstream consumers.

## Architecture mapping

### Roles

- `Creator`
  - initializes global structure and per-protocol bundles.
- `Updater`
  - adds metadata such as ZIP-32 derivations and user-facing addresses.
- `IoFinalizer`
  - computes binding-signature signing keys, signs dummy spends, freezes modifiability bits.
- `Signer`
  - computes sighashes and applies transparent / Sapling / Orchard signatures.
- `Low-level Signer`
  - exposes parsed bundles directly to dependency-constrained callers.
- `Prover`
  - adds proof material.
- `Combiner`
  - merges multiple PCZTs.
- `Redactor`
  - prunes fields.
- `SpendFinalizer`
  - finalizes transparent partial signatures.
- `TransactionExtractor`
  - assembles and verifies the final transaction.
- `Verifier`
  - exposes parsed protocol bundles for caller-driven checks.

### Data flow

1. `Pczt::parse` / `serialize` store protocol bundles plus auxiliary metadata.
2. `Pczt::extract_tx_data` parses each per-protocol bundle into upstream `transparent::pczt`, `sapling::pczt`, and `orchard::pczt`.
3. `Signer::new` derives a transaction-wide `shielded_sighash` from **effects-only** extraction.
4. Per-protocol signers apply signatures against `rk` / `sighash`.
5. `TransactionExtractor::extract`:
   - extracts fully signed bundles,
   - adds binding signatures,
   - freezes the final transaction,
   - verifies the result.
6. `extract_and_store_transaction_from_pczt`
   - finalizes spends,
   - reconstructs sent-output summaries from a mix of committed note data and PCZT metadata,
   - stores sent-transaction history.

### Trust boundaries

- **Cryptographically committed transaction data**
  - transparent prevouts, values, script pubkeys, sequence
  - Sapling / Orchard commitments, nullifiers, randomized keys, ciphertexts, proofs
  - global tx version / branch / expiry / locktime / outputs / spends
  - all data flowing into `TransactionData` and `sighash`

- **Helper metadata only**
  - `user_address`
  - proprietary output metadata (`PROPRIETARY_OUTPUT_INFO`)
  - ZIP-32 derivation metadata
  - some ownership / provenance hints

- **Partially semantic fields**
  - `recipient`, `value`, `rseed`, `rho`, `witness`, `alpha`
  - these can be used to recompute invariants, but many checks are optional or caller-driven

### Pruning boundaries

Redactors can remove:
- recipient/value fields,
- `alpha`,
- witnesses,
- proof-generation keys / FVKs,
- user-facing addresses,
- proprietary output info.

That means later roles may still have enough for low-level signing while lacking full semantic context.

### Signature boundaries

- Transparent signatures bind the per-input sighash and selected transparent fields.
- Sapling / Orchard spend signatures bind the **shielded sighash**, which commits to transaction effects.
- Binding signatures are generated later from accumulated trapdoors.

### Semantic-validation boundaries

Upstream Sapling / Orchard PCZT bundles provide helpers such as:
- `verify_nullifier`
- `verify_rk`
- `verify_cv`
- `verify_note_commitment`

But these checks are:
- not uniformly invoked by high-level roles,
- conditional on optional fields being present,
- and frequently described as caller responsibilities.

## Authoritative recipient source mapping

### Committed recipient source

For shielded outputs, the committed recipient is derived from:

- Sapling:
  - `recipient`
  - `value`
  - `rseed`
  - note commitment / ciphertext

- Orchard:
  - `recipient`
  - `value`
  - `rho`
  - `rseed`
  - note commitment / ciphertext

These determine:
- note commitment validity,
- note decryption,
- output recovery,
- actual on-chain recipient semantics.

### Displayed / stored recipient source

For external sent outputs, `extract_and_store_transaction_from_pczt` uses:

- `user_address`
- `PROPRIETARY_OUTPUT_INFO` -> `PcztRecipient::External`

to build:

- `Recipient::External { recipient_address: addr, ... }`

That `Recipient::External` is then persisted as the wallet’s sent-output view.

### Critical distinction

Therefore:

- **committed recipient source** = note/output fields
- **stored/displayed external recipient source** = metadata

This is the semantic split the PoC exploits.

## UI / proposal / history trust analysis

### Proposal-time metadata production

`create_pczt_from_proposal` writes:
- `user_address`
- proprietary `PcztRecipient`

from proposal recipient metadata.

This is reasonable for honest coordinators, but it means later consumers depend on those fields remaining honest.

### History reconstruction

`extract_and_store_transaction_from_pczt`:

- reconstructs note and memo from committed output fields,
- but reconstructs external-recipient label from metadata.

So a mixed-source object is created:
- recipient label from metadata,
- note/memo semantics from committed data.

### Signer trust

Inside this repo, I still did **not** find a final human signer UI that definitely displays `user_address` before approval.

So the demonstrated exploit is strongest as:
- **wallet history / transaction summary deception**

not yet:
- **human signer-confirmation deception**

## Why the PoC is still valid without `create_pczt_from_proposal`

One possible objection is: the successful runtime PoCs do not start from the exact
high-level helper `create_pczt_from_proposal`, so maybe they only demonstrate misuse
of lower-level internals rather than a real vulnerability.

I do **not** think that objection holds.

Why:

1. The PoCs use public, intended APIs throughout:
   - `Builder::build_for_pczt`
   - `Creator::build_from_parts`
   - `IoFinalizer`
   - `Updater`
   - `Prover`
   - `Signer`
   - `SpendFinalizer`
   - `TransactionExtractor`
   - `extract_and_store_transaction_from_pczt`
2. The exploit does not rely on byte-level patching, invalid serialization, `unsafe`, or
   private-field mutation.
3. PCZT exists precisely for multi-party / coordinator / hardware-wallet style workflows.
   In that threat model, a malicious or semantically dishonest coordinator is a natural
   adversary, not an absurd out-of-scope caller.
4. Therefore the right security question is not “does the honest helper produce this state
   by itself?”, but “can a caller using the public PCZT API construct or mutate a valid
   PCZT whose displayed semantics diverge from its committed semantics?”

That question is answered **yes** by the current PoCs.

So the strongest accurate framing is:

- this is a public-API semantic-integrity issue in the PCZT workflow;
- it is not merely a bug in `create_pczt_from_proposal`;
- and it is not invalidated by the fact that the adversarial test uses lower-level public
  PCZT roles instead of only the honest high-level helper.

## Why `PcztRecipient::External` matters in the exploit

Another subtle point is that mutating only `user_address` is not by itself sufficient to
make the fake recipient appear in sent history.

The reason is that `extract_and_store_transaction_from_pczt` reconstructs two pieces of
information independently:

1. the committed shielded note from note fields such as `recipient`, `value`, `rseed`
   (and `rho` for Orchard), and
2. the recipient *classification* from proprietary metadata,
   specifically `PROPRIETARY_OUTPUT_INFO -> PcztRecipient`.

For external shielded outputs, the decisive match is:

- `PcztRecipient::External`
- plus `Some(user_address)`

which is what causes the code to build:

- `Recipient::External { recipient_address: addr, ... }`

If the output is not marked `External`:

- the sent-history code may treat it as internal,
- or skip it as dummy / unrecoverable if the proprietary metadata is absent,
- and the forged displayed external recipient will not materialize in the same way.

So the exploit requires:

- preserving the output's classification as an external recipient, and
- changing the advisory displayed address attached to that classification.

This is not an artificial extra condition. In the honest high-level flow,
`create_pczt_from_proposal` already writes `PcztRecipient::External` for external outputs.
Our lower-level PoCs simply preserve or recreate that same public metadata state while
changing the displayed address.

## Is this just misuse of the `Updater` role?

I do not think the best reading is “the PoC abuses `Updater` in a way the library never
intended, so the result is not a real vulnerability.”

The more precise reading is:

- semantically, yes, the coordinator is behaving dishonestly;
- technically, no, the library does not enforce a contract that forbids this mutation.

Why:

1. The `Updater` role is intentionally broad and public-facing: “anyone can contribute.”
2. The updater APIs expose mutation of bundle fields and proprietary metadata directly.
3. `IoFinalizer` freezes transaction effects by lowering `Global.tx_modifiable`, but that
   mechanism protects committed transaction structure, not advisory display metadata like
   `user_address`.
4. No later generic role re-checks that:
   - `user_address`
   - `PcztRecipient::External`
   - and the committed output recipient
   remain semantically aligned.

So the PoC is not relying on undefined behavior. It relies on a missing invariant:

> the library never re-binds recipient presentation metadata to the committed shielded recipient

That missing invariant is exactly the bug.

## Exploit engineering

### Exact exploit goal

Produce a valid PCZT / transaction flow where:

- committed shielded recipient = `B`
- mutated metadata recipient = `A`
- signatures still verify
- extraction/storage still succeeds
- stored/displayed recipient = `A`

### Exact mutated fields

In the successful Sapling PoC, I mutated only:

- `output.user_address`

and set / preserved:

- `PROPRIETARY_OUTPUT_INFO = PcztRecipient::External`

I did **not** mutate:
- committed Sapling `recipient`
- `value`
- `rseed`
- note commitment fields
- ciphertexts
- sighash-relevant transaction effects

### Why signatures still verify

The shielded signatures bind the shielded sighash over transaction effects.

They do **not** bind:
- `user_address`
- proprietary output metadata

And the high-level signer does not perform output-side semantic equality checks.

## End-to-end semantic mismatch analysis

### Exact extraction/storage path

The exploit flows through:

1. `Creator::build_from_parts`
2. `IoFinalizer::finalize_io`
3. `Updater::update_sapling_with(...)`
   - mutate `user_address = A`
   - leave committed recipient fields = `B`
4. `Prover::create_sapling_proofs(...)`
5. `Signer::sign_transparent(...)`
6. `SpendFinalizer::finalize_spends(...)`
7. `TransactionExtractor::extract(...)`
8. `extract_and_store_transaction_from_pczt(...)`
9. `store_transactions_to_be_sent(...)`

### Exact committed recipient

Recovered from the finalized transaction via sender OVK output recovery:

- `recovered_recipient_addr == committed_recipient_addr == B`

### Exact displayed/history recipient

Captured from stored sent outputs:

- `displayed_recipient == fake_displayed_addr == A`

### Exact result

Committed recipient: `B`  
Displayed/stored recipient: `A`  
Transaction accepted: `yes`

## Coordinator deception feasibility

### Can coordinator set committed recipient = B and displayed recipient = A?

**Yes**

Because:
- committed shielded recipient fields and `user_address` are distinct,
- updater APIs allow direct modification of `user_address`,
- no generic PCZT role rebinds them before signing.

### Can valid signatures still be obtained?

**Yes**

Demonstrated by the passing PoC test.

### Can different participants see different semantics?

**Yes**

- committed on-chain / decryptable recipient remains `B`
- sent-history reconstruction stores `A`

## Concrete PoC attempts

### PoC attempt 1: static source proof

Succeeded.

I proved by tracing that:

1. `create_pczt_from_proposal` stores external recipient metadata in `user_address` and `PROPRIETARY_OUTPUT_INFO`
2. the high-level signer does not verify output-side semantic equality
3. `extract_and_store_transaction_from_pczt` reconstructs external sent recipients from metadata rather than committed recipient fields

### PoC attempt 2: SQLite-backed harness

Partially attempted, but blocked by unrelated `zcash_client_sqlite --features pczt-tests` build failures outside this specific candidate.

This blocker is now non-fatal, because the backend-only PoC below succeeded.

### PoC attempt 3: backend-only executable unit test (Sapling)

Succeeded.

Implemented test:

- `data_api::wallet::tests::extract_and_store_transaction_from_pczt_can_store_metadata_recipient_distinct_from_committed_sapling_recipient`

Command:

```sh
cargo test -p zcash_client_backend --features pczt extract_and_store_transaction_from_pczt_can_store_metadata_recipient_distinct_from_committed_sapling_recipient -- --nocapture
```

Observed result:

```text
running 1 test
test data_api::wallet::tests::extract_and_store_transaction_from_pczt_can_store_metadata_recipient_distinct_from_committed_sapling_recipient ... ok
```

### What the passing PoC proves

The test:

1. builds a valid transparent->Sapling PCZT,
2. sets the real committed external Sapling recipient to `B`,
3. mutates only `user_address` to `A`,
4. proves and signs successfully,
5. extracts a valid transaction successfully,
6. recovers the committed recipient from the finalized transaction as `B`,
7. stores sent-output history in a capturing wallet DB mock,
8. observes stored/displayed recipient `A`.

This is the strongest result of the entire PCZT audit.

### PoC attempt 4: backend-only executable unit test (Orchard)

Succeeded.

Implemented test:

- `data_api::wallet::tests::extract_and_store_transaction_from_pczt_can_store_metadata_recipient_distinct_from_committed_orchard_recipient`

Command:

```sh
cargo test -p zcash_client_backend --features pczt extract_and_store_transaction_from_pczt_can_store_metadata_recipient_distinct_from_committed_orchard_recipient -- --nocapture
```

Observed result:

```text
running 1 test
test data_api::wallet::tests::extract_and_store_transaction_from_pczt_can_store_metadata_recipient_distinct_from_committed_orchard_recipient ... ok
```

### What the passing Orchard PoC proves

The Orchard test:

1. builds a valid transparent->Orchard PCZT,
2. sets the real committed external Orchard recipient to `B`,
3. mutates only `output.user_address` to `A`,
4. proves and signs successfully,
5. extracts a valid transaction successfully,
6. recovers the committed recipient from the finalized transaction as `B`,
7. stores sent-output history in a capturing wallet DB mock,
8. observes stored/displayed recipient `A`.

This shows the same semantic split is not Sapling-specific; it also holds for Orchard in the backend path.

### PoC attempt 5: `zcash_client_memory` implementation harness

Succeeded.

Implemented tests:

- `testing::pool::sapling::pczt_sent_history_can_be_misled_by_user_address`
- `testing::pool::orchard::pczt_sent_history_can_be_misled_by_user_address`

Command:

```sh
cargo test -p zcash_client_memory --features pczt-tests pczt_sent_history_can_be_misled_by_user_address -- --nocapture
```

Observed result:

```text
running 2 tests
test testing::pool::sapling::pczt_sent_history_can_be_misled_by_user_address ... ok
test testing::pool::orchard::pczt_sent_history_can_be_misled_by_user_address ... ok
```

What this adds beyond the backend-only PoCs:

1. The same committed-recipient vs displayed-recipient mismatch survives not only through `extract_and_store_transaction_from_pczt`, but also through a concrete wallet-store implementation that records and returns sent-output history.
2. This removes the argument that the issue is limited to a synthetic mock DB used only in unit tests.
3. It still does not prove reachability in every downstream wallet, but it does prove the bug against one real storage backend in this repository.

### PoC attempt 6: local Zallet history/RPC harness

Succeeded as a local harness in a forked checkout of `zcash/wallet` (Zallet).

What was demonstrated:

1. Zallet does not currently use PCZT in its normal send path; its current send flow goes through `propose_transfer` and `create_proposed_transactions`.
2. A local harness can nevertheless construct an Orchard transaction from a malicious PCZT using the local `librustzcash` checkout, seed Zallet's wallet DB with the resulting transaction and cached `to_address = A`, and then query Zallet's transaction-history logic.
3. Zallet's `list_transactions` path returns the cached/displayed address `A`, while the committed recipient recovered from the Orchard output remains `B`.

What this does **not** show:

- It does not show that stock Zallet users can reach this state through the currently shipped `z_send_many` flow.
- It does not show signer-confirmation deception in Zallet.

Why this still matters:

- It confirms that the history/display layer in another real wallet consumer will trust the cached recipient string once a malicious-PCZT-derived transaction is present.
- It narrows the remaining gap to reachability, not to impact shape.

### Short note on zcashd

I also reviewed the legacy wallet path in `zcashd` to see whether it appears to inherit the same bug shape.

Current conclusion: **probably not, at least not in the same form**.

Why:

1. `zcashd` does not currently use PCZT in its wallet send flow. Its current wallet path goes through `z_sendmany`, `WalletTxBuilder`, and the legacy builder stack rather than a PCZT import / sign / extract flow.
2. Although `zcashd` caches recipient display information for `z_viewtransaction`, that cache is not a free-form `user_address` string. Instead it stores a `RecipientMapping` between the concrete receiver and an optional UA.
3. When persisting this mapping, `CWalletDB::WriteRecipientMapping` explicitly checks that the cached UA actually contains the receiver being stored.
4. The display path in `CWallet::GetPaymentAddressForRecipient` may prefer the cached UA for presentation, but only after matching the receiver and reusing the same underlying recipient semantics.

Relevant code points reviewed:

- `src/wallet/wallet_tx_builder.cpp`
- `src/wallet/wallet.h`
- `src/wallet/wallet.cpp`
- `src/wallet/walletdb.cpp`
- `src/wallet/rpcwallet.cpp`

So the best current reading is:

- `zcashd` may cache a more user-friendly address form for display;
- but it appears to require that cached form to contain the same receiver that the transaction actually commits to;
- therefore it does not currently look like the same “committed recipient B vs displayed recipient A” bug.

### Exact PoC steps

1. Construct a valid transparent-to-Sapling PCZT whose committed shielded recipient is `B`.
2. Mutate only:
   - `sapling output.user_address = A.encode()`
   - preserve `PROPRIETARY_OUTPUT_INFO = PcztRecipient::External`
3. Leave committed fields unchanged:
   - `recipient = B`
   - `value`
   - `rseed`
   - note commitment / ciphertext
4. Create proofs.
5. Sign the transaction successfully.
6. Finalize transparent spends successfully.
7. Extract the final transaction successfully.
8. Recover committed recipient from the finalized transaction using sender OVK:
   - result = `B`
9. Store sent-output history through `extract_and_store_transaction_from_pczt`.
10. Read stored external recipient from captured sent outputs:
   - result = `A`

### Exact mutated fields

- Mutated:
  - `output.user_address`
- Preserved:
  - `PcztRecipient::External`
  - all committed shielded output fields

### Exact committed recipient

- variable in test:
  - `committed_recipient_addr`
- recovered from tx:
  - `recovered_recipient_addr`
- equality asserted:
  - `recovered_recipient_addr == committed_recipient_addr`

### Exact displayed/history recipient

- variable in test:
  - `fake_displayed_addr`
- stored value:
  - `displayed_recipient`
- equality asserted:
  - `displayed_recipient == fake_displayed_addr`
- mismatch asserted:
  - `displayed_recipient != recovered_recipient_addr`

## Remaining blockers

1. I still do not have a proof that an actual human signer UI in a deployed wallet shows `A` before approval.
2. `zcash_client_sqlite --features pczt-tests` still has unrelated build breakage, so SQLite-backed confirmation remains blocked there for both Sapling and Orchard.
3. The current Zallet extension is a local harness result, not a proof that Zallet's current production send path is PCZT-reachable.
4. The current PoC proves backend/history deception, not an actual signer-approval screen.

## Prioritized findings

### [P1] Sent-history reconstruction can record recipient A while the committed shielded output pays recipient B

**Status:** Confirmed  
**Affected components:**  
- `pczt/src/orchard.rs`
- `pczt/src/sapling.rs`
- `pczt/src/roles/signer/mod.rs`
- `zcash_client_backend/src/data_api/wallet.rs`
- upstream `orchard::pczt`
- upstream `sapling::pczt`

**Attack preconditions:**  
- malicious coordinator can construct or mutate a PCZT
- downstream consumer uses `extract_and_store_transaction_from_pczt`
- downstream wallet/history trusts sent-output recipient metadata

**Bug class:** semantic binding failure / transaction-view mismatch / coordinator deception  
**Impact:** wallet history or transaction summaries can show recipient `A` even though the committed shielded output pays recipient `B`. If a signer workflow trusts the same metadata for approval, this could escalate into signer deception.  
**Root cause:** external-recipient metadata (`user_address`, `PcztRecipient`) is not cryptographically bound, but is used as the authoritative source for external sent-output reconstruction.  
**Detailed attack scenario:**  
1. Coordinator creates a valid PCZT for committed recipient `B`.
2. Coordinator overwrites `user_address` to `A`.
3. Proofs and signatures still succeed because output-side semantic equality is not enforced before signing.
4. Transaction extraction succeeds.
5. Sent-output reconstruction stores `A` as the external recipient while the actual committed note/ciphertext still target `B`.

**Why cryptographic validity != semantic validity here:**  
The signatures and proofs commit to transaction effects, not helper metadata used for output presentation.

**Can a malicious coordinator exploit this?:**  
Yes.

**Can different participants see different transaction semantics?:**  
Yes.
- committed / recoverable recipient = `B`
- stored/displayed recipient = `A`

**Evidence from code:**  
- `create_pczt_from_proposal` sets `user_address` and `PROPRIETARY_OUTPUT_INFO`
- updater APIs allow later mutation of `user_address`
- high-level signer does not verify output-side semantic equality
- `extract_and_store_transaction_from_pczt` reconstructs `Recipient::External` from metadata
- backend unit tests demonstrate the mismatch end-to-end for both Sapling and Orchard

**Counterarguments / existing mitigations:**  
- this does not alter on-chain recipient semantics
- a downstream UI that recomputes recipient semantics from the committed output could avoid being fooled
- signer-UI exploitation is still not directly demonstrated here
- Zallet currently appears to avoid direct reachability through its normal send path because it does not yet consume PCZTs there; the demonstrated Zallet impact today is confined to a local malicious-PCZT harness.

**Bounty viability:** Medium  
**Recommended next test:** demonstrate that a user-facing signer or sent-history UI actually surfaces `A` under a production-reachable workflow, and if possible reproduce the same mismatch through the SQLite-backed persistence layer once that crate is back in sync.

### [P3] High-level shielded signer performs only opportunistic semantic verification, allowing pruned spends to remain signable

**Status:** Confirmed  
**Affected components:**  
- `pczt/src/roles/signer/mod.rs`
- upstream `sapling::pczt::Spend`
- upstream `orchard::pczt::Action` / `Spend`
- redaction flows

**Attack preconditions:**  
- malicious coordinator or intermediate role can prune or omit semantic fields
- signer uses the high-level signer role and assumes it performs “enough” semantic validation

**Bug class:** partial-validation hazard / role confusion  
**Impact:** a signer can still produce valid spend signatures even when recipient/value/randomness/witness context is absent. This weakens signer assurance and can make coordinator deception easier.  
**Root cause:** high-level signer explicitly treats missing semantic fields as non-fatal.  
**Detailed attack scenario:** coordinator prunes semantic spend context, signer still signs, and signs based on lower-level authorization semantics rather than a full semantic spend view.

**Why cryptographic validity != semantic validity here:**  
The signature proves authorization over committed spend effects, not that the signer saw the full semantic identity of the note being spent.

**Can a malicious coordinator exploit this?:**  
Yes, as a signer-deception / visibility reduction tactic.

**Can different participants see different transaction semantics?:**  
Yes.

**Evidence from code:**  
- Sapling path treats `MissingRecipient | MissingValue | MissingRandomSeed` as success
- Orchard path treats `MissingRecipient | MissingValue | MissingRho | MissingRandomSeed` as success

**Counterarguments / existing mitigations:**  
- appears intentional to support partially-pruned workflows
- not a spend forgery

**Bounty viability:** Low  
**Recommended next test:** build a multi-signer workflow where one signer receives a pruned PCZT and still signs successfully with materially less semantic visibility.

## Strongest semantic mismatch candidates

1. **External `user_address` / proprietary recipient metadata not bound to committed shielded recipient**
   - Best candidate.
   - Now has an end-to-end passing backend PoC.

2. **Post-signature reconstruction of sent outputs depends on auxiliary metadata**
   - This is the downstream consumer that turns the mismatch into a concrete exploit.

3. **Pruned spend semantics still signable**
   - Real role-separation concern.

4. **Unchecked Sapling proof-generation-key setter**
   - Still relevant as helper-overclaim, but weaker.

5. **Transparent signer `TODO` consistency gap**
   - Real omission, lower value than the shielded mismatch.

## Rejected hypotheses

### Hypothesis

PCZT randomized verification key handling allows signature forgery or wrong-key acceptance.

**Why it looked suspicious**

- randomized keys are central in Sapling and Orchard

**What disproved it**

- signer methods still require the spend-authorizing key to randomize to the stored `rk`
- external signatures must already verify against stored `rk` and sighash

### Hypothesis

Pruning lets a participant produce a final valid transaction whose cryptographic outputs differ from the transaction frozen into `sighash`.

**What disproved it**

- final extraction rebuilds `TransactionData`, recomputes sighash, and re-verifies Sapling / Orchard bundles

### Hypothesis

Verification helpers silently accept fully specified semantically inconsistent spends.

**What disproved it**

- when necessary fields are present, upstream helpers do recompute nullifier / note-commitment / randomized-key invariants

## Final triage

Would a Zcash security triager consider this:
- a real semantic authorization bug,
- merely a wallet UX issue,
- an API misuse hazard,
- or an intended trust-model assumption?

### Variant A conclusion

- Most likely triager reaction now: **real semantic authorization / presentation bug**
- Why:
  - the mismatch is no longer theoretical
  - it survives:
    - proof creation
    - signature application
    - spend finalization
    - transaction extraction
    - sent-output storage
  - the concrete demonstrated result is:
    - committed recipient `B`
    - stored/displayed recipient `A`
    - transaction accepted `yes`

The remaining downgrade pressure is only on scope of impact:
- it is clearly a semantic integrity problem,
- but still not yet shown as a signer-confirmation exploit against a deployed UI.

## Can forged metadata reach signer confirmation?

**UNCLEAR**

What is proven:
- forged metadata reaches backend sent-history reconstruction
- forged metadata is authoritative for external-recipient storage there
- signatures still verify and extraction still succeeds

What is not proven from the current codebase:
- a signer-facing confirmation screen or review API that consumes `user_address` / `PcztRecipient` as the displayed approval target

So the precise answer is:

Can a signer realistically approve recipient A while signing recipient B?

**UNCLEAR**

because the signer-facing UI component is missing from this codebase.

Can a wallet/history consumer store or display recipient A while the committed output pays B?

**YES**
