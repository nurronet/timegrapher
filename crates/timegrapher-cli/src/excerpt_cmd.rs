//! `excerpt`: a stretch of a recording or take written as one bare FLAC
//! file, its samples untouched (core's `excerpt::cut` and
//! `excerpt::encode_flac`, which decodes what it wrote and checks it sample
//! for sample). For a program that keeps the sound beside a reading, such
//! as a shop's records; `report` makes a zip for an issue instead.

use crate::output;
use serde::Serialize;
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use timegrapher_core::archive::sha256_hex;
use timegrapher_core::excerpt;

pub struct Options {
    pub recording: Vec<PathBuf>,
    pub out: PathBuf,
    pub from: f64,
    pub to: Option<f64>,
    pub json: bool,
}

/// What was written.
#[derive(Debug, Serialize)]
pub struct Written {
    /// The FLAC file, as given.
    pub out: String,
    /// Where the excerpt starts and ends, seconds from the start of the
    /// recording (its segments read one after another), as cut: `to_s` is
    /// the recording's end where `--to` was past it or not given.
    pub from_s: f64,
    pub to_s: f64,
    pub duration_s: f64,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    /// The file's size and SHA-256, lowercase hex.
    pub bytes: u64,
    pub sha256: String,
}

fn check_span(from: f64, to: Option<f64>) -> Result<(), String> {
    if !from.is_finite() || from < 0.0 {
        return Err("--from must be a number of seconds, 0 or more".into());
    }
    match to {
        Some(t) if !t.is_finite() || t <= from => {
            Err("--to must be a number of seconds after --from".into())
        }
        _ => Ok(()),
    }
}

/// Writes `bytes` to `path` as a new file, refusing one already there.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => {
                format!(
                    "{} is already there; it is not written over",
                    path.display()
                )
            }
            _ => format!("{}: {e}", path.display()),
        })?;
    f.write_all(bytes).map_err(|e| {
        let _ = std::fs::remove_file(path);
        format!("{}: {e}", path.display())
    })
}

pub fn run(o: &Options) -> Result<(), String> {
    check_span(o.from, o.to)?;
    let files = crate::long::expand_dirs(&o.recording)?;
    if files.is_empty() {
        return Err("no sound files found (a take kept as its analysis alone has none)".into());
    }
    crate::long::note_join_order(&files);
    let len = o.to.map_or(f64::INFINITY, |t| t - o.from);
    let pcm = excerpt::cut(&files, o.from, len).map_err(|e| e.to_string())?;
    if pcm.frames() == 0 {
        return Err(format!("the recording ends before {} s (--from)", o.from));
    }
    let bytes = excerpt::encode_flac(&pcm).map_err(|e| e.to_string())?;
    write_new(&o.out, &bytes)?;
    let written = Written {
        out: o.out.display().to_string(),
        from_s: o.from,
        to_s: o.from + pcm.duration_s(),
        duration_s: pcm.duration_s(),
        sample_rate: pcm.sample_rate,
        channels: pcm.channels,
        bits_per_sample: pcm.bits_per_sample,
        bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
    };
    if o.json {
        let input = json!({
            "files": files.iter().map(|f| json!({"path": f.display().to_string()})).collect::<Vec<_>>(),
            "settings": {"from_s": o.from, "to_s": o.to},
        });
        output::print("excerpt", input, &written)
    } else {
        println!(
            "wrote {} ({:.1} s from {:.1} s, {} bytes)",
            written.out, written.duration_s, written.from_s, written.bytes
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_span_must_be_seconds_in_order() {
        assert!(check_span(0.0, None).is_ok());
        assert!(check_span(10.0, Some(20.0)).is_ok());
        assert!(check_span(-1.0, None).is_err());
        assert!(check_span(f64::NAN, None).is_err());
        assert!(check_span(10.0, Some(10.0)).is_err());
        assert!(check_span(10.0, Some(5.0)).is_err());
        assert!(check_span(0.0, Some(f64::INFINITY)).is_err());
    }

    #[test]
    fn an_existing_file_is_never_written_over() {
        let dir = std::env::temp_dir().join(format!("tg-excerpt-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.flac");
        write_new(&p, b"first").unwrap();
        let e = write_new(&p, b"second").unwrap_err();
        assert!(e.contains("not written over"), "{e}");
        assert_eq!(std::fs::read(&p).unwrap(), b"first");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
