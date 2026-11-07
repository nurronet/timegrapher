//! A recording that opens with the stand's noise before the watch is
//! placed (#844): the whole file reads the watch, with the lead left out
//! and named, as long as the watch is heard for long enough after it.

use timegrapher_core::audio::Audio;
use timegrapher_core::measure::Caveat;
use timegrapher_core::synth::{generate, noise_rms, Rng, SynthConfig, SCALE};
use timegrapher_core::{analyze, take, AnalysisConfig};

/// `noise_s` of the synthetic stand noise (as the live tests make it),
/// then `watch_s` of a watch at +5 s/d and 270°.
fn noise_then_watch(noise_s: f64, watch_s: f64) -> (Audio, Audio) {
    let watch = generate(
        &SynthConfig {
            duration_s: watch_s,
            rate_s_per_day: 5.0,
            beat_error_ms: 0.4,
            ..Default::default()
        },
        |_| 270.0,
        |_| 0.0,
    );
    let rms = noise_rms(SynthConfig::default().snr_db) * SCALE;
    let mut rng = Rng::new(5);
    let n = (noise_s * watch.sample_rate as f64) as usize;
    let mut samples: Vec<f32> = (0..n).map(|_| (rms * rng.normal()) as f32).collect();
    samples.extend_from_slice(&watch.samples);
    let led = Audio {
        samples,
        sample_rate: watch.sample_rate,
    };
    (watch, led)
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
