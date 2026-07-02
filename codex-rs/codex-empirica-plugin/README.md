# codex-empirica-plugin

Codex plugin that brings the [Empirica](https://github.com/EmpiricaAI/empirica) epistemic discipline framework — sentinel firewall, PREFLIGHT/CHECK/POSTFLIGHT transactions, calibration vectors — to codex.

## How it works

This crate builds a single binary, `codex-empirica-plugin`, that codex invokes for each hook event. The binary dispatches by subcommand:

```
codex-empirica-plugin pre-tool-use         # Sentinel firewall
codex-empirica-plugin post-tool-use        # Tool result capture
codex-empirica-plugin session-start        # Empirica session bootstrap
codex-empirica-plugin user-prompt-submit   # Context injection, hedge detection
codex-empirica-plugin stop                 # Transaction enforcer
codex-empirica-plugin permission-request   # (codex-specific, currently no-op)
```

Each handler reads codex's hook JSON from stdin, forwards it to the corresponding Empirica Python script, and returns the script's response in codex's expected format.

## v1 implementation: subprocess shellout

The plugin currently invokes Empirica's existing Python hook scripts (e.g. `sentinel-gate.py`) via subprocess. Latency budget is ~100–300ms per hook fire (Python startup + Empirica logic). This validates the integration end-to-end without rewriting any Empirica logic.

**The single optimization target** is `src/empirica_cli.rs`. Future versions can replace the `python3 <script>` subprocess with PyO3 in-process embedding or a long-running Empirica sidecar daemon over UDS — without changing the hook handlers or the codex-side wiring.

## Sandbox carve-out (writableRoots)

The plugin's manifest declares `writableRoots: ["~/.empirica"]`. This is by design — Empirica's project lifecycle is **deliberately cross-cwd**:

- `~/.empirica/instance_projects/<key>.json` — instance↔project pointers
- `~/.empirica/sessions/sessions.db` — per-user session DB across all projects
- `~/.empirica/workspace/workspace.db` — cross-project workspace state
- `~/.empirica/active_transaction*.json` — open-transaction state
- `~/.empirica/sentinel_paused*` — `/empirica off` toggle markers
- `~/.empirica/voice/`, `ref-docs/`, `epp/` — subsystem state

Without this declaration, codex's `WorkspaceWrite` sandbox blocks every empirica state write with `EROFS`, `sentinel-gate.py:2808` catches the exception and silently fail-opens (`allow`), and the entire discipline framework runs as a no-op while *appearing* healthy.

Codex's plugin loader honors the declaration by merging it into the active `SandboxPolicy.writable_roots` at session bootstrap. See [`docs/ecodex/api/plugin-writable-roots.md`](../../docs/ecodex/api/plugin-writable-roots.md) for the full contract, including the audit-attribution model and limitations.

The doctor regression check `ecodex plugin writable_roots declared` (run via `empirica diagnose --frontend ecodex`) verifies the declaration is intact in the installed cache.

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `EMPIRICA_HOOKS_DIR` | `~/.claude/plugins/local/empirica/hooks` | Location of Empirica's Python hook scripts |

## Install (planned)

The plugin manifest (`manifest.json`) is loadable by codex's plugin marketplace at `~/.codex/plugins/cache/empirica/<version>/manifest.json`. Install steps are part of the ecodex distribution layer.

## Status

| Hook event | Status | Backed by |
|---|---|---|
| `pre-tool-use` | ✅ wired | `sentinel-gate.py` |
| `post-tool-use` | ✅ wired | `tool-failure.py` |
| `session-start` | ✅ wired | `session-init.py` |
| `user-prompt-submit` | ✅ wired | `tool-router.py` |
| `stop` | ✅ wired | `transaction-enforcer.py` |
| `permission-request` | stub | (codex-specific; design TBD) |

See the parent goal "Build v1 empirica plugin for codex" for the full porting plan.

## Future scope (beyond v1)

The codex plugin manifest exposes five extension surfaces — this crate currently uses only `hooks`. Roadmap surfaces:

| Surface | What we'd put there |
|---|---|
| `hooks` (in v1) | sentinel firewall, transaction enforcer, calibration capture |
| `skills` | The Empirica skill set (epistemic-transaction, EPP, brainstorming, debugging, etc.) — port from CC's empirica plugin |
| `mcp_servers` | Register the existing `mcp__empirica__*` server (already exists; just needs registration) |
| `apps` / connectors | Possible future home for Cockpit (multi-instance Empirica TUI) integration |
| `interface` | Done — display name, brand color, etc. |

See [`docs/ecodex/architecture.md`](../../docs/ecodex/architecture.md) for the broader architecture and [`docs/ecodex/integrations/`](../../docs/ecodex/integrations/) for Cockpit/Symphony integration notes (when populated).
