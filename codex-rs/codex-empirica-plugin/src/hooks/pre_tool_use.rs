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

use crate::empirica_cli;
use crate::translate_output;

/// Run the PreToolUse hook against the current invocation.
///
/// Reads codex's hook payload from stdin, dispatches it to
/// `sentinel-gate.py`, translates the script's CC-shape JSON output to
/// codex's `hookSpecificOutput.permissionDecision` form, and propagates
/// stderr + exit code unchanged.
///
/// Exit code semantics (matched to codex's PreToolUse contract):
/// - `0` → allow the tool call.
/// - `2` → deny the tool call (fail-closed). Codex emits the script's
///   stdout to the model as the rejection reason.
/// - any other → treated as allow (fail-open). The plugin is the
///   firewall; a crash inside the python script must not silently
///   block legit tool calls. Discipline relies on explicit `2`.
pub fn handle() -> ExitCode {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("codex-empirica-plugin pre-tool-use: failed to read stdin: {e}");
        return ExitCode::SUCCESS;
    }

    match empirica_cli::run_hook_script("sentinel-gate.py", &input) {
        Ok(output) => {
            // ecodex T81 Tx-AE: translate CC-shape output (decision/stopReason/
            // suppressOutput at top level) into codex-shape (permissionDecision/
            // permissionDecisionReason inside hookSpecificOutput, suppressOutput
            // dropped). codex's PreToolUse schema rejects suppressOutput.
            print!(
                "{}",
                translate_output::translate("PreToolUse", &output.stdout)
            );
            eprint!("{}", output.stderr);
            match output.exit_code {
                0 => ExitCode::SUCCESS,
                2 => ExitCode::from(2),
                code => ExitCode::from((code & 0xff) as u8),
            }
        }
        Err(e) => {
            // Fail-open: matches Empirica's own sentinel-gate behavior on crash.
            // If the plugin itself can't reach the script, do not block work.
            eprintln!("codex-empirica-plugin pre-tool-use: subprocess failure ({e})");
            ExitCode::SUCCESS
        }
    }
}
