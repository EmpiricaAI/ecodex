# Codex Hook Events — ecodex Divergence Roadmap

Tracks the 7 hook events ecodex adds on top of stock codex's 6, and where each one's dispatch site needs to be wired in `codex-rs/core/` for the event to actually fire.

**Status (2026-05-17):**
- ✅ **Schema landed** (PR `<commit>`): `HookEventName` enum extended; exhaustive matches updated; `hooks.json` accepts the new event names without rejection. Empirica plugin can declare handlers today.
- ❌ **Dispatch sites pending**: events are declarable but won't fire until each one's lifecycle-point patch lands in `codex-rs/core`. Tracked under goal `f0004294`.

## Stock codex events (already wired)

| Event | Lifecycle point | empirica handler |
|---|---|---|
| `PreToolUse` | Before any tool invocation | `sentinel-gate.py` |
| `PermissionRequest` | When tool needs permission | (no-op for empirica) |
| `PostToolUse` | After successful tool invocation | `tool-router.py` + `entity-extractor.py` |
| `SessionStart` | New or resumed codex session | `session-init.py` + siblings |
| `UserPromptSubmit` | User submits a prompt | `tool-router.py` + 5 siblings |
| `Stop` | Agent turn ends | `transaction-enforcer.py` |

## ecodex additions (schema landed, dispatch pending)

Listed with the file:line where the dispatch call needs to be inserted in `codex-rs/core` (best guess from current architecture — confirm during implementation).

### `PreCompact` / `PostCompact`

**Lifecycle:** Just before / just after codex compacts the conversation history (when token usage approaches context limit).

**Why:** Compaction destroys epistemic state in working memory. Pre-compact hook lets empirica snapshot transaction state, vectors, recent artifacts to `~/.empirica/breadcrumbs/`. Post-compact restores via system-message injection.

**Dispatch site (TBC):** `codex-rs/core/src/compact.rs` — find the `compact_conversation()` entrypoint, fire `PreCompact` just before the truncation, `PostCompact` just after the new history is sized down.

**Handler:** `pre-compact.py` (snapshots), `post-compact.py` (restores via SessionStart fan-out already in place).

### `SessionEnd`

**Lifecycle:** When a codex session terminates (user `/quit`, signal, parent process death, etc.).

**Why:** Symmetric counterpart to `SessionStart`. Lets empirica run final POSTFLIGHT, capture session snapshot, curate the conversation rollout.

**Dispatch site (TBC):** `codex-rs/core/src/session/mod.rs` — find the session shutdown path (likely a `Drop` impl or explicit `close()`). Fire `SessionEnd` with the final session state in the payload.

**Handlers:** `session-end-postflight.py`, `curate-snapshots.py`.

### `SubagentStart` / `SubagentStop`

**Lifecycle:** When a subagent (Agent tool spawn) is created / completes.

**Why:** Subagents inherit epistemic context but run their own transactions. Empirica needs to track the parent→child relationship and merge findings back on completion.

**Dispatch site (TBC):** `codex-rs/core/src/agent/control.rs` — find `spawn_agent()` and the parent-notification path on agent completion. Fire `SubagentStart` after the child is registered, `SubagentStop` when the child reports done.

**Handlers:** `subagent-start.py`, `subagent-stop.py`.

### `TaskCompleted`

**Lifecycle:** When the agent declares an explicit "I'm done with this task" signal (semantically distinct from `Stop` which fires every turn-end).

**Why:** Forces POSTFLIGHT before the conversation moves on. The "knows but doesn't do" gap David observed in 2026-05-13 is partly because there's no hook point that says "agent claimed done — did you POSTFLIGHT?".

**Dispatch site (TBC):** `codex-rs/core/src/session/turn.rs` — find where the agent's final-message-marker or update_plan completion-marker is processed. Fire `TaskCompleted` with the agent's stated completion claim in the payload.

**Handler:** `task-completed.py`.

### `PostToolUseFailure`

**Lifecycle:** When a tool invocation fails (non-zero exit, exception, timeout). Separate from `PostToolUse` which fires on success.

**Why:** Failure is a learning signal — empirica's `tool-failure.py` captures the failed approach as a dead-end artifact for future calibration. Without this hook, failures are invisible to the artifact system.

**Dispatch site (TBC):** Wherever tool invocations are awaited in `codex-rs/core/src/agent/`. Branch on result: success → `PostToolUse` (already wired), failure → `PostToolUseFailure` (new).

**Handler:** `tool-failure.py`.

## Implementation order (recommended)

Picking targets by demo-impact-per-hour-of-work:

1. **`TaskCompleted` + `PostToolUseFailure`** — most impactful for closing the "knows-but-doesn't-do" gap. These let empirica enforce POSTFLIGHT and learn from failures.
2. **`PreCompact` + `PostCompact`** — most impactful for cross-compaction calibration continuity. Currently empirica state survives via breadcrumb files but the hand-off is awkward.
3. **`SessionEnd`** — symmetric closure; lower priority because `Stop` partially handles end-of-turn cleanup.
4. **`SubagentStart` + `SubagentStop`** — needed if/when subagent usage in ecodex becomes common; lower priority for the v0.0.x demo path.

## Notes for the implementer

- **Hook contract:** input is JSON on stdin (event-shape per `codex-rs/protocol/src/protocol.rs`), output is JSON on stdout per the codex hook output schema (`hookSpecificOutput.additionalContext` for context injection, `continue: false` for blocking).
- **Fail-open:** if the hook subprocess errors, codex should continue normally — the event is informational/optional except for `PreToolUse` (which is gated) and `TaskCompleted` (which may want to block on POSTFLIGHT-required).
- **Sync vs async:** most empirica hooks are sync (block until done). The new compact hooks should be sync (we need the snapshot before compaction completes).
- **Upstream PR vs ecodex fork:** for upstream PR-ability, keep each new event behind a feature flag or as an additive enum variant (which is what's done now). The dispatch-site patches will need similar care — guard each fire behind a clear comment so an upstream maintainer can see the divergence boundary.
