//! Where Help > User Guide opens the manual: the copy installed with the
//! app, so it reads without an internet connection, else the copy on
//! GitHub. packaging/macos.sh, windows.sh and linux.sh put the pages that
//! packaging/docs_html.py renders where `candidates` looks.

use std::path::{Path, PathBuf};

/// The manual on GitHub, for a build that has no copy beside it (one run
/// from the source tree, or a copy someone moved out of its folder): at
/// the commit a release build was made from, so it matches the app, else
/// on main.
pub fn online() -> String {
    online_at(option_env!("TIMEGRAPHER_COMMIT").filter(|c| !c.is_empty()))
}

fn online_at(commit: Option<&str>) -> String {
    format!(
        "https://github.com/peterhudson/timegrapher/blob/{}/docs/user-guide/README.md",
        commit.unwrap_or("main")
    )
}

/// The rendered manual's contents page, inside the folder that holds it:
/// packaging/docs_html.py's GUIDE_OUT folder and its index.html (a test
/// below holds the two to each other).
const INDEX: &str = "User Guide/index.html";
/// Where macos.sh and linux.sh put the rendered pages, from the folder
/// holding the executable (tested against the scripts below).
const MAC_DOCS: &str = "../Resources/Documentation";
const LINUX_DOCS: &str = "../share/timegrapher";

/// Where an installed build's manual may be, best first, for the app at
/// `exe` and, on Linux, the AppImage file at `appimage`:
/// - macOS: the app bundle's Contents/Resources/Documentation;
/// - Windows: the download's folder, beside Timegrapher.exe;
/// - Linux: the download's folder beside the AppImage, which outlives the
///   app, then the copy inside the AppImage, in usr/share/timegrapher.
fn candidates(exe: &Path, appimage: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Some(dir) = exe.parent() else {
        return out;
    };
    if let Some(folder) = appimage.and_then(Path::parent) {
        out.push(folder.join(INDEX));
    }
    out.push(dir.join(INDEX));
    out.push(dir.join(MAC_DOCS).join(INDEX));
    out.push(dir.join(LINUX_DOCS).join(INDEX));
    out
}

/// The installed manual's contents page, if this build has one.
fn installed() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let appimage = std::env::var_os("APPIMAGE").map(PathBuf::from);
    candidates(&exe, appimage.as_deref())
        .into_iter()
        .find(|p| p.is_file())
        .and_then(|p| p.canonicalize().ok())
}

/// A file: URL for `path`, an absolute path: spaces and other bytes a URL
/// can't carry are percent-encoded, and Windows' backslashes and its
/// `\\?\` prefix become a plain file:///C:/ path.
fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = text
        .strip_prefix(r"\\?\")
        .unwrap_or(&text)
        .replace('\\', "/");
    let mut url = String::from("file://");
    if !text.starts_with('/') {
        url.push('/');
    }
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(&b) {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// What Help > User Guide opens: the installed manual, else [`online`].
pub fn url() -> String {
    installed().map_or_else(online, |p| file_url(&p))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each platform's place for the manual is looked in, the folder
    /// beside an AppImage first.
    #[test]
    fn each_install_layout_is_looked_in() {
        let mac = candidates(Path::new("/Applications/T.app/Contents/MacOS/t"), None);
        assert!(mac.contains(&PathBuf::from(
            "/Applications/T.app/Contents/MacOS/../Resources/Documentation/User Guide/index.html"
        )));
        let win = candidates(Path::new("/d/Timegrapher 1.0/Timegrapher.exe"), None);
        assert_eq!(
            win[0],
            PathBuf::from("/d/Timegrapher 1.0/User Guide/index.html")
        );
        let linux = candidates(
            Path::new("/tmp/.mount_x/usr/bin/timegrapher-app"),
            Some(Path::new("/home/u/Timegrapher 1.0/Timegrapher.AppImage")),
        );
        assert_eq!(
            linux[0],
            PathBuf::from("/home/u/Timegrapher 1.0/User Guide/index.html")
        );
        assert!(linux.contains(&PathBuf::from(
            "/tmp/.mount_x/usr/bin/../share/timegrapher/User Guide/index.html"
        )));
    }

    /// The manual is found in a bundle laid out as macos.sh lays it out.
    #[test]
    fn finds_the_manual_in_a_bundle() {
        let tmp = std::env::temp_dir().join(format!("tg-guide-{}", std::process::id()));
        let page = tmp
            .join("T.app/Contents/Resources/Documentation")
            .join(INDEX);
        std::fs::create_dir_all(page.parent().unwrap()).unwrap();
        std::fs::write(&page, "<h1>Guide</h1>").unwrap();
        let exe = tmp.join("T.app/Contents/MacOS/t");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        let found = candidates(&exe, None).into_iter().find(|p| p.is_file());
        assert_eq!(
            found.unwrap().canonicalize().unwrap(),
            page.canonicalize().unwrap()
        );
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    /// The fallback opens the manual at the commit built, else on main.
    #[test]
    fn online_copy_is_pinned_to_the_commit_built() {
        assert_eq!(
            online_at(Some("0123abc")),
            "https://github.com/peterhudson/timegrapher/blob/0123abc/docs/user-guide/README.md"
        );
        assert!(online_at(None).contains("/blob/main/docs/user-guide/README.md"));
    }

    /// The folder and page looked for are the ones packaging renders and
    /// copies: docs_html.py's GUIDE_OUT with its index.html, and the
    /// places macos.sh and linux.sh copy the pages to.
    #[test]
    fn looks_where_packaging_puts_the_manual() {
        let packaging = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging");
        let read = |f: &str| std::fs::read_to_string(packaging.join(f)).unwrap();
        let (folder, page) = INDEX.split_once('/').unwrap();
        let py = read("docs_html.py");
        assert!(
            py.contains(&format!("GUIDE_OUT = \"{folder}\"")),
            "GUIDE_OUT"
        );
        assert!(py.contains(&format!("{{GUIDE_OUT}}/{page}")), "index page");
        let mac = format!("$app/Contents/{}", MAC_DOCS.trim_start_matches("../"));
        assert!(read("macos.sh").contains(&mac), "{mac}");
        let linux = format!("$app/usr/{}", LINUX_DOCS.trim_start_matches("../"));
        assert!(read("linux.sh").contains(&linux), "{linux}");
    }

    #[test]
    fn file_urls_encode_spaces_and_windows_paths() {
        assert_eq!(
            file_url(Path::new("/Applications/T.app/User Guide/index.html")),
            "file:///Applications/T.app/User%20Guide/index.html"
        );
        assert_eq!(
            file_url(Path::new(r"\\?\C:\Timegrapher 1.0\User Guide\index.html")),
            "file:///C:/Timegrapher%201.0/User%20Guide/index.html"
        );
        assert_eq!(
            file_url(Path::new("/a/é#?.html")),
            "file:///a/%C3%A9%23%3F.html"
        );
    }
}
