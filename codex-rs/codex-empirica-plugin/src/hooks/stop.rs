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

pub fn handle() -> ExitCode {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("codex-empirica-plugin stop: failed to read stdin: {e}");
        return ExitCode::SUCCESS;
    }

    match empirica_cli::run_hook_script("transaction-enforcer.py", &input) {
        Ok(output) => {
            print!("{}", output.stdout);
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
