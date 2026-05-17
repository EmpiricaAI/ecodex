# ecodex docs

Fork-specific documentation for ecodex (Nubaeon's branded build of [openai/codex](https://github.com/openai/codex) with the Empirica epistemic discipline plugin bundled).

## Layout

| Path | Contains |
|---|---|
| [`INSTALL.md`](INSTALL.md) | Install + first-run guide. Prerequisites, `--user` vs `--system`, provider config, hot-swap semantics, idempotent reinstall. |
| [`system-overview.md`](system-overview.md) | The three-layer architecture (L1 codex foundation / L2 empirica integration / L3 specialised ecodex code) + how state files compose. |
| [`monitor.md`](monitor.md) | The `monitor` tool — arm a watched subprocess + regex pattern, get sub-second wakes via injected `<task-notification>` messages. Parity with CC's `Monitor`. |
| [`cross-ai-mesh.md`](cross-ai-mesh.md) | The cross-AI mesh story — how ecodex sessions participate as first-class peers in the Empirica AI mesh (cortex MCP + `monitor` primitive + vendored hook scripts). Setup + walkthrough + troubleshooting. |
| [`hook-events-roadmap.md`](hook-events-roadmap.md) | The 7 PR2 hook events ecodex adds beyond stock codex's 6, where each one's dispatch site lives, and the "dispatch pattern (minimal sibling)" template for new events. |
| [`epistemic-llms.md`](epistemic-llms.md) | Background on why empirica's epistemic-discipline framing matters for LLM-based agents. |
| [`inspection.md`](inspection.md) | T2 investigation of codex-rs architecture (hook system, plugin marketplace, thread-scoped goals). Drives the architecture decision. |
| [`architecture.md`](architecture.md) | T3 architecture commitments (distribution model, empirica-language strategy, goal pairing, memory interop, fork posture). |
| [`specs/`](specs/) | Design specs for parked or in-progress work. |
| [`api/`](api/) | API contracts (plugin CLI subcommands, hook payload schemas, MCP tool registrations). Populated as APIs stabilize. |
| [`integrations/`](integrations/) | Integration design notes for future components (Cockpit, Symphony, etc.). Populated as decisions land. |

## Current status

- **Architecture:** committed in [`architecture.md`](architecture.md) (T3). Dual-product distribution (plugin + fork), subprocess shellout for empirica integration, goal pairing via objective-tag, memory parallel.
- **v1 plugin:** all 6 stock codex hook events fully wired (PreToolUse / PermissionRequest / PostToolUse / SessionStart / UserPromptSubmit / Stop), CC-shape ↔ codex-shape JSON translation in the Rust dispatcher, plus multi-script fan-out per event mirroring CC's `~/.claude/settings.json`. See the crate's [README](../../codex-rs/codex-empirica-plugin/README.md).
- **Extended hook surface (PR2):** the 7 new ecodex events (`TaskCompleted`, `PostToolUseFailure`, `PreCompact`, `PostCompact`, `SessionEnd`, `SubagentStart`, `SubagentStop`) shipped 2026-05-17 — declarable in `hooks.json` AND firing at their lifecycle points. See [`hook-events-roadmap.md`](hook-events-roadmap.md).
- **Cross-AI mesh:** ecodex participates as a first-class peer in the empirica AI mesh — cortex MCP wires the call-side, the new `monitor` tool wires the wake-side. See [`cross-ai-mesh.md`](cross-ai-mesh.md).
- **Distribution:** Homebrew + direct binary + cargo + source-build all produce the same `ecodex`. See [`INSTALL.md`](INSTALL.md).

## Where things live

| Concern | Location |
|---|---|
| The plugin source | `codex-rs/codex-empirica-plugin/` |
| The plugin's own README (cargo convention) | `codex-rs/codex-empirica-plugin/README.md` |
| Plugin manifest | `codex-rs/codex-empirica-plugin/manifest.json` |
| Investigation + architecture docs | `docs/ecodex/` (this directory) |
| Upstream codex docs | `docs/` (parent dir, untouched) |

## Contributing

ecodex is a product fork — quality fixes that aren't ecodex-specific should land both in our fork and as upstream PRs against [openai/codex](https://github.com/openai/codex). See [`architecture.md`](architecture.md) §"Strategic posture".
