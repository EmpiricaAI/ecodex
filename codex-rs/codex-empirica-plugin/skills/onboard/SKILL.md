---
name: onboard
description: >
  ecodex first-run onboarding + diagnostics orchestrator. Composes
  `empirica diagnose --frontend ecodex` (the deterministic integration
  checks) with `empirica onboard --ai-id` and ecodex's self-provisioning
  installer/bootstrap,
  then fills the gaps diagnose doesn't cover: local model-server probe,
  model-metadata-fallback check (the 32K-context-loss bug class fixed in
  0efb8c7), model smoke-test, plain-vs-ecosystem mode selection,
  per-project practice setup, and sandbox-network check. Defaults the
  provider to OpenRouter (zero-config — OpenRouter speaks the Responses
  API natively). Triggers when the user says "onboard ecodex", "set up
  ecodex", "first-run ecodex", "is my ecodex install ready", or runs
  ecodex for the first time and hits a provider/model gap. Distinct from
  the `diagnose` skill — that checks integration health; this is the
  full bring-up-to-working flow.
---

<!-- ECODEX VENDOR ADAPTATION: Empirica's generic setup command deliberately
refuses ecodex because ecodex self-provisions its plugin and hooks. Keep the
onboarding path below on ecodex's installer/bootstrap plus the shared
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
  Plugin + strict empirica defaults (the A+E layers from
  `config.toml.default`). No cortex MCP, no listener, no
  `credentials.yaml` cortex key. The casual single-user mode.
- **Ecosystem ecodex** — plain + the mesh (cortex MCP server + listener +
  `credentials.yaml` cortex key). Multi-practice, cross-project,
  AI-to-AI orchestration. Opt in with `--with-cortex` (or answer the
  mode prompt).

## Prerequisite

Run `ecodex/scripts/install.sh` FIRST. It builds + installs the binary,
the empirica plugin, the default config, and the feature-flag gates
(`plugin_hooks = true`). This skill handles the **post-install** gap-fill
+ provider/mode setup that install.sh doesn't cover.

## How to run

### Step 0 — pick the mode

Ask the user (or detect `--with-cortex`): plain or ecosystem? The mode
decides which setup steps apply. Plain skips the cortex wiring
(Step 5). Surface the trade-off plainly: plain = single-user, no mesh;
ecosystem = multi-practice + AI-to-AI, needs a cortex account/key.

### Step 1 — run the deterministic diagnostics

```bash
empirica diagnose --frontend ecodex --output json --fast
```

`--fast` skips `cargo check` (slow). This runs ~12 checks: Python +
empirica CLI, plugin install + config enablement, statusline runtime
wiring + runnability, codex-empirica-translator `/healthz`, curated
model provider `env_keys`, Rust compliance. Parse the JSON, walk each
`FAIL` then `WARN` with the user — propose fixes, apply safe reversible
ones (file writes to `~/.codex`, `chmod`, install-script re-invocation),
escalate code edits per transaction discipline (code edits are praxic;
declare PREFLIGHT scope first). **Defer to the `diagnose` skill** for the
integration-layer triage — don't re-reason what the script already
determined.

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

Is a local OpenAI-compatible server running? Ollama speaks the Responses
API natively → zero-config local path. llama.cpp / vLLM speak
chat-completions only → need the translator.

```bash
for ep in http://localhost:11434/v1/models http://localhost:8080/v1/models http://localhost:8000/v1/models; do
  echo "== $ep =="
  curl -s -m 2 "$ep" | head -c 200 || echo "(no response)"
  echo
done
```

Ollama up (`:11434`) → zero-config local option. llama.cpp (`:8080`) /
vLLM (`:8000`) up → flag the translator requirement
(`codex-empirica-translator` on `:18080`).

#### 3b — model-metadata-fallback check (the 32K bug class)

Does the configured model resolve WITHOUT the family-prefix-fallback
warning? An unseeded slug (e.g. a new GLM/Qwen tag not in
`models.curated.json` or `~/.codex/models.user.json`) falls through to
the `recognize_open_weights_family` prefix table → wrong context window
(e.g. **32K for `glm-*`** vs the real **1M** for glm-5.2) → ecodex
auto-compacts ~30× too aggressively → state loss. This is the bug fixed
in `0efb8c7` for `z-ai/glm-5.2`; `diagnose` does NOT catch it for other
unseeded slugs, so check explicitly.

```bash
MODEL=$(grep -E '^[[:space:]]*model[[:space:]]*=' ~/.codex/config.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
echo "configured model: $MODEL"
# A fallback warning at ecodex startup:
grep -riE "Unknown model .* fallback model metadata|Model metadata for .* not found" ~/.codex/log/ 2>/dev/null | tail -3
```

If the configured model warns → **seed it**: add a lean entry to
`~/.codex/models.user.json` (Layer 1 overlay, user-wins-on-collision)
with the correct `context_window`, modeled on the `z-ai/glm-5.1` entry
in `models.curated.json`. **Re-verify the model's real context window
against the provider's docs** — don't guess. Cross-ref the `diagnose`
skill + commit `0efb8c7`.

#### 3c — model smoke-test (does the model actually respond?)

A 1-token probe via the configured provider catches 404 / auth / quoting
failures (the ecodex-lab bring-up 404 class) that metadata checks miss.

For OpenRouter (Responses-native, zero-config):
```bash
curl -s -m 15 -X POST https://openrouter.ai/api/v1/responses \
  -H "Authorization: Bearer $OPENROUTER_API_KEY" \
  -H "Content-Type: application/json" \
  -d "{\"model\":\"$MODEL\",\"input\":\"ping\",\"max_output_tokens\":5}" | head -c 300
```

A valid response = the provider path works end-to-end. `401` = bad key;
`404` = wrong slug or endpoint; timeout = network/sandbox block (→ 3f).

For a local Ollama model, probe `http://localhost:11434/v1/responses`
with the same shape. For direct chat-completions-only providers
(DeepSeek/GLM/Qwen/Kimi/Mistral), the translator must be up first
(`:18080`).

#### 3d — plain-vs-ecosystem mode (apply Step 0's decision)

Plain → skip Step 5. Ecosystem → continue to Step 5. Nothing to probe
here; this is the routing decision.

#### 3e — per-project practice setup

For each project the user wants ecodex in:
```bash
cd <project-dir>
empirica onboard --ai-id          # writes .empirica/project.yaml ai_id
# custom per-project instructions → the project's CLAUDE.md / AGENTS.md
```

Multiple projects = multiple practices, each its own `ai_id` + calibration
trajectory. (Ecosystem mode: these practices can address each other over
the mesh via canonical 3-form `<org>.<tenant>.<project>`.)

#### 3f — sandbox-network check (the landlock-blocks-LAN trap)

On Linux, the workspace-write sandbox (landlock) blocks ALL network
including localhost AND LAN — this severs the empirica embedding path
(Ollama/Qdrant on `empirica-server`) and any local model server. David
hit this exact trap. Check + fix:

```bash
grep -A3 'sandbox_workspace_write' ~/.codex/config.toml
# trusted projects using LAN/localhost models need:
#   [sandbox_workspace_write]
#   network_access = true
```

Cross-ref `docs/sandbox.md`. Only on **trusted** projects — the flag is
coarse (all-or-nothing, no per-host allowlist). Keep OFF anywhere
untrusted code runs.

### Step 4 — default the provider to OpenRouter (zero-config)

OpenRouter speaks the Responses API natively (verified — beta, drop-in
OpenAI-compatible at `https://openrouter.ai/api/v1/responses`, supports
reasoning/tool-calling/web-search). So the default provider is
**zero-config**: just needs `OPENROUTER_API_KEY`.

```bash
[ -n "${OPENROUTER_API_KEY:-}" ] && echo "OPENROUTER_API_KEY set ✓" \
  || echo "✗ OPENROUTER_API_KEY missing — get one at https://openrouter.ai/keys"
```

- Key present → ecodex works immediately. Set `model` to a seeded
  OpenRouter slug (e.g. `z-ai/glm-5.2` @ 1M, or `openrouter/auto`).
- Key absent → route the user to `https://openrouter.ai/keys`, OR offer
  local Ollama (zero-config if running per 3a), OR a direct provider
  (DeepSeek/GLM/Qwen/Kimi/Mistral — chat-completions only, needs the
  translator; users pick these for subscription economics that beat bare
  API token costs).

**Translator necessity, reasoned** (so you can explain it): the
translator (`codex-empirica-translator`, `:18080`) is needed ONLY for
chat-completions-only providers — direct open-weight-cloud
(DeepSeek/Zhipu/Qwen/Kimi/Mistral) and local non-Ollama servers
(llama.cpp/vLLM). It is NOT needed for OpenRouter, OpenAI direct, or
Ollama — those speak Responses natively. `wire_api = "responses"` is
ecodex's only wire (upstream codex removed `wire_api = "chat"`); the
translator bridges chat↔Responses for the providers that can't speak it.

### Step 5 — ecosystem wiring (ecosystem mode only)

ecodex already self-provisions its plugin, hooks, MCP surface, and native
listener through its own installer/bootstrap. Do not run Empirica's generic
harness setup command: the current CLI deliberately refuses ecodex rather than
writing another harness's files.

Plain mode needs no mesh configuration. For ecosystem mode, provision the
shared, harness-neutral `~/.empirica/credentials.yaml` through the existing
ecodex bootstrap or with Cortex-issued credentials supplied by the user. Never
invent or print credential values. Then verify the resulting Cortex and ntfy
configuration with `empirica doctor` and re-run
`empirica diagnose --frontend ecodex`.

### Step 6 — verify end-to-end

Re-run `empirica diagnose --frontend ecodex` (expect green) + the
smoke-test (Step 3c — model responds). The user now has: gaps
diagnosed, provider verified (model actually answers), mode wired,
practices set up per project. ecodex is ready to run.

## What this skill is NOT for

- **Re-implementing `empirica diagnose` checks in reasoning** — always
  call the CLI; the script is the truth source.
- **Replacing `install.sh`** — install.sh builds + drops the binary +
  plugin + config + feature-flags. Run it FIRST; this skill is the
  post-install gap-fill + provider/mode setup.
- **The TUI onboarding** (`onboarding_screen.rs`: Welcome → Auth →
  Trust) — that's codex-upstream's API-key-auth + project-trust flow.
  This skill is the ecodex provider/model/cortex setup that complements
  it.
- **Editing empirica's engine** — if a gap is an empirica-engine issue
  (e.g. the `get_cortex_config` env-first precedence, the artifact-decay
  substrate), surface it to the owning practice via mesh collab / a PR —
  don't edit empirica's repo directly (MEMBRANE RULE: cross-boundary
  code changes materialize as PRs).

## Related

- **`diagnose` skill** — the integration-health reasoning glue this composes (Step 1).
- **`empirica diagnose --frontend ecodex`** — the deterministic checker (~12 checks).
- **`empirica onboard --ai-id`** — practice identity derivation (Step 2/3e).
- **ecodex installer/bootstrap** — self-provisions plugin/hooks and consumes shared mesh credentials (Step 5).
- **`empirica doctor`** — frontend-agnostic health check (cortex reachability).
- **`ecodex/scripts/install.sh`** — the binary+plugin installer (prerequisite).
- **commit `0efb8c7`** — the model-metadata-fallback / glm-5.2 1M fix (Step 3b cross-ref).
- **`docs/sandbox.md`** — the sandbox-network trap (Step 3f).
