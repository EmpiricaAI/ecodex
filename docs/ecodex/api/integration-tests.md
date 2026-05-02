# codex-empirica-plugin — Integration Tests

Real-script integration test results (2026-05-02, T14). Validates the v1 plugin against actual Empirica Python hook scripts (not mocks).

## Test environment

- Plugin binary: `target/release/codex-empirica-plugin` (built T10)
- Hook scripts: real Empirica hooks at `/home/yogapad/.claude/plugins/local/empirica/hooks/`
- Isolation: `cd /tmp/ecodex-real-script-test/` (fresh git repo + minimal `.empirica/config.yaml`) + `EMPIRICA_INSTANCE_ID=ecodex-real-script-test` + `EMPIRICA_HOOKS_DIR=<real-hooks-dir>`

## Results

| Test | Hook | Input | Outcome | Pass |
|---|---|---|---|---|
| 1 | PreToolUse | Bash `echo hello` (safe) | `{"permissionDecision": "allow", "permissionDecisionReason": "Safe Bash (read-only)"}`, exit 0 | ✅ |
| 2 | PreToolUse | Bash `rm -rf /tmp/foo` (praxic) | allow with reason "PREFLIGHT confidence sufficient (U<=31%)" | ✅ (see isolation note) |
| 3 | PreToolUse | Write to /tmp/foo.txt (praxic) | allow with reason "PREFLIGHT confidence sufficient" | ✅ (see isolation note) |
| 4 | Stop | `{"session_id":"...","stop_hook_active":false}` | empty `{}` JSON, exit 0 | ✅ |
| 5 | PreToolUse with no `.empirica/` root | (broken environment) | `permissionDecision: allow`, `Sentinel error (fail-open): ValueError: Cannot determine .empirica root` | ✅ (fail-open works correctly) |

## Findings

### Field-level protocol compatibility CONFIRMED

Codex's hook JSON payload (per `codex-rs/hooks/schema/generated/`) flows through to Empirica's sentinel-gate.py with **no field translation needed**. The plugin forwards stdin verbatim and Empirica's sentinel reads its expected fields (`session_id`, `cwd`, `tool_name`, `tool_input`, `permission_mode`, `transcript_path`, `tool_use_id`) directly. Working assumption #1 (hook protocol stability) gains real evidence — confidence raised from 0.7 to 0.9.

### Real isolation requires EMPIRICA_HOME override (not just instance/cwd)

Sentinel-gate keys its transaction state lookup on `(instance_id + cwd → .empirica/ root)`, **not** on the JSON `session_id`. So a synthetic session_id in the hook payload still hits whatever transaction state lives at the resolved `.empirica/` root for the (instance, cwd) pair.

In our tests, sentinel found our active session's PREFLIGHT (uncertainty 0.25) at the resolved root and decided "PREFLIGHT confidence sufficient — proceeding." That's correct behavior for sentinel; just incomplete isolation for our test.

For truly isolated gating tests, override the empirica state directory entirely (e.g. `EMPIRICA_PROJECT_ROOT=/tmp/test-empirica/` or rely on a separate `EMPIRICA_HOME` if defined). Documented as a TODO for the live integration smoke test transaction.

### Fail-open path validates in production

When sentinel-gate.py crashes (no `.empirica/` root, etc.), it returns `permissionDecision: allow` with the error message in the reason field. The plugin propagates this to codex correctly: codex sees an "allow" decision and proceeds with the tool call. **The plugin's own fail-open code path (T7 fix) and sentinel's fail-open path are both correct and don't double-fail.**

### Stop hook is a clean pass-through

Transaction-enforcer.py accepts the codex Stop payload and returns `{}` for the synthetic session (no active transaction → no enforcement). Exit 0. No translation issues.

## What was NOT tested at T14 (deferred to live integration — most addressed in T19 below)

- **Strict-gating mode** — would need `EMPIRICA_PROJECT_ROOT` override or fresh empirica install to verify rm/Write actually get blocked when no PREFLIGHT exists for the test session
- **Latency measurement** — didn't time the subprocess round-trip
- **All 5 active hooks end-to-end** — only PreToolUse + Stop tested; PostToolUse + SessionStart + UserPromptSubmit follow same pattern but unverified against real scripts
- **Codex actually loading the plugin** — requires building the codex binary and configuring plugin discovery, deferred to live integration

## T19: Full live integration smoke test (2026-05-02)

### Build

```sh
cd codex-rs && cargo build --release -p codex-cli
```

Result: **24m 35s wall-clock** (full release build of codex-cli's transitive workspace, ~110 crates). Produced `target/release/ecodex` (207MB binary). One non-blocking warning about dead code in `codex-app-server` (upstream issue, not ours).

### Install

```sh
./ecodex/scripts/install.sh --user
```

Result: ✅ all artifacts landed in expected paths:

| Artifact | Path | Size |
|---|---|---|
| Real binary | `~/.local/lib/ecodex/bin/ecodex` | 207MB |
| Wrapper | `~/.local/bin/ecodex` | 1628B |
| Managed lock | `~/.ecodex/managed.toml` | 1186B |
| Default config | `~/.codex/config.toml` (was absent → freshly installed) | 4102B |

The install script correctly:
- Created the parent dirs without error
- Detected absent `~/.codex/config.toml` and installed the default
- Patched the wrapper's binary path placeholder to the resolved install path
- Printed a clear summary with verification command

### Rebrand verification

```
$ ~/.local/bin/ecodex --version
codex-cli 0.0.0
$ ~/.local/bin/ecodex --help | head -6
Codex CLI

If no subcommand is specified, options will be forwarded to the interactive CLI.

Usage: ecodex [OPTIONS] [PROMPT]
       ecodex [OPTIONS] <COMMAND> [ARGS]
```

✅ **`bin_name = "ecodex"` works at runtime** — usage line shows `ecodex` in both forms.

⚠️ **Two cosmetic followups surfaced:**

1. **`--version` shows `codex-cli 0.0.0`** — that's the Cargo `[package]` name (`codex-cli`), not the `[[bin]]` name. Changing the displayed name would require either renaming the package itself (large blast radius — many internal references) or overriding clap's auto-generated version string. Not blocking but worth a small future transaction.
2. **Help title is still `Codex CLI`** — that's the clap `about = "..."` attribute (or auto-derived from doc-comment). Would need to set `about = "ecodex CLI"` (or similar) in the same `#[command(...)]` block we already touched in T10. Trivial follow-up.

Neither blocks v1 ship — `ecodex --help` is unambiguously branded in the usage section, and `--version` correctness is a separate dimension. Logging as known nits.

### Subcommand surface intact

Help output lists all expected codex subcommands: `exec`, `review`, `login`, `logout`, `mcp`, `plugin`, `mcp-server`, `app-server`, `completion`, `update`, `sandbox`, `debug`, `apply`, `resume`, `fork`, `cloud`, `exec-server`, `features`, `help`. Rebrand didn't break dispatch.

### What T19 did NOT cover (T20+ candidates)

- **Actual session run** — `ecodex exec "say hi"` against a real model provider (would need DEEPSEEK_API_KEY or similar). Skipped because it needs a live model API key and would consume tokens. Manual test by the human collaborator.
- **Plugin actually loading at runtime** — requires the empirica plugin to be installed at `~/.codex/plugins/cache/empirica/<version>/`. Plugin install is its own step (the `install.sh` script doesn't yet drop the plugin into the cache dir). T20 candidate: extend install.sh to also install the plugin.
- **Hooks firing in a real session** — depends on plugin-load (above)
- **managed.toml lock enforcement** — would need to attempt setting `plugins.empirica.enabled = false` via config and observe rejection. Requires plugin loaded first.
- **Strict env vars actually altering sentinel behavior** — assumption a_strict_env_overrides (T18) still untested at runtime. Will surface naturally during real session use.

### Cleanup (T19 end)

```sh
./ecodex/scripts/uninstall.sh   # leaves ~/.codex/config.toml in place
```

Run after committing T19 docs.

### v1 ship readiness assessment

| Component | Status |
|---|---|
| Plugin v1 (5 hooks + 10 skills + MCP server) | ✅ feature-complete |
| Plugin compiles + smoke tests pass against mocks (T7) | ✅ |
| Plugin works against real Empirica scripts (T14) | ✅ field-level compatibility confirmed |
| ecodex binary builds | ✅ T19 (24m cold, 207MB) |
| ecodex binary launches + branding applied | ✅ T19 |
| Install script lands all artifacts correctly | ✅ T19 |
| Plugin actually installed into `~/.codex/plugins/` | ❌ T20 — install.sh missing this step |
| Hooks fire in a real codex session | ❌ T21 — depends on T20 |
| managed.toml lock rejects user-config writes | ❌ T22 — depends on T20+T21 |

**v1 is ~85% to ship.** The remaining 15% (T20-T22) is end-to-end runtime verification of what the v1 components were designed to do. Substantively risk-free given everything else has validated, but worth running before declaring v1 done.

## T20–T21: Plugin install + load (2026-05-02)

### What worked

- **install.sh extended (T20)**: drops plugin into `~/.codex/plugins/cache/<marketplace>/<plugin>/<version>/` + plugin binary on PATH at `~/.local/bin/codex-empirica-plugin`.
- **Plugin loads at runtime (T21)** after fixing three discovery-format issues iteratively (each surfaced by codex's WARN logs at `RUST_LOG=info`).

### Discovery-format issues fixed during T21

| # | What we had | What codex required | Fix |
|---|---|---|---|
| 1 | Plugin name `"empirica"` | `<plugin>@<marketplace>` format | Renamed to `"empirica@nubaeon"` everywhere (manifest.json, install.sh PLUGIN_KEY, managed.toml, config.toml.default) |
| 2 | Cache path `cache/empirica/<version>/` | `cache/<marketplace>/<plugin>/<version>/` | Updated install.sh to split PLUGIN_KEY into marketplace + name and build path accordingly |
| 3 | Manifest at `<root>/manifest.json` | `<root>/.codex-plugin/plugin.json` (or `.claude-plugin/plugin.json`) | Updated install.sh to mkdir `.codex-plugin/` and copy `manifest.json` → `.codex-plugin/plugin.json` |

After all three fixes: `RUST_LOG=warn ecodex exec ...` no longer emits any `codex_core_plugins::loader: failed to load plugin` — **plugin loads cleanly.** Source `manifest.json` filename in the plugin crate stays as-is (canonical artifact); the `.codex-plugin/plugin.json` rename happens at install time.

### Discovery format reference (for future work)

- **Plugin manifest discovered at:** `<plugin_root>/.codex-plugin/plugin.json` (preferred) OR `<plugin_root>/.claude-plugin/plugin.json` (fallback) — per `codex-rs/utils/plugins/src/plugin_namespace.rs::DISCOVERABLE_PLUGIN_MANIFEST_PATHS`.
- **Plugin install root:** `<codex_home>/plugins/cache/<marketplace>/<plugin>/<version>/`
- **Plugin key format:** `<plugin>@<marketplace>` — both segments alphanumeric/dash/underscore (per `codex-rs/plugin/src/plugin_id.rs::validate_plugin_segment`).

### What couldn't be tested without model auth

- **Hook firing during a real agent session** — codex tries to connect to the model, gets 401 from OpenAI without `OPENAI_API_KEY`, never executes a tool, so PreToolUse never fires. To verify: run with a valid model API key for any provider that codex's current build supports (see "Strategic finding" below).

### Strategic finding: codex removed `wire_api = "chat"` upstream

While testing T21, encountered: `wire_api = "chat" is no longer supported. How to fix: set wire_api = "responses" in your provider config.` (per upstream discussion linked in error). Investigated `codex-rs/model-provider-info/src/lib.rs::WireApi` and confirmed the enum now has only the `Responses` variant — chat-completions wire support is **gone from upstream codex**.

**Implication for ecodex:** All our curated open-weights providers (DeepSeek, Qwen, GLM, Kimi, Ollama, LMStudio) speak OpenAI-compatible chat completions, NOT the Responses API. **They cannot be configured directly with this codex version**, even with `wire_api = "responses"` set (the file parses, but the endpoint won't actually work because the providers don't speak Responses).

Three options surface (need David's call in next session):
1. **Pin our fork to a pre-removal codex base** — git revert/older base, lose newer features
2. **Build a chat-completions ↔ Responses API adapter** — separate crate, bridges open-weights providers to codex's Responses-only client
3. **Contribute back chat-completions wire support to upstream codex** — aligns with our fork-and-PR-back posture; may or may not be accepted

Codex's existing `responses-api-proxy` crate is OpenAI-only (privilege isolation tool, not a translator). Doesn't solve our problem.

This is a **major strategic finding** — affects ecodex's whole open-weights value prop. v1 ship blocked until decided.

### Other minor observations (logged for future)

- `ecodex --version` shows `codex-cli 0.0.0` — Cargo `[package]` name unchanged (T19 finding).
- `ecodex --help` title still says "Codex CLI" — clap `about` field, separate edit (T19 finding).
- Default model resolution defaulted to `gpt-5.5` despite `model = "deepseek-chat"` in config.toml. Possibly a profile-config interaction or a cached default. Worth investigating.
- Codex pre-loads other plugins from `~/.codex/.tmp/plugins/plugins/` (build-ios-apps, plugin-eval) — those are codex's bundled plugins. They emit unrelated warnings about `interface.defaultPrompt` length limits.

### v1 ship readiness — updated assessment

| Component | Status |
|---|---|
| Plugin v1 (5 hooks + 10 skills + MCP) | ✅ feature-complete |
| Plugin compiles + smoke tests vs mocks | ✅ |
| Plugin works against real Empirica scripts | ✅ T14 |
| ecodex binary builds + installs + rebrand applied | ✅ T19 |
| Plugin installs to correct cache layout | ✅ T20 (after T21 format fixes) |
| Plugin loads at codex startup | ✅ T21 (after 3 discovery-format fixes) |
| Hooks fire in real session | ⚠️ blocked on model auth + chat-API removal |
| managed.toml lock rejects user-config writes | ⚠️ T22 — testable now that plugin loads |
| Curated providers actually usable | ❌ **chat-API removal blocks them** — T-strategic |

**v1 ship is ~90% to runtime-verified plugin install/load**, but **the chat-API removal is a strategic blocker** for the open-weights provider story. Need David's call on the three options above before declaring v1 ship-ready.

## Action items surfaced

1. **Strict-gating test** — needs proper EMPIRICA_HOME isolation; add to v1 ship checklist.
2. **Latency baseline** — measure subprocess round-trip time during live integration; validate the 100-300ms estimate from T2.
3. **PostToolUse / SessionStart / UserPromptSubmit real-script pass** — repeat T14 pattern for these three; should pass since pattern is identical, but worth confirming.
