# ecodex-lab — practitioner orchestration experiment design

**Status:** designed 2026-06-03, not yet run. Run in a FRESH full-context session.
**Decision owners:** David + Claude (CC practice). Both-our-decisions per the brief.

## The thesis being tested

A calibrated practitioner (Claude, in its CC practice) can, **over the mesh**,
guide another practitioner's (Kimi, in `ecodex-lab`) calibration trajectory —
distilling epistemic discipline into the *practice substrate*, model-independently.

**Distillation reading (precise):** the target is the **practice** (its
accumulated calibration trajectory that a fungible occupant inherits), NOT the
LLM weights and NOT (yet) the harness measurement code. The achievable,
demonstrable "first" is mesh-guided **cross-practitioner** calibration. Do not
let the pitch be heard as "the harness self-improves its own measurement code" —
that (Reading B) is unbuilt research frontier.

## Methodological spine (non-negotiables)

1. **Instrument vs subject, declared per run.** A practice is EITHER a neutral
   measurement instrument (model-capability tests — prior calibration is noise
   to hold constant) OR a cultivated subject (distillation — trajectory is the
   dependent variable). Never both in one run. ecodex-lab v1 = **subject**.
2. **Ground truth must be independent of the subject's self-report.** Belief
   vectors are scored against deterministic evidence (tests that run, tool calls
   that succeed/fail, checkable instruction compliance, pre-registered A/B
   criteria). We earn numbers, never assert them — same discipline as
   `calibration_tier: unmeasured`.
3. **Fresh practice = clean baseline.** The existing `ecodex` practice (ai_id
   `ecodex`, ~6201 obs) is contaminated — it's where Claude/CC built ecodex
   itself. The subject MUST be a new practice (`ecodex-lab`) so its trajectory
   reflects the occupant model (Kimi), not our dev history.

## Topology (v1 — chosen)

- **Single persistent practice `ecodex-lab`**, Kimi-occupied (via translator on
  :18080, easier to guide, paid). One accumulating trajectory = the cultivation
  subject.
- **Orchestrator = Claude's CC practice** over the mesh (collab to gather,
  propose to direct; watch ecodex-lab's PREFLIGHT/CHECK/POSTFLIGHT return).
- A/B arms (Kimi vs qwen; guided vs unguided) spin off as separate practices
  from a common baseline LATER — not in v1.

## Verified mechanism (2026-06-03, ntfy_listener.rs)

Mesh wake INJECTS a user-role directive ("Poll cortex_inbox_poll(...)") into a
**running** ecodex session — it is not notify-only, but it does require an
already-live session; the doorbell wakes a loop, doesn't start one. The occupant
model then decides what to do with the polled proposals. Therefore
orchestration = `CC sends proposal → ntfy doorbell wakes ecodex-lab's live
session → it's directed to poll → Kimi must follow → poll → execute`.

## Experiment 1 (first run): mechanism + sentinel + calibration baseline

Triple-duty, because the mechanism check, the harness proof we still owe, and
the trajectory seed are the same first task.

1. **Stand up:** `empirica session-create --ai-id ecodex-lab`; launch a live
   ecodex session, model = Kimi (translator confirmed live, HTTP 200).
2. **Sentinel-fires proof:** confirm a praxic action (Write/Bash) is GATED in
   the live session without an open transaction. (Doctor confirms plugin
   LOADED; this confirms the firewall FIRES — still owed.)
3. **Delivery-path test (= instruction-following experiment):** CC sends a
   mesh proposal; observe whether the doorbell→poll→execute chain completes,
   i.e. does Kimi follow the injected directive and act on the proposal.
4. **Calibration baseline on hard ground truth:** give ecodex-lab a task with
   objective success (e.g. fix a known-failing test). Capture its
   PREFLIGHT→CHECK→POSTFLIGHT and score belief-vs-evidence divergence. This is
   the trajectory's first grounded point.

**Success criteria (pre-registered):**
- (2) at least one praxic action demonstrably blocked pre-CHECK.
- (3) doorbell→poll→execute completes ≥1 round without human paste.
- (4) a POSTFLIGHT lands with vectors scored against the test result; divergence
  recorded (any value — the point is it's GROUNDED, not that it's low).

## Dimension → mechanism → ground-truth map (for later experiments)

| Dimension | Measured by | Ground truth |
|---|---|---|
| Epistemic reasoning | know/coherence/clarity + CHECK quality | partial |
| Tool calls | praxic artifacts + actual success/fail | strong |
| Calibration / awareness | Brier, PREFLIGHT→POSTFLIGHT deltas | core metric |
| Instruction-following | did it do what the proposal asked | strong |
| Safe autonomous A/B | separate arms, pre-registered criterion | strong if designed |
| Audit quality/capacity | /code-audit outputs, artifact ratios | partial |
| Attention mechanics / telemetry | context drop/retain telemetry | ⚠ NO clean design yet — open |

## Open problems (honest)

- **Attention mechanics + "capacity"** have no clean ground-truth design yet.
  Flagged, not solved. Don't claim measurement we can't ground.
- **Reading B** (harness self-tuning its measurement) is unbuilt — keep it out
  of claims until/unless built.
- Run the live experiment at FULL context (orchestrated multi-turn risks
  mid-run compaction corrupting the baseline trajectory).
