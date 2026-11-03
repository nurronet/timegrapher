//! A save of the clock store that fails part way leaves the corrections
//! already stored as they were (#872).

#[cfg(unix)]
#[test]
fn a_failed_clock_save_keeps_the_stored_corrections() {
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("tg-clock-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args: &[&str], limited: bool| {
        let mut c = if limited {
            // Files may grow to 512 bytes; a write past that fails
            // instead of stopping the program.
            let mut c = Command::new("sh");
            c.args(["-c", "trap '' XFSZ; ulimit -f 1; exec \"$0\" \"$@\""])
                .arg(env!("CARGO_BIN_EXE_timegrapher"));
            c
        } else {
            Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        };
        c.args(args)
            .env("TIMEGRAPHER_CONFIG_DIR", &dir)
            .output()
            .unwrap()
    };
    for (device, ppm) in [
        ("Known Mic", "12.5"),
        ("Second Mic", "-3"),
        ("Third Mic", "40"),
    ] {
        let out = run(&["clock", "set", device, ppm], false);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let store = dir.join("clocks.json");
    let before = std::fs::read(&store).unwrap();
    assert!(before.len() > 512 - 64, "the store nearly fills the limit");

    let failed = run(
        &["clock", "set", "A Microphone With A Long Name", "25"],
        true,
    );
    let after = std::fs::read(&store).unwrap();
    let list = run(&["clock", "list", "--json"], false);
    let _ = std::fs::remove_dir_all(&dir);

    assert!(!failed.status.success(), "the save can't fit");
    assert_eq!(after, before, "the stored corrections are as they were");
    assert!(
        list.status.success(),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
}
