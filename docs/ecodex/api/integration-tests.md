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

## What was NOT tested (deferred to live integration)

- **Strict-gating mode** — would need `EMPIRICA_PROJECT_ROOT` override or fresh empirica install to verify rm/Write actually get blocked when no PREFLIGHT exists for the test session
- **Latency measurement** — didn't time the subprocess round-trip
- **All 5 active hooks end-to-end** — only PreToolUse + Stop tested; PostToolUse + SessionStart + UserPromptSubmit follow same pattern but unverified against real scripts
- **Codex actually loading the plugin** — requires building the codex binary and configuring plugin discovery, deferred to live integration

## Action items surfaced

1. **Strict-gating test** — needs proper EMPIRICA_HOME isolation; add to v1 ship checklist.
2. **Latency baseline** — measure subprocess round-trip time during live integration; validate the 100-300ms estimate from T2.
3. **PostToolUse / SessionStart / UserPromptSubmit real-script pass** — repeat T14 pattern for these three; should pass since pattern is identical, but worth confirming.
