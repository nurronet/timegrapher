//! Renames and removals that wait a moment for a file another program
//! has open, so a save is not refused by a passing virus scan.

use std::io;

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
        assert!(!held_open(&io::Error::from_raw_os_error(2)));
    }
}
