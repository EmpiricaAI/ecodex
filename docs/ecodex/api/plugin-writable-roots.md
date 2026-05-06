# Plugin `writableRoots` — Cross-cwd Sandbox Carve-outs

**Status:** Live. Schema + discovery + integration shipped 2026-05-06 (T82
Tx-AI/1–4). This is the contract for plugins whose runtime needs filesystem
write access *outside the session cwd*.

## Why this exists

Codex's `WorkspaceWrite` sandbox profile pins writable scope to the session
cwd. That works for plugins whose state lives entirely *under* the user's
project tree — but not for plugins that operate **across** project boundaries:

- A plugin that manages a global session DB at `~/<plugin>/sessions.db`
- A plugin that tracks user-level state spanning multiple projects
- A plugin whose lifecycle includes creating new projects at user-chosen paths
- A plugin that needs to read/write a config or cache outside any cwd

Without an explicit declaration, `landlock` (Linux) / `seatbelt` (macOS) /
the Windows sandbox layer block every cross-cwd write with `EROFS` /
permission-denied. Plugins that fail-open on errors (e.g. Empirica's
sentinel-gate falls back to "allow" on uncaught exceptions) will silently
operate as no-ops while *appearing* healthy — a uniquely costly failure mode
for a discipline framework, since the discipline goes dark without raising.

`writableRoots` lets plugins declare exactly the cross-cwd paths they need,
and the codex sandbox layer honors those declarations as part of the active
`SandboxPolicy`. **The plugin makes the contract explicit; the host enforces it.**

## Manifest schema

In your plugin's `plugin.json` (or `manifest.json`):

```json
{
  "name": "your-plugin@vendor",
  "version": "1.0.0",
  "writableRoots": [
    "~/.your-plugin",
    "/var/lib/your-plugin-cache"
  ]
}
```

**Resolution rules** (applied at manifest load time):

| Form | Behavior |
|------|----------|
| `~/...` | Expanded against `$HOME` |
| `/...` | Absolute path, kept verbatim |
| `./...`, `../...`, `.`, `..` | **Rejected** with warning — relative paths would be ambiguous against the agent's mutable cwd |
| paths containing `..` post-expansion | **Rejected** to keep the path contract auditable (no traversal escapes from declared roots) |
| `""`, whitespace-only | Skipped with warning |
| Field unset | No additional roots; plugin runs under default sandbox scope |

## Runtime behavior

At session bootstrap, codex:

1. Loads each enabled plugin's manifest.
2. Calls `effective_plugin_writable_roots()` on the resulting `PluginLoadOutcome`,
   producing one `PluginWritableRootSource { plugin_id, plugin_root, root }` per
   declared root, per plugin.
3. Calls `FileSystemSandboxPolicy::with_additional_writable_roots(cwd, roots)`
   on the session's base profile, which de-duplicates roots already covered by
   cwd or existing entries.
4. Rebuilds the active `PermissionProfile` via
   `from_runtime_permissions_with_enforcement` preserving enforcement +
   network policy.
5. Threads that profile through every `TurnContext` and `SandboxAttempt`
   spawned for the rest of the session.

The merge is **a structural no-op for unrestricted and external-sandbox
profiles** — only `Restricted` (workspace-write equivalent) profiles consume
the carve-out. A plugin declaring roots under a fully-trusted profile is
harmless; declaring roots under a fully-locked-down profile contributes
nothing because the locked profile bypasses the merge.

## By design: empirica is cwd-permissive

Empirica is the canonical example of a `writableRoots`-using plugin. Its
project lifecycle is **deliberately cross-cwd**:

| Path | Why |
|------|-----|
| `~/.empirica/instance_projects/<key>.json` | Maps terminal/session instances to projects. Required for *any* empirica state read/write. |
| `~/.empirica/sessions/sessions.db` | Per-user session DB across all projects. |
| `~/.empirica/workspace/workspace.db` | Cross-project workspace state. |
| `~/.empirica/active_transaction*.json` | Open-transaction state (PRE/CHECK/POSTFLIGHT lifecycle). |
| `~/.empirica/sentinel_paused*` | `/empirica off` toggle markers (per-instance + global). |
| `~/.empirica/voice/`, `~/.empirica/ref-docs/`, `~/.empirica/epp/` | Subsystem state. |

Additionally, empirica's AI-guided project flow (agent runs `cd /path/projB
&& empirica project-create && empirica project-init && empirica project-switch
projB`) writes to `<projB>/.empirica/`, which is by definition outside the
session-start cwd. **This is intentional**: empirica manages projects, and
"the AI creates a new project elsewhere" is a first-class operation — the
agent should be able to perform it without the sandbox blocking the writes
the plugin's CLI needs to make.

The empirica plugin therefore declares `writableRoots: ["~/.empirica"]` and
documents (in its README) that its project-lifecycle commands may need to
write outside the current cwd. Future work will extend this to a *dynamic*
carve-out for AI-chosen project paths — the plugin will request the host
expand `writableRoots` at runtime when `project-create` resolves to a path
outside session-start cwd.

## Doctor regression

`empirica diagnose --frontend ecodex` includes
**`ecodex plugin writable_roots declared`** which:

- Reads the cached plugin manifest at
  `~/.codex/plugins/cache/nubaeon/empirica/<version>/.codex-plugin/plugin.json`
- Asserts `writableRoots` exists, is a list, and contains `~/.empirica`
- Fails (not warns) if the declaration is missing — the failure mode is
  "discipline silently dark", which deserves a hard check.

Run before / after every install to confirm the carve-out is wired through.

## Limitations

- **Static only (today)**: declarations are read once at manifest load.
  Plugins cannot request new writable roots at runtime. Tx-AL (planned) adds
  a plugin-host IPC channel for dynamic carve-outs.
- **Profile-scoped**: declarations only take effect under
  `WorkspaceWrite`-equivalent profiles. Under `ReadOnly` the plugin gets
  nothing extra; under `DangerFullAccess` the declaration is moot.
- **Audit attribution**: each granted root carries the declaring plugin's
  `plugin_id` so security audits can trace each carve-out to its source.

## See also

- `codex-rs/core-plugins/src/manifest.rs` — schema parser + tests
- `codex-rs/plugin/src/lib.rs` — `PluginWritableRootSource` struct
- `codex-rs/plugin/src/load_outcome.rs` — `effective_plugin_writable_roots()`
- `codex-rs/core-plugins/src/loader.rs` — `load_plugin_writable_roots()` discovery
- `codex-rs/core/src/session/mod.rs` — `enrich_permission_profile_with_plugin_writable_roots`
- `codex-rs/protocol/src/permissions.rs` — `with_additional_writable_roots` mechanism
- `empirica/empirica/cli/command_handlers/diagnose_ecodex.py` — `check_ecodex_plugin_writable_roots_declared`
