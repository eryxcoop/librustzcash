# Audit Findings

Audit target: wallet-side decryption, compact scanning, recovery, and malformed-input handling where untrusted or consensus-unvalidated shielded data crosses into wallet state.

High-level result: several earlier reports were describing the same cluster of wallet-boundary bugs:

- decryptable shielded outputs can carry values that are locally accepted first and only later panic when converted to wallet amount types;
- `decrypt_and_store_transaction` can mutate wallet state for transactions that have not passed local consensus validation;
- compact-block scanning can surface shielded outputs as spendable without local proof/signature validation;
- malformed compact metadata and malformed compact nullifiers can panic the scanner outright.

This report consolidates the overlapping findings previously split across:

- consensus-vs-wallet validation gaps;
- scanning/decryption false positives;
- amount overflow / signedness confusion;
- panic / DoS on malformed transactions.

Affected code:

- `zcash_client_backend/src/decrypt.rs:89-145`
- `zcash_client_backend/src/data_api/wallet.rs:205-229`
- `zcash_client_backend/src/data_api/ll/wallet.rs:663-750`
- `zcash_client_backend/src/proto.rs:58-95`
- `zcash_client_backend/src/proto.rs:109-114`
- `zcash_client_backend/src/proto.rs:142-145`
- `zcash_client_backend/src/scanning/compact.rs:124-129`
- `zcash_client_backend/src/scanning/compact.rs:223-245`
- `zcash_client_backend/src/scanning/compact.rs:289-303`
- `zcash_client_backend/src/wallet.rs:427-435`
- `zcash_client_backend/src/wallet.rs:599-647`
- `zcash_client_sqlite/src/wallet/common.rs:111-120`
- `zcash_client_sqlite/src/wallet/sapling.rs:50-52`
- `zcash_client_sqlite/src/wallet/sapling.rs:113-121`
- `zcash_client_sqlite/src/wallet/orchard.rs:51-53`
- `zcash_client_sqlite/src/wallet/orchard.rs:108-114`

## Medium: decryptable shielded outputs with out-of-range note values can panic wallet code instead of being rejected cleanly

### What happens

Sapling and Orchard note plaintext parsing can locally produce note values as raw `u64`.

Later, several wallet-side paths convert those values into `Zatoshis` using `expect(...)`-style assumptions that the value has already been validated by consensus. That assumption is false in:

- `decrypt_transaction`;
- `decrypt_and_store_transaction`;
- note reconstruction and storage helpers;
- wallet recovery paths that process decryptable-but-not-yet-consensus-validated transactions.

So a decryptable output with a note value above `MAX_MONEY` can be accepted first and only later crash the wallet.

### Why this matters

This is the same underlying bug that had shown up previously as:

- a consensus-vs-wallet validation gap;
- a decryption false-positive side effect;
- an amount-conversion confusion;
- and one of the panic/DoS findings.

The root cause is one trust-boundary mistake: wallet code assumes decrypted note values already satisfy the narrower wallet money invariant.

### Impact

I rate this `Medium` because the panic is concrete and reachable from untrusted transaction data that decrypts successfully.

## Medium: `decrypt_and_store_transaction` can mutate wallet state for transactions that have never passed local consensus validation

### What happens

`decrypt_and_store_transaction` is a wallet attribution / recovery API, not a consensus verifier. It can:

- decrypt outputs;
- infer note ownership;
- persist outputs, memos, fee-related data, and spend-state information

without first proving that the transaction is locally consensus-valid.

That means invalid-but-decryptable transactions can still drive wallet state transitions before any full validation happens elsewhere.

### Why this matters

This does not mean the wallet will accept the transaction on chain. It means wallet-side state can be influenced by data that consensus would reject, which is exactly the trust-boundary mismatch behind several of the overlapping reports.

### Impact

I rate this `Medium` because this is a broad state-poisoning boundary, not just a one-off crash.

## Medium: malformed compact metadata and malformed compact nullifiers can panic light-client scanning

### What happens

The compact protobuf helpers expose convenience methods that panic on malformed lengths or out-of-range values, and the compact scanning path calls them on untrusted remote data.

Confirmed panic surfaces include:

- malformed compact block / transaction identifiers and heights;
- malformed compact Sapling / Orchard commitment fields;
- malformed compact spend/action nullifiers used during spend detection.

So a malicious or corrupted server can crash compact scanning before the wallet reaches a graceful error path.

### Why this matters

This is the same wallet-side trust-boundary family as above, but in a stricter DoS form: the scanner is directly exposed to malformed remote metadata and uses panic-prone helpers.

### Impact

I rate this `Medium` because it is a practical untrusted-input DoS against light-client scanning.

## Low: compact scanning can mark shielded outputs as spendable without local proof/signature validation

### What happens

Compact scanning decrypts shielded outputs and can promote them into wallet state without doing local proof/signature verification of the full transaction.

That behavior is expected for a light-client scanner in one sense, but it is still a real validation gap: wallet APIs can act on decrypted shielded state before local consensus checks have been performed.

### Why this matters

This is not the same as accepting an invalid transaction on chain. It is a weaker but real trust assumption:

- if the remote source is malicious or wrong,
- locally decrypted outputs can look wallet-owned and potentially spendable before full verification.

### Impact

I rate this `Low` because I did not confirm a direct theft or consensus bypass, but it is still an important trust-boundary distinction.

## No confirmed superlinear resource issue

I did not confirm a superlinear or worse resource-amplification issue in the paths I inspected.

The strongest confirmed problems in this cluster are wallet-boundary panics, state mutation on unvalidated transactions, and compact scanning trust gaps.
