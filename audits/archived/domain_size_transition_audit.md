# Audit Findings

Audit target: evaluation-domain sizing, quotient-degree bounds, coset sizes, padding, public-input inclusion, and SRS length requirements around domain-size transition boundaries.

High-level result: I did not confirm a mismatch between the Halo2/Orchard code paths present here and an independent calculation of domain-size transition behavior. The measured tiny and boundary circuits matched the expected formulas for:

- evaluation domain size `n = 2^k`
- quotient degree bound `degree - 1`
- extended-domain size `2^extended_k`
- coset factor `2^(extended_k - k)`
- unusable / padding rows `blinding_factors + 1`
- minimum rows `blinding_factors + 3`
- smallest acceptable SRS size `2^k`
- public-input inclusion in proof verification

## Method

I audited the Halo2 formulas used by the Orchard proving stack, then ran a standalone harness against `halo2_proofs 0.3.2` (the dependency version used here) using tiny circuits designed to sit exactly on, or just past, transition boundaries.

The independent model used the following formulas from the Halo2 code:

- `degree = max(permutation.required_degree(), lookup.required_degree(), gate_degree, minimum_degree_or_1)`
- `quotient_poly_degree = degree - 1`
- `extended_k = min e >= k such that 2^e >= 2^k * quotient_poly_degree`
- `blinding_factors = max(max_advice_queries_per_column, 3) + 2`
- `minimum_rows = blinding_factors + 3`
- `usable_rows = 2^k - (blinding_factors + 1)`

For boundary acceptance, I checked that:

- `keygen_vk` fails at `k - 1`
- `keygen_vk` succeeds at the smallest expected `k`

For a public-input case, I additionally checked that:

- proof generation succeeds with the correct public input
- native verification rejects an incorrect public input

## Measured boundary cases

### `empty_rows1`

- measured:
  - degree `3`
  - quotient degree `2`
  - `k = 3`, `n = 8`
  - `extended_k = 4`, extended size `16`
  - coset factor `2`
  - blinding factors `5`
  - minimum rows `8`
  - usable rows `2`
  - unusable rows `6`
- independent calculation matched exactly

### `rows3_q1_boundary`

This case used a tiny custom gate with one queried advice rotation and enough enabled rows to force an SRS-size transition through padding rather than quotient degree.

- measured:
  - degree `3`
  - quotient degree `2`
  - smallest valid `k = 4`, `n = 16`
  - `extended_k = 5`, extended size `32`
  - coset factor `2`
  - blinding factors `5`
  - minimum rows `8`
  - required row capacity `>= 9`
- independent calculation matched exactly

### `deg3_q2`

- measured:
  - degree `3`
  - quotient degree `2`
  - `k = 3`, `n = 8`
  - `extended_k = 4`, extended size `16`
  - coset factor `2`
- independent calculation matched exactly

### `deg4_q3`

This is the first quotient-degree transition where the extended domain grows by a factor of `4` instead of `2`.

- measured:
  - degree `4`
  - quotient degree `3`
  - `k = 3`, `n = 8`
  - `extended_k = 5`, extended size `32`
  - coset factor `4`
  - blinding factors `5`
  - minimum rows `8`
- independent calculation matched exactly

### `deg5_q4`

This case crosses both:

- the quotient-degree transition to `degree = 5`
- and the blinding-factor transition caused by `4` distinct advice queries in one column

- measured:
  - degree `5`
  - quotient degree `4`
  - smallest valid `k = 4`, `n = 16`
  - `extended_k = 6`, extended size `64`
  - coset factor `4`
  - blinding factors `6`
  - minimum rows `9`
  - usable rows `9`
  - unusable rows `7`
- independent calculation matched exactly

### `public_input_rows1`

- measured:
  - degree `3`
  - quotient degree `2`
  - `k = 3`, `n = 8`
  - `extended_k = 4`, extended size `16`
  - coset factor `2`
- proof verification behavior:
  - correct public input verified successfully
  - incorrect public input was rejected

This confirms that instance commitments / public-input inclusion behave as expected for the local native verifier path.

## Orchard circuit sanity check

The checked-in Orchard pinned verification-key description records:

- `k = 11`
- `extended_k = 14`
- `num_instance_columns = 1`

I did not find anything inconsistent with the generic Halo2 formulas above. This is the expected shape for a nontrivial circuit with a larger quotient bound than the tiny test cases.

## No confirmed findings

I did not confirm:

- an off-by-one domain-size transition bug
- incorrect quotient-degree accounting
- wrong extended-domain / coset sizing
- incorrect padding / unusable-row accounting
- omission of public-input commitments from the native proof path
- an SRS-length acceptance bug at the tested boundaries

## Notes

- I did not add a permanent test harness to this repository; the boundary cases were executed in a standalone local harness against the exact Halo2 dependency version in use.
- I did not compare against a separate Solidity verifier here because this repository does not contain one.
