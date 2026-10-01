# Installing ecodex

ecodex installs the same way whichever channel you pick: you put the binaries on
your `PATH`, and the first session sets up everything else. That includes the
empirica plugin (the Sentinel, the hooks and the bundled skills) and a curated
`config.toml`. Every later upgrade refreshes the plugin.

## Prerequisites

- **The `empirica` CLI on `PATH`.** The plugin's hooks shell out to it. Install it
  from [`EmpiricaAI/empirica`](https://github.com/EmpiricaAI/empirica) before your
  first session. Without it the hooks fail quietly and the discipline goes dark,
  although ecodex itself still runs.
- **Linux or macOS.** Windows is not supported yet.
- **A Rust toolchain** ([rustup.rs](https://rustup.rs/), stable) only for the
  cargo and source-build channels.

## Choose a channel

| Channel | Command | Compiles? |
|---|---|---|
| Install script (Mac/Linux) | `curl -fsSL https://raw.githubusercontent.com/EmpiricaAI/ecodex/main/scripts/install.sh \| bash` | No |
| Homebrew (Mac/Linux) | `brew install EmpiricaAI/tap/ecodex` | No |
| Release tarball | Download `ecodex-<target>.tar.gz` from [Releases](https://github.com/EmpiricaAI/ecodex/releases/latest) | No |
| Cargo | `cargo install --git https://github.com/EmpiricaAI/ecodex codex-cli` | Yes |
| Source build | `git clone https://github.com/EmpiricaAI/ecodex.git && cd ecodex && ./ecodex/scripts/install.sh` | Yes |

Non-developers should use the install script or Homebrew. Both download prebuilt,
stripped binaries for macOS (arm64, x86_64) and Linux (arm64, x86_64); the Linux
builds target glibc 2.35 (Ubuntu 22.04) and newer.

### Install script

Detects your OS and CPU, downloads the matching release tarball, verifies its
SHA-256 and installs four binaries into `~/.local/bin`: `ecodex`,
`codex-empirica-plugin`, `codex-empirica-translator` and `codex-code-mode-host`.
Install elsewhere with `--prefix DIR` or `ECODEX_INSTALL_DIR`, and pin a release
with `ECODEX_VERSION=<tag>`.

### Homebrew

Installs the same four prebuilt binaries from the
[`EmpiricaAI/homebrew-tap`](https://github.com/EmpiricaAI/homebrew-tap) tap.

### Release tarball

Each tarball holds the four binaries and a `.sha256` file to check it against.
Put all four in one directory on your `PATH`: the plugin binary runs for every
hook event, and the translator serves providers that need it.

### Cargo

`cargo install` builds only the `ecodex` binary. Install the plugin binary too,
or the hooks cannot run (ecodex warns you on first session if it is missing):

```sh
cargo install --git https://github.com/EmpiricaAI/ecodex codex-cli
cargo install codex-empirica-plugin
cargo install codex-empirica-translator   # only for providers routed through the translator
```

### Source build

Builds the workspace in release mode (10–25 minutes the first time, minutes
after that), installs a wrapper at `~/.local/bin/ecodex` with the binary under
`~/.local/lib/ecodex/bin/`, and lays down the plugin and config immediately
instead of at the first session.

| Flag | Effect |
|---|---|
| `--user` (default) | Per-user install under `~/.local` and `~/.codex`. No sudo. |
| `--system` | Installs under `/usr/local` (or `--prefix DIR`) and writes `/etc/codex/requirements.toml`, which locks the empirica plugin on so a runtime cannot disable it without root. Needs sudo. |
| `--no-build` | Skips the build; point `ECODEX_BINARY` and `PLUGIN_BINARY` at prebuilt binaries. |
| `--fast` | Builds with a thin-LTO profile for quicker iteration. |

On a `--user` install a determined runtime can still disable the plugin, because
codex reads managed requirements only from `/etc/codex/requirements.toml`.

## What the first session sets up

The first time you start a session (`ecodex`, `ecodex exec`, `ecodex resume` or
`ecodex fork`), ecodex installs what it carries inside the binary. Commands such
as `ecodex mcp list` or `ecodex login` leave your files alone.

| Path | What ecodex does |
|---|---|
| `~/.codex/plugins/cache/empiricaAI/empirica/<version>/` | Writes the empirica plugin: manifest, hooks, MCP servers, skills, hook scripts and subagents. Rewritten whenever it differs from the copy inside the binary, so upgrades refresh it. |
| `~/.codex/config.toml` | Created from the bundled default (curated providers, plugin enabled) when it does not exist. An existing config is edited only to add `[plugins."empirica@empiricaAI"]` when it has no entry for the plugin. |
| `~/.codex/huggingface.config.toml` | Added when missing: a profile for Hugging Face Inference Providers. |
| `~/.codex/translator-upstreams.toml` | Added when missing: the translator's routes for Mistral (Devstral, Codestral). |

Your own settings always win. ecodex never overwrites an existing file other than
its plugin directory, and an explicit `enabled = false` on the plugin entry keeps
it off. A config from before the plugin's rename, with
`[plugins."empirica@nubaeon"]`, is migrated to the new key with its settings
intact.

Several sessions starting at once, as the empirica cockpit does, are safe: they
take turns.

## Verify

```sh
ecodex --version
empirica diagnose --frontend ecodex
```

`empirica diagnose` checks the plugin files, the hook scripts, the statusline and
the empirica CLI, and prints a hint for anything that fails.

## Configure a provider

The default config sets DeepSeek as the starting model and carries curated
entries for other providers. Each entry names the environment variable that holds
its key.

1. Get an API key, or start a local model server.
2. Export the variable the provider's entry names, for example
   `export DEEPSEEK_API_KEY=...`.
3. Run `ecodex` and pick the model with `/model`.

- **Local models.** Ollama (`localhost:11434`) and LM Studio (`localhost:1234`)
  are built in and need no config. llama.cpp and vLLM have entries in the default
  config.
- **Mistral.** Devstral and Codestral speak only Chat Completions, so they go
  through the translator: store the key and run `codex-empirica-translator`. See
  [`MISTRAL_SOVEREIGN.md`](MISTRAL_SOVEREIGN.md).
- **Hugging Face.** See [`integrations/huggingface.md`](integrations/huggingface.md).

You can switch provider mid-session with `/model`; the next turn uses the new one.

## Update

```sh
ecodex update
```

`ecodex update` works out how ecodex was installed and updates it the same way:

| Installed with | `ecodex update` runs |
|---|---|
| Install script | the install script again, into the directory ecodex runs from |
| Homebrew | `brew upgrade EmpiricaAI/tap/ecodex` |
| Cargo | `cargo install --git https://github.com/EmpiricaAI/ecodex codex-cli` |

For a source build, run `git pull && ./ecodex/scripts/install.sh` in your
checkout. If `ecodex update` says it cannot detect the install method, your
ecodex predates channel detection: run your install command once by hand, and
`ecodex update` works from then on.

The next session after an update refreshes the plugin. Sessions already running
keep the old binary until you restart them.

## Uninstall

For a source build, `./ecodex/scripts/uninstall.sh` removes the binaries and the
plugin cache; `--purge` also removes `~/.codex/`. For the other channels, remove
the four binaries (or `brew uninstall ecodex`) and
`~/.codex/plugins/cache/empiricaAI/`. To keep ecodex but run without the plugin,
set `enabled = false` under `[plugins."empirica@empiricaAI"]`.

## Troubleshooting

**"`codex-empirica-plugin` is not on PATH, so its hooks cannot run"**
The plugin binary must sit on your `PATH`. The install script, Homebrew and the
tarball install it alongside `ecodex`; after `cargo install`, run
`cargo install codex-empirica-plugin`.

**`empirica: command not found` in hook output**
Install the empirica CLI. The hooks fail quietly without it.

**"could not install the empirica plugin"**
ecodex could not write under `~/.codex` (or `$CODEX_HOME`). Check the directory's
permissions; the session continues without a refreshed plugin.

**The translator says it has no upstreams**
`~/.codex/translator-upstreams.toml` is missing. Start an ecodex session once to
create it, or pass `--upstreams-config <path>`.

**`Text file busy` while updating a source build**
The installer removes each binary before copying the new one, so this should not
happen. If it does, your checkout's installer is old: pull and retry.
