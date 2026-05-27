# Audit Findings

Audit target: transaction builders/parsers and adjacent PCZT flows for cases where a recipient, Unified Address receiver, memo, pool selector, or output index can be swapped, dropped, or rebound without invalidating the transaction.

High-level result: I did not confirm a direct on-chain fund-redirection bug in the audited transaction builder, ZIP 321 parser, or proposal parser paths. The one reportable issue I found is in the PCZT metadata/extraction flow: mutable, non-consensus-bound recipient metadata is trusted after transaction extraction.

## Medium: PCZT output metadata can be rebound or dropped without invalidating the extracted transaction

Affected code:

- `zcash_client_backend/src/data_api/wallet.rs:1990-2184`
- `zcash_client_backend/src/data_api/wallet.rs:2257-2555`
- `pczt/src/roles/signer/mod.rs:111-198`
- `pczt/src/roles/signer/mod.rs:201-329`
- `pczt/src/sapling.rs:248-254`
- `pczt/src/orchard.rs:251-257`
- `pczt/src/transparent.rs:172-178`
- `pczt/src/roles/redactor/sapling.rs:221-268`
- `pczt/src/roles/redactor/orchard.rs:159-199`
- `pczt/src/roles/redactor/transparent.rs:243-247`

### What happens

When a proposal is converted into a PCZT, the wallet stores output-recipient metadata in two mutable places:

- `user_address`
- proprietary `zcash_client_backend:output_info`

That metadata is added by the updater in `create_pczt_from_proposal` and is not part of the extracted transaction effects. Later, `extract_and_store_transaction_from_pczt` trusts those same mutable fields to decide whether an output was:

- an external payment, and to which address
- a wallet-internal output
- an ephemeral transparent output

For shielded outputs, the extractor rebuilds the actual note from effecting data, but it still trusts the mutable metadata to classify the recipient and to recover the user-facing address recorded in wallet history.

### Why this is a problem

A malicious PCZT participant positioned after creation but before final extraction can modify or redact those metadata fields without changing the final transaction, transaction ID, or signatures.

That allows:

- rebinding an external output to a different displayed/stored Zcash address
- substituting a different Unified Address that still contains the same actual receiver used in the transaction, but carries attacker-controlled alternate receivers for other pools
- reclassifying a shielded external output as `InternalAccount`, causing the wallet to store a foreign note as if it were wallet-internal
- dropping internal-output bookkeeping by clearing recipient/value/rseed/user-address metadata so the extractor silently skips recording the sent output

The signer role documentation says signers must confirm that `user_address` contains the actual receiver, but the provided signer implementation does not enforce that check; it explicitly leaves semantic validation to the caller, and the transparent path still contains a `TODO` for input consistency checks.

### Security impact

I did not confirm a same-transaction receiver-redirection bug here: the actual on-chain recipient remains bound by the output script or shielded ciphertext/proofs.

The impact is wallet-integrity failure:

- sent-transaction history can misreport who was paid
- wallet-internal outputs can be hidden or misclassified
- balances can be corrupted if a foreign shielded note is recorded as wallet-internal
- stored recipient addresses can be poisoned for future reuse

I rate this `Medium` rather than `High` because exploitation appears to require a PCZT-based workflow with an untrusted or compromised participant between PCZT creation and final extraction, and I did not verify immediate theft in the same transaction. I would still treat it as disclosure-worthy because it breaks recipient binding at the wallet layer in a security-sensitive signing/extraction flow.

### Why no PoC is included

Per request, I am only including concrete PoCs for cases where funds can be sent to an unintended receiver. I did not confirm that for this finding in the current transaction; the issue is metadata rebinding and wallet-state corruption around an otherwise valid transaction.

## No confirmed issues in the other audited paths

- `zip321` parsing/serialization preserved payment indices as expected and rejected duplicate indexed parameters; I did not confirm a memo/recipient rebind there.
- `zcash_client_backend` proposal decoding revalidates that `payment_pools` exactly match the transaction request and that the selected pool is compatible with the recipient address, including Unified Addresses.
- The transaction builder paths I inspected did not expose a confirmed way to swap a memo, receiver, or pool selection after signing without invalidating the transaction itself.
