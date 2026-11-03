//! Automatic rate inference with alternating beat intervals (#477).

use timegrapher_core::beats::STANDARD_BPH;
use timegrapher_core::synth::{generate, SynthConfig};
use timegrapher_core::{analyze, AnalysisConfig};

fn sweep(fast: bool) {
    let mut wrong = Vec::new();
    for bph in STANDARD_BPH
        .into_iter()
        .filter(|&b| b >= 18_000 && (b >= 28_800) == fast)
    {
        for (seed, error) in [1, 477, 991]
            .into_iter()
            .flat_map(|seed| (0..=9).map(move |error| (seed, error)))
        {
            let audio = generate(
                &SynthConfig {
                    duration_s: 30.0,
                    bph,
                    beat_error_ms: error as f64,
                    rate_s_per_day: 5.0,
                    seed,
                    ..Default::default()
                },
                |_| 270.0,
                |_| 0.0,
            );
            let summary = analyze(&audio, &AnalysisConfig::default()).summary;
            if summary.bph != bph {
                wrong.push(format!(
                    "seed {seed}, {bph} bph, {error} ms: heard {}",
                    summary.bph
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn slower_standard_rates_keep_their_rate_with_large_beat_error() {
    sweep(false);
}

#[test]
fn faster_standard_rates_keep_their_rate_with_large_beat_error() {
    sweep(true);
}
