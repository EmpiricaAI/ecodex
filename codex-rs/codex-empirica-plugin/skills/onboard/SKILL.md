---
name: onboard
description: >
  ecodex first-run onboarding + diagnostics orchestrator. Composes
  `empirica diagnose --frontend ecodex` (the deterministic integration
  checks) with `empirica onboard --ai-id` and ecodex's self-provisioning,
  then fills the gaps diagnose doesn't cover: local model-server probe,
  model-metadata-fallback check (the 32K-context-loss bug class fixed in
  0efb8c7), model smoke-test, plain-vs-ecosystem mode selection,
  per-project practice setup, and sandbox-network check. Recommends
  OpenRouter as the zero-config provider (it serves the Responses API
  itself). Triggers when the user says "onboard ecodex", "set up ecodex",
  "first-run ecodex", "is my ecodex install ready", or runs ecodex for the
  first time and hits a provider/model gap. Distinct from the `diagnose`
  skill — that checks integration health; this is the full
  bring-up-to-working flow.
---

<!-- ECODEX VENDOR ADAPTATION: Empirica's generic setup command deliberately
refuses ecodex because ecodex self-provisions its plugin and hooks. Keep the
onboarding path below on ecodex's own provisioning plus the shared
credentials file; re-apply this adaptation if the snapshot is refreshed. -->

# Onboard ecodex

## Purpose

Walk a fresh ecodex install from "binary present" to "agent responds" in
one guided pass. This skill is **reasoning glue** over the existing CLI
tools — it does NOT re-implement checks. Run the deterministic checkers,
triage failures, fill the gaps they don't cover, pick a working provider,
and smoke-test that the model actually answers.

Two user modes, kept explicitly distinct (David's requirement):

- **Plain ecodex** — the AI calibration training environment, no mesh.
  The empirica plugin plus strict mode, which the `ecodex` binary turns on
  by itself. No cortex MCP, no listener, no `credentials.yaml` cortex key.
  The casual single-user mode.
- **Ecosystem ecodex** — plain + the mesh (cortex MCP server + listener +
  `credentials.yaml` cortex key). Multi-practice, cross-project,
  AI-to-AI orchestration. Opt in with `--with-cortex` (or answer the
  mode prompt).

## What is already done before this skill runs

Whatever channel the user installed with (install script, Homebrew,
tarball, cargo, source build), the first ecodex session wrote the empirica
plugin to `~/.codex/plugins/cache/empiricaAI/empirica/`, created
`~/.codex/config.toml` from the bundled default if there was none, and
wrote `~/.codex/translator-upstreams.toml` if it was missing. The install
script, Homebrew and the source build also set up the `empirica` CLI.
There are no feature flags to set. This skill covers what comes after:
gaps, provider, mode.

If `empirica` is not on `PATH` (a cargo or tarball install), have the user
run `pipx install empirica` first; the hooks run under its interpreter.

## How to run

### Step 0 — pick the mode

Ask the user (or detect `--with-cortex`): plain or ecosystem? The mode
decides which setup steps apply. Plain skips the cortex wiring
(Step 5). Surface the trade-off plainly: plain = single-user, no mesh;
ecosystem = multi-practice + AI-to-AI, needs a cortex account/key.

Also ask whether this install will run jobs nobody watches (CI, cron,
scripted `ecodex exec`). No one opens a transaction in those, so inside a
git repository the Sentinel refuses every command that writes. Send the
user to "Unattended runs" in `docs/ecodex/INSTALL.md`, which gives the
three ways to run them; this skill covers interactive use.

### Step 1 — run the deterministic diagnostics

```bash
empirica diagnose --frontend ecodex --output json --fast
```

`--fast` skips `cargo check` (slow). The checks cover the Python and
empirica CLI, the plugin install and its config entry, plugin hooks
reachable, statusline wiring, the translator's `/healthz`, curated
provider keys, and Rust compliance. Parse the JSON, walk each `FAIL` then
`WARN` with the user — propose fixes, apply safe reversible ones (file
writes to `~/.codex`, `chmod`), escalate code edits per transaction
discipline (code edits are praxic; declare PREFLIGHT scope first).
**Defer to the `diagnose` skill** for the integration-layer triage —
don't re-reason what the script already determined.

### Step 2 — derive the practice ai_id

```bash
empirica onboard --ai-id
```

Reads/derives the `ai_id` from the project basename or
`.empirica/project.yaml`. This is the practice identity ecodex inhabits
(the calibration trajectory is per-practice).

### Step 3 — fill the gaps diagnose doesn't cover

These six checks are NOT in `empirica diagnose`. Run them as shell probes.

#### 3a — local model-server probe

Is a local OpenAI-compatible server running?

```bash
for ep in http://localhost:11434/v1/models http://localhost:1234/v1/models http://localhost:8080/v1/models http://localhost:8000/v1/models; do
  echo "== $ep =="
  curl -s -m 2 "$ep" | head -c 200 || echo "(no response)"
  echo
done
```

Ollama (`:11434`) and LM Studio (`:1234`) are built-in providers and serve
the Responses API: a zero-config local option. llama.cpp (`:8080`) and
vLLM (`:8000`) have entries in the default config pointing straight at
them. If one of those servers answers only Chat Completions, route it
through the translator instead: point its provider's `base_url` at
`http://localhost:18080/v1` and add a route for its models in
`~/.codex/translator-upstreams.toml`.

#### 3b — model-metadata-fallback check (the 32K bug class)

Does the model the user picked resolve WITHOUT the family-prefix-fallback
warning? An unseeded slug (e.g. a new GLM/Qwen tag not in
`models.curated.json` or `~/.codex/models.user.json`) falls through to
the `recognize_open_weights_family` prefix table → wrong context window
(e.g. **32K for `glm-*`** vs the real **1M** for glm-5.2) → ecodex
auto-compacts ~30× too aggressively → state loss. This is the bug fixed
in `0efb8c7` for `z-ai/glm-5.2`; `diagnose` does NOT catch it for other
unseeded slugs, so check explicitly.

The default config sets no model; ask which model they picked in `/model`
(or read `model =` if they set one at the top of `~/.codex/config.toml`),
then look for the warning:

```bash
grep -riE "Unknown model .* fallback model metadata|Model metadata for .* not found" ~/.codex/log/ 2>/dev/null | tail -3
```

If the model warns → **seed it**: add a lean entry to
`~/.codex/models.user.json` (Layer 1 overlay, user-wins-on-collision)
with the correct `context_window`, modeled on the `z-ai/glm-5.1` entry
in `models.curated.json`. **Re-verify the model's real context window
against the provider's docs** — don't guess. Cross-ref the `diagnose`
skill + commit `0efb8c7`.

#### 3c — model smoke-test (does the model actually respond?)

A 1-token probe via the chosen provider catches 404 / auth / quoting
failures (the ecodex-lab bring-up 404 class) that metadata checks miss.
`$MODEL` is the model from 3b.

For OpenRouter (serves Responses, zero-config):
```bash
curl -s -m 15 -X POST https://openrouter.ai/api/v1/responses \
  -H "Authorization: Bearer $OPENROUTER_API_KEY" \
  -H "Content-Type: application/json" \
  -d "{\"model\":\"$MODEL\",\"input\":\"ping\",\"max_output_tokens\":5}" | head -c 300
```

A valid response = the provider path works end-to-end. `401` = bad key;
`404` = wrong slug or endpoint; timeout = network/sandbox block (→ 3f).

For a local Ollama model, probe `http://localhost:11434/v1/responses`
with the same shape. For a translated provider (Mistral, DeepSeek, Qwen,
GLM, Kimi), probe the translator instead, `http://localhost:18080/v1/responses`,
with no Authorization header: the translator holds the key.

#### 3d — plain-vs-ecosystem mode (apply Step 0's decision)

Plain → skip Step 5. Ecosystem → continue to Step 5. Nothing to probe
here; this is the routing decision.

#### 3e — per-project practice setup

For each project the user wants ecodex in:
```bash
cd <project-dir>
empirica onboard --ai-id          # writes .empirica/project.yaml ai_id
# custom per-project instructions → the project's AGENTS.md
```

Multiple projects = multiple practices, each its own `ai_id` + calibration
trajectory. (Ecosystem mode: these practices can address each other over
the mesh via canonical 3-form `<org>.<tenant>.<project>`.)

#### 3f — sandbox-network check (the landlock-blocks-LAN trap)

On Linux, the workspace-write sandbox (landlock) blocks ALL network
including localhost AND LAN — this severs the empirica embedding path
(Ollama/Qdrant on a LAN server) and any local model server. David hit
this exact trap. Check + fix:

```bash
grep -A3 'sandbox_workspace_write' ~/.codex/config.toml
# trusted projects using LAN/localhost models need:
#   [sandbox_workspace_write]
#   network_access = true
```

Cross-ref `docs/sandbox.md`. Only on **trusted** projects — the flag is
coarse (all-or-nothing, no per-host allowlist). Keep OFF anywhere
untrusted code runs.

### Step 4 — recommend a provider (OpenRouter is zero-config)

OpenRouter serves the Responses API itself (OpenAI-compatible, at
`https://openrouter.ai/api/v1/responses`, with tool calling), so it needs
nothing but `OPENROUTER_API_KEY`:

```bash
[ -n "${OPENROUTER_API_KEY:-}" ] && echo "OPENROUTER_API_KEY set ✓" \
  || echo "✗ OPENROUTER_API_KEY missing — get one at https://openrouter.ai/keys"
```

- Key present → ecodex works immediately. Pick a seeded OpenRouter slug in
  `/model` (e.g. `z-ai/glm-5.2` @ 1M, or `openrouter/auto`).
- Key absent → route the user to `https://openrouter.ai/keys`, OR offer
  local Ollama (zero-config if running per 3a), OR OpenAI (ChatGPT
  sign-in), OR a translated provider (below; users pick these for
  sovereignty or for subscription economics that beat bare API token
  costs).

**The translator, explained** (so you can walk the user through it):
codex speaks only the Responses API (upstream removed `wire_api = "chat"`).
Mistral, DeepSeek, Qwen, GLM and Kimi speak only Chat Completions, so they
go through `codex-empirica-translator`, which ships with ecodex and listens
on `127.0.0.1:18080`. The default config already points those providers
at it, and `~/.codex/translator-upstreams.toml` already has their routes.
Three steps:

1. Store the key in `~/.empirica/credentials.yaml` under the provider's
   section (`mistral`, `deepseek`, `dashscope`, `zhipu`, `moonshot`) as
   `api_key: …`, or export `MISTRAL_API_KEY` / `DEEPSEEK_API_KEY` /
   `DASHSCOPE_API_KEY` / `ZHIPU_API_KEY` / `MOONSHOT_API_KEY` in the shell
   that starts the translator. Never print a key and never write one into a
   config file.
2. Start `codex-empirica-translator` (no flags) and leave it running. It
   warns about each provider it has no key for and serves the rest.
3. Pick the model in `/model`.

Not needed for OpenRouter, OpenAI, Hugging Face, Ollama or LM Studio.

### Step 5 — ecosystem wiring (ecosystem mode only)

ecodex already self-provisions its plugin, hooks, MCP surface, and native
listener. Do not run Empirica's generic harness setup command: the current
CLI deliberately refuses ecodex rather than writing another harness's
files.

Plain mode needs no mesh configuration. For ecosystem mode, provision the
shared, harness-neutral `~/.empirica/credentials.yaml` with
Cortex-issued credentials supplied by the user. Never invent or print
credential values. Then verify the resulting Cortex and ntfy configuration
with `empirica doctor` and re-run `empirica diagnose --frontend ecodex`.

### Step 6 — verify end-to-end

Re-run `empirica diagnose --frontend ecodex` (expect green) + the
smoke-test (Step 3c — model responds). The user now has: gaps
diagnosed, provider verified (model actually answers), mode wired,
practices set up per project. ecodex is ready to run.

## What this skill is NOT for

- **Re-implementing `empirica diagnose` checks in reasoning** — always
  call the CLI; the script is the truth source.
- **Installing ecodex** — the installers and the first session do that;
  `docs/ecodex/INSTALL.md` covers the channels. This skill is the
  post-install gap-fill + provider/mode setup.
- **Unattended runners** — `docs/ecodex/INSTALL.md` ("Unattended runs")
  covers them; see Step 0.
- **The TUI onboarding** (`onboarding_screen.rs`: Welcome → Auth →
  Trust) — that's codex-upstream's auth + project-trust flow. This skill
  is the ecodex provider/model/cortex setup that complements it.
- **Editing empirica's engine** — if a gap is an empirica-engine issue,
  surface it to the owning practice via mesh collab / a PR — don't edit
  empirica's repo directly (MEMBRANE RULE: cross-boundary code changes
  materialize as PRs).

## Related

- **`diagnose` skill** — the integration-health reasoning glue this composes (Step 1).
- **`empirica diagnose --frontend ecodex`** — the deterministic checker.
- **`empirica onboard --ai-id`** — practice identity derivation (Step 2/3e).
- **`empirica doctor`** — frontend-agnostic health check (cortex reachability).
- **`docs/ecodex/INSTALL.md`** — install channels, providers, troubleshooting.
- **`docs/ecodex/integrations/providers.md`** — every provider and the translator routes.
- **commit `0efb8c7`** — the model-metadata-fallback / glm-5.2 1M fix (Step 3b cross-ref).
- **`docs/sandbox.md`** — the sandbox-network trap (Step 3f).
