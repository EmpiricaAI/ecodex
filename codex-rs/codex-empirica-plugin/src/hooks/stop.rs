//! Stop hook → Empirica transaction-enforcer.
//!
//! Fires when codex ends a session/turn. Forwards codex's Stop JSON payload
//! to `transaction-enforcer.py`, which decides whether to block (POSTFLIGHT
//! still pending past the hard threshold) or allow.
//!
//! Block semantics match `pre_tool_use`: exit code 2 + stderr signals block;
//! exit 0 allows. Any other condition fails open (matches Empirica's own
//! sentinel/enforcer convention — never strand the session on plugin error).

use std::io::Read;
use std::process::ExitCode;

use crate::empirica_cli;
use crate::translate_output;

/// Run the Stop hook against the current invocation.
///
/// Fires when codex ends a session or turn. Forwards the payload to
/// `transaction-enforcer.py`, which decides whether the session has an
/// open transaction past the hard POSTFLIGHT threshold and should be
/// blocked from ending until measurement closes.
///
/// Exit code semantics:
/// - `0` → allow the stop (no open transaction OR within grace period).
/// - `2` → block the stop. Codex emits the script's stderr to the
///   model with instructions to POSTFLIGHT before ending.
/// - any other → fail-open. Plugin infrastructure failures must not
///   strand the session.
pub fn handle() -> ExitCode {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("codex-empirica-plugin stop: failed to read stdin: {e}");
        return ExitCode::SUCCESS;
    }

    match empirica_cli::run_hook_script("transaction-enforcer.py", &input) {
        Ok(output) => {
            // ecodex T81 Tx-AE: Stop schema is mostly compatible with CC,
            // but we still pass through the whitelisted fields to drop any
            // unknown keys codex would reject.
            print!("{}", translate_output::translate("Stop", &output.stdout));
            eprint!("{}", output.stderr);
            match output.exit_code {
                0 => ExitCode::SUCCESS,
                2 => ExitCode::from(2),
                code => ExitCode::from((code & 0xff) as u8),
            }
        }
        Err(e) => {
            eprintln!("codex-empirica-plugin stop: subprocess failure ({e})");
            ExitCode::SUCCESS
        }
    }
}
