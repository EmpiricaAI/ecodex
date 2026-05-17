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

### Notes
- Goal `f0004294` tracks the PR2 dispatch-site implementation. 1/7 shipped (TaskCompleted). Priority remaining: `PostToolUseFailure` next for agent-discipline enforcement gap, then compact pair, then SessionEnd, then Subagent pair.

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
