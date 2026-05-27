# Audit Findings

Audit target: Sapling and Orchard diversifier search and default-address generation, including Unified Address generation over mixed receiver requirements.

Focus:

- invalid diversifier indices
- skipped indices
- wraparound
- mixed receiver requirements
- account / pool / receiver-policy consistency of generated addresses

High-level result: I did not confirm a case where `find_address` / `default_address` returns an address inconsistent with the requested account, requested receiver set, or receiver policy. The audited code generally fails closed:

- Sapling invalid diversifiers are skipped only where the API explicitly promises search.
- Transparent invalid child-index projections are rejected rather than skipped into a different address.
- Orchard address derivation is total over its 11-byte diversifier-index domain and does not share transparent/Sapling validity conditions.
- Mixed-account Unified Addresses are treated as ambiguous by wallet account lookup instead of being silently attributed to the wrong account.

## No confirmed findings

### Unified address search does not silently relax receiver requirements

The central Unified Address generation path is `UnifiedIncomingViewingKey::address` and `find_address`:

- `zcash_keys/src/keys.rs:1594-1681`
- `zcash_keys/src/keys.rs:1713-1747`

Important invariants I checked:

- Required Orchard / Sapling / transparent receivers must either be derivable or the call errors.
- Transparent receiver generation does not silently fall back to a different diversifier index when the requested `DiversifierIndex` cannot be projected into a non-hardened child index.
- `find_address` only retries on `InvalidSaplingDiversifierIndex`, which matches Sapling's “search the diversifier space for a valid diversifier” semantics.
- Other errors, including unsupported required receiver types, missing keys, or invalid transparent child index projections, are returned immediately.

That means I did not find a case where a request like “require transparent + shielded receivers” returns some different address that merely satisfies a subset of the request.

### Transparent child-index projection is explicit and non-lossy

Transparent receivers reuse the lower 31-bit child-index space, but the projection from `DiversifierIndex` is guarded:

- `zcash_keys/src/keys.rs:92-100`
- `zcash_keys/src/keys.rs:1602-1610`
- `zcash_keys/src/keys.rs:1669-1680`

The rules are:

- the upper 7 bytes of the 11-byte diversifier index must be zero;
- the low 4-byte value must be a valid `NonHardenedChildIndex`;
- otherwise the projection fails with `InvalidTransparentChildIndex`.

So I did not find truncation or wraparound that would cause:

- two distinct diversifier indices to map to the same transparent receiver in a “successful” path, or
- an invalid requested transparent receiver to be replaced with a different valid one.

### Sapling search semantics appear consistent with the API contract

Sapling key code explicitly models “search starting at index `j` until a valid diversifier exists”:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/zip32.rs:47-64`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/zip32.rs:757-775`
- `zcash_keys/src/keys.rs:1713-1737`

The higher-level unified search preserves that behavior by retrying only on Sapling-invalid indices and otherwise preserving the original request semantics.

I did not find:

- skipped-index behavior that crosses accounts,
- wraparound that returns a valid address after overflow,
- or a way for a Sapling-invalid index to produce a Unified Address inconsistent with the original UIVK / account.

### Orchard diversifier mapping is total and account-local

Orchard uses a reversible diversifier-key mapping over the full 11-byte diversifier-index space:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/keys.rs:484-507`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/keys.rs:656-660`

Unlike Sapling, there is no “invalid diversifier” concept requiring search retries at this layer. So I did not find a skipped-index or invalid-index condition that could produce an Orchard address inconsistent with the requested account or policy.

### Mixed-account / mixed-receiver UAs do not silently resolve to the wrong account

One subtle edge case is a “franken-UA” combining receivers attributable to different accounts or different diversifier indices. The key-level helper intentionally exposes ambiguity:

- `zcash_keys/src/keys.rs:1756-1775`

and the SQLite wallet lookup treats multi-account matches conservatively:

- `zcash_client_sqlite/src/wallet.rs:1255-1283`

If more than one account algebraically matches a Unified Address, lookup returns `UnifiedAddressConflict` instead of silently choosing one account. So I did not confirm wrong-account attribution from mixed receiver sets.

## Notes not reported as findings

I noticed one behavior that is worth documenting but does not qualify as a finding:

- unified `find_address` only advances past Sapling-invalid indices; it does not “search around” invalid transparent projections or unsupported required receiver combinations.

This is not a security issue; it is the mechanism that preserves receiver-policy correctness. It means callers requesting transparent receivers at a diversifier index outside the transparent `u31` space will get an error instead of the next later valid mixed-pool address.

## Conclusion

I did not confirm a diversifier-search invariant break that causes:

- an address from the wrong account,
- an address with the wrong pool set,
- or an address that violates the requested receiver policy.

The audited paths appear to preserve those invariants by failing closed on transparent/policy mismatches and only performing index search in the Sapling-specific invalid-diversifier case where that behavior is explicitly intended.
