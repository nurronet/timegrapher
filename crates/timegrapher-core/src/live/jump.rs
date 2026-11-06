//! The watch verdict either side of a jump in the beats.

use super::*;

/// How long the watch verdict judges a reading ending at `end_s` over,
/// seconds: none while no scatter window fits (`fitted` false) within
/// [`SCATTER_LOOKBACK_S`] of the latest heard break, `heard_since`, or of
/// `jump_since`, a jump in the beats out of a watch heard: the windows
/// skip the jump, so until one fits after it there may be none, and none
/// is no verdict of noise (#1170). Past that, having none is.
pub(super) fn judged_s(
    fitted: bool,
    end_s: f64,
    window_from: f64,
    heard_since: f64,
    jump_since: f64,
) -> f64 {
    if !fitted && end_s - jump_since.max(heard_since) < SCATTER_LOOKBACK_S {
        0.0
    } else {
        (end_s - window_from.max(heard_since)).max(0.0)
    }
}

impl LiveAnalyzer {
    /// The latest of `holes` by `end_s`, within [`SCATTER_LOOKBACK_S`] and
    /// after `heard_since`, when the sound before it was a watch: its
    /// scatter windows over the look-back before it are regular. Out of
    /// noise onto a watch it is no such jump, and the short run after it
    /// stays no watch (#679, #680; #1250's tiling). Minus infinity with
    /// none.
    pub(super) fn jump_after_a_watch(
        &self,
        holes: &Cuts,
        bph: u32,
        end_s: f64,
        heard_since: f64,
    ) -> f64 {
        let cut = holes.stretch_from(end_s);
        if cut <= heard_since || end_s - cut >= SCATTER_LOOKBACK_S {
            return f64::NEG_INFINITY;
        }
        let from = (cut - SCATTER_LOOKBACK_S).max(heard_since).max(0.0);
        let before = crate::session::scatter_windows(
            holes,
            &self.heard_peaks(from, cut),
            bph,
            from,
            cut,
            crate::session::window_part_s(&self.amp),
        );
        let jitter = crate::session::scatter_jitter_us(&before);
        if crate::beats::watch_heard(jitter, bph) {
            cut
        } else {
            f64::NEG_INFINITY
        }
    }
}
