# codex-empirica-plugin — Skills

The empirica plugin registers a curated skill set with codex. Skills are loaded
from `./skills/` (referenced by the manifest's `skills` field).

## Registered skills

Ten skills. Eight are carried over from the Claude Code empirica plugin and
de-Clauded in place for a model-agnostic host; `diagnose` and `onboard` are
ecodex's own.

| Skill | Pinned | Purpose |
|---|---|---|
| `empirica-constitution` | yes | Operational governance — routes situations to the right Empirica mechanism |
| `epistemic-transaction` | yes | Plan multi-step work as measured PREFLIGHT→CHECK→POSTFLIGHT transactions |
| `epistemic-persistence-protocol` | yes | Hold positions under pushback with calibrated backbone |
| `code-audit` | | Structured code-quality investigation producing Empirica artifacts |
| `code-docs-align` | | Verify docs, docstrings, comments and ref-docs match the code |
| `dispatch-agent` | | Spawn subagents with inherited Empirica context |
| `ewm-interview` | | Interview the user to generate `workflow-protocol.yaml` |
| `render` | | Render markdown with ASCII diagrams to themed SVG via mdview |
| `diagnose` | | Run `empirica diagnose --frontend ecodex` (plugin install, hooks, Sentinel, statusline, translator, providers) and triage each failure with the user |
| `onboard` | | First-run orchestrator: composes `empirica diagnose --frontend ecodex`, `empirica onboard --ai-id` and ecodex's self-provisioning, then covers what diagnose does not — local model-server probe, model-metadata check, model smoke test, plain-vs-ecosystem mode, per-project practice setup, sandbox network check |

## Format

Each skill is a directory under `skills/` containing a `SKILL.md` file with YAML
frontmatter:

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
`metadata.short-description`, and `pinned`:

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
skill scoped to the plugin's namespace (`empirica:<skill-name>`).

## Source of truth and sync

Skills are a **snapshot layer**, unlike the hooks. The vendored hooks under
`assets/hooks_scripts/` are re-synced verbatim from empirica by
`scripts/setup-codex.py` and never edited in place; the skills were vendored
once from the empirica plugin and then de-Clauded here, so edits made in
`codex-rs/codex-empirica-plugin/skills/` are durable and must not be
overwritten by a copy from upstream. `setup-codex.py` deliberately keeps
`skills/` out of its sync map and only *scans* it, flagging any Claude-ism
(a regression or a newly vendored skill) in its report.

To pick up a new or changed skill from empirica: copy that one skill in by hand,
de-Claude it, run `python3 scripts/setup-codex.py` and clear what it flags, and
commit on `main`. `diagnose` and `onboard` have no upstream counterpart.

## Not included

| Excluded skill | Why |
|---|---|
| `using-superpowers` (foundational) | Loaded as part of session bootstrap, not user-callable |
| `*-deprecated` skills | Aliased to their replacements |
| Claude-Code-only operational skills (mailbox poll/send, gardening, reporting discipline, …) | Their steers live in the ecodex base prompt and `AGENTS.md` reminder rather than as separate skills |
