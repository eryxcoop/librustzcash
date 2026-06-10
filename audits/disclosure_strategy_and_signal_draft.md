# Disclosure Strategy and Signal Draft

## Goal

Provide a practical disclosure plan for the wallet-boundary issue cluster that
currently appears strongest as:

- `librustzcash`: likely `Medium`, with a plausible but not guaranteed path to
  a stronger availability classification if maintainers agree with the freezing
  interpretation;
- `zallet`: likely `Low` or `Medium`, depending on how much weight maintainers
  give to restart persistence, recovery-path failure, visible wallet-owned
  state, and the now-demonstrated absence of an available local
  `truncate-wallet` repair path in one historic-recovery scenario.

This document intentionally avoids claiming a final severity. The point is to
maximize credibility and make maintainer triage easy.

## Recommended disclosure order

### 1. Report `zcash/librustzcash` first

This should be the primary report.

Reason:

- the root issues are library-level wallet-boundary behaviors;
- the acceptance, persistence, and malformed-input handling problems originate
  there;
- and `zallet` is best understood as a downstream product impact amplifier for
  the same cluster.

Channel:

- follow the `zcash/librustzcash` security policy;
- create a new private Signal group;
- include `dairaemma.31` and `pilizcash.01`;
- do not use public issues and do not send by email.

### 2. Mention downstream `zallet` impact inside the first report

Do not make the first contact about `zallet` alone unless you specifically want
product-only triage.

Reason:

- the strongest technical story starts at the library boundary;
- the `zallet` PoCs are best presented as downstream confirmation that the
  library-level weaknesses can manifest as user-visible freezing or denial of
  service;
- and the `zallet` policy is more conservative about wallet-local DoS.

### 3. Open a separate `zallet` security report only if requested or useful

A separate downstream report can still be worthwhile, but it should be framed
carefully:

- not as “this is definitely High”;
- but as “this downstream product also appears affected, with likely Low/Medium
  availability impact under your rubric.”

For `zallet`, the most realistic expectation is that maintainers may classify it
as `Low` or `Medium`.

## Suggested severity expectations

These are working expectations, not claims to send to maintainers as demands.

### `zcash/librustzcash`

Expected maintainer outcome: `Medium`

Why:

- wallet-side logic accepts or acts on insufficiently validated state;
- malformed follow-up input can break ordinary wallet workflows;
- the issue is security-relevant and not merely cosmetic;
- but there is no confirmed theft, consensus breakage, or privacy compromise.

Possible stronger interpretation:

- if maintainers strongly accept the “practical wallet freezing” framing, they
  may view the availability impact more seriously;
- but I would not lead with that as a requirement.

### `zallet`

Expected maintainer outcome: `Low` or `Medium`

Why:

- there are now stronger product-level PoCs than before;
- they show restart persistence, impact on funded wallets, impact on
  `recover_history`, and in one scenario the lack of an available
  checkpoint-backed `repair truncate-wallet` path;
- but the issue is still wallet-local and tied to malformed or malicious
  backend-fed data.

My practical expectation:

- `Medium` is defendable;
- `Low` would not be surprising;
- `High` is still a stretch unless maintainers heavily emphasize temporary
  freezing of practical wallet operation.

## Recommended framing

The report should be framed as:

- a private security triage request;
- not a public accusation;
- not a demand for a specific severity label;
- and not a “giant exploit” narrative.

Recommended framing sentence:

> I am reporting a wallet-boundary issue cluster that appears able to affect
> ordinary wallet workflows through insufficient validation, persisted
> wallet-relevant state, and malformed follow-up input. I am not asserting a
> final severity classification and would appreciate maintainer triage on
> whether the strongest confirmed impact should be treated as Low, Medium, or
> High under your rubric.

## What to emphasize

For the first private report, emphasize:

- wallet-side acceptance or persistence before sufficient local validation;
- ordinary workflow impact, especially scan, startup, and recovery;
- restart persistence;
- funded-wallet impact;
- and the strongest practical freezing evidence from the `zallet` PoCs.

## What to keep secondary

Keep these in supporting material or an appendix:

- false history;
- recipient-intent corruption;
- internal versus external reclassification;
- late proving failures;
- weaker composition ideas that are not part of the strongest executable path.

These are useful, but they should not become the headline if the goal is
credible triage.

## Signal message draft

Use this as the first message in the new Signal group for `zcash/librustzcash`.

> Hi, I’m reporting a private security issue affecting wallet-side logic in
> `zcash/librustzcash`.
>
> At a high level, the issue cluster appears to involve insufficient
> wallet-side validation, persistence or reuse of wallet-relevant state after
> that boundary, and malformed follow-up input that can break ordinary wallet
> workflows such as scan, startup, or recovery.
>
> I am not asserting a final severity classification up front. My goal is to
> provide a clean technical report for triage and let maintainers decide
> whether the strongest confirmed impact is best categorized as Low, Medium, or
> High under your rubric.
>
> I also have downstream product evidence from `zallet` showing stronger
> wallet-operability impact than a one-off crash: restart persistence, impact
> on a wallet with visible owned balance, failure in historic recovery, and in
> one scenario no available checkpoint-backed `repair truncate-wallet` path
> after the failure.
>
> I can share a short summary first and then the detailed PoCs and affected
> paths if that works best for triage.

## Shorter Signal version

If you want something more compact:

> Hi, I’d like to report a private security issue in `zcash/librustzcash`
> affecting wallet-side validation and malformed-input handling. The strongest
> current evidence suggests ordinary wallet workflows such as scan, startup, or
> recovery can be broken after insufficiently validated state is accepted, with
> downstream `zallet` PoCs showing restart-persistent operability problems. I’m
> not asserting a final severity up front and would appreciate maintainer
> triage on the correct classification.

## Attachments to prepare

When sending the report, have these ready:

- the private triage write-up:
  [local_wallet_private_triage_report.md](/home/lorenzo/Desktop/zcash/librustzcash/audits/local_wallet_private_triage_report.md)
- the library-oriented analysis and supporting audits in `audits/`
- the strongest `zallet` product-level PoCs, especially:
  - historic recovery termination with visible wallet balance
  - restart persistence
  - no checkpoint-backed `truncate-wallet` repair path in the strongest repair-path scenario

## Bottom line

The most credible plan is:

1. report privately to `zcash/librustzcash` first via Signal;
2. ask for triage rather than demanding `High`;
3. present `zallet` as downstream impact evidence;
4. treat `Medium` as the most likely maintainer classification for
   `librustzcash`, and `Low/Medium` as the most likely range for `zallet`.
