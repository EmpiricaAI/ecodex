# codex-empirica-plugin

Codex plugin that brings the [Empirica](https://github.com/Nubaeon/empirica) epistemic discipline framework — sentinel firewall, PREFLIGHT/CHECK/POSTFLIGHT transactions, calibration vectors — to codex.

## How it works

This crate builds a single binary, `codex-empirica-plugin`, that codex invokes for each hook event. The binary dispatches by subcommand:

```
codex-empirica-plugin pre-tool-use         # Sentinel firewall
codex-empirica-plugin post-tool-use        # Tool result capture
codex-empirica-plugin session-start        # Empirica session bootstrap
codex-empirica-plugin user-prompt-submit   # Context injection, hedge detection
codex-empirica-plugin stop                 # Transaction enforcer
codex-empirica-plugin permission-request   # (codex-specific, currently no-op)
```

Each handler reads codex's hook JSON from stdin, forwards it to the corresponding Empirica Python script, and returns the script's response in codex's expected format.

## v1 implementation: subprocess shellout

The plugin currently invokes Empirica's existing Python hook scripts (e.g. `sentinel-gate.py`) via subprocess. Latency budget is ~100–300ms per hook fire (Python startup + Empirica logic). This validates the integration end-to-end without rewriting any Empirica logic.

**The single optimization target** is `src/empirica_cli.rs`. Future versions can replace the `python3 <script>` subprocess with PyO3 in-process embedding or a long-running Empirica sidecar daemon over UDS — without changing the hook handlers or the codex-side wiring.

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `EMPIRICA_HOOKS_DIR` | `~/.claude/plugins/local/empirica/hooks` | Location of Empirica's Python hook scripts |

## Install (planned)

The plugin manifest (`manifest.json`) is loadable by codex's plugin marketplace at `~/.codex/plugins/cache/empirica/<version>/manifest.json`. Install steps are part of the ecodex distribution layer.

## Status

**v1 scaffolding** — only `pre-tool-use` is wired through. Other events stub-succeed. See parent goal "Build v1 empirica plugin for codex" for porting plan.
