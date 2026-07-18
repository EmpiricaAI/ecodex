---
name: diagnose
description: >
  Walk through ecodex's full integration health (plugin install, hooks,
  sentinel, statusline, translator, providers, Rust compliance) by running
  the deterministic `empirica diagnose --frontend ecodex` checker, then
  triaging each failure with the user. Triggers when the user reports
  "statusline isn't working", "agent isn't picking up tools", "translator
  silent", "is empirica wired correctly", or asks "diagnose ecodex".
  Distinct from any generic `diagnose` skill — this one targets the
  ecodex integration surface specifically.
---

# Diagnose ecodex

## Purpose

`empirica diagnose --frontend ecodex` runs ~12 deterministic checks
covering every layer of the ecodex stack:

- Python interpreter + empirica CLI
- codex-empirica-plugin install + config enablement
- statusline runtime stdin wiring (the `Stdio::null()` regression)
- statusline script runnability
- codex-empirica-translator listening + `/healthz`
- curated model provider env_keys
- Rust compliance (cargo fmt + cargo check)

This skill walks the user through the failure list — running fixes
where safe, escalating where judgment is needed.

## How to run

The skill's job is **reasoning glue around the script's output**. The
script is the truth source. You should NOT re-implement checks in
agent reasoning — always invoke the CLI.

### Step 1 — run the doctor

```bash
empirica diagnose --frontend ecodex --output json --fast
```

`--fast` skips `cargo check` (slow). Use without `--fast` only when
the user explicitly wants a full Rust compliance pass.

### Step 2 — parse the JSON

The output shape:

```json
{
  "ok": false,
  "summary": {"PASS": 8, "FAIL": 1, "WARN": 1, "SKIP": 0},
  "checks": [
    {
      "name": "ecodex statusline runtime pipes session_id",
      "status": "FAIL",
      "detail": "plugin_statusline_runtime.rs still uses Stdio::null() ...",
      "hint": "Switch to Stdio::piped() and write {\"session_id\":...} ...",
      "data": {"file": "/path/to/plugin_statusline_runtime.rs"}
    }
  ]
}
```

### Step 3 — walk failures with the user

For each `FAIL` (then `WARN`), in order:

1. State the check name + factual `detail` to the user
2. Quote the `hint` as the proposed fix
3. Decide based on **fix risk**:
   - **Safe + reversible** (file write to user's home, `chmod`, install
     script invocation): propose to run, apply on confirm
   - **Code edits in a project repo**: propose to read the file +
     suggest the edit, but DO NOT auto-edit without user consent
     (transaction discipline — code edits are praxic and want
     PREFLIGHT scope declared first)
   - **Network or stateful** (start/restart translator, install
     packages): describe the command, run on confirm only
4. After the fix, re-run the same check (`empirica diagnose --frontend
   ecodex --output json --fast` and grep the relevant check) to verify
5. If the re-run still fails: escalate to the user with what was tried +
   what the residual symptom is. Do not loop.

### Step 4 — finish

When all FAILs are resolved (and WARNs at user's discretion), run
once more without `--output json` so the user sees the human-readable
green report.

## Decision tree per status

| Status | What it means | Action |
|---|---|---|
| `PASS` | Check succeeded | Skip — don't bring it up |
| `FAIL` | Hard failure; integration broken at this layer | Propose fix → confirm → apply → re-check |
| `WARN` | Soft issue; integration works but degraded (e.g. translator without /healthz still serves traffic) | Mention; propose fix as optional |
| `SKIP` | Check couldn't run (missing dependency on another check) | Mention so user knows the coverage gap, but only if upstream FAIL |

## Anti-patterns

- **Don't** describe a check as "passed" without running the CLI.
  The whole point of this skill is grounding in deterministic output.
- **Don't** edit code in the ecodex repo to fix a `FAIL` without
  PREFLIGHTing a transaction first. Treat code edits as praxic work
  with the same discipline as any other coding task.
- **Don't** restart the translator without verifying nothing
  else is depending on the existing process (e.g. don't kill it
  during another active turn).
- **Don't** re-implement check logic in your reasoning. If a check
  is missing, log a goal to add it to `diagnose_ecodex.py`.

## Adding a new check

When the user reports a new failure mode that isn't currently covered:

1. Don't just remember it — log it as a goal to add a check.
2. Suggest the check shape: `def check_ecodex_<thing>() -> CheckResult:
   ...`. Each new check is a tiny PR against
   `empirica/cli/command_handlers/diagnose_ecodex.py`.
3. The goal should reference `docs/diagnose-ecodex.md`'s "Adding a new
   check" section.

## Pointers

- Truth-source code: `empirica/cli/command_handlers/diagnose_ecodex.py`
- Doc: `empirica/docs/diagnose-ecodex.md`
- Sister command: `empirica doctor` for desktop / MCP install
