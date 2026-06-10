# Audit Findings

Audit target: hash personalization strings, domain-separation tags, transcript labels, and related namespace constants used for note commitments, nullifiers, signatures, key derivation, ZIP-32, Sapling, Orchard, and transparent code.

High-level result: I did not confirm a domain-separation collision that would let one object type verify as another, or cause one protocol object to be accepted under the domain intended for a different object. The namespaces I inspected are mostly cleanly partitioned, and the few reused personalizations I found are additionally separated by distinct basepoints, message bytes, or object structure.

## No confirmed findings

I did not find a concrete case where:

- a note commitment could be reinterpreted as a nullifier or signature challenge
- a Sapling object could verify as an Orchard object, or vice versa
- a transparent digest domain collided with a shielded digest domain in a way that crossed a verification boundary
- a ZIP-32 key-derivation tag collided with a transaction, note-encryption, or signature domain

## Reviewed namespaces

### Sapling

Reviewed tags included:

- `Zcashivk`
- `Zcash_nf`
- `Zcash_PH`
- `Zcash_gd`
- `Zcash_G_`
- `Zcash_H_`
- `Zcash_cv`
- `Zcash_J_`
- `Zcash_SaplingKDF`
- `Zcash_Derive_ock`
- `ZcashIP32Sapling`
- `ZcashSaplingFVFP`
- `Zcash_SaplingInt`
- RedJubjub `Zcash_RedJubjubH`

### Orchard

Reviewed tags included:

- `z.cash:Orchard`
- `z.cash:Orchard-cv`
- `z.cash:Orchard-gd`
- `z.cash:Orchard-NoteCommit`
- `z.cash:Orchard-CommitIvk`
- `z.cash:Orchard-MerkleCRH`
- `Zcash_OrchardKDF`
- `Zcash_Orchardock`
- `ZcashIP32Orchard`
- `ZcashOrchardFVFP`
- RedPallas `Zcash_RedPallasH`
- Orchard bundle / ZIP-244 tags such as:
  - `ZTxIdOrchardHash`
  - `ZTxIdOrcActCHash`
  - `ZTxIdOrcActMHash`
  - `ZTxIdOrcActNHash`
  - `ZTxAuthOrchaHash`

### Transaction digests / transparent

Reviewed tags included:

- `ZcashSigHash` prefix family
- `ZcashPrevoutHash`
- `ZcashSequencHash`
- `ZcashOutputsHash`
- `ZcashJSplitsHash`
- `ZcashSSpendsHash`
- `ZcashSOutputHash`
- `Zcash___TxInHash`
- `ZTxTrAmountsHash`
- `ZTxTrScriptsHash`
- `ZcashTxHash_` prefix family
- `ZTxIdHeadersHash`
- `ZTxIdTranspaHash`
- `ZTxIdSaplingHash`
- `ZTxIdPrevoutHash`
- `ZTxIdSequencHash`
- `ZTxIdOutputsHash`
- `ZTxIdSSpendsHash`
- `ZTxIdSSpendCHash`
- `ZTxIdSSpendNHash`
- `ZTxIdSOutputHash`
- `ZTxIdSOutC__Hash`
- `ZTxIdSOutM__Hash`
- `ZTxIdSOutN__Hash`
- `ZTxAuthHash_` prefix family
- `ZTxAuthTransHash`
- `ZTxAuthSapliHash`

## Suspicious reuses checked and why they do not appear exploitable

### RedJubjub and RedPallas reuse the same `H*` personalization within a curve

Both spend authorization and binding signatures share:

- `Zcash_RedJubjubH` for RedJubjub
- `Zcash_RedPallasH` for RedPallas

I did not treat this as a finding because the signature domains are still separated by:

- distinct basepoints for spend-auth vs binding
- distinct verification keys
- distinct higher-level transaction binding context

So this is name reuse within a signature family, not a demonstrated cross-object verification collision.

### Orchard reuses `z.cash:Orchard` as a personalization family

I checked the use of `z.cash:Orchard` for multiple fixed-base constructions. The code distinguishes those uses by additional message bytes or dedicated labels:

- spend auth base uses the Orchard family with one input label
- nullifier base `K` uses the same family with a different input label
- value commitments, note commitments, IVK commitments, and Merkle CRH each have their own distinct strings

I did not find a place where two semantically different Orchard objects share both:

- the same personalization string
- and the same auxiliary discriminator bytes / construction shape

### ZIP-244 transaction digest subtrees

I checked the many `ZTxId*` and `ZTxAuth*` labels for collisions across transparent, Sapling, and Orchard digest trees. They appear intentionally disjoint:

- txid vs auth commitments use different prefix families
- compact / memos / noncompact components have separate labels
- transparent and shielded subtree labels are distinct

I did not find a collision that would make one serialized subtree hash valid in place of another.

## Residual risk / caution

I did not exhaustively prove collision-freedom of all dependent libraries’ transcript or PRF constructions; this was a code audit of the tag layout and call sites. The most suspicious patterns I found were reviewed and did not yield a concrete exploit path.
