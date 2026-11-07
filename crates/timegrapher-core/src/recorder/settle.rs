//! Renames and removals that wait a moment for a file another program
//! has open, so a save is not refused by a passing virus scan.

use std::io;
use std::path::Path;

/// Rename `from` to `to`, waiting up to about a second while Windows says
/// another program has either open: every save that puts a file in place
/// by a rename follows the one rule (#1169, #1258).
pub fn rename_settled(from: &Path, to: &Path) -> io::Result<()> {
    settled(|| std::fs::rename(from, to), held_open)
}

/// Write the folder `dir`'s entries to the disk, so a power cut can't
/// undo a rename or a new file in it. Windows has no folder to sync; its
/// renames are written through by the file system.
pub fn sync_folder(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    std::fs::File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// Write the file at `path` to the disk, waiting the same way: it is
/// opened for writing, as Windows flushes no file opened to read, and a
/// file just written may be held by a virus scan.
pub fn sync_file(path: &Path) -> io::Result<()> {
    settled(
        || std::fs::OpenOptions::new().write(true).open(path),
        held_open,
    )?
    .sync_all()
}

/// Read `path` as text, waiting the same way: a save reads the file it
/// rewrites first, and that read is refused too while another program
/// has the file open with no sharing (Walter on #1259).
pub fn read_settled(path: &Path) -> io::Result<String> {
    settled(|| std::fs::read_to_string(path), held_open)
}

/// `op`, tried again for up to about a second while `held` says another
/// program has the file open. On Windows a virus scanner or the search
/// indexer opens a file just written, and a rename or removal of it is
/// refused until it lets go: Save Again then failed now and then (#1169).
pub(super) fn settled<T>(
    mut op: impl FnMut() -> io::Result<T>,
    held: impl Fn(&io::Error) -> bool,
) -> io::Result<T> {
    for _ in 0..20 {
        match op() {
            Err(e) if held(&e) => std::thread::sleep(std::time::Duration::from_millis(50)),
            done => return done,
        }
    }
    op()
}

/// A refusal because another program has the file open: Windows' access
/// denied, sharing violation and lock violation. Nowhere else, where the
/// same codes mean something that waiting does not change.
pub(super) fn held_open(e: &io::Error) -> bool {
    cfg!(windows) && matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_held_open_for_a_moment_is_waited_for_and_other_errors_are_not() {
        let held = |e: &io::Error| e.kind() == io::ErrorKind::WouldBlock;
        let mut tries = 0;
        let got = settled(
            || {
                tries += 1;
                match tries {
                    1..=3 => Err(io::Error::from(io::ErrorKind::WouldBlock)),
                    _ => Ok(tries),
                }
            },
            held,
        );
        assert_eq!(got.unwrap(), 4);
        let mut tries = 0;
        let got: io::Result<()> = settled(
            || {
                tries += 1;
                Err(io::Error::from(io::ErrorKind::NotFound))
            },
            held,
        );
        assert_eq!(got.unwrap_err().kind(), io::ErrorKind::NotFound);
        assert_eq!(
            tries, 1,
            "an error that waiting does not change is not tried again"
        );
        let mut tries = 0;
        let got: io::Result<()> = settled(
            || {
                tries += 1;
                Err(io::Error::from(io::ErrorKind::WouldBlock))
            },
            held,
        );
        assert!(got.is_err());
        assert_eq!(tries, 21, "a file never let go gives up");
        // Windows' held-open codes count only there; another error nowhere.
        for code in [5, 32, 33] {
            assert_eq!(
                held_open(&io::Error::from_raw_os_error(code)),
                cfg!(windows)
            );
        }
        assert!(!held_open(&io::Error::from_raw_os_error(2)));
    }

    #[test]
    fn a_settled_rename_puts_the_file_in_place_and_a_missing_one_fails_at_once() {
        let dir = std::env::temp_dir().join(format!("tg-settle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (from, to) = (dir.join("a.saving"), dir.join("a"));
        std::fs::write(&from, "new").unwrap();
        std::fs::write(&to, "old").unwrap();
        rename_settled(&from, &to).unwrap();
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "new");
        let started = std::time::Instant::now();
        let missing = rename_settled(&from, &to).unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_settled_read_gives_the_text_and_a_missing_file_fails_at_once() {
        let dir = std::env::temp_dir().join(format!("tg-settle-read-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("session.toml");
        std::fs::write(&file, "name = \"a\"\n").unwrap();
        assert_eq!(read_settled(&file).unwrap(), "name = \"a\"\n");
        let started = std::time::Instant::now();
        let missing = read_settled(&dir.join("gone")).unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
