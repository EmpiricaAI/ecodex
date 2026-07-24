# codex-empirica-plugin — Hook API

How the plugin binary integrates with codex's hook system.

## Plugin invocation

Codex's hook engine invokes our plugin binary by event name as the first arg:

```
codex-empirica-plugin pre-tool-use
codex-empirica-plugin post-tool-use
codex-empirica-plugin session-start
codex-empirica-plugin user-prompt-submit
codex-empirica-plugin stop
codex-empirica-plugin permission-request
```

Each invocation reads the codex hook payload as JSON on stdin, writes any response to stdout, and exits with a status code that codex interprets per its hook protocol.

## Wire-up via plugin manifest

Hooks are registered in `hooks.json` (referenced from `manifest.json`):

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
    "Stop": [...]
  }
}
```

Schema follows codex's `HookEventsToml` (`codex-rs/config/src/hook_config.rs:31`). Identical shape to Claude Code's `settings.json` hook block.

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

Other events (Stop, SessionStart, etc.) follow the same envelope minus the per-event-specific fields.

## Stdout / exit code → codex behavior

| Plugin signal | Codex interpretation |
|---|---|
| Exit 0, no stdout | Allow (no-op) |
| Exit 0, stdout JSON `{"hookSpecificOutput": {"permissionDecision": "deny", "permissionDecisionReason": "..."}}` | Block with reason |
| Exit 0, deprecated stdout `{"decision": "block", "reason": "..."}` | Block with reason |
| Exit 2, reason on stderr | Block with reason |
| Exit ≠0 ≠2 | Failed hook (codex logs error, treatment varies by event) |

## Fail-open vs fail-closed semantics

Failure handling is **not uniform across events** — it splits along a security
boundary. The `pre-tool-use` **firewall** is a security floor and fails
**CLOSED**; the informational (non-firewall) handlers fail **open**.

### PreToolUse firewall — fails CLOSED (security floor)

`src/hooks/pre_tool_use.rs` treats the gate as a firewall that must never
silently allow when it is broken. The policy is a pure mapping
(`firewall_outcome`, lines 47-65) over the gate run result plus whether the
gate script is installed:

| Gate state | Firewall response | Why |
|---|---|---|
| Ran, exit `0` | Forward → allow (exit 0) | Gate approved the call. |
| Ran, exit `2` | Forward → deny (exit 2 + stderr) | Gate blocked the call. |
| Present but **unrunnable** (spawn/IO error) or crashed (exit ∉ {0,2}, e.g. python traceback → exit 1) | **Fail CLOSED** → deny (synthesized exit 2 + stderr) | A broken firewall must block, not silently allow. |
| stdin payload unreadable | **Fail CLOSED** → deny (exit 2) | Can't read the payload → can't gate → deny. |
| Genuinely **absent** (uninstalled, `FailOpenAbsent`) | Fail open → allow (exit 0) | The user opted out of the firewall; don't brick an un-gated install. |

The key distinction (lines 57-62, 115-138): only a genuinely **absent** gate
fails open. A gate that is **present but broken** fails closed. codex's raw
PreToolUse contract only ever blocks on `exit 2 + non-empty stderr`, so the
firewall synthesizes that shape to close the gap.

### Informational handlers — fail open

The non-firewall handlers (`post-tool-use`, `session-start`,
`user-prompt-submit`, `stop`) are informational: `src/empirica_cli.rs::run_hook_script`
returns `Err` for infrastructure failures (script-file pre-check, spawn errors),
and these handlers translate `Err` → `ExitCode::SUCCESS` (fail open) so the
plugin's own brokenness doesn't strand the user. A **script-emitted** block
(script exits 2 with stderr, or stdout JSON says deny) is a deliberate decision
and propagates verbatim via `Ok(HookOutput { exit_code, .. })`.

## Configuration

| Variable | Purpose |
|---|---|
| `EMPIRICA_HOOKS_DIR` | Manual **override** for the directory containing the Empirica Python hook scripts to subprocess to (dev / debugging / non-standard layouts). |

`EMPIRICA_HOOKS_DIR` is an override, not the default. `resolve_hooks_dir()`
(`src/empirica_cli.rs:128-136`) resolves the scripts directory in **three
tiers**, highest priority first:

1. **`$EMPIRICA_HOOKS_DIR`** — if set, used verbatim (tilde-expanded). Manual
   override for dev / debugging / non-standard layouts.
2. **`$PLUGIN_ROOT/hooks_scripts/hooks`** — the **normal runtime path**. codex
   sets `PLUGIN_ROOT` when invoking plugin hook commands, so the plugin runs
   the copy of the scripts bundled inside its own install.
3. **`~/.claude/plugins/local/empirica/hooks`** — last-resort fallback for
   coexisting CC-empirica installs / dev-mode runs of the bare binary when
   neither of the above is set.

So under a normal codex plugin install, tier 2 (the `PLUGIN_ROOT`-relative
bundled path) is what's used; the CC path is only a fallback.

## Per-hook status

| codex event | Plugin handler | Backed by | Status |
|---|---|---|---|
| `pre-tool-use` | `hooks::pre_tool_use::handle` | `sentinel-gate.py` | ✅ live |
| `stop` | `hooks::stop::handle` | `transaction-enforcer.py` | ✅ live |
| `post-tool-use` | `hooks::post_tool_use::handle` | `tool-failure.py` | ✅ live |
| `session-start` | `hooks::session_start::handle` | `session-init.py` | ✅ live |
| `user-prompt-submit` | `hooks::user_prompt_submit::handle` | `tool-router.py` | ✅ live |
| `permission-request` | (no-op) | (codex-specific; design TBD) | stub |

All five event handlers are wired as canonical handlers in `src/main.rs`
(the `match event.as_str()` dispatch): `pre-tool-use`, `post-tool-use`,
`session-start`, `user-prompt-submit`, and `stop` each dispatch to their
handler module. Only `permission-request` is a genuine no-op stub —
`src/main.rs` returns `ExitCode::SUCCESS` for it directly (design TBD).

## Smoke test results (2026-05-02)

T7 mini smoke test (binary-only; no live codex integration yet):

| Test | Expected | Result |
|---|---|---|
| No args | Usage to stderr, exit 64 | ✅ |
| Bogus event | Usage to stderr, exit 64 | ✅ |
| `pre-tool-use` with mock script (exit 0 + JSON stdout) | Forward stdin, propagate stdout, exit 0 | ✅ |
| `stop` with mock script | Forward stdin, exit 0 | ✅ |
| Stub event (`post-tool-use`) | Silent, exit 0 | ✅ |
| `pre-tool-use` with missing hooks dir | Fail open: warn to stderr, exit 0 | ❌→✅ (initially exited 2; fixed in T7 by adding script-exists pre-check in `empirica_cli.rs`) |
| `pre-tool-use` with script that intentionally blocks (exit 2 + stderr) | Propagate: exit 2 + stderr verbatim | ✅ |

**Live integration smoke test** (loading the plugin into a running codex) is deferred to a future transaction — requires building the codex binary itself and configuring plugin discovery against `~/.codex/plugins/cache/empirica/`.
