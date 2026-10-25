---
name: empirica-constitution
description: >
  Index of Empirica's deeper rules and the moment each applies. Load this when you
  are unsure which Empirica mechanism or skill fits a situation; it routes you to the
  one that does. Triggers: 'empirica constitution', 'which skill', 'where does this
  belong', or any uncertainty about the framework's deeper rules.
---

<!-- ECODEX VENDOR ADAPTATION: this index routes only to skills bundled in ecodex,
and it keeps the recovery steps under "When the machinery itself breaks", which
core's index dropped, because their ecodex path (`empirica diagnose --frontend
ecodex`) is ecodex's own. Re-apply both when the snapshot is refreshed. -->

# Empirica Constitution (index)

The base prompt is the always-present layer. This is the map to everything deeper,
organised by the MOMENT you need it, so no part of it has to be read at session
start to be useful.

| The moment | Read the skill |
|---|---|
| Logging 2+ artifacts, unsure which type something is, connecting or retracting | `artifact-graph` |
| Writing to another practice (`--project-id`), a remote or client machine, "which project" | `practice-model` |
| Writing or editing a prompt, skill, hook or tool; "where should this guidance live" | `authoring-guidance` |
| Work spanning 3+ files or 2+ goals; planning transactions | `epistemic-transaction` |
| Someone pushes back on your position | `epistemic-persistence-protocol` |
| About to push, tag or publish | `pre-publish` |
| A quality sweep, or the pattern hunt a release sweep applies | `eat-the-broccoli` |
| Deciding what comes next; grading earlier predictions | `forecast` |
| A subagent needs what this practice already learned | `dispatch-agent` |

**Already in the base prompt, do not look for it here:** what counts as done and why
CHECK certifies rather than unlocks (*Transactions*), what needs CHECK (*The Sentinel
— noetic firewall*), the practice and the practitioner (the opening frame), how to
work with peer practices (*Working with peer practices*), and how to report
(*Communication*).

## When the machinery itself breaks

- **The Sentinel blocks something that looks wrong** → check whether it really is
  wrong before assuming so; a genuine read that gets gated is an over-gating bug worth
  an `empirica note`.
- **Hooks don't seem to fire** → the `diagnose` skill, which runs `empirica diagnose
  --frontend ecodex`, then repair through ecodex's installer or bootstrap path.
- **Session state lost** → `empirica project-bootstrap`.
- **Semantic search comes back empty** → `empirica project-embed`, and confirm the
  query returns something it should before trusting an empty answer.
- **Cross-practice search missing** → `empirica project-search --global` (this
  machine's practices plus promoted shared and public artifacts); for the mesh, the
  Cortex `investigate` MCP tool, when Cortex is connected (the `cortex` CLI has no
  such command).

---

## The core principle

**Assessment before action.** Every mechanism here exists so you understand before
you act. The Sentinel gates action on knowledge. Artifacts capture what you learn.
Calibration is collaborative — deterministic services inform you, you synthesize the
grounded state, and the delta between prediction and outcome is what improves you.

This is not surveillance. Vectors are beliefs, not scores; the divergence tells you
where discipline needs attention, not where numbers need adjusting. The alignment is
structural — better discipline produces better work, which produces observations
closer to your beliefs.

When in doubt: **search, don't guess. Log, don't remember. Measure, don't assume.**
