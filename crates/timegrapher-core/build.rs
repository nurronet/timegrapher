//! The engine's identity for readings kept between runs
//! (`library::ENGINE`): a hash of core's own source, so a kept reading is
//! worked out again when the code that reads takes changes, even within
//! one version, and not when only the app or the command line does.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src");
    let mut files = Vec::new();
    collect(Path::new("src"), &mut files);
    files.sort();
    // FNV-1a, 64 bits: the same on every platform and toolchain.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for f in &files {
        // Forward slashes, so Windows hashes the same names.
        let name = f.to_string_lossy().replace('\\', "/");
        eat(name.as_bytes());
        eat(&[0]);
        // Line ends as written, whatever a checkout made of them.
        let text = std::fs::read(f).expect("a core source file reads");
        let text: Vec<u8> = text.into_iter().filter(|&b| b != b'\r').collect();
        eat(&text);
        eat(&[0]);
    }
    println!("cargo:rustc-env=TIMEGRAPHER_ENGINE_SOURCE={h:016x}");
}

/// Every file under `dir`.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).expect("core's source folder reads");
    for e in entries {
        let path = e.expect("a source folder entry reads").path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}
