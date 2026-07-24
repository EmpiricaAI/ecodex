# Codex Hook Events — ecodex Divergence Map

Tracks how ecodex's hook-event surface relates to upstream codex, and where
each ecodex-divergent event is dispatched in `codex-rs/core/`.

> **Premise update (post-convergence).** An earlier version of this doc framed
> ecodex as adding *7 events on top of stock codex's 6*. That framing is
> **obsolete**. Stock codex's `HookEventName` now carries **11** variants, and
> **5** of the events ecodex once added independently (`SessionEnd`,
> `PreCompact`, `PostCompact`, `SubagentStart`, `SubagentStop`) **converged with
> upstream** — ecodex now adopts upstream's variants rather than maintaining its
> own. Only **2** events remain genuine ecodex divergences: `TaskCompleted` and
> `PostToolUseFailure`.

## Ground truth — `HookEventName` (`codex-rs/protocol/src/protocol.rs:1705-1729`)

The enum has **13** variants total: **11 upstream** + **2 ecodex-divergent**.

**11 upstream variants** (ecodex adopts these directly):

| Event | Lifecycle point | empirica handler(s) |
|---|---|---|
| `PreToolUse` | Before any tool invocation | `sentinel-gate.py` |
| `PermissionRequest` | When a tool needs permission | (no-op for empirica) |
| `PostToolUse` | After a successful tool invocation | `tool-failure.py` (canonical) + `entity-extractor.py` |
| `PreCompact` | Just before compaction runs | `pre-compact.py` |
| `PostCompact` | Just after compaction (`success: bool`) | `post-compact.py` |
| `SessionStart` | New or resumed codex session | `session-init.py` + siblings |
| `SessionEnd` | Session shutdown | `session-end-postflight.py` |
| `UserPromptSubmit` | User submits a prompt | `tool-router.py` + siblings |
| `SubagentStart` | Parent session, after a subagent spawns | `subagent-start.py` |
| `SubagentStop` | Subagent's own session, on self-report done | `subagent-stop.py` |
| `Stop` | Agent turn ends | `transaction-enforcer.py` |

**2 ecodex-divergent variants** (marked in `protocol.rs` under the
`── ecodex divergence ──` comment):

| Event | Lifecycle point | empirica handler |
|---|---|---|
| `TaskCompleted` | Agent declares task-completion (Stop's broader sibling) | `task-completed.py` |
| `PostToolUseFailure` | A tool invocation fails (non-zero exit / exception / timeout) | `tool-failure.py` |

## Dispatch mechanism (current)

All hook dispatch is centralized in **`codex-rs/core/src/hook_runtime.rs`**,
which builds the per-event request and runs the plugin engine. Callers in the
lifecycle files (`session/turn.rs`, `session/handlers.rs`, `tools/registry.rs`,
the compaction task, the subagent spawn/report paths) invoke these helpers
rather than dispatching inline.

### The 5 converged events — dispatched via `hook_runtime.rs`

- **`SessionEnd`** — `run_session_end_hooks(sess)`, called from
  `codex-rs/core/src/session/handlers.rs` during shutdown (before
  `abort_all_tasks`).
- **`PreCompact` / `PostCompact`** — `run_pre_compact_hooks()` /
  `run_post_compact_hooks()`, fired around the compaction task so all
  compaction implementations get the events. PreCompact awaits synchronously
  (the `.await` is the natural block — snapshot work completes before the
  summarizer touches history).
- **`SubagentStart`** — selected as `StartHookTarget::SubagentStart` inside the
  session-start dispatch path (`run_pending_session_start_hooks`); fires in the
  **parent's** session when a subagent thread is spawned.
- **`SubagentStop`** — selected as `StopHookTarget::SubagentStop` inside the
  turn-stop dispatch path (`run_turn_stop_hooks`); fires in the **subagent's
  own** session when it self-reports done. Plugins correlate parent→child via
  `child_thread_id` (Start) + `session_id` (Stop).

### The 2 ecodex-divergent events — fired from lifecycle sites via `hook_runtime.rs` helpers

- **`TaskCompleted`** — fires from **`codex-rs/core/src/session/turn.rs`**
  (guarded by an `ecodex addition (goal f0004294)` comment), building a
  `codex_hooks::TaskCompletedRequest` and calling `hooks.run_task_completed(..)`
  after the Stop continuation flow resolves. Distinct event name lets plugins
  attach POSTFLIGHT-enforcement handlers without changing `Stop` semantics.
- **`PostToolUseFailure`** — fires from **`codex-rs/core/src/tools/registry.rs`**
  (the tool-failure branch, `ecodex addition (goal f0004294)`), calling
  `run_post_tool_use_failure_hooks(..)` in `hook_runtime.rs` — the failure-branch
  sibling to `run_post_tool_use_hooks`. Carries `tool_name` + `tool_input` +
  `error_message` + `duration_ms` so `tool-failure.py` can record the failed
  approach as a dead-end artifact.

> These file pointers describe the current dispatch mechanism; exact line
> numbers drift with refactors, so locate the fire sites by the
> `ecodex addition (goal f0004294)` comments and the `hook_runtime.rs` helper
> names rather than by line.

## Feature gating (current)

Plugin hooks run under **`Feature::Plugins`** (`codex-rs/features/src/lib.rs`),
which is **`Stage::Stable`, `default_enabled: true`** — so hooks are on by
default with no opt-in flag required.

The old **`Feature::PluginHooks`** flag (the `[features] plugin_hooks = true`
toml key) is now **`Stage::Removed`**: the `plugin_hooks` toml key is **ignored**
(parsed and `continue`d as a no-op compatibility flag). Any instruction to set
`[features] plugin_hooks = true` to enable hooks is obsolete — it has no effect.

## Notes

- **Hook contract:** input is JSON on stdin (event-shape per
  `codex-rs/protocol/src/protocol.rs`), output is JSON on stdout per the codex
  hook output schema (`hookSpecificOutput.additionalContext` for context
  injection, `permissionDecision`/`continue: false` for blocking).
- **Fail-open vs fail-closed:** the `PreToolUse` firewall is a security floor
  and fails **closed** when its gate is present-but-broken; informational
  handlers fail **open**. See `docs/ecodex/api/hooks.md` for the full policy.
- **Upstream-PR-ability:** the 2 remaining divergent events are additive enum
  variants guarded by clear `ecodex addition` comments at each dispatch site, so
  the divergence boundary stays visible to an upstream maintainer.
