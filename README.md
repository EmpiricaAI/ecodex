<p align="center"><strong>ecodex</strong> — the epistemic agent environment by Empirica.</p>
<p align="center"><em>A coding agent that measures what it knows.</em></p>

---

ecodex is a fork of [openai/codex](https://github.com/openai/codex) bundled with the **Empirica** epistemic-discipline framework. Where vanilla codex runs an agent loop and lets the model speak with whatever confidence it generates, ecodex runs the loop inside a measured cycle:

- Every **transaction** (a unit of measured agent work) opens with a **PREFLIGHT**, where the agent declares what it knows and doesn't via 13 calibration vectors, and names the **claims** its next actions rest on — each graded by how it was grounded (`ran`, `read`, `retrieved`, `assumed`).
- A **CHECK** certifies what the agent's actions will rest on before it moves from investigating (the **noetic** phase) to acting (the **praxic** phase). If the agent did its reading before opening the transaction, grounded claims at PREFLIGHT certify it and no separate CHECK is needed.
- **POSTFLIGHT** closes the loop: each claim is adjudicated `held`, `refuted` or `untested`, and the agent's self-assessment is grounded against deterministic services — test results, git metrics, artifact counts — so the divergence between belief and outcome is recorded.
- A **Sentinel** firewall sits between the model and the tools and judges each call by its effect. Anything that can change state — `apply_patch`, file writes, commits, mutating shell commands and the write flags of read-only tools — needs an open, certified transaction. Reads and searches (`rg`, `cat`, `git log`) flow freely, until a hypothesis-bearing prompt arms the **investigation-proportionality budget**, which caps open-ended surveying after a configurable threshold.

The result is an agent that builds a calibration history. Over time, the divergence between what the agent believed and what actually happened becomes a signal you can act on.

This is **not** a drop-in replacement for codex. It is opinionated: the discipline overhead is the point.

---

## Status

Alpha. **`main` tracks upstream codex 0.157** (synced from the stable `rust-v0.157.0` tag; latest release [v0.154.0](https://github.com/EmpiricaAI/ecodex/releases)). The version tracks the upstream [openai/codex](https://github.com/openai/codex) base this build is derived from — hence the jump from `0.2.x` at the first rebased release (`0.146.0`). Tracking the base keeps the client version ecodex reports compatible with OpenAI's per-model version gates, so frontier models work over ChatGPT-subscription sign-in; ecodex patches increment as `0.157.x`, then move to the new base on each upstream sync. (First public release v0.1.0 was 2026-06-02.) The release pipeline (gated build/test/clippy, GitHub release, crates.io publish for owned crates, Homebrew tap) lives in `scripts/release.sh`.

`main` is the canonical branch and carries all active work: the empirica plugin, the protocol translator, curated open-weights provider defaults and the model registry, the native mesh listener, the Empirica welcome mark, and the discipline wiring. Upstream `openai/codex` is tracked through the `upstream` remote; stable upstream tags are merged in periodically (see `docs/ecodex/` and the upstream-sync issue template).

Public-facing pieces (CI workflows, contributor templates, expanded docs) land incrementally as we approach a broader launch. The CLI itself is daily-driven by the ecodex team.

## What ecodex adds on top of codex

Three layers (full architecture: [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md)):

| Layer | Owns | Examples |
|---|---|---|
| **L1 — codex foundation** | Upstream | Agent runtime, TUI, sandbox, app-server, MCP, plugin host, hook system |
| **L2 — empirica integration** | Us (`codex-rs/codex-empirica-plugin/`) | Hook routing to the Sentinel, transaction lifecycle, calibration grounding, the empirica base prompt and skills |
| **L3 — specialised ecodex code** | Us | Wire-protocol translator, curated provider defaults, model registry, native mesh listener, `ecodex` wrapper, install/uninstall |

What users notice that vanilla codex doesn't do:

- **An empirica base prompt.** The model's instructions are ecodex's own (`codex-rs/models-manager/prompt-empirica.md`): the transaction loop, the claims contract, the artifact types, and codex's operational guidance folded into one document, with a short reminder block seeded into `~/.codex/AGENTS.md`.
- **Curated open-weights provider defaults**: out-of-the-box config for DeepSeek, Qwen3-Coder, Kimi, GLM, Ollama, LM Studio, llama.cpp and vLLM. Pick a model in `/model` and routing swaps to its provider mid-session — no restart.
- **Curated model registry and discovery** (`ecodex models`): a capability-tagged seed of coding and agentic models (context, tool use, reasoning, route, jurisdiction) so the picker is short and relevant. `ecodex models refresh` probes your configured providers' `/v1/models` (including local Ollama, llama.cpp, vLLM and LM Studio) and adds only recognised coding-family models. Per-model `calibration_tier` starts `unmeasured` and fills from your own grounded usage. See [`docs/ecodex/integrations/model-registry.md`](docs/ecodex/integrations/model-registry.md).
- **Wire-protocol translator** (`codex-rs/codex-empirica-translator/`): a small bridge that lets codex's Responses-format API talk to providers that only speak Chat Completions or Anthropic Messages.
- **Empirica skills**: the transaction lifecycle, the empirica constitution and the persistence protocol are *framework* skills (`pinned: true`) — standing policy the model reads early and re-reads after a compaction — alongside task skills for code audits, docs alignment, subagent dispatch, workflow interviews and diagram rendering. Guaranteed-present guidance rides in the `AGENTS.md` reminder rather than in re-injected skill bodies, which keeps per-session overhead small enough for small-context models.
- **Subagent seeding**: empirica's specialised subagents (security, ux, performance, …) are bundled and spawned through codex's `spawn_agent` tool.
- **Statusline and welcome mark**: the welcome screen and fresh conversations show the Empirica "E" mark (it turns a few times, then fades; reduced-motion settings suppress the animation); the statusline shows live epistemic state (phase indicator, intuition-vs-search badge).
- **Cross-AI mesh participation**: ecodex sessions receive mesh events natively — an in-process listener holds the Cortex notification stream and wakes the session when a peer's proposal arrives, with no polling and no token cost while idle. Agents read and answer through the `empirica mailbox` CLI and the Cortex MCP tools, and the `monitor` tool wakes the agent on background subprocess output. A non-Claude model running in ecodex can receive a proposal from another practice, act on it and reply within seconds. See [`docs/ecodex/cross-ai-mesh.md`](docs/ecodex/cross-ai-mesh.md).
- **Extended hook event surface** (seven events beyond stock codex's): `TaskCompleted`, `PostToolUseFailure`, `PreCompact`, `PostCompact`, `SessionEnd`, `SubagentStart`, `SubagentStop` — lifecycle coverage so plugin handlers can enforce POSTFLIGHT, capture failures as dead-ends, snapshot state across compaction, and track parent→child subagent relationships. Plugin authors declare handlers in `hooks.json`. See [`docs/ecodex/hook-events-roadmap.md`](docs/ecodex/hook-events-roadmap.md).

## Install

| Channel | Command | Compiles? |
|---|---|---|
| **Install script** (Mac/Linux) | `curl -fsSL https://raw.githubusercontent.com/EmpiricaAI/ecodex/main/scripts/install.sh \| bash` | No — prebuilt |
| **Homebrew** (Mac/Linux) | `brew install EmpiricaAI/tap/ecodex` | No — prebuilt |
| **Direct binary** | Download `ecodex-<target>.tar.gz` from [Releases](https://github.com/EmpiricaAI/ecodex/releases/latest) | No — prebuilt |
| **Cargo** (Rust devs) | `cargo install --git https://github.com/EmpiricaAI/ecodex codex-cli` | Yes (source) |
| **Build from source** | `git clone … && cd ecodex && ./ecodex/scripts/install.sh` | Yes (source) |

The first three paths download prebuilt, stripped binaries for macOS (arm64/x64) and Linux (arm64/x64) — **no Rust toolchain, no compile**. Non-developers should use the install script or Homebrew. The cargo and source-build paths compile the workspace (10–25 min) and are for developers.

The empirica CLI must also be on `PATH` — install it from [`EmpiricaAI/empirica`](https://github.com/EmpiricaAI/empirica). Without it the plugin's hooks fail quietly and the discipline goes dark.

See [`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md) for `--user` vs `--system` installs, prerequisites, provider configuration and troubleshooting.

## Run

```shell
ecodex
```

The first run uses the curated `config.toml` defaults. Add your API keys (per-provider environment variables are documented in the seeded config), pick a model with `/model`, and start a session. Switch provider mid-session through `/model` — no restart needed.

## Glossary

| Term | What it means |
|---|---|
| **transaction** | One measured chunk of agent work, framed by PREFLIGHT → (CHECK →) praxic → POSTFLIGHT and linked to a goal. |
| **PREFLIGHT** | The transaction-opening assessment: 13 calibration vectors (`know`, `uncertainty`, `do`, `clarity`, …) describing the agent's belief about its epistemic state, plus the claims its work rests on. |
| **claim** | A belief the agent's next actions depend on, graded by how it was grounded: `ran` (executed and observed, with the scope measured and the count returned), `read`, `retrieved` (from the practice's own prior artifact) or `assumed`. Adjudicated at POSTFLIGHT. |
| **CHECK** | Certifies what the praxic work rests on before the agent moves from investigating to acting. Returns `proceed` or `investigate`. Skipped when PREFLIGHT claims already certify the transaction. |
| **POSTFLIGHT** | The transaction-closing reflection: re-declares vectors, adjudicates claims, and is compared against deterministic observations (lint, tests, git metrics) to compute calibration deltas. |
| **falsifier** | The observation that would refute a belief the agent is acting on, registered before the evidence arrives. Unlike a claim it outlives the transaction and resurfaces until adjudicated. |
| **noetic** | Investigation: reads and searches, always allowed. |
| **praxic** | Action: anything that can change state, which needs an open, certified transaction. |
| **Sentinel** | The firewall that gates praxic tool calls on transaction state. Lives in `sentinel-gate.py` and runs as a codex `PreToolUse` hook. |
| **calibration** | The divergence between an agent's stated vector and the deterministic observation — the signal that improves over time. |
| **artifact** | A logged epistemic unit: finding (verified discovery), unknown (open question), assumption (unverified belief), decision (chosen path), dead end (failed approach), mistake (the agent's own error), goal (target). Artifacts are connected into a graph. |
| **practice** | An empirica project: the goals, artifacts and history that outlive any one session. The model working in it is the practitioner. |
| **investigation-proportionality budget** | A per-session counter that caps open-ended reads and searches after a hypothesis-bearing prompt arms it. Prevents investigation-as-procrastination. |
| **monitor** | ecodex tool that watches a subprocess's output for a pattern; each matching line is injected into the agent's pending input — sub-second wake on background events. |
| **cross-AI mesh** | Empirica's AI-to-AI communication layer: practices send each other proposals through Cortex; ecodex receives them through its native listener and answers through the mailbox CLI and Cortex tools. |

For the full vocabulary and how the pieces compose, see [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md).

## Documentation

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — **start here** — the top-level map: fork boundary, the three moving parts, the harness/enforcement layer, the de-Claude pipeline, known tensions
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — three-layer contribution model, dev workflow, conventions
- [`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md) — install modes, providers, troubleshooting
- [`docs/ecodex/integrations/model-notes.md`](docs/ecodex/integrations/model-notes.md) — which providers and models to use, and how each one authenticates
- [`docs/ecodex/MISTRAL_SOVEREIGN.md`](docs/ecodex/MISTRAL_SOVEREIGN.md) — wiring EU-sovereign Mistral models (Devstral/Codestral) for data residency and cost
- [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md) — three-layer architecture, runtime composition, file layout
- [`docs/ecodex/architecture.md`](docs/ecodex/architecture.md) — decision record (distribution model, fork posture, integration strategy)
- [`docs/ecodex/inspection.md`](docs/ecodex/inspection.md) — inspection of codex-rs (hook system, plugin marketplace, thread-scoped goals)
- [`docs/ecodex/api/`](docs/ecodex/api/) — plugin API contracts (`hooks.md`, `skills.md`, `mcp.md`, `plugin-statusline.md`, `plugin-writable-roots.md`, `integration-tests.md`)
- [`codex-rs/codex-empirica-plugin/README.md`](codex-rs/codex-empirica-plugin/README.md) — plugin architecture, hook-by-hook status
- [`codex-rs/codex-empirica-translator/README.md`](codex-rs/codex-empirica-translator/README.md) — translator design and adapter map

## Relationship to upstream codex

ecodex is a **product fork**, not a derivative. Upstream improvements flow in by merging stable upstream tags into `main`; our hardening fixes go back as PRs against `openai/codex`. The agent runtime, sandbox, RPC protocol, plugin host and hook system all come from upstream.

What ecodex adds is mostly *additive*: new crates (`codex-empirica-plugin`, `codex-empirica-translator`), new manifest fields (`writableRoots`, `statusline`), new provider entries, the empirica base prompt, and an install script that handles the bundled plugin layout. We do **not** rename, reorganise or break upstream APIs. Where a feature has to live inside upstream code — the mid-session provider swap, the native mesh listener, the extra hook events, the welcome mark — it is kept small and marked as an ecodex extension, so each upstream merge can carry it forward.

The one policy exception we maintain inside upstream code is the `ECODEX_AUTO_TRUSTED_PLUGIN_IDS` allowlist in `codex-rs/hooks/src/engine/discovery.rs` — first-party plugin trust on first install, in lieu of upstream's user-trust review flow. When empirica becomes a marketplace plugin, this comes off and we use the upstream-intended flow.

## License

Apache-2.0 (inherited from `openai/codex`). See [`LICENSE`](LICENSE).

ecodex is built by [Empirica](https://github.com/EmpiricaAI/empirica). Upstream codex is built by [OpenAI](https://github.com/openai/codex).
