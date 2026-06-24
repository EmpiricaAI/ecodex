# Changelog

All notable ecodex-specific changes are documented here. ecodex is a fork of [openai/codex](https://github.com/openai/codex); upstream codex changes are tracked at the [openai/codex releases](https://github.com/openai/codex/releases) page.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and version numbers follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`scripts/release.sh` rolls the [Unreleased] section under a new version stamp on each release. Add entries to [Unreleased] as you ship; the release script promotes them.

## [Unreleased]

## [0.2.2] - 2026-06-24

### Fixed
- **empirica MCP server now actually starts** (`f6525b23f2`): the plugin's `mcp_servers.json` launched `empirica mcp`, which is not a valid subcommand — so even with v0.2.1's manifest `mcpServers`-key fix, the empirica MCP server never started. The real server is a separate binary, **`empirica-mcp`**. Runtime-verified via an MCP `initialize` handshake (`serverInfo: empirica 1.27.1`, tools capability). The `mcp__empirica__*` tools are now reachable in ecodex.

### Changed
- **Curated ecodex CI** (`ddb9bb1d76`, `de8fe8b95c`): replaced openai/codex's inherited CI workflows — which tested codex's *repo invariants* (TUI↔core boundary, Bazel/Cargo clippy parity, npm staging from openai's own release runs, README ASCII house-style) and didn't even run cargo, so ecodex's own code went untested on push — with a lean CI that builds + tests the crates ecodex owns (`codex-empirica-plugin`, `codex-empirica-translator`) on the pinned 1.95.0 toolchain. Removed 6 inherited openai workflows; the new CI immediately caught + fixed an env-var-race test flake.

## [0.2.1] - 2026-06-24

### Fixed
- **Sentinel gating restored** (`83c635e9b4`) — the headline fix. In v0.2.0 the PreToolUse firewall did **not** actually block tool calls: the plugin's hook-output translation layer (added in T81 to strip `suppressOutput`) read only the legacy top-level `decision` field, so the sentinel's `hookSpecificOutput.permissionDecision: "deny"` was silently dropped and praxic actions ran despite a deny — model-agnostic. `translate_pre_tool_use` now carries the codex-native `permissionDecision` (`allow`/`deny`/`ask`) through, maps legacy `block`/`approve` → `deny`/`allow` (codex's enum rejects `block`/`approve`), and pulls the reason from either shape. Covered by regression tests on the exact sentinel output shape, and runtime-confirmed (a praxic tool with no open transaction is denied end-to-end).
- **empirica MCP server now loads** (`83c635e9b4`): the plugin manifest declared `mcp_servers` (snake_case) where codex's `camelCase` manifest schema expects `mcpServers`; the key silently resolved to `None`, so the `mcp__empirica__*` tools were unavailable.
- **`tar` 0.4.45 → 0.4.46** (`a4902bca42`): upstream archive-extraction security fix; `tar` ships in the binary via the plugin archive-extraction path.

### Changed
- **De-Claude pass on the always-on skills + scanner** (`4cf4fa754d`): the pinned `epistemic-persistence-protocol` and `epistemic-transaction` skills (injected into model context every session) no longer refer to the running model as "Claude" / "Claude Desktop" / "Claude Code Tasks" — ecodex runs non-Claude models. The `setup-codex` de-Claude scanner now also covers the `skills/` directory (previously an unscanned blind spot).
- **Retired dead Claude-Code-only loop/listener machinery** (`dbe7082801`, −1002 LOC): removed the `loop`/`listener` install+uninstall-pickup hooks (built on `CronCreate`/`/loop`, which don't exist in codex) and the `loop-cron` / `inbox-listener` skills — superseded by the native ntfy listener (`ntfy_listener.rs`) + `session-monitor-arm` + the `empirica loop` CLI. A `setup-codex` skip-list keeps them retired across syncs.
- **Dependabot disabled** (`f9ff24a322`): the inherited openai/codex config scanned the whole upstream dependency surface; security posture for the shipped binary now relies on periodic `cargo audit`.

## [0.2.0] - 2026-06-23

### Added
- **Mistral as the EU data-sovereignty cloud route** (`c2457d0d6e`): adds Mistral (Devstral coding models) as a curated provider tagged `jurisdiction.eu_data_residency`, giving teams under GDPR / EU AI Act constraints a hosted non-US route alongside the existing OpenRouter / direct-API / local options. Wired into `config.toml.default`, `models.curated.json`, and the curated-models registry. See `docs/ecodex/integrations/providers.md`.
- **Harness-integrity guards on the vendored hook layer** — three of empirica's integrity checks, adapted for ecodex's vendored Python hooks:
  - **SQL schema-reference guard** (#1, `7d79941a6f`): a test that fails if the vendored hooks reference DB columns/tables absent from the schema, catching schema drift between empirica and the vendored copy before it ships.
  - **Import-budget gate** (#3, `7d620f2a5a`): bounds the import surface of the hot-path hook scripts so a heavyweight import can't silently regress per-call latency.
  - **Compliance crosswalk** (#4, `56aa7d4c20`): a published asset mapping ecodex's controls to EU AI Act / GDPR / ISO 42001 obligations (`docs/ecodex/positioning/compliance-crosswalk.{md,html}`).

### Changed
- **2026-06 upstream codex sync merged** (`a0208269ca`, `0befb393f3`): large forward-port of openai/codex onto ecodex's plugin layer. Notable upstream surface now carried includes per-turn and thread-level multi-agent mode, remote exec-environment connection lifecycle + snapshots, token-budget-driven compaction (with budget-expiry turn aborts), UUIDv7 context-window lineage IDs, indexed/cached web-search modes, protected-resource OAuth discovery, and a clock current-time tool. ecodex's integrations (plugin writable-roots, provider hot-swap, the `monitor` tool) were re-reconciled against the new session semantics.
- **PR#138 sentinel rush-guard fix re-vendored** (`d31ef316b5`): re-syncs 25 drifted hook/lib/script files from empirica `develop`, landing the rush-guard recovery-verb hoist so check / postflight / `*-log` calls aren't blocked when the noetic window is still fresh.

### Fixed
- **Model picker stuck on single-effort curated models + unsent queued input** (`f6657fa948`): the TUI treated a single supported reasoning-effort as "no choice" with the wrong comparison (`== 1` rather than `<= 1`), wedging the picker for curated models that expose one effort level, and could drop queued input.
- **Release-gate test reconciliation** (`2ee947032b`, `5ad69972ce`…`5c8e274879`): app-server integration tests fixed for the merged signatures and ecodex branding — `mcp_server_status` made robust under load via bounded test parallelism (after reverting an ineffective read-timeout bump), `executor_mcp` updated to the `ecodex` binary name, `command_exec` switched to `sh -c` (non-login shell) to avoid login-profile network-probe noise, and plugin-manifest test fixtures gained the upstream-added `statusline` / `writable_roots` / `pinned` fields.
- **Compliance report green (11/11, score 1.0)** (`2ee947032b`): added a minimal `pyproject.toml [tool.ecodex] version` mirror so the Python-centric `release_chain` check reads a true version for the Rust workspace, excluded the positioning visual-gen scripts from lint, and refactored (rather than config-dodged) the SQL schema-reference test's complexity violation.

## [0.1.0] - 2026-06-02

### Added
- **L3 model registry — curated seed + provider discovery** (`codex-models-manager` + `cli`, commits `aec472f4b8` / `db37c97461` / `b93d47da5c`): ecodex now ships a curated model registry so the picker is useful out of the box. Three-layer slug resolution (`model_info_from_slug`): exact-curated entry (bundled `models.curated.json` ∪ `~/.codex/models.user.json`) → family-prefix table → generic fallback. Lean seed schema (curated/epistemic fields only — context, tools, `reasoning.supported`, routes, `jurisdiction`, `calibration_tier`, `last_verified`); codex-runtime fields inherited from the runtime template at `enrich()` time. Seed: 11 entries (8 deep-research 3-vote-verified non-GPT coding models — Kimi K2.6, Qwen3.7/3.6 Max, DeepSeek-R1, gpt-oss 120B/20B, Qwen3-Coder-30B, GLM-5.1, MiniMax-M2.7 — plus 2 EU-sovereignty Mistral Devstral entries tagged `jurisdiction.eu_data_residency`). New `ecodex models list` (show resolved registry) and `ecodex models refresh [--provider ID] [--dry-run] [--no-filter]` (probe each `[model_providers.*]` `/v1/models`, curated-families filter + non-coding/unstable-variant exclusion + latest-per-line collapse, write `models.user.json`). Discovery proven live (OpenRouter + local: 366 discovered → 81 kept). `calibration_tier` ships `unmeasured` for every entry by design — populated from grounded usage, never asserted. Local serving backends documented + exemplified: Ollama/LM Studio (built-in), llama.cpp `:8080` + vLLM `:8000` (added to `config.toml.default`). See `docs/ecodex/integrations/model-registry.md`. 43/43 models-manager tests.
- **Native ntfy mesh wake-listener** (`codex-rs/core/src/ntfy_listener.rs`, branch `feature/native-ntfy-listener` merged `54bf8b6ea6`): native Rust held-connection ntfy stream loop (reqwest, reconnect/backoff, doorbell wake via `inject_response_items` mirroring `monitor.rs`) + session-boot wiring (creds-gated, `cfg!(test)`-guarded) + shutdown abort. Decouples the mesh push transport from the empirica Python CLI so ecodex wakes on mesh events natively. Config + credentials loader, subscribe-URL builder, ai_id resolution. codex-core 1826/0 (+14 listener tests).
- **Translator multi-upstream router** (`codex-empirica-translator`, commit `dcec5b4099`): one translator process now serves N upstream providers, routing per-request via first-match-wins glob on the incoming Responses request's `model` field. New `--upstreams-config <TOML>` flag; existing single-upstream flags preserved for backwards compatibility. Sample config at `examples/upstreams.toml` covers Kimi (Anthropic mode), DeepSeek/Qwen/GLM/Anthropic/OpenRouter as commented templates. New `Upstream` + `UpstreamRouter` public API. `/healthz` now lists configured upstreams inventory. 33/33 tests pass.
- **Plugin multi-script fan-out** (`codex-empirica-plugin`, commit `630b7b2ab7`): empirica plugin now mirrors Claude Code's `~/.claude/settings.json` multi-handler-per-event wiring. New `run-hook EVENT SCRIPT.py` generic dispatcher subcommand; `hooks.json` declares sibling scripts per event (UserPromptSubmit fires 6 scripts, SessionStart 4, PostToolUse 2). Closes the "compulsion gap" where tool-router.py's siblings (`context-shift-tracker`, `loop/listener install/uninstall pickups`) were never firing. Vendored previously-missing `session-monitor-arm.py`.
- **Hook event schema additions** (`codex-rs/protocol/src/protocol.rs` + 12 other files, commit `7bcf85c3b8`): `HookEventName` enum extended with 7 new variants matching CC's wider surface — `PreCompact`, `PostCompact`, `SessionEnd`, `SubagentStart`, `SubagentStop`, `TaskCompleted`, `PostToolUseFailure`. Plugins can declare handlers in `hooks.json` today.
- **TaskCompleted dispatch site** (`codex-rs/hooks/src/events/task_completed.rs` + 6 wiring sites): first of the 7 PR2 dispatch sites. Informational hook fires at the normal agent-done lifecycle point (`codex-rs/core/src/session/turn.rs:570`, after Stop's continuation flow, before the legacy AfterAgent notify). Plugin handlers attach POSTFLIGHT-enforcement here without changing Stop semantics. Pattern proven for the remaining 6 events — see `docs/ecodex/hook-events-roadmap.md` "Dispatch pattern (minimal sibling)" section.
- **PostToolUseFailure dispatch site** (`codex-rs/hooks/src/events/post_tool_use_failure.rs` + 7 wiring sites): second PR2 dispatch site. Fires when a tool invocation fails (non-zero exit, exception, timeout) — sibling to `PostToolUse` which only fires on success. Dispatch at `codex-rs/core/src/tools/registry.rs:434`. Payload: tool_name + tool_input + error_message + duration_ms. Plugin handlers attach dead-end-logging here.
- **PreCompact + PostCompact dispatch sites** (`codex-rs/hooks/src/events/{pre,post}_compact.rs` + 9 wiring sites): third and fourth PR2 dispatch sites. Both fire from `codex-rs/core/src/tasks/compact.rs:CompactTask::run` — the single entry point that branches into the three compaction implementations (local/remote/remote_v2). PreCompact awaits synchronously before compaction runs (natural block via `.await` — no continuation/should_block semantics needed). PostCompact fires after with `success: bool`. Plugins snapshot epistemic state to `~/.empirica/breadcrumbs` on PreCompact, restore on PostCompact.
- **SessionEnd dispatch site** (`codex-rs/hooks/src/events/session_end.rs` + 7 wiring sites): fifth PR2 dispatch site. Fires at the start of `shutdown()` in `codex-rs/core/src/session/handlers.rs`, before `abort_all_tasks`. Plugin handlers run final POSTFLIGHT + capture session snapshot + curate the rollout while history/tasks/MCP are still alive. Payload carries `turn_count` so plugins can apply session-summary thresholds.
- **SubagentStart + SubagentStop dispatch sites** (`codex-rs/hooks/src/events/subagent_{start,stop}.rs` + 9 wiring sites): sixth and seventh PR2 dispatch sites. SubagentStart fires in the parent session after `spawn_agent` tool succeeds (`codex-rs/core/src/tools/handlers/multi_agents/spawn.rs:~176`). SubagentStop fires in the subagent's own session when it calls `report_agent_job_result` (`codex-rs/core/src/tools/handlers/agent_jobs/report_agent_job_result.rs:handle()`). Asymmetric session contexts reflect codex's threading model — plugins correlate parent→child via `child_thread_id` (Start) + `session_id` (Stop).
- **`monitor` tool primitive** (`codex-rs/core/src/monitor.rs` + tool handler + spec registration, commit `5a1ae1658c`): new agent-callable tool that arms a watch on a background subprocess. On each line matching the supplied regex, a `<task-notification>` message is injected into the agent's pending input via `Session::inject_response_items` — sub-second wake on background events. Parity with Claude Code's `Monitor` tool. Bundled spawn+watch in a single tool call (one atomic API). Per-session registry on `SessionServices`; `shutdown()` aborts all entries. Closes the wake-on-event gap that prevented non-Claude models from participating fully in the Empirica AI mesh. See `docs/ecodex/monitor.md`.
- **Inherited-transaction-bug fix** (`codex-rs/codex-empirica-plugin/assets/hooks_scripts/hooks/session-init.py`, commit `b8c9f02ece`): SessionStart now auto-postflights orphaned open transactions with reason `session-pickup auto-close` instead of silently adopting them. Vectors are carried forward from the orphaned state with `completion=1.0`. Prevents cross-session calibration corruption (previous adoption logic continued the old measurement window across unrelated work). Fallback to legacy adoption on auto-postflight failure preserves data.
- **Statusline sync from CC vendored copy** (`codex-rs/codex-empirica-plugin/assets/hooks_scripts/scripts/statusline_empirica.py`, commit `8f2ee46b5c`): pulls in three CC improvements ecodex was missing — praxic-phase emoji changed `⚙` → `🔨` (east-asian-width fix preventing digit overlap on some terminals), threshold info no longer rendered in live statusline (Sentinel-scoped, not actionable mid-call), caller-side threshold fetch removed (saves a DB round-trip per render tick).

### Configuration (no code change)
- **Cortex MCP server wiring guidance for `~/.codex/config.toml`**: ecodex agents can now reach the same `mcp__cortex__*` tools that CC sessions use by adding a `[mcp_servers.cortex]` block with the streamable_http transport and a bearer-token env var. Documented in `docs/ecodex/cross-ai-mesh.md`. With the new `monitor` tool above, this completes the cross-AI mesh story — ecodex participates as a first-class peer.

### Notes
- Goal `f0004294` complete: **7/7 PR2 dispatch sites shipped** this session. All 7 hook events declarable in `hooks.json` AND wired to fire at their lifecycle points. Plugin authors can attach handlers and they will execute.
- Cross-AI mesh participation now structurally complete in ecodex: cortex MCP integration (call mesh tools) + `monitor` primitive (wake on mesh events) + all PR2 dispatch sites (lifecycle hook coverage). Demonstrating an end-to-end mesh interaction between a CC session and an ecodex session running a non-Claude model is the open-source proof point that empirica is a cross-platform layer, not a Claude-specific framework.

## [0.0.1] - 2026-05-10

### Added
- **Tx-BC** (`scripts/release.sh`): Phase 1 of the release pipeline — version bump, CHANGELOG roll, commit, tag. Replaces the prior placeholder stub.
- **Tx-BB**: rustdoc on `codex-empirica-plugin` (47% → 100%) and `codex-empirica-translator` (56% → 64%); overall owned-crate docs coverage 53.6% → 75.0%.
- **Tx-BA** (`empirica rust-docs-assess`): Rust-aware docstring-coverage measurement on Cargo.toml workspace members. Replaces docpistemic for forks where Python-discovery inflates the denominator with upstream noise.
- **Tx-AY**: `[features] plugin_hooks=true` + `plugins=true` seeded by `install.sh` on first install. Cargo.toml recognized by empirica's repo_hygiene `version_file` check.
- **Tx-AW** (`scripts/install.sh`): `--fast` flag (T79) flips builds to `[profile.fast-release]`. Idempotent `[features]` block append for existing configs. `rm`-then-`cp` pattern dodges ETXTBSY on running ecodex sessions. Post-install `pgrep` warning when in-flight sessions need restart.
- **Tx-AT**: empirica plugin auto-trusted on first install via `ECODEX_AUTO_TRUSTED_PLUGIN_IDS` allowlist in upstream's hook discovery (codex-rs/hooks/src/engine/discovery.rs). Restores pre-PR-#20321 first-install runnability.
- **Tx-AO**: upstream codex sync — 162 commits forward-ported. T78 `ArcSwap<ModelClient>` hot-swap pattern preserved against upstream's reverted `client_session` shape.
- **Tx-AI**: plugin manifest `writableRoots` contribution surface. Plugins declare cross-cwd writable scope; codex's SandboxPolicy honors them at session bootstrap.
- **Tx-AG**: investigation-proportionality budget enforcement. `tool-router.py` arms a per-session counter on hypothesis-bearing prompts; `sentinel-gate.py` denies Read/Grep/Glob after the configured limit (default 5) until the next user prompt resets.
- **Tx-AJ**: `EMPIRICA_SENTINEL_FAIL_CLOSED` env opt-in for hardened deployments — outermost catch flips from fail-open allow to fail-closed deny.
- **CONTRIBUTING.md**: three-layer contribution model (L1 codex foundation / L2 empirica plugin / L3 ecodex-specific) with concrete routing.
- **docs/ecodex/INSTALL.md**: prerequisites, --user vs --system, providers config, hot-swap semantics, idempotent reinstall.

### Changed
- **README.md**: sharper transaction-lifecycle phrasing (PREFLIGHT / CHECK / POSTFLIGHT defined inline with the noetic↔praxic distinction). New Glossary section. Relationship-to-codex section now names the one upstream patch we maintain (Tx-AT allowlist).

### Fixed
- **Tx-AU**: workspace test fixtures resolved post-Tx-AO — `TurnStartParams` literal in v2/tests.rs gained `model_provider`; `LoadedPlugin` test fixture gained extension fields; 7 conflict markers in core-skills/loader_tests.rs resolved.
- **Tx-AV**: `cargo clippy --workspace --all-targets` from 28 errors → 0 after rust 1.93's stricter lints landed via the upstream sync.

## [0.0.0]

Pre-release development. Not yet versioned. The full pre-versioning history is in the git log and on the [build/v1-plugin branch](https://github.com/Nubaeon/ecodex/commits/build/v1-plugin).

[Unreleased]: https://github.com/Nubaeon/ecodex/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/Nubaeon/ecodex/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/Nubaeon/ecodex/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/Nubaeon/ecodex/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Nubaeon/ecodex/compare/v0.0.1...v0.1.0
[0.0.1]: https://github.com/Nubaeon/ecodex/compare/v0.0.0...v0.0.1
[0.0.0]: https://github.com/Nubaeon/ecodex/releases/tag/v0.0.0
