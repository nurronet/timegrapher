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

/// Check the crate in `dir` (`cargo check`: its build script runs, as in
/// any build, but nothing is compiled to code or linked, #907) and give the
/// dirty flag its build script handed the compiler.
fn checked(dir: &Path, target: &Path) -> String {
    let out = Command::new(env!("CARGO"))
        .args(["check", "--quiet", "--offline"])
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
    // The build script's run writes what it printed to a file under
    // build/: `<run>/output` before Cargo 1.100, `<crate>/<hash>/run/stdout`
    // since (its new build-dir layout); the one probe crate has one such run.
    let build = target.join("debug").join("build");
    let dirs = |d: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(d)
            .map(|r| r.filter_map(|e| Some(e.ok()?.path())).collect())
            .unwrap_or_default()
    };
    let outputs: Vec<PathBuf> = dirs(&build)
        .iter()
        .flat_map(|run| {
            let mut found = vec![run.join("output")];
            found.extend(dirs(run).iter().map(|h| h.join("run").join("stdout")));
            found
        })
        .filter(|p| p.is_file())
        .collect();
    assert_eq!(outputs.len(), 1, "{outputs:?}");
    let printed = std::fs::read_to_string(&outputs[0]).unwrap();
    printed
        .lines()
        .find_map(|l| l.strip_prefix("cargo:rustc-env=TIMEGRAPHER_GIT_DIRTY="))
        .unwrap_or_else(|| panic!("no dirty flag in {printed}"))
        .to_string()
}

/// The probe's program: it uses the flag, so a build without it fails.
const MAIN_RS: &str =
    "fn main() {\n    println!(\"dirty={}\", env!(\"TIMEGRAPHER_GIT_DIRTY\"));\n}\n";

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
    std::fs::write(&main, MAIN_RS).unwrap();
    std::fs::write(repo.join(".gitignore"), "Cargo.lock\n").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "probe"]);

    let clean = checked(&krate, &target);
    // A file's change is seen by its time, so let the clock move on first.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(&main, format!("{MAIN_RS}// edited\n")).unwrap();
    let edited = checked(&krate, &target);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(&main, MAIN_RS).unwrap();
    let undone = checked(&krate, &target);
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(clean, "false");
    assert_eq!(edited, "true");
    assert_eq!(undone, "false");
}
