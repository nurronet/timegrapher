//! The lead rule's parts one at a time (#1418): `session::watch_lead_s`
//! on scatter windows laid by hand, so each case turns on one part.

use timegrapher_core::session::watch_lead_s;
use timegrapher_core::timing::{TimingFit, WindowFit};

const BPH: u32 = 18_000;

/// 10 s windows from each `(start, scatter)`, the scatter a share of a
/// beat at 18,000 bph.
fn windows(spec: &[(f64, f64)]) -> Vec<WindowFit> {
    let beat_us = 3.6e9 / f64::from(BPH);
    spec.iter()
        .map(|&(start_s, f)| WindowFit {
            start_s,
            end_s: start_s + 10.0,
            fit: TimingFit {
                rate_s_per_day: 0.0,
                beat_error_ms: 0.0,
                period_s: 0.2,
                jitter_us: f * beat_us,
                beats_used: 50,
            },
        })
        .collect()
}

/// Windows every 5 s from `from` to 120 s, all at `f`.
fn run(from: f64, f: f64) -> Vec<(f64, f64)> {
    (0..)
        .map(|i| from + 5.0 * f64::from(i))
        .take_while(|&s| s <= 110.0)
        .map(|s| (s, f))
        .collect()
}

/// Noise at 2% of a beat from 0 to 30 s, then `mid`, then a watch at
/// 0.01% from `watch_s` on.
fn lead(mid: &[(f64, f64)], watch_s: f64) -> Vec<WindowFit> {
    let mut spec = run(0.0, 0.02);
    spec.retain(|w| w.0 <= 30.0);
    spec.extend_from_slice(mid);
    spec.extend(run(watch_s, 0.0001));
    windows(&spec)
}

/// One noise window that passes on its own, with the window clear of it
/// failing, is not the watch: its confirmer must pass too.
#[test]
fn a_lone_passing_noise_window_needs_its_confirmer() {
    let w = lead(&[(35.0, 0.0003), (40.0, 0.02), (45.0, 0.02)], 50.0);
    assert_eq!(watch_lead_s(&w, BPH, &[]), Some(50.0));
}

/// Two overlapping noise windows that pass are not each other's
/// confirmer: the confirmer is the first window clear of the first.
#[test]
fn an_overlapping_window_is_not_a_confirmer() {
    let mid = [(35.0, 0.0003), (40.0, 0.0003), (45.0, 0.02), (50.0, 0.02)];
    assert_eq!(watch_lead_s(&lead(&mid, 55.0), BPH, &[]), Some(55.0));
}

/// A window and its confirmer with a grid window missing between them
/// (here the one across the step onto the watch) do not confirm.
#[test]
fn a_missing_window_between_a_pair_breaks_it() {
    let w = lead(&[(35.0, 0.0003)], 45.0);
    assert_eq!(watch_lead_s(&w, BPH, &[]), Some(45.0));
}

/// No pair may straddle a jump of the beats: with one at 52 s, every pair
/// from 35 s to 50 s has it inside, and the watch starts at the jump.
#[test]
fn a_pair_across_a_jump_does_not_confirm() {
    let mid = [(35.0, 0.0003), (40.0, 0.0003), (45.0, 0.0003)];
    assert_eq!(watch_lead_s(&lead(&mid, 50.0), BPH, &[52.0]), Some(52.0));
}

/// A window over the 0.8% doubt line does not start the watch, though
/// the windows after it are clear by their median and it is within 4
/// times their 0.3%.
#[test]
fn a_window_over_the_doubt_line_is_not_the_watchs() {
    let mut spec = vec![(0.0, 0.02), (5.0, 0.02), (10.0, 0.01)];
    spec.extend(run(15.0, 0.003));
    assert_eq!(watch_lead_s(&windows(&spec), BPH, &[]), Some(15.0));
}

/// A watch whose first window scatters 3 times its later ones (0.05%)
/// has no lead; first windows at 5 times are left out.
#[test]
fn a_first_window_within_4_times_the_rest_is_the_watchs() {
    let with_first = |first: &[f64]| {
        let mut spec: Vec<(f64, f64)> = first
            .iter()
            .enumerate()
            .map(|(i, &f)| (5.0 * i as f64, f))
            .collect();
        spec.extend(run(5.0 * first.len() as f64, 0.0005));
        watch_lead_s(&windows(&spec), BPH, &[])
    };
    assert_eq!(with_first(&[0.0015]), None);
    assert_eq!(with_first(&[0.0025, 0.0025]), Some(10.0));
}

/// The rest is taken as at least 0.02% of a beat: a clean watch at 0.01%
/// whose first window reads 0.06% has no lead (the line is 0.08%, not
/// 0.04%); first windows at 0.1% are left out (the line is not 0.4%).
#[test]
fn a_clean_watchs_first_window_is_held_to_the_floor() {
    let with_first = |first: &[f64]| {
        let mut spec: Vec<(f64, f64)> = first
            .iter()
            .enumerate()
            .map(|(i, &f)| (5.0 * i as f64, f))
            .collect();
        spec.extend(run(5.0 * first.len() as f64, 0.0001));
        watch_lead_s(&windows(&spec), BPH, &[])
    };
    assert_eq!(with_first(&[0.0006]), None);
    assert_eq!(with_first(&[0.001, 0.001]), Some(10.0));
}

/// The watch starts at a jump only between its first window and the one
/// before: a jump back in the noise is not where it starts.
#[test]
fn a_jump_in_the_noise_is_not_where_the_watch_starts() {
    let mid = [(35.0, 0.02), (40.0, 0.02), (45.0, 0.02)];
    assert_eq!(watch_lead_s(&lead(&mid, 50.0), BPH, &[12.0]), Some(50.0));
}

/// Where windows are missing before the watch's first (the noise gave too
/// few beats to fit), a jump in that gap is in the noise: the watch
/// starts at a jump no more than one grid step before its first window
/// (#1423).
#[test]
fn a_jump_across_missing_windows_is_not_where_the_watch_starts() {
    let w = lead(&[], 80.0);
    assert_eq!(watch_lead_s(&w, BPH, &[60.0]), Some(80.0));
    assert_eq!(watch_lead_s(&w, BPH, &[75.0]), Some(80.0));
    assert_eq!(watch_lead_s(&w, BPH, &[77.0]), Some(77.0));
    assert_eq!(watch_lead_s(&w, BPH, &[60.0, 77.0]), Some(77.0));
}
