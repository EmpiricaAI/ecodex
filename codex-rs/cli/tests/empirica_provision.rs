use std::path::Path;

use anyhow::Result;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("ecodex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

/// What ecodex left in a codex home: (plugin manifest present, config.toml present).
fn provisioned_state(codex_home: &Path) -> (bool, bool) {
    (
        codex_home
            .join("plugins/cache/empiricaAI/empirica/0.1.0/.codex-plugin/plugin.json")
            .is_file(),
        codex_home.join("config.toml").is_file(),
    )
}

#[test]
fn session_commands_install_the_empirica_plugin() -> Result<()> {
    let codex_home = TempDir::new()?;

    // No prompt on stdin, so exec stops right after startup provisioning.
    codex_command(codex_home.path())?
        .args(["exec", "--skip-git-repo-check"])
        .write_stdin("")
        .assert()
        .failure();

    assert_eq!(provisioned_state(codex_home.path()), (true, true));
    Ok(())
}

#[test]
fn admin_commands_leave_a_fresh_home_alone() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["mcp", "list"])
        .assert()
        .success();

    assert_eq!(provisioned_state(codex_home.path()), (false, false));
    Ok(())
}
