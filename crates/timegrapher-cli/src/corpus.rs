//! The public test corpus (github.com/peterhudson/timegrapher-corpus) as
//! `regress` reads it: a checkout holds `corpus.json` and each take's
//! metadata under `takes/<take>/`; the audio is attached to a release per
//! take. `--fetch` downloads what the run needs and checks it by SHA-256;
//! `--smoke` runs the takes that have no tg reference through the engine,
//! which passes unless the engine panics, or hears a watch in a take
//! recorded without one. See docs/regress.md.

use crate::archive::sha256_hex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use timegrapher_core::analysis::{self, AnalysisConfig};
use timegrapher_core::audio;
use timegrapher_core::library::fixed_text;
use timegrapher_core::stream::StreamConfig;
use timegrapher_core::take::{self, EntrySettings};

/// The repository the release assets live in.
pub const REPO: &str = "peterhudson/timegrapher-corpus";
pub const INDEX: &str = "corpus.json";
/// The tier of takes the engine is never tuned on (see the corpus README).
pub const HOLDOUT: &str = "holdout";

/// Smoke takes recorded with no watch on the stand: `analyze` must say no
/// watch is heard in each of their files. Their readings are not compared.
pub const NO_WATCH: &[&str] = &["silence_empty_stand"];

#[derive(Debug, Deserialize)]
pub struct Corpus {
    pub takes: Vec<Take>,
}

#[derive(Debug, Deserialize)]
pub struct Take {
    pub id: String,
    /// "smoke", "referenced", "paired" or "holdout".
    pub tier: String,
    pub release: String,
    pub files: Vec<File>,
}

#[derive(Debug, Deserialize)]
pub struct File {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

/// The corpus index in `root`, if `root` is a corpus checkout. An index
/// naming a take, release or file that isn't one plain name is refused
/// whole, before anything is read or downloaded (#1325).
pub fn load(root: &Path) -> Result<Option<Corpus>, String> {
    let p = root.join(INDEX);
    match std::fs::read_to_string(&p) {
        Ok(t) => {
            let c: Corpus =
                serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))?;
            check(&c).map_err(|e| format!("{}: {e}", p.display()))?;
            Ok(Some(c))
        }
        Err(_) => Ok(None),
    }
}

/// The folder of a corpus checkout that holds the takes.
pub fn takes_dir(root: &Path) -> PathBuf {
    root.join("takes")
}

/// Whether `s` is one plain name that stays where it is joined: not
/// empty, no folder separator or drive, not hidden or `..`.
fn plain_name(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('.') && !s.contains(['/', '\\', ':', '\0'])
}

/// Every take id, release and file name in `c` is a plain name, so a
/// download lands inside the checkout and comes from the corpus's own
/// releases.
fn check(c: &Corpus) -> Result<(), String> {
    for t in &c.takes {
        if !plain_name(&t.id) {
            return Err(format!("unsafe take id {:?}", t.id));
        }
        if !plain_name(&t.release) {
            return Err(format!("{}: unsafe release {:?}", t.id, t.release));
        }
        if let Some(f) = t.files.iter().find(|f| !plain_name(&f.name)) {
            return Err(format!("{}: unsafe file name {:?}", t.id, f.name));
        }
    }
    Ok(())
}

fn picked(id: &str, filter: &[String]) -> bool {
    filter.is_empty() || filter.iter().any(|t| id.contains(t.as_str()))
}

/// Whether a take has a session file for `regress` to read.
fn has_session(dir: &Path) -> bool {
    fn walk(d: &Path, depth: usize) -> bool {
        if d.join(crate::session_cmd::MANIFEST).is_file() {
            return true;
        }
        depth > 0
            && std::fs::read_dir(d).is_ok_and(|rd| {
                rd.flatten()
                    .any(|e| e.path().is_dir() && walk(&e.path(), depth - 1))
            })
    }
    walk(dir, 2)
}

/// The takes of the holdout tier, read only by `regress --holdout`.
pub fn holdout_ids(c: &Corpus) -> std::collections::BTreeSet<&str> {
    c.takes
        .iter()
        .filter(|t| t.tier == HOLDOUT)
        .map(|t| t.id.as_str())
        .collect()
}

/// A digest actually verified by this invocation's existing fetch pass.
#[derive(Debug, Clone)]
pub struct VerifiedFile {
    /// Resolved during verification; never resolve this historical evidence later.
    pub canonical_path: Option<PathBuf>,
    pub bytes: u64,
    pub sha256: String,
}

/// Download the audio of every take this run reads (those with a session
/// file, and the smoke takes with `smoke`) that is missing or changed.
/// With `holdout` only the holdout takes are read, and only then: their
/// names and sizes are not printed.
pub fn fetch(
    root: &Path,
    c: &Corpus,
    filter: &[String],
    smoke: bool,
    holdout: bool,
    quiet: bool,
) -> Result<Vec<VerifiedFile>, String> {
    let mut verified = Vec::new();
    let mut got = 0u64;
    let quiet = quiet || holdout;
    for t in &c.takes {
        let dir = takes_dir(root).join(&t.id);
        let wanted = picked(&t.id, filter)
            && (t.tier == HOLDOUT) == holdout
            && (has_session(&dir) || (smoke && t.tier == "smoke"));
        if !wanted {
            continue;
        }
        for f in &t.files {
            let path = dir.join(&f.name);
            if let Ok(data) = std::fs::read(&path) {
                if sha256_hex(&data) == f.sha256 {
                    verified.push(VerifiedFile {
                        canonical_path: path.canonicalize().ok(),
                        bytes: data.len() as u64,
                        sha256: f.sha256.clone(),
                    });
                    continue;
                }
            }
            if !quiet {
                eprintln!(
                    "fetching {}/{} ({} MB)",
                    t.id,
                    f.name,
                    fixed_text(f.bytes as f64 / 1e6, 0)
                );
            }
            let url = format!(
                "https://github.com/{REPO}/releases/download/{}/{}",
                t.release, f.name
            );
            let part = dir.join(format!("{}.part", f.name));
            let status = Command::new("curl")
                .args(["-fsSL", "--retry", "3", "-o"])
                .arg(&part)
                .arg(&url)
                .status()
                .map_err(|e| format!("curl: {e} (curl downloads the corpus audio)"))?;
            if !status.success() {
                let _ = std::fs::remove_file(&part);
                return Err(format!("{}/{}: download failed ({status})", t.id, f.name));
            }
            let data = std::fs::read(&part).map_err(|e| e.to_string())?;
            if sha256_hex(&data) != f.sha256 {
                let _ = std::fs::remove_file(&part);
                return Err(format!(
                    "{}/{}: SHA-256 does not match {INDEX}",
                    t.id, f.name
                ));
            }
            std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
            got += data.len() as u64;
            verified.push(VerifiedFile {
                canonical_path: path.canonicalize().ok(),
                bytes: data.len() as u64,
                sha256: f.sha256.clone(),
            });
        }
    }
    if got > 0 {
        eprintln!(
            "fetched {} MB of corpus audio",
            fixed_text(got as f64 / 1e6, 0)
        );
    }
    Ok(verified)
}

/// What the engine did with one smoke recording.
#[derive(Debug, Clone, Serialize)]
pub struct Smoke {
    pub take: String,
    /// File name; cleared with --quiet.
    pub file: String,
    /// "ran", "stopped" (an error message, which is fine) or "panicked".
    pub outcome: &'static str,
    pub beats: Option<usize>,
    pub detail: Option<String>,
    /// `analyze`'s `watch_heard` for the file; none if it could not run.
    pub watch_heard: Option<bool>,
    /// Whether the take was recorded with no watch (see [`NO_WATCH`]).
    pub no_watch: bool,
}

impl Smoke {
    pub fn fails(&self) -> bool {
        self.outcome == "panicked" || (self.no_watch && self.watch_heard != Some(false))
    }
}

/// Run every smoke take's files through the stream analysis.
pub fn smoke(root: &Path, c: &Corpus, filter: &[String]) -> Vec<Smoke> {
    let mut out = Vec::new();
    for t in c
        .takes
        .iter()
        .filter(|t| t.tier == "smoke" && picked(&t.id, filter))
    {
        for f in &t.files {
            let path = takes_dir(root).join(&t.id).join(&f.name);
            // Opened as a recording with no session file is: through core
            // `take`, on no clock but the card's (#514).
            let settings = EntrySettings {
                stream: StreamConfig::default(),
                card_clock: true,
                settle_s: None,
            };
            let r = std::panic::catch_unwind(|| {
                take::open_files(
                    std::slice::from_ref(&path),
                    None,
                    &settings,
                    |_| Ok(None),
                    |_| Ok(None),
                    |_, _| {},
                )
            });
            let (outcome, beats, detail) = match r {
                Ok(Ok(opened)) => ("ran", Some(opened.log.beats.len()), None),
                Ok(Err(e)) => ("stopped", None, Some(e)),
                Err(p) => {
                    let msg = p
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
                    ("panicked", None, msg)
                }
            };
            let watch_heard = std::panic::catch_unwind(|| {
                audio::load(&path).ok().map(|a| {
                    analysis::analyze(&a, &AnalysisConfig::default())
                        .summary
                        .watch_heard
                })
            })
            .ok()
            .flatten();
            out.push(Smoke {
                take: t.id.clone(),
                file: f.name.clone(),
                outcome,
                beats,
                detail,
                watch_heard,
                no_watch: NO_WATCH.contains(&t.id.as_str()),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smoke(outcome: &'static str, watch_heard: Option<bool>, no_watch: bool) -> Smoke {
        Smoke {
            take: "t".into(),
            file: "f.flac".into(),
            outcome,
            beats: None,
            detail: None,
            watch_heard,
            no_watch,
        }
    }

    /// An index whose take id, release or file name would reach outside
    /// the checkout or the corpus's releases is refused as it is read,
    /// before anything is downloaded or written (#1325).
    #[test]
    fn an_index_naming_a_path_is_refused_whole() {
        let dir = std::env::temp_dir().join(format!("tg-corpus-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("corpus/takes")).unwrap();
        let outside = dir.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("take.flac"), "untouched").unwrap();
        let index = |id: &str, release: &str, name: &str| {
            serde_json::json!({"takes": [{"id": id, "tier": "smoke", "release": release,
                "files": [{"name": name, "bytes": 1, "sha256": "00"}]}]})
            .to_string()
        };
        let root = dir.join("corpus");
        std::fs::write(root.join(INDEX), index("daytona", "daytona", "take.flac")).unwrap();
        assert!(load(&root).unwrap().is_some());
        for (id, release, name) in [
            ("../../outside", "daytona", "take.flac"),
            ("/abs", "daytona", "take.flac"),
            ("a\\b", "daytona", "take.flac"),
            ("..", "daytona", "take.flac"),
            ("", "daytona", "take.flac"),
            ("C:x", "daytona", "take.flac"),
            ("daytona", "../../other/releases/download/x", "take.flac"),
            ("daytona", "..", "take.flac"),
            ("daytona", "daytona", "../take.flac"),
            ("daytona", "daytona", ".hidden"),
        ] {
            std::fs::write(root.join(INDEX), index(id, release, name)).unwrap();
            let err = load(&root).unwrap_err();
            assert!(err.contains("unsafe"), "{id} {release} {name}: {err}");
        }
        assert_eq!(
            std::fs::read_to_string(outside.join("take.flac")).unwrap(),
            "untouched"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// One unsafe name anywhere refuses the whole index: in a later take,
    /// in a later file of a take, or as a NUL inside a name.
    #[test]
    fn a_path_in_any_take_or_file_refuses_the_index() {
        let dir = std::env::temp_dir().join(format!("tg-corpus-any-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("corpus");
        std::fs::create_dir_all(&root).unwrap();
        let take = |id: &str, names: &[&str]| {
            let files: Vec<_> = names
                .iter()
                .map(|n| serde_json::json!({"name": n, "bytes": 1, "sha256": "00"}))
                .collect();
            serde_json::json!({"id": id, "tier": "smoke", "release": id, "files": files})
        };
        let write = |takes: Vec<serde_json::Value>| {
            let index = serde_json::json!({ "takes": takes }).to_string();
            std::fs::write(root.join(INDEX), index).unwrap();
        };
        write(vec![
            take("a", &["1.flac", "2.flac"]),
            take("b", &["1.flac"]),
        ]);
        assert!(load(&root).unwrap().is_some());
        for takes in [
            vec![take("a", &["1.flac"]), take("../b", &["1.flac"])],
            vec![take("a", &["1.flac"]), take("b", &["1.flac", "../2.flac"])],
            vec![take("a", &["1.flac", "2\0.flac"])],
            vec![take("a\0b", &["1.flac"])],
        ] {
            write(takes);
            let err = load(&root).unwrap_err();
            assert!(err.contains("unsafe"), "{err}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_watch_heard_on_an_empty_stand_fails() {
        assert!(!smoke("ran", Some(false), true).fails());
        assert!(smoke("ran", Some(true), true).fails());
        assert!(smoke("ran", None, true).fails());
        assert!(!smoke("ran", Some(true), false).fails());
        assert!(!smoke("stopped", None, false).fails());
        assert!(smoke("panicked", None, false).fails());
    }
}
