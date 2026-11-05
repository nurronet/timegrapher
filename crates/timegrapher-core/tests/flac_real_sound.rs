//! FLAC size on real watch sound. One second of a public corpus take
//! (aquanaut324_DU_20min_newstand, 60 s in, CC0) is written through
//! `flac::encode` at the levels a 24-bit sound card can deliver: louder
//! by every power of two up to clipping, with added hiss, at 16 and 24
//! bits, mono and stereo. Every FLAC must hold exactly its samples and be
//! no bigger than those samples written plain, with the stream's and each
//! block's headers. Before #1171, flacenc wrote this sound at 24 bits
//! 32 times louder in 1,168 times its plain size; synthetic signals alone
//! had not shown it.

use std::path::{Path, PathBuf};
use timegrapher_core::flac::{encode, Format};

/// The fixture's samples: 48 kHz, mono, 16-bit.
fn real_second() -> Vec<i32> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/aquanaut324_DU_1s.flac");
    let mut r = claxon::FlacReader::open(path).unwrap();
    let i = r.streaminfo();
    assert_eq!(
        (i.sample_rate, i.channels, i.bits_per_sample),
        (48_000, 1, 16)
    );
    r.samples().map(Result::unwrap).collect()
}

/// A plain, repeatable noise source (xorshift), in [-1, 1).
struct Noise(u64);
impl Noise {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tg-flac-real-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Write `samples` (interleaved) as FLAC at `path` through `flac::encode`,
/// then check its size and that it decodes to exactly `samples`.
fn check(label: &str, path: &Path, format: Format, samples: &[i32]) {
    let mut out = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut rest = samples;
    encode(format, &mut out, |buf, most| {
        let (now, later) = rest.split_at(most.min(rest.len()));
        buf.extend_from_slice(now);
        rest = later;
        Ok(())
    })
    .unwrap();
    drop(out);
    let size = std::fs::metadata(path).unwrap().len();
    let ch = u64::from(format.channels);
    let frames = samples.len() as u64 / ch;
    let blocks = frames.div_ceil(4096);
    // Plain samples, the 42-byte stream header, and per block what
    // `flac::encode` allows over plain (26 bytes of headers and checksums)
    // and a byte of padding a channel.
    let plain = (samples.len() as u64 * u64::from(format.bits_per_sample)).div_ceil(8);
    let most = plain + 42 + blocks * (26 + ch);
    assert!(
        size <= most,
        "{label}: {size} bytes > {most} (plain {plain})"
    );
    let back: Vec<i32> = claxon::FlacReader::open(path)
        .unwrap()
        .samples()
        .map(Result::unwrap)
        .collect();
    assert!(back == samples, "{label}: decodes to other samples");
}

#[test]
fn real_watch_sound_is_never_kept_bigger_than_plain_at_any_level() {
    let dir = scratch("levels");
    let f = dir.join("x.flac");
    let real = real_second();
    let mono = |bits: u16| Format {
        sample_rate: 48_000,
        channels: 1,
        bits_per_sample: bits,
    };
    check("16-bit as recorded", &f, mono(16), &real);
    let clip = |v: i64, bits: u32| v.clamp(-(1 << (bits - 1)), (1 << (bits - 1)) - 1) as i32;
    for gain in 0..=12 {
        let s: Vec<i32> = real
            .iter()
            .map(|&v| clip(i64::from(v) << (8 + gain), 24))
            .collect();
        check(&format!("24-bit, x{}", 1 << gain), &f, mono(24), &s);
        let s: Vec<i32> = real
            .iter()
            .map(|&v| clip(i64::from(v) << gain, 16))
            .collect();
        check(&format!("16-bit, x{}", 1 << gain), &f, mono(16), &s);
    }
    let mut noise = Noise(0x2545_f491_4f6c_dd1d);
    for k in [8, 12, 16, 20, 22] {
        let s: Vec<i32> = real
            .iter()
            .map(|&v| {
                clip(
                    (i64::from(v) << 8) + (noise.next() * f64::from(1 << k)) as i64,
                    24,
                )
            })
            .collect();
        check(&format!("24-bit with hiss of ±2^{k}"), &f, mono(24), &s);
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn real_watch_sound_in_stereo_is_never_kept_bigger_than_plain() {
    let dir = scratch("stereo");
    let f = dir.join("x.flac");
    let real = real_second();
    let stereo = Format {
        sample_rate: 48_000,
        channels: 2,
        bits_per_sample: 24,
    };
    let clip = |v: i64| v.clamp(-(1 << 23), (1 << 23) - 1) as i32;
    for gain in [0, 4, 8, 12] {
        // The second channel a sample late and at half level, as a second
        // microphone might hear it.
        let s: Vec<i32> = (0..real.len())
            .flat_map(|i| {
                let left = i64::from(real[i]) << (8 + gain);
                let right = (i64::from(real[i.saturating_sub(1)]) << (8 + gain)) / 2;
                [clip(left), clip(right)]
            })
            .collect();
        check(&format!("24-bit stereo, x{}", 1 << gain), &f, stereo, &s);
    }
    std::fs::remove_dir_all(&dir).ok();
}
