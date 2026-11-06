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
        assert!(started.elapsed() < std::time::Duration::from_millis(40));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
