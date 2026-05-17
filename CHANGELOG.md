# Changelog

All notable ecodex-specific changes are documented here. ecodex is a fork of [openai/codex](https://github.com/openai/codex); upstream codex changes are tracked at the [openai/codex releases](https://github.com/openai/codex/releases) page.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and version numbers follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`scripts/release.sh` rolls the [Unreleased] section under a new version stamp on each release. Add entries to [Unreleased] as you ship; the release script promotes them.

## [Unreleased]

### Added
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

[Unreleased]: https://github.com/Nubaeon/ecodex/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/Nubaeon/ecodex/compare/v0.0.0...v0.0.1
[0.0.0]: https://github.com/Nubaeon/ecodex/releases/tag/v0.0.0
