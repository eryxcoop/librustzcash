# Audit Findings

Audit target: `FromBytes` / `parse` / deserialization paths for Jubjub, Pallas, RedJubjub, Sapling, Orchard, amounts, diversifiers, nullifiers, note commitments, and signatures, with a focus on inputs accepted by safe APIs but rejected later by downstream or consensus-facing code.

High-level result: I did not find a consensus-invalid amount, nullifier, note-commitment, or Merkle-node encoding that cleanly crosses the local parse boundary and only fails much later. Most amount and commitment-tree deserializers I inspected are strict. I did find three boundary-mismatch cases where a safe, ergonomic API accepts structurally valid data and defers semantic rejection to a later layer.

## Low: Unified Address parsing is structural, so invalid Sapling/Orchard receiver bytes can be accepted first and rejected only on conversion

Affected code:

- `components/zcash_address/src/lib.rs:217-245`
- `components/zcash_address/src/kind/unified/address.rs:20-38`
- `zcash_keys/src/address.rs:42-64`
- `zcash_keys/src/encoding.rs:176-182`

### What happens

`ZcashAddress::try_from_encoded` and Unified Address decoding accept a Unified Address once its outer encoding, item ordering, and receiver lengths are valid.

For known shielded receiver typecodes, `components/zcash_address` stores:

- Sapling receiver bytes as raw `[u8; 43]`
- Orchard receiver bytes as raw `[u8; 43]`

At that stage, the receiver payload is only length-checked, not converted into a semantic `sapling::PaymentAddress` or `orchard::Address`.

Later, downstream conversion in `zcash_keys::address::UnifiedAddress::try_from` performs the stricter checks:

- `PaymentAddress::from_bytes(data)`
- `orchard::Address::from_raw_address_bytes(data)`

Those conversions can fail even though the original Unified Address already parsed successfully.

### Why this matters

This is a real acceptance/rejection mismatch at a commonly used API boundary:

- syntax-level address parsing succeeds
- transaction-construction-facing address conversion fails later

The `zcash_address` docs do explain that semantic handling is delegated downstream, so this looks intentional rather than an outright bug. I still rate it `Low` because it is easy for callers to accidentally treat successful `ZcashAddress` parsing as meaning "the contained Sapling/Orchard receivers are transaction-usable".

### Impact

This is primarily a caller-footgun and validation-boundary mismatch:

- wallet or service code can accept a user-supplied address too early
- the failure appears later, farther from the parse boundary
- error handling and audit logging can attribute the failure to the wrong stage

I did not find a path here that causes consensus-invalid receivers to be used in a transaction.

## Low: Compact Sapling/Orchard output parsers accept arbitrary ephemeral-key bytes that downstream note handling rejects

Affected code:

- `zcash_client_backend/src/proto.rs:149-157`
- `zcash_client_backend/src/proto.rs:189-197`
- `zcash_client_backend/src/proto.rs:213-221`
- `zcash_client_backend/src/proto.rs:250-258`

Dependency behavior confirmed during inspection:

- `sapling-crypto` wraps compact-output `ephemeral_key` as `EphemeralKeyBytes` before later decoding it as a Jubjub point
- `orchard` wraps compact-action `ephemeral_key` as raw bytes before later decoding it as a Pallas point

### What happens

The lightwalletd compact-format helpers validate:

- note commitment encoding
- nullifier encoding
- ciphertext lengths

But for Sapling and Orchard ephemeral keys they only validate length, then return `EphemeralKeyBytes`.

That means malformed Jubjub/Pallas ephemeral-key encodings can pass these convenience parsers and only be rejected later when a downstream note-encryption or consensus-facing component attempts to interpret them as actual curve points.

### Why this matters

This is another safe-API boundary mismatch:

- a caller receives a successfully parsed compact output/action object
- later note decryption or bundle verification rejects the same data because the ephemeral key is not a valid canonical curve-point encoding

The impact is limited because downstream code does reject the malformed values, and I did not find a case where malformed ephemeral-key bytes survive into a valid transaction or valid decrypted note.

### Impact

This is a `Low` severity robustness issue:

- it can mislead callers into treating compact-output parsing as full semantic validation
- malformed network data can survive one abstraction layer farther than expected
- failures are deferred into later decryption/verification code paths

I did not find evidence of consensus divergence from this path.

## Low: PCZT parsing accepts malformed signature/proof material structurally and defers rejection to transaction extraction

Affected code:

- `pczt/src/lib.rs:78-99`
- `pczt/src/roles/tx_extractor/mod.rs:79-127`
- `pczt/src/roles/tx_extractor/sapling.rs:9-25`
- `pczt/src/roles/tx_extractor/orchard.rs:5-29`

Dependency behavior confirmed during inspection:

- Sapling PCZT parsing accepts `spend_auth_sig` and Groth proof bytes structurally, with cryptographic validity checked later during extraction
- Orchard PCZT parsing accepts `spend_auth_sig` and proof bytes structurally, with cryptographic validity checked later during extraction

### What happens

`Pczt::parse` is a format parser, not a full semantic verifier. After decoding the envelope, later parsing and extraction steps validate some fields immediately, but not all cryptographic material.

In particular, malformed RedJubjub / RedPallas signatures and malformed proof bytes can survive parsing and only fail later in `TransactionExtractor::extract`, where Sapling and Orchard batch validation finally enforce the consensus-facing checks.

### Why this matters

This is not a consensus bypass; extraction fails. But it is still a notable mismatch:

- the PCZT can be parsed and handed across roles
- some invalid cryptographic material is only rejected at the extraction boundary

For multi-party PCZT workflows, that means malformed inputs can move farther through the pipeline than a caller may expect if they assume `parse()` means "ready except for signing".

### Impact

I rate this `Low` because:

- the final extractor does reject invalid proofs/signatures
- I did not find a way to turn this into a valid but consensus-rejected transaction
- the effect is delayed validation, not silent acceptance at the final boundary

This is still worth documenting for implementers building orchestration around PCZT roles.

## No confirmed findings in the remaining audited categories

I did not confirm a delayed-rejection issue in the local code I inspected for:

- `Zatoshis` / `ZatBalance` amount decoding
- Orchard nullifier decoding
- Sapling/Orchard extracted note commitments
- Sapling and Orchard Merkle-tree node decoding
- wallet proposal amount decoding

Those paths were generally strict at parse time and rejected invalid encodings immediately.
