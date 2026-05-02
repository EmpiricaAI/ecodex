# Asynchronous-Ground-Truth Calibration — Research Spec

**Status:** research-grade · Created 2026-05-01 · Feeds web product but stands alone

## Problem statement

Empirica's calibration model assumes **synchronous deterministic services**: when POSTFLIGHT closes a transaction, ground-truth observations are already available — `pytest` passed/failed, lint produced N errors, git diff captured M changes. The divergence between self-assessed vectors and these observations is the calibration signal.

For non-code domains (and for some code domains too), ground truth arrives **asynchronously, often hours or days later**:

| Domain | Self-assessment available | Ground truth arrives |
|---|---|---|
| Customer email reply | When draft is sent | When customer replies (hours-days) |
| Sales outreach | When sequence sent | When meeting booked / no-show occurs (days-weeks) |
| Legal contract draft | When clause is added | When opposing counsel redlines / signs (days-weeks) |
| Customer-service resolution | When ticket closed | When CSAT survey lands / customer escalates (days) |
| Research synthesis | When report delivered | When stakeholder accepts/rejects/requests revision (days) |
| Marketing copy | When ad live | When CTR/conversion data accumulates (hours-days) |

POSTFLIGHT closes the measurement window before any of these signals exist. Today's calibration framework would either: (a) ground vectors against zero observations and produce noise, or (b) skip grounding entirely (the `remote-ops` work_type does this — `calibration_status=ungrounded_remote_ops`).

Neither is satisfying. We want **delayed grounding**: the transaction closes with self-assessed vectors, and grounded confirmation arrives later via an out-of-band signal that retroactively updates the calibration record.

## Why this matters

1. **Strategic differentiator.** Empirica's "we measure our own confidence" pitch is much stronger when we can validate that confidence against real outcomes, not just code-side proxies. For non-coder users (web product), this *is* the trust mechanism — there's no other ground truth available.
2. **Research credibility.** Calibration research is mature in domains like medicine and forecasting. Bringing async-confirmation calibration into AI-agent telemetry is novel and academically respectable.
3. **Cross-domain unification.** Today empirica works for code; with async grounding, the same framework works for any knowledge work. That widens the addressable market significantly.

## Conceptual sketch

### Synchronous vs asynchronous calibration loops

```
Synchronous (today):
  PREFLIGHT → work → POSTFLIGHT (services ground vectors NOW) → next transaction

Asynchronous (proposed):
  PREFLIGHT → work → POSTFLIGHT (self-assessed vectors stored, grounding pending)
                                 ↓
                      [hours/days/weeks later]
                                 ↓
            grounded-update event (e.g. customer replied, CSAT 4/5, contract signed)
                                 ↓
            calibration record updated retroactively → future Brier scoring uses delayed signal
```

### Required pieces

1. **Pending-grounding queue** — POSTFLIGHT writes a record `{transaction_id, predicted_vectors, expected_signals: [...]}` into a queue keyed by signal source.
2. **Signal ingestion** — webhooks / pollers / explicit user-confirmation widgets that match incoming signals back to pending records.
3. **Retroactive calibration update** — when a signal lands, recompute the divergence and update the calibration store. Future Brier scoring includes this delayed correction.
4. **Stale-signal expiry** — if no signal arrives within a domain-configured window, mark `expired` (not failed — absence of signal is itself information).
5. **UX for the user** — non-coders need to see "we predicted high confidence on this draft; the customer replied positively, calibration confirmed" as a domain-meaningful narrative, not vector deltas.

## Domain-specific deterministic services (sketch)

For each domain, identify the services that can ground vectors:

| Domain | Possible deterministic services |
|---|---|
| Email reply | User accept/reject of draft, recipient reply time, recipient sentiment classifier (LLM-as-judge), thread length post-reply |
| Document drafting | Schema validators, redline acceptance rate, peer review LLM-as-judge, template-compliance checks |
| Research synthesis | Citation-existence checks, source-recency thresholds, claim/source ratio, fact-checker LLM-as-judge against grounded corpus |
| Customer service | CSAT survey scores, escalation rate, time-to-resolution, repeat-contact rate |
| Sales | Reply rate, meeting-booked rate, deal-closed rate, deal-cycle-time |
| Legal | Clause-presence checks via specialized models, jurisdiction-rule validators, redline rate, signature rate |
| Marketing | CTR, conversion rate, A/B test results, brand-voice classifier |

Most of these are mid-fidelity signals (not perfect ground truth, but informative). The framework needs to handle **noisy delayed signals** — assigning confidence weights to each signal source and combining them properly.

## Open research questions

1. **Brier scoring with delayed signals.** Standard Brier scoring assumes all observations land at scoring time. How do we extend it for retroactive updates? Time-decay weighting? Bayesian update?
2. **Signal selection.** With multiple noisy signals per domain, which combination yields tightest calibration? Likely domain-specific, learnable from historical data.
3. **Signal absence handling.** No reply in 7 days — is that ground-truth-negative ("the draft didn't engage") or signal-absent ("we don't know")? Domain-dependent.
4. **LLM-as-judge bias.** Many ground-truth services in non-code domains are LLM-based (sentiment, schema-compliance, redline-quality). LLM judges have their own miscalibration. Need to calibrate the judges before they can calibrate us. Recursive but tractable.
5. **User-in-the-loop signals.** When the user explicitly says "this draft was good" / "this one missed the mark", that's high-quality signal. How do we weight user-explicit signals against derived signals?
6. **Privacy.** Async signals often involve recipient behavior (did they reply? what tone?). For SaaS contexts, this is sensitive. Requires careful permission UX and probably client-side processing.

## Implementation sketch — minimum viable

A v1 of async calibration could be:

1. POSTFLIGHT writes `pending_calibration` records to a new SQLite table.
2. A simple webhook receiver (or scheduled poller) updates records as signals arrive.
3. One or two domain-specific signal providers (e.g. "user explicit accept/reject" + "thread-length-post-reply") for email use case.
4. A calibration-update job that retroactively recomputes Brier scores when records mature.
5. UX in the empirica CLI / web frontend that shows pending vs grounded vs expired calibration state.

Even this minimum unlocks the framework for one domain. Expanding to others is then mostly signal-provider engineering.

## Why this fits Empirica's research thesis

Empirica's existing thesis ("collaborative measurement: AI's beliefs informed by deterministic services, divergence is the calibration signal") is **already domain-agnostic**. The async extension keeps the same mathematical structure, just relaxes the timing assumption. It's an extension, not a redesign.

That's why this is research-grade rather than greenfield — we're extending a working framework into new territory, not starting over.

## Why log this now

Same as the web-product spec — the strategic conversation surfaced this and it would be costly to lose. Linked empirica goal: "Research: asynchronous-ground-truth calibration model". Status: planned. Activates either alongside web product MVP or earlier as a research line.
