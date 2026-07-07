//! PreToolUse hook → Empirica sentinel-gate.
//!
//! Reads codex's PreToolUse JSON payload from stdin, forwards it to the
//! existing Empirica `sentinel-gate.py` script, and propagates the
//! script's stdout / stderr / exit code back to codex.
//!
//! Codex's PreToolUse hook protocol matches Claude Code's. The two share
//! field names (`session_id`, `cwd`, `tool_name`, `tool_input`,
//! `permission_mode`, etc.), so the input is forwarded verbatim. If
//! protocol drift appears later, translation lands in this module.

use std::io::Read;
use std::process::ExitCode;

use anyhow::Result;

use crate::empirica_cli;
use crate::translate_output;

/// What the firewall should do, derived from the gate's run result + whether
/// the gate script is installed at all. Kept as a pure, testable mapping so the
/// fail-open/fail-closed policy is verified independently of stdin/stdout I/O.
#[derive(Debug, PartialEq, Eq)]
enum FirewallOutcome {
    /// The gate ran and produced a decision in stdout; forward it with this
    /// exit code (`0` = allow, `2` = deny). codex parses the translated stdout
    /// (allow / deny+reason).
    Forward(u8),
    /// The gate is present but did NOT produce a usable decision — it ran and
    /// exited with an unexpected code (e.g. a python traceback → exit 1), or it
    /// is installed but could not be spawned. A broken firewall must BLOCK, not
    /// silently allow → synthesize a deny (exit 2 + non-empty stderr).
    FailClosed,
    /// The gate is genuinely absent (not installed). The user opted out of the
    /// firewall; allow so an un-gated install isn't bricked.
    FailOpenAbsent,
}

/// Pure policy: map the gate run + installed-ness to a firewall outcome.
///
/// codex's PreToolUse contract only ever BLOCKS on `exit 2 + non-empty stderr`
/// or `exit 0 + stdout permissionDecision=deny+reason`; every other shape —
/// including any unexpected exit code — FAILS OPEN. So a gate that crashes
/// mid-run (exit ∉ {0,2}) or can't be spawned would, untranslated, let the
/// tool through. This mapping closes that: only a genuinely absent gate
/// fails open; a present-but-broken gate fails closed.
fn firewall_outcome(
    run: &Result<empirica_cli::HookOutput>,
    script_exists: bool,
) -> FirewallOutcome {
    match run {
        Ok(output) => match output.exit_code {
            0 => FirewallOutcome::Forward(0),
            2 => FirewallOutcome::Forward(2),
            _ => FirewallOutcome::FailClosed,
        },
        Err(_) => {
            if script_exists {
                FirewallOutcome::FailClosed
            } else {
                FirewallOutcome::FailOpenAbsent
            }
        }
    }
}

/// Run the PreToolUse hook against the current invocation.
///
/// Reads codex's hook payload from stdin, dispatches it to `sentinel-gate.py`,
/// translates the script's CC-shape JSON output to codex's
/// `hookSpecificOutput.permissionDecision` form, and decides allow/deny.
///
/// Exit code semantics (matched to codex's PreToolUse contract):
/// - `0` → allow the tool call.
/// - `2` → deny the tool call. codex blocks on `exit 2 + non-empty stderr`.
/// - any other code, or a spawn failure, → the firewall is the security floor,
///   so a BROKEN gate fails CLOSED (re-emitted as `exit 2 + stderr`), not open.
///   The one fail-open case is a genuinely ABSENT (uninstalled) gate.
pub fn handle() -> ExitCode {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        // Can't read the tool payload → can't gate. The firewall is the floor:
        // deny rather than silently allow. codex blocks on exit 2 + stderr.
        eprintln!(
            "ecodex firewall: could not read PreToolUse payload ({e}) — failing closed (denying this tool call)."
        );
        return ExitCode::from(2);
    }

    let run = empirica_cli::run_hook_script("sentinel-gate.py", &input);
    let script_exists = empirica_cli::hook_script_exists("sentinel-gate.py");

    match firewall_outcome(&run, script_exists) {
        FirewallOutcome::Forward(code) => {
            // Forward is only produced from an Ok(run). Defensively fail
            // closed rather than panic if that invariant is ever violated —
            // a firewall must never crash-open.
            let Ok(output) = run else {
                eprintln!(
                    "ecodex firewall: Forward outcome without Ok(run) — internal invariant \
                     violated; failing closed (denying this tool call)."
                );
                return ExitCode::from(2);
            };
            // ecodex T81 Tx-AE: translate CC-shape output into codex-shape
            // (permissionDecision/permissionDecisionReason inside
            // hookSpecificOutput, suppressOutput dropped — codex rejects it).
            print!(
                "{}",
                translate_output::translate("PreToolUse", &output.stdout)
            );
            eprint!("{}", output.stderr);
            ExitCode::from(code)
        }
        FirewallOutcome::FailClosed => {
            // Surface any stderr the gate emitted, then a clear fail-closed
            // reason. codex blocks on exit 2 + non-empty stderr, so the reason
            // line below is what reaches the model.
            let detail = match &run {
                Ok(o) => {
                    eprint!("{}", o.stderr);
                    format!("sentinel-gate ran but exited {} (crashed)", o.exit_code)
                }
                Err(e) => format!("sentinel-gate is installed but failed to run ({e})"),
            };
            eprintln!(
                "ecodex firewall: {detail} — failing closed (denying this tool call). \
                 Fix or remove the gate to proceed."
            );
            ExitCode::from(2)
        }
        FirewallOutcome::FailOpenAbsent => {
            if let Err(e) = &run {
                eprintln!(
                    "codex-empirica-plugin pre-tool-use: sentinel-gate not installed ({e}) — allowing (firewall absent)."
                );
            }
            ExitCode::SUCCESS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::empirica_cli::HookOutput;

    fn ran(exit_code: i32) -> Result<HookOutput> {
        Ok(HookOutput {
            stdout: String::new(),
            stderr: String::new(),
            exit_code,
        })
    }

    fn failed_to_run() -> Result<HookOutput> {
        Err(anyhow::anyhow!("spawn failed"))
    }

    #[test]
    fn exit_0_forwards_allow() {
        assert_eq!(firewall_outcome(&ran(0), true), FirewallOutcome::Forward(0));
    }

    #[test]
    fn exit_2_forwards_deny() {
        assert_eq!(firewall_outcome(&ran(2), true), FirewallOutcome::Forward(2));
    }

    #[test]
    fn unexpected_exit_code_fails_closed() {
        // A python traceback exits 1; codex would treat any non-2 code as
        // allow. The firewall must block instead.
        assert_eq!(firewall_outcome(&ran(1), true), FirewallOutcome::FailClosed);
        assert_eq!(
            firewall_outcome(&ran(127), true),
            FirewallOutcome::FailClosed
        );
        assert_eq!(
            firewall_outcome(&ran(-1), true),
            FirewallOutcome::FailClosed
        );
    }

    #[test]
    fn run_failure_with_gate_present_fails_closed() {
        // Gate installed but unrunnable (spawn/IO error) → block.
        assert_eq!(
            firewall_outcome(&failed_to_run(), true),
            FirewallOutcome::FailClosed
        );
    }

    #[test]
    fn run_failure_with_gate_absent_fails_open() {
        // Gate genuinely not installed → don't brick the harness.
        assert_eq!(
            firewall_outcome(&failed_to_run(), false),
            FirewallOutcome::FailOpenAbsent
        );
    }
}
