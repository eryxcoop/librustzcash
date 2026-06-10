# Signal Disclosure Draft

Hi, I would like to privately disclose a security issue affecting `librustzcash`, with downstream impact also demonstrated in `Zallet`.

At a high level, this involves a composed wallet-side problem including:

- incorrect trust in wallet-bound metadata after signing / extraction;
- wallet-visible misclassification of outputs and recipient intent;
- legacy chain-context confusion in wallet-side transaction attribution;
- and a follow-on crash / operational freeze path when malformed sync data is processed.

I have working PoCs and test coverage for the relevant paths in:

- `zcash_client_backend`
- `zcash_client_memory`
- `zcash_client_sqlite`
- and a real `Zallet` integration path

The currently demonstrated impact is local wallet state corruption / misattribution, inconsistent sent-history or recipient intent, and a reproducible wallet crash / freeze scenario under malicious sync input.

I can share a writeup, the exact tests, and reproduction steps privately right away.

## Likely Triage Objections

### "This is not one bug, it is an artificial composition."

That is partly true at the root-cause level, but the important point is that the composed behavior is not hypothetical. The relevant wallet-side transitions are reproduced with executable PoCs, and the downstream product impact is demonstrated in a real wallet consumer.

### "There is no confirmed theft of funds or consensus bypass."

Correct. The report should not claim direct on-chain theft or consensus breakage. The demonstrated impact is wallet-side state corruption, incorrect recipient / history attribution, and a real crash / freeze path under malicious sync input.

### "The PCZT part is not production-reachable in every consumer."

This is a reasonable scope objection. The answer is to separate the PCZT-specific issue from the rest of the composed story:

- PCZT metadata / recipient classification corruption is a confirmed library-level issue.
- Legacy chain-context confusion is separately confirmed.
- Malformed sync input crash / freeze is separately confirmed.
- The composed writeup explains why these families matter more together.

### "The Zallet impact depends on a malicious or nonstandard sync source."

Yes, but that still matters. The demonstrated product impact comes from input arriving through the wallet's normal sync path, not from arbitrary local memory corruption or a synthetic parser entrypoint.

### "These are just tests or harnesses, not production behavior."

The tests are being used as reproducible proof, not as evidence of an imaginary surface. The underlying APIs and state transitions exercised by the PoCs are the real ones used by the wallet layers.

### "This is only a UI or accounting problem."

The strongest response is that it is not limited to presentation. The issues affect persisted recipient classification, wallet-visible sent history, transaction summaries, and downstream wallet state reuse. In the broader composed path, that state corruption coexists with a real operational crash / freeze scenario.

### "The SQLite result changed after your local fixes, so maybe this was just branch-specific."

The SQLite fix addressed a real wallet-store bug: stale `to_address` / `to_account_id` persistence in `sent_notes` upserts and external-recipient projection. That does not erase the underlying report; it removes one backend-specific divergence and brings SQLite in line with the already-demonstrated backend / memory semantics.

### "The `get_tx_history()` inconsistency no longer reproduces exactly."

That is fine and should be stated honestly. The report should not over-commit to one exact final symptom if different backends now converge on slightly different post-corruption behavior. The stronger invariant is that the wallet can still persist and reuse corrupted semantic state.

### "This only happens if callers pass `None`, mutable metadata, or malformed input."

That is precisely the trust-boundary issue. The question is not whether those inputs are magically safe by default; it is whether downstream wallet logic persists or reuses semantically dangerous state after accepting them through supported or realistic flows.

### "The malformed compact block panic is just a parser robustness issue."

If presented alone, they may try to rate it that way. The answer is to frame it as a wallet operational failure reached through the normal sync surface, and to emphasize that it composes with the other wallet-state issues rather than standing alone as a toy parser crash.

### "These should really be split into multiple reports."

They may be right operationally. Be ready to split the material into:

- PCZT metadata / recipient binding corruption
- legacy chain-context attribution confusion
- malformed compact sync-input DoS
- SQLite sticky persistence / history projection bug

The composed document is still useful because it explains why the combined impact is more important than each component viewed in isolation.

## What We Should Avoid Claiming

- Do not claim direct theft of funds.
- Do not claim consensus bypass or chain-level invalid state acceptance.
- Do not claim that every part of the composed story is production-reachable in every downstream consumer.
- Do not claim that the PCZT-specific path is already part of every normal wallet send flow.
- Do not overstate `get_tx_history()` as a universal final symptom if different backends now differ in how they summarize the corrupted state.
- Do not present the malformed compact-block panic as if it were the only issue or as if it alone explained the full impact.
- Do not frame the report as “one magic Critical bug”; that will make the writeup easier to dismiss.

## What We Can Say Confidently

- There are confirmed wallet-side trust-boundary issues in `librustzcash`.
- Mutable wallet-bound metadata can be trusted after signing / extraction in ways that diverge from committed transaction semantics.
- Wallet-visible recipient classification and sent-history state can become semantically incorrect.
- Legacy chain-context confusion in wallet-side attribution is reproducible.
- The relevant wallet-state corruption paths are demonstrated with executable PoCs in `zcash_client_backend`, `zcash_client_memory`, and `zcash_client_sqlite`.
- There is a demonstrated downstream `Zallet` impact path.
- A real wallet can be crashed or operationally frozen by malicious sync input delivered through its normal sync surface.
- The composed impact is stronger than any one sub-bug considered in isolation, even if the maintainers prefer to triage the components separately.
