<p align="center"><strong>ecodex</strong> — the epistemic agent environment by Empirica.</p>
<p align="center"><em>A coding agent that measures what it knows.</em></p>

---

ecodex is [openai/codex](https://github.com/openai/codex) with the **Empirica** epistemic-discipline framework built in. Vanilla codex runs an agent loop and lets the model speak with whatever confidence it generates. ecodex runs the same loop inside a measured cycle, and shows you the measurement while the agent works:

```
 ecodex │ CHECK 82% │ G3 U1 A2 F14/D5 │ Δ ✓ │ 🔨 act - devstral-latest
```

That line sits under the prompt for the whole session: which practice the agent is in, where its transaction stands and how confident it is, what it has open (goals, unknowns, assumptions) and logged (findings, decisions), whether its self-assessment moved toward the evidence since last time, whether the Sentinel would let it act right now — and which model is driving.

## The loop

- Every **transaction** (a unit of measured agent work) opens with a **PREFLIGHT**: the agent declares what it knows and doesn't across 13 calibration vectors, and names the **claims** its next actions rest on, each graded by how it was grounded (`ran`, `read`, `retrieved`, `assumed`).
- A **CHECK** certifies what the agent's actions will rest on before it moves from investigating (the **noetic** phase) to acting (the **praxic** phase). An agent that did its reading before opening the transaction certifies it at PREFLIGHT and needs no separate CHECK.
- **POSTFLIGHT** closes the loop: each claim is adjudicated `held`, `refuted` or `untested`, and the agent's self-assessment is graded against deterministic evidence — test results, git metrics, artifact counts — so the gap between belief and outcome is recorded.
- The **Sentinel** sits between the model and the tools and judges every call by its effect. Anything that can change state — `apply_patch`, file writes, commits, mutating shell commands, the write flags of read-only tools — needs an open, certified transaction. Reads and searches flow freely, until a hypothesis-bearing prompt arms the **investigation-proportionality budget**, which caps open-ended surveying.

Over sessions this builds a **calibration history**: the divergence between what the agent believed and what happened becomes a signal you can act on — per practice, per model.

This is **not** a drop-in replacement for codex. It is opinionated: the discipline overhead is the point.

---

## Status

Alpha. **`main` tracks upstream codex 0.164** (synced from the stable `rust-v0.164.0` tag; latest release [v0.164.2](https://github.com/EmpiricaAI/ecodex/releases)). The version tracks the upstream [openai/codex](https://github.com/openai/codex) base this build is derived from — hence the jump from `0.2.x` at the first rebased release (`0.146.0`). Tracking the base keeps the client version ecodex reports compatible with OpenAI's per-model version gates, so frontier models work over ChatGPT-subscription sign-in; ecodex patches increment as `0.164.x`, then move to the new base on each upstream sync. (First public release v0.1.0 was 2026-06-02.) The release pipeline (gated build/test/clippy, GitHub release, crates.io publish for owned crates, Homebrew tap) lives in `scripts/release.sh`.

`main` is the canonical branch and carries all active work. Upstream `openai/codex` is tracked through the `upstream` remote; stable upstream tags are merged in periodically. The CLI is daily-driven by the ecodex team.

## What's New in 0.167.2

- Plugin writable roots survive a permission-profile change
- Four `hickory-proto` 0.25.2 advisories accepted
- Empirica hooks re-vendored from core 1.16.0
- The installer sets up `empirica[api]`
- The forecast is re-made at 0.167.1
- CI runs the owned-crate clippy gate

Plus 1 more entries in the [CHANGELOG](CHANGELOG.md).

**Earlier releases**

- **0.167.1** — The release binary compiles again
- **0.167.0** — The plugin records when codex asks a person to approve a call

## What's Coming

ecodex's own forecast, made in 0.167.1 from its open work and its known weak spots. Each prediction carries a confidence, the evidence behind it and the observation that would prove it wrong, and is graded when it ships or is dropped. The word beside the confidence says how the basis is known: **measured** by a command, **reviewed** in the code, **reported** by someone else, or **inferred**; weaker evidence cannot carry a higher confidence. **planned** means a goal for it already existed when we predicted it: those are scored separately, because delivering planned work is not foresight.

| Direction | Confidence | Why we expect it | What would change our mind |
|---|---|---|---|
| **README and docs match what ships**: By the 0.167.2 release, the README and the docs that describe Empirica behaviour match the code, including the bundled skill set, which skills are framework skills, and the removal of Shared Epistemic Records. | 70% (reviewed, planned) | Reviewed in the docs. Since core 1.14.14 the constitution skill became an index over three new skills, Shared Epistemic Records were removed in favour of proposals with several targets, and a forecasting skill was added. At 0.167.1 the skills page names exactly the sixteen bundled skills, but dated experiment designs under docs/ecodex/experiments still describe Shared Epistemic Records as the mechanism they serve. | At the 0.167.2 tag, the README or a page under docs/ecodex names a skill that is not bundled in that tag, leaves a bundled skill out of a list that presents itself as complete, calls a skill a framework skill when its bundled SKILL.md is not pinned (or the reverse), or describes Shared Epistemic Records as a mechanism ecodex still uses. |
| **Every release ships all four binaries within six hours**: From 0.167.2 on, every ecodex release has its four binaries (Linux and macOS, x86_64 and arm64) attached to the GitHub release within six hours of the tag. | 70% (measured) | Measured on 0.167.1. The Linux builds take about an hour, the macOS builds about three (2 h 55 on the Intel runner, 3 h 05 on Apple Silicon), under the six-hour job limit the workflow now sets. The risk is GitHub's macOS arm64 pool, which refused the job twice in one day before a third attempt was picked up; the Intel pool was never short. | A release tag whose GitHub release still lacks one of the four tarballs six hours after the tag was pushed. |
| **Core 1.16 reaches codex seats within two days**: Within two days of empirica core 1.16.0's release, ecodex 0.167.2 ships with its hooks re-vendored from it, so apply_patch gets discipline mode's small-edit budget on codex seats and planned goals carry due dates. | 65% (reported, planned) | Core reports the apply_patch budget built on its development branch, with ecodex's hook payload shape confirmed (the patch text arrives in the command field), and the due-date half of goals released in 1.15.0. The re-vendor itself is a scripted step ecodex has run for every core release since 1.14.15, usually the same day. The two-day window is a bet on core's release landing when ecodex is not mid-fold-in. | More than two days pass between core's 1.16.0 release on GitHub and ecodex's v0.167.2 release, or in v0.167.2 an apply_patch that only adds or edits files inside the practice is refused by the Sentinel outside a transaction in discipline mode. |
| **Plugin writable roots survive a permission change**: In 0.167.2, choosing a permission profile mid-session keeps the writable roots the empirica plugin declares, and a test covers it. | 60% (reviewed, planned) | Read in the code at 0.167.1. A profile chosen mid-session is resolved again from config without the plugin's roots, which are only added when a session starts, so later writes to the plugin's state directory are refused by the sandbox. The fix is in ecodex's own plugin-roots code, not upstream's. | In 0.167.2, a session that changes its permission profile mid-session cannot write to ~/.empirica, or no test changes the profile with a root-declaring plugin loaded. |
| **Upstream stables folded in within a week**: From upstream codex's next stable tag on, ecodex releases its matching version within seven days of that tag. | 50% (measured) | Measured over the last three upstream stables. 0.164 was matched within four hours, 0.165 and 0.166 were skipped, and 0.167 was matched after four and a half days. A fold-in of 450 upstream commits took three working days including the audit, test and release-build fixes it surfaced, and a release-profile check now runs in CI so that class of miss costs no second release. | An upstream rust-v stable tag published after this forecast with no matching ecodex release seven days later. |
| **A local decision model is scored against the Sentinel**: By 0.169.0, ecodex carries an evaluation that runs a local decision model over the per-call decisions codex seats already record and reports its agreement with the Sentinel's gate per class of call. | 50% (inferred, planned) | Reasoned from what exists. Since core 1.14.22 every gated call on a codex seat is recorded with its command class and outcome, which is the labelled data such an evaluation needs; the Strix Halo box serves local models through an OpenAI-compatible endpoint. Nothing of the evaluation itself is written yet, and no model has been chosen. | At the 0.169.0 tag, no script or page in the repository runs a decision model over recorded gate decisions and reports agreement per class. |

Forecast record: 2 shipped (0 of them not on the plan when predicted), 1 dropped, 0 retracted, 0 superseded so far.

## What ecodex adds on top of codex

Three layers (full architecture: [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md)):

| Layer | Owns | Examples |
|---|---|---|
| **L1 — codex foundation** | Upstream | Agent runtime, TUI, sandbox, app-server, MCP, plugin host, hook system |
| **L2 — empirica integration** | Us (`codex-rs/codex-empirica-plugin/`) | Hook routing to the Sentinel, transaction lifecycle, calibration grounding, the empirica base prompt, skills, statusline |
| **L3 — specialised ecodex code** | Us | Wire-protocol translator, curated model seed and `/model` picker, native mesh listener, `ecodex` wrapper, install/uninstall |

### What you see

- **The statusline.** Live epistemic state under the prompt, refreshed every 1.5 s from the practice's own records — the line above. Compact by default; `echo expanded > ~/.empirica/statusline_mode` switches to the fuller layout (phase composite, key vectors, CHECK gate) without a restart. Contract: [`docs/ecodex/api/plugin-statusline.md`](docs/ecodex/api/plugin-statusline.md).
- **The welcome mark.** The welcome screen and fresh conversations show the Empirica "E" mark; it turns a few times, then fades (reduced-motion settings suppress the animation).
- **The Sentinel talking back.** A praxic call before CHECK is refused with the reason and the remedy, inline — the hook result is what the model reads next, so the discipline is taught in the loop rather than in a manual.
- **Hook outcomes by name.** When a hook fails, the TUI says which event it was (`Hook failed · SessionStart`), so a broken install is diagnosable from the screen.

### What the agent gets

- **An empirica base prompt.** The model's instructions are ecodex's own (`codex-rs/models-manager/prompt-empirica.md`): the transaction loop, the claims contract, the artifact types and codex's operational guidance in one document, with a short reminder block seeded into `~/.codex/AGENTS.md` so the essentials survive any compaction.
- **Empirica skills**: the transaction lifecycle and the persistence protocol are *framework* skills (`pinned: true`) the model reads early and re-reads after a compaction. Task skills cover the rest: an index that routes each moment to its skill, artifact typing and retraction, which practice work belongs to, writing guidance, code audits, docs alignment, quality sweeps, pre-release gates, forecasting, subagent dispatch, workflow interviews, diagnosis, onboarding and diagram rendering.
- **Subagent seeding**: empirica's specialised subagents (security, ux, performance, …) are bundled and spawned through codex's `spawn_agent` tool.
- **A practice wherever it runs.** The first session in a directory makes it an empirica practice (`git init` if needed, `empirica project-init`) — never in `$HOME` — so the measurement has somewhere to accrue from the first turn.
- **Two extra hook events**: `TaskCompleted` (the agent declares a task done) and `PostToolUseFailure` (a tool call failed), on top of upstream codex's lifecycle events, so handlers can enforce POSTFLIGHT at completion and record a failed call as a dead-end. See [`docs/ecodex/hook-events-roadmap.md`](docs/ecodex/hook-events-roadmap.md).

### Models and providers

- **A curated `/model` picker.** Twelve entries across four categories — cloud coding, cloud reasoning, local open-weights, routers — derived from one seed (`codex-rs/models-manager/models.curated.json`) with context, tool use, reasoning, route and jurisdiction tagged. Pick a model and routing swaps to its provider mid-session; no restart. A bare OpenAI id always goes to OpenAI, whatever provider was active.
- **Curated open-weights provider defaults**: out-of-the-box config for Mistral (EU-hosted), DeepSeek, Qwen, GLM, Kimi, OpenRouter, Hugging Face, Ollama, LM Studio, llama.cpp and vLLM.
- **Model registry and discovery** (`ecodex models`): `ecodex models refresh` probes your configured providers' `/v1/models` — local servers included — and adds only recognised coding-family models. Per-model `calibration_tier` starts `unmeasured` and fills from your own grounded usage. See [`docs/ecodex/integrations/model-registry.md`](docs/ecodex/integrations/model-registry.md).
- **Wire-protocol translator** (`codex-rs/codex-empirica-translator/`): codex speaks only the Responses API; the translator lets it talk to providers that only speak Chat Completions or Anthropic Messages, repairs the tool-call habits of chat models, and rides out rate limits.

### Working with other agents

- **Cross-AI mesh participation**: an in-process listener holds the Cortex notification stream and wakes the session when a peer practice's proposal arrives — no polling, no token cost while idle. Agents read and answer through the `empirica mailbox` CLI and the Cortex MCP tools. Any model running in ecodex can receive a proposal from another practice, act on it and reply within seconds. See [`docs/ecodex/cross-ai-mesh.md`](docs/ecodex/cross-ai-mesh.md).
- **The `monitor` tool** wakes the agent on background subprocess output (a build, a log, a CI run), one matching line at a time.
- **Several instances at once.** Each ecodex session is keyed by its own thread id, so many agents can run in one directory or across practices without sharing state. empirica's cockpit can lay them out in tmux, a pane per practice, each running ecodex in that practice's directory; the `ewm-interview` skill drafts the cockpit profile for the practices it provisions. `empirica cockpit refresh` respawns only Claude Code panes so far, so a dead ecodex pane is restarted by hand with `ecodex resume --last`.
- **Unattended runs.** `ecodex exec` in CI or cron has no practitioner to open a transaction; [`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md#unattended-runs) shows how to run the job with the plugin off and keep the practice intact.

## Install

| Channel | Command | Compiles? |
|---|---|---|
| **Install script** (Mac/Linux) | `curl -fsSL https://raw.githubusercontent.com/EmpiricaAI/ecodex/main/scripts/install.sh \| bash` | No — prebuilt |
| **Homebrew** (Mac/Linux) | `brew install EmpiricaAI/tap/ecodex` | No — prebuilt |
| **Release tarball** | Download `ecodex-<target>.tar.gz` from [Releases](https://github.com/EmpiricaAI/ecodex/releases/latest) | No — prebuilt |
| **Cargo** (Rust devs) | `cargo install --git https://github.com/EmpiricaAI/ecodex codex-cli` | Yes (source) |
| **Build from source** | `git clone … && cd ecodex && ./ecodex/scripts/install.sh` | Yes (source) |

Every channel ends up the same. You put the binaries on your `PATH`, and the first session sets up the rest: the empirica plugin (the Sentinel, the hooks, the skills, the statusline) and a curated `~/.codex/config.toml`. Each upgrade refreshes the plugin, and `ecodex update` upgrades through whichever channel you used.

Non-developers should use the install script or Homebrew: prebuilt binaries for macOS and Linux, no Rust toolchain. The cargo and source-build channels compile the workspace (10–25 min). With cargo, also run `cargo install codex-empirica-plugin codex-empirica-translator` so the hooks and the translator are there.

The plugin's hooks run under the [empirica](https://github.com/EmpiricaAI/empirica) CLI. The install script, Homebrew and the source build install it for you when it is missing; with cargo or a bare tarball, run `pipx install 'empirica[api]'` (the `[api]` extra is the local mesh server that several practices on one machine use without a Cortex).

[`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md) covers what the first session writes, updating, uninstalling and troubleshooting.

## Run

```shell
ecodex
```

If you have no `~/.codex/config.toml`, the first session writes the curated one. It sets no default model: pick one with `/model`. You can switch provider mid-session through `/model` without restarting.

codex speaks only the OpenAI Responses API, so providers come in two kinds:

- **Direct** — OpenAI (sign in, or `OPENAI_API_KEY`), OpenRouter (`OPENROUTER_API_KEY`), Hugging Face (`HF_TOKEN`), and local servers (Ollama and LM Studio need no config; llama.cpp and vLLM are in the default config). Set the key or start the server, then pick the model.
- **Through the translator** — Mistral (Devstral, Codestral), DeepSeek, Qwen, GLM and Kimi speak only Chat Completions, and Anthropic (Claude) speaks the Messages API. `codex-empirica-translator`, installed with ecodex, bridges them:
  1. Put the key in `~/.empirica/credentials.yaml` under the provider's section (`mistral`, `deepseek`, `dashscope`, `zhipu`, `moonshot` or `anthropic`) as `api_key: …`, or export `MISTRAL_API_KEY` / `DEEPSEEK_API_KEY` / `DASHSCOPE_API_KEY` / `ZHIPU_API_KEY` / `MOONSHOT_API_KEY` / `ANTHROPIC_API_KEY`.
  2. Run `codex-empirica-translator` (no flags) and leave it running. It serves the providers it has keys for and warns about the rest.
  3. Pick the model in `/model`, for example `devstral-latest`.

[`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md#choose-a-model-provider) has the details, including what to copy if your config predates these routes; [`docs/ecodex/MISTRAL_SOVEREIGN.md`](docs/ecodex/MISTRAL_SOVEREIGN.md) walks through the EU route; [`docs/ecodex/epistemic-llms.md`](docs/ecodex/epistemic-llms.md) says which models hold up under the discipline.

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
| **statusline** | The one-line view of the practice's live epistemic state under the prompt: stage, confidence, open and logged artifacts, learning delta, investigate-or-act, model. |
| **monitor** | ecodex tool that watches a subprocess's output for a pattern; each matching line is injected into the agent's pending input — sub-second wake on background events. |
| **cross-AI mesh** | Empirica's AI-to-AI communication layer: practices send each other proposals through Cortex; ecodex receives them through its native listener and answers through the mailbox CLI and Cortex tools. |

For the full vocabulary and how the pieces compose, see [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md).

## Documentation

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — **start here** — the top-level map: fork boundary, the three moving parts, the harness/enforcement layer, the de-Claude pipeline, known tensions
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — three-layer contribution model, dev workflow, conventions
- [`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md) — install channels, first session, providers, unattended runs, troubleshooting
- [`docs/ecodex/epistemic-llms.md`](docs/ecodex/epistemic-llms.md) — which models behave well under measured discipline, and the picker
- [`docs/ecodex/integrations/model-notes.md`](docs/ecodex/integrations/model-notes.md) — which providers and models to use, and how each one authenticates
- [`docs/ecodex/MISTRAL_SOVEREIGN.md`](docs/ecodex/MISTRAL_SOVEREIGN.md) — wiring EU-sovereign Mistral models (Devstral/Codestral) for data residency and cost
- [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md) — three-layer architecture, runtime composition, file layout
- [`docs/ecodex/cross-ai-mesh.md`](docs/ecodex/cross-ai-mesh.md) — the mesh: listener, mailbox, Cortex MCP, walkthrough
- [`docs/ecodex/api/`](docs/ecodex/api/) — plugin API contracts (`hooks.md`, `skills.md`, `mcp.md` (no Empirica MCP server), `plugin-statusline.md`, `plugin-writable-roots.md`, `integration-tests.md`)
- [`codex-rs/codex-empirica-plugin/README.md`](codex-rs/codex-empirica-plugin/README.md) — the plugin: handlers, provisioning, manifest surfaces
- [`codex-rs/codex-empirica-translator/README.md`](codex-rs/codex-empirica-translator/README.md) — the translator: routes, adapters, repairs
- [`docs/ecodex/README.md`](docs/ecodex/README.md) — index of everything under `docs/ecodex/`, including the historical records

## Relationship to upstream codex

ecodex is a **product fork**, not a derivative. Upstream improvements flow in by merging stable upstream tags into `main`; our hardening fixes go back as PRs against `openai/codex`. The agent runtime, sandbox, RPC protocol, plugin host and hook system all come from upstream.

What ecodex adds is mostly *additive*: new crates (`codex-empirica-plugin`, `codex-empirica-translator`), new manifest fields (`writableRoots`, `statusline`), new provider entries, the empirica base prompt, installers, and the startup step that writes the bundled plugin. We do **not** rename, reorganise or break upstream APIs. Where a feature has to live inside upstream code — the mid-session provider swap, the native mesh listener, the extra hook events, the welcome mark — it is kept small and marked as an ecodex extension, so each upstream merge can carry it forward.

The one policy exception we maintain inside upstream code is the `ECODEX_AUTO_TRUSTED_PLUGIN_IDS` allowlist in `codex-rs/hooks/src/engine/discovery.rs` — first-party plugin trust on first install, in lieu of upstream's user-trust review flow. When empirica becomes a marketplace plugin, this comes off and we use the upstream-intended flow.

## License

Apache-2.0 (inherited from `openai/codex`). See [`LICENSE`](LICENSE).

ecodex is built by [Empirica](https://github.com/EmpiricaAI/empirica). Upstream codex is built by [OpenAI](https://github.com/openai/codex).
