use super::*;
use crate::file_system_protected_metadata::ProtectedCreateTarget;
use crate::file_system_protected_metadata::SyntheticMountTarget;
use pretty_assertions::assert_eq;

fn ignore_metadata_violation(_: &std::path::Path) {}

#[test]
fn cleanup_synthetic_mount_targets_removes_only_empty_mount_targets() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let empty_file = temp_dir.path().join(".git");
    let empty_dir = temp_dir.path().join(".agents");
    let non_empty_file = temp_dir.path().join("non-empty");
    let missing_file = temp_dir.path().join(".missing");
    std::fs::write(&empty_file, "").expect("write empty file");
    std::fs::create_dir(&empty_dir).expect("create empty dir");
    std::fs::write(&non_empty_file, "keep").expect("write nonempty file");

    let registrations = register_synthetic_mount_targets(&[
        SyntheticMountTarget::missing(&empty_file),
        SyntheticMountTarget::missing_empty_directory(&empty_dir),
        SyntheticMountTarget::missing(&non_empty_file),
        SyntheticMountTarget::missing(&missing_file),
    ]);
    cleanup_synthetic_mount_targets(&registrations);

    assert!(!empty_file.exists());
    assert!(!empty_dir.exists());
    assert_eq!(
        std::fs::read_to_string(&non_empty_file).expect("read nonempty file"),
        "keep"
    );
    assert!(!missing_file.exists());
}

#[test]
fn cleanup_synthetic_mount_targets_waits_for_other_active_registrations() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let empty_file = temp_dir.path().join(".git");
    std::fs::write(&empty_file, "").expect("write empty file");
    let target = SyntheticMountTarget::missing(&empty_file);

    let registrations = register_synthetic_mount_targets(std::slice::from_ref(&target));
    let active_marker = registrations[0].marker_dir.join("1");
    std::fs::write(&active_marker, "").expect("write active marker");

    cleanup_synthetic_mount_targets(&registrations);
    assert!(empty_file.exists());

    std::fs::remove_file(active_marker).expect("remove active marker");
    let registrations = register_synthetic_mount_targets(std::slice::from_ref(&target));
    cleanup_synthetic_mount_targets(&registrations);

    assert!(!empty_file.exists());
}

#[test]
fn cleanup_synthetic_mount_targets_removes_transient_file_after_concurrent_owner_exits() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let empty_file = temp_dir.path().join(".git");
    let first_target = SyntheticMountTarget::missing(&empty_file);

    let first_registrations = register_synthetic_mount_targets(&[first_target]);
    std::fs::write(&empty_file, "").expect("write transient empty file");
    let active_marker = first_registrations[0].marker_dir.join("1");
    std::fs::write(&active_marker, SYNTHETIC_MOUNT_MARKER_SYNTHETIC).expect("write active marker");
    let metadata = std::fs::symlink_metadata(&empty_file).expect("stat empty file");
    let second_target = SyntheticMountTarget::existing_empty_file(&empty_file, &metadata);
    let second_registrations = register_synthetic_mount_targets(&[second_target]);

    cleanup_synthetic_mount_targets(&first_registrations);
    assert!(empty_file.exists());

    std::fs::remove_file(active_marker).expect("remove active marker");
    cleanup_synthetic_mount_targets(&second_registrations);

    assert!(!empty_file.exists());
}

#[test]
fn cleanup_synthetic_mount_targets_preserves_real_pre_existing_empty_file() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let empty_file = temp_dir.path().join(".git");
    std::fs::write(&empty_file, "").expect("write pre-existing empty file");
    let metadata = std::fs::symlink_metadata(&empty_file).expect("stat empty file");
    let first_target = SyntheticMountTarget::existing_empty_file(&empty_file, &metadata);
    let second_target = SyntheticMountTarget::existing_empty_file(&empty_file, &metadata);

    let first_registrations = register_synthetic_mount_targets(&[first_target]);
    let second_registrations = register_synthetic_mount_targets(&[second_target]);

    cleanup_synthetic_mount_targets(&first_registrations);
    cleanup_synthetic_mount_targets(&second_registrations);

    assert!(empty_file.exists());
}

#[test]
fn cleanup_protected_create_targets_removes_created_path_and_reports_violation() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let dot_git = temp_dir.path().join(".git");
    let target = ProtectedCreateTarget::missing(&dot_git);

    let registrations = register_protected_create_targets(&[target]);
    std::fs::create_dir(&dot_git).expect("create protected path");
    let violation = cleanup_protected_create_targets(&registrations, ignore_metadata_violation);

    assert!(violation);
    assert!(!dot_git.exists());
}

#[test]
fn cleanup_protected_create_targets_waits_for_other_active_registrations() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let dot_git = temp_dir.path().join(".git");
    let target = ProtectedCreateTarget::missing(&dot_git);

    let registrations = register_protected_create_targets(std::slice::from_ref(&target));
    let active_marker = registrations[0].marker_dir.join("1");
    std::fs::write(&active_marker, PROTECTED_CREATE_MARKER).expect("write active marker");
    std::fs::write(&dot_git, "").expect("create protected path");

    let violation = cleanup_protected_create_targets(&registrations, ignore_metadata_violation);
    assert!(violation);
    assert!(dot_git.exists());

    std::fs::remove_file(active_marker).expect("remove active marker");
    let registrations = register_protected_create_targets(std::slice::from_ref(&target));
    let violation = cleanup_protected_create_targets(&registrations, ignore_metadata_violation);

    assert!(violation);
    assert!(!dot_git.exists());
}
