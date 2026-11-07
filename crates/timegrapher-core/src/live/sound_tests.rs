//! The beats' sound live keeps: the profiles' recent span and the
//! verdict's sample across the take.

use super::tests::{feed, steady_live};
use super::*;

/// The beats of a stretch between two jumps too close to check join
/// neither side's Tick Tock profile, whichever of the two is found
/// first.
#[test]
fn an_unchecked_stretch_joins_no_profile() {
    for later_first in [false, true] {
        let (mut live, rest) = steady_live(45.0, 30.0);
        live.set_profile_span(MAX_PROFILE_S);
        let now = live.duration_s();
        let (t1, t2) = (now - 2.0, now - 1.0);
        let between = |live: &LiveAnalyzer| {
            live.snippets
                .iter()
                .filter(|s| t1 <= s.0 && s.0 < t2)
                .count()
        };
        let sampled = |live: &LiveAnalyzer| {
            live.sample
                .items()
                .iter()
                .filter(|s| t1 <= s.0 && s.0 < t2)
                .count()
        };
        assert!(between(&live) > 0);
        assert!(sampled(&live) > 0);
        let (a, b) = if later_first { (t2, t1) } else { (t1, t2) };
        live.jump_at(a, false);
        assert!(
            live.sides.pending().is_some(),
            "a check from the first jump"
        );
        live.jump_at(b, false);
        assert!(!live.sides_known_at(t1 + 0.5));
        assert_eq!(between(&live), 0, "kept for the profiles");
        assert_eq!(sampled(&live), 0, "kept for the verdict");
        // Once the sides are told apart again, the profiles are made of
        // the kept sounds alone.
        feed(&mut live, &rest);
        assert!(live.sides.pending().is_none(), "sides settled");
        assert_eq!(between(&live), 0);
        let used: usize = live.tick_profiles().iter().flatten().map(|p| p.beats).sum();
        assert!(used > 0);
        assert_eq!(used, live.snippets.len());
    }
}

/// After the sides are swapped back, the verdict's sample names each
/// beat's side as the beat's count does.
pub(super) fn sample_sides_agree(live: &LiveAnalyzer, case: &str) {
    let mut shared = 0;
    for k in live.sample.items() {
        if let Some(b) = live.beats.iter().find(|b| b.time == k.0) {
            assert_eq!(
                k.1,
                b.index.rem_euclid(2) == 0,
                "{case}: the side at {} s",
                k.0
            );
            shared += 1;
        }
    }
    assert!(shared > 0, "{case}: no beat kept");
}

/// What live hands a saved log ([`LiveAnalyzer::snippets`]) is the
/// take's sample, from its start, not the profiles' recent span.
#[test]
fn live_gives_the_sound_of_the_whole_take() {
    let (live, _) = steady_live(2.0 * MAX_PROFILE_S, 2.0 * MAX_PROFILE_S);
    let kept = live.snippets();
    let end = live.duration_s();
    let (first, last) = (kept[0].0, kept[kept.len() - 1].0);
    assert!(first < end - 1.5 * MAX_PROFILE_S, "{first} of {end}");
    assert!(last > end - 1.0, "{last} of {end}");
}

/// Stand noise whose beats are put back on a near-even grid, so its
/// timing scatter is in the doubt zone and only its sound can say no
/// watch: 5 min of it, opened from the file.
fn noise_in_the_doubt_zone() -> LiveAnalyzer {
    let mut log = crate::stream::analyze_samples(
        &crate::edge_takes::silence(300.0, 1),
        48_000,
        &Default::default(),
    );
    // The noise's own pace, from its first beat to its last.
    let (first, last) = (log.beats[0], log.beats[log.beats.len() - 1]);
    let beat = (last.time - first.time) / (last.index - first.index) as f64;
    let mut moved = std::collections::HashMap::new();
    for b in &mut log.beats {
        let k = b.index - first.index;
        let t =
            first.time + k as f64 * beat + 0.0008 * ((k * 7919).rem_euclid(13) as f64 / 6.0 - 1.0);
        moved.insert(b.time.to_bits(), t);
        b.time = t;
    }
    for s in &mut log.snippets {
        s.0 = moved[&s.0.to_bits()];
    }
    LiveAnalyzer::from_log(log, Default::default())
}

/// Once live keeps only a minute of sound for the profiles, its verdict
/// still judges 5 min of stand noise by the take's sample, as a saved
/// reading does, and reads no watch.
#[test]
fn the_verdict_judges_the_whole_stretch_not_the_profiles_span() {
    let mut live = noise_in_the_doubt_zone();
    let r = live.reading_at(300.0, 300.0);
    let scatter = r.timing_scatter_fraction.expect("a scatter");
    assert!((0.008..0.02).contains(&scatter), "{scatter}");
    live.set_profile_span(MAX_PROFILE_S);
    let beat = 3600.0 / live.bph.expect("a rate") as f64;
    live.update_profiles(300.0, beat);
    let profiles_from = live.snippets.front().expect("a beat kept").0;
    assert!(
        profiles_from > 300.0 - MAX_PROFILE_S - 1.0,
        "{profiles_from}"
    );
    assert!(!live.reading_at(300.0, 300.0).watch_heard);
}

/// A new watch's verdict hears none of the last watch's sound.
#[test]
fn a_restart_forgets_the_takes_sound() {
    let (mut live, _) = steady_live(20.0, 20.0);
    assert!(!live.snippets().is_empty());
    live.restart();
    assert!(live.snippets().is_empty());
}
