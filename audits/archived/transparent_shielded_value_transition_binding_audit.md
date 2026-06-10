# Audit Findings

Audit target: shielding, deshielding, change-output, and proposal code, with a focus on cross-pool value conservation and mismatches between transparent inputs/outputs and shielded value balances for fees, change, dust handling, or recipient classification.

High-level result: I did not confirm a value-conservation or pool-accounting bug in the audited proposal and transaction-construction paths.

The strongest invariants I verified are:

- proposal construction requires exact equality between total inputs and `payments + change + fee`;
- the builder independently recomputes the required fee from the final transaction shape and rejects any nonzero post-fee balance;
- ZIP 320 / TEX two-step proposals explicitly account for the second-step fee in the first-step ephemeral output amount;
- payment pool selection is carried from proposal to builder via `payment_pools`, and the builder revalidates that the chosen receiver exists in the requested pool.

Affected code:

- `zcash_client_backend/src/proposal.rs:429-523`
- `zcash_client_backend/src/data_api/wallet/input_selection.rs:399-689`
- `zcash_client_backend/src/data_api/wallet/input_selection.rs:758-1058`
- `zcash_client_backend/src/data_api/wallet/input_selection.rs:1077-1187`
- `zcash_client_backend/src/fees/common.rs:250-554`
- `zcash_client_backend/src/data_api/wallet.rs:921-971`
- `zcash_client_backend/src/data_api/wallet.rs:1129-1660`
- `zcash_primitives/src/transaction/builder.rs:709-837`
- `zcash_primitives/src/transaction/builder.rs:996-1032`
- `zcash_primitives/src/transaction/builder.rs:1201-1218`

## No confirmed finding: proposal accounting and builder accounting stay bound across pools

### What I checked

I traced:

- how `GreedyInputSelector` chooses payment pools and computes change/fees;
- how single-step and multi-step proposals encode payments, change, prior-step outputs, and shielding intent;
- how TEX / ZIP 320 flows split value across an ephemeral transparent output and a second-step transparent payment;
- how `create_proposed_transactions` turns proposal-side `payment_pools` and `proposed_change` into actual transparent, Sapling, and Orchard outputs;
- how the low-level `transaction::Builder` recalculates fees and enforces net zero value balance.

### What I found

`Step::from_parts(...)` enforces the core wallet-side accounting invariant:

- `transparent_input_total + shielded_input_total + prior_step_input_total`
- must equal
- `transaction_request.total() + balance.total()`

where `balance.total()` is exactly `proposed_change + fee`.

That means a proposal cannot encode:

- hidden extra fee,
- dropped change,
- unaccounted prior-step value,
- or a cross-pool value shift that is not represented either as a payment output, a change output, or fee.

The builder then independently enforces the same invariant at transaction-construction time:

- it recomputes the fee from the final bundle structure with the selected fee rule;
- it computes the aggregate transparent/Sapling/Orchard value balance;
- it rejects any positive remainder with `ChangeRequired`;
- it rejects any negative remainder with `InsufficientFunds`.

So even if proposal-side accounting drifted, the builder would fail rather than silently construct a transaction with a different fee or value transition.

I also checked the ZIP 320 / TEX split flow specifically. There, the first-step ephemeral transparent output is set to:

- `sum(TEX payments) + second-step fee`

and the code immediately recomputes the second-step `TransactionBalance` and asserts:

- `tr1_balance.total() == tr1_balance.fee_required()`

That closes the obvious class of bug where the intermediate transparent step would underfund or overfund the second transaction.

### Why this is not a finding

I did not confirm a case where:

- transparent inputs and shielded outputs disagree about the effective fee;
- change is computed for one pool but emitted into another;
- dust handling causes value to disappear outside the declared `fee_required`;
- a shielding or deshielding proposal encodes one recipient/pool split while the builder emits another;
- or a prior-step transparent output is rebound with a different value than the proposal recorded.

The builder’s independent fee recomputation is especially important here: it makes proposal/build divergence fail closed.

## Residual notes

These did not rise to reportable findings in this audit, but they are worth keeping in mind:

- `propose_shielding(...)` compares `shielding_threshold` against `balance.total()` rather than against just the net shielded change value. I did not classify this as a bug because the current API/docs can be read as thresholding the total economically useful input value selected for shielding, not the post-fee net value that lands in the shielded pool.
- Recipient-intent integrity issues for Unified Addresses and PCZT metadata do exist elsewhere, but they are already covered by `audits/unified_address_receiver_substitution_audit.md` and `audits/pczt_cryptographic_intent_binding_audit.md`. I did not find a new cross-pool value-conservation bug beyond those metadata-layer issues.
