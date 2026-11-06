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
        assert!(between(&live) > 0);
        let (a, b) = if later_first { (t2, t1) } else { (t1, t2) };
        live.jump_at(a, false);
        assert!(
            live.sides.pending().is_some(),
            "a check from the first jump"
        );
        live.jump_at(b, false);
        assert!(!live.sides_known_at(t1 + 0.5));
        assert_eq!(between(&live), 0, "kept for the profiles");
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
