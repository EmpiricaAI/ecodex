# EXP-SHADOW corpus design — ecodex's leg of the prevention-value experiment

**Status:** DRAFT for SER convergence, not yet run. Written 2026-08-11.
**Owner:** ecodex practice. **Scopes:** goal `63be9970-c314-47f3-a265-82e862440988`.
**Serves:** SER `ser_c2c87f7d63154258b59a14e4` (prevention-value base-rate
experiment) and the participating-tier commitment in SER `prop_rdgppyim...`
(prevention-currency causal measurement).
**Depends on / builds on:** `empirica/docs/architecture/PREVENTION_MEASUREMENT_SPEC.md`
(H1, `prevention_events` schema, oracle contract — confirmed already built in
`empirica/core/prevention/` + 4 test files, not just planned).

> **Read this as a proposal, not a ruling.** §4 below (open questions) is
> exactly the SER-agenda list the measurement spec names — Q1-Q4 there — plus
> one ecodex-specific addition. These are not decided here. This doc's job is
> to give research and core something concrete to react to, grounded in what
> ecodex's actual infra can produce, rather than staying abstract.

---

## 1. What ecodex owns (per the spec's role split)

> "ecodex = shadow/control arm + validation battery" — spec §"Serves SERs" line.
> "the EXP-SHADOW randomizer (ecodex)" — spec §8, listed as a non-goal for
> core's build, i.e. ours to build.

Concretely: **ecodex owns producing a corpus where a known anti-pattern's
PREFLIGHT prior is surfaced on some subjects and withheld on others**, run
under full transaction discipline on both arms (per empirica core's corpus
constraint, already logged as a finding this practice holds), so core's
`prevention_events` emission has real exposed/non-exposed rows to detect
`prevented` vs `failed` against.

## 2. The reusable infra (verified this transaction, not the 2026-06-03 design)

The relevant existing infra is **not** `ecodex-lab-design.md`'s Kimi-guided
distillation topology (that was a different, earlier experiment — single
persistent practice, mesh-guided calibration, abandoned after run 1's mesh
mechanism check). The reusable piece is the **paired-worktree comparison
methodology** actually run 7 rounds, 2026-08-05 through 2026-08-09:

- One git worktree + branch per round, named `experiment/<actor>-<task-slug>`
  (e.g. `experiment/claude-session-end-warning`,
  `experiment/gpt-5.6-sol-session-end-warning`).
- Each worktree gets an identical `TASK.md` brief: problem, constraint,
  success criteria, "work in this worktree, follow your normal engineering
  discipline."
- The actor (previously: Claude vs gpt-5.6-sol, a model-comparison axis) works
  the task under full PREFLIGHT/CHECK/POSTFLIGHT discipline, independently
  verified before landing.

**The adaptation for EXP-SHADOW:** swap the axis. Instead of *same task,
different model*, the pair becomes *same task, same model, prior surfaced
(treatment) vs prior withheld (control)*. The mechanical change is small —
the harness/task-brief machinery ports directly; what changes is which
`TASK.md` briefs get selected (ones where a cataloged anti-pattern is a live
risk) and whether the PREFLIGHT/CHECK context includes that pattern's prior.

## 3. Corpus construction — concrete proposal

### 3.1 Subject definition

A **subject** = one `TASK.md`-scoped unit of work (matches the spec's
"goal/subtask" subject-key option) where a specific `pattern_key` from
ecodex's `.broccoli-accept`-style catalog is plausibly triggerable. Examples
already in-repo as real task shapes: the SessionEnd-clamp task (§B "fallback
masks primary" family), the cwd-deletion session-freeze fix (§D "unrecoverable
gate" / "authority on the wrong field" family) — both drawn from this
session's own commit history, so the anti-pattern catalog is not
hypothetical, it's populated from real incidents already logged as findings.

### 3.2 Treatment / control assignment

- **Treatment arm:** the pattern's `broken-if`/`by-design-if` disambiguator is
  injected as a PREFLIGHT prior (a `finding`/`unknown` surfaced before the
  actor starts, phrased as ecodex's own broccoli-catalog entries already are).
- **Control arm:** identical `TASK.md`, identical model, **no prior surfaced**
  — the actor investigates cold.
- **Both arms run under full transaction discipline** (per empirica core's
  constraint) — this is a hard gate on corpus validity, not a nice-to-have.
  ecodex-lab's existing setup already defaults to this (Sentinel firewall is
  vendored and fires — verified live in the 2026-06-03 experiment run); no
  separate verification work needed here.

### 3.3 What gets measured

`prevention_events` rows accrue automatically from live PREFLIGHT/CHECK/
POSTFLIGHT telemetry once core's emission is wired to read ecodex's
transaction stream the same way it reads empirica-core's own (this is core's
Leg A, already built — the open item is whether it's currently subscribed to
ecodex's session stream, which I have NOT verified and am logging as an
unknown, not assuming).

---

## 4. Open questions — SER agenda, not decided here

| # | Question (from spec §6) | ecodex-specific data point | Recommendation (non-binding) |
|---|---|---|---|
| Q1 | Window W per outcome family | ecodex tasks complete in ~10-60 min (round 1 gpt-5.6-sol timing: 10m06s wall, per this practice's logged findings), not the spec's default 30-day production window. A 30-day W doesn't fit a worktree-per-round corpus model. | Use **task-completion + N follow-on rounds touching the same `pattern_key`** as W, not calendar time — but this is a real deviation from the spec's default and needs research's sign-off, not a unilateral swap. |
| Q2 | Unit of randomization (subject / session / practice) | Contamination risk is concrete here: the SAME practice (Claude via `ecodex`, or whichever model occupies `ecodex-lab`) running both a treatment and control subject in sequence will generalize from the first regardless of assignment — this practice's own accumulated `.broccoli-accept` history is exactly that generalization already happening. | Recommend **fresh practice per subject** (like the existing paired-worktree pattern already does per round) rather than reusing one practice across the corpus — expensive but avoids the contamination this session's own artifact history demonstrates is real. |
| Q3 | Min n / power for a credible per-pattern ATE | No effect-size estimate exists yet to compute this — that's research's causal-model job, not inferable from ecodex's infra alone. | ecodex can report throughput: manual paired-worktree rounds run at roughly 1/session:pair. A corpus of n≥20 pairs needs either (a) many sessions or (b) automating round spin-up (extending `lab_stall_monitor.py` / cockpit tooling) — worth scoping ONLY once research names a target n. |
| Q4 | Fabrication-oracle differentiation | Not built on ecodex's side at all. | Explicitly out of scope for this design pass — flag as a later slice, don't block on it. |

## 5. What this design does NOT claim

- Does not claim the causal model, estimator, or ATE computation — research's.
- Does not claim core's `prevention_events` emission is currently wired to
  ecodex's transaction stream — unverified, logged as an unknown below.
- Does not commit to a specific n or start date — gated on Q2/Q3 convergence.
- Does not replace or conflict with EXP-SHADOW T1b (ground-truth-reconstruction
  gate on ecodex-lab, separate track) — that goal's own description already
  says it "MUST land before the prevention experiment leans on ground-truth
  reconstruction," and this design doesn't lean on that yet (it uses
  PREFLIGHT/CHECK/POSTFLIGHT telemetry directly, not reconstructed wake logs).

## 6. Next step

Route this doc into SER `ser_c2c87f7d63154258b59a14e4` (open → in_progress)
via `cortex_propose(payload.action='transition_ser')` semantics, addressed to
empirica core (measurement side) and, if in-tenant per David's ruling, kept
off Philipp's `empirica-research` per the 2026-08-11 ECO decision — this is
ecodex + empirica-core's own design, not routed through research's tenant.
