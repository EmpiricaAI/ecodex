//! PostToolUse hook → Empirica tool-failure capture.
//!
//! Fires after every codex tool execution. Forwards codex's PostToolUse JSON
//! payload to `tool-failure.py` (which captures error context for failures
//! and updates Empirica's session state).
//!
//! Same fail-open semantics as the other hooks: infrastructure failures
//! become exit 0; intentional script blocks (exit 2 + stderr) propagate.

use std::io::Read;
use std::process::ExitCode;

use crate::empirica_cli;
use crate::translate_output;

/// Run the PostToolUse hook against the current invocation.
///
/// Fires after codex executes a tool call. Forwards the codex payload
/// (tool result + duration + status) to `tool-failure.py` (legacy name;
/// the script handles success too). Updates noetic vs praxic counters,
/// tracks edited file paths for non-git change detection, surfaces
/// re-read advisories.
///
/// Always exits `0` for infrastructure failures — PostToolUse is
/// observation-only, it can't undo a tool call that already ran.
/// Intentional script blocks (`exit 2` + structured stderr) propagate.
pub fn handle() -> ExitCode {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("codex-empirica-plugin post-tool-use: failed to read stdin: {e}");
        return ExitCode::SUCCESS;
    }

    match empirica_cli::run_hook_script("tool-failure.py", &input) {
        Ok(output) => {
            // ecodex T81 Tx-AE: CC→codex shape translation.
            print!(
                "{}",
                translate_output::translate("PostToolUse", &output.stdout)
            );
            eprint!("{}", output.stderr);
            match output.exit_code {
                0 => ExitCode::SUCCESS,
                2 => ExitCode::from(2),
                code => ExitCode::from((code & 0xff) as u8),
            }
        }
        Err(e) => {
            eprintln!("codex-empirica-plugin post-tool-use: subprocess failure ({e})");
            ExitCode::SUCCESS
        }
    }
}
