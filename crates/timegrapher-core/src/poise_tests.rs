use super::*;
use crate::synth::Rng;

fn pt(angle_deg: f64, rate: f64, se: f64) -> Point {
    Point {
        angle_deg,
        rate_s_per_day: rate,
        rate_se: Some(se),
        amplitude_deg: Some(180.0),
        amplitude_se: Some(1.0),
    }
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// Signed difference of two angles, degrees, -180 to 180.
fn angle_off(a: f64, b: f64) -> f64 {
    (a - b + 180.0).rem_euclid(360.0) - 180.0
}

/// Four positions 90° apart with equal ±: the closed form, a = (r0 −
/// r180)/2, b = (r90 − r270)/2, m the mean, SE(a) = ½√(se0² + se180²),
/// and the one check left, the cos 2α term (r0 + r180 − r90 − r270)/4 with
/// SE ¼√Σse².
#[test]
fn four_positions_give_the_closed_form_and_its_se() {
    let (r0, r90, r180, r270) = (-282.1, 46.2, 486.0, 54.0);
    let s = 3.0;
    let f = fit(
        &[
            pt(0.0, r0, s),
            pt(90.0, r90, s),
            pt(180.0, r180, s),
            pt(270.0, r270, s),
        ],
        None,
    )
    .unwrap();
    let (a, b) = ((r0 - r180) / 2.0, (r90 - r270) / 2.0);
    let size = a.hypot(b);
    let w = f.wave;
    assert!(close(w.mean.value, (r0 + r90 + r180 + r270) / 4.0, 1e-9));
    assert!(close(w.size.value, size, 1e-9));
    assert!(close(
        w.peak_deg.value,
        b.atan2(a).to_degrees().rem_euclid(360.0),
        1e-9
    ));
    // The user's 8-angle fit put the fast side at 181°.
    assert!(close(w.peak_deg.value, 180.6, 0.05), "{}", w.peak_deg.value);
    let se_ab = 0.5 * (2.0 * s * s).sqrt();
    assert!(close(w.size.se.unwrap(), se_ab, 1e-9));
    assert!(close(
        w.peak_deg.se.unwrap(),
        (se_ab / size).to_degrees(),
        1e-9
    ));
    let d = f.shape_cos2_s_per_day.unwrap();
    assert!(close(d.value, (r0 + r180 - r90 - r270) / 4.0, 1e-9));
    assert!(close(d.se.unwrap(), (4.0 * s * s).sqrt() / 4.0, 1e-9));
    assert!(
        f.shape_sin2_s_per_day.is_none(),
        "sin 2α is zero at 90° steps"
    );
}

/// Eight positions 45° apart recover a wave and both twice-round terms.
#[test]
fn eight_positions_recover_the_wave_and_both_shape_terms() {
    let rate = |deg: f64| {
        let t = deg.to_radians();
        5.0 + 30.0 * (t - 100f64.to_radians()).cos() + 4.0 * (2.0 * t).cos() - 3.0 * (2.0 * t).sin()
    };
    let pts: Vec<Point> = (0..8)
        .map(|i| pt(45.0 * f64::from(i), rate(45.0 * f64::from(i)), 1.0))
        .collect();
    let f = fit(&pts, None).unwrap();
    assert!(close(f.wave.size.value, 30.0, 1e-9));
    assert!(close(f.wave.peak_deg.value, 100.0, 1e-9));
    assert!(close(f.wave.mean.value, 5.0, 1e-9));
    assert!(close(f.shape_cos2_s_per_day.unwrap().value, 4.0, 1e-9));
    assert!(close(f.shape_sin2_s_per_day.unwrap().value, -3.0, 1e-9));
    assert_eq!(f.positions, 8);
}

/// Three positions fix the wave exactly and leave no check on its shape;
/// two cannot fix it.
#[test]
fn three_positions_fit_without_a_shape_check_and_two_do_not_fit() {
    let rate = |deg: f64| 2.0 + 10.0 * (deg.to_radians() - 1.0).cos();
    let three = [
        pt(0.0, rate(0.0), 1.0),
        pt(90.0, rate(90.0), 1.0),
        pt(180.0, rate(180.0), 1.0),
    ];
    let f = fit(&three, None).unwrap();
    assert!(close(f.wave.size.value, 10.0, 1e-9));
    assert!(close(f.wave.peak_deg.value, 1f64.to_degrees(), 1e-9));
    assert!(f.shape_cos2_s_per_day.is_none() && f.shape_sin2_s_per_day.is_none());
    assert!(fit(&three[..2], None).is_none());
}

/// A position without a ± leaves the fit's figures without one; one
/// without an amplitude leaves the heavy spot out.
#[test]
fn a_missing_se_or_amplitude_is_carried_through() {
    let mut pts: Vec<Point> = (0..4)
        .map(|i| pt(90.0 * f64::from(i), f64::from(i), 1.0))
        .collect();
    pts[2].rate_se = None;
    let f = fit(&pts, None).unwrap();
    assert!(f.wave.size.se.is_none() && f.wave.peak_deg.se.is_none());
    assert!(f.shape_cos2_s_per_day.unwrap().se.is_none());
    pts[1].amplitude_deg = None;
    let f = fit(&pts, None).unwrap();
    assert_eq!(f.withheld, Some(Withheld::NoAmplitude));
    assert!(f.heavy_spot.is_none() && f.amplitude_wave.is_none());
}

/// f = 2·J1(θ)/θ at the ruling's figures.
#[test]
fn the_amplitude_factor_matches_the_ruling() {
    assert!(close(amplitude_factor(0.0), 1.0, 1e-15));
    for (deg, want) in [
        (180.0, 0.181),
        (200.0, 0.08),
        (230.0, -0.036),
        (250.0, -0.088),
        (270.0, -0.12),
    ] {
        let f = amplitude_factor(deg);
        assert!(close(f, want, 0.006), "{deg}: {f}");
    }
    assert!(close(amplitude_factor(NULL_AMPLITUDE_DEG), 0.0, 1e-3));
    // Its deepest, near 300°, about -0.13.
    let deepest = (250..340)
        .map(|d| amplitude_factor(f64::from(d)))
        .fold(f64::INFINITY, f64::min);
    assert!(close(deepest, -0.132, 0.003), "{deepest}");
}

/// The null band: 219.5° within 2 SE plus 8% of the amplitude.
#[test]
fn the_null_band_reaches_two_se_and_eight_percent() {
    assert!(!in_null_band(203.0, Some(0.0)));
    assert!(in_null_band(204.0, Some(0.0)));
    assert!(in_null_band(238.0, None));
    assert!(!in_null_band(239.0, None));
    // A wide ± reaches further.
    assert!(in_null_band(197.0, Some(4.0)));
    assert!(!in_null_band(195.0, Some(4.0)));
    assert!(!in_null_band(180.0, Some(2.0)));
}

#[test]
fn hours_are_named_on_the_half_hour() {
    assert_eq!(hour_code(0.0), "12H");
    assert_eq!(hour_code(181.0), "6H");
    assert_eq!(hour_code(359.0), "12H");
    assert_eq!(hour_code(45.0), "1H30");
    assert_eq!(hour_code(315.0), "10H30");
    assert_eq!(hour_code(-90.0), "9H");
}

/// Synthetic positions: a poise error `u` (s/d at a small swing) with the
/// heavy spot lowest at rest with `psi` at the top, each position at
/// amplitude `amp` plus `amp_wave`·cos(α − 90°), isochronism `iso` s/d per
/// degree, and rate noise of each position's ±.
fn synthetic(
    rng: &mut Rng,
    angles: &[f64],
    (u, psi): (f64, f64),
    (amp, amp_wave): (f64, f64),
    iso: f64,
) -> Vec<Point> {
    angles
        .iter()
        .map(|&deg| {
            let a = amp + amp_wave * (deg - 90.0).to_radians().cos();
            let se = 1.5 + rng.next_f64();
            let truth =
                3.0 + u * amplitude_factor(a) * (deg - psi).to_radians().cos() + iso * (a - amp);
            Point {
                angle_deg: deg,
                rate_s_per_day: truth + se * rng.normal(),
                rate_se: Some(se),
                amplitude_deg: Some(a),
                amplitude_se: Some(1.0),
            }
        })
        .collect()
}

fn eight() -> Vec<f64> {
    (0..8).map(|i| 45.0 * f64::from(i)).collect()
}

/// The ruling's acceptance seeds: the heavy spot is recovered within its
/// ± on both sides of 219.5° (in about 95 of 100 tests at 180° and at
/// 260°), the measured fastest hour turning half round between them, and
/// it is withheld at 225°.
#[test]
fn the_heavy_spot_is_recovered_either_side_of_the_null_and_withheld_in_it() {
    let mut rng = Rng::new(20_261_112);
    let trials = 800;
    for amp in [180.0, 260.0] {
        let mut hits = 0;
        for _ in 0..trials {
            let pts = synthetic(&mut rng, &eight(), (300.0, 130.0), (amp, 0.0), 0.0);
            let f = fit(&pts, None).unwrap();
            let h = f.heavy_spot.expect("outside the null band");
            let off = angle_off(h.peak_deg.value, 130.0);
            hits += usize::from(off.abs() <= 1.96 * h.peak_deg.se.unwrap());
            let fastest = if amp < NULL_AMPLITUDE_DEG {
                130.0
            } else {
                310.0
            };
            assert!(
                angle_off(f.wave.peak_deg.value, fastest).abs() < 30.0,
                "{amp}"
            );
        }
        let rate = hits as f64 / f64::from(trials);
        assert!((0.92..=0.975).contains(&rate), "{amp}°: {rate}");
    }
    let pts = synthetic(&mut rng, &eight(), (300.0, 130.0), (225.0, 0.0), 0.0);
    let f = fit(&pts, None).unwrap();
    assert!(f.heavy_spot.is_none());
    assert!(matches!(f.withheld, Some(Withheld::NullBand { .. })));
    assert_eq!(
        f.heavy_text,
        "At 225°, poise barely moves the rate. Heavy spot not stated."
    );
}

/// Seeded coverage of the measured wave's ±: with a wave (size and peak),
/// and with none (the components a and b, as a size is never below
/// zero), on four and eight positions and with one of four missing.
#[test]
fn the_measured_waves_se_covers_the_true_wave() {
    let mut rng = Rng::new(20_261_113);
    let trials = 800;
    for (angles, u) in [
        (vec![0.0, 90.0, 180.0, 270.0], 120.0),
        (eight(), 120.0),
        (vec![0.0, 90.0, 270.0], 120.0),
        (vec![0.0, 90.0, 180.0, 270.0], 0.0),
        (eight(), 0.0),
    ] {
        let size = u * amplitude_factor(180.0);
        let mut hits = [0usize; 2];
        for _ in 0..trials {
            let pts = synthetic(&mut rng, &angles, (u, 130.0), (180.0, 0.0), 0.0);
            let w = fit(&pts, None).unwrap().wave;
            if u > 0.0 {
                hits[0] += usize::from((w.size.value - size).abs() <= 1.96 * w.size.se.unwrap());
                let off = angle_off(w.peak_deg.value, 130.0);
                hits[1] += usize::from(off.abs() <= 1.96 * w.peak_deg.se.unwrap());
            } else {
                let rad = w.peak_deg.value.to_radians();
                let (a, b) = (w.size.value * rad.cos(), w.size.value * rad.sin());
                let ses: Vec<f64> = pts.iter().map(|p| p.rate_se.unwrap()).collect();
                let (sa, sb) = component_ses(&pts, &ses);
                hits[0] += usize::from(a.abs() <= 1.96 * sa);
                hits[1] += usize::from(b.abs() <= 1.96 * sb);
            }
        }
        for h in hits {
            let rate = h as f64 / f64::from(trials);
            assert!((0.92..=0.975).contains(&rate), "{angles:?} u {u}: {rate}");
        }
    }
}

/// The SEs of the cos and sin coefficients of the weighted fit, worked
/// the long way for the null check.
fn component_ses(pts: &[Point], ses: &[f64]) -> (f64, f64) {
    let rows: Vec<Vec<f64>> = pts
        .iter()
        .map(|p| {
            let t = p.angle_deg.to_radians();
            vec![1.0, t.cos(), t.sin()]
        })
        .collect();
    let y = vec![0.0; pts.len()];
    let (_, cov) = wls(&rows, &y, Some(ses)).unwrap();
    let c = cov.unwrap();
    (c[1][1].sqrt(), c[2][2].sqrt())
}

/// The isochronism alias: no poise error, but the amplitude varies once
/// round the dial and the rate follows it. With no slope known, the
/// amplitude wave is named; with the slope taken off, the heavy spot's
/// size is within its ± of zero.
#[test]
fn an_amplitude_wave_through_isochronism_is_named_or_taken_off() {
    let mut rng = Rng::new(20_261_114);
    let pts = synthetic(&mut rng, &eight(), (0.0, 0.0), (180.0, 12.0), 0.8);
    let f = fit(&pts, None).unwrap();
    assert!(
        matches!(f.caveats.as_slice(), [Caveat::AmplitudeWave { size_deg, .. }] if close(*size_deg, 12.0, 2.0)),
        "{:?}",
        f.caveats
    );
    assert!(
        f.heavy_text
            .contains("so part of the wave may be isochronism."),
        "{}",
        f.heavy_text
    );
    assert!(f.wave.size.value > 6.0, "the alias shows in the rate wave");
    let slope = Slope {
        s_per_day_per_deg: 0.8,
        se: 0.01,
    };
    let f = fit(&pts, Some(slope)).unwrap();
    assert!(matches!(
        f.caveats.as_slice(),
        [Caveat::IsochronismRemoved { .. }]
    ));
    let h = f.heavy_spot.unwrap();
    assert!(h.size.value < 3.0 * h.size.se.unwrap(), "{h:?}");
    // No amplitude wave and no slope: no caveat.
    let flat = synthetic(&mut rng, &eight(), (300.0, 40.0), (180.0, 0.0), 0.0);
    assert!(fit(&flat, None).unwrap().caveats.is_empty());
}

#[test]
fn the_sentence_states_the_wave_then_the_heavy_spot() {
    let mut pts: Vec<Point> = [(-282.1, 0.0), (46.2, 90.0), (486.0, 180.0), (54.0, 270.0)]
        .iter()
        .map(|&(r, a)| pt(a, r, 2.0))
        .collect();
    let f = fit(&pts, None).unwrap();
    assert_eq!(f.text, f.sentence());
    assert_eq!(
        f.text,
        "Once-round wave over 4 upright positions: 384.1 ± 1.4 s/d each way from the mean, \
         fastest at 180.6 ± 0.2° (nearest 6H at the top). Twice-round terms: cos 2α +25.9 ± \
         1.0 s/d. Amplitude 180°. Heavy spot hangs lowest with 6H at the top (180.6 ± 0.2°). The \
         heavy spot is from the fit at each position's own amplitude, so it rests on the lift \
         angle."
    );
    assert_eq!(
        f.short_text,
        "Once-round wave: 384.1 ± 1.4 s/d each way, fastest near 6H at the top · at 180°"
    );
    for p in &mut pts {
        p.amplitude_deg = Some(260.0);
    }
    let f = fit(&pts, None).unwrap();
    assert!(
        f.heavy_text
            .starts_with("Heavy spot hangs lowest with 12H at the top (0.6 ± 0.2°)"),
        "above 219.5° the heavy spot is opposite the fastest hour: {}",
        f.heavy_text
    );
    assert_eq!(pending_text(), "Fitted once 5 positions are measured");
}

/// The drawn curve passes through the fit: fastest at its peak, and it
/// closes on itself.
#[test]
fn the_curve_is_the_measured_wave() {
    let rate = |deg: f64| 7.0 + 12.0 * (deg - 90.0).to_radians().cos();
    let pts: Vec<Point> = (0..4)
        .map(|i| pt(90.0 * f64::from(i), rate(90.0 * f64::from(i)), 1.0))
        .collect();
    let c = fit(&pts, None).unwrap().curve(8);
    assert_eq!(c.len(), 9);
    for &(deg, r) in &c {
        assert!(close(r, rate(deg), 1e-9), "{deg} {r}");
    }
    assert!(close(c[2].1, 19.0, 1e-9), "fastest at 90°");
}

/// Writes the fits the UX mock-ups are drawn from, when asked:
/// `POISE_SAMPLES=dir cargo test -p timegrapher-core poise_samples`.
#[test]
fn poise_samples() {
    let Ok(dir) = std::env::var("POISE_SAMPLES") else {
        return;
    };
    // The reddit user's table: rate and amplitude at their 0° to 315°.
    let user = [
        (-282.1, 196.0),
        (-263.1, 199.0),
        (46.2, 186.0),
        (345.6, 171.0),
        (486.0, 173.0),
        (376.9, 175.0),
        (54.0, 159.0),
        (-255.5, 175.0),
    ];
    let pts = |step: usize| -> Vec<Point> {
        user.iter()
            .enumerate()
            .step_by(step)
            .map(|(i, &(r, a))| Point {
                angle_deg: 45.0 * i as f64,
                rate_s_per_day: r,
                rate_se: Some(3.0),
                amplitude_deg: Some(a),
                amplitude_se: Some(1.0),
            })
            .collect()
    };
    for (name, step) in [("poise-4.json", 2), ("poise-8.json", 1)] {
        let f = fit(&pts(step), None).unwrap();
        let json = serde_json::to_string_pretty(&f).unwrap();
        std::fs::write(format!("{dir}/{name}"), json).unwrap();
    }
}
