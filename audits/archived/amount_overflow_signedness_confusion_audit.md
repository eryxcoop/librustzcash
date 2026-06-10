# Audit Findings

Audit target: conversions among `Zatoshis`, `ZatBalance`, Sapling / Orchard `NoteValue`, Sapling / Orchard `ValueSum`, raw `i64` / `u64`, and `BalanceError`, with a focus on values outside `MAX_MONEY`, negative balances, overflow, panics, wrapping, and meaning changes across wallet / consensus boundaries.

High-level result: I did not confirm a raw arithmetic wraparound in the core monetary types in `zcash_protocol`; those types are generally guarded. The reportable issues are at the wallet boundary, where Sapling / Orchard `NoteValue` and decrypted notes can carry arbitrary `u64` values while wallet-facing APIs later reinterpret them as `Zatoshis` and sometimes panic instead of returning an error.

## Medium: out-of-range shielded note values are accepted at wallet boundaries and later panic when converted to `Zatoshis`

Affected code:

- `components/zcash_protocol/src/value.rs:263-352`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/value.rs:4-22`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/value.rs:51-73`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/value.rs:98-118`
- `zcash_client_backend/src/decrypt.rs:89-100`
- `zcash_client_backend/src/wallet.rs:427-435`
- `zcash_client_backend/src/wallet.rs:599-647`
- `zcash_client_backend/src/data_api/ll/wallet.rs:992-1046`
- `zcash_client_memory/src/wallet_write.rs:675-699`
- `zcash_client_memory/src/wallet_write.rs:746-769`
- `zcash_client_sqlite/src/wallet/sapling.rs:50-52`
- `zcash_client_sqlite/src/wallet/sapling.rs:113-121`
- `zcash_client_sqlite/src/wallet/orchard.rs:51-53`
- `zcash_client_sqlite/src/wallet/orchard.rs:108-114`

### What happens

`Zatoshis` enforces the Zcash money range `{0..MAX_MONEY}`:

- `components/zcash_protocol/src/value.rs:263-352`

But Sapling and Orchard `NoteValue` explicitly do not enforce that smaller Zcash-specific bound:

- Sapling `NoteValue::from_raw` only enforces “unsigned 64-bit integer”:
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/value.rs:67-73`
- Orchard `NoteValue::from_raw` does the same:
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/value.rs:113-118`

That by itself is intentional. The problem is that wallet-side code accepts or reconstructs notes with those raw `u64` note values and then later converts them to `Zatoshis` with `expect(...)`.

Concrete panic sites include:

- decrypted output helpers:
  - `zcash_client_backend/src/decrypt.rs:89-100`
- generic wallet note wrapper:
  - `zcash_client_backend/src/wallet.rs:427-435`
- fee/input-view paths for received notes:
  - `zcash_client_backend/src/wallet.rs:599-647`
- sent-output storage during decrypted-transaction ingestion:
  - `zcash_client_backend/src/data_api/ll/wallet.rs:992-1046`
- memory-wallet sent-output storage:
  - `zcash_client_memory/src/wallet_write.rs:675-699`
  - `zcash_client_memory/src/wallet_write.rs:746-769`

At the same time, SQLite note reconstruction only checks that the stored note value is non-negative before rebuilding the note:

- Sapling:
  - `zcash_client_sqlite/src/wallet/sapling.rs:50-52`
  - `zcash_client_sqlite/src/wallet/sapling.rs:113-121`
- Orchard:
  - `zcash_client_sqlite/src/wallet/orchard.rs:51-53`
  - `zcash_client_sqlite/src/wallet/orchard.rs:108-114`

So a shielded note value can cross the wallet boundary as:

- a valid `u64` note value,
- but an invalid `Zatoshis`,
- and the later conversion site may panic instead of rejecting it cleanly.

### Why this matters

This is a real accepted-invalid / panic boundary issue.

The note-value types in Sapling and Orchard are broader than the Zcash money range by design, but wallet code repeatedly assumes those values were “already validated by consensus”. That assumption is false in at least two important situations:

- decrypted / scanned data can be processed before full consensus validation;
- persisted wallet state can reconstruct notes from stored components without re-establishing the `MAX_MONEY` invariant.

As a result, a note value outside `MAX_MONEY` can be accepted into wallet state and later:

- panic when displayed as a `Zatoshis` amount,
- panic during sent-output ingestion,
- or panic during spend-selection / fee-view conversion.

### Impact

I rate this `Medium` because:

- the issue is reachable from untrusted transaction-derived data at wallet boundaries;
- it can crash wallet code rather than returning a typed error;
- and it reflects a true meaning mismatch between “valid note value” and “valid Zcash amount”.

This overlaps with the previously observed oversized-note-value panic issues, but here the root cause is specifically the unsafe conversion boundary between `NoteValue(u64)` and `Zatoshis(MAX_MONEY)`.

## Low: the same out-of-range value is treated inconsistently across APIs, changing from `Result` to panic and from overflow to “negative value” corruption

Affected code:

- `zcash_client_backend/src/wallet.rs:581-647`
- `zcash_client_sqlite/src/wallet/common.rs:780-818`
- `zcash_client_sqlite/src/wallet.rs:2572-2577`

### What happens

For the same reconstructed shielded note:

- `ReceivedNote::note_value()` returns `Result<Zatoshis, BalanceError>`:
  - `zcash_client_backend/src/wallet.rs:581-590`
- but the `InputView::value()` impl for that same `ReceivedNote` panics with `expect(...)`:
  - `zcash_client_backend/src/wallet.rs:607-647`

So the same out-of-range note value is:

- recoverable error in one wallet API,
- unconditional panic in another.

There is also an error-reporting confusion in SQLite helpers:

- `zcash_client_sqlite/src/wallet/common.rs:811-814`
- `zcash_client_sqlite/src/wallet.rs:2572-2577`

These map any failure from `Zatoshis::from_nonnegative_i64(...)` to messages like:

- `"Negative received note value: ..."`

but that conversion can fail for two different reasons:

- negative value
- overflow above `MAX_MONEY`

So overflowing positive values are reported as if they were negative-value corruption.

### Why this matters

I rate this `Low` because it is primarily an API-consistency / diagnosability problem:

- callers cannot rely on a uniform error model for out-of-range note values;
- and operators / developers can be misled by underflow-only error messages that are actually hiding overflow.

I did not rate this higher because the stronger security issue is the panic / accepted-invalid boundary above.

## No confirmed wraparound in the core `zcash_protocol::value` arithmetic

I specifically checked the core monetary types for arithmetic wrap or sign confusion:

- `ZatBalance::from_i64`, `from_nonnegative_i64`, `from_u64`
- `Zatoshis::from_u64`, `from_nonnegative_i64`
- `TryFrom<ZatBalance> for Zatoshis`
- additive / subtractive ops on `ZatBalance` and `Zatoshis`

Relevant code:

- `components/zcash_protocol/src/value.rs:17-120`
- `components/zcash_protocol/src/value.rs:263-352`

I did not confirm a raw integer wraparound bug there. The core protocol types mostly fail closed and return `BalanceError::{Overflow,Underflow}` as intended.

## Conclusion

The strongest confirmed issue is not in the protocol amount types themselves, but in wallet-layer conversions that assume `NoteValue(u64)` already satisfies the narrower `Zatoshis(MAX_MONEY)` invariant. That creates a real accepted-invalid / panic boundary for oversized shielded note values, plus a smaller API-consistency problem where the same malformed value sometimes yields `Result` and sometimes panics or is mislabeled as “negative”.
