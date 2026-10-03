# ecodex — Architecture

> **Scope.** A top-level orientation: what ecodex is made of, why the pieces are
> separate, and where the real detail lives. [`docs/ecodex/`](docs/ecodex/)
> covers each subject one at a time; this is the map across them. For the
> discipline engine's own architecture, see
> [Empirica](https://github.com/EmpiricaAI/empirica)'s `ARCHITECTURE.md` — ecodex
> is its harness, not a reimplementation.
>
> Numbers here are measured against this repo, not estimated. They drift; the
> shapes they illustrate are the durable part.

---

## The one idea

Vanilla [codex](https://github.com/openai/codex) runs an agent loop and lets the
model speak with whatever confidence it generates. **ecodex gates that loop on a
measured epistemic cycle.** It is a *product fork* of codex (Apache-2.0) with the
[**Empirica**](https://github.com/EmpiricaAI/empirica) discipline framework
bundled in, so the same agent that edits your code first declares — and is later
graded on — what it actually understands:

```
PREFLIGHT  →  [noetic: investigate]  →  CHECK  →  [praxic: change things]  →  POSTFLIGHT
   │                                      │                                       │
 declare beliefs                    gate reading→writing                   re-declare, and be
 before starting                    (a hook, not a prompt)                  graded against evidence
```

The enforcement is not advice to the model — it is a `PreToolUse` hook that
refuses edits and mutating shell commands until CHECK passes, and
self-assessment is scored against things the model does not control (tests,
commits, artifact ratios). The full reasoning lives in Empirica; ecodex's job is
to **carry that enforcement faithfully into the codex harness** while staying a
good citizen of the upstream codebase.

---

## Fork, not divergence

ecodex is codex plus a bounded, well-marked surface. Almost everything under
`codex-rs/` is upstream; ecodex's own additions are concentrated so that
periodic upstream re-syncs stay tractable.

| Layer | Where | What it is |
|---|---|---|
| **Upstream codex** | `codex-rs/` (most of 152 workspace crates) | The agent, TUI, exec, providers, MCP, sandbox — tracked via the `upstream` remote and merged in periodically |
| **The Empirica plugin** | `codex-rs/codex-empirica-plugin/` | Empirica's hooks, skills and agents, vendored and de-Claude'd, plus the Rust host that runs them; embedded in the `ecodex` binary |
| **The translator** | `codex-rs/codex-empirica-translator/` | Serves the Responses API locally and forwards to providers that speak only Chat Completions or Anthropic Messages (Mistral/Devstral and others) |
| **Fork touch-points** | about 300 files elsewhere under `codex-rs/` | Curated provider defaults, the `Monitor` tool, mid-session provider hot-swap, local-provider tool filtering, plugin provisioning at startup, the Empirica welcome, the ntfy mesh listener |

**The fork is distribution plus a thin behavioral surface, not a technical
divergence.** Quality issues found anywhere in the tree — upstream lint, bugs —
get fixed here *and* PR'd back upstream. Hardening flows both ways.

---

## The three moving parts

**1. The codex fork (Rust).** The shipped `ecodex` binary. It carries the
upstream agent plus the fork touch-points above. Its major.minor version *tracks
the upstream codex base* it was cut from, and ecodex releases bump only the patch
number on that base. That keeps the client version inside providers' per-model
version gates, which is why frontier models work over ChatGPT-subscription auth.

**2. The Empirica plugin (Python hooks, Rust host).** codex has a native
plugin and hook engine; the Empirica integration is a codex plugin. Its assets
(hooks, skills, agents, `hooks.json`) are embedded in the `ecodex` binary, and on
every interactive, `exec`, resume or fork start ecodex writes them to
`~/.codex/plugins/cache/empiricaAI/empirica/<ver>/` when they are missing or out
of date, and enables `empirica@empiricaAI` in `config.toml` unless the user set
`enabled = false`. codex then runs `codex-empirica-plugin`, which runs each
Python hook with the interpreter of the `empirica` CLI on PATH (the one named in
that script's shebang), so the hooks import the same empirica the CLI uses.
**There is nothing hook-shaped for a harness setup to write for codex; the plugin
is the integration.**

**3. The translator (Rust, optional sidecar).** `codex-empirica-translator`
bridges providers that do not speak the Responses API. Started by hand with no
flags, it listens on 127.0.0.1:18080, routes by model name using
`~/.codex/translator-upstreams.toml` (which ecodex writes on first start), and
takes keys from the environment or `~/.empirica/credentials.yaml`. Not needed for
OpenAI, Hugging Face or local Responses-speaking servers.

---

## The harness boundary

Enforcement lives in the **vendored hooks**, which codex itself runs on every
event, so the model cannot skip them by how it phrases a request or which tool it
picks. Nothing locks the plugin on: `enabled = false` on its entry in
`config.toml` turns it off. `codex-rs/codex-empirica-plugin/hooks.json` wires
them:

| Event | Hook | Does |
|---|---|---|
| `PreToolUse` | `sentinel-gate` | Classifies every tool call noetic/praxic by **effect, not name**; blocks praxic before CHECK. Over-gating a read is a defect too, so a read named to convention is classified correctly for free |
| `SessionStart` | host-side practice bootstrap, then `session-init`, `ewm-protocol-loader`, `post-compact`, `session-monitor-arm` | Makes the cwd a practice when it is not one (`git init`, `empirica project-init`; never in `$HOME`), binds the session and loads epistemic context and the workflow protocol; restores state after a compaction; arms the mesh listener when peer messaging is configured |
| `UserPromptSubmit` | `tool-router`, `context-shift-tracker` | Assesses each prompt against the current epistemic state; records whether a prompt answers the model's own question or starts something unasked |
| `PostToolUse` | `tool-failure`, `entity-extractor`, `truncation-legibility` (shell only) | Counts noetic vs praxic work; extracts the functions, classes and imports of edited files; tells the model when the output it just read was partial |
| `PostToolUseFailure` | `tool-failure` | Filters genuine dead-ends from operational noise (timeouts, signals, outages) before they become "avoid re-trying" retrieval; redacts credentials |
| `Stop` | `transaction-enforcer` | Blocks stopping when a transaction has run for many turns without a POSTFLIGHT |
| `PreCompact` / `PostCompact` | `pre-compact`, `post-compact` | Persists epistemic state across a context compaction and restores it after — compaction is routine and lossless by design |
| `SessionEnd`, `TaskCompleted`, `SubagentStart`/`SubagentStop` | `session-end-postflight`, `task-completed`, `subagent-*` | Close the measurement loop; bridge codex thread and subagent lifecycle to Empirica goals |

The hooks are **de-Claude'd**: model-facing Claude-isms are genericized so a
non-Claude model reads clean guidance. Which harness is a runtime fact carried by
`EMPIRICA_HARNESS`; the session's identity is codex's thread id, carried by
`EMPIRICA_INSTANCE_ID`.

---

## The de-Claude pipeline (the maintainer's spine)

The plugin is *vendored*, so it can drift from its Empirica source. The single
most load-bearing bit of ecodex-specific infrastructure is what keeps it honest:

- **`scripts/setup-codex.py`** — per-file diff of the vendored assets against
  `empirica@<ref>`, updates drifted files verbatim, lists new upstream files
  ecodex does not carry yet, scans for model-facing Claude-isms (report-only),
  verifies (`py_compile` plus the vendored-hooks test suite), and stamps the
  vendored version and commit into the plugin manifest. This is how each Empirica
  release re-vendors into ecodex.
- **`scripts/check_vendored_firewall.py`** — asserts the vendored firewall hooks
  keep their critical safety invariants (a behavioral check, not a content diff,
  since the vendored copy is deliberately genericized).
- **`scripts/check_empirica_core_pin.py`** — fails CI when the empirica commit
  CI tests the hooks against differs from the one they were vendored from.

> **The recurring hazard, named once:** a vendored asset that drifts from source
> is invisible until something reads the stale copy. A package upgrade of
> `empirica` does **not** fix a vendored hook — you must re-vendor. The CI
> guards exist to catch this class.

---

## The mesh (optional)

ecodex is a first-class peer in Empirica's AI-to-AI mesh:

- **Native ntfy listener** (in-process, started with the session; nothing to
  arm) wakes the session in seconds via a `<task-notification>` when a peer
  proposal arrives, not on the next prompt. The `Monitor` tool is the
  general-purpose sibling for streams the listener does not cover.
- **Cortex** carries ECO-gated proposals (`mailbox`); **git-notes messaging**
  carries server-less words. Rule of thumb: *messages carry words, proposals
  carry authority.* Everything mesh-related is optional — the binary is fully
  functional alone.

State (SQLite / git-notes / Qdrant) is Empirica's, not ecodex's — see Empirica's
`ARCHITECTURE.md` for the three-store model.

---

## The tree

| Path | What |
|---|---|
| `codex-rs/` | The Rust workspace (upstream codex + fork touch-points), 152 crates |
| `codex-rs/codex-empirica-plugin/` | The plugin host, the vendored Empirica hooks / skills / agents, the default config ecodex writes on first start, and the vendored-hooks test suite |
| `codex-rs/codex-empirica-translator/` | The Responses-API translator for Chat Completions and Anthropic providers |
| `scripts/` | `install.sh` (prebuilt install), `release.sh`, `sync-homebrew.sh`, `setup-codex.py` (re-vendor + de-Claude), the CI drift guards, `scoped_cargo_audit.py` |
| `ecodex/` | The source-build installer, its uninstaller and the `ecodex` wrapper script |
| `docs/ecodex/` | ecodex-specific docs: architecture decisions, `api/`, `integrations/`, `positioning/`, `specs/` |
| `.github/workflows/ci.yml` | Owned-crate build and test, the drift guards, the vendored-hook tests against a pinned empirica |
| `.github/workflows/release.yml` | On a version tag, builds the four release targets and attaches them to the GitHub release |
| `.github/workflows/security-audit.yml` | Weekly + PR-triggered `cargo audit`, scoped to what actually ships (via `cargo tree -i`, not raw Cargo.lock) |

---

## Known tensions

An architecture document that lists no problems is marketing.

- **Vendored-hook drift is structural.** The plugin is a *copy*; keeping it in
  sync is a maintainer step (`setup-codex`), and a hook that starts importing a
  new library file can break quietly, because `setup-codex` only re-syncs files
  ecodex already carries. The CI guards and the re-vendor discipline are the
  mitigation, not a cure.
- **Upstream-merge tax.** Every fork touch-point on a heavily-constructed
  upstream struct (provider fields, hook events, session tuples) is a future
  merge conflict — resolved by hand, verified by build, clippy and tests.
  Convergent features (both sides add the same thing) are the sharp edge.
- **The Python dependency.** The hooks need the `empirica` CLI installed next to
  ecodex. The installers set it up, but a hook can only be as current as the
  empirica it runs under.
- **Shellout latency.** 30–270 hook fires per session at roughly 100–300 ms each
  is a 1–15% overhead — tolerable for now; a sidecar or in-process path is the
  parked escalation.

---

## Where to go next

| You want | Read |
|---|---|
| What ecodex adds, and getting started | [`README.md`](README.md) |
| Install, update, providers, troubleshooting | [`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md) |
| The full subsystem tour | [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md) |
| The original fork decisions (historical) | [`docs/ecodex/architecture.md`](docs/ecodex/architecture.md) |
| Hooks / MCP / skills API | [`docs/ecodex/api/`](docs/ecodex/api/) |
| The cross-AI mesh | [`docs/ecodex/cross-ai-mesh.md`](docs/ecodex/cross-ai-mesh.md) |
| EU-sovereign Mistral/Devstral wiring | [`docs/ecodex/MISTRAL_SOVEREIGN.md`](docs/ecodex/MISTRAL_SOVEREIGN.md) |
| Contributing | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| The discipline engine itself | [Empirica](https://github.com/EmpiricaAI/empirica) + `/empirica-constitution` |
