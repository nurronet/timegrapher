//! The build information the CLI and the app report (`crates/build_info.rs`)
//! is that of the source the build compiled: a tracked file edited without a
//! commit builds as dirty, and undoing the edit builds as clean again, with
//! HEAD where it was (audit F3). Checked on a small crate in a git
//! repository of its own, laid out as this workspace is, built with the same
//! script.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=build test",
            "-c",
            "user.email=",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Build the crate in `dir` and run it: what it prints.
fn built(dir: &Path, target: &Path) -> String {
    let out = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--offline"])
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("cargo runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn main_rs(marker: &str) -> String {
    format!(
        "fn main() {{\n    println!(\"source={marker} dirty={{}}\", env!(\"TIMEGRAPHER_GIT_DIRTY\"));\n}}\n"
    )
}

#[test]
fn a_source_edit_without_a_commit_builds_as_dirty() {
    let root = std::env::temp_dir().join(format!("tg-build-provenance-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let repo = root.join("repo");
    let target = root.join("target");
    let krate: PathBuf = repo.join("crates").join("probe");
    std::fs::create_dir_all(krate.join("src")).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../build_info.rs"),
        repo.join("crates").join("build_info.rs"),
    )
    .unwrap();
    std::fs::write(
        repo.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/probe\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    std::fs::write(
        krate.join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(krate.join("build.rs"), "include!(\"../build_info.rs\");\n").unwrap();
    let main = krate.join("src").join("main.rs");
    std::fs::write(&main, main_rs("original")).unwrap();
    std::fs::write(repo.join(".gitignore"), "Cargo.lock\n").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "probe"]);

    let clean = built(&krate, &target);
    // A file's change is seen by its time, so let the clock move on first.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(&main, main_rs("modified")).unwrap();
    let edited = built(&krate, &target);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(&main, main_rs("original")).unwrap();
    let undone = built(&krate, &target);
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(clean, "source=original dirty=false");
    assert_eq!(edited, "source=modified dirty=true");
    assert_eq!(undone, "source=original dirty=false");
}
