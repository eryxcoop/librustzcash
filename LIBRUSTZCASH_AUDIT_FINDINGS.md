# librustzcash cryptographic audit findings

## Executive summary

- Overall risk posture: I did not confirm a high-value cryptographic break in the audited `librustzcash` surfaces. I did not find a signature forgery, spend authorization bypass, subgroup-mismatch acceptance bug, note commitment alias, or confirmed wallet/consensus invariant split in the RedJubjub/Jubjub/Sapling/Orchard paths reviewed.
- Best bounty candidates: the strongest confirmed issue remains the malformed-input panic surface in compact-block scanning. After a second-pass reachability review, this now looks like a realistic remote wallet DoS against lightwallet consumers that trust a malicious or compromised `lightwalletd`, but still not like a strong ZCG payout candidate unless it is demonstrated against a deployed wallet stack.
- Areas audited:
- `zcash_primitives` Sapling and Orchard transaction component parsing
- `zcash_client_backend` compact-block scanning, protobuf parsing helpers, note decryption entrypoints
- `zcash_client_sqlite` Sapling note rehydration from stored rows
- `zcash_keys` unified address / viewing key parsing boundaries
- upstream dependency behavior traced where necessary in local sources for `sapling-crypto`, `redjubjub`, `reddsa`, and `orchard`
- audit lanes applied from local skill pack:
- `crypto-audit-orchestrator`
- `subgroup-cofactor-audit`
- `reddsa-jubjub-audit`
- `hash-to-curve-transcript-audit`
- `js-crypto-boundary-audit`
- `bug-bounty-report-triage`
- Areas not fully audited:
- I did not do a full external duplicate/advisory sweep beyond local code, comments, docs, and changelogs.
- I did not reproduce the crash behavior end-to-end in a deployed wallet app such as Zashi.
- I did not deeply audit external Orchard internals outside the local call sites in this repo.
- The Decaf/Ristretto/Banderwagon quotient-group lane was largely not applicable to this target.

After the reachability pass, the main downgrade is not technical but programmatic: this finding is more real-world reachable than a purely local parser footgun, yet still weak under ZCG because it affects light-wallet availability rather than core Zcash tenets, and it depends on a lightwalletd trust-boundary failure rather than a direct consensus or funds invariant break.

## Prioritized findings

### [P3] Compact-block scanning previously panicked on malformed txid and spend nullifier fields

**Status:** Confirmed, patched locally in this audit branch  
**Affected crates/modules:** `zcash_client_backend`, `zcash_client_sqlite` light-client scan path  
**Relevant files/functions:**  
- [`zcash_client_backend/src/scanning/compact.rs`](/Users/lorenzord/Desktop/zk/librustzcash/zcash_client_backend/src/scanning/compact.rs)
  - `scan_block_with_runners`
- [`zcash_client_backend/src/proto.rs`](/Users/lorenzord/Desktop/zk/librustzcash/zcash_client_backend/src/proto.rs)
  - `CompactTx::txid`
  - `CompactSaplingSpend::nf`
  - `CompactOrchardAction::nf`
- [`zcash_client_backend/src/scanning.rs`](/Users/lorenzord/Desktop/zk/librustzcash/zcash_client_backend/src/scanning.rs)
  - `ScanError`
  - `find_spent`
**Bug class:** malformed-input parsing panic / availability bug at a remote protocol boundary  
**Impact:** Before the hardening changes in this branch, a semantically malformed compact block could crash a wallet process instead of returning a structured scan error. Practical impact is wallet DoS or sync interruption for clients ingesting compact blocks from a malicious or compromised `lightwalletd`, or from a corrupted local compact-block cache populated by such a server.  
**Why this matters:** The scan path already mapped malformed compact outputs to `ScanError::EncodingInvalid`, so callers reasonably expected malformed compact data to fail closed, not panic. Several compact-block helper paths were inconsistent with that contract, and those helpers sit on a real remote ingestion path used by deployed light-wallet stacks.  
**Root cause:** `scan_block_with_runners` used `spend.nf().expect(...)` for Sapling and Orchard spend/action nullifiers, eagerly called panicking convenience helpers like `CompactTx::txid()`, and used `expect(...)` for oversized `tx.index`. These sat behind a public scan API that otherwise returned `Result<_, ScanError>`.  
**Attack sketch:** A malicious or compromised compact-block source supplies a transaction with either:
- a Sapling spend `nf` that is not a 32-byte nullifier encoding,
- an Orchard action nullifier that is not a valid 32-byte field encoding, or
- a `txid` whose byte length is not 32,
- or an oversized `tx.index`.

When the wallet scans the block, it reaches either `spend.nf().expect(...)`, `CompactTx::txid()`, or `TxIndex::try_from(...).expect(...)` and panics. In the default release profile of this workspace, `panic = 'abort'`, so the likely outcome is process termination rather than a recoverable unwind.

**Reproducer / test idea:**  
- Minimal PoC command used during audit:
```sh
cargo run --manifest-path /tmp/librustzcash_audit_poc/Cargo.toml
```
- Observed result before local patch:
```text
thread 'main' ... panicked at ... zcash_client_backend/src/scanning/compact.rs:250:28:
Could not deserialize nullifier for spend from protobuf representation.: ()
thread 'main' ... panicked at ... zcash_client_backend/src/proto.rs:113:20:
copy_from_slice: source slice length (31) does not match destination slice length (32)
malformed_nf: panic
malformed_txid: panic
```
- Regression tests now added in-repo:
- `scan_block_rejects_malformed_txid_without_panic`
- `scan_block_rejects_out_of_range_tx_index_without_panic`
- `scan_block_rejects_malformed_sapling_spend_nullifier_without_panic`
- `scan_block_rejects_malformed_orchard_action_nullifier_without_panic`
- helper boundary tests in `zcash_client_backend::proto`:
- `compact_tx_parse_txid_rejects_short_input`
- `compact_sapling_spend_nf_rejects_short_input`
- `compact_sapling_output_cmu_rejects_short_input`

**Evidence from code:**  
- Before this patch set, `scan_block_with_runners` used `expect(...)` directly on spend nullifier parsing for Sapling and Orchard in [`zcash_client_backend/src/scanning/compact.rs`](/Users/lorenzord/Desktop/zk/librustzcash/zcash_client_backend/src/scanning/compact.rs).
- The same function eagerly called `tx.txid()` and `TxIndex::try_from(tx.index).expect(...)`.
- By contrast, malformed outputs were already converted into `ScanError::EncodingInvalid` in the same file.
- `CompactTx::txid()` in [`zcash_client_backend/src/proto.rs`](/Users/lorenzord/Desktop/zk/librustzcash/zcash_client_backend/src/proto.rs) blindly `copy_from_slice`d `self.txid`.

**Existing mitigations or counterarguments:**  
- This is not a signature, subgroup, or consensus-validity bypass.
- Some `proto.rs` helpers already documented panic-on-malformed-input behavior, so a triager may treat part of this as an internal/trusted-data assumption.
- That said, the public `scan_block` path already exposed a structured `Result<_, ScanError>` contract, and malformed outputs were already handled gracefully, so the nullifier / txid / tx-index cases were inconsistent safe-path boundaries.
- Programs that trust their `lightwalletd` or local cache for integrity may downgrade this further to hardening-only.

**Bounty viability:** Low  
**Recommended next step:** The obvious engineering next step was to make these boundaries fail with `ScanError` instead of panicking. That is what was implemented locally in this audit branch.

## Reachability analysis

- In `zcash_client_backend`, compact blocks are fetched directly from remote `lightwalletd` over the gRPC `GetBlockRange` stream in [`zcash_client_backend/src/sync.rs`](/Users/lorenzord/Desktop/zk/librustzcash/zcash_client_backend/src/sync.rs).
- The download path collects streamed protobuf messages into `Vec<CompactBlock>` and inserts them into the cache without semantic validation of fields such as `txid` length or spend nullifier encoding.
- In `zcash_client_sqlite`, the cache DB stores serialized `CompactBlock` protobufs; later scanning reads them back with `CompactBlock::decode(&data[..])` and then calls the scan path in [`zcash_client_sqlite/src/chain.rs`](/Users/lorenzord/Desktop/zk/librustzcash/zcash_client_sqlite/src/chain.rs).
- Prost decoding validates protobuf wire structure, but not semantic invariants like “this `txid` must be 32 bytes” or “this nullifier must decode canonically”.
- Before the hardening changes in this turn, those semantic failures reached panicking helpers in the scan path rather than `ScanError`.
- The same malformed block, once persisted to cache, would be revisited on the next scan attempt unless the cache entry was replaced or the cache rewound/purged.

Reachability summary:
- Malformed protobuf bytes that fail prost decoding are already rejected earlier.
- Semantically malformed `CompactBlock` / `CompactTx` messages that remain valid protobufs were reachable up to the panic site.
- The most realistic adversarial source is a malicious or compromised `lightwalletd`, not the public blockchain directly.

## Real-world deployment analysis

### Known consumers

- `zcash_client_backend` is documented on docs.rs as “a crate for implementing Zcash light clients.”
- `zcash_client_sqlite` is documented on docs.rs as “an SQLite-based Zcash light client” and explicitly stores `CompactBlock`s in a cache database.
- Zcash’s Light Client Development docs describe the Android and iOS SDKs as the maintained mobile wallet SDKs built around this stack.
- The Android SDK `Synchronizer` docs explicitly show:
  - `LightWalletGrpcService`
  - `CompactBlockStore`
  - `CompactBlockProcessor`
  - `RustBackend`
  - a `lightwalletdHost` / `lightwalletdPort`
- The `lightwalletd` README states it serves mobile and other wallets “such as Zashi and Ywallet.”

What I can state confidently:
- Zashi is a real deployed consumer of `lightwalletd`.
- The official Android/iOS SDK architecture clearly consumes compact blocks from `lightwalletd` and uses a Rust backend / compact-block cache model that matches the crates audited here.
- I did not, in this pass, independently prove that every named third-party wallet embeds these exact Rust crates, so I do not claim that for YWallet or others.

### Trust boundary

- Official docs describe light clients as referencing a trusted full node’s blockchain copy through `lightwalletd`.
- The wallet threat model explicitly defines:
  - a `Lightwalletd-Compromising Adversary`
  - a `Typical Adversary` where the connection to `lightwalletd` is protected by TLS and the operator is trusted to serve valid chain data
- The same threat model explicitly acknowledges that `lightwalletd` compromise should be assumed to happen eventually.

### Authentication / integrity

- Compact blocks are not cryptographically authenticated by the client before scanning in the reviewed path.
- The client receives length-delimited gRPC protobuf messages, decodes them, caches them, and scans them.
- TLS can protect the transport against a passive or active network MITM in best-practice deployments, but it does not protect against a malicious or compromised `lightwalletd`.
- The official threat model says they plan to improve this class of issue with block header and note commitment tree validation via ZIP 307, which strongly implies the current model does not fully authenticate all server-provided compact-block semantics before use.

### Crash behavior

- I found no `catch_unwind` or panic recovery wrappers in the relevant scanning path.
- `scan_blocks` expects `scan_cached_blocks` to return a `Result`; continuity errors are specially handled, but a panic bypasses that entire error model.
- The workspace release profile sets `panic = 'abort'` in `Cargo.toml`, which means a reached panic is likely process-fatal in standard release builds unless a downstream consumer overrides the profile.
- I did not verify downstream mobile wrapper behavior beyond this repo, so I do not claim whether a given app supervisor restarts the process automatically.

### Persistence / crash loop potential

- Because downloaded compact blocks are cached before scanning, a semantically malformed block can persist in the local cache.
- If the same cached block is rescanned on restart, the wallet can re-hit the same failure.
- Therefore:
  - malicious server still in use: repeated crash is plausible
  - single transient compromise that already poisoned the cache: crash loop is plausible until cache replacement, rewind, or purge
- I did not demonstrate this end-to-end in a deployed wallet app, so I treat this as plausible rather than fully proven.

## ZCG bounty viability analysis

Program fit under the ZCG initiative is mixed:

- In-scope repo: yes, because the affected code is in `librustzcash`.
- Strong fit to “core tenets”: weak.
  - No demonstrated supply impact
  - No demonstrated funds loss
  - No demonstrated finalization issue
  - No demonstrated privacy break
- Best honest harm model:
  - remote wallet DoS / sync interruption via malicious or compromised `lightwalletd`
  - possibly persistent until restart or cache cleanup

Why this is weaker than it first appears under ZCG:
- The ZCG framework is centered on supply, store of value, privacy, and finalization.
- The finding primarily affects light-wallet availability.
- The program text says `librustzcash` findings will be graded based on whether the affected code actually flows into core; this code path is in light-client crates, not node consensus validation.

Why it is not purely dismissible:
- The official wallet threat model treats malicious or compromised `lightwalletd` as a real security adversary, not an out-of-scope fantasy.
- The affected path is used by real wallet ecosystems rather than only by test code.
- The issue is not “unsafe API misuse”; it was a panic in a public light-client ingestion path that already advertised structured scan errors for related malformed inputs.

Bottom line:
- This is stronger than “local DB corruption only”.
- This is weaker than a core-tenet ZCG finding.
- Presently it looks more like a low-value security-hardening / remote-wallet-DoS candidate than a high-confidence ZCG bounty winner.

## Revised severity assessment

- Technical classification: `remote wallet DoS via malformed compact-block data from malicious or compromised lightwalletd`
- Best conservative repository-local priority: `P3`
- Best conservative ZCG-style severity: `Low`, with an argument for `Medium` only if demonstrated against a real deployed wallet showing reliable crash or persistent sync failure under best-practice operation.

Why not higher:
- No funds loss
- No privacy loss
- No consensus divergence
- No direct core-node impact

What could raise it:
- Reproducing a persistent crash loop or unrecoverable sync interruption in Zashi or the official mobile SDK stack
- Showing that a standard wallet deployment can be driven into this state by a malicious public `lightwalletd` without any user opt-in beyond normal server selection

## Recommended disclosure strategy

**Recommendation:** gather stronger reachability evidence first

Rationale:
- There is enough here to justify a hardening fix and possibly a security report.
- But a Zcash triager may plausibly say: “this is malformed trusted input from lightwalletd, not a core security issue.”
- The best next evidence would be an end-to-end reproduction against a deployed wallet stack, ideally:
  - Zashi or the official Android/iOS SDK
  - release build behavior
  - proof that the malformed block can be supplied by a malicious `lightwalletd`
  - proof that restart does not automatically recover because the cached block re-triggers the failure

If that stronger evidence is hard to obtain, this is a good candidate to convert into a hardening PR instead of pursuing a bounty submission aggressively.

## Rejected hypotheses

### Hypothesis

RedJubjub randomized verification keys or small-subgroup encodings can slip through librustzcash transaction parsing and enable a spend authorization bypass.

### Why it looked suspicious

Sapling transaction parsing defers some checks to later verification, and `read_spend_auth_sig` accepts raw 64-byte signatures without immediate validation.

### What code/tests disproved it

- Sapling `rk` parsing uses `redjubjub::VerificationKey::try_from(bytes)` in `zcash_primitives/src/transaction/components/sapling.rs:145-149`, but the surrounding code comments already indicate that “not small order” is enforced later in verification, not necessarily at parse time.
- Upstream Sapling verification explicitly rejects small-order `rk` before proof public-input construction in `sapling-crypto/src/verifier.rs:45-50`.
- I did not find a safe spend-verification path in this repo that accepts a small-order randomized verification key without subsequently passing through that verifier-side rejection.

### Whether it is still worth fuzzing

Yes. Fuzzing malformed `R`/signature encodings and randomized-key edge cases is still worthwhile, but I do not currently have a demonstrated forgery or bypass path.

### Hypothesis

The wallet’s Sapling DB rehydration path turns ZIP 212 notes into pre-ZIP 212 notes and can make valid notes invisible or unspendable.

### Why it looked suspicious

`zcash_client_sqlite/src/wallet/sapling.rs:57-66` intentionally reconstructs stored notes as `Rseed::BeforeZip212(rcm)` even if the original note used post-ZIP-212 note plaintext.

### What code/tests disproved it

- In local `sapling-crypto`, `Rseed::AfterZip212` derives the note commitment trapdoor `rcm`, while `Rseed::BeforeZip212` stores `rcm` directly; see `sapling-crypto/src/note.rs:25-40`.
- Note equality, `cmu`, and nullifier derivation are commitment-based and depend on `rcm`, not on retaining the original post-ZIP-212 `esk` derivation path; see `sapling-crypto/src/note.rs:55-59`, `:112-121`.
- The SQLite path also revalidates diversifier length and reconstructs the recipient from the stored UFVK and scope before rebuilding the note at `zcash_client_sqlite/src/wallet/sapling.rs:38-47`, `:95-111`, `:113-121`.
- I did not find a concrete path from this representation choice to an incorrect nullifier, incorrect note commitment, or spend failure.
- I later added a direct regression test, `to_received_note_roundtrips_zip212_note_invariants`, that confirms `cmu`, `nf`, `rcm`, scope, and tree position survive rehydration through `to_received_note`.

### Whether it is still worth fuzzing

Yes. The basic invariant is now covered, but migration-time and restore-time note interpretation paths still merit deeper adversarial tests.

### Hypothesis

Unified address / UFVK parsing accepts duplicate or invalid receiver sets and can create cross-layer address ambiguity.

### Why it looked suspicious

Unified containers often fail on duplicate typecodes, only-transparent receiver sets, or receiver-order ambiguities.

### What code/tests disproved it

- Unified parsing rejects duplicate typecodes, out-of-order encodings, both transparent receiver types, and only-transparent containers in `components/zcash_address/src/kind/unified.rs:340-359`.
- Sapling payment-address parsing checks both diversifier validity and non-identity prime-order `pk_d` via `PaymentAddress::from_bytes` and `from_parts` in `sapling-crypto/src/address.rs:28-39`, `:57-71`.
- I did not find two distinct valid byte encodings for the same logical Unified Address in the code paths reviewed.

### Whether it is still worth fuzzing

Yes. Length, ordering, and duplicate-receiver fuzzing is still useful for hardening, but I do not have a security-impacting ambiguity yet.

### Hypothesis

PCZT Sapling verification fails to bind randomized verification keys, nullifiers, or note commitments tightly enough, enabling cross-layer signing misuse.

### Why it looked suspicious

PCZT intentionally stores partial data and can be pruned, which makes “weak invariant at construction, strong invariant later” bugs plausible.

### What code/tests disproved it

- In local `sapling-crypto`, PCZT verification includes dedicated checks for `verify_nullifier`, `verify_rk`, `verify_cv`, and `verify_note_commitment` in `sapling-crypto/src/pczt/verify.rs:11-114` and `:125-158`.
- `verify_rk` recomputes the randomized verification key from `vk.ak.randomize(alpha)` and compares it to `self.rk` at `:103-114`.
- `verify_nullifier` recomputes the note from recipient, value, and `rseed`, verifies FVK ownership of the note, and recomputes the nullifier from the witness position at `:67-100`.
- I did not find a reachable safe caller in this repo that skipped those checks and still treated the resulting PCZT as semantically validated.

### Whether it is still worth fuzzing

Yes. The best next step is adversarial tests around partially-pruned PCZTs and signer/verifier role separation, but I do not currently have a bounty-quality exploit.

### Hypothesis

Sapling or Orchard hash-to-curve / nullifier derivation uses a missing or incorrect domain separator, missing subgroup clearing, or a non-canonical point encoding that could create note/nullifier aliases.

### Why it looked suspicious

This repo depends on curve-specific hash-to-group, note-commitment extraction, and randomized nullifier logic that would be bounty-grade if any DST/cofactor/canonicalization step were missing.

### What code/tests disproved it

- Sapling `group_hash` clears the cofactor and rejects the identity before returning a `jubjub::SubgroupPoint` in `sapling-crypto/src/group_hash.rs`.
- Sapling `diversify_hash` is just `group_hash(d, KEY_DIVERSIFICATION_PERSONALIZATION)` in `sapling-crypto/src/spec.rs`, so the subgroup and non-identity guarantees flow into diversified base generation.
- Sapling nullifier derivation hashes canonical subgroup encodings of `nk` and `rho` via `to_bytes()` in `sapling-crypto/src/spec.rs` and `src/note/nullifier.rs`.
- Orchard nullifier derivation uses an explicit hash-to-curve domain, `pallas::Point::hash_to_curve("z.cash:Orchard")(b"K")`, and canonical base-field nullifier encoding in `orchard/src/note/nullifier.rs`.
- Orchard note construction rejects `RandomSeed` values whose derived `esk` is invalid for the note’s `rho`, and `Note::from_parts` returns `None` if the resulting note commitment is invalid in `orchard/src/note.rs`.
- I did not find a reachable path where two encodings of the same logical note/nullifier/commitment are both accepted by the safe APIs reviewed.

### Whether it is still worth fuzzing

Yes. The best follow-up is differential/fuzz testing around `group_hash`, Orchard `hash_to_curve`-derived fixed bases, and note commitment extraction, but I do not currently have a mismatch worth reporting.

### Hypothesis

The high-level PCZT `Signer` role can be tricked into authorizing a spend or recipient set inconsistent with the semantic note fields because it validates too little before signing.

### Why it looked suspicious

`pczt/src/roles/signer/mod.rs` only calls `verify_nullifier(None)` opportunistically when enough note fields are present, and does not automatically call `verify_cv`, `verify_rk`, or `verify_note_commitment` before signing.

### What code/tests disproved it

- The low-level Sapling and Orchard PCZT signing methods still require the provided spend-authorizing key to match the randomized verification key:
- Sapling `sign` computes `rsk = ask.randomize(alpha)` and rejects unless `self.rk == VerificationKey::from(&rsk)` in `sapling-crypto/src/pczt/signer.rs`.
- Orchard `sign` does the same with RedPallas in `orchard/src/pczt/signer.rs`.
- External signatures are only accepted if they already verify against the stored `rk` and `sighash`.
- The high-level API explicitly states that semantic validity checks are the caller’s responsibility before invoking signing methods.
- I did not identify a safe, documented caller in this repo that treats those semantic fields as trusted while skipping the underlying `rk` ownership check.

### Whether it is still worth fuzzing

Yes. This is worth adversarial UX and signer-policy testing, especially for hardware-wallet style integrations, but on current evidence it is a misuse-risk / API-footgun discussion rather than a clear vulnerability.

## High-value follow-up tests

Implemented in this pass:
- `zcash_client_backend/src/scanning/compact.rs`
- `scan_block_rejects_malformed_sapling_spend_nullifier_without_panic`
- `scan_block_rejects_malformed_orchard_action_nullifier_without_panic`
- `scan_block_rejects_malformed_txid_without_panic`
- `scan_block_rejects_out_of_range_tx_index_without_panic`
- `zcash_client_backend/src/proto.rs`
- `compact_tx_parse_txid_rejects_short_input`
- `compact_sapling_spend_nf_rejects_short_input`
- `compact_sapling_output_cmu_rejects_short_input`
- `zcash_client_sqlite/src/wallet/sapling.rs`
- `to_received_note_roundtrips_zip212_note_invariants`

Still good next candidates:
- `zcash_primitives/src/transaction/components/sapling.rs`
- `sapling_spend_sig_noncanonical_encoding_is_rejected_in_verification_path`
- `zcash_client_sqlite/src/wallet/sapling.rs`
- `invalid_diversifier_row_is_rejected_as_corrupted_data`
- `zcash_keys/src/address.rs` or `components/zcash_address/src/kind/unified.rs`
- `unified_address_rejects_duplicate_typecodes`
- `unified_address_rejects_both_p2pkh_and_p2sh`

## Search log

- Commands run:
- broad repo grep for `from_bytes`, `to_bytes`, `TryFrom`, `CtOption`, `clear_cofactor`, `is_identity`, `is_torsion_free`, `verify`, `verify_with_zip216`, `randomized`, `SpendAuth`, `Binding`, `diversifier`, `ivk`, `ovk`, `note`, `commitment`, `nullifier`, `transcript`, `challenge`, `domain`
- targeted reads of:
- `zcash_primitives/src/transaction/components/sapling.rs`
- `zcash_primitives/src/transaction/components/orchard.rs`
- `zcash_client_backend/src/scanning.rs`
- `zcash_client_backend/src/scanning/compact.rs`
- `zcash_client_backend/src/proto.rs`
- `zcash_client_backend/src/decrypt.rs`
- `zcash_client_backend/src/sync.rs`
- `zcash_client_backend/src/data_api/chain.rs`
- `zcash_client_backend/src/tor/grpc.rs`
- `zcash_client_sqlite/src/wallet/sapling.rs`
- `zcash_client_sqlite/src/chain.rs`
- `zcash_client_sqlite/src/lib.rs`
- `zcash_keys/src/address.rs`
- `zcash_keys/src/keys.rs`
- `components/zcash_address/src/kind/unified.rs`
- local dependency reads:
- `redjubjub-0.8.0/src/verification_key.rs`
- `sapling-crypto/src/verifier.rs`
- `sapling-crypto/src/note.rs`
- `sapling-crypto/src/address.rs`
- `sapling-crypto/src/group_hash.rs`
- `sapling-crypto/src/spec.rs`
- `sapling-crypto/src/pczt/verify.rs`
- `orchard/src/note.rs`
- `orchard/src/note/nullifier.rs`
- `orchard/src/address.rs`
- `orchard/src/pczt/verify.rs`
- external docs reviewed:
- ZCG disclosure initiative announcement
- Lightwalletd README
- Zcash light-client development docs
- Android SDK `Synchronizer` docs
- wallet app threat model docs

- Crates inspected:
- `zcash_primitives`
- `zcash_client_backend`
- `zcash_client_sqlite`
- `zcash_keys`
- `components/zcash_address`
- `pczt`
- `zcash_proofs`

- Keywords searched:
- `RedJubjub`
- `RedDSA`
- `jubjub`
- `verify_with_zip216`
- `SpendAuth`
- `Binding`
- `randomized`
- `clear_cofactor`
- `mul_by_cofactor`
- `is_torsion_free`
- `is_identity`
- `from_bytes`
- `to_bytes`
- `TryFrom`
- `CtOption`
- `diversifier`
- `ivk`
- `ovk`
- `note`
- `commitment`
- `nullifier`
- `transcript`
- `challenge`
- `domain`

- Tests run:
- `cargo test -p zcash_client_backend scan_block_with_my_spend -- --nocapture`
- `cargo test -p zcash_client_backend scan_block_with_my_tx -- --nocapture`
- `cargo run --manifest-path /tmp/librustzcash_audit_poc/Cargo.toml`
- `cargo test -p zcash_client_backend scan_block_rejects_malformed_txid_without_panic -- --nocapture`
- `cargo test -p zcash_client_backend scan_block_rejects_out_of_range_tx_index_without_panic -- --nocapture`
- `cargo test -p zcash_client_backend scan_block_rejects_malformed_sapling_spend_nullifier_without_panic -- --nocapture`
- `cargo test -p zcash_client_backend --features orchard scan_block_rejects_malformed_orchard_action_nullifier_without_panic -- --nocapture`
- `cargo test -p zcash_client_backend compact_tx_parse_txid_rejects_short_input -- --nocapture`
- `cargo test -p zcash_client_sqlite to_received_note_roundtrips_zip212_note_invariants -- --nocapture`
- `cargo fmt --all`

- Tests added:
- compact-block malformed-field regression tests in `zcash_client_backend`
- protobuf helper boundary tests in `zcash_client_backend::proto`
- ZIP-212 note rehydration regression in `zcash_client_sqlite::wallet::sapling`

- Tests that passed:
- existing `zcash_client_backend` compact scanning unit tests
- new malformed compact-block regression tests
- new `CompactTx::parse_txid` helper boundary test
- new ZIP-212 Sapling note rehydration round-trip test

- PoC result before patch:
- confirmed panic on malformed Sapling spend nullifier in compact-block scanning
- confirmed panic on malformed compact `txid` length in compact-block scanning

- Tests that failed:
- one exploratory test attempt that assumed small-order `rk` should be rejected at parse time; this assumption turned out to be wrong because the relevant rejection happens later in verification. The test was removed rather than forcing an incorrect contract.

## Triage check

Would a Zcash triager likely say this is only malformed trusted input and therefore not security-relevant?

- Likely yes, or at least “only weakly security-relevant,” for two reasons:
  - the official typical deployment model assumes trust in `lightwalletd` correctness
  - the impact is availability of a light wallet, not a direct violation of a core Zcash tenet

Why I still would not discard it outright:
- the official wallet threat model explicitly includes malicious or compromised `lightwalletd` as a meaningful adversary
- the affected path is remote ingestion, not just local corruption
- the panic occurred in a path that already modeled related malformed inputs as recoverable `ScanError`s

What additional evidence would change that conclusion:
- a demonstrated crash loop in an actual wallet app or official SDK consumer
- confirmation that default or recommended wallet deployments can be pointed at attacker-controlled `lightwalletd` instances
- evidence that the malformed cached block persists across restart and prevents sync recovery without manual cache intervention
- confirmation from downstream maintainers that this materially impacts deployed mobile wallets rather than only developer/test setups

Final triage outcome:
- keep as a conservative `P3`
- treat it as a realistic remote-wallet-DoS candidate under malicious/compromised `lightwalletd`, but still weak under ZCG without stronger deployed-wallet evidence
- do not overclaim it as a cryptographic break
- do not claim funds risk, consensus risk, or signature bypass

## Next best audit targets

1. PCZT role-separation and signer-policy surfaces.
   The remaining promising area is not raw signature forgery but cross-role misuse: partially-pruned PCZTs, incomplete semantic validation before signing, and any path where a signer can be induced to bless a misleading transaction view.

2. Orchard wallet-layer reconstruction invariants.
   The Orchard rehydration path mirrors the Sapling logic but has more moving pieces (`rho`, `rseed`, `RandomSeed::from_bytes`, `Note::from_parts`). It still looks underexplored for note/nullifier/recipient mismatches.

3. Full lightwalletd malformed-input surface beyond the currently fixed fields.
   `CompactBlock::{hash, prev_hash, height}` and related helpers still deserve systematic hostile-input testing, especially for persistent cache poisoning scenarios.

4. Native verifier vs helper-path mismatches around RedJubjub / RedPallas randomized keys.
   The main verification paths looked sound, but helper and multi-party transaction-construction flows remain a place where “valid enough for helper code, invalid for consensus” mismatches could still hide.

5. ZIP-212 / memo / note-decryption edge cases across migration and restore code.
   The basic note rehydration invariant now has coverage, but migration-time and enhancement-time note interpretation paths still merit deeper adversarial tests.
