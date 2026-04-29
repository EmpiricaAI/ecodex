use std::fs;
use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::file_system_protected_metadata::ProtectedCreateTarget;
use crate::file_system_protected_metadata::SyntheticMountTarget;

pub(crate) type ViolationReporter = fn(&Path);

const SYNTHETIC_MOUNT_MARKER_SYNTHETIC: &[u8] = b"synthetic\n";
const SYNTHETIC_MOUNT_MARKER_EXISTING: &[u8] = b"existing\n";
const PROTECTED_CREATE_MARKER: &[u8] = b"protected-create\n";

pub(crate) struct TargetRegistration<T> {
    target: T,
    marker_file: PathBuf,
    marker_dir: PathBuf,
}

pub(crate) fn register_synthetic_mount_targets(
    targets: &[SyntheticMountTarget],
) -> Vec<TargetRegistration<SyntheticMountTarget>> {
    with_metadata_runtime_registry_lock(|| {
        targets
            .iter()
            .map(|target| {
                let marker_dir = metadata_runtime_marker_dir(target.path());
                fs::create_dir_all(&marker_dir).unwrap_or_else(|err| {
                    panic!(
                        "failed to create metadata runtime marker directory {}: {err}",
                        marker_dir.display()
                    )
                });
                let target = if target.preserves_pre_existing_path()
                    && metadata_runtime_marker_dir_has_active_synthetic_owner(&marker_dir)
                {
                    target.without_pre_existing_path()
                } else {
                    target.clone()
                };
                let marker_file = marker_dir.join(std::process::id().to_string());
                fs::write(&marker_file, synthetic_mount_marker_contents(&target)).unwrap_or_else(
                    |err| {
                        panic!(
                            "failed to register synthetic bubblewrap mount target {}: {err}",
                            target.path().display()
                        )
                    },
                );
                TargetRegistration {
                    target,
                    marker_file,
                    marker_dir,
                }
            })
            .collect()
    })
}

pub(crate) fn register_protected_create_targets(
    targets: &[ProtectedCreateTarget],
) -> Vec<TargetRegistration<ProtectedCreateTarget>> {
    with_metadata_runtime_registry_lock(|| {
        targets
            .iter()
            .map(|target| {
                let marker_dir = metadata_runtime_marker_dir(target.path());
                fs::create_dir_all(&marker_dir).unwrap_or_else(|err| {
                    panic!(
                        "failed to create protected create marker directory {}: {err}",
                        marker_dir.display()
                    )
                });
                let marker_file = marker_dir.join(std::process::id().to_string());
                fs::write(&marker_file, PROTECTED_CREATE_MARKER).unwrap_or_else(|err| {
                    panic!(
                        "failed to register protected create target {}: {err}",
                        target.path().display()
                    )
                });
                TargetRegistration {
                    target: target.clone(),
                    marker_file,
                    marker_dir,
                }
            })
            .collect()
    })
}

fn synthetic_mount_marker_contents(target: &SyntheticMountTarget) -> &'static [u8] {
    if target.preserves_pre_existing_path() {
        SYNTHETIC_MOUNT_MARKER_EXISTING
    } else {
        SYNTHETIC_MOUNT_MARKER_SYNTHETIC
    }
}

fn metadata_runtime_marker_dir_has_active_synthetic_owner(marker_dir: &Path) -> bool {
    metadata_runtime_marker_dir_has_active_process_matching(marker_dir, |path| {
        match fs::read(path) {
            Ok(contents) => contents == SYNTHETIC_MOUNT_MARKER_SYNTHETIC,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
            Err(err) => panic!(
                "failed to read metadata runtime marker {}: {err}",
                path.display()
            ),
        }
    })
}

fn metadata_runtime_marker_dir_has_active_process(marker_dir: &Path) -> bool {
    metadata_runtime_marker_dir_has_active_process_matching(marker_dir, |_| true)
}

fn metadata_runtime_marker_dir_has_active_process_matching(
    marker_dir: &Path,
    matches_marker: impl Fn(&Path) -> bool,
) -> bool {
    let entries = match fs::read_dir(marker_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return false,
        Err(err) => panic!(
            "failed to read metadata runtime marker directory {}: {err}",
            marker_dir.display()
        ),
    };
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            panic!(
                "failed to read metadata runtime marker in {}: {err}",
                marker_dir.display()
            )
        });
        let path = entry.path();
        let Some(pid) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        if !process_is_active(pid) {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => panic!(
                    "failed to remove stale metadata runtime marker {}: {err}",
                    path.display()
                ),
            }
            continue;
        }
        let matches_marker = matches_marker(&path);
        if matches_marker {
            return true;
        }
    }
    false
}

pub(crate) fn cleanup_synthetic_mount_targets(
    targets: &[TargetRegistration<SyntheticMountTarget>],
) {
    with_metadata_runtime_registry_lock(|| {
        for target in targets.iter().rev() {
            match fs::remove_file(&target.marker_file) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => panic!(
                    "failed to unregister synthetic bubblewrap mount target {}: {err}",
                    target.target.path().display()
                ),
            }
        }

        for target in targets.iter().rev() {
            if metadata_runtime_marker_dir_has_active_process(&target.marker_dir) {
                continue;
            }
            remove_synthetic_mount_target(&target.target);
            match fs::remove_dir(&target.marker_dir) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                Err(err) => panic!(
                    "failed to remove metadata runtime marker directory {}: {err}",
                    target.marker_dir.display()
                ),
            }
        }
    });
}

pub(crate) fn cleanup_protected_create_targets(
    targets: &[TargetRegistration<ProtectedCreateTarget>],
    report_violation: ViolationReporter,
) -> bool {
    with_metadata_runtime_registry_lock(|| {
        for target in targets.iter().rev() {
            match fs::remove_file(&target.marker_file) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => panic!(
                    "failed to unregister protected create target {}: {err}",
                    target.target.path().display()
                ),
            }
        }

        let mut violation = false;
        for target in targets.iter().rev() {
            if metadata_runtime_marker_dir_has_active_process(&target.marker_dir) {
                if target.target.path().exists() {
                    violation = true;
                }
                continue;
            }
            violation |= remove_protected_create_target(&target.target, report_violation);
            match fs::remove_dir(&target.marker_dir) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                Err(err) => panic!(
                    "failed to remove protected create marker directory {}: {err}",
                    target.marker_dir.display()
                ),
            }
        }
        violation
    })
}

fn remove_protected_create_target(
    target: &ProtectedCreateTarget,
    report_violation: ViolationReporter,
) -> bool {
    for attempt in 0..100 {
        match try_remove_protected_create_target(target) {
            Ok(true) => {
                report_violation(target.path());
                return true;
            }
            Ok(false) => return false,
            Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty && attempt < 99 => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(err) => {
                panic!(
                    "failed to remove protected create target {}: {err}",
                    target.path().display()
                );
            }
        }
    }
    unreachable!("protected create removal retry loop should return or panic")
}

pub(crate) fn remove_protected_create_target_best_effort(
    target: &ProtectedCreateTarget,
    report_violation: ViolationReporter,
) -> bool {
    for _ in 0..100 {
        match try_remove_protected_create_target(target) {
            Ok(true) => {
                report_violation(target.path());
                return true;
            }
            Ok(false) => return false,
            Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(_) => return true,
        }
    }
    true
}

fn try_remove_protected_create_target(target: &ProtectedCreateTarget) -> std::io::Result<bool> {
    let path = target.path();
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };

    let result = if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match result {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    }
    Ok(true)
}

fn remove_synthetic_mount_target(target: &SyntheticMountTarget) {
    let path = target.path();
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => panic!(
            "failed to inspect synthetic bubblewrap mount target {}: {err}",
            path.display()
        ),
    };
    if !target.should_remove_after_bwrap(&metadata) {
        return;
    }
    if target.is_directory() {
        match fs::remove_dir(path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
            Err(err) => panic!(
                "failed to remove synthetic bubblewrap mount target {}: {err}",
                path.display()
            ),
        }
    } else {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => panic!(
                "failed to remove synthetic bubblewrap mount target {}: {err}",
                path.display()
            ),
        }
    }
}

fn process_is_active(pid: libc::pid_t) -> bool {
    let result = unsafe { libc::kill(pid, 0) };
    if result == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error();
    !matches!(err.raw_os_error(), Some(libc::ESRCH))
}

fn with_metadata_runtime_registry_lock<T>(f: impl FnOnce() -> T) -> T {
    let registry_root = metadata_runtime_registry_root();
    fs::create_dir_all(&registry_root).unwrap_or_else(|err| {
        panic!(
            "failed to create protected metadata runtime registry {}: {err}",
            registry_root.display()
        )
    });
    let lock_path = registry_root.join("lock");
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap_or_else(|err| {
            panic!(
                "failed to open protected metadata runtime registry lock {}: {err}",
                lock_path.display()
            )
        });
    if unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX) } < 0 {
        let err = std::io::Error::last_os_error();
        panic!(
            "failed to lock protected metadata runtime registry {}: {err}",
            lock_path.display()
        );
    }
    let result = f();
    if unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_UN) } < 0 {
        let err = std::io::Error::last_os_error();
        panic!(
            "failed to unlock protected metadata runtime registry {}: {err}",
            lock_path.display()
        );
    }
    result
}

fn metadata_runtime_marker_dir(path: &Path) -> PathBuf {
    metadata_runtime_registry_root().join(format!("{:016x}", hash_path(path)))
}

fn metadata_runtime_registry_root() -> PathBuf {
    std::env::temp_dir().join("codex-bwrap-synthetic-mount-targets")
}

fn hash_path(path: &Path) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.as_os_str().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
#[path = "file_system_protected_metadata_cleanup_tests.rs"]
mod tests;
