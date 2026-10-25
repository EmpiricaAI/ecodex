# codex-empirica-plugin — Skills

The empirica plugin registers a curated skill set with codex. Skills are loaded
from `./skills/`, which the manifest's `skills` field points at, and are compiled
into the plugin binary, so every ecodex release ships the set it was built with.

## Registered skills

Sixteen skills. Fourteen come from the Claude Code empirica plugin: thirteen were
adapted in place for a model-agnostic host, and `eat-the-broccoli` is carried
verbatim because its catalog is written for any host. `diagnose` and `onboard` are
ecodex's own.

| Skill | Pinned | Purpose |
|---|---|---|
| `epistemic-transaction` | yes | Plan multi-step work as measured PREFLIGHT→CHECK→POSTFLIGHT transactions |
| `epistemic-persistence-protocol` | yes | Hold positions under pushback with calibrated backbone |
| `empirica-constitution` | | Index: routes each moment to the Empirica skill that covers it, plus recovery steps when the machinery breaks |
| `artifact-graph` | | Typing, connecting and retracting artifacts: type collapse, split before you log, provenance and visibility, orphans, the cognitive immune system |
| `practice-model` | | Which practice work belongs to: `--project-id`, remote or client machines, the entity registry, practice versus repository |
| `authoring-guidance` | | Where guidance belongs (tool, skill or prompt) and the rules for writing it |
| `code-audit` | | Structured code-quality investigation producing Empirica artifacts |
| `code-docs-align` | | Verify docs, docstrings, comments and ref-docs match the code |
| `dispatch-agent` | | Spawn subagents with inherited Empirica context |
| `eat-the-broccoli` | | Quality sweep: deterministic tooling plus a pattern catalog for failures that pass every test |
| `pre-publish` | | Ordered gates before a push, tag or release: scope, deterministic tier, independent sweep with every finding traced, hot path, gaps, CI on the exact commit, cut and verify |
| `forecast` | | Turn "what comes next" into graded predictions: evidence kind, capped confidence, at most three questions with predicted answers, planned goals only after the user confirms, and gardening of past predictions |
| `ewm-interview` | | Interview the user to generate `workflow-protocol.yaml` |
| `render` | | Render markdown with ASCII diagrams to themed SVG via mdview |
| `diagnose` | | Run `empirica diagnose --frontend ecodex` (plugin install, hooks, Sentinel, statusline, translator, providers) and triage each failure with the user |
| `onboard` | | First-run orchestrator: composes `empirica diagnose --frontend ecodex`, `empirica onboard --ai-id` and ecodex's self-provisioning, then covers what diagnose does not — local model-server probe, model-metadata check, model smoke test, plain-vs-ecosystem mode, per-project practice setup, sandbox network check |

`pre-publish` carries two reference files beside its `SKILL.md`:
`references/sweep-brief.md`, the brief each read-only reviewer gets, and
`references/ecodex-release.md`, ecodex's own release procedure. For any other
project the skill tells the model to use that project's release doc instead.

## Format

Each skill is a directory under `skills/` containing a `SKILL.md` file with YAML
frontmatter, plus any files its body refers to by relative path:

```markdown
---
name: <skill-id>
description: "<when to use this skill — the trigger conditions>"
pinned: <bool>            # optional; default false
metadata:
  short-description: "<one-line summary>"   # optional
---

# Skill Title

<skill body — instructions, examples, references>
```

The loader (`codex-rs/ext/skills/src/loader/host.rs`, fields on
`codex-rs/skills/src/model.rs::SkillMetadata`) parses `name`, `description`,
`metadata.short-description`, and `pinned`. `name` is capped at 64 characters and
`description` at 1024 (`codex-rs/skills/src/interface.rs`); the description is what
the skills catalog shows the model every turn, so it carries the trigger phrases.

- **`pinned: true`** marks a *framework* skill: session-wide standing policy
  rather than a per-task tool. The body is **not** auto-injected — the skills
  catalog prompt tells the model to read framework `SKILL.md` files early and
  re-read them after a compaction, and anything that must always be present
  rides in codex's native `AGENTS.md` channel (the reminder block the plugin
  seeds). Earlier ecodex builds re-injected pinned bodies each context window;
  that stopped when upstream removed its skill-injection mechanism. Default is
  `false` (progressive disclosure).

A `version:` field is **not** consumed by the loader — including one is
harmless but ignored.

This format is identical to Claude Code's skill format and to codex's bundled
samples (`codex-rs/skills/src/assets/samples/`). No translation between hosts.

## Manifest registration

`manifest.json` references the directory:

```json
{
  "skills": "./skills"
}
```

Codex discovers each subdirectory's `SKILL.md` and registers it as a callable
skill scoped to the plugin's namespace, which is the manifest `name` verbatim:
`empirica@empiricaAI:<skill-name>` (`codex-rs/core-plugins/src/loader.rs` sets
the namespace, `ext/skills/src/loader/namespace.rs` qualifies the name).

The directory is embedded at build time and written into the plugin cache on the
first session start after an upgrade. A skill edited in the repo therefore reaches
a running install only through a new build; sessions already running keep the
copy they loaded.

## Source of truth and sync

Skills are a **snapshot layer**, unlike the hooks. The vendored hooks under
`assets/hooks_scripts/` are re-synced verbatim from empirica by
`scripts/setup-codex.py` and never edited in place; the skills were vendored
from the empirica plugin and then adapted here, so edits made in
`codex-rs/codex-empirica-plugin/skills/` are durable and must not be
overwritten by a copy from upstream. `setup-codex.py` deliberately keeps
`skills/` out of its sync map and only *scans* it, flagging any host-specific
wording (a regression or a newly vendored skill) in its report.

To pick up a new or changed skill from empirica: copy that one skill in by hand,
adapt it, run `python3 scripts/setup-codex.py` and clear what it flags, and commit
on `main`. Two exceptions:

- **`eat-the-broccoli`** is not adapted. Copy empirica's file over it, put back
  the vendor notice at its top, which records the adaptations a sync must
  re-apply, and run the scan. Its catalog rows tell real incidents, some of which
  name Claude Code, so the scan flags it by design.
- **`pre-publish`** keeps ecodex's `references/ecodex-release.md`, which has no
  upstream counterpart. Empirica's own release reference is not bundled.

`diagnose` and `onboard` have no upstream counterpart.

## Not included

| Excluded skill | Why |
|---|---|
| `using-superpowers` (foundational) | Loaded as part of session bootstrap, not user-callable |
| `*-deprecated` skills | Aliased to their replacements |
| Claude-Code-only operational skills (mailbox poll/send, gardening, reporting discipline, …) | Their steers live in the ecodex base prompt and `AGENTS.md` reminder rather than as separate skills |
