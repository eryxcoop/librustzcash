# Audit Findings

Audit target: Unified Address handling across parsing, proposal creation, transaction building, PCZT creation, signing, extraction, and storage, with a focus on whether one receiver can be substituted, omitted, or added without being detected by the layer that records recipient intent.

High-level result: I did not confirm an on-chain fund-redirection bug in the direct transaction-building path. The confirmed issue is `Medium`: once proposal creation chooses a specific pool/receiver from a Unified Address, the rest of the UA is no longer transaction-bound, but later PCZT extraction and wallet storage still trust mutable full-UA metadata to record recipient intent. That allows receiver addition / omission / substitution within the displayed/stored UA without changing the extracted signed transaction, as long as the actually used receiver for the selected pool stays the same.

## Medium: full Unified Address metadata can be rebound after pool selection, and wallet history will trust the modified UA

Affected code:

- `zcash_client_backend/src/data_api/wallet/input_selection.rs:453-515`
- `zcash_client_backend/src/data_api/wallet.rs:1990-2195`
- `zcash_client_backend/src/data_api/wallet.rs:2250-2560`
- `pczt/src/roles/signer/mod.rs:111-198`
- `pczt/src/roles/signer/mod.rs:201-329`
- `pczt/src/roles/redactor/sapling.rs:265-268`
- `pczt/src/roles/redactor/orchard.rs:196-199`
- `pczt/src/roles/redactor/transparent.rs:244-247`
- `zcash_client_sqlite/src/wallet.rs:4650-4733`

### What happens

For Unified Address payments, proposal creation deterministically chooses a concrete output pool:

- Orchard if present and supported
- otherwise Sapling if present and supported
- otherwise transparent if present

After that choice, the transaction being built is only bound to the receiver for the selected pool. The other receivers that may be present in the original UA do not affect the final transaction effects.

Later, `create_pczt_from_proposal` stores recipient-intent metadata in mutable PCZT fields:

- `user_address`
- proprietary `zcash_client_backend:output_info`

Those fields can carry the full external Unified Address, not just the receiver actually used in the transaction.

When `extract_and_store_transaction_from_pczt` runs, it:

- extracts the actual transaction from tx-bound effect fields only;
- reconstructs `SentTransactionOutput` recipient/account display data from the mutable PCZT metadata;
- and stores that data into wallet history / `sent_notes`.

The signer role does not bind or verify this metadata. Its documentation explicitly leaves semantic validity checks to the caller, and the provided implementation does not check that `user_address` is the exact intended UA or that its receiver set matches what was originally proposed.

### Why this matters

An attacker or buggy participant who can mutate the PCZT after proposal creation but before final extraction can change the full UA metadata without changing the actual transaction, as long as the selected receiver for the chosen pool remains the same.

Examples:

- substitute the original UA with another UA that has the same Sapling receiver but different Orchard or transparent receivers;
- drop non-selected receivers from the UA;
- add extra receivers to the UA;
- rebind a shielded output from one externally provided UA to a different externally provided UA that contains the same actually used receiver.

Because the actual transaction only commits to the selected receiver, these changes do not invalidate:

- transaction extraction
- signatures
- txid

But the wallet layer that records recipient intent will store the modified UA as if it were the original user-facing recipient.

### Impact

This is a recipient-intent integrity issue, not an on-chain theft issue.

The impact includes:

- sent transaction history can show the wrong Unified Address;
- the stored UA can omit receivers the user originally intended to share;
- the stored UA can add attacker-chosen receivers the user never approved;
- later wallet/UI logic consuming `to_address` can be misled about the exact receiver set the payment was intended for.

I rate this `Medium` because:

- the transaction itself is still bound to the correct selected receiver for the paid pool;
- but the layer that records recipient intent can be silently rewritten for the full UA, which is exactly the boundary this audit asked about.

## Why this is specifically a UA receiver-substitution issue

This is not just generic mutable metadata.

The distinguishing property of Unified Addresses is that:

- multiple receivers coexist inside one user-facing address;
- transaction construction may only use one of them;
- and the non-selected receivers are semantically meaningful to the user even though they are not committed into the transaction.

That makes UA handling especially vulnerable to “same paid receiver, different displayed UA” substitution if the system records the whole UA separately from the tx-bound receiver.

## No confirmed finding in direct proposal parsing / builder pool validation

I did not confirm a substitution bug in the earlier layers that choose the pool:

- proposal creation picks the pool from the actual address object provided in the request;
- unsupported UA combinations fail rather than silently degrading into a different pool choice once the chosen receiver is unavailable.

So the strongest confirmed issue is not in choosing which receiver to use, but in later recording the full UA without rebinding that recorded intent to the selected receiver and original proposal metadata.

## No confirmed same-transaction redirection bug

I did not confirm a case where this UA receiver-substitution issue causes funds in the current transaction to be sent to an unintended receiver.

The actual on-chain recipient for the selected pool remains bound by:

- the transparent script, or
- the shielded ciphertext / note commitment / proof material.

The confirmed issue is that the wallet can later record a different full Unified Address than the one originally intended, even though the concrete paid receiver stays the same.
