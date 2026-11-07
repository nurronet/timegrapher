//! The beats' sound kept across a whole take for the no-watch verdict:
//! each kept beat's envelope window ([`crate::profile::snippet`]), sampled
//! evenly in time within a fixed budget whatever the take's length, so a
//! stretch anywhere in a take can be judged by its sound
//! ([`crate::measure::stretch_beat_sound`]), not only one in its first
//! minute. One sample, in core: a saved reading's log, `analyze`, the
//! app's whole take and live's verdict all read it.

/// The most beats the sample keeps: about 2 MB of envelope windows at
/// 48 kHz, 4 MB at 96 kHz, within the live engine's memory budget. Over
/// a stretch of any length that is up to a few hundred beats, far more
/// than an averaged profile needs.
pub const SOUND_SAMPLE_BEATS: usize = 384;

/// The sample's first spacing, seconds: a beat in each of these cells,
/// so every beat up to 16 a second until the budget is reached (the
/// first 24 s or more).
const FIRST_GAP_S: f64 = 1.0 / 16.0;

/// Beats kept one per cell of [`Self::gap_s`] seconds of the take, the
/// first in each; when more than [`SOUND_SAMPLE_BEATS`] are kept the cells
/// double and each keeps its first again, so the kept beats stay spread
/// evenly over everything heard. `T` is what is kept with each beat's
/// time. Beats are pushed in time order.
#[derive(Debug, Clone)]
pub struct SoundSample<T> {
    gap_s: f64,
    items: Vec<(f64, T)>,
}

impl<T> Default for SoundSample<T> {
    fn default() -> Self {
        SoundSample {
            gap_s: FIRST_GAP_S,
            items: Vec::new(),
        }
    }
}

impl<T> SoundSample<T> {
    fn cell(t: f64, gap: f64) -> i64 {
        (t / gap).floor() as i64
    }

    /// Keep the beat at `t` (seconds, at or after the last pushed) if its
    /// cell holds none yet, then thin to the budget.
    pub fn push(&mut self, t: f64, v: T) {
        let g = self.gap_s;
        if self
            .items
            .last()
            .is_some_and(|l| Self::cell(l.0, g) >= Self::cell(t, g))
        {
            return;
        }
        self.items.push((t, v));
        while self.items.len() > SOUND_SAMPLE_BEATS {
            self.gap_s *= 2.0;
            let g = self.gap_s;
            let mut last = None;
            self.items.retain(|(t, _)| {
                let c = Self::cell(*t, g);
                let keep = last != Some(c);
                last = Some(c);
                keep
            });
        }
    }

    /// The kept beats, oldest first.
    pub fn items(&self) -> &[(f64, T)] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut [(f64, T)] {
        &mut self.items
    }

    pub fn into_items(self) -> Vec<(f64, T)> {
        self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn retain(&mut self, f: impl FnMut(&(f64, T)) -> bool) {
        self.items.retain(f);
    }

    /// Forget every beat and start again at the first spacing.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 80 hours of beats at 10 a second: never more than the budget, and
    /// the kept beats spread evenly over the whole run, the spacing between
    /// neighbours within a factor of two of the widest.
    #[test]
    fn a_long_run_is_kept_evenly_within_the_budget() {
        let mut s = SoundSample::default();
        let n = 80 * 3600 * 10;
        for i in 0..n {
            s.push(i as f64 * 0.1, ());
            assert!(s.len() <= SOUND_SAMPLE_BEATS);
        }
        let t: Vec<f64> = s.items().iter().map(|i| i.0).collect();
        assert!(t.len() > SOUND_SAMPLE_BEATS / 2, "{}", t.len());
        assert_eq!(t[0], 0.0);
        let last = (n - 1) as f64 * 0.1;
        let gaps: Vec<f64> = t.windows(2).map(|w| w[1] - w[0]).collect();
        let (lo, hi) = gaps
            .iter()
            .fold((f64::INFINITY, 0.0f64), |(a, b), &g| (a.min(g), b.max(g)));
        assert!(hi <= 2.0 * lo + 0.2, "{lo} {hi}");
        assert!(last - t[t.len() - 1] <= hi, "the end is kept too");
    }

    /// Under the budget every beat is kept.
    #[test]
    fn a_short_take_keeps_every_beat() {
        let mut s = SoundSample::default();
        for i in 0..SOUND_SAMPLE_BEATS {
            s.push(i as f64 * 0.125, i);
        }
        assert_eq!(s.len(), SOUND_SAMPLE_BEATS);
    }
}
