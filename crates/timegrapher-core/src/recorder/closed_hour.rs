//! Opening a closed hour of a recording for a save, as its WAV or its
//! FLAC, without a look before the open that the FLAC thread can race.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

/// The closed hour `path`, held open, with its length: the WAV, or its
/// FLAC once it has been kept as one. The WAV is tried first and its FLAC
/// on any failure while the FLAC is there, never judged by a look before
/// the open: the thread keeping it as FLAC deletes the WAV once its FLAC
/// is in place, and between a look and an open that failed the save
/// (#1486).
pub(super) fn open_closed_hour(path: &Path) -> io::Result<(PathBuf, File, u64)> {
    let open = |p: &Path| -> io::Result<(File, u64)> {
        super::regular_file(p)?;
        let f = File::open(p)?;
        let len = f.metadata()?.len();
        Ok((f, len))
    };
    match open(path) {
        Ok((f, len)) => Ok((path.to_path_buf(), f, len)),
        Err(e) => {
            let flac = crate::flac::flac_path(path);
            if !crate::flac::is_wav(path) || !flac.is_file() {
                return Err(e);
            }
            let (f, len) = open(&flac)?;
            Ok((flac, f, len))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A closed hour is opened as its WAV, else its FLAC, and still is when
    /// the thread keeping it as FLAC deletes the WAV during the open
    /// (#1486: a save begun at the Stop failed on Windows).
    #[test]
    fn a_closed_hour_opens_though_its_wav_is_deleted_meanwhile() {
        let dir = std::env::temp_dir().join(format!("tg-closed-hour-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (wav, flac) = (dir.join("audio-001.wav"), dir.join("audio-001.flac"));
        let opened = |p: &Path| open_closed_hour(p).map(|(at, _, len)| (at, len));
        fs::write(&wav, b"wav sound").unwrap();
        assert_eq!(opened(&wav).unwrap(), (wav.clone(), 9));
        fs::write(&flac, b"flac").unwrap();
        assert_eq!(opened(&wav).unwrap(), (wav.clone(), 9));
        fs::remove_file(&wav).unwrap();
        assert_eq!(opened(&wav).unwrap(), (flac.clone(), 4));
        fs::remove_file(&flac).unwrap();
        assert_eq!(opened(&wav).unwrap_err().kind(), io::ErrorKind::NotFound);
        assert_eq!(opened(&flac).unwrap_err().kind(), io::ErrorKind::NotFound);
        // The WAV deleted while it is opened, as the FLAC thread does once
        // the FLAC is in place: every open finds one or the other.
        for _ in 0..2000 {
            fs::write(&wav, b"wav sound").unwrap();
            fs::write(&flac, b"flac").unwrap();
            let gone = std::thread::spawn({
                let wav = wav.clone();
                move || fs::remove_file(wav)
            });
            let got = opened(&wav);
            gone.join().unwrap().unwrap();
            let (at, len) = got.unwrap();
            assert_eq!(len, if at == wav { 9 } else { 4 });
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
