# ecodex docs

Fork-specific documentation for ecodex (Nubaeon's branded build of [openai/codex](https://github.com/openai/codex) with the Empirica epistemic discipline plugin bundled).

## Layout

| Path | Contains |
|---|---|
| [`inspection.md`](inspection.md) | T2 investigation of codex-rs architecture (hook system, plugin marketplace, thread-scoped goals). Drives the architecture decision. |
| [`architecture.md`](architecture.md) | T3 architecture commitments (distribution model, empirica-language strategy, goal pairing, memory interop, fork posture). |
| [`specs/`](specs/) | Design specs for parked or in-progress work. |
| [`api/`](api/) | API contracts (plugin CLI subcommands, hook payload schemas, MCP tool registrations). Populated as APIs stabilize. |
| [`integrations/`](integrations/) | Integration design notes for future components (Cockpit, Symphony, etc.). Populated as decisions land. |

## Current status

- **Architecture:** committed in [`architecture.md`](architecture.md) (T3). Dual-product distribution (plugin + fork), subprocess shellout for empirica integration, goal pairing via objective-tag, memory parallel.
- **v1 plugin:** scaffolded at `codex-rs/codex-empirica-plugin/`. `PreToolUse` and `Stop` hook events ported (subprocess to existing Empirica Python scripts). Other 4 events stubbed. See the crate's [README](../../codex-rs/codex-empirica-plugin/README.md) for hook-by-hook status.
- **Distribution layer:** planned, depends on plugin completion.

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
