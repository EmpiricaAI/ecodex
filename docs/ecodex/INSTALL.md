# Installing ecodex

ecodex installs the same way whichever channel you pick: you put the binaries on
your `PATH`, and the first session sets up everything else. That includes the
empirica plugin (the Sentinel, the hooks and the bundled skills), a curated
`config.toml` and the translator's routes. Every later upgrade refreshes the
plugin.

## Prerequisites

- **Linux or macOS.** Windows is not supported yet.
- **The `empirica` CLI.** The plugin's hooks run under it, so the Sentinel needs
  it. The install script, Homebrew and the source build set it up for you (with
  `pipx` or `uv` when it is missing). With cargo or a bare tarball, install it
  yourself: `pipx install empirica`.
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
SHA-256, installs four binaries into `~/.local/bin` (`ecodex`,
`codex-empirica-plugin`, `codex-empirica-translator` and `codex-code-mode-host`),
and sets up the empirica CLI if it is missing. Install elsewhere with
`--prefix DIR` or `ECODEX_INSTALL_DIR`, and pin a release with
`ECODEX_VERSION=<tag>`.

### Homebrew

Installs the same four prebuilt binaries from the
[`EmpiricaAI/homebrew-tap`](https://github.com/EmpiricaAI/homebrew-tap) tap, and
the tap's `empirica` formula with them.

### Release tarball

Each tarball holds the four binaries and a `.sha256` file to check it against.
Put all four in one directory on your `PATH`: the plugin binary runs for every
hook event, and the translator serves the providers that need it. Then install
the empirica CLI.

### Cargo

`cargo install` builds only the `ecodex` binary. Install the plugin binary, the
translator and the empirica CLI too:

```sh
cargo install --git https://github.com/EmpiricaAI/ecodex codex-cli
cargo install codex-empirica-plugin       # the hooks cannot run without it; ecodex warns if it is missing
cargo install codex-empirica-translator   # for Mistral, DeepSeek, Qwen, GLM and Kimi
pipx install empirica
```

### Source build

Builds the workspace in release mode (10–25 minutes the first time, minutes
after that), installs a wrapper at `~/.local/bin/ecodex` with the binary under
`~/.local/lib/ecodex/bin/`, puts the plugin binary and the translator beside it,
sets up the empirica CLI if it is missing, and lays down the plugin and config
immediately instead of at the first session.

| Flag | Effect |
|---|---|
| `--user` (default) | Per-user install under `~/.local` and `~/.codex`. No sudo. |
| `--system` | Installs the binaries under `/usr/local` (or `--prefix DIR`). Needs sudo. |
| `--no-build` | Skips the build; point `ECODEX_BINARY`, `PLUGIN_BINARY` and `TRANSLATOR_BINARY` at prebuilt binaries. |
| `--fast` | Builds with a thin-LTO profile for quicker iteration. |

## What the first session sets up

The first time you start a session (`ecodex`, `ecodex exec`, `ecodex resume` or
`ecodex fork`), ecodex installs what it carries inside the binary. Commands such
as `ecodex mcp list` or `ecodex login` leave your files alone.

| Path | What ecodex does |
|---|---|
| `~/.codex/plugins/cache/empiricaAI/empirica/<version>/` | Writes the empirica plugin: manifest, hooks, MCP servers, skills, hook scripts and subagents. Rewritten whenever it differs from the copy inside the binary, so upgrades refresh it. |
| `~/.codex/config.toml` | Created from the bundled default (curated providers, plugin enabled) when it does not exist. An existing config is edited only to add `[plugins."empirica@empiricaAI"]` when it has no entry for the plugin. Either way the plugin's Empirica MCP server is set off; [`api/mcp.md`](api/mcp.md) says when to turn it on. |
| `~/.codex/huggingface.config.toml` | Added when missing: a profile for Hugging Face Inference Providers. |
| `~/.codex/translator-upstreams.toml` | Added when missing: the translator's routes for Mistral, DeepSeek, Qwen, GLM and Kimi. |

Your own settings always win. ecodex never overwrites an existing file other than
its plugin directory, and an explicit `enabled = false` on the plugin entry keeps
it off. A config from before the plugin's rename, with
`[plugins."empirica@nubaeon"]`, is migrated to the new key with its settings
intact.

Several sessions starting at once, as the empirica cockpit does, are safe: they
take turns.

**The working directory becomes a practice.** When a session starts, ecodex
runs `git init` if the directory has no `.git`, and `empirica project-init` if it
has no `.empirica/`. That writes `.empirica/` into the directory and registers it
as a practice in `~/.empirica/workspace`, which is what the Sentinel measures
against. ecodex skips this in your home directory, at the filesystem root, and in
a directory inside another git repository; if it fails, the session continues
without a practice. With the plugin off, none of it happens.

## Verify

```sh
ecodex --version
empirica diagnose --frontend ecodex
```

`empirica diagnose` checks the plugin files, the hook scripts, the statusline and
the empirica CLI, and prints a hint for anything that fails.

## Choose a model provider

ecodex sets no default model; it starts on codex's own default until you pick one
with `/model`. codex speaks only the OpenAI Responses API, which splits the
providers in two.

**Direct.** These serve the Responses API themselves, so ecodex talks to them
straight away:

| Provider | What you do |
|---|---|
| OpenAI | Sign in with your ChatGPT account, or export `OPENAI_API_KEY`. Pick a GPT model in `/model`. |
| OpenRouter | Export `OPENROUTER_API_KEY`. |
| Hugging Face | Export `HF_TOKEN`; `ecodex -p huggingface` uses the bundled profile. See [`integrations/huggingface.md`](integrations/huggingface.md). |
| Local servers | Ollama (`localhost:11434`) and LM Studio (`localhost:1234`) need nothing; llama.cpp and vLLM have entries in the default config. Start the server and pick its model. |

**Through the translator.** Mistral (Devstral, Codestral), DeepSeek, Qwen, GLM
and Kimi speak only Chat Completions. `codex-empirica-translator`, installed with
ecodex, sits between them and codex on `127.0.0.1:18080`. Three steps:

1. **Store the key.** Either in the empirica key store,
   `~/.empirica/credentials.yaml`:

   ```yaml
   mistral:
     api_key: <your key>
   ```

   using the section `mistral`, `deepseek`, `dashscope` (Qwen), `zhipu` (GLM) or
   `moonshot` (Kimi), or as an environment variable in the shell that starts the
   translator: `MISTRAL_API_KEY`, `DEEPSEEK_API_KEY`, `DASHSCOPE_API_KEY`,
   `ZHIPU_API_KEY` or `MOONSHOT_API_KEY`. Never put a key in a ecodex config file.
2. **Start the translator** and leave it running, with no flags:

   ```sh
   codex-empirica-translator
   # or in the background:
   nohup codex-empirica-translator >~/.codex/translator.log 2>&1 &
   ```

   It logs a warning for each provider it has no key for and serves the rest.
   Restart it after adding a key.
3. **Pick the model** in `/model` (or `ecodex -c model_provider=mistral -m
   devstral-latest`). The default config already points these providers at the
   translator.

If your `~/.codex/config.toml` or `translator-upstreams.toml` was written by an
older ecodex, it is left as it is: copy the provider blocks from the
[default config](https://github.com/EmpiricaAI/ecodex/blob/main/codex-rs/codex-empirica-plugin/assets/config/config.toml.default)
and the routes from the
[default routes](https://github.com/EmpiricaAI/ecodex/blob/main/codex-rs/codex-empirica-plugin/assets/config/translator-upstreams.toml).
[`integrations/providers.md`](integrations/providers.md) has every provider,
model ids to start with, and how to add your own;
[`MISTRAL_SOVEREIGN.md`](MISTRAL_SOVEREIGN.md) walks through the EU route in
detail.

You can switch provider mid-session with `/model`; the next turn uses the new one.

## Unattended runs

The Sentinel expects a practitioner: before the model may change anything, it
has to open a transaction (PREFLIGHT). A CI job, a cron job or any `ecodex exec`
run that nobody watches never opens one. The session still sets the working
directory up as an empirica practice (see above), and the Sentinel then refuses
every command that writes or runs something ("No open transaction. Submit
PREFLIGHT ..."); reads still work.

Run such jobs with the plugin off. For a single run:

```sh
ecodex exec -c plugins.empirica@empiricaAI.enabled=false "<prompt>"
```

For a runner that only does unattended work, give it its own `CODEX_HOME` and
put this in that directory's `config.toml`:

```toml
[plugins."empirica@empiricaAI"]
enabled = false
```

ecodex keeps the entry through upgrades. It writes its default config only where
there is no `config.toml`, so add the provider settings the job needs to the same
file. With the plugin off, no hooks run and no `.empirica/` is created.

To keep the plugin's hooks running but stop the Sentinel refusing anything, set
`EMPIRICA_SENTINEL_LOOPING=false` in the job's environment instead. A
`~/.empirica/sentinel_enabled` file overrides it: if that file exists, it decides,
whatever the variable says.

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

The next session after an update refreshes the plugin. Sessions already running,
and a running translator, keep the old binary until you restart them.

## Uninstall

For a source build, `./ecodex/scripts/uninstall.sh` removes the binaries and the
plugin cache; `--purge` also moves `~/.codex/config.toml` aside to a timestamped
backup. For the other channels, remove
the four binaries (or `brew uninstall ecodex`) and
`~/.codex/plugins/cache/empiricaAI/`. To keep ecodex but run without the plugin,
set `enabled = false` under `[plugins."empirica@empiricaAI"]`.

## Troubleshooting

**"this CLI has no complete local package; install a packaged Codex CLI or use the standalone installer"**
Your ecodex tries to start upstream codex's background app-server, which needs a
packaged CLI layout ecodex does not ship. Current ecodex leaves it off; on an
older build, add `daemon_auto_start = false` under `[features]` in
`~/.codex/config.toml`, or start with `ecodex --no-daemon`. Then update.

**"`codex-empirica-plugin` is not on PATH, so its hooks cannot run"**
The plugin binary must sit on your `PATH`. The install script, Homebrew and the
tarball install it alongside `ecodex`; after `cargo install`, run
`cargo install codex-empirica-plugin`.

**"Empirica Sentinel is OFF" in a session**
The hooks run with the Python of the `empirica` command on your `PATH`, and that
Python could not import empirica. Check that `empirica --version` works in the
shell you start ecodex from, then run `empirica diagnose --frontend ecodex`.
Older ecodex builds used the first `python3` on `PATH` instead, which cannot see
a pipx or Homebrew empirica: update ecodex.

**"No CHECK, and no grounded claims declared at PREFLIGHT" right after a PREFLIGHT that declared claims**
The claims were not recorded. Core stores a claim only when its text is under
the key `claim`; an item keyed `statement` (the falsifier shape) is dropped
silently. The PREFLIGHT response echoes what it recorded under
`claims.declared` — if that is empty or short, fix the shape
(`{"claim": …, "grounding": "ran", "scope": …, "count": N}`) and re-run
PREFLIGHT, or submit CHECK. This is the Sentinel working, not a broken hook: a
hook failure reads *Hook failed · <Event>* in the TUI.

**"No open transaction. Submit PREFLIGHT ..." in an unattended job**
Nobody opened a transaction, so the Sentinel refuses the job's commands. Run the
job with the plugin off, as in [Unattended runs](#unattended-runs).

**"could not install the empirica plugin"**
ecodex could not write under `~/.codex` (or `$CODEX_HOME`). Check the directory's
permissions; the session continues without a refreshed plugin.

**The translator says "route skipped"**
It has no key for that provider. Add the key as in step 1 above and restart the
translator. If it refuses to start with "no route … has a key", it has no key for
any provider in its routes file.

**A Chat Completions provider fails straight away**
Its `[model_providers.*]` block still points at the provider's own API, which
codex cannot use. Point `base_url` at `http://localhost:18080/v1`, make sure the
translator has a route and a key for the model, and that it is running.

**The translator says it has no upstreams**
`~/.codex/translator-upstreams.toml` is missing. Start an ecodex session once to
create it, or pass `--upstreams-config <path>`.

**`Text file busy` while updating a source build**
The installer removes each binary before copying the new one, so this should not
happen. If it does, your checkout's installer is old: pull and retry.
