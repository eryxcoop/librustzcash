# Audit Findings

Audit target: panic / DoS on malformed transactions, compact blocks, and wallet-facing recovery APIs. Scope included public parsing, transaction / proof verification, compact block scanning, and wallet recovery. I report only confirmed panics or clearly attacker-triggerable excessive-resource issues reachable from untrusted input.

High-level result: I confirmed three panic-based DoS issues reachable from untrusted input. I did not confirm a superlinear or worse resource-amplification issue in the code paths I inspected.

## Medium: malformed compact block / transaction metadata can panic light-client scanning

Affected code:

- `zcash_client_backend/src/proto.rs:58-95`
- `zcash_client_backend/src/proto.rs:109-114`
- `zcash_client_backend/src/proto.rs:142-145`
- `zcash_client_backend/src/scanning/compact.rs:124-129`
- `zcash_client_backend/src/scanning/compact.rs:223-245`
- `zcash_client_backend/src/scanning/compact.rs:289-303`

### What happens

Several `zcash_client_backend::proto` convenience methods on compact protobuf types panic on malformed field lengths or out-of-range numeric values:

- `CompactBlock::hash()` panics if `hash` is not 32 bytes when no header is present.
- `CompactBlock::prev_hash()` panics if `prev_hash` is not 32 bytes when no header is present.
- `CompactBlock::height()` panics if the protobuf height is not representable as `u32`.
- `CompactTx::txid()` panics if `txid` is not 32 bytes.
- `CompactSaplingOutput::cmu()` panics if `cmu` is not 32 bytes.

The compact scanning path calls these methods directly while processing untrusted compact block data from a server.

There is also an additional panic in the same path:

- `TxIndex::try_from(tx.index).expect(...)`

So a malformed compact transaction index can crash scanning even before any graceful `ScanError` path is taken.

### Why this matters

These are not mere internal assertions. They sit on the boundary where a wallet consumes remote compact block data. A malicious or corrupted server can therefore crash the scanner with malformed lengths or oversized numeric values.

### Impact

I rate this `Medium` because it is a straightforward remote DoS against light-client scanning.

## Medium: malformed compact spend/action nullifiers can panic scan-time spend detection

Affected code:

- `zcash_client_backend/src/proto.rs:200-205`
- `zcash_client_backend/src/proto.rs:239-247`
- `zcash_client_backend/src/scanning/compact.rs:246-253`
- `zcash_client_backend/src/scanning/compact.rs:261-268`

### What happens

The compact scanning path uses:

- `spend.nf().expect("Could not deserialize nullifier ...")`

for both Sapling spends and Orchard actions.

`nf()` returns a `Result`, but malformed nullifier encodings from untrusted compact block data are escalated to `expect(...)` instead of being converted into a recoverable `ScanError`.

### Why this matters

This gives an attacker controlling the compact block source another clean crash primitive:

- malformed nullifier bytes
- panic during spend detection
- wallet scan aborts

### Impact

I rate this `Medium` because it is another direct remote DoS in a core wallet ingestion path.

## Medium: `decrypt_and_store_transaction` can panic on decryptable shielded note values outside the wallet amount range

Affected code:

- `zcash_client_backend/src/decrypt.rs:89-100`
- `zcash_client_backend/src/data_api/wallet.rs:205-229`

Dependency behavior involved:

- Sapling note values are locally parsed as arbitrary `u64`
- Orchard note values are locally parsed as arbitrary `u64`

### What happens

`decrypt_and_store_transaction` accepts an arbitrary `Transaction`, decrypts it, and then wallet-side code converts decrypted shielded note values to `Zatoshis` using:

- `Zatoshis::from_u64(...).expect(...)`

So an invalid but decryptable transaction carrying a note value above the wallet’s accepted monetary range can panic wallet recovery instead of returning an error.

### Why this matters

This is reachable from untrusted transaction input if a caller uses the public recovery API on data that has not already been consensus-validated elsewhere.

### Impact

I rate this `Medium` because it is a concrete crash surface on a high-level wallet API.

## No confirmed superlinear resource issue

I did not confirm a superlinear or worse resource-consumption issue in the audited paths.

In particular:

- batch decryption is expensive but appeared structurally linear in `#outputs * #ivks`
- compact scanning allocates and iterates over attacker-controlled vectors, but I did not identify a stronger-than-linear amplification pattern from the local code alone
