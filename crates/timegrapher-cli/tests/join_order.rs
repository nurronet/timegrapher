//! The command line says, as the app does, when the files it reads as one
//! recording were written in another order than they are joined in (#1203).

use std::process::Command;

#[test]
fn files_joined_out_of_their_written_order_are_noted() {
    let dir = std::env::temp_dir().join(format!("tg-cli-join-order-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = timegrapher_core::audio::Audio {
        samples: vec![0.0; 8000],
        sample_rate: 8000,
    };
    for name in ["a.wav", "b.wav"] {
        timegrapher_core::audio::write_wav(&dir.join(name), &a).unwrap();
    }
    std::fs::File::options()
        .write(true)
        .open(dir.join("b.wav"))
        .unwrap()
        .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3600))
        .unwrap();
    let stderr = |files: [&str; 2]| {
        let run = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
            .arg("long")
            .args(files.map(|f| dir.join(f)))
            .output()
            .unwrap();
        String::from_utf8_lossy(&run.stderr).into_owned()
    };
    let note = "note: b.wav was written before a.wav, which comes ahead of it: check the \
                order the files are joined in.";
    assert!(stderr(["a.wav", "b.wav"]).contains(note));
    assert!(!stderr(["b.wav", "a.wav"]).contains("check the order"));
    std::fs::remove_dir_all(&dir).ok();
}
