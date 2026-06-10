# Audit Findings

Audit target: ZIP-32 derivation and wallet use of ZIP-32-derived metadata, with a focus on:

- hardened / non-hardened boundary mistakes
- account-index overflow
- diversifier-index overflow
- transparent-child truncation
- cross-pool receiver mismatch

Reporting threshold: only cases causing aliasing, wrong account attribution, or invalid key acceptance.

High-level result: I did not confirm any issue meeting that threshold in the audited paths. The relevant code consistently rejects out-of-range account indices, rejects hardened transparent child indices, refuses transparent derivation from diversifier indices outside the `u31` range, and handles mixed-pool / mixed-account Unified Address cases conservatively by returning ambiguity rather than silently attributing the address to the wrong account.

## No confirmed findings

### Account-index overflow / hardened boundary

The base ZIP-32 `AccountId` type is restricted to 31-bit values:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zip32-0.2.1/src/lib.rs:26-59`

That same type is used at the wallet/import boundaries:

- `zcash_client_sqlite/src/wallet.rs:412-424`
- `zcash_client_sqlite/src/wallet.rs:1818-1852`

So I did not find a path where a `u32 >= 2^31` can silently wrap into a valid ZIP-32 account and alias another account.

### Diversifier-index overflow / transparent-child truncation

`DiversifierIndex` is an 11-byte value with checked conversion back down to narrower integer types:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zip32-0.2.1/src/lib.rs:190-246`

Transparent-child conversion is also explicit and lossless-only:

- `zcash_keys/src/keys.rs:92-100`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_transparent-0.7.0/src/keys.rs:98-176`

If the diversifier index has any non-zero high bytes, or if the low 32-bit value has the hardened bit set, the conversion fails rather than truncating.

The SQLite layer preserves that invariant both in decoding and schema constraints:

- `zcash_client_sqlite/src/wallet/transparent.rs:101-110`
- `zcash_client_sqlite/src/wallet/db.rs:207-216`

So I did not find a case where two distinct diversifier indices collapse onto the same transparent child index, or where a hardened / oversized child index is silently accepted as a valid transparent receiver index.

### Cross-pool receiver mismatch

Unified address generation applies per-pool derivation rules independently and fails closed when a required receiver cannot be generated:

- `zcash_keys/src/keys.rs:1598-1681`

In particular:

- transparent derivation refuses non-`u31` diversifier indices
- Sapling derivation returns an explicit invalid-diversifier error
- Orchard uses its own 11-byte diversifier index domain without reusing transparent child semantics

I did not find a case where a transparent/Sapling/Orchard receiver from one derivation path is silently rebound as another pool's receiver.

### Mixed-account / “franken” Unified Addresses

`UnifiedIncomingViewingKey::decrypt_diversifiers` intentionally returns a set of candidate diversifier indices recovered from shielded receivers:

- `zcash_keys/src/keys.rs:1756-1775`

The SQLite account-resolution logic then handles multi-account matches conservatively:

- `zcash_client_sqlite/src/wallet.rs:1255-1283`

If more than one account matches a UA algebraically, the code returns `UnifiedAddressConflict` instead of choosing one account. So while mixed-account UAs can produce multiple matches, I did not find silent wrong-account attribution.

### ZIP-32 / BIP-44 / ZIP-48 metadata extraction from PCZT

The derivation-path extraction helpers validate hardened vs non-hardened positions before returning account/scope/address fields:

- transparent:
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_transparent-0.7.0/src/pczt.rs:244-320`
- Sapling:
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/pczt.rs:320-339`
- Orchard:
  - `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/pczt.rs:316-329`

I did not find a path where a non-standard derivation path is silently reinterpreted as a valid account/scope/address triple belonging to another account.

## Notes not reported as findings

I noticed one operational edge case that does not meet the reporting threshold:

- the SQLite transparent gap-allocation code uses an end-exclusive range and therefore never preallocates the maximum child index `2^31 - 1`
  - `zcash_client_sqlite/src/wallet/transparent.rs:536-545`

That is a capacity quirk, not aliasing, wrong-account attribution, or invalid key acceptance, so I did not classify it as a finding here.

## Conclusion

I did not confirm a ZIP-32 hierarchy bug causing:

- aliasing between distinct accounts or child indices
- silent wrong-account attribution
- or acceptance of invalid ZIP-32-derived key/index data as a different valid one

The audited paths appear to enforce the relevant boundaries explicitly, and ambiguous cross-pool / cross-account cases are handled conservatively rather than resolved to the wrong account.
