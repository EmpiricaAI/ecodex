# Plugin `writableRoots` — Cross-cwd Sandbox Carve-outs

**Status:** live. This is the contract for plugins whose runtime needs
filesystem write access *outside the session cwd*.

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
permission-denied. A plugin that fails open on errors (Empirica's sentinel-gate
allows a tool call when it hits an uncaught exception) then runs as a no-op
while *appearing* healthy — a uniquely costly failure for a discipline
framework, since the discipline goes dark without saying so.

`writableRoots` lets a plugin declare exactly the cross-cwd paths it needs,
and the codex sandbox layer honors those declarations as part of the active
`SandboxPolicy`. **The plugin makes the contract explicit; the host enforces it.**

## Manifest schema

In your plugin's `plugin.json` (the empirica plugin's source file is
`manifest.json`, written as `.codex-plugin/plugin.json` in the plugin cache):

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

**Resolution rules** (applied when the manifest is loaded):

| Form | Behavior |
|------|----------|
| `~/...` | Expanded against `$HOME` |
| `/...` | Absolute path, kept verbatim |
| `./...`, `../...`, `.`, `..` | **Rejected** with a warning — relative paths would be ambiguous against the agent's mutable cwd |
| paths containing `..` after expansion | **Rejected**, to keep the path contract auditable (no traversal out of a declared root) |
| `""`, whitespace-only | Skipped with a warning |
| Field unset | No additional roots; the plugin runs under the default sandbox scope |

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
   `from_runtime_permissions_with_enforcement`, preserving enforcement and
   network policy.
5. Threads that profile through every `TurnContext` and `SandboxAttempt`
   spawned for the rest of the session.

The merge is **a structural no-op for unrestricted and external-sandbox
profiles** — only `Restricted` (workspace-write equivalent) profiles consume
the carve-out. A plugin declaring roots under a fully-trusted profile is
harmless; under a fully locked-down profile the declaration contributes
nothing, because the locked profile bypasses the merge.

## By design: empirica is cwd-permissive

Empirica is the canonical `writableRoots` plugin. Its project lifecycle is
**deliberately cross-cwd**:

| Path | Why |
|------|-----|
| `~/.empirica/instance_projects/<key>.json` | Maps session instances to projects. Required for *any* empirica state read/write. |
| `~/.empirica/sessions/sessions.db` | Per-user session DB across all projects. |
| `~/.empirica/workspace/workspace.db` | Cross-project workspace state. |
| `~/.empirica/active_transaction*.json` | Open-transaction state (PREFLIGHT / CHECK / POSTFLIGHT lifecycle). |
| `~/.empirica/sentinel_paused*` | `/empirica off` toggle markers (per-instance and global). |
| `~/.empirica/voice/`, `~/.empirica/ref-docs/`, `~/.empirica/epp/` | Subsystem state. |

Empirica's AI-guided project flow also writes outside the session's cwd: an
agent that runs `cd /path/projB && empirica project-create && empirica
project-init && empirica project-switch projB` writes to `<projB>/.empirica/`.
**This is intentional**: empirica manages projects, and "the AI creates a new
project elsewhere" is a first-class operation the sandbox should not silently
break.

The empirica plugin therefore declares `writableRoots: ["~/.empirica"]`. Writes
to a new project's own `.empirica/` outside the session cwd are not covered yet
(see Limitations).

## Doctor check

`empirica diagnose --frontend ecodex` includes
**`ecodex plugin writable_roots declared`**, which:

- Reads the cached plugin manifest at
  `~/.codex/plugins/cache/empiricaAI/empirica/<version>/.codex-plugin/plugin.json`
  (or the legacy `nubaeon` marketplace directory on older installs)
- Asserts `writableRoots` exists, is a list, and contains `~/.empirica`
- Fails (not warns) if the declaration is missing — the failure mode is
  "discipline silently dark", which deserves a hard check.

Run it after every install or update to confirm the carve-out is wired through.

## Limitations

- **Static only**: declarations are read once, at manifest load. A plugin
  cannot ask for new writable roots at runtime, so a project that empirica
  creates outside the session cwd is still written under the default sandbox.
  A plugin-host channel for dynamic carve-outs is planned.
- **Profile-scoped**: declarations take effect only under
  `WorkspaceWrite`-equivalent profiles. Under `ReadOnly` the plugin gets
  nothing extra; under `DangerFullAccess` the declaration is moot.
- **Audit attribution**: each granted root carries the declaring plugin's
  `plugin_id`, so a security audit can trace every carve-out to its source.

## See also

- `codex-rs/core-plugins/src/manifest.rs` — schema parser and tests
- `codex-rs/plugin/src/lib.rs` — `PluginWritableRootSource`
- `codex-rs/plugin/src/load_outcome.rs` — `effective_plugin_writable_roots()`
- `codex-rs/core-plugins/src/loader.rs` — `load_plugin_writable_roots()` discovery
- `codex-rs/core/src/session/mod.rs` — `enrich_permission_profile_with_plugin_writable_roots`
- `codex-rs/protocol/src/permissions.rs` — `with_additional_writable_roots`
- `empirica/cli/command_handlers/diagnose_ecodex.py` (in Empirica) — `check_ecodex_plugin_writable_roots_declared`
