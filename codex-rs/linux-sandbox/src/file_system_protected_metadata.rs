use std::fs;
use std::fs::Metadata;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    dev: u64,
    ino: u64,
}

impl FileIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SyntheticMountTarget {
    path: PathBuf,
    is_directory: bool,
    // If an empty metadata path was already present, remember its inode so
    // cleanup does not delete a real pre-existing file or directory.
    pre_existing_path: Option<FileIdentity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProtectedCreateTarget {
    path: PathBuf,
}

#[derive(Debug, Default)]
pub(crate) struct FileSystemPermissionsEnforcement {
    pub(crate) synthetic_mount_targets: Vec<SyntheticMountTarget>,
    pub(crate) protected_create_targets: Vec<ProtectedCreateTarget>,
}

impl ProtectedCreateTarget {
    pub(crate) fn missing(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl SyntheticMountTarget {
    pub(crate) fn missing(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            is_directory: false,
            pre_existing_path: None,
        }
    }

    pub(crate) fn missing_empty_directory(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            is_directory: true,
            pre_existing_path: None,
        }
    }

    pub(crate) fn existing_empty_file(path: &Path, metadata: &Metadata) -> Self {
        Self {
            path: path.to_path_buf(),
            is_directory: false,
            pre_existing_path: Some(FileIdentity::from_metadata(metadata)),
        }
    }

    pub(crate) fn existing_empty_directory(path: &Path, metadata: &Metadata) -> Self {
        Self {
            path: path.to_path_buf(),
            is_directory: true,
            pre_existing_path: Some(FileIdentity::from_metadata(metadata)),
        }
    }

    pub(crate) fn preserves_pre_existing_path(&self) -> bool {
        self.pre_existing_path.is_some()
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn is_directory(&self) -> bool {
        self.is_directory
    }

    pub(crate) fn without_pre_existing_path(&self) -> Self {
        Self {
            path: self.path.clone(),
            is_directory: self.is_directory,
            pre_existing_path: None,
        }
    }

    pub(crate) fn should_remove_after_bwrap(&self, metadata: &Metadata) -> bool {
        if self.is_directory {
            if !metadata.file_type().is_dir() {
                return false;
            }
        } else if !metadata.file_type().is_file() || metadata.len() != 0 {
            return false;
        }

        match self.pre_existing_path {
            Some(pre_existing_path) => pre_existing_path != FileIdentity::from_metadata(metadata),
            None => true,
        }
    }
}

pub(crate) fn append_protected_create_targets_for_writable_root(
    enforcement: &mut FileSystemPermissionsEnforcement,
    protected_metadata_names: &[String],
    root: &Path,
    symlink_target: Option<&Path>,
    read_only_subpaths: &[PathBuf],
) {
    for name in protected_metadata_names {
        let mut path = root.join(name);
        if let Some(target) = symlink_target
            && let Ok(relative_path) = path.strip_prefix(root)
        {
            path = target.join(relative_path);
        }
        if read_only_subpaths.iter().any(|subpath| subpath == &path) || path.exists() {
            continue;
        }
        enforcement
            .protected_create_targets
            .push(ProtectedCreateTarget::missing(&path));
    }
}

pub(crate) fn append_metadata_path_masks_for_writable_root(
    read_only_subpaths: &mut Vec<PathBuf>,
    root: &Path,
    mount_root: &Path,
    protected_metadata_names: &[String],
) {
    for name in protected_metadata_names {
        let path = root.join(name);
        if should_leave_missing_git_for_parent_repo_discovery(mount_root, name) {
            continue;
        }
        if !read_only_subpaths.iter().any(|subpath| subpath == &path) {
            read_only_subpaths.push(path);
        }
    }
}

fn should_leave_missing_git_for_parent_repo_discovery(mount_root: &Path, name: &str) -> bool {
    let path = mount_root.join(name);
    name == ".git"
        && matches!(
            path.symlink_metadata(),
            Err(err) if err.kind() == io::ErrorKind::NotFound
        )
        && mount_root
            .ancestors()
            .skip(1)
            .any(ancestor_has_git_metadata)
}

fn ancestor_has_git_metadata(ancestor: &Path) -> bool {
    let git_path = ancestor.join(".git");
    let Ok(metadata) = git_path.symlink_metadata() else {
        return false;
    };
    if metadata.is_dir() {
        return git_path.join("HEAD").symlink_metadata().is_ok();
    }
    if metadata.is_file() {
        return fs::read_to_string(git_path)
            .is_ok_and(|contents| contents.trim_start().starts_with("gitdir:"));
    }
    false
}
