//! UserPromptSubmit hook → Empirica tool-router / context injection.
//!
//! Fires when the user submits a prompt to the agent. Forwards codex's
//! UserPromptSubmit JSON payload to `tool-router.py` (which detects hedging
//! language, injects EWM-protocol context, and rewrites the prompt with
//! Empirica session context).
//!
//! Same fail-open semantics as the other hooks.

use std::io::Read;
use std::process::ExitCode;

use crate::empirica_cli;
use crate::translate_output;

pub fn handle() -> ExitCode {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("codex-empirica-plugin user-prompt-submit: failed to read stdin: {e}");
        return ExitCode::SUCCESS;
    }

    match empirica_cli::run_hook_script("tool-router.py", &input) {
        Ok(output) => {
            // ecodex T81 Tx-AE: translate CC-shape JSON ({continue, context})
            // into codex-shape ({continue, hookSpecificOutput:{...}}). Codex's
            // hook output schema is `additionalProperties: false`; raw CC
            // output gets rejected as "invalid user prompt submit JSON".
            print!(
                "{}",
                translate_output::translate("UserPromptSubmit", &output.stdout)
            );
            eprint!("{}", output.stderr);
            match output.exit_code {
                0 => ExitCode::SUCCESS,
                2 => ExitCode::from(2),
                code => ExitCode::from((code & 0xff) as u8),
            }
        }
        Err(e) => {
            eprintln!("codex-empirica-plugin user-prompt-submit: subprocess failure ({e})");
            ExitCode::SUCCESS
        }
    }
}
