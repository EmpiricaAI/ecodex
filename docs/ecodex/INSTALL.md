# Installing ecodex

ecodex installs from source. There is no prebuilt release yet; the install script auto-builds via `cargo` on first invocation.

## Prerequisites

- **Rust toolchain** — install via [rustup.rs](https://rustup.rs/). The workspace builds with stable Rust 1.93 or later.
- **`empirica` CLI** on `PATH` — the empirica plugin shells out to it. Install from [`Nubaeon/empirica`](https://github.com/Nubaeon/empirica) before running ecodex; without it, the plugin's hook subprocesses fail-quiet and discipline goes dark.
- **Linux or macOS** — Windows isn't supported yet (requires `landlock` / sandbox parity work).

## One-command install

```sh
git clone https://github.com/Nubaeon/ecodex.git
cd ecodex
./ecodex/scripts/install.sh
```

This builds the Rust workspace (`-p codex-cli -p codex-empirica-plugin --release`), installs binaries + bundled plugin assets, and seeds `~/.codex/config.toml` if no config exists. First-time builds take 10–25 minutes depending on hardware; rebuilds are minutes.

## What the install lays down

| Path | What |
|---|---|
| `~/.local/bin/ecodex` | wrapper script (this is what users run) |
| `~/.local/lib/ecodex/bin/ecodex` | the actual binary; the wrapper exec's into this |
| `~/.local/bin/codex-empirica-plugin` | plugin binary; codex invokes this for each hook event |
| `~/.codex/plugins/cache/nubaeon/empirica/0.1.0/` | bundled plugin assets (hooks scripts, manifest, MCP, skills, statusline, subagents) |
| `~/.codex/config.toml` | seeded with curated provider defaults *only if* no config existed; existing configs are preserved |

The install also patches `[features] plugin_hooks = true` and `plugins = true` into an existing config (idempotently — no-op if the keys are already present). Without these the plugin host stays dark and every empirica hook silently no-ops.

## Modes

| Flag | Effect |
|---|---|
| `--user` (default) | per-user install under `~/.local/bin` and `~/.codex/`. No sudo required. The structural lock against runtime-disable is **not** enforced (codex hardcodes `/etc/codex/requirements.toml` as the only managed-config path on Unix — out of scope for `--user`). |
| `--system` | system-wide install under `/usr/local/bin` and the system codex paths. Requires sudo. Installs `requirements.toml` to enforce the empirica-enabled lock — a determined runtime can't disable the plugin without root. |
| `--prefix DIR` | override the binary install dir for `--system` mode (default `/usr/local`). |
| `--no-build` | skip cargo build; assume binaries already built (use `ECODEX_BINARY=...` and `PLUGIN_BINARY=...` env vars to point at prebuilt artifacts). |

## Verify the install

```sh
ecodex --version
empirica diagnose-ecodex
```

The doctor runs ~15 checks covering plugin discovery, hook scripts, feature gates, statusline, translator health, env keys, and Rust toolchain presence. A clean install reports green or `WARN` only on optional pieces.

## Configure providers

The seeded `~/.codex/config.toml` includes curated entries for DeepSeek, Qwen3-Coder, Kimi K2.6, GLM, Ollama, LM Studio, and llama.cpp. To use one:

1. Get an API key (or run a local provider).
2. Export the env key the provider expects. For example, `export DEEPSEEK_API_KEY="sk-..."`.
3. Run `ecodex` and pick the model from `/model`.

For local providers (Ollama, LM Studio), make sure the server is running and reachable at the base URL declared in `config.toml`.

The `wire_api` field tells codex which protocol to speak. `responses` means the provider supports OpenAI's Responses API natively. `chat` or `anthropic` means the wrapper auto-spawns the translator on `localhost:18080` and rewrites the base URL — codex still speaks Responses; the translator does the protocol conversion behind the scenes.

## Hot-swap mid-session

ecodex supports cross-provider hot-swap via `/model` without restarting the session (T78). Pick a curated entry that maps to a different provider, and the session-shared `ModelClient` swaps under the hood. Existing turn-scoped state (websocket sessions, prewarm caches) is invalidated cleanly.

## Updating an existing install

Re-running `./ecodex/scripts/install.sh` is safe and idempotent. The script:

1. Builds the latest workspace state (cargo-cached, fast on no-changes).
2. Re-installs the binaries via `rm -then-cp` (dodges `Text file busy` on running ecodex sessions).
3. Re-syncs vendored plugin assets (hooks, agents, statusline).
4. Idempotently patches the `[features]` block in your existing `~/.codex/config.toml`.
5. Detects in-flight ecodex sessions via `pgrep` and prints a "restart to pick up this build" warning when needed.

In-flight sessions keep running on the OLD binary via inherited file descriptors — Linux holds the inode alive even after the directory entry is replaced. New behavior takes effect on session restart.

## Uninstalling

```sh
./ecodex/scripts/uninstall.sh
```

`--purge` removes user config (`~/.codex/`) too. By default the uninstall preserves your config and just rips out the binaries + plugin cache.

## Troubleshooting

**`empirica: command not found` during plugin hook fires**
The plugin needs `empirica` on `PATH`. Install from [`Nubaeon/empirica`](https://github.com/Nubaeon/empirica). The plugin fail-quiets on subprocess failures so ecodex itself still works — but discipline goes dark.

**`Text file busy` during reinstall**
Should not happen — the install script uses `rm`-then-`cp`. If it does, you may have an old install.sh; pull latest and retry.

**Doctor reports "plugin_hooks feature disabled"**
Re-run install.sh — it idempotently writes the `[features] plugin_hooks = true` key. Or edit `~/.codex/config.toml` to add the key under a `[features]` block.

**Doctor reports "translator not listening" but you're on a `responses`-API provider**
Expected. The translator only spawns when the active provider's `wire_api` is `chat` or `anthropic`. The doctor surfaces this as INFO, not FAIL, in that case.

**Hot-swap from `/model` doesn't actually swap providers**
T78 should handle this — `services.model_client` is `ArcSwap<ModelClient>` and the picker writes through to it. If you observe stale routing, check `~/.codex/log/codex-tui.log` for "swapped model_client" tracing. File a bug with the log excerpt.
