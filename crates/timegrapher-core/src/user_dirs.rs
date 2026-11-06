//! The user's folders the system gives each program: one for settings, one
//! for data kept between starts, one for a cache it may clear. One rule for
//! the settings, the unsaved sound, the beats cache, the photo thumbnails
//! and the applications-menu entry, so they cannot drift apart.

use std::ffi::OsString;
use std::path::PathBuf;

/// Which of the user's folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Settings (`APPDATA`; `~/Library/Application Support`;
    /// `$XDG_CONFIG_HOME` or `~/.config`).
    Config,
    /// Data kept between starts that the system doesn't clear
    /// (`LOCALAPPDATA`; `~/Library/Application Support`; `$XDG_DATA_HOME`
    /// or `~/.local/share`).
    Data,
    /// A cache the system may clear (`LOCALAPPDATA`; `~/Library/Caches`;
    /// `$XDG_CACHE_HOME` or `~/.cache`).
    Cache,
}

/// The system's folder of `kind` for this user, not yet Timegrapher's own
/// folder inside it. `None` when the variables that name it are not set.
pub fn base(kind: Kind) -> Option<PathBuf> {
    let os = if cfg!(target_os = "windows") {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::Mac
    } else {
        Os::Unix
    };
    base_from(kind, os, |k| std::env::var_os(k))
}

/// Timegrapher's own folder of `kind`: [`base`] plus `timegrapher`.
pub fn app_dir(kind: Kind) -> Option<PathBuf> {
    base(kind).map(|b| b.join("timegrapher"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Os {
    Windows,
    Mac,
    Unix,
}

/// [`base`] on `os`, reading the environment through `var`. A variable set
/// but empty is not set. An XDG variable that is not an absolute path is
/// ignored, as the XDG Base Directory spec says.
fn base_from(kind: Kind, os: Os, var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let env = |k: &str| var(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let home = || env("HOME");
    match os {
        Os::Windows => env(match kind {
            Kind::Config => "APPDATA",
            Kind::Data | Kind::Cache => "LOCALAPPDATA",
        }),
        Os::Mac => home().map(|h| match kind {
            Kind::Config | Kind::Data => h.join("Library").join("Application Support"),
            Kind::Cache => h.join("Library").join("Caches"),
        }),
        Os::Unix => {
            let (xdg, fallback): (&str, &[&str]) = match kind {
                Kind::Config => ("XDG_CONFIG_HOME", &[".config"]),
                Kind::Data => ("XDG_DATA_HOME", &[".local", "share"]),
                Kind::Cache => ("XDG_CACHE_HOME", &[".cache"]),
            };
            env(xdg)
                .filter(|p| p.is_absolute())
                .or_else(|| home().map(|h| fallback.iter().fold(h, |p, s| p.join(s))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |k| set.iter().find(|(n, _)| *n == k).map(|(_, v)| v.into())
    }

    #[test]
    fn each_system_names_its_own_folders() {
        let win = vars(&[("APPDATA", r"C:\R"), ("LOCALAPPDATA", r"C:\L")]);
        assert_eq!(
            base_from(Kind::Config, Os::Windows, &win),
            Some(r"C:\R".into())
        );
        assert_eq!(
            base_from(Kind::Data, Os::Windows, &win),
            Some(r"C:\L".into())
        );
        assert_eq!(
            base_from(Kind::Cache, Os::Windows, &win),
            Some(r"C:\L".into())
        );

        let mac = vars(&[("HOME", "/Users/p")]);
        let support = PathBuf::from("/Users/p/Library/Application Support");
        assert_eq!(
            base_from(Kind::Config, Os::Mac, &mac),
            Some(support.clone())
        );
        assert_eq!(base_from(Kind::Data, Os::Mac, &mac), Some(support));
        assert_eq!(
            base_from(Kind::Cache, Os::Mac, &mac),
            Some("/Users/p/Library/Caches".into())
        );

        let unix = vars(&[("HOME", "/home/p")]);
        assert_eq!(
            base_from(Kind::Config, Os::Unix, &unix),
            Some("/home/p/.config".into())
        );
        assert_eq!(
            base_from(Kind::Data, Os::Unix, &unix),
            Some("/home/p/.local/share".into())
        );
        assert_eq!(
            base_from(Kind::Cache, Os::Unix, &unix),
            Some("/home/p/.cache".into())
        );
    }

    #[test]
    fn an_xdg_folder_counts_only_when_absolute() {
        let set = vars(&[
            ("HOME", "/home/p"),
            ("XDG_DATA_HOME", "/d"),
            ("XDG_CACHE_HOME", "c"),
        ]);
        assert_eq!(base_from(Kind::Data, Os::Unix, &set), Some("/d".into()));
        assert_eq!(
            base_from(Kind::Cache, Os::Unix, &set),
            Some("/home/p/.cache".into())
        );
    }

    #[test]
    fn a_variable_set_but_empty_is_not_set() {
        let set = vars(&[("HOME", ""), ("XDG_CONFIG_HOME", ""), ("APPDATA", "")]);
        assert_eq!(base_from(Kind::Config, Os::Unix, &set), None);
        assert_eq!(base_from(Kind::Config, Os::Mac, &set), None);
        assert_eq!(base_from(Kind::Config, Os::Windows, &set), None);
    }
}
