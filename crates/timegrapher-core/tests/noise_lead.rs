//! A recording that opens with the stand's noise before the watch is
//! placed (#844): the whole file reads the watch, with the lead left out
//! and named, as long as the watch is heard for long enough after it.

use timegrapher_core::audio::Audio;
use timegrapher_core::measure::Caveat;
use timegrapher_core::synth::{generate, noise_rms, Rng, SynthConfig, SCALE};
use timegrapher_core::{analyze, AnalysisConfig};

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
/// stays no watch, as a set average over such a stretch does live.
#[test]
fn a_brief_watch_after_a_long_noise_lead_is_still_no_watch() {
    let (_, led) = noise_then_watch(110.0, 15.0);
    let got = analyze(&led, &AnalysisConfig::default()).summary;
    assert!(!got.watch_heard, "{:?}", got.caveats);
    assert!(lead_left_out(&got.caveats).is_none());
}
