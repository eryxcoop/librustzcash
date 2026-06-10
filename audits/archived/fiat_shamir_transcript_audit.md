# Audit Findings

Audit target: Fiat-Shamir challenge derivations in prover, native verifier, and Solidity verifier paths; review of domain labels, ordering, included commitments / openings / public inputs, and whether any verifier-side randomness replaces transcript-derived challenges.

High-level result: I did not confirm a Fiat-Shamir mismatch in the local prover/native-verifier stack that exists in this repository. The Orchard proving and verification wrappers both delegate to the same Halo2 transcript implementation and challenge schedule. I did not find any Solidity verifier code in this repository, so I could not perform the requested prover/native/Solidity three-way comparison here.

## No confirmed findings in the local Orchard/Halo2 prover-native verifier path

### What is present

The only substantial Fiat-Shamir transcript path I found in scope is Orchard’s Halo2 PLONK proof path:

- prover wrapper:
  - `orchard::circuit::Proof::create` uses `Blake2bWrite`
  - calls `plonk::create_proof(...)`
- native verifier wrapper:
  - `orchard::circuit::Proof::verify` uses `Blake2bRead`
  - calls `plonk::verify_proof(...)`

The underlying transcript implementation is Halo2’s `Blake2bWrite` / `Blake2bRead`, both initialized with the same personalization:

- `b"Halo2-Transcript"`

### Challenge schedule reviewed

In Halo2’s PLONK prover/verifier, I checked the challenge derivation order and the transcript inputs around the major rounds.

Observed prover-side challenge order:

- hash verification key into transcript
- hash instance commitments
- hash advice commitments
- derive `theta`
- hash lookup permuted commitments
- derive `beta`
- derive `gamma`
- hash permutation / lookup product commitments
- hash vanishing commitments-before-`y`
- derive `y`
- hash vanishing commitments-after-`y`
- derive `x`
- hash instance/advice/fixed evaluations
- evaluate vanishing / permutation / lookup arguments
- multiopen subprotocol derives additional transcript challenges (`x_1`, `x_2`, `x_3`, `x_4`) from the same transcript

Observed verifier-side challenge order matches the same sequence:

- hash verification key into transcript
- hash instance commitments
- read advice commitments
- derive `theta`
- read lookup permuted commitments
- derive `beta`
- derive `gamma`
- read permutation / lookup product commitments
- read vanishing commitments-before-`y`
- derive `y`
- read vanishing commitments-after-`y`
- derive `x`
- read instance/advice/fixed evaluations
- continue evaluation of vanishing / permutation / lookup arguments
- multiopen verification consumes the same transcript-driven challenge flow

### Included transcript data reviewed

I checked that the following are included on both sides:

- verification key transcript representation
- external instance commitments / public-input commitments
- prover advice commitments
- lookup commitments and products
- permutation commitments and products
- vanishing commitments
- queried polynomial evaluations
- multiopening commitments / scalars

I did not find a local path where:

- prover includes a commitment/evaluation that verifier omits
- verifier reads transcript items in a different order
- public inputs are committed on one side but not the other

### Verifier-side randomness

I did not find verifier-side randomness replacing transcript-derived challenges in the local native verifier path.

The local Orchard verifier wrapper constructs:

- `SingleVerifier::new(&vk.params)`
- `Blake2bRead::init(&proof[..])`

and then calls `plonk::verify_proof(...)`.

The Fiat-Shamir challenges are derived from the transcript, not sampled randomly by the verifier wrapper.

## No Solidity verifier code present in this repository

I did not find:

- Solidity verifier contracts
- EVM calldata encoders for Halo2 / PLONK proofs
- on-chain verifier challenge derivation code
- tests comparing native verifier output against Solidity verifier behavior

Because that code is not present here, I could not generate the requested `0, 1, 2, 3+ custom gates/openings` prover/native/Solidity agreement tests within this repository.

## Out of scope / not applicable

- `zcash_proofs` mainly contains Groth16 / Sprout proving and verification, which does not expose the same Halo2-style Fiat-Shamir challenge schedule audited above.
- I did not identify any local custom Halo2 circuit family in this repo that varies the number of custom gates/openings independently in a way suitable for the requested Solidity-comparison test matrix.

## Residual risk / caution

This audit relies on code inspection of the local Orchard wrapper and Halo2 transcript schedule in the dependency source available in the build environment. I did not add new transcript-consistency tests in this repo because the requested Solidity-verifier path is absent here.
