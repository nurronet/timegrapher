//! A recording that opens with the stand's noise before the watch is
//! placed (#844): the whole file reads the watch, with the lead left out
//! and named, as long as the watch is heard for long enough after it.

use timegrapher_core::audio::Audio;
use timegrapher_core::measure::{Caveat, RATE_WINDOW_STEP_S};
use timegrapher_core::synth::{generate, noise_rms, Rng, SynthConfig, SCALE};
use timegrapher_core::{analyze, take, AnalysisConfig};

/// `noise_s` of the synthetic stand noise (as the live tests make it),
/// then `watch_s` of a watch at +5 s/d and 270°.
fn noise_then_watch(noise_s: f64, watch_s: f64) -> (Audio, Audio) {
    noise_then(noise_s, watch_s, 28_800, 5)
}

/// [`noise_then_watch`] at `bph`, the noise drawn from `seed`.
fn noise_then(noise_s: f64, watch_s: f64, bph: u32, seed: u64) -> (Audio, Audio) {
    let watch = watch_at(bph, watch_s);
    let led = noise_before(noise_s, &watch, seed);
    (watch, led)
}

/// `watch_s` of a watch at `bph`, +5 s/d and 270°.
fn watch_at(bph: u32, watch_s: f64) -> Audio {
    generate(
        &SynthConfig {
            duration_s: watch_s,
            bph,
            rate_s_per_day: 5.0,
            beat_error_ms: 0.4,
            ..Default::default()
        },
        |_| 270.0,
        |_| 0.0,
    )
}

/// `noise_s` of the synthetic stand noise drawn from `seed`, then `watch`.
fn noise_before(noise_s: f64, watch: &Audio, seed: u64) -> Audio {
    let rms = noise_rms(SynthConfig::default().snr_db) * SCALE;
    let mut rng = Rng::new(seed);
    let n = (noise_s * watch.sample_rate as f64) as usize;
    let mut samples: Vec<f32> = (0..n).map(|_| (rms * rng.normal()) as f32).collect();
    samples.extend_from_slice(&watch.samples);
    Audio {
        samples,
        sample_rate: watch.sample_rate,
    }
}

fn lead_left_out(c: &[Caveat]) -> Option<f64> {
    c.iter().find_map(|c| match c {
        Caveat::LeadLeftOut { seconds } => Some(*seconds),
        _ => None,
    })
}

#[test]
fn a_watch_after_a_long_noise_lead_reads_as_the_watch_with_the_lead_named() {
    for (noise_s, watch_s) in [(49.0, 30.0), (60.0, 60.0), (120.0, 40.0)] {
        let what = format!("{noise_s} s of noise, then {watch_s} s of watch");
        let (alone, led) = noise_then_watch(noise_s, watch_s);
        let cfg = AnalysisConfig::default();
        let want = analyze(&alone, &cfg).summary;
        let got = analyze(&led, &cfg).summary;
        assert!(want.watch_heard, "{what}: the watch alone");
        assert!(
            lead_left_out(&want.caveats).is_none(),
            "{what}: no lead on the watch alone"
        );
        assert!(got.watch_heard, "{what}: {:?}", got.caveats);
        let lead = lead_left_out(&got.caveats).expect("the lead named");
        assert!((lead - noise_s).abs() < 1.0, "{what}: lead {lead}");
        let near = |a: Option<f64>, b: Option<f64>, tol: f64, name: &str| {
            let (a, b) = (a.unwrap(), b.unwrap());
            assert!((a - b).abs() <= tol, "{what}: {name} {a} against {b}");
        };
        // The beats found are counted against those expected over the
        // sound after the lead, the time listened (#1417).
        for s in [&want, &got] {
            let f = s.beats_found_fraction;
            assert!((0.97..1.03).contains(&f), "{what}: found {f}");
        }
        near(got.rate_s_per_day, want.rate_s_per_day, 0.3, "rate");
        near(got.amplitude_deg, want.amplitude_deg, 1.0, "amplitude");
        near(
            got.beat_error_unlock_ms,
            want.beat_error_unlock_ms,
            0.03,
            "beat error",
        );
    }
}

/// At 18,000 and 19,800 bph a 10 s window of the noise can scatter under
/// the doubt line (0.7% of a beat), far more than the watch after it
/// (#1418): the lead runs on to where the scatter drops to the watch's,
/// not to the first such noise window, and the noise's beats stay out of
/// the rate. With the rate set too, as the guess is the same.
#[test]
fn a_noise_lead_before_a_slower_watch_ends_where_the_watch_starts() {
    for (bph, noise_s, seed) in [
        (18_000, 49.0, 5),
        (18_000, 60.0, 5),
        (18_000, 60.0, 21),
        (19_800, 60.0, 5),
        (19_800, 60.0, 9),
        (19_800, 49.0, 5),
        // Seeds the fix was not tuned on.
        (18_000, 60.0, 101),
        (18_000, 75.0, 404),
        (19_800, 60.0, 202),
        (19_800, 49.0, 505),
        (21_600, 60.0, 303),
        (21_600, 49.0, 606),
        (18_000, 60.0, 707),
        (19_800, 60.0, 808),
        (21_600, 60.0, 909),
    ] {
        let what = format!("{bph} bph after {noise_s} s of noise ({seed})");
        let (alone, led) = noise_then(noise_s, 60.0, bph, seed);
        let want = analyze(&alone, &AnalysisConfig::default()).summary;
        for set in [None, Some(bph)] {
            let cfg = AnalysisConfig {
                bph: set,
                ..Default::default()
            };
            let got = analyze(&led, &cfg).summary;
            assert_eq!(got.bph, bph, "{what}");
            let lead = lead_left_out(&got.caveats).expect("the lead named");
            // Within a 5 s window step of the watch's start before it and
            // two after: the windows step 5 s, and none spans the jump
            // onto the watch's beats. The rate says the noise is out.
            assert!(
                (noise_s - 5.0..noise_s + 10.0).contains(&lead),
                "{what}, set {set:?}: lead {lead}"
            );
            let (r, w) = (got.rate_s_per_day.unwrap(), want.rate_s_per_day.unwrap());
            assert!((r - w).abs() <= 0.3, "{what}: rate {r} against {w}");
        }
    }
}

/// A watch heard from the start whose first `rough_s` scatter about
/// `sd_frac` of a beat: each beat there is moved by a draw of that SD,
/// cut at the midpoints between the beats `analyze` finds.
fn rough_start(bph: u32, rough_s: f64, sd_frac: f64, seed: u64) -> Audio {
    scatter(&watch_at(bph, 120.0), bph, rough_s, sd_frac, seed)
}

/// `watch` with each beat before `upto_s` moved by a draw of `sd_frac`
/// of a beat, cut at the midpoints between the beats `analyze` finds.
fn scatter(watch: &Audio, bph: u32, upto_s: f64, sd_frac: f64, seed: u64) -> Audio {
    let fs = f64::from(watch.sample_rate);
    let beats = analyze(watch, &AnalysisConfig::default()).beats;
    let beat_s = 3600.0 / f64::from(bph);
    let mut rng = Rng::new(seed);
    let mut out = watch.samples.clone();
    for b in beats.iter().filter(|b| b.time > beat_s && b.time < upto_s) {
        let (a, z) = (
            ((b.time - beat_s / 2.0) * fs) as usize,
            ((b.time + beat_s / 2.0) * fs) as usize,
        );
        let shift = (rng.normal() * sd_frac * beat_s * fs).round() as isize;
        for (i, v) in out.iter_mut().enumerate().take(z).skip(a) {
            let from = i as isize - shift;
            *v = if (a as isize..z as isize).contains(&from) {
                watch.samples[from as usize]
            } else {
                0.0
            };
        }
    }
    Audio {
        samples: out,
        sample_rate: watch.sample_rate,
    }
}

/// A noise window can scatter under a watch's line when the watch
/// scatters 0.1% of a beat: on this noise (seed 101) the window from 45 s
/// reads 0.34% against a line of 0.40%, and the window after it holds
/// no beat clear enough to fit, so the next window clear of it is the
/// one across the step onto the watch at 60 s, which reads as the
/// watch's (#1418). Such a pair is not taken as the watch's: the lead
/// runs to the watch and the rate stays the watch's alone, guessed and
/// set. With 0.05% of scatter, and every acceptance seed, the same.
#[test]
fn a_noise_window_paired_across_the_step_onto_the_watch_stays_in_the_lead() {
    let bph = 18_000;
    for (sd, seed) in [(0.001, 101), (0.0005, 101), (0.001, 2004), (0.001, 3001)] {
        let what = format!("watch scattering {sd} of a beat after noise {seed}");
        let watch = scatter(&watch_at(bph, 60.0), bph, 60.0 - 0.2, sd, 77);
        let want = analyze(&watch, &AnalysisConfig::default()).summary;
        let led = noise_before(60.0, &watch, seed);
        for set in [None, Some(bph)] {
            let cfg = AnalysisConfig {
                bph: set,
                ..Default::default()
            };
            let got = analyze(&led, &cfg).summary;
            assert_eq!(got.bph, bph, "{what}");
            let lead = lead_left_out(&got.caveats).expect("the lead named");
            assert!((55.0..70.0).contains(&lead), "{what}: lead {lead}");
            let (r, w) = (got.rate_s_per_day.unwrap(), want.rate_s_per_day.unwrap());
            assert!((r - w).abs() <= 0.3, "{what}: rate {r} against {w}");
        }
    }
}

/// A watch heard from the start whose first 30 to 60 s scatter about
/// 0.5% of a beat, under the doubt line, and then 0.01%: that rough start
/// scatters more than 4 times the windows after it, so it is left out as
/// a lead would be (#1418). The rate stays the watch's.
#[test]
fn a_watchs_rough_start_is_left_out_as_a_lead() {
    for (bph, rough_s) in [
        (28_800, 45.0),
        (18_000, 45.0),
        (21_600, 60.0),
        (28_800, 30.0),
    ] {
        let what = format!("{bph} bph, rough for {rough_s} s");
        let s = analyze(
            &rough_start(bph, rough_s, 0.005, 3),
            &AnalysisConfig::default(),
        )
        .summary;
        assert!(s.watch_heard, "{what}");
        let lead = lead_left_out(&s.caveats).expect("the rough start left out");
        assert!(lead > 10.0 && lead <= rough_s + 1.0, "{what}: lead {lead}");
        let r = s.rate_s_per_day.unwrap();
        assert!((r - 5.0).abs() <= 0.3, "{what}: rate {r}");
    }
}

/// Under 20 s of watch after the noise, nothing is left out: the file
/// stays no watch, as a set average over such a stretch does live. The
/// 11 s cases are the ones `LEAD_WATCH_MIN_S` decides: with no floor they
/// would read as a watch with the lead left out.
#[test]
fn a_brief_watch_after_a_long_noise_lead_is_still_no_watch() {
    for (noise_s, watch_s) in [(110.0, 15.0), (60.0, 11.0), (110.0, 11.0)] {
        let what = format!("{noise_s} s of noise, then {watch_s} s of watch");
        let (_, led) = noise_then_watch(noise_s, watch_s);
        let got = analyze(&led, &AnalysisConfig::default()).summary;
        assert!(!got.watch_heard, "{what}: {:?}", got.caveats);
        assert!(lead_left_out(&got.caveats).is_none(), "{what}");
    }
}

/// An entry of a take that starts into its recording names the lead it
/// leaves out from its own start, not the recording's (#756).
#[test]
fn an_entrys_lead_is_counted_from_its_own_start() {
    let (_, led) = noise_then_watch(60.0, 40.0);
    let dir = std::env::temp_dir().join(format!("tg-entry-lead-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    timegrapher_core::audio::write_wav(&dir.join("take.wav"), &led).unwrap();
    let m: timegrapher_core::session::Manifest = toml::from_str(
        "bph = 28800\nsettle_s = 0\n\n[[recording]]\nfile = \"take.wav\"\nposition = \"DU\"\nstart_s = 20.0\n",
    )
    .unwrap();
    let s = take::EntrySettings {
        stream: take::stream_settings(&m, None, take::Given::default()),
        card_clock: false,
        settle_s: None,
    };
    let read =
        take::measure_entry(&dir, &m, &m.recordings[0], &s, |_| Ok(None), |_| Ok(None)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(read.start_s, 20.0);
    let lead = lead_left_out(&read.measurement.caveats).expect("the lead named");
    assert!((lead - 40.0).abs() < 1.0, "lead {lead}");
}

/// A long lead of stand noise before a slow watch, where the noise is as
/// loud as half the watch's ticks, does not set the rate (#1423): with
/// no rate set the rate is guessed again as session reads the file, and
/// the recording reads as it does with the rate set; with the rate set,
/// the noise is not heard as another rate. Before, `analyze` guessed from
/// the noise's first 10 s (43,200 or 28,800 bph) and found no watch.
/// The lead ends before the start of the first stream chunk at or after
/// the noise's end, plus one lead window step and 0.5 s: at 14,400 after
/// 120 s it runs to 141 s, where the watch's first 21 s, in a chunk that
/// opens on noise, track too loosely to be kept. Its rate is still the
/// watch's. Seeds 5 and 33 are the ones the lead work was built on.
/// Seeds 3011 to 3019 were run once after the change; 3012 found a jump
/// in the noise taken as the watch's start, across windows missing
/// there. Seeds 3031 to 3039 were run once after that fix.
#[test]
fn a_long_noise_lead_does_not_set_the_rate() {
    let mut cases = vec![
        (14_400, 120.0, 5),
        (18_000, 120.0, 5),
        (19_800, 120.0, 5),
        (14_400, 90.0, 33),
    ];
    for (i, bph) in [14_400, 18_000, 19_800].into_iter().enumerate() {
        for first in [3011, 3031] {
            cases.extend((0..3).map(|k| (bph, 120.0, first + 3 * i as u64 + k)));
        }
    }
    for (bph, noise_s, seed) in cases {
        let lead_to = (noise_s / CHUNK_S).ceil() * CHUNK_S + RATE_WINDOW_STEP_S + 0.5;
        let what = format!("{bph} bph after {noise_s} s of noise ({seed})");
        let (alone, led) = noise_then(noise_s, 60.0, bph, seed);
        let want = analyze(&alone, &AnalysisConfig::default()).summary;
        for set in [None, Some(bph)] {
            let cfg = AnalysisConfig {
                bph: set,
                ..Default::default()
            };
            let got = analyze(&led, &cfg).summary;
            assert_eq!(got.bph, bph, "{what}, set {set:?}");
            assert!(got.watch_heard, "{what}, set {set:?}: {:?}", got.caveats);
            assert_eq!(got.bph_heard, None, "{what}, set {set:?}");
            assert_eq!(got.bph_set, set, "{what}");
            let (r, w) = (got.rate_s_per_day.unwrap(), want.rate_s_per_day.unwrap());
            assert!((r - w).abs() <= 0.3, "{what}: rate {r} against {w}");
            let lead = lead_left_out(&got.caveats).expect("the lead named");
            assert!(
                (noise_s - 5.0..lead_to).contains(&lead),
                "{what}, set {set:?}: lead {lead}"
            );
        }
    }
}

/// The stream's chunk, in seconds: `analyze` guesses again from each.
const CHUNK_S: f64 = 47.0;

/// Stand noise alone, with no rate set, is still no watch and names no
/// lead: guessing again (#1423) gives the noise a second chance.
#[test]
fn stand_noise_alone_is_no_watch_when_guessed_again() {
    let none = Audio {
        samples: Vec::new(),
        sample_rate: watch_at(28_800, 1.0).sample_rate,
    };
    for noise_s in [60.0, 180.0] {
        for seed in [3021, 3022, 3023] {
            let what = format!("{noise_s} s of noise ({seed})");
            let got = analyze(
                &noise_before(noise_s, &none, seed),
                &AnalysisConfig::default(),
            )
            .summary;
            assert!(!got.watch_heard, "{what}: {:?}", got.caveats);
            assert!(lead_left_out(&got.caveats).is_none(), "{what}");
        }
    }
}
