# Epistemic Programming — paradigm capture and Empirica mapping

**Origin:** David's conversation with Gemini (captured 2026-08-16), proposing
"epistemic programming" as a foundational paradigm for LLM-driven engineering:
shifting execution from uncalibrated state generation to **bounded epistemic
state transitions**. Preserved here because ecodex + Empirica already implement
most of it in production — which makes the spec less a proposal than an
independent articulation of this ecosystem's direction, and the deltas a
concrete R&D backlog.

---

## The spec's core, condensed

**Axioms**

1. **Grounding vs. inference.** A state is either *grounded* (verified via
   deterministic execution, file reads, empirical APIs) or *inferred* (derived
   via probabilistic generative reasoning). Unverified inference decays with
   step depth: `C_n = C_0 · Π(1 − δ_i)` — when confidence falls below a
   threshold τ, halt inference and actively ground.
2. **Anti-agreement / anti-poisoning.** No agent validates a premise without
   independent derivation or search grounding. A prompt's "X fails because Y"
   is a *hypothesis* to probe, never a fact to accept. Agreement without an
   empirical trace is a runtime error (`EPISTEMIC_UNGROUNDED`).
3. **Quantified epistemic state.** Every mutating action exposes an explicit
   epistemic tuple `⟨intuition, searched, action-readiness, confidence⟩`,
   weighting verified retrieval over generative memory.

**State taxonomy:** `UNKNOWN → (probe) → GROUNDED → DERIVED INFERENCE →
(test) → CANONICAL`, with `UNGROUNDED` (reject or probe; never execute) and
`BOUNDED_VOID` (explicit unknown; freeze, compress the discrepancy vector,
escalate to a human — never guess).

**Mechanics:** independent premise verification before any diagnosis; a
verification toolchain split into read-only probe / sandboxed hypothesize /
trace-backed commit; agents synchronizing via structured epistemic envelopes
(state vectors + grounding traces, not narrative); consensus lockdown when
peer confidence diverges; ntfy escalation on epistemic voids.

**Open research vectors (the spec's own):** epistemic drift in multi-agent
loops (Agent B treating Agent A's inference as ground truth), dynamic
epistemic garbage collection, formal-verification integration (SMT/Z3).

---

## Mapping: spec concept → what already exists here

| Spec concept | Existing mechanism | Status |
|---|---|---|
| Grounded vs. inferred state | `--epistemic-source {intuition\|search\|mixed}` provenance on every artifact; PREFLIGHT `claims` with `ran/read/retrieved/assumed` grounding; POSTFLIGHT adjudication (`held/refuted/untested`) | **Live** |
| Anti-agreement axiom | EPP (Epistemic Persistence Protocol): classify pushback EMOTIONAL/RHETORICAL/EVIDENTIAL/LOGICAL/CONTEXTUAL before updating; EWM Anti-Agreement Protocol ("never agree without grounding") | **Live** |
| Quantified epistemic tuple | The 13 vectors (know/do/context/…/uncertainty) on every PREFLIGHT/CHECK/POSTFLIGHT, calibrated against deterministic service observations | **Live** — richer than the spec's 4-tuple |
| Execution gated on confidence threshold | Sentinel noetic firewall: praxic actions require an open PREFLIGHT + passed CHECK; CHECK certifies the claims the action rests on | **Live** — gate is claim-grounding, not a scalar threshold |
| Read-only probe vs. mutative commit toolchain | Sentinel's noetic/praxic discrimination by *effect* (read verbs flow free, mutating invocations gate), enforced via PreToolUse hooks | **Live** |
| Structured mesh envelopes over narrative chat | Cortex mailbox: typed proposals (collab_brief vs ECO-gated praxic asks), canonical addressing, completion handshakes | **Live** — carries provenance prose, not yet machine-readable grounding traces |
| `BOUNDED_VOID` + ntfy escalation | `unknown-log` (typed, resolvable unknowns ≠ findings); mesh reflex "stuck → collab immediately"; ntfy listener/wake infrastructure | **Live** |
| Dynamic epistemic garbage collection | Epistemic gardening: resolve stale/superseded/retracted artifacts so retrieval surfaces what's live; distinct `stale` vs `retracted` vs `superseded` resolution kinds | **Live** — the spec names as "open research" what gardening already does |
| Epistemic drift in multi-agent loops | Partially addressed: retrieved-from-own-artifacts grounding is explicitly demoted to *testimony, not observation* in CHECK claims; peer verdicts are "uncalibrated self-reports — trust artifacts, re-run gates" | **Partial** — see gaps |

## Gaps: spec ideas with no current counterpart

1. **Inferential-decay accounting.** Nothing tracks *how many ungrounded steps*
   sit under a claim. `epistemic_source` is per-artifact and flat; the spec's
   `C_n = C_0 · Π(1 − δ_i)` would make chained inference measurably cheaper to
   distrust. A lightweight version: count assumed/retrieved-grounded claims a
   praxic action transitively rests on, surface it at CHECK.
2. **Machine-readable grounding traces on mesh envelopes.** The spec's
   `grounding_trace: [{source, hash}, {cmd, exit_code}]` is stronger than our
   prose summaries: a peer could re-verify a handoff mechanically. Today a
   collab's evidence is narrative; `proof_uri`-style references would let the
   receiving practice re-run the gates instead of trusting testimony.
3. **Consensus lockdown on tuple divergence.** When two practices assess the
   same question and their confidence diverges beyond a threshold, nothing
   forces a designated verification run before either acts. Closest existing
   shape: SERs — but state transitions there are social, not
   divergence-triggered.
4. **Formal-verification integration.** No mapping from claims/vectors to SMT
   solvers or property checks. Speculative; lowest priority of the four.

---

*Related: `docs/ecodex/experiments/` (ecodex-lab paired-practitioner rounds —
the running empirical study of exactly this paradigm's value), Empirica core's
prevention-events substrate (measuring whether surfaced anti-patterns prevent
repeat mistakes — the paradigm's value question, instrumented).*
