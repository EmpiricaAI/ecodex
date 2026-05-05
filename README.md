<p align="center"><strong>ecodex</strong> — the epistemic agent environment by Empirica.</p>
<p align="center"><em>A coding agent that measures what it knows.</em></p>

---

ecodex is a fork of [openai/codex](https://github.com/openai/codex) bundled with the **Empirica** epistemic-discipline framework. Where vanilla codex runs an agent loop and lets the model speak with whatever confidence it generates, ecodex gates the loop on a measured cycle:

- Every transaction opens with a **PREFLIGHT** declaring what the agent knows and doesn't.
- A **CHECK** gate decides whether the agent has enough context to act, or needs to keep investigating.
- **POSTFLIGHT** closes the loop and grounds the agent's self-assessment against deterministic services (test results, git metrics, artifact counts).
- A **Sentinel** firewall sits between the model and the tools — actions that would touch state require an open transaction with the right epistemic posture.

The result is an agent that can build a calibration history. Over time, the divergence between what the agent believed and what actually happened becomes a signal you can act on.

This is **not** a drop-in replacement for codex. It is opinionated: the discipline overhead is the point.

---

## Status

Alpha. Daily-driven by the ecodex team but not yet packaged for general install. The repo's main branches:

- `main` — clean tracking branch for upstream `openai/codex/main`. We rebase here and PR fixes upstream.
- `build/v1-plugin` (default) — the active ecodex work: the empirica plugin, the protocol translator, curated open-weights provider defaults, the koru-spiral welcome screen, and discipline wiring.

Public release timing is gated on T80 (docs suite) and T79 (release pipeline) — see open goals in `empirica goals-list`.

## What ecodex adds on top of codex

Three layers (full architecture: [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md)):

| Layer | Owns | Examples |
|---|---|---|
| **L1 — codex foundation** | Upstream | Agent runtime, TUI, sandbox, app-server, MCP, plugin host, hook system |
| **L2 — empirica integration** | Us (`codex-rs/codex-empirica-plugin/`) | Hook routing to Sentinel, transaction lifecycle, calibration grounding |
| **L3 — specialised ecodex code** | Us | Wire-protocol translator, curated provider defaults, `ecodex` wrapper, install/uninstall, lint scope, marketing surface |

Concretely, what users notice that vanilla codex doesn't do:

- **Curated open-weights provider defaults**: out-of-the-box config for DeepSeek, Qwen3-Coder, Kimi K2.6, GLM, Ollama, LM Studio, llama.cpp. Pick a provider in `/model` and routing swaps mid-session — no restart.
- **Wire-protocol translator** (`codex-rs/codex-empirica-translator/`): a small `tiny_http` bridge that lets codex's Responses-format API talk to providers that only speak Chat Completions or Anthropic Messages. CIF (Canonical Intermediate Format) validated at N=3 adapters.
- **Pinned epistemic skills**: framework-level skills (`epistemic-transaction`, `empirica-constitution`, `epistemic-persistence-protocol`) survive `/compact` so the agent retains its own discipline guidance across context boundaries.
- **Subagent seeding**: empirica's specialised subagents (security, ux, performance, outreach-scout, …) are bundled and dispatched through the standard codex Agent tool.
- **Statusline + welcome screen**: the koru-spiral animation matches Empirica's marketing identity; statusline shows live epistemic state (phase indicator, intuition-vs-search badge).

## Install

The install script lives at `ecodex/scripts/install.sh`:

```shell
git clone https://github.com/Nubaeon/ecodex.git
cd ecodex
./ecodex/scripts/install.sh
```

This builds the Rust workspace (`-p codex-cli --release`) and installs:

- `~/.local/bin/ecodex` — the wrapper
- `~/.local/lib/ecodex/bin/ecodex` — the binary
- `~/.local/bin/codex-empirica-plugin` — the plugin binary
- `~/.codex/plugins/cache/nubaeon/empirica/0.1.0/` — bundled hooks, MCP, skills, statusline

Empirica must be installed separately (the plugin shells out to its CLI). See [Empirica](https://github.com/Nubaeon/empirica) for the framework itself.

## Run

```shell
ecodex
```

The first run creates `~/.codex/config.toml` with the curated provider defaults. Add your API keys (see `~/.codex/.env.example`), pick a model with `/model`, and start a session.

## Documentation

- [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md) — three-layer architecture, runtime composition, file layout
- [`docs/ecodex/architecture.md`](docs/ecodex/architecture.md) — T3 decision record (distribution model, fork posture, integration strategy)
- [`docs/ecodex/inspection.md`](docs/ecodex/inspection.md) — T2 inspection of codex-rs (hook system, plugin marketplace, thread-scoped goals)
- [`codex-rs/codex-empirica-plugin/README.md`](codex-rs/codex-empirica-plugin/README.md) — plugin architecture, hook-by-hook status
- [`codex-rs/codex-empirica-translator/README.md`](codex-rs/codex-empirica-translator/README.md) — translator design, CIF, adapter map

## Relationship to upstream codex

ecodex is a **product fork**, not a derivative. Upstream improvements flow into us via `main` rebase; our hardening fixes go back via PRs against `openai/codex`. Branding is rebadged; the agent runtime, sandbox, RPC protocol, and plugin host are all upstream.

We do not rename, reorganize, or break upstream APIs. We add layers and curated defaults; we do not divert.

## License

Apache-2.0 (inherited from `openai/codex`). See [`LICENSE`](LICENSE).

ecodex is built by [Empirica](https://github.com/Nubaeon/empirica). Upstream codex is built by [OpenAI](https://github.com/openai/codex).
