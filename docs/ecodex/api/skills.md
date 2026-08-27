# codex-empirica-plugin — Skills

The empirica plugin registers a curated skill set with codex. Skills are loaded from `./skills/` (referenced by the manifest's `skills` field).

## Registered skills

10 skills, mirrored from the Claude Code empirica plugin (same `SKILL.md` format, identical content):

| Skill | Purpose |
|---|---|
| `empirica-constitution` | Operational governance framework — routes situations to the right Empirica mechanism |
| `epistemic-transaction` | Plan complex multi-step work as measured PREFLIGHT→CHECK→POSTFLIGHT transactions |
| `epistemic-persistence-protocol` | Hold positions under user pushback with calibrated backbone (replaces sycophancy) |
| `code-audit` | Structured code-quality investigation (duplication, dead code, complexity) producing Empirica artifacts |
| `code-docs-align` | Verify documentation, docstrings, comments, and ref-docs match current code state |
| `dispatch-agent` | Spawn subagents with inherited Empirica context (findings, dead-ends, anti-patterns) |
| `ewm-interview` | Interview users to discover goals, domains, tools, preferences; generate workflow-protocol.yaml |
| `render` | Render markdown with ASCII art diagrams to themed SVG via mdview |
| `diagnose` | Walk ecodex's integration health (plugin install, hooks, sentinel, statusline, translator, providers) via the deterministic `empirica diagnose --frontend ecodex` checker, then triage each failure |
| `onboard` | ecodex first-run onboarding + diagnostics orchestrator — composes `empirica diagnose`, `empirica onboard`, and `empirica setup-claude-code`, filling gaps (model-server probe, mode selection, per-project practice setup) |

## Format

Each skill is a directory under `skills/` containing a `SKILL.md` file with YAML frontmatter:

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

The loader (`codex-rs/ext/skills/src/loader/host.rs`, field on `codex-rs/skills/src/model.rs::SkillMetadata`) parses `name`, `description`, `metadata.short-description`, and `pinned`:

- **`pinned: true`** — marks a FRAMEWORK skill: session-wide standing policy rather than a per-task tool. Used by `empirica-constitution`, `epistemic-transaction`, and `epistemic-persistence-protocol`. The skill body is **not** auto-injected (earlier ecodex releases reinjected pinned bodies each context window; that behavior was dropped when upstream removed its skill-injection mechanism — see CHANGELOG): the skills catalog prompt instead instructs the model to proactively read framework `SKILL.md` files and re-read them after `/compact`, and anything needing guaranteed ambience routes through codex's native AGENTS.md instructions channel. Default is `false` (progressive disclosure).

A `version:` field is **not** consumed by the loader — including one is harmless but ignored.

This format is identical to Claude Code's skill format and to codex's bundled samples (`codex-rs/skills/src/assets/samples/skill-creator/SKILL.md` etc.). No translation required between hosts.

## Manifest registration

`manifest.json` references the directory:

```json
{
  "skills": "./skills"
}
```

Codex discovers each subdirectory's `SKILL.md` and registers it as a callable skill scoped to the plugin's namespace (`empirica:<skill-name>`).

## Source of truth + sync convention

The canonical source for these skills is the Empirica core repo (currently at `/home/yogapad/.claude/plugins/local/empirica/skills/`). The copies under `codex-rs/codex-empirica-plugin/skills/` are **mirrored** from that source.

**Workflow:**
1. Edit skills upstream in the Empirica core repo.
2. Re-mirror into the ecodex plugin via `cp -r` (a future `scripts/sync-skills.sh` will automate this).
3. Commit the mirrored update on the `build/v1-plugin` branch (or wherever skill changes are landing).

This mirrors the broader Empirica → ecodex relationship: empirica is the source of truth, ecodex's plugin distribution is a derivative kept in sync.

## Skills NOT included in v1

Some skills present in the Empirica plugin are intentionally excluded or deferred:

| Excluded skill | Why |
|---|---|
| `using-superpowers` (foundational) | Loaded as part of session bootstrap, not user-callable |
| Various `*-deprecated` skills | Aliased to their replacements |

If the Empirica plugin grows new skills upstream, sync them in the next iteration.

## Future iteration

- `scripts/sync-skills.sh` to automate the mirror (with diff against upstream + warning if upstream is ahead)
- Skill-specific tests (validate frontmatter, validate `description` is a strong trigger string)
- Per-skill metadata extension if codex's plugin system gains additional frontmatter fields beyond the currently-parsed name/description/metadata.short-description/pinned
