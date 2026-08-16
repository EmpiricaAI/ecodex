use std::cell::RefCell;
use std::fs;
use std::path::Path;

use anyhow::Result;
use tempfile::tempdir;

use super::BootstrapCommand;
use super::PracticeBootstrapOutcome;
use super::ensure_practice_with;

fn input_for(cwd: &Path) -> String {
    serde_json::json!({"cwd": cwd}).to_string()
}

fn recording_runner<'a>(
    calls: &'a RefCell<Vec<BootstrapCommand>>,
) -> impl FnMut(BootstrapCommand, &Path) -> Result<()> + 'a {
    move |command, workspace| {
        calls.borrow_mut().push(command);
        match command {
            BootstrapCommand::GitInit => fs::create_dir(workspace.join(".git"))?,
            BootstrapCommand::EmpiricaProjectInit => {
                fs::create_dir(workspace.join(".empirica"))?;
                fs::write(
                    workspace.join(".empirica/project.yaml"),
                    "name: fresh\nai_id: fresh\n",
                )?;
            }
        }
        Ok(())
    }
}

#[test]
fn fresh_workspace_initializes_transport_then_practice_and_preserves_env_identity() {
    let temp = tempdir().expect("create temp dir");
    let calls = RefCell::new(Vec::new());

    let outcome = ensure_practice_with(
        &input_for(temp.path()),
        temp.path(),
        /*home*/ None,
        Some("ecodex-lab"),
        |_| false,
        recording_runner(&calls),
    )
    .expect("bootstrap fresh practice");

    assert!(
        outcome
            == PracticeBootstrapOutcome {
                workspace: temp.path().canonicalize().expect("canonical temp dir"),
                git_initialized: true,
                empirica_initialized: true,
            }
    );
    assert!(
        calls.into_inner()
            == vec![
                BootstrapCommand::GitInit,
                BootstrapCommand::EmpiricaProjectInit,
            ]
    );
    let project =
        fs::read_to_string(temp.path().join(".empirica/project.yaml")).expect("read project yaml");
    assert!(project.contains("ai_id: \"ecodex-lab\"\n"));
}

#[test]
fn initialized_workspace_is_a_noop() {
    let temp = tempdir().expect("create temp dir");
    fs::create_dir(temp.path().join(".git")).expect("create git metadata");
    fs::create_dir(temp.path().join(".empirica")).expect("create practice metadata");
    let calls = RefCell::new(Vec::new());

    let outcome = ensure_practice_with(
        &input_for(temp.path()),
        temp.path(),
        /*home*/ None,
        Some("ecodex-lab"),
        |_| false,
        recording_runner(&calls),
    )
    .expect("check initialized practice");

    assert!(!outcome.changed());
    assert!(calls.into_inner().is_empty());
}

#[test]
fn existing_git_transport_only_initializes_empirica() {
    let temp = tempdir().expect("create temp dir");
    fs::create_dir(temp.path().join(".git")).expect("create git metadata");
    let calls = RefCell::new(Vec::new());

    ensure_practice_with(
        &input_for(temp.path()),
        temp.path(),
        /*home*/ None,
        /*ai_id*/ None,
        |_| false,
        recording_runner(&calls),
    )
    .expect("bootstrap practice metadata");

    assert!(calls.into_inner() == vec![BootstrapCommand::EmpiricaProjectInit]);
}

#[test]
fn hook_cwd_must_match_process_cwd() {
    let process = tempdir().expect("create process dir");
    let other = tempdir().expect("create other dir");
    let calls = RefCell::new(Vec::new());

    let error = ensure_practice_with(
        &input_for(other.path()),
        process.path(),
        /*home*/ None,
        /*ai_id*/ None,
        |_| false,
        recording_runner(&calls),
    )
    .expect_err("reject mismatched cwd");

    assert!(error.to_string().contains("refusing to bootstrap"));
    assert!(calls.into_inner().is_empty());
}

#[test]
fn nested_workspace_does_not_mutate_ancestor_repository() {
    let temp = tempdir().expect("create temp dir");
    fs::create_dir(temp.path().join(".git")).expect("create ancestor git metadata");
    let nested = temp.path().join("nested");
    fs::create_dir(&nested).expect("create nested workspace");
    let calls = RefCell::new(Vec::new());

    let error = ensure_practice_with(
        &input_for(&nested),
        &nested,
        /*home*/ None,
        /*ai_id*/ None,
        |_| true,
        recording_runner(&calls),
    )
    .expect_err("reject nested workspace");

    assert!(error.to_string().contains("ancestor git repository"));
    assert!(calls.into_inner().is_empty());
    assert!(!nested.join(".git").exists());
    assert!(!nested.join(".empirica").exists());
}

#[test]
fn git_failure_prevents_project_initialization() {
    let temp = tempdir().expect("create temp dir");
    let calls = RefCell::new(Vec::new());

    let error = ensure_practice_with(
        &input_for(temp.path()),
        temp.path(),
        /*home*/ None,
        /*ai_id*/ None,
        |_| false,
        |command, _workspace| {
            calls.borrow_mut().push(command);
            anyhow::bail!("git unavailable")
        },
    )
    .expect_err("surface git failure");

    assert!(error.to_string() == "git unavailable");
    assert!(calls.into_inner() == vec![BootstrapCommand::GitInit]);
}

#[test]
fn home_directory_workspace_is_refused() {
    let temp = tempdir().expect("create temp dir");
    let calls = RefCell::new(Vec::new());

    let error = ensure_practice_with(
        &input_for(temp.path()),
        temp.path(),
        /*home*/ Some(temp.path()),
        /*ai_id*/ None,
        |_| false,
        recording_runner(&calls),
    )
    .expect_err("refuse home-directory workspace");

    assert!(error.to_string().contains("home directory"));
    assert!(calls.into_inner() == Vec::new());
    assert!(!temp.path().join(".git").exists());
    assert!(!temp.path().join(".empirica").exists());
}

#[test]
fn non_home_workspace_passes_home_guard() {
    let temp = tempdir().expect("create temp dir");
    let home = temp.path().join("home");
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(&home).expect("create home");
    fs::create_dir_all(&workspace).expect("create workspace");
    let calls = RefCell::new(Vec::new());

    let outcome = ensure_practice_with(
        &input_for(&workspace),
        &workspace,
        /*home*/ Some(&home),
        /*ai_id*/ None,
        |_| false,
        recording_runner(&calls),
    )
    .expect("bootstrap non-home workspace");

    assert!(outcome.changed());
}
