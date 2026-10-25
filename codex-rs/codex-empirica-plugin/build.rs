//! Rebuild when anything under the embedded directories changes.
//!
//! `src/provision.rs` embeds these trees with `include_dir!`. That tracks the
//! bytes of files that already exist, but on stable Rust it cannot tell cargo
//! about directory membership, so a new skill or hook script would not be
//! embedded until something else forced a rebuild. Same pattern as
//! `codex-rs/skills/build.rs`.

use std::fs;
use std::path::Path;

const EMBEDDED_DIRS: [&str; 3] = ["skills", "assets/hooks_scripts", "assets/agents"];

fn main() {
    for dir in EMBEDDED_DIRS {
        let dir = Path::new(dir);
        if dir.exists() {
            println!("cargo:rerun-if-changed={}", dir.display());
            visit_dir(dir);
        }
    }
}

fn visit_dir(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        println!("cargo:rerun-if-changed={}", path.display());
        if path.is_dir() {
            visit_dir(&path);
        }
    }
}
