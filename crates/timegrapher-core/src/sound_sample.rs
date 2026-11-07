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

/// Beats kept one of each side per cell of [`Self::gap_s`] seconds of the
/// take, the first of that side in each; when more than
/// [`SOUND_SAMPLE_BEATS`] are kept the cells double and each keeps its
/// first of each side again, so the kept beats stay spread evenly over
/// everything heard and both Tick and Tock are kept. Cells hold a whole
/// number of beats once they are wider than a beat, so keeping only the
/// first in each would keep one side only. Each kept beat is its time,
/// its side (even or not) and its `T`. Beats are pushed in time order.
#[derive(Debug, Clone)]
pub struct SoundSample<T> {
    gap_s: f64,
    items: Vec<(f64, bool, T)>,
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

    /// Keep the beat at `t` (seconds, at or after the last pushed) on side
    /// `even` if its cell holds none of that side yet, then thin to the
    /// budget.
    pub fn push(&mut self, t: f64, even: bool, v: T) {
        let g = self.gap_s;
        let c = Self::cell(t, g);
        let held = self
            .items
            .iter()
            .rev()
            .take_while(|k| Self::cell(k.0, g) >= c)
            .any(|k| k.1 == even);
        if held {
            return;
        }
        self.items.push((t, even, v));
        while self.items.len() > SOUND_SAMPLE_BEATS {
            self.gap_s *= 2.0;
            let g = self.gap_s;
            let mut seen = [None, None];
            self.items.retain(|(t, even, _)| {
                let c = Some(Self::cell(*t, g));
                let last = &mut seen[usize::from(*even)];
                let keep = *last != c;
                *last = c;
                keep
            });
        }
    }

    /// The sides counted the other way round from `t` on, as a swap back
    /// of the sides renumbers the beats ([`crate::beats::SideCall`]).
    pub fn swap_sides_from(&mut self, t: f64) {
        for k in self.items.iter_mut().filter(|k| k.0 >= t) {
            k.1 = !k.1;
        }
    }

    /// The kept beats, oldest first.
    pub fn items(&self) -> &[(f64, bool, T)] {
        &self.items
    }

    /// Each kept beat's time and `T`, oldest first.
    pub fn into_items(self) -> Vec<(f64, T)> {
        self.items.into_iter().map(|(t, _, v)| (t, v)).collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn retain(&mut self, f: impl FnMut(&(f64, bool, T)) -> bool) {
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
    /// each side's kept beats spread evenly over the whole run, the spacing
    /// between neighbours within a factor of two of the widest.
    #[test]
    fn a_long_run_is_kept_evenly_within_the_budget() {
        let mut s = SoundSample::default();
        let n = 80 * 3600 * 10;
        for i in 0..n {
            s.push(i as f64 * 0.1, i % 2 == 0, ());
            assert!(s.len() <= SOUND_SAMPLE_BEATS);
        }
        assert!(s.len() > SOUND_SAMPLE_BEATS / 2, "{}", s.len());
        let last = (n - 1) as f64 * 0.1;
        for even in [true, false] {
            let t: Vec<f64> = s
                .items()
                .iter()
                .filter(|k| k.1 == even)
                .map(|k| k.0)
                .collect();
            assert!(t[0] <= 0.1, "{even}: {}", t[0]);
            let gaps: Vec<f64> = t.windows(2).map(|w| w[1] - w[0]).collect();
            let (lo, hi) = gaps
                .iter()
                .fold((f64::INFINITY, 0.0f64), |(a, b), &g| (a.min(g), b.max(g)));
            assert!(hi <= 2.0 * lo + 0.2, "{even}: {lo} {hi}");
            assert!(last - t[t.len() - 1] <= hi, "{even}: the end is kept too");
        }
    }

    /// Under the budget every beat is kept.
    #[test]
    fn a_short_take_keeps_every_beat() {
        let mut s = SoundSample::default();
        for i in 0..SOUND_SAMPLE_BEATS {
            s.push(i as f64 * 0.125, i % 2 == 0, i);
        }
        assert_eq!(s.len(), SOUND_SAMPLE_BEATS);
    }

    /// A steady watch's beats fill whole cells once the cells are wider
    /// than a beat (1/8 s at 28,800 bph), yet both sides stay kept, evenly
    /// over the take, and about as many of each.
    #[test]
    fn both_sides_are_kept_on_a_steady_watch() {
        for (bph, hours) in [(28_800u32, 1.0), (21_600, 8.0), (18_000, 80.0)] {
            let beat = 3600.0 / bph as f64;
            let n = (hours * 3600.0 / beat) as usize;
            let mut s = SoundSample::default();
            for i in 0..n {
                s.push(i as f64 * beat, i % 2 == 0, i);
                assert!(s.len() <= SOUND_SAMPLE_BEATS);
            }
            assert!(s.gap_s > 2.0 * beat, "{bph}: cells wider than two beats");
            let even = s.items().iter().filter(|k| k.1).count();
            let odd = s.len() - even;
            assert!(even.abs_diff(odd) <= 1, "{bph}: {even} even, {odd} odd");
            for k in s.items() {
                assert_eq!(k.1, k.2 % 2 == 0, "{bph}: the side kept with its beat");
            }
            let last = s.items().last().map(|k| k.0).unwrap();
            assert!(
                (n - 1) as f64 * beat - last <= s.gap_s,
                "{bph}: the end is kept"
            );
        }
    }

    /// A swap of the sides counts every kept beat from then on the other
    /// way round, and none before.
    #[test]
    fn a_swap_turns_the_sides_from_its_time() {
        let mut s = SoundSample::default();
        for i in 0..40 {
            s.push(i as f64 * 0.125, i % 2 == 0, ());
        }
        s.swap_sides_from(2.5);
        for (i, k) in s.items().iter().enumerate() {
            assert_eq!(k.1, (i % 2 == 0) != (k.0 >= 2.5), "{}", k.0);
        }
    }
}
