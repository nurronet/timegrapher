//! Sound written as FLAC through `flac::encode`, the one place every FLAC
//! the software keeps is coded. The first byte picks the format (16 to 24
//! bits, one or two channels) and a gain; the rest is the samples. The
//! FLAC must decode to exactly those samples and be no bigger than them
//! written plain, with the stream's and each block's headers: flacenc
//! once wrote loud 24-bit blocks in up to a GB each (#1171).
#![no_main]
use libfuzzer_sys::fuzz_target;
use timegrapher_core::flac::{encode, Format};

fuzz_target!(|data: &[u8]| {
    let Some((&pick, bytes)) = data.split_first() else {
        return;
    };
    let bits: u16 = [16, 20, 24, 24][(pick & 3) as usize];
    let channels: u16 = 1 + u16::from(pick >> 2 & 1);
    let gain = u32::from(pick >> 3); // 0 to 31: shifted left, then clipped
    let width = usize::from(bits).div_ceil(8);
    let (lo, hi) = (-(1i64 << (bits - 1)), (1i64 << (bits - 1)) - 1);
    let mut samples: Vec<i32> = bytes
        .chunks_exact(width)
        .map(|c| {
            let mut v = 0i64;
            for &b in c {
                v = v << 8 | i64::from(b);
            }
            // Sign-extend from the chunk's width, then keep its top `bits`.
            let v = (v << (64 - 8 * width)) >> (64 - 8 * width);
            let v = v >> (8 * width - usize::from(bits));
            (v.checked_shl(gain).unwrap_or(v.signum() * hi)).clamp(lo, hi) as i32
        })
        .collect();
    samples.truncate(samples.len() / usize::from(channels) * usize::from(channels));
    let format = Format {
        sample_rate: 48_000,
        channels,
        bits_per_sample: bits,
    };
    let mut out = std::io::Cursor::new(Vec::new());
    let mut rest = samples.as_slice();
    encode(format, &mut out, |buf, most| {
        let (now, later) = rest.split_at(most.min(rest.len()));
        buf.extend_from_slice(now);
        rest = later;
        Ok(())
    })
    .expect("a FLAC of samples in range is always written");
    let bytes = out.into_inner();
    let ch = u64::from(channels);
    let blocks = (samples.len() as u64 / ch).div_ceil(4096);
    let plain = samples.len() as u64 * u64::from(bits) / 8;
    // As `flac::encode` allows a block: its samples plain and 26 bytes of
    // headers and checksums, and a byte of padding a channel.
    let most = plain + 42 + blocks * (26 + ch);
    assert!(bytes.len() as u64 <= most, "{} bytes > {most}", bytes.len());
    if samples.is_empty() {
        return;
    }
    let back: Vec<i32> = claxon::FlacReader::new(std::io::Cursor::new(&bytes))
        .expect("the FLAC reads")
        .samples()
        .map(|s| s.expect("every sample reads"))
        .collect();
    assert_eq!(back, samples);
});
