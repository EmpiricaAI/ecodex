# Codex Hook Events — ecodex Divergence Roadmap

Tracks the 7 hook events ecodex adds on top of stock codex's 6, and where each one's dispatch site needs to be wired in `codex-rs/core/` for the event to actually fire.

**Status (2026-05-17):**
- ✅ **Schema landed** (commit `7bcf85c3b8`): `HookEventName` enum extended; exhaustive matches updated; `hooks.json` accepts the new event names without rejection. Empirica plugin can declare handlers today.
- ✅ **TaskCompleted dispatch site landed**: fires at `codex-rs/core/src/session/turn.rs:570` after Stop's continuation flow, before AfterAgent. **Pattern proven** — see "Dispatch pattern (minimal sibling)" below.
- ✅ **PostToolUseFailure dispatch site landed**: fires at `codex-rs/core/src/tools/registry.rs:434` in the failure branch (else of post_tool_use_payload). Carries tool_name + tool_input + error_message + duration_ms for plugin handlers consuming failures as dead-end artifacts.
- ✅ **PreCompact + PostCompact dispatch sites landed**: both fire from `codex-rs/core/src/tasks/compact.rs:CompactTask::run` (single entry point for all 3 compaction implementations — local/remote/remote_v2). PreCompact awaits synchronously before compaction runs (the `.await` is the natural block — plugin handlers complete their snapshot work before the summarizer touches history). PostCompact fires after with `success: bool`. Payload carries `compact_type` so handlers know which path ran.
- ❌ **Remaining 3 dispatch sites pending**: SessionEnd, SubagentStart/Stop. Tracked under goal `f0004294`.

## Dispatch pattern (minimal sibling) — proven by TaskCompleted

For each informational event (everything except PreCompact, which needs sync continuation semantics), the dispatch wire-up is ~150 LOC new + ~30 LOC across 6 wiring sites:

| File | Change | LOC |
|---|---|---|
| `codex-rs/hooks/src/events/<event>.rs` | New: `<Event>Request` struct (StopRequest-shape, drop fields not relevant), `<Event>Outcome { hook_events: Vec<HookCompletedEvent> }`, `preview()` + `run()` + minimal `parse_completed()` (no decision/block parsing — informational) | ~150 |
| `codex-rs/hooks/src/events/mod.rs` | `pub mod <event>;` | 1 |
| `codex-rs/hooks/src/lib.rs` | Re-export `<Event>Request` + `<Event>Outcome` | 2 |
| `codex-rs/hooks/src/schema.rs` | Add `<Event>CommandInput` struct + `<event>_hook_event_name_schema()` fn | ~25 |
| `codex-rs/hooks/src/registry.rs` | Add `preview_<event>()` + `run_<event>()` methods on `Hooks` | ~15 |
| `codex-rs/hooks/src/engine/mod.rs` | Add `preview_<event>()` + `run_<event>()` wrappers on engine | ~10 |
| `codex-rs/core/src/session/turn.rs` (or appropriate lifecycle file) | Build request, fire preview HookStarted events, run, emit completed events | ~25 |

Build + verify: `cargo build --manifest-path codex-rs/Cargo.toml --workspace --exclude codex-bwrap` should pass clean. No new unit tests required for the dispatch path itself — the wiring is mechanical and verified by build; behavioral tests belong on plugin handlers.

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

### `PreCompact` / `PostCompact` ✅ SHIPPED

**Lifecycle:** PreCompact fires just before any compaction implementation runs (local, remote-V1, or remote-V2). PostCompact fires just after, with `success: bool` indicating whether the compaction completed cleanly.

**Why:** Compaction destroys epistemic state in working memory. Pre-compact hook lets empirica snapshot transaction state, vectors, recent artifacts to `~/.empirica/breadcrumbs/`. Post-compact restores via system-message injection.

**Dispatch site:** `codex-rs/core/src/tasks/compact.rs:CompactTask::run` — single orchestration entry that branches into three implementations. Both fires happen at this level so all three paths get the events. Helpers `run_pre_compact_hooks` + `run_post_compact_hooks` in `codex-rs/core/src/hook_runtime.rs`.

**Block semantics:** PreCompact uses the `.await` on the hook run as its natural block — plugin handlers complete their snapshot work before the summarizer touches history. No should_block/continuation_fragments needed (would have been a richer Stop-like sibling pattern; the simpler awaited dispatch was sufficient).

**Payload:** Standard 6 fields plus `compact_type` (`"local"` / `"remote"` / `"remote_v2"`). PostCompact additionally carries `success: bool`.

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

### `TaskCompleted` ✅ SHIPPED

**Lifecycle:** Fires at the normal agent-done lifecycle point — after `Stop`'s continuation flow resolves, before the legacy AfterAgent notify dispatch. Codex has no explicit task-completion marker, so this fires once per turn-end-without-follow-up (same trigger as Stop, but distinct event name lets plugins attach POSTFLIGHT-enforcement handlers without changing Stop semantics).

**Why:** Forces POSTFLIGHT before the conversation moves on. The "knows but doesn't do" gap David observed in 2026-05-13 is partly because there's no hook point that says "agent claimed done — did you POSTFLIGHT?".

**Dispatch site:** `codex-rs/core/src/session/turn.rs:570` (after `if stop_outcome.should_stop { break; }`, before `let hook_outcomes = sess.hooks().dispatch(HookPayload { ... AfterAgent ... })`). Skip-on-should_stop matches existing AfterAgent semantics.

**Handler:** `task-completed.py` (vendor when defining the plugin handler).

### `PostToolUseFailure` ✅ SHIPPED

**Lifecycle:** Fires when a tool invocation fails (non-zero exit, exception, timeout) — the failure branch sibling to `PostToolUse` which only fires on success. Informational only (no continuation/block semantics).

**Why:** Failure is a learning signal — empirica's `tool-failure.py` captures the failed approach as a dead-end artifact for future calibration. Without this hook, failures are invisible to the artifact system.

**Dispatch site:** `codex-rs/core/src/tools/registry.rs:434` (the `else` branch of `post_tool_use_payload`, gated on `!success`). Helper `run_post_tool_use_failure_hooks` in `codex-rs/core/src/hook_runtime.rs` mirrors `run_post_tool_use_hooks`.

**Payload:** `tool_use_id` (correlates with prior PreToolUse), `tool_name`, `matcher_aliases` (recovered from `handler.pre_tool_use_payload`), `tool_input` (the input that failed), `error_message`, `duration_ms`.

**Handler:** `tool-failure.py` (vendor when defining the plugin handler).

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
