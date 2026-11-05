//! FLAC written a block at a time, and a WAV turned into FLAC that way
//! (#503). Memory stays at a few blocks however long the sound is, so an
//! hour's part can be converted beside a running recording.

use crate::audio::AudioError;
use flacenc::component::{BitRepr, Frame, FrameHeader, FrameOffset, Stream};
use flacenc::error::Verify;
use flacenc::source::{Context, Fill, FrameBuf};
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom, Write};
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

/// The highest sample rate flacenc 0.5 takes in a stream's info, though
/// FLAC holds rates up to 655,350 Hz (#1175). A faster stream is made at
/// this rate, and its info and frames are given the true one as written
/// ([`header`], [`at_rate`]).
const FLACENC_MAX_RATE: u32 = 96_000;

/// "fLaC" and the stream info block, as `stream` (with no frames) gives
/// them, with `rate` as its sample rate: the same length whatever the
/// figures in it.
fn header(stream: &Stream, rate: u32) -> Result<Vec<u8>, AudioError> {
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).map_err(failed)?;
    let mut bytes = sink.as_slice().to_vec();
    // The stream info's 20-bit sample rate starts 18 bytes in: after
    // "fLaC", the block's own header, and the block and frame sizes.
    let (Some(b), true) = (bytes.get_mut(18..21), rate < 1 << 20) else {
        return Err(failed(format!("sample rate {rate}")));
    };
    b[0] = (rate >> 12) as u8;
    b[1] = (rate >> 4) as u8;
    b[2] = (b[2] & 0x0f) | ((rate & 0x0f) as u8) << 4;
    Ok(bytes)
}

/// `frame`, frame `n` of a stream of `bps`-bit samples, with `rate` as
/// the sample rate its header names: flacenc names the stream info's.
fn at_rate(frame: Frame, rate: u32, bps: usize, n: usize) -> Result<Frame, AudioError> {
    let (head, subframes) = frame.into_parts();
    let head = FrameHeader::new(
        head.block_size(),
        head.channel_assignment().clone(),
        bps,
        rate as usize,
        FrameOffset::Frame(n as u32),
    )
    .map_err(failed)?;
    Frame::new(head, subframes.into_iter()).map_err(failed)
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
    let mut plain = flacenc::config::Encoder::default();
    plain.subframe_coding.use_fixed = false;
    plain.subframe_coding.use_lpc = false;
    let plain = plain.into_verified().map_err(|(_, e)| failed(e))?;
    let block = cfg.block_size;
    let rate = format.sample_rate;
    let mut stream = Stream::new(rate.min(FLACENC_MAX_RATE) as usize, ch, bps).map_err(failed)?;
    let start = out.stream_position().map_err(AudioError::Io)?;
    out.write_all(&header(&stream, rate)?)
        .map_err(AudioError::Io)?;
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
        let mut frame = flacenc::encode_fixed_size_frame(&cfg, &fill.0, n, stream.stream_info())
            .map_err(failed)?;
        // flacenc can code a block of loud 24-bit sound in vastly more
        // than its plain samples (a 4096-sample block in up to a GB): still
        // lossless, so it checks, but a 12 minute take came out 40 GB. A
        // block that comes out bigger than plain is coded plain.
        if frame.count_bits() as u64 > plain_bits(buf.len() as u64, bps as u64, 1) {
            frame = flacenc::encode_fixed_size_frame(&plain, &fill.0, n, stream.stream_info())
                .map_err(failed)?;
        }
        let frame = at_rate(frame, rate, bps, n)?;
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
    out.write_all(&header(&stream, rate)?)
        .map_err(AudioError::Io)?;
    out.seek(SeekFrom::Start(end)).map_err(AudioError::Io)?;
    Ok(frames)
}

/// The most bits a stream's blocks may take: `samples` samples (all
/// channels) of `bps` bits written plain, in `blocks` blocks, with room
/// for each block's frame header, subframe headers and checksums.
fn plain_bits(samples: u64, bps: u64, blocks: u64) -> u64 {
    samples * bps + blocks * 8 * (16 + 8 + 2)
}

/// The FLAC file `wav` becomes: the same name with `.flac`.
pub fn flac_path(wav: &Path) -> PathBuf {
    wav.with_extension("flac")
}

/// Whether `p` names a WAV, by its extension.
pub fn is_wav(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
}

/// The name a sound file is kept under: a WAV's FLAC, any other as it
/// is. A WAV and the FLAC it was kept as share it.
pub fn kept_name(p: &Path) -> PathBuf {
    match is_wav(p) {
        true => flac_path(p),
        false => p.to_path_buf(),
    }
}

/// Encode the integer WAV `wav` as the FLAC file `flac`, a block at a
/// time, then check that it decodes to exactly the WAV's samples. The
/// FLAC is written under a temporary name and takes its own only once it
/// checks, so a FLAC at `flac` is always a whole, checked one. The WAV is
/// left as it is. Returns the frames converted.
pub fn encode_wav(wav: &Path, flac: &Path) -> Result<u64, AudioError> {
    encode_wav_file(&File::open(wav).map_err(AudioError::Io)?, flac)
}

/// [`encode_wav`] from a WAV already open, read from its start: a file
/// held open is read whole even when its name is deleted meanwhile.
pub fn encode_wav_file(wav: &File, flac: &Path) -> Result<u64, AudioError> {
    let mut reader = wav_reader(wav)?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int {
        return Err(AudioError::Unsupported("floating-point WAV".into()));
    }
    let format = Format {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        bits_per_sample: spec.bits_per_sample,
    };
    let mut samples = reader.samples::<i32>();
    // The FLAC keeps the WAV's time: when the sound was written, which
    // readers check the order of a take's hours by.
    let written = wav.metadata().and_then(|m| m.modified()).ok();
    write_checked(
        format,
        flac,
        written,
        |buf, most| {
            for s in samples.by_ref().take(most) {
                buf.push(s.map_err(AudioError::Wav)?);
            }
            Ok(())
        },
        |part| same_samples_file(wav, part),
    )
}

/// Write the FLAC `flac` from the samples `next` hands over ([`encode`])
/// under a temporary name, dated `written`, synced, and give it its own
/// name only once `same` says it holds the samples it should: a FLAC at
/// `flac` is always a whole, checked one, and one already there is
/// replaced in one step. Returns the frames written.
fn write_checked(
    format: Format,
    flac: &Path,
    written: Option<std::time::SystemTime>,
    next: impl FnMut(&mut Vec<i32>, usize) -> Result<(), AudioError>,
    same: impl FnOnce(&Path) -> Result<bool, AudioError>,
) -> Result<u64, AudioError> {
    let part = flac.with_extension("flac.part");
    let done = (|| {
        let mut out = std::io::BufWriter::new(File::create(&part).map_err(AudioError::Io)?);
        let frames = encode(format, &mut out, next)?;
        let out = out
            .into_inner()
            .map_err(|e| AudioError::Io(e.into_error()))?;
        if let Some(t) = written {
            out.set_modified(t).map_err(AudioError::Io)?;
        }
        out.sync_all().map_err(AudioError::Io)?;
        if !same(&part)? {
            return Err(failed(
                "the FLAC does not decode to the samples it was given",
            ));
        }
        std::fs::rename(&part, flac).map_err(AudioError::Io)?;
        sync_folder(flac)?;
        Ok(frames)
    })();
    if done.is_err() {
        std::fs::remove_file(&part).ok();
    }
    done
}

/// The format, largest block and length in frames a FLAC file's stream
/// info names, read from its first 42 bytes; None when they are not a
/// FLAC's.
fn stream_info(flac: &Path) -> Option<(Format, u64, u64)> {
    use std::io::Read;
    let mut b = [0u8; 42];
    File::open(flac).ok()?.read_exact(&mut b).ok()?;
    // "fLaC", then the first block's header: type 0 (stream info), 34 long.
    if &b[..4] != b"fLaC" || b[4] & 0x7f != 0 || b[5..8] != [0, 0, 34] {
        return None;
    }
    // From byte 18: 20 bits of sample rate, 3 of channels less one, 5 of
    // bits per sample less one, 36 of frames.
    let x = u64::from_be_bytes(b[18..26].try_into().ok()?);
    let format = Format {
        sample_rate: (x >> 44) as u32,
        channels: ((x >> 41) & 0x7) as u16 + 1,
        bits_per_sample: ((x >> 36) & 0x1f) as u16 + 1,
    };
    let block = u64::from(u16::from_be_bytes([b[10], b[11]]));
    Some((format, block, x & 0xf_ffff_ffff))
}

/// Whether the FLAC `flac` is bigger than its samples would be written
/// plain, as flacenc wrote some blocks of loud 24-bit sound before
/// [`encode`] checked each block's size (a 12:53 take kept in 40 GB).
/// [`shrink`] writes such a one again. Never one whose stream info does
/// not give its length or block size (0, "unknown").
pub fn bloated(flac: &Path) -> bool {
    let Some((f, block, frames)) = stream_info(flac) else {
        return false;
    };
    let Ok(size) = std::fs::metadata(flac).map(|m| m.len()) else {
        return false;
    };
    if frames == 0 || block == 0 {
        return false;
    }
    let samples = frames * u64::from(f.channels);
    let bits = plain_bits(
        samples,
        u64::from(f.bits_per_sample),
        frames.div_ceil(block),
    );
    size > 42 + bits.div_ceil(8) + 64 * 1024
}

/// Write the FLAC `flac` again from its own samples, a block at a time,
/// and put the new one in its place only once it decodes to exactly the
/// old one's samples: the old one is whole until then. It keeps its
/// time. For a [`bloated`] FLAC, which this makes no bigger than its
/// samples written plain. Returns the frames written.
pub fn shrink(flac: &Path) -> Result<u64, AudioError> {
    let mut old = claxon::FlacReader::open(flac).map_err(AudioError::Flac)?;
    let i = old.streaminfo();
    let format = Format {
        sample_rate: i.sample_rate,
        channels: i.channels as u16,
        bits_per_sample: i.bits_per_sample as u16,
    };
    let written = std::fs::metadata(flac).and_then(|m| m.modified()).ok();
    let mut samples = old.samples();
    write_checked(
        format,
        flac,
        written,
        |buf, most| {
            for s in samples.by_ref().take(most) {
                buf.push(s.map_err(AudioError::Flac)?);
            }
            Ok(())
        },
        |part| same_flac_samples(flac, part),
    )
}

/// Whether the FLACs `a` and `b` hold the same samples in the same format.
fn same_flac_samples(a: &Path, b: &Path) -> Result<bool, AudioError> {
    let mut a = claxon::FlacReader::open(a).map_err(AudioError::Flac)?;
    let mut b = claxon::FlacReader::open(b).map_err(AudioError::Flac)?;
    let (x, y) = (a.streaminfo(), b.streaminfo());
    if (x.sample_rate, x.channels, x.bits_per_sample)
        != (y.sample_rate, y.channels, y.bits_per_sample)
    {
        return Ok(false);
    }
    let mut back = b.samples();
    for s in a.samples() {
        let s = s.map_err(AudioError::Flac)?;
        let Some(v) = back.next() else {
            return Ok(false);
        };
        if v.map_err(AudioError::Flac)? != s {
            return Ok(false);
        }
    }
    Ok(back.next().is_none())
}

/// Make the names in the folder holding `file` last: once this returns,
/// a power cut cannot undo a rename into it, so a WAV deleted after its
/// FLAC took its name is never lost with it. Windows has no folder to sync;
/// its renames are written through by the file system.
fn sync_folder(file: &Path) -> Result<(), AudioError> {
    #[cfg(unix)]
    {
        let dir = match file.parent() {
            Some(d) if !d.as_os_str().is_empty() => d,
            _ => Path::new("."),
        };
        File::open(dir)
            .and_then(|d| d.sync_all())
            .map_err(AudioError::Io)?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

/// The open WAV `wav`, read from its start.
fn wav_reader(wav: &File) -> Result<hound::WavReader<BufReader<&File>>, AudioError> {
    let mut from = wav;
    from.seek(SeekFrom::Start(0)).map_err(AudioError::Io)?;
    hound::WavReader::new(BufReader::new(from)).map_err(AudioError::Wav)
}

/// Keep the WAV `wav` as FLAC: [`encode_wav`] beside it under the same
/// name, then delete the WAV once the FLAC checks and its name is synced
/// to disk. Readers take the FLAC
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
    same_samples_file(&File::open(wav).map_err(AudioError::Io)?, flac)
}

/// [`same_samples`] for a WAV already open, read from its start.
pub fn same_samples_file(wav: &File, flac: &Path) -> Result<bool, AudioError> {
    let mut w = wav_reader(wav)?;
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

/// A FLAC at `path` as flacenc writes it unchecked, bloated: 16 blocks
/// of a loud 24-bit mono sine at 48 kHz, in over twice their plain size.
/// Returns its samples.
#[cfg(test)]
pub(crate) fn write_bloated(path: &Path) -> Vec<i32> {
    let (hi, lo) = ((1i32 << 23) - 1, -(1i32 << 23));
    let samples: Vec<i32> = (0..16 * 4096)
        .map(|i| ((i as f64 * 0.3).sin() * 1e7).clamp(f64::from(lo), f64::from(hi)) as i32)
        .collect();
    std::fs::write(path, tests::whole_stream(&samples, 1, 24)).unwrap();
    samples
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
        wav_at(path, frames, channels, bits, 48000)
    }

    /// [`wav`] at `rate` samples a second.
    fn wav_at(path: &Path, frames: usize, channels: u16, bits: u16, rate: u32) -> Vec<i32> {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
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
    pub(super) fn whole_stream(samples: &[i32], channels: u16, bits: u16) -> Vec<u8> {
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

    /// A take recorded faster than 96 kHz, flacenc's own limit, is kept as
    /// FLAC too, at its own rate in the stream info and in every frame's
    /// header, up to FLAC's 655,350 Hz (#1175). Above that it stays WAV.
    #[test]
    fn a_wav_faster_than_96_khz_becomes_a_flac_at_its_own_rate() {
        let dir = scratch("fast");
        for (rate, channels, bits) in [
            (96_000, 1, 24),
            (176_400, 2, 16),
            (192_000, 1, 24),
            (384_000, 1, 16),
            (655_350, 2, 24),
        ] {
            let w = dir.join(format!("a-{rate}.wav"));
            let samples = wav_at(&w, 10_000, channels, bits, rate);
            let f = flac_path(&w);
            assert_eq!(encode_wav(&w, &f).unwrap(), 10_000, "{rate}");
            assert!(same_samples(&w, &f).unwrap(), "{rate}");
            let mut r = claxon::FlacReader::open(&f).unwrap();
            assert_eq!(r.streaminfo().sample_rate, rate);
            let back: Vec<i32> = r.samples().map(Result::unwrap).collect();
            assert_eq!(back, samples, "{rate}");
            // Each frame's header names the rate as the stream info does.
            assert_eq!(header_rates(&std::fs::read(&f).unwrap()), vec![rate; 3]);
        }
        let w = dir.join("too-fast.wav");
        wav_at(&w, 1_000, 1, 16, 700_000);
        assert!(encode_wav(&w, &flac_path(&w)).is_err());
        assert!(w.exists() && !flac_path(&w).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The sample rate each frame header of the FLAC `bytes` names, read
    /// as the format lays a header out: the sync code, then the block
    /// size and rate tags, the frame number (UTF-8 coded), any block size
    /// and rate bytes the tags call for, and a CRC-8 over all of it, which
    /// must check.
    fn header_rates(bytes: &[u8]) -> Vec<u32> {
        let crc8 = |b: &[u8]| {
            b.iter().fold(0u8, |c, &x| {
                (0..8).fold(
                    c ^ x,
                    |c, _| if c & 0x80 != 0 { c << 1 ^ 0x07 } else { c << 1 },
                )
            })
        };
        let mut out = Vec::new();
        for at in 42..bytes.len().saturating_sub(16) {
            if bytes[at] != 0xff || bytes[at + 1] != 0xf8 {
                continue;
            }
            let (block_tag, rate_tag) = (bytes[at + 2] >> 4, bytes[at + 2] & 0x0f);
            let mut i = at + 4 + bytes[at + 4].leading_ones().max(1) as usize;
            i += match block_tag {
                6 => 1,
                7 => 2,
                _ => 0,
            };
            let (rate, extra) = match rate_tag {
                2 => (176_400, 0),
                3 => (192_000, 0),
                10 => (48_000, 0),
                11 => (96_000, 0),
                12 => (u32::from(bytes[i]) * 1000, 1),
                13 => (u32::from(u16::from_be_bytes([bytes[i], bytes[i + 1]])), 2),
                14 => (
                    u32::from(u16::from_be_bytes([bytes[i], bytes[i + 1]])) * 10,
                    2,
                ),
                _ => continue,
            };
            i += extra;
            if crc8(&bytes[at..i]) == bytes[i] {
                out.push(rate);
            }
        }
        out
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

    #[test]
    fn a_flac_keeps_the_time_its_wav_was_written() {
        let dir = scratch("mtime");
        let w = dir.join("audio-001.wav");
        wav(&w, 6_000, 1, 16);
        let then =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_761_600_000);
        std::fs::File::options()
            .write(true)
            .open(&w)
            .unwrap()
            .set_modified(then)
            .unwrap();
        let f = keep_as_flac(&w).unwrap();
        assert_eq!(std::fs::metadata(&f).unwrap().modified().unwrap(), then);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Loud 24-bit sound that flacenc codes in far more than its plain
    /// samples (a 12:53 take came out 40 GB; flacenc's Rice size estimate
    /// wraps for big residuals): a full-scale square wave of 34 samples,
    /// one 4096-sample block of which flacenc alone writes in about a GB,
    /// a clipped sine, and uniform noise of ±2^22. Every block comes out
    /// no bigger than plain, so the FLAC is no bigger than its samples
    /// plain with the stream's and each block's headers (about its WAV's
    /// size at worst), and it holds the same samples.
    #[test]
    fn loud_24_bit_sound_is_never_kept_bigger_than_plain() {
        let dir = scratch("loud");
        let w = dir.join("audio-001.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let (hi, lo) = ((1i32 << 23) - 1, -(1i32 << 23));
        let mut out = hound::WavWriter::create(&w, spec).unwrap();
        for i in 0..4096 {
            out.write_sample(if (i / 17) % 2 == 0 { hi } else { lo })
                .unwrap();
        }
        for i in 0..4096 {
            let a = (i as f64 * 0.3).sin() * 3e7;
            out.write_sample(a.clamp(f64::from(lo), f64::from(hi)) as i32)
                .unwrap();
        }
        let mut x: u64 = 88_172_645_463_325_252;
        for _ in 0..4096 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            out.write_sample((x % (1 << 23)) as i32 - (1 << 22))
                .unwrap();
        }
        out.finalize().unwrap();
        let f = dir.join("audio-001.flac");
        assert_eq!(encode_wav(&w, &f).unwrap(), 3 * 4096);
        let size = |p: &Path| std::fs::metadata(p).unwrap().len();
        let most = 42 + 3 * (4096 * 3 + 16 + 8 + 2);
        assert!(size(&f) <= most, "{} > {most}", size(&f));
        assert!(same_samples(&w, &f).unwrap());
        assert!(!bloated(&f));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A FLAC flacenc wrote bloated, as it was kept before blocks were
    /// checked: [`bloated`] finds it, and [`shrink`] writes it again no
    /// bigger than plain, with the same samples and time.
    #[test]
    fn a_bloated_flac_is_written_again_with_the_same_samples() {
        let dir = scratch("shrink");
        let f = dir.join("audio-001.flac");
        let samples = write_bloated(&f);
        let then =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_761_600_000);
        File::options()
            .write(true)
            .open(&f)
            .unwrap()
            .set_modified(then)
            .unwrap();
        let size = || std::fs::metadata(&f).unwrap().len();
        let plain = samples.len() as u64 * 3;
        assert!(size() > 2 * plain, "not bloated: {} bytes", size());
        assert!(bloated(&f));
        assert_eq!(shrink(&f).unwrap(), samples.len() as u64);
        // Plain, with the stream's and each block's headers.
        let most = plain + 42 + 16 * (16 + 8 + 2);
        assert!(size() <= most, "{} > {most}", size());
        assert!(!bloated(&f));
        let back: Vec<i32> = claxon::FlacReader::open(&f)
            .unwrap()
            .samples()
            .map(Result::unwrap)
            .collect();
        assert_eq!(back, samples);
        assert_eq!(std::fs::metadata(&f).unwrap().modified().unwrap(), then);
        assert!(!dir.join("audio-001.flac.part").exists());
        // A stream whose info leaves its length unknown (0) is never
        // taken for bloated, however big.
        let mut bytes = std::fs::read(&f).unwrap();
        bytes[21] &= 0xf0;
        bytes[22..26].fill(0);
        bytes.resize(bytes.len() + 128 * 1024, 0);
        std::fs::write(&f, bytes).unwrap();
        assert_eq!(stream_info(&f).unwrap().2, 0);
        assert!(!bloated(&f));
        std::fs::remove_dir_all(&dir).ok();
    }
}
