# Audit Findings

Audit target: ZIP-32 key derivation, spending/viewing key derivation, diversifier generation, default address generation, IVK/OVK handling, and account/index boundary values.

High-level result: I did not confirm a collision, invalid-but-accepted key, or receiver-mismatch vulnerability in the key-derivation and diversifier code I inspected. The derivation and address-generation paths appear to reject the main boundary failures explicitly, and the wallet-side lookup logic handles mixed-account Unified Addresses conservatively.

## No confirmed findings

I did not find a concrete case where:

- two distinct ZIP-32 account / diversifier derivation paths collide into the same accepted wallet key state unexpectedly
- invalid UFVK / UIVK / IVK / OVK material is accepted and then used as if valid
- default address generation returns a receiver set inconsistent with the originating key material
- transparent child-index handling silently truncates or aliases nonzero high diversifier bytes into a different accepted receiver
- mixed-account Unified Address receiver sets are silently attributed to the wrong wallet account when more than one wallet account matches

## Key edge cases reviewed

### ZIP-32 account index boundaries

Reviewed code showed:

- automatic account creation advances from the current maximum ZIP-32 account index and returns `Zip32AccountIndexOutOfRange` on overflow
- explicit HD account import derives a `UnifiedSpendingKey` from the caller-provided `zip32::AccountId`, so out-of-range values are blocked by the typed API boundary

I did not confirm account-index wraparound or aliasing.

### Transparent child-index boundary handling

Transparent receiver derivation only accepts a diversifier index whose upper 7 bytes are zero, because the transparent path maps through a 32-bit non-hardened child index.

The important behavior is:

- invalid transparent child indices are rejected explicitly with `InvalidTransparentChildIndex`
- `get_address_for_index` maps that case to `Ok(None)` rather than manufacturing a mismatched address
- default-address generation does not start from arbitrary high diversifier indices when a transparent receiver is required

I did not find silent truncation or collision from this path.

### Sapling diversifier invalidity

Sapling address generation correctly distinguishes:

- "try exactly this index" via `address(...)`
- "search forward for a valid index" via `find_address(...)` / `default_address(...)`

Invalid Sapling diversifier indices are surfaced as `InvalidSaplingDiversifierIndex`, and the search path advances only in that case.

I did not find a case where an invalid Sapling diversifier was accepted as a valid receiver.

### UFVK / UIVK validity checks

The UFVK construction / parse path performs meaningful validation:

- parsed Orchard / Sapling / transparent components are semantically decoded, not just length-checked
- UFVK construction verifies transparent external IVK derivation succeeds before accepting the key

I did not find an invalid-but-accepted viewing key state in the local code I traced.

## Suspicious semantics checked

### `decrypt_diversifiers` is not a uniqueness proof

The strongest suspicious behavior I found was not a confirmed bug, but an important semantic property:

- a "frankenstein" Unified Address can be attributable to more than one diversifier index for a single key, or to different keys for different receivers
- `decrypt_diversifiers` therefore returns a set of candidate diversifier indices rather than a unique answer

This appears intentional, and wallet lookup code handles the dangerous case conservatively:

- if multiple wallet accounts match a Unified Address’s shielded receivers, the lookup returns `UnifiedAddressConflict`

So I did not treat this as a finding.

### Mixed receiver-account lookup semantics

I also reviewed the wallet-side `find_account_for_address` behavior for mixed-receiver Unified Addresses.

The code intentionally allows a UA to resolve to an account if at least one shielded receiver is attributable to that account and no other wallet account also matches. Cross-account mixes are rejected with `UnifiedAddressConflict`.

That is a semantic design choice, but I did not find evidence that it causes silent account misattribution among wallet-controlled accounts.

## Residual risk / caution

This pass was a focused edge-case audit and boundary review. I also ran a small amount of targeted test execution, but I did not add a new fuzz harness or run long randomized campaigns in this turn.
