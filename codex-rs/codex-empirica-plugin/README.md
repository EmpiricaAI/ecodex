# codex-empirica-plugin

The [Empirica](https://github.com/EmpiricaAI/empirica) plugin compiled into
ecodex: the Sentinel firewall, PREFLIGHT / CHECK / POSTFLIGHT transactions,
calibration, the skills and the statusline — plus the provisioner that installs
it on first start.

## How it works

The crate builds one binary, `codex-empirica-plugin`, that codex runs for each
hook event. It reads codex's hook JSON on stdin, runs the matching Empirica
Python script from the vendored copy, translates the script's Claude-Code-shaped
output into codex's strict per-event schema (`src/translate_output.rs`), and
prints that. Subcommands:

```
codex-empirica-plugin pre-tool-use                      # Sentinel firewall (sentinel-gate.py)
codex-empirica-plugin post-tool-use                     # tool result capture (tool-failure.py)
codex-empirica-plugin session-start                     # practice bootstrap + session-init.py
codex-empirica-plugin user-prompt-submit                # context router (tool-router.py)
codex-empirica-plugin stop                              # transaction enforcer (transaction-enforcer.py)
codex-empirica-plugin run-hook <Event> <script.py>      # any other vendored script on any event
codex-empirica-plugin permission-request                # accepted, no-op
```

`run-hook` is how one event fans out to several scripts without a Rust handler
per script; `hooks.json` uses it for twelve of its seventeen entries, including
every event beyond the five dedicated ones (`PreCompact`, `PostCompact`,
`SessionEnd`, `SubagentStart`, `SubagentStop`, `TaskCompleted`,
`PostToolUseFailure`). `hooks.json` is the wiring; `docs/ecodex/api/hooks.md` is
the contract.

### Interpreter and script location

Hooks run under the interpreter named in the shebang of the `empirica` CLI on
`PATH` — pipx, uv and Homebrew put empirica in a private venv the first `python3`
cannot import from. `python3` is the fallback when no `empirica` is on `PATH` or
its shebang cannot be read. Resolved on every run, so it follows upgrades.

Scripts are found in this order (`src/empirica_cli.rs`):

1. `$EMPIRICA_HOOKS_DIR` — manual override.
2. `$PLUGIN_ROOT/hooks_scripts/hooks` — the normal path; codex sets `PLUGIN_ROOT`
   when it runs a plugin hook, and the plugin ships its own copy.
3. `~/.claude/plugins/local/empirica/hooks` — last resort, for a bare binary run
   next to a Claude Code install.

Each hook run also sets `EMPIRICA_INSTANCE_ID` to the codex thread id, so the
Python side keys its per-instance state to the session (the TUI sets the same
variable for the statusline).

### What `session-start` does on the host

Before `session-init.py`, the Rust handler runs the host-side practice bootstrap
(`src/practice_bootstrap.rs`): if the session cwd is not a git repository it
runs `git init`, and if it is not an empirica practice it runs `empirica
project-init --non-interactive`. It refuses to bootstrap the home directory or
the filesystem root — a session opened there is not a workspace the user
pointed ecodex at. Codex runs `SessionStart` hooks in the harness process at the
session cwd, before the agent's commands enter the sandbox, which is what lets
ecodex create the git transport without giving the agent write access to
`.git`. It also seeds the `AGENTS.md` reminder (`src/agents_md.rs`) and
Empirica's subagents into `<codex_home>/agents/` (`src/subagents.rs`), because
codex's plugin manifest has no field for either.

## What the plugin declares

`manifest.json` declares every surface codex's plugin loader knows:

| Surface | Contents |
|---|---|
| `hooks` | `hooks.json` — the wiring above |
| `skills` | `skills/`: `code-audit`, `code-docs-align`, `diagnose`, `dispatch-agent`, `empirica-constitution`, `epistemic-persistence-protocol`, `epistemic-transaction`, `ewm-interview`, `onboard`, `render` (`docs/ecodex/api/skills.md`) |
| `mcpServers` | `mcp_servers.json` registers `empirica-mcp`. The shipped config turns it **off** (`[plugins."empirica@empiricaAI".mcp_servers.empirica] enabled = false`): it only wraps the empirica CLI, which ecodex runs directly. Enable it for a front end without a shell (`docs/ecodex/api/mcp.md`) |
| `statusline` | `hooks_scripts/scripts/statusline_empirica.py` (`docs/ecodex/api/plugin-statusline.md`) |
| `writableRoots` | `["~/.empirica"]` — see below |

`empiricaVendorVersion` / `empiricaVendorCommit` in the manifest record which
empirica release the vendored scripts came from; `scripts/setup-codex.py --ref
<tag> --apply` re-vendors them.

### Sandbox carve-out (`writableRoots`)

Empirica's state is deliberately cross-cwd:

- `~/.empirica/instance_projects/<thread id>.json` — instance → project pointers
- `~/.empirica/sessions/sessions.db` — the session DB across all projects
- `~/.empirica/workspace/workspace.db` — cross-project workspace state
- `~/.empirica/active_transaction*.json`, `sentinel_paused*` — transaction and
  pause markers
- `~/.empirica/voice/`, `ref-docs/`, `epp/` — subsystem state

Without the declaration codex's `WorkspaceWrite` sandbox refuses every one of
those writes with `EROFS`. The Sentinel's crash handler then allows the action
and writes `SENTINEL_CRASH: <error>` to stderr with the reason *Sentinel error
(fail-open)* — visible, but the discipline is off. (Set
`EMPIRICA_SENTINEL_FAIL_CLOSED` to turn a crash into a deny.) Codex merges the
declaration into the active `SandboxPolicy.writable_roots` at session start;
`docs/ecodex/api/plugin-writable-roots.md` has the contract and the
audit-attribution model. `empirica diagnose --frontend ecodex` checks the
declaration is intact in the installed cache.

## Install: the provisioner

Nothing has to be installed by hand. The prebuilt channels and `cargo install`
only put binaries on disk; `provision()` (`src/provision.rs`), called on every
ecodex start, turns that into the integrated harness:

- writes the bundled plugin to
  `$CODEX_HOME/plugins/cache/empiricaAI/empirica/<version>/` whenever the
  installed copy's fingerprint differs from the bundle — so every upgrade
  refreshes it;
- creates a missing `config.toml` from `assets/config/config.toml.default`
  (curated providers, plugin enabled, its MCP server off), adds a missing
  `huggingface.config.toml` and `translator-upstreams.toml`, and adds the plugin
  entry to an existing config that has none. An entry the user wrote, including
  `enabled = false`, is left alone;
- migrates a pre-rename `[plugins."empirica@nubaeon"]` entry.

Cost when nothing changed: one marker read and one config read.

## Layout

| Path | Role |
|---|---|
| `src/main.rs` | subcommand dispatch |
| `src/hooks/` | the five dedicated handlers + `generic.rs` (`run-hook`) |
| `src/empirica_cli.rs` | interpreter and hooks-dir resolution, script execution |
| `src/translate_output.rs` | Claude Code hook output → codex schema |
| `src/provision.rs`, `practice_bootstrap.rs`, `agents_md.rs`, `subagents.rs` | first-start provisioning and host-side seeding |
| `assets/hooks_scripts/{hooks,lib,scripts}`, `assets/agents` | vendored from empirica (`scripts/setup-codex.py`) |
| `assets/config/` | `config.toml.default`, `huggingface.config.toml`, `translator-upstreams.toml` |
| `assets/empirica-system-prompt.md` | the empirica base prompt source |
| `skills/`, `hooks.json`, `mcp_servers.json`, `manifest.json` | the plugin as codex loads it |

## Tests

```sh
cargo nextest run -p codex-empirica-plugin                          # Rust unit tests
pytest codex-rs/codex-empirica-plugin/tests/vendored_hooks            # vendored hooks against codex payloads
cargo nextest run -p codex-cli --test empirica_provision              # provisioning end to end
python3 scripts/check_vendored_firewall.py                            # vendored scripts carry no Claude-Code-only paths
```

`tests/fresh_practice_bootstrap.sh` exercises the host-side bootstrap in a
throwaway directory. `docs/ecodex/api/integration-tests.md` covers the live
checks.
