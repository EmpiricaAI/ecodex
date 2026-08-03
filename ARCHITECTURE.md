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
refuses `Edit`/`Write`/mutating shell until CHECK passes, and self-assessment is
scored against things the model does not control (tests, commits, artifact
ratios). The full reasoning lives in Empirica; ecodex's job is to **carry that
enforcement faithfully into the codex harness** while staying a good citizen of
the upstream codebase.

---

## Fork, not divergence

ecodex is codex plus a bounded, well-marked surface. Almost everything under
`codex-rs/` is upstream; ecodex's own additions are deliberately concentrated so
that periodic upstream re-syncs stay tractable.

| Layer | Where | What it is |
|---|---|---|
| **Upstream codex** | `codex-rs/` (most crates) | The agent, TUI, exec, providers, MCP, sandbox — tracked via the `upstream` remote and merged in periodically |
| **The vendored Empirica plugin** | `codex-rs/codex-empirica-plugin/` | Empirica's hooks + skills + agents, de-Claude'd and compiled into the binary |
| **The translator** | `codex-rs/codex-empirica-translator/` | A shim that adapts chat-completions providers (Mistral/Devstral, local llama.cpp/vLLM) to what codex expects |
| **Fork touch-points** | ~130 files across `codex-rs/` | Curated provider defaults, the `Monitor` tool, provider hot-swap (T78), local-provider tool filtering (Task C), the static Empirica welcome, the ntfy mesh listener |

**The fork is distribution + a thin behavioral surface, not a technical
divergence.** Quality issues found anywhere in the tree — upstream lint, bugs —
get fixed here *and* PR'd back upstream. Hardening flows both ways.

---

## The three moving parts

**1. The codex fork (Rust).** The shipped `ecodex` binary. It carries the
upstream agent plus the fork touch-points above. Its version *tracks the upstream
codex base* it was cut from (hence `0.146.0`, not `0.2.x`) so the client version
stays compatible with providers' per-model version gates — the reason frontier
models like gpt-5.6 work over ChatGPT-subscription auth.

**2. The vendored Empirica plugin (Python, deployed at build/install).** codex has
a native plugin/hook engine; ecodex ships the Empirica integration as assets under
`codex-empirica-plugin/assets/` (hooks, skills, agents) that deploy to
`~/.codex/plugins/cache/nubaeon/empirica/<ver>/`. The hooks shell out to the
canonical `empirica` Python CLI — the same subprocess-shellout model the Claude
Code integration uses. **There is nothing hook-shaped for a harness setup to
write for codex; the plugin is the integration.**

**3. The translator (Rust, optional sidecar).** `codex-empirica-translator`
bridges chat-completions-only providers into codex's Responses-API expectations.
Not needed for OpenAI-direct models; needed for the curated open-weights /
sovereign-EU providers.

---

## The harness boundary

Enforcement lives in the **vendored hooks**, which codex loads natively — the AI
cannot bypass them:

| Hook | Does |
|---|---|
| `sentinel-gate` | Classifies every tool call noetic/praxic by **effect, not name**; blocks praxic before CHECK. Over-gating a read is a defect too, so a read named to convention is classified correctly for free |
| `session-init` / `session-end-postflight` | Bootstraps epistemic context at start; closes the measurement loop at end |
| `pre-compact` / `post-compact` | Persists epistemic state across a context compaction and restores it after — compaction is routine and lossless by design |
| `tool-failure` | Filters genuine dead-ends from operational noise (timeouts, signals, outages) before they become "avoid re-trying" retrieval; redacts credentials |
| `session-monitor-arm` | Arms the mesh listener when peer messaging is configured |
| `task-completed` / `tool-router` | Bridges codex thread lifecycle to Empirica goals; routes calls to the right handler |

The hooks are **de-Claude'd**: model-facing Claude-isms are genericized so a
non-Claude model reads clean guidance. Which harness is a runtime fact carried by
`EMPIRICA_HARNESS`.

---

## The de-Claude pipeline (the maintainer's spine)

The plugin is *vendored*, so it can drift from its Empirica source. Two tools
keep it honest — this is the single most load-bearing bit of ecodex-specific
infrastructure:

- **`scripts/setup-codex.py`** — per-file diff of the vendored assets against
  `empirica@<ref>`, updates drifted files verbatim, scans for model-facing
  Claude-isms (report-only), verifies (`py_compile` + the vendored-hooks test
  suite), and optionally deploys to the runtime cache. This is how each Empirica
  release re-vendors into ecodex.
- **`scripts/check_vendored_firewall.py`** — a drift-guard asserting the vendored
  firewall hooks retain their critical safety invariants (a behavioral check, not
  a content diff, since the vendored copy is deliberately genericized).

> **The recurring hazard, named once:** a vendored asset that drifts from source
> is invisible until something reads the stale copy. A package upgrade of
> `empirica` does **not** fix a vendored hook — you must re-vendor. Both `ci.yml`
> jobs exist to catch this class.

---

## The mesh (optional)

ecodex is a first-class peer in Empirica's AI-to-AI mesh:

- **Native ntfy listener** + the `Monitor` tool arm a background watcher, so a
  peer proposal wakes the session in seconds via a `<task-notification>`, not on
  the next prompt.
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
| `codex-rs/` | The Rust workspace (upstream codex + fork touch-points), ~250 crates |
| `codex-rs/codex-empirica-plugin/` | Vendored Empirica hooks / skills / agents + the vendored-hooks test suite |
| `codex-rs/codex-empirica-translator/` | Chat-provider → Responses-API shim |
| `scripts/` | `setup-codex.py` (re-vendor + de-Claude), `check_vendored_firewall.py`, `release.sh` |
| `docs/ecodex/` | ecodex-specific docs: architecture decisions, `api/`, `integrations/`, `positioning/`, `specs/` |
| `.github/workflows/ci.yml` | Owned-crate build+test + vendored-firewall drift-guard |

Versioning tracks the upstream codex base; ecodex patches increment as `0.146.x`,
then move to the next base on each upstream re-sync (`scripts/release.sh` +
`docs/ecodex/`).

---

## Known tensions

An architecture document that lists no problems is marketing.

- **Vendored-hook drift is structural.** The plugin is a *copy*; keeping it in
  sync is manual (`setup-codex`) and the failure mode is silent. The CI
  drift-guard and the re-vendor discipline are the mitigation, not a cure.
- **Upstream-merge tax.** Every fork touch-point on a heavily-constructed
  upstream struct (provider fields, hook events, session tuples) is a future
  merge conflict — resolved by hand, verified by build+clippy+tests. Convergent
  features (both sides add the same thing) are the sharp edge.
- **Upstream is alpha-only above 0.137.** There is no stable codex tag to track;
  ecodex ships its own clean version on top of a pinned alpha base — a deliberate
  release-strategy choice.
- **Shellout latency.** 30–270 hook fires/session × ~100–300ms each is a 1–15%
  overhead — tolerable for now; a sidecar/IPC path is the parked escalation.

---

## Where to go next

| You want | Read |
|---|---|
| What ecodex adds, and getting started | [`README.md`](README.md) |
| Install modes, providers, troubleshooting | [`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md) |
| The full subsystem tour | [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md) |
| The original fork decisions (historical) | [`docs/ecodex/architecture.md`](docs/ecodex/architecture.md) |
| Hooks / MCP / skills API | [`docs/ecodex/api/`](docs/ecodex/api/) |
| The cross-AI mesh | [`docs/ecodex/cross-ai-mesh.md`](docs/ecodex/cross-ai-mesh.md) |
| EU-sovereign Mistral/Devstral wiring | [`docs/ecodex/MISTRAL_SOVEREIGN.md`](docs/ecodex/MISTRAL_SOVEREIGN.md) |
| The discipline engine itself | [Empirica](https://github.com/EmpiricaAI/empirica) + `/empirica-constitution` |
