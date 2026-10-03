# Plugin Statusline Contribution Surface

Codex plugins can declare a **statusline command** that the TUI invokes
on a debounced tick and renders below the user prompt. This is the
mechanism ecodex's empirica plugin uses to display live epistemic state
(stage, confidence, open artifacts, whether to investigate or act)
without the model or the user having to query for it.

The surface is **generalized** — any plugin can contribute one. There
is nothing empirica-specific in the codex side of the contract, apart
from one environment variable (below) that carries the session identity
any plugin may use.

## Quick start (plugin author)

Add a `statusline` field to your plugin's `manifest.json`:

```json
{
  "name": "my-plugin@my-marketplace",
  "version": "0.1.0",
  "hooks": "./hooks.json",
  "skills": "./skills",
  "statusline": "./scripts/render-statusline.sh"
}
```

The path **must** be relative to the plugin root and start with `./`
(same validation rule as `hooks`, `skills`, `mcpServers`, and `apps`).

Write a script (any language; just needs to be executable) that prints
a single line of ANSI-formatted text to stdout and exits cleanly:

```bash
#!/usr/bin/env bash
# scripts/render-statusline.sh
printf '\033[36m%s\033[0m │ \033[32m%s\033[0m\n' "my-plugin" "ok"
```

That's it. Codex picks up the new field at session start and the script
output appears in the footer below the user prompt.

## Lifecycle

1. **Plugin install** — codex parses `manifest.statusline` via
   `PluginManifestPaths::statusline: Option<AbsolutePathBuf>` (same path
   validation as siblings). Invalid or missing → no statusline contributed.
2. **Session start** — `PluginsManager` walks loaded plugins; for each
   that declared `statusline`, a `PluginStatuslineSource` is collected.
3. **TUI boot** — `ChatWidget` receives the source set via
   `AppEvent::PluginStatuslineSourcesLoaded` (mirroring the existing
   `PluginMentionsLoaded` async pattern).
4. **Background runtime** — one `tokio::spawn` per source. Each task
   fires the script immediately, then loops:
   `sleep 1.5s → invoke → emit AppEvent::PluginStatuslineOutputUpdated`.
5. **Render** — `recompute_plugin_statusline` (in
   `tui/src/chatwidget/status_controls.rs`) aggregates cached outputs
   (sorted by `PluginId` for stable order, joined with `' │ '`), parses
   ANSI via `codex_ansi_escape`, and pushes the resulting `Line` through
   `set_status_line`.

## Subprocess contract

Each tick, codex spawns the declared command with:

| Env var | Value |
|---|---|
| `PLUGIN_ROOT` | absolute path to the plugin install dir |
| `CLAUDE_PLUGIN_ROOT` | same as `PLUGIN_ROOT` (CC compat) |
| `PLUGIN_DATA` | absolute path to the plugin data dir |
| `CLAUDE_PLUGIN_DATA` | same as `PLUGIN_DATA` (CC compat) |
| `EMPIRICA_INSTANCE_ID` | the codex thread id, once the session is configured; unset before that |

The same contract as the plugin's hook subprocesses, so vendored asset
lookups work identically, and the thread id is what lets a script tell
apart several sessions running in one directory.

**stdin** is a piped JSON object, then closed:

```json
{"session_id": "<empirica session id>", "cwd": "<ecodex working directory>"}
```

Either key is omitted when it cannot be resolved (an empty object `{}` is
still sent). `session_id` is read from empirica's instance files under
`~/.empirica/instance_projects/`: first `<thread id>.json` (the
`EMPIRICA_INSTANCE_ID` value), then the ids empirica's own
`get_instance_id()` would try (`EMPIRICA_INSTANCE_ID` from the
environment, a tmux pane, an X window id), and failing all of those the
most recently written instance file whose `project_path` is a prefix of
`cwd`. That last fallback cannot distinguish sessions sharing a
directory, which is why the thread id goes first.

stdout is captured up to the timeout and treated as ANSI text. stderr is
discarded.

**Timeout:** 2 seconds per invocation. If the script hangs longer than
that, codex kills the child and reports an empty output for this tick.
The next tick fires a fresh attempt — the runtime never accumulates
concurrent invocations for the same plugin.

**Failure handling:** any of {non-zero exit code, spawn error, timeout}
result in an empty output. `ChatWidget` removes the plugin's entry from
the cache on empty output, so a now-broken plugin's last good text
disappears within one tick rather than lingering as stale content.

## Tick cadence

Fixed at **1.5 seconds** (`TICK_INTERVAL` in
`tui/src/plugin_statusline_runtime.rs`). This matches the cadence
empirica's chat statusline settled on. The cadence is a single `Duration`
constant — change with care; faster ticks proportionally increase
subprocess fan-out.

## Render order and caveat

When **any** plugin contributes statusline content, the plugin output
**overrides** the codex-managed `/statusline` items (model, git branch,
context %, etc.) in the footer slot. This is intentional for ecodex —
empirica's statusline IS the primary signal for epistemic-discipline
work — but it means a user who configured codex's built-in statusline
items via `/statusline` won't see them while plugin output is present.
Plugins that contribute a statusline should treat it as a takeover.

When the cache becomes empty (no plugins, all failed, all cleared),
the override clears and the codex-managed items reappear on the next
`refresh_status_surfaces` tick.

## Security considerations

- The plugin install dir is the source of truth for the script path.
  A user installing a plugin is implicitly trusting its statusline
  command — same trust model as plugin hooks and MCP servers.
- The runtime never spawns concurrent invocations for the same plugin
  (back-pressure is built-in). A misbehaving script can't slow the
  TUI by more than the 2s timeout per tick.
- The script sees codex-internal paths, the thread id and the working
  directory. No conversation content, model output, or session secrets
  are exposed.

## Performance considerations

- Subprocess fan-out is `O(plugins-with-statusline)`. With one plugin
  declaring it, that's one subprocess per 1.5s — negligible.
- ANSI parsing happens on each output update via `codex_ansi_escape`,
  not on each render frame. Re-render of the cached `Line` is cheap.
- `tokio::process::Command` with `kill_on_drop(true)` ensures hung
  children don't leak across timeouts.

## Code map

| File | Responsibility |
|---|---|
| `core-plugins/src/manifest.rs` | `PluginManifestPaths.statusline` schema + parser |
| `core-plugins/src/loader.rs` | `load_plugin_statusline()` per-plugin builder |
| `plugin/src/lib.rs` | `PluginStatuslineSource` struct |
| `plugin/src/load_outcome.rs` | `LoadedPlugin.statusline_source` + `PluginLoadOutcome::effective_plugin_statusline_sources()` |
| `tui/src/app/background_requests.rs` | `refresh_plugin_statusline_sources()` async fetch |
| `tui/src/app_event.rs` | `RefreshPluginStatuslineSources` + `PluginStatuslineSourcesLoaded` + `PluginStatuslineOutputUpdated` events |
| `tui/src/app/event_dispatch.rs` | event-routing handlers |
| `tui/src/plugin_statusline_runtime.rs` | per-plugin tokio task, subprocess invoker, stdin payload and instance-file resolution |
| `tui/src/chatwidget/status_controls.rs` | `plugin_statusline_outputs` cache + `recompute_plugin_statusline()` |
| `tui/src/chatwidget/session_flow.rs` | hands the configured thread id to the runtime |

## Example: empirica plugin

ecodex's bundled empirica plugin (`codex-rs/codex-empirica-plugin/`)
declares `"statusline": "./hooks_scripts/scripts/statusline_empirica.py"`.
The vendored script reads empirica's session database for the session
named on stdin and prints one line. The default **compact** mode is:

```
practice │ stage confidence │ G U A F/D │ Δ mark │ context │ 🔍 investigate | 🔨 act - model
```

that is: the practice name, the transaction stage with its confidence,
open goals / unknowns / assumptions and findings / decisions counts, a
learning mark after the delta sign when vectors moved, context-window use
when the harness reports it, and whether the Sentinel would currently
let the agent act, with the active model.

Modes: `compact` (default), `expanded` (the previous layout: phase
composite, key vectors, open counts, CHECK gate), `basic`, `learning`,
`full`. The mode is read on every render from `~/.empirica/statusline_mode`
(`echo expanded > ~/.empirica/statusline_mode`; no restart), and from
`EMPIRICA_STATUS_MODE` when that file is absent. `EMPIRICA_STATUS_MODEL=0`
hides the model tag. The script's header comment is the authoritative
list.

## See also

- `docs/ecodex/system-overview.md` — three-layer architecture (codex /
  empirica integration / specialised ecodex code).
- `docs/ecodex/api/hooks.md` — sibling plugin contribution surface
  (event-driven rather than render-tick).
