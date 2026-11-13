//! The ± borrowing and fixed places of #1470, read as the app reads a
//! live take: every second.

use timegrapher_core::live::{LiveAnalyzer, LiveConfig};
use timegrapher_core::synth::{generate, SynthConfig};

/// A live 10 s average read every second for 10 minutes of a steady
/// watch keeps its figures at one place, and its ± never under one
/// unit of it (#1470): the card can't flick to 148.94° ± 0.01°.
#[test]
fn a_live_figure_keeps_its_place_as_its_plus_minus_moves() {
    use timegrapher_core::figure_text::{base_digits as base, figure_with_pm};
    let audio = generate(
        &SynthConfig {
            duration_s: 600.0,
            seed: 1470,
            ..Default::default()
        },
        |_| 280.0,
        |_| 0.0,
    );
    let mut a = LiveAnalyzer::new(audio.sample_rate, LiveConfig::default());
    let second = audio.sample_rate as usize;
    let decimals = |t: &str| t.split_once('.').map_or(0, |(_, d)| d.len());
    let (mut amplitudes, mut rates) = (0, 0);
    for chunk in audio.samples.chunks(second) {
        for block in chunk.chunks(480) {
            a.push(block);
        }
        let r = a.reading(10.0);
        for (v, se, place, signed, n) in [
            (
                r.amplitude_deg,
                r.amplitude_error_deg,
                base::AMPLITUDE,
                false,
                &mut amplitudes,
            ),
            (
                r.rate_s_per_day,
                r.rate_error_s_per_day,
                base::RATE,
                true,
                &mut rates,
            ),
        ] {
            let Some(v) = v else { continue };
            let text = figure_with_pm(v, se, place, signed);
            let mut parts = text.split(" ± ");
            let figure = parts.next().unwrap();
            assert_eq!(decimals(figure), place, "{text}");
            if let Some(pm) = parts.next() {
                assert_eq!(decimals(pm), place, "{text}");
                let unit = 10f64.powi(-(place as i32));
                assert!(pm.parse::<f64>().unwrap() >= unit - 1e-12, "{text}");
                *n += 1;
            }
        }
    }
    // Most seconds carried a ± to hold to the rule: the amplitude's from
    // about its first minute, the rate's from about its fifth, once the
    // take holds the 30 values each takes its scatter from (#1470).
    assert!(amplitudes > 500 && rates > 250, "{amplitudes} {rates}");
}
