# Handoff — drive ecodex-lab (next session, post-compact)

**Written 2026-06-03, end of a long ecodex session. Read this + `ecodex-lab-design.md` first; everything below is verified-current, not aspirational.**

## One-line state
The harness is built, installed (v0.1.0), and verified running. All infra blockers to the ecodex-lab orchestration experiment are fixed. The next move is the **first live experiment**: stand up ecodex-lab against Kimi, confirm the Sentinel actually fires in a live session, take the first grounded calibration baseline.

## What is READY (verified this session, not assumed)
- **ecodex binary**: `~/.local/bin/ecodex`, `codex-cli 0.1.0`. `models list` → 11 L3 entries. doctor mostly green.
- **ecodex-lab practice**: project created + session came up clean. `ai_id=ecodex-lab`, project_id `810e03fc-514f-457c-ae25-9c72a1e68162`, folder `~/empirical-ai/ecodex-lab/`. (Created via the `project-switch` + `session-create` workaround; the auto-init bug that forced that is now FIXED in empirica — see below.)
- **Kimi path (preferred model — easier to guide)**: translator binary built (`codex-rs/target/release/codex-empirica-translator`), config at `~/.codex/upstreams.toml` (kimi/anthropic). Start it: `set -a; . ~/.codex/.env; set +a; <translator-bin> --upstreams-config ~/.codex/upstreams.toml --bind 127.0.0.1:18080`. Verified live: `/v1/responses` → Kimi → HTTP 200. KIMI_API_KEY is in `~/.codex/.env`. NOTE: it's a foreground/manual start — re-launch it if the box rebooted.
- **empirica-server (Ollama) fallback**: live at `http://empirica-server:11434/v1` (qwen3.6:35b etc.) if Kimi is down.
- **cortex MCP**: FIXED — use `url = "https://cortex.getempirica.com/mcp/"` (trailing slash REQUIRED). Already updated in `~/.codex/config.toml` + `docs/ecodex/integrations/cross-ai-mesh.md`.

## What got FIXED at source this session (all 3 mesh round-trips closed)
- cortex MCP 405 → cortex `b1f5ff0` (streamable_http `/mcp` endpoint). Verified live.
- project-init git-init chicken-egg → empirica `651c4b5c3`. Verified by diff.
- session-create `--auto-init` missing-active-pointer → empirica `59fce18c4` (wires `instance_projects/{instance_id}.json` via project_path override). Verified by diff. **So `--auto-init` should now work directly — the project-switch workaround is no longer needed.**

## The verified orchestration mechanism (load-bearing)
Mesh wake = the ntfy listener `inject_response_items` a USER-role directive ("Poll cortex_inbox_poll(...)") into a RUNNING ecodex session. It wakes an existing loop; it does NOT start one. So orchestration = `CC sends proposal → doorbell wakes ecodex-lab's live session → it's told to poll → Kimi must follow → poll → execute`. Whether Kimi reliably does doorbell→poll→execute IS experiment 1's delivery-path test.

## EXPERIMENT 1 — what to run (planned goal `1ef51dbd`)
The local half needs no mesh; do it first:
1. Launch a live ecodex session in `~/empirical-ai/ecodex-lab/`, model = Kimi.
2. **Sentinel-fires proof**: confirm a praxic action (Write/Bash) is GATED without an open transaction. (doctor confirms plugin LOADED; this is the still-owed proof it FIRES in a live session. NOTE: live demo'd in-session this session — `echo` got gated as praxic — but a clean in-ecodex-lab repro is the real test.)
3. **Calibration baseline**: give ecodex-lab a hard-ground-truth task (e.g. fix a known-failing test). Capture PREFLIGHT→CHECK→POSTFLIGHT, record belief-vs-evidence divergence = trajectory point 1.
4. **Delivery-path** (needs cortex MCP live in ecodex-lab + a Cortex project registration for ecodex-lab — NOT yet done): CC sends a proposal, observe doorbell→poll→execute.

Pre-registered success criteria are in `ecodex-lab-design.md`.

## Open methodological commitments (don't drop)
- Practice is SUBJECT in v1 (cultivated trajectory), not instrument. Declared per-run.
- Ground truth INDEPENDENT of the subject's self-report (tests/tools/checkable), or it's vibes-vs-vibes.
- Distillation = Reading A (calibration accrues to the practice substrate, model-independent). Reading B (harness self-tunes its measurement code) is UNBUILT — keep it out of claims.
- `calibration_tier` stays `unmeasured` until earned from real usage.

## Loose ends (tracked, none blocking experiment 1's local half)
- ecodex-lab not yet a registered Cortex project → blocks the mesh delivery-path step (4) only. Register before that step.
- Self-bootstrap goal `c7233d09`: the bugs are fixed, but the declarative "setup asks which practitioners to provision" idea + ai_id-anchoring refactor remain (empirica's SER `ser_cd095e` owns this).
- Vanilla-plugin compat goal `2f6eaec1`: still open (does vanilla codex's PluginManifest deny_unknown reject statusline/writableRoots?).
- Minor mesh issues flagged in-band: inbox-replay noise (every poll re-delivers accepted backlog); MCP client no auto-reconnect on dropped SSE.
- Disk: 78%+ / target ~126G — `cargo clean` ecodex/target/debug reclaims a lot if it climbs.

## My own calibration notes this session (for the record)
Three slips, same family — acting on a heuristic without checking it against the case: fabricated test counts (twice), the api_key rabbit hole, the wrong fix-mechanism for bug#2 (right diagnosis, clumsier patch than empirica's instance_projects fix). The discipline that worked: verify the artifact (read the diff / log / probe), don't trust the status line. Applied cleanly 4× by session end.
