//! FLAC written a block at a time, and a WAV turned into FLAC that way
//! (#503). Memory stays at a few blocks however long the sound is, so an
//! hour's part can be converted beside a running recording.

use crate::audio::AudioError;
use flacenc::component::{BitRepr, Stream};
use flacenc::error::Verify;
use flacenc::source::{Context, Fill, FrameBuf};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// The shape of the samples a FLAC stream holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
}

fn failed(e: impl std::fmt::Debug) -> AudioError {
    AudioError::Unsupported(format!("FLAC encoder: {e:?}"))
}

/// "fLaC" and the stream info block, as `stream` (with no frames) gives
/// them: the same length whatever the figures in it.
fn header(stream: &Stream) -> Result<Vec<u8>, AudioError> {
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).map_err(failed)?;
    Ok(sink.as_slice().to_vec())
}

/// Write a FLAC stream of `format` to `out`, from interleaved samples
/// that `next` hands over a block at a time: it is given an empty buffer
/// and the most samples to put in it (a whole number of frames), and
/// leaves it empty at the end. The stream info (length, MD5, frame sizes)
/// is written last, over a placeholder of the same length. Returns the
/// frames (samples per channel) written.
pub fn encode<W: Write + Seek>(
    format: Format,
    out: &mut W,
    mut next: impl FnMut(&mut Vec<i32>, usize) -> Result<(), AudioError>,
) -> Result<u64, AudioError> {
    let ch = format.channels as usize;
    let bps = format.bits_per_sample as usize;
    let unsupported = |what: String| Err(AudioError::Unsupported(what));
    if !(4..=24).contains(&bps) {
        return unsupported(format!("{bps}-bit samples"));
    }
    if !(1..=8).contains(&ch) {
        return unsupported(format!("{ch} channels of {bps}-bit samples"));
    }
    if !(1..=655_350).contains(&format.sample_rate) {
        return unsupported(format!("sample rate {}", format.sample_rate));
    }
    let cfg = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, e)| failed(e))?;
    let block = cfg.block_size;
    let mut stream = Stream::new(format.sample_rate as usize, ch, bps).map_err(failed)?;
    let start = out.stream_position().map_err(AudioError::Io)?;
    out.write_all(&header(&stream)?).map_err(AudioError::Io)?;
    let mut fill = (
        FrameBuf::with_size(ch, block).map_err(failed)?,
        Context::new(bps, ch),
    );
    let mut buf: Vec<i32> = Vec::with_capacity(block * ch);
    let mut frames = 0u64;
    loop {
        buf.clear();
        next(&mut buf, block * ch)?;
        if buf.is_empty() {
            break;
        }
        if buf.len() > block * ch || !buf.len().is_multiple_of(ch) {
            return unsupported(format!("a block of {} samples", buf.len()));
        }
        fill.fill_interleaved(&buf).map_err(failed)?;
        let n = fill.1.current_frame_number().unwrap_or(0);
        let frame = flacenc::encode_fixed_size_frame(&cfg, &fill.0, n, stream.stream_info())
            .map_err(failed)?;
        stream.stream_info_mut().update_frame_info(&frame);
        let mut sink = flacenc::bitsink::ByteSink::new();
        frame.write(&mut sink).map_err(failed)?;
        out.write_all(sink.as_slice()).map_err(AudioError::Io)?;
        frames += (buf.len() / ch) as u64;
    }
    let info = stream.stream_info_mut();
    info.set_md5_digest(&fill.1.md5_digest());
    info.set_total_samples(frames as usize);
    // flacenc counts a short last block in the minimum block size, which
    // can then be under 16, and claxon (rightly) refuses such a stream. A
    // fixed-block stream names its block size as both, as `flac` writes it.
    info.set_block_sizes(block, block).map_err(failed)?;
    if frames == 0 {
        info.set_frame_sizes(0, 0).map_err(failed)?;
    }
    let end = out.stream_position().map_err(AudioError::Io)?;
    out.seek(SeekFrom::Start(start)).map_err(AudioError::Io)?;
    out.write_all(&header(&stream)?).map_err(AudioError::Io)?;
    out.seek(SeekFrom::Start(end)).map_err(AudioError::Io)?;
    Ok(frames)
}

/// The FLAC file `wav` becomes: the same name with `.flac`.
pub fn flac_path(wav: &Path) -> PathBuf {
    wav.with_extension("flac")
}

/// Encode the integer WAV `wav` as the FLAC file `flac`, a block at a
/// time, then check that it decodes to exactly the WAV's samples. The
/// FLAC is written under a temporary name and takes its own only once it
/// checks, so a FLAC at `flac` is always a whole, checked one. The WAV is
/// left as it is. Returns the frames converted.
pub fn encode_wav(wav: &Path, flac: &Path) -> Result<u64, AudioError> {
    let mut reader = hound::WavReader::open(wav).map_err(AudioError::Wav)?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int {
        return Err(AudioError::Unsupported("floating-point WAV".into()));
    }
    let format = Format {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        bits_per_sample: spec.bits_per_sample,
    };
    let part = flac.with_extension("flac.part");
    let written = (|| {
        let mut out =
            std::io::BufWriter::new(std::fs::File::create(&part).map_err(AudioError::Io)?);
        let mut samples = reader.samples::<i32>();
        let frames = encode(format, &mut out, |buf, most| {
            for s in samples.by_ref().take(most) {
                buf.push(s.map_err(AudioError::Wav)?);
            }
            Ok(())
        })?;
        out.into_inner()
            .map_err(|e| AudioError::Io(e.into_error()))?
            .sync_all()
            .map_err(AudioError::Io)?;
        if !same_samples(wav, &part)? {
            return Err(failed("the FLAC does not decode to the WAV's samples"));
        }
        std::fs::rename(&part, flac).map_err(AudioError::Io)?;
        Ok(frames)
    })();
    if written.is_err() {
        std::fs::remove_file(&part).ok();
    }
    written
}

/// Keep the WAV `wav` as FLAC: [`encode_wav`] beside it under the same
/// name, then delete the WAV once the FLAC checks. Readers take the FLAC
/// over a WAV of the same name ([`crate::recording_order::recordings_in`]),
/// so at no moment is the sound missing or there twice. Returns the FLAC.
pub fn keep_as_flac(wav: &Path) -> Result<PathBuf, AudioError> {
    let flac = flac_path(wav);
    encode_wav(wav, &flac)?;
    std::fs::remove_file(wav).map_err(AudioError::Io)?;
    Ok(flac)
}

/// Whether the integer WAV `wav` and the FLAC `flac` hold the same
/// samples, in the same format, read side by side a sample at a time.
pub fn same_samples(wav: &Path, flac: &Path) -> Result<bool, AudioError> {
    let mut w = hound::WavReader::open(wav).map_err(AudioError::Wav)?;
    let mut f = claxon::FlacReader::open(flac).map_err(AudioError::Flac)?;
    let (ws, fs) = (w.spec(), f.streaminfo());
    if ws.sample_format != hound::SampleFormat::Int
        || ws.sample_rate != fs.sample_rate
        || u32::from(ws.channels) != fs.channels
        || u32::from(ws.bits_per_sample) != fs.bits_per_sample
    {
        return Ok(false);
    }
    let mut back = f.samples();
    for s in w.samples::<i32>() {
        let s = s.map_err(AudioError::Wav)?;
        let Some(v) = back.next() else {
            return Ok(false);
        };
        if v.map_err(AudioError::Flac)? != s {
            return Ok(false);
        }
    }
    Ok(back.next().is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tg-flac-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A WAV of `frames` frames of `channels` channels at `bits`, every
    /// sample different and reaching both ends of the range.
    fn wav(path: &Path, frames: usize, channels: u16, bits: u16) -> Vec<i32> {
        let spec = hound::WavSpec {
            channels,
            sample_rate: 48000,
            bits_per_sample: bits,
            sample_format: hound::SampleFormat::Int,
        };
        let top = (1i64 << (bits - 1)) - 1;
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        let samples: Vec<i32> = (0..frames * channels as usize)
            .map(|i| {
                let x = (i as i64 * 7919 + (i as i64 * i as i64) % 1013) % (2 * top + 2) - top - 1;
                x as i32
            })
            .collect();
        for &s in &samples {
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();
        samples
    }

    /// The bytes flacenc writes for `samples` encoding them whole, with
    /// the block sizes a fixed-block stream names.
    fn whole_stream(samples: &[i32], channels: u16, bits: u16) -> Vec<u8> {
        let cfg = flacenc::config::Encoder::default().into_verified().unwrap();
        let src = flacenc::source::MemSource::from_samples(
            samples,
            channels as usize,
            bits as usize,
            48000,
        );
        let mut stream = flacenc::encode_with_fixed_block_size(&cfg, src, cfg.block_size).unwrap();
        stream
            .stream_info_mut()
            .set_block_sizes(cfg.block_size, cfg.block_size)
            .unwrap();
        let mut sink = flacenc::bitsink::ByteSink::new();
        stream.write(&mut sink).unwrap();
        sink.as_slice().to_vec()
    }

    #[test]
    fn a_wav_becomes_a_flac_of_the_same_samples() {
        let dir = scratch("same");
        // Mono and stereo, 16 and 24 bits, a length that ends in a short
        // block, one under a block, and none at all.
        for (frames, channels, bits) in [(10_000, 1, 16), (9_000, 2, 24), (100, 1, 16), (0, 1, 16)]
        {
            let w = dir.join(format!("a-{frames}-{channels}-{bits}.wav"));
            let samples = wav(&w, frames, channels, bits);
            let f = flac_path(&w);
            assert_eq!(encode_wav(&w, &f).unwrap(), frames as u64);
            assert!(same_samples(&w, &f).unwrap());
            assert!(!f.with_extension("flac.part").exists());
            let mut r = claxon::FlacReader::open(&f).unwrap();
            let info = r.streaminfo();
            // FLAC writes a length of 0 as "unknown".
            assert_eq!(info.samples, (frames > 0).then_some(frames as u64));
            assert_eq!(
                (info.channels, info.bits_per_sample),
                (channels as u32, bits as u32)
            );
            let back: Vec<i32> = r.samples().map(Result::unwrap).collect();
            assert_eq!(back, samples, "{frames} {channels} {bits}");
            // Byte for byte what flacenc writes holding the whole sound in
            // memory: the same frames, the same stream info and MD5.
            if frames > 0 {
                assert_eq!(
                    std::fs::read(&f).unwrap(),
                    whole_stream(&samples, channels, bits)
                );
            }
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_wav_kept_as_flac_is_gone_once_its_flac_checks() {
        let dir = scratch("keep");
        let w = dir.join("audio-001.wav");
        let samples = wav(&w, 6_000, 1, 16);
        let f = keep_as_flac(&w).unwrap();
        assert_eq!(f, dir.join("audio-001.flac"));
        assert!(!w.exists());
        let back: Vec<i32> = claxon::FlacReader::open(&f)
            .unwrap()
            .samples()
            .map(Result::unwrap)
            .collect();
        assert_eq!(back, samples);
        // One that can't be kept stays a WAV.
        let w = dir.join("audio-002.wav");
        wav(&w, 6_000, 1, 16);
        let bytes = std::fs::read(&w).unwrap();
        std::fs::write(&w, &bytes[..bytes.len() / 2]).unwrap();
        assert!(keep_as_flac(&w).is_err());
        assert!(w.exists() && !flac_path(&w).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_wav_that_cannot_be_read_leaves_no_flac() {
        let dir = scratch("cut");
        let w = dir.join("a.wav");
        wav(&w, 10_000, 1, 16);
        // Cut short, as a file still being written or a damaged one is.
        let bytes = std::fs::read(&w).unwrap();
        std::fs::write(&w, &bytes[..bytes.len() * 3 / 4]).unwrap();
        let f = flac_path(&w);
        assert!(encode_wav(&w, &f).is_err());
        assert!(!f.exists() && !f.with_extension("flac.part").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn samples_that_differ_are_told_apart() {
        let dir = scratch("differ");
        let (a, b) = (dir.join("a.wav"), dir.join("b.wav"));
        wav(&a, 5_000, 1, 16);
        wav(&b, 5_001, 1, 16);
        let f = flac_path(&a);
        encode_wav(&a, &f).unwrap();
        // One frame more, and one sample changed.
        assert!(!same_samples(&b, &f).unwrap());
        let mut s = wav(&b, 5_000, 1, 16);
        s[2_500] ^= 1;
        let mut w =
            hound::WavWriter::create(&b, hound::WavReader::open(&a).unwrap().spec()).unwrap();
        for v in s {
            w.write_sample(v).unwrap();
        }
        w.finalize().unwrap();
        assert!(!same_samples(&b, &f).unwrap());
        assert!(same_samples(&a, &f).unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }
}
