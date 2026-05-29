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

The plugin distinguishes two failure modes:

| Failure | Plugin response | Why |
|---|---|---|
| **Infrastructure** — script file missing, python3 won't spawn, stdin IO error | Fail open: log to stderr, exit 0 | Plugin's own brokenness shouldn't strand the user. Matches Empirica sentinel-gate's own fail-open posture. |
| **Script-emitted block** — script exits 2 with stderr, OR script stdout JSON says deny | Propagate: exit 2 + stderr verbatim, OR exit 0 + JSON | This is a deliberate block decision; it must reach codex. |

Implementation: `src/empirica_cli.rs::run_hook_script` returns `Err` for infrastructure failures (script-file pre-check, spawn errors). Hook handlers translate `Err` → `ExitCode::SUCCESS`. Script-returned exit codes propagate via `Ok(HookOutput { exit_code, .. })`.

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `EMPIRICA_HOOKS_DIR` | `~/.claude/plugins/local/empirica/hooks` | Directory containing the Empirica Python hook scripts to subprocess to |

The default points at the existing CC empirica install. Future ecodex distribution will bundle hooks under the codex plugin install path and update this default accordingly.

## Per-hook status

| codex event | Plugin handler | Backed by | Status |
|---|---|---|---|
| `pre-tool-use` | `hooks::pre_tool_use::handle` | `sentinel-gate.py` | ✅ live |
| `stop` | `hooks::stop::handle` | `transaction-enforcer.py` | ✅ live |
| `post-tool-use` | (stub) | (planned: `tool-failure.py`) | stub |
| `session-start` | (stub) | (planned: `session-init.py`) | stub |
| `user-prompt-submit` | (stub) | (planned: `tool-router.py`) | stub |
| `permission-request` | (stub) | (codex-specific; design TBD) | stub |

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
