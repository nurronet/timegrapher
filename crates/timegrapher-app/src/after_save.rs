//! Settings > General > Run After Saving a Take: a command of the user's
//! own, run once a take is saved with listening stopped, so another program
//! (a shop's records, a backup) can pick the take up. The take's folder is
//! passed as the command's last argument and as `TIMEGRAPHER_TAKE`. The
//! app doesn't wait for it: what it exits with goes to the app's log, and
//! only there. It may outlive the app.

use std::path::Path;
use std::process::{Command, Stdio};

/// The environment variable that holds the saved take's folder.
pub const TAKE_VAR: &str = "TIMEGRAPHER_TAKE";

/// The command as the system's shell runs it, with `take` added as its
/// last argument (quoted) and in [`TAKE_VAR`]. `None` for a blank command.
pub fn command(line: &str, take: &Path) -> Option<Command> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let mut c = shell(line, take);
    c.env(TAKE_VAR, take)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Some(c)
}

#[cfg(not(windows))]
fn shell(line: &str, take: &Path) -> Command {
    // `"$1"` is the folder, passed as an argument of its own, so no
    // character in its name is read by the shell.
    let mut c = Command::new("/bin/sh");
    c.arg("-c")
        .arg(format!("{line} \"$1\""))
        .arg("timegrapher")
        .arg(take);
    c
}

#[cfg(windows)]
fn shell(line: &str, take: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    // A Windows path can't hold a double quote, so quoting it is enough.
    // The whole line goes in one more pair of quotes, which `/S` has cmd
    // take off, so a command that starts with a quoted program path keeps
    // its own quotes (Trevor on #1478). CREATE_NO_WINDOW: no console window
    // flashes up beside the app.
    let mut c = Command::new("cmd");
    c.raw_arg(format!("/S /C \"{line} \"{}\"\"", take.display()))
        .creation_flags(0x0800_0000);
    c
}

/// Start `line` for `take`, and log what it exits with once it does. An
/// error when it can't be started; `Ok(false)` for a blank command.
pub fn run(line: &str, take: &Path) -> std::io::Result<bool> {
    let Some(mut c) = command(line, take) else {
        return Ok(false);
    };
    let mut child = c.spawn()?;
    let what = format!("the command run after saving {}", take.display());
    crate::applog::info(&format!("started {what}"));
    std::thread::spawn(move || match child.wait() {
        Ok(s) if s.success() => crate::applog::info(&format!("{what} finished")),
        Ok(s) => crate::applog::info(&format!("{what} ended with {s}")),
        Err(e) => crate::applog::info(&format!("{what}: {e}")),
    });
    Ok(true)
}

#[cfg(test)]
fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("tg-after-save-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    /// The usual Windows setting: a program under a folder with a space in
    /// its name, quoted, with an argument of its own, on a take folder with
    /// spaces in its name.
    #[test]
    fn a_quoted_program_gets_the_take_as_its_last_argument() {
        let dir = scratch("win");
        let bin = dir.join("Program Files").join("Shop");
        std::fs::create_dir_all(&bin).unwrap();
        let script = bin.join("import.cmd");
        std::fs::write(
            &script,
            "@echo off\r\n> \"%~dp0out.txt\" echo %~1^|%~2^|%TIMEGRAPHER_TAKE%\r\n",
        )
        .unwrap();
        let take = dir.join("Watches").join("2026-11-12 10.00 Dial Up");
        std::fs::create_dir_all(&take).unwrap();
        let line = format!("\"{}\" first", script.display());
        let status = command(&line, &take).unwrap().status().unwrap();
        assert!(status.success(), "{status}");
        let got = std::fs::read_to_string(bin.join("out.txt")).unwrap();
        let t = take.display();
        assert_eq!(got.trim_end(), format!("first|{t}|{t}"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn a_blank_command_runs_nothing() {
        assert!(command("  ", Path::new("/x")).is_none());
        assert!(!run("", Path::new("/x")).unwrap());
    }

    #[test]
    fn the_take_is_the_last_argument_and_in_the_environment() {
        let dir = scratch("args");
        // A folder name the shell would otherwise split or expand.
        let take = dir.join("2026-11-12 10.00 Dial Up $HOME; *");
        std::fs::create_dir_all(&take).unwrap();
        let out = dir.join("out.txt");
        let line = format!(
            "printf '%s|%s|%s' \"$TIMEGRAPHER_TAKE\" first > '{}'; printf '|%s'",
            out.display()
        );
        let status = command(&line, &take).unwrap().status().unwrap();
        assert!(status.success());
        let got = std::fs::read_to_string(&out).unwrap();
        assert_eq!(got, format!("{}|first|", take.display()));
        // The folder as the last argument, untouched.
        let line = format!("printf '%s' > '{}'", out.display());
        assert!(command(&line, &take).unwrap().status().unwrap().success());
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            take.display().to_string()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
