# Audit Findings

Audit target: Sapling spend auth signatures, Orchard spend auth signatures, shielded binding signatures, transparent signatures, and ZIP-244-style signature hash construction.

Focus:

- binding to txid / authorizing digest inputs
- consensus branch ID
- transaction version / version group
- transparent input amounts and scripts
- nullifiers, commitments, anchors, flags, and value balances
- proof public inputs

High-level result: I did not confirm a signature-domain or sighash-binding vulnerability in the audited paths. The signature and commitment code appears to bind the expected transaction data:

- the transaction header digest commits to version, version group ID, branch ID, lock time, and expiry height;
- transparent ZIP-244 sighashes commit to prevouts, sequences, outputs, per-input prevout/value/scriptPubKey/sequence, and all input amounts/scriptPubKeys when required;
- the shielded sighash composes the header digest together with transparent, Sapling, and Orchard effect digests;
- Sapling and Orchard spend authorization signatures and binding signatures are both verified against that same shielded sighash;
- the witness/auth commitment path separately binds proofs and signatures into `auth_commitment`.

## No confirmed findings

### Header / branch / version binding

The transaction header digest includes:

- transaction version header
- version group ID
- consensus branch ID
- lock time
- expiry height

Relevant code:

- `zcash_primitives/src/transaction/txid.rs:221-247`

The final transaction digest / sighash domain is additionally personalized by branch ID:

- `zcash_primitives/src/transaction/txid.rs:371-395`

So I did not find a path where signatures are computed over a digest that omits branch ID or transaction version context.

### Transparent signature binding

ZIP-244 transparent signature hashing includes the expected components:

- `hash_type`
- prevouts digest
- amounts digest
- scripts digest
- sequence digest
- outputs digest
- per-input prevout/value/scriptPubKey/sequence digest

Relevant code:

- `zcash_primitives/src/transaction/sighash_v5.rs:45-125`

The transparent builder provides the required authorizing context:

- `input_amounts()`
- `input_scriptpubkeys()`

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_transparent-0.7.0/src/builder.rs:584-603`

And transparent signing uses those sighashes together with the actual input value and script context:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zcash_transparent-0.7.0/src/builder.rs:615-676`

I did not confirm omission of transparent amounts, scripts, or outputs from the digest in the supported signing paths.

### Shielded sighash composition

The v5 shielded sighash is formed by combining:

- `header_digest`
- transparent signature digest
- Sapling txid digest
- Orchard txid digest

Relevant code:

- `zcash_primitives/src/transaction/sighash_v5.rs:172-192`
- `zcash_primitives/src/transaction/sighash.rs:45-64`

That means shielded signatures are not just “signing the txid”; they sign a branch-personalized digest that already commits to the transaction’s effecting data across pools.

### Sapling spend auth and binding signatures

The builder loads one common shielded sighash for Sapling signatures and binding signature creation:

- `zcash_primitives/src/transaction/builder.rs:1188-1212`

Sapling signing code signs:

- spend auth signatures with randomized spend keys over `sighash`
- binding signature with `bsk.sign(..., &sighash)`

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/builder.rs:1167-1293`

Consensus verification uses the same `sighash_value`:

- spend auth signature verified against `rk` and the supplied sighash
- binding signature verified against `bvk` and the supplied sighash

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/verifier/single.rs:29-80`

The Sapling proof public inputs also bind:

- `rk`
- `cv`
- anchor
- nullifier

through the verifier input construction:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sapling-crypto-0.7.0/src/verifier.rs:29-90`

I did not confirm a missing binding for Sapling nullifiers, anchors, commitments, or value balance in the signed / verified path.

### Orchard spend auth and binding signatures

The Orchard builder mirrors the Sapling pattern:

- one common `sighash` loaded into the bundle
- each spend auth signature signs that digest
- the binding signature signs that same digest

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/builder.rs:982-1106`

Orchard batch verification uses the same provided sighash for:

- each action signature under `rk`
- the bundle binding signature under `binding_validating_key()`

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/bundle/batch.rs:39-58`

The Orchard txid/effects commitment includes:

- nullifier
- `cmx`
- `ephemeral_key`
- encrypted note chunks
- `cv_net`
- `rk`
- flags
- value balance
- anchor

Relevant code:

- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/bundle/commitments.rs:16-60`

I did not confirm a missing binding for Orchard public inputs or bundle-level value/flag context.

### Txid and auth commitment coverage

When an authorized transaction is frozen, the txid is derived from `TxIdDigester`:

- `zcash_primitives/src/transaction/mod.rs:749-805`

The separate `auth_commitment()` binds witness/authorization data:

- transparent scriptSigs
- Sapling proofs, spend auth sigs, output proofs, binding sig
- Orchard proof, spend auth sigs, binding sig

Relevant code:

- `zcash_primitives/src/transaction/mod.rs:1214-1217`
- `zcash_primitives/src/transaction/txid.rs:438-483`
- `/home/lorenzo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/orchard-0.13.0/src/bundle/commitments.rs:74-83`

So I did not find a path where witness/auth data can be swapped without changing the auth commitment.

### Vector coverage reviewed

The transaction tests check ZIP-244 / related vectors for:

- `txid`
- `auth_digest`
- shielded sighash

Relevant code:

- `zcash_primitives/src/transaction/tests.rs:232-244`
- `zcash_primitives/src/transaction/tests.rs:461-468`

## Residual notes

I did not classify the following as findings:

- `sighash_v6` currently delegates to `v5_signature_hash` with a TODO comment for fuller ZIP-246 handling:
  - `zcash_primitives/src/transaction/sighash_v6.rs:11-19`

That is an implementation note for the unstable V6 path, but I did not confirm a present binding failure in the audited behavior here.

## Conclusion

I did not confirm a signature-domain / sighash-binding vulnerability in the audited Sapling, Orchard, transparent, or ZIP-244 digest paths. The expected transaction fields and proof/public-input commitments appear to be bound where they should be, and the included vector tests exercise the resulting `txid`, `auth_digest`, and shielded sighash outputs.
