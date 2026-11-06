# Sound files: formats, FLAC and its limits

How the software stores sound, which formats it reads and writes, and what
is known to go wrong. Everything here is in core `flac.rs`, `recorder.rs`,
`capture.rs` and `audio.rs`.

## What the recorder writes

- **Mono, 16-bit integer** (`recorder::STORED_BITS`), whatever the
  microphone gives: a 24-bit or floating-point microphone (common on macOS)
  is rounded to the nearest 16-bit step. Tested on four 24-bit takes (#1176):
  their low 8 bits held nothing, the 16-bit copies read the same rate,
  amplitude and beat error, and their FLACs were 46% the size. The
  stand's noise is a few 16-bit steps; rounding adds a third of one.
- **48 kHz when the device offers it** (`capture::PREFERRED_RATE`). Otherwise
  the device's own default rate is used.
- **The hour being recorded is a WAV.** Each hour is kept as FLAC once it
  closes (`flac::keep_as_flac`): the FLAC is written under a temporary name,
  checked sample for sample against the WAV, and renamed. Only then is the
  WAV deleted. At start-up the app does the same for any hour still left as
  a WAV.

## Every FLAC goes through one function

`flac::encode` is the only code that calls the FLAC encoder (flacenc). The
recorder, the start-up pass, the repair of bloated files and report
excerpts all use it. Each block is checked after coding, and a block that
comes out bigger than its samples written plain is coded again plain. So a
FLAC is never bigger than its WAV, give or take a few bytes of header per
4096-sample block.

## The bloat found on 2026-11-05

flacenc 0.5.1 estimates how many bits each block will take. For loud 20-
and 24-bit sound that estimate wraps around in 32-bit arithmetic
(`PrcBitTable::from_errors` in its `rice.rs`), so the encoder picks a coding
far bigger than the samples. The file is still lossless, so it decodes
correctly and passed the sample-for-sample check. It is just huge, and slow
to open.

- **Seen on real takes:** a 12:53, 24-bit Positions Test kept as 40.7 GB
  (366x its plain size), and a 44 s take kept as 792 MB (125x). Both were
  recorded at 24 bits on a Mac. On a 16-bit workstation, none of
  70 FLACs was affected.
- **Before the fix (main at 6a99c3c):** 8-, 12- and 16-bit sound never
  bloated. 20-bit sound bloated up to 543x and 24-bit up to 57,786x. Real
  corpus takes made 24-bit and louder bloated up to 3,955x.
- **The fix (#1171):** the per-block check above. `flac::bloated` finds a
  file written before the fix, and `flac::shrink` writes it again,
  replacing the old file only once the new one decodes to exactly the same
  samples and keeping its date. The start-up pass repairs such hours by
  itself.

## Formats checked

After the fix, 4,125 encodes were checked for the same samples and a size
no bigger than plain. They covered:

- sample rates of 8, 11.025, 16, 22.05, 32, 44.1, 48, 88.2 and 96 kHz;
- 8-, 12-, 16-, 20- and 24-bit samples;
- 1, 2, 4 and 8 channels;
- silence, full-scale DC, hiss from −48 dB to full scale, real watch ticks
  at their recorded level and 8 and 64 times louder, ticks with mains hum, a
  square wave, a full-scale alternating signal and a clipped sine;
- lengths of 0, 1, 15, 16, 4095, 4096 and 4097 samples, and several blocks.

The 560 real-corpus cases (28 public takes, each at 20 levels and kinds of
added hiss) were all within 0.08% of plain. A fuzz target
(`fuzz/fuzz_targets/flac_encode.rs`) tests the same promise on made-up sound.

## Known limits

- **Above 655,350 Hz, or above 65,535 Hz at a rate that isn't a multiple
  of 10, no FLAC is written**: the frame headers flacenc writes always name
  the rate, and can't name those (FLAC itself could leave it to the stream
  info). Such a take's hours stay WAV, whole and readable.
  flacenc itself takes rates only up to 96 kHz, so a faster stream is made
  at 96 kHz and its stream info and every frame header are given the true
  rate as they are written (#1175); the FLAC is then checked sample for
  sample against the WAV, as every FLAC is.
- **32-bit integer and floating-point WAVs stay WAV** (#1176). The recorder
  never writes them, so they come only from files copied into a take.

## Tests that guard this

- `flac::tests::loud_24_bit_sound_is_never_kept_bigger_than_plain`
- `flac::tests::a_bloated_flac_is_written_again_with_the_same_samples`
- `flac::tests::a_wav_faster_than_96_khz_becomes_a_flac_at_its_own_rate`
- `recorder::tests::a_saved_hour_kept_as_a_bloated_flac_is_written_again_in_its_place`
- `tests/flac_real_sound.rs`: one second of a public corpus take (CC0) at
  every level up to clipping, 16- and 24-bit, mono and stereo.
- The `flac_encode` fuzz target, run with the other fuzz targets overnight.
