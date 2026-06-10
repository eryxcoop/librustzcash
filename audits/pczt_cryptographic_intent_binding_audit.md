# Audit Findings

Audit target: PCZT fields that represent recipient or output intent across proposal creation, PCZT creation, signing, extraction, sender recovery, Unified Address handling, and wallet storage.

High-level result: several earlier reports were describing the same underlying `Medium` issue: output-side PCZT metadata is only wallet-bound, not consensus-bound or signature-bound, yet the wallet still trusts it after extraction to decide recipient, value, account, and user-facing address intent.

This single issue has multiple manifestations:

- generic recipient metadata can be rebound or dropped without changing the extracted transaction;
- full Unified Address metadata can be substituted, added to, or pruned after pool selection, as long as the actually used receiver for the selected pool stays the same;
- sent-output extraction can report the wrong recipient, wrong value, or wrong account because sender recovery is used only for memo recovery while recipient/value/account are taken from mutable metadata first.

Affected code:

- `pczt/src/lib.rs:117-181`
- `pczt/src/roles/io_finalizer/mod.rs:43-74`
- `pczt/src/roles/tx_extractor/mod.rs:70-121`
- `pczt/src/roles/verifier/mod.rs:1-31`
- `pczt/src/roles/verifier/sapling.rs:1-37`
- `pczt/src/roles/verifier/orchard.rs:1-37`
- `pczt/src/roles/verifier/transparent.rs:1-41`
- `zcash_client_backend/src/data_api/wallet.rs:1990-2560`
- `zcash_client_backend/src/data_api/wallet/input_selection.rs:453-515`
- `pczt/src/roles/signer/mod.rs:111-329`
- `pczt/src/roles/redactor/sapling.rs:221-268`
- `pczt/src/roles/redactor/orchard.rs:159-199`
- `pczt/src/roles/redactor/transparent.rs:243-247`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt/tx_extractor.rs:69-90`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/tx_extractor.rs:69-80`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt/verify.rs:146-157`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt/verify.rs:128-143`

## Medium: output-side PCZT intent fields remain mutable after signing, but wallet extraction still trusts them

### What happens

The extracted transaction only depends on effecting fields such as:

- version / branch / locktime / expiry;
- transparent prevouts, values, scripts, and final authorizing data;
- Sapling / Orchard ciphertext-bearing output fields, commitments, nullifiers, proofs, and signatures.

By contrast, output-intent metadata such as:

- `user_address`;
- proprietary `zcash_client_backend:output_info`;
- reconstructed shielded note fields used only for wallet display;
- and full Unified Address wrappers around the actually used receiver

is not committed into the extracted transaction and is not protected by signatures.

Even so, after extraction the wallet still uses that mutable metadata to:

- classify outputs as external vs internal vs ephemeral;
- decide which user-facing address to record in history;
- reconstruct recipient / value / account attribution for sent shielded outputs.

For Unified Address payments, proposal creation first chooses one concrete receiver pool. After that point, the transaction is bound only to that receiver. The rest of the original UA is no longer transaction-bound, but PCZT metadata can still carry and later mutate the full UA string.

For shielded sent-output extraction, the wallet can rebuild a note-like structure from mutable metadata and then use sender recovery only for memo recovery. Recipient / value / account attribution is therefore taken from metadata before any final note-commitment rebinding is enforced.

### Why this is the same bug across the earlier reports

The previously separate reports were all different views of the same trust-boundary failure:

- `recipient_binding_audit`: generic recipient metadata rebinding;
- `unified_address_receiver_substitution_audit`: same issue specialized to full UAs;
- `ovk_sender_recovery_audit`: same issue specialized to sender-visible recipient/value/account attribution for shielded outputs;
- the older version of this report: the generalized PCZT field classification showing that the fields were only wallet-bound.

The transaction itself is not changed in these cases. What changes is the wallet’s post-extraction interpretation of recipient intent.

### Why this matters

This is not an on-chain fund-redirection bug, but it is a real integrity problem for wallet history and signing intent:

- the wallet can record a different recipient than the one the user originally intended;
- it can record a different full Unified Address even when the actual paid receiver for the selected pool is unchanged;
- it can report wrong sent-output value / account attribution for shielded outputs reconstructed from mutable PCZT metadata.

Because these metadata channels survive after signing and extraction, downstream consumers can mistake them for tx-bound truth when they are not.

### Impact

I rate this `Medium` because:

- extracted transactions and txids are unchanged, so this is not a consensus or theft issue;
- but wallet-visible recipient intent and sent-output attribution can be materially wrong after signing;
- and the same root cause affects multiple wallet features, not just one narrow code path.

## Field classification summary

- `consensus-bound`: effecting transaction fields that change the extracted tx, txid, proofs, or signatures.
- `signature-bound`: helper fields whose incorrect mutation is detected before a valid authorized tx can be extracted.
- `wallet-bound`: fields not protected by consensus or signatures, but still trusted later by wallet extraction or storage.
- `advisory`: helper metadata that should not be trusted as recipient truth after extraction.

The reportable problem is specifically that several output-side intent fields currently live in the `wallet-bound` bucket but are still treated as if they were stronger than that.

## No confirmed same-transaction fund-redirection bug

I did not confirm a case where mutating these PCZT fields redirects the current transaction’s funds on chain to a different receiver while preserving validity.

The confirmed issue is post-signing intent integrity, not direct theft.
