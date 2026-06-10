# Private Security Triage Report

Repository target: `zcash/librustzcash`

Submission route: follow the current private security reporting instructions for
`zcash/librustzcash` and submit via a new Signal group, not by public issue and
not by email.

## Summary

This report concerns wallet-side logic in `librustzcash` that can accept,
persist, or act on insufficiently validated shielded state, and that can then
be driven into deterministic failure by malformed follow-up input during
ordinary wallet workflows such as scanning, startup, or recovery.

I am **not** asserting a final severity classification in this report. My goal
is to provide a clean technical summary for maintainer triage. The strongest
impact hypothesis appears to be some form of local-wallet denial of service or
practical wallet freezing against an individual wallet, but I would prefer the
maintainers assign the final severity under the applicable repository and ZCG
rubrics.

## What appears to be happening

At a high level, the relevant behaviors seem to compose as follows:

1. wallet-side code accepts or acts on shielded state before the strongest
   local validation boundary has been enforced;
2. some of that state or related metadata can persist long enough to influence
   later ordinary wallet behavior;
3. a malformed follow-up input can then cause failure in a normal wallet path
   such as scan, startup, or recovery;
4. in the stronger variants, the same backend conditions can cause recurrence
   after restart, creating a practical freezing condition.

I am intentionally keeping the claim narrow. The main point is not “false
history” in the abstract. The main point is that insufficient wallet-side
validation plus later malformed input may be enough to break ordinary wallet
operation in a user-visible and repeatable way.

## Confirmed technical ingredients

The following points are the parts I would present as confirmed or strongly
supported by existing work in this branch:

- `decrypt_and_store_transaction` can persist wallet-relevant state without
  first establishing full local consensus validity.
- compact scanning can surface shielded outputs into wallet state before local
  proof or signature validation.
- malformed compact metadata can panic scanning code instead of being rejected
  through a graceful error path.
- note-adjacent wallet metadata may persist without enough rebinding to the
  original tx-bound context to make later wallet behavior obviously robust.
- some composed same-wallet paths appear to show that state corruption can
  coexist with later operational failure in the same wallet instance.
- stronger downstream product variants appear to show restart-persistent
  breakage while the same malicious or malformed data source remains in use.

These points are listed as triage inputs, not as a final exploit narrative.

## Strongest impact hypothesis

The strongest current impact hypothesis appears to be:

- local-wallet denial of service against an individual wallet;
- practical wallet freezing while the same malicious or malformed upstream data
  source remains in use;
- failure in ordinary scan, startup, or recovery workflows rather than only in
  synthetic internal test harnesses.

I am **not** claiming that this is necessarily the correct final severity. I am
only noting that this seems like the strongest plausible severity direction,
pending maintainer review.

## What I am not claiming

To keep the report narrow and easy to triage, I am **not** claiming:

- theft of funds on chain;
- consensus bypass;
- network-wide privacy failure;
- permanent freezing of funds requiring a fork;
- a valid on-chain spend constructed from incorrectly bound local note state.

I am also not asking maintainers to accept a specific `Low`, `Medium`, or
`High` label solely on the basis of this write-up.

## Why I think this merits private security triage

Even if the final severity ends up being lower than the strongest current
hypothesis, I believe private security triage is appropriate because:

- the behaviors are reachable through wallet-relevant ingestion and processing
  paths rather than purely dead code;
- malformed or insufficiently validated input appears able to affect persisted
  or later-reused wallet state;
- the most concerning variants affect ordinary operational workflows such as
  scanning and recovery;
- and at least some variants appear capable of causing user-visible breakage
  that is not trivially self-healing.

## Suggested maintainer triage questions

The questions I think matter most for classification are:

1. Do maintainers agree that the wallet-side validation boundary is too weak in
   the paths exercised here?
2. Do maintainers agree that malformed follow-up input can trigger ordinary
   wallet failure after that state has been accepted?
3. Is the restart-persistent or recovery-blocking behavior considered within
   scope for the repository's security policy?
4. Under the maintainers' preferred rubric, does the strongest confirmed impact
   land as `Low`, `Medium`, or `High`?

## Recommended submission style

I would recommend submitting this as a private report in a restrained tone:

- present the confirmed behaviors first;
- describe the strongest impact as a hypothesis rather than a demand;
- explicitly state that final severity is left to maintainer judgment;
- keep weaker supporting themes, such as false history or mutable recipient
  intent, in an appendix or follow-up material rather than in the headline.

## Optional wording for the opening message

The following wording is likely safer than leading with a hard severity label:

> I am reporting a private security issue in `librustzcash` involving
> insufficient wallet-side validation, persistence of wallet-relevant state,
> and malformed follow-up input that appears capable of breaking ordinary
> wallet workflows such as scan, startup, or recovery. I am not asserting a
> final severity classification and would appreciate maintainer triage on
> whether the strongest confirmed impact should be treated as Low, Medium, or
> High under your rubric.

## Bottom line

My recommendation is to report this privately to `zcash/librustzcash` without
claiming a final severity upfront.

The strongest plausible interpretation appears to be local-wallet DoS or
practical freezing, but it is more credible to let maintainers decide whether
the final classification is `Low`, `Medium`, or `High`.
