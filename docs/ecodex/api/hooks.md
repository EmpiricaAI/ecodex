# codex-empirica-plugin — Hook API

How the plugin binary integrates with codex's hook system.

## Plugin invocation

Codex's hook engine runs the plugin binary with the event as the first
argument. Five events have a dedicated handler; everything else goes through
`run-hook`, which runs any vendored script for any event:

```
codex-empirica-plugin pre-tool-use                     # sentinel-gate.py (the firewall)
codex-empirica-plugin post-tool-use                    # tool-failure.py
codex-empirica-plugin session-start                    # practice bootstrap, then session-init.py
codex-empirica-plugin user-prompt-submit               # tool-router.py
codex-empirica-plugin stop                             # transaction-enforcer.py
codex-empirica-plugin run-hook <EventName> <script.py> # any other script on any event
codex-empirica-plugin permission-request               # accepted, no-op
```

Each invocation reads the codex hook payload as JSON on stdin, writes any
response to stdout, and exits with a status code that codex interprets per its
hook protocol. `run-hook` resolves the script name against the vendored
`hooks_scripts/hooks/` tree and is how one event fans out to several scripts
without a Rust module per script.

## Wire-up via plugin manifest

Hooks are registered in `hooks.json` (referenced from `manifest.json`). The
schema is codex's `HookEventsToml` (`codex-rs/config/src/hook_config.rs`), the
same shape as Claude Code's `settings.json` hook block:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": ".*",
      "hooks": [{
        "type": "command",
        "command": "codex-empirica-plugin pre-tool-use",
        "timeout": 30,
        "statusMessage": "Empirica sentinel"
      }]
    }],
    "TaskCompleted": [{
      "matcher": ".*",
      "hooks": [{
        "type": "command",
        "command": "codex-empirica-plugin run-hook TaskCompleted task-completed.py",
        "timeout": 10,
        "statusMessage": "Empirica POSTFLIGHT-enforcement check"
      }]
    }]
  }
}
```

The shipped `hooks.json` wires twelve events — upstream's `PreToolUse`,
`PostToolUse`, `SessionStart`, `UserPromptSubmit`, `Stop`, `PreCompact`,
`PostCompact`, `SessionEnd`, `SubagentStart`, `SubagentStop` and ecodex's
`TaskCompleted`, `PostToolUseFailure` — with seventeen commands in all;
`PostToolUse`, `SessionStart` and `UserPromptSubmit` fan out to several
scripts. `docs/ecodex/hook-events-roadmap.md` lists the script behind each
event. `PermissionRequest` and `Interrupt` have no entry.

## Stdin payload (codex format)

Per `codex-rs/hooks/schema/generated/`. PreToolUse example:

```json
{
  "session_id": "...",
  "turn_id": "...",
  "cwd": "...",
  "transcript_path": "..." | null,
  "model": "...",
  "permission_mode": "default | acceptEdits | plan | dontAsk | bypassPermissions",
  "tool_name": "...",
  "tool_input": { ... },
  "tool_use_id": "...",
  "hook_event_name": "PreToolUse"
}
```

Other events follow the same envelope plus their own fields: `SessionStart`
carries `source` (`startup | resume | clear | compact | fork`), `PostCompact`
carries `success`, `SubagentStart` carries the child thread id, and so on.

Every hook subprocess also receives `EMPIRICA_INSTANCE_ID` set to the codex
thread id, which the Python side uses to key per-session state.

## Stdout / exit code → codex behavior

| Plugin signal | Codex interpretation |
|---|---|
| Exit 0, no stdout | Allow (no-op) |
| Exit 0, stdout JSON `{"hookSpecificOutput": {"permissionDecision": "deny", "permissionDecisionReason": "..."}}` | Block with reason |
| Exit 0, deprecated stdout `{"decision": "block", "reason": "..."}` | Block with reason |
| Exit 2, reason on stderr | Block with reason |
| Exit ≠0 ≠2 | Failed hook (codex logs error, treatment varies by event) |

Empirica's scripts emit Claude Code's flat output shape
(`{continue, context, decision, suppressOutput}`); codex validates each event's
output against a strict `additionalProperties: false` schema. Every handler
therefore routes script output through `src/translate_output.rs`, which maps
`context` to `hookSpecificOutput.additionalContext`, `decision: "block"` on
`PreToolUse` to `permissionDecision: "deny"`, drops unknown fields, and turns
an empty flat output into `{continue: true}`.

## Fail-open vs fail-closed semantics

Failure handling is **not uniform across events** — it splits along a security
boundary. The `pre-tool-use` **firewall** is a security floor and fails
**CLOSED**; the informational handlers fail **open**.

### PreToolUse firewall — fails CLOSED (security floor)

`src/hooks/pre_tool_use.rs` treats the gate as a firewall that must never
silently allow when it is broken. The policy is a pure mapping
(`firewall_outcome`) over the gate run result plus whether the gate script is
installed:

| Gate state | Firewall response | Why |
|---|---|---|
| Ran, exit `0` | Forward → allow (exit 0) | Gate approved the call. |
| Ran, exit `2` | Forward → deny (exit 2 + stderr) | Gate blocked the call. |
| Present but **unrunnable** (spawn/IO error) or crashed (exit ∉ {0,2}, e.g. python traceback → exit 1) | **Fail CLOSED** → deny (synthesized exit 2 + stderr) | A broken firewall must block, not silently allow. |
| stdin payload unreadable | **Fail CLOSED** → deny (exit 2) | Can't read the payload → can't gate → deny. |
| Genuinely **absent** (uninstalled, `FailOpenAbsent`) | Fail open → allow (exit 0) | The user opted out of the firewall; don't brick an un-gated install. |

Only a genuinely **absent** gate fails open. A gate that is **present but
broken** fails closed. codex's raw PreToolUse contract only ever blocks on
`exit 2 + non-empty stderr`, so the firewall synthesizes that shape to close
the gap.

Note the two layers: this is the Rust host's policy when the *script* cannot
run. When the script runs but hits an exception inside, `sentinel-gate.py`'s
own handler allows the action and writes `SENTINEL_CRASH: …` to stderr
(deny instead when `EMPIRICA_SENTINEL_FAIL_CLOSED` is set) — exit 0, so the
host forwards it as allow.

### Informational handlers — fail open

The other handlers (`post-tool-use`, `session-start`, `user-prompt-submit`,
`stop`, and every `run-hook` invocation) are informational:
`src/empirica_cli.rs::run_hook_script` returns `Err` for infrastructure
failures (script-file pre-check, spawn errors), and these handlers translate
`Err` → `ExitCode::SUCCESS` (fail open) so the plugin's own brokenness doesn't
strand the user. A **script-emitted** block (script exits 2 with stderr, or
stdout JSON says deny) is a deliberate decision and propagates verbatim.

## Interpreter and script location

Scripts run under the interpreter named in the shebang of the `empirica` CLI on
`PATH` (resolved on every run), with `python3` as the fallback. pipx, uv and
Homebrew install empirica into a private venv the first `python3` cannot
import from; running under that venv's interpreter is what keeps the Sentinel
on.

| Variable | Purpose |
|---|---|
| `EMPIRICA_HOOKS_DIR` | Manual **override** for the directory holding the Empirica hook scripts (dev / debugging / non-standard layouts). |

`resolve_hooks_dir()` (`src/empirica_cli.rs`) resolves the scripts directory in
three tiers, highest priority first:

1. **`$EMPIRICA_HOOKS_DIR`** — if set, used verbatim (tilde-expanded).
2. **`$PLUGIN_ROOT/hooks_scripts/hooks`** — the **normal runtime path**. codex
   sets `PLUGIN_ROOT` when invoking plugin hook commands, so the plugin runs
   the copy of the scripts bundled inside its own install.
3. **`~/.claude/plugins/local/empirica/hooks`** — last-resort fallback for a
   dev-mode run of the bare binary next to a Claude Code install.

## Per-event handlers

| Event | Plugin handler | Backed by |
|---|---|---|
| `PreToolUse` | `hooks::pre_tool_use::handle` | `sentinel-gate.py` |
| `PostToolUse` | `hooks::post_tool_use::handle`, then `run-hook` | `tool-failure.py`, `entity-extractor.py`, `truncation-legibility.py` |
| `SessionStart` | `hooks::session_start::handle`, then `run-hook` | host-side practice bootstrap (`src/practice_bootstrap.rs`), `session-init.py`, `ewm-protocol-loader.py`, `post-compact.py`, `session-monitor-arm.py` |
| `UserPromptSubmit` | `hooks::user_prompt_submit::handle`, then `run-hook` | `tool-router.py`, `context-shift-tracker.py` |
| `Stop` | `hooks::stop::handle` | `transaction-enforcer.py` |
| `TaskCompleted`, `PostToolUseFailure`, `PreCompact`, `PostCompact`, `SessionEnd`, `SubagentStart`, `SubagentStop` | `run-hook` | one script each (`docs/ecodex/hook-events-roadmap.md`) |
| `PermissionRequest` | no-op | — |

## Tests

- `cargo nextest run -p codex-empirica-plugin` — the Rust handlers, output
  translation, provisioning and practice bootstrap.
- `pytest codex-rs/codex-empirica-plugin/tests/vendored_hooks` — the vendored
  scripts against codex-shaped payloads (needs empirica importable).
- `cargo nextest run -p codex-cli --test empirica_provision` — the plugin
  provisioned and loaded by a real ecodex binary.
- `docs/ecodex/api/integration-tests.md` — the live checks and what
  `empirica diagnose --frontend ecodex` verifies.
