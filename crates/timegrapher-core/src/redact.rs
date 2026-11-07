//! What an issue report keeps of the computer it was made on, for the app's
//! Report an Issue and `timegrapher report` alike: the home folder, user
//! name and host name taken out of every text it holds, the OS's version,
//! and the week's logs. One copy, so a privacy fix reaches both.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

/// Removes the home folder, user name and host name from text, counting
/// what it removed.
pub struct Redactor {
    /// The home folder, as written and as JSON writes a Windows path.
    pub home: Vec<String>,
    pub user: Option<String>,
    pub host: Option<String>,
    /// Removed: home folders, user names, host names.
    pub counts: [usize; 3],
}

impl Redactor {
    /// For the computer this runs on, from its environment.
    pub fn from_env() -> Self {
        Self::from_vars(
            |k| std::env::var(k).ok(),
            || std::fs::read_to_string("/etc/hostname").ok(),
        )
    }

    /// From the environment variables `var` gives (an empty one counts as
    /// unset), on Linux and macOS or on Windows: the home folder from
    /// `HOME` or `USERPROFILE`, the user name from `USER` or `USERNAME`,
    /// and the host name from `HOSTNAME` or `COMPUTERNAME`, else from
    /// `hostname_file`, the text of `/etc/hostname`.
    fn from_vars(
        var: impl Fn(&str) -> Option<String>,
        hostname_file: impl FnOnce() -> Option<String>,
    ) -> Self {
        let var = |k: &str| var(k).filter(|v| !v.is_empty());
        let home = var("HOME").or_else(|| var("USERPROFILE"));
        let mut homes = Vec::new();
        if let Some(h) = home {
            let h = h.trim_end_matches(['/', '\\']).to_string();
            // As written, and as JSON writes a Windows path.
            if h.contains('\\') {
                homes.push(h.replace('\\', "\\\\"));
            }
            homes.push(h);
        }
        let host = var("HOSTNAME").or_else(|| var("COMPUTERNAME")).or_else(|| {
            hostname_file()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
        Redactor {
            home: homes,
            user: var("USER").or_else(|| var("USERNAME")),
            host,
            counts: [0; 3],
        }
    }

    /// `text` with the home folder written `~`, the user name `<user>`
    /// and the host name `<host>` (each a whole word).
    pub fn apply(&mut self, text: &str) -> String {
        let mut s = text.to_string();
        for h in &self.home {
            if h.len() > 1 {
                self.counts[0] += s.matches(h.as_str()).count();
                s = s.replace(h.as_str(), "~");
            }
        }
        if let Some(u) = &self.user {
            let (t, n) = replace_word(&s, u, "<user>");
            s = t;
            self.counts[1] += n;
        }
        if let Some(h) = &self.host {
            let (t, n) = replace_word(&s, h, "<host>");
            s = t;
            self.counts[2] += n;
        }
        s
    }

    /// What was removed, by kind, as the report lists it.
    pub fn summary(&self) -> Value {
        json!({
            "home_folder": self.counts[0],
            "user_name": self.counts[1],
            "host_name": self.counts[2],
        })
    }
}

/// Replace whole-word occurrences of `word` (at least two characters).
pub fn replace_word(s: &str, word: &str, with: &str) -> (String, usize) {
    if word.chars().count() < 2 {
        return (s.to_string(), 0);
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::with_capacity(s.len());
    let mut n = 0;
    let mut rest = s;
    while let Some(i) = rest.find(word) {
        let before = rest[..i].chars().next_back();
        let after = rest[i + word.len()..].chars().next();
        let whole = !before.is_some_and(is_word) && !after.is_some_and(is_word);
        out.push_str(&rest[..i]);
        if whole {
            out.push_str(with);
            n += 1;
        } else {
            out.push_str(word);
        }
        rest = &rest[i + word.len()..];
    }
    out.push_str(rest);
    (out, n)
}

/// The OS's own name for its version, where it is easy to ask.
pub fn os_version() -> Option<String> {
    if cfg!(target_os = "linux") {
        let t = std::fs::read_to_string("/etc/os-release").ok()?;
        t.lines()
            .find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .map(|v| v.trim_matches('"').to_string())
    } else if cfg!(target_os = "macos") {
        let o = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()?;
        Some(format!(
            "macOS {}",
            String::from_utf8_lossy(&o.stdout).trim()
        ))
    } else if cfg!(target_os = "windows") {
        let o = Command::new("cmd").args(["/C", "ver"]).output().ok()?;
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        None
    }
}

/// Files modified in the last week in `dir` whose names pass `keep`.
pub fn recent_files(dir: &Path, keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let week = SystemTime::now() - Duration::from_secs(7 * 86400);
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| keep(&e.file_name().to_string_lossy()))
        .filter(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|t| t >= week)
        })
        .map(|e| e.path())
        .collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_takes_whole_words() {
        let mut r = Redactor {
            home: vec!["/home/ann".into()],
            user: Some("ann".into()),
            host: Some("bench".into()),
            counts: [0; 3],
        };
        let t = r.apply("/home/ann/takes/a.flac by ann on bench; annual benchmark");
        assert_eq!(t, "~/takes/a.flac by <user> on <host>; annual benchmark");
        assert_eq!(
            r.summary(),
            json!({"home_folder": 1, "user_name": 1, "host_name": 1})
        );
    }

    #[test]
    fn windows_home_in_json() {
        let mut r = Redactor {
            home: vec!["C:\\\\Users\\\\Ann".into(), "C:\\Users\\Ann".into()],
            user: None,
            host: None,
            counts: [0; 3],
        };
        let j = serde_json::to_string(&json!({"path": "C:\\Users\\Ann\\x.wav"})).unwrap();
        assert_eq!(r.apply(&j), r#"{"path":"~\\x.wav"}"#);
    }

    #[test]
    fn a_name_inside_a_longer_word_stays() {
        assert_eq!(
            replace_word("ann's annex", "ann", "<user>"),
            ("<user>'s annex".to_string(), 1)
        );
        // A one-letter name is never taken out.
        assert_eq!(replace_word("a b", "a", "<user>").1, 0);
    }

    /// A lookup of these variables, as an environment would give them.
    fn vars(set: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let set: Vec<(String, String)> = set
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| set.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
    }

    #[test]
    fn linux_and_mac_names_come_from_their_variables() {
        let r = Redactor::from_vars(
            vars(&[
                ("HOME", "/home/ann/"),
                ("USERPROFILE", "/elsewhere"),
                ("USER", "ann"),
                ("USERNAME", "other"),
                ("HOSTNAME", "bench"),
            ]),
            || panic!("the host name is in HOSTNAME"),
        );
        // The trailing slash goes, so a path keeps its own after `~`.
        assert_eq!(r.home, ["/home/ann"]);
        assert_eq!(r.user.as_deref(), Some("ann"));
        assert_eq!(r.host.as_deref(), Some("bench"));
        assert_eq!(r.counts, [0; 3]);
    }

    #[test]
    fn windows_names_come_from_their_variables() {
        let mut r = Redactor::from_vars(
            vars(&[
                ("HOME", ""),
                ("USERPROFILE", "C:\\Users\\Ann\\"),
                ("USER", ""),
                ("USERNAME", "Ann"),
                ("COMPUTERNAME", "BENCH-PC"),
            ]),
            || None,
        );
        // As written, and with each backslash doubled as JSON writes it.
        assert_eq!(r.home, ["C:\\\\Users\\\\Ann", "C:\\Users\\Ann"]);
        assert_eq!(r.user.as_deref(), Some("Ann"));
        assert_eq!(r.host.as_deref(), Some("BENCH-PC"));
        let j = serde_json::to_string(&json!({"log": "C:\\Users\\Ann\\x.wav"})).unwrap();
        assert_eq!(
            r.apply(&format!("{j} C:\\Users\\Ann\\y.wav on BENCH-PC")),
            r#"{"log":"~\\x.wav"} ~\y.wav on <host>"#
        );
        assert_eq!(
            r.summary(),
            json!({"home_folder": 2, "user_name": 0, "host_name": 1})
        );
    }

    #[test]
    fn the_host_name_falls_back_to_its_file() {
        let host = |file: Option<&str>| {
            let file = file.map(str::to_string);
            Redactor::from_vars(vars(&[("HOSTNAME", "")]), || file).host
        };
        assert_eq!(host(Some("bench\n")).as_deref(), Some("bench"));
        assert_eq!(host(Some(" \n")), None);
        assert_eq!(host(None), None);
        // With nothing set, nothing is taken out.
        let mut r = Redactor::from_vars(vars(&[]), || None);
        assert!(r.home.is_empty() && r.user.is_none());
        assert_eq!(r.apply("/home/ann on bench"), "/home/ann on bench");
    }

    #[test]
    fn the_logs_kept_are_this_weeks_files_that_match() {
        let dir = std::env::temp_dir().join(format!("tg-redact-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("b.log")).unwrap();
        let write = |name: &str, age_days: u64| {
            let f = std::fs::File::create(dir.join(name)).unwrap();
            let t = SystemTime::now() - Duration::from_secs(age_days * 86400);
            f.set_modified(t).unwrap();
        };
        write("c.log", 0);
        write("a.log", 6);
        write("old.log", 8);
        write("notes.txt", 0);
        let kept = recent_files(&dir, |n| n.ends_with(".log"));
        // Sorted by name; not the folder named like a log, the week-old
        // file, or the one the filter turns away.
        assert_eq!(kept, [dir.join("a.log"), dir.join("c.log")]);
        std::fs::remove_dir_all(&dir).unwrap();
        // A folder that isn't there has no logs.
        assert!(recent_files(&dir, |_| true).is_empty());
    }
}
