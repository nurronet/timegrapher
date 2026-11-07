//! The cycle a `periodic` headline rests on: its candidates, refined
//! and tested on every core, each peak once per source. Kept beside
//! [`super`] to keep that file under the length cap.

use super::*;

/// The cycle a `periodic` headline would rest on. Candidates come from
/// the two-state finder (a low level that comes back on a period), from
/// a repeat in the autocorrelation and from the period search; each is
/// tested by folding the readings at it, and the one whose cycle explains
/// the most is kept when that is enough. A period shorter than three
/// readings cannot show in the readings and is left to the period search
/// report. With `fine` given, the folds and the autocorrelation's
/// candidates come from its shorter windows.
pub(super) fn cycle(
    s: &SeriesCheck,
    g: &Grid,
    fine: Option<&Grid>,
    wheels: &[Wheel],
    cfg: &Config,
) -> Option<Cycle> {
    let (d, refined) = refined_candidates(s, g, fine, cfg);
    let cands = same_peaks_once(&refined, d.span());
    strongest_cycle(s, fine.unwrap_or(g), &d, &cands, wheels, cfg)
}

/// The readings [`cycle`] folds (detrended), and each candidate period
/// refined, with its source and fold share, in the order found.
pub(super) fn refined_candidates(
    s: &SeriesCheck,
    g: &Grid,
    fine: Option<&Grid>,
    cfg: &Config,
) -> (Grid, Vec<(f64, CycleSource, f64)>) {
    let fine_ac = fine.map(|f| detrended_autocorrelation(f, false, cfg));
    let ac = fine_ac.as_ref().or(s.autocorrelation.as_ref());
    let g = fine.unwrap_or(g);
    let d = longterm::detrend(g, longterm::DETREND_S);
    // Each candidate's period, how far around it to refine, and where it
    // came from; refined below, several at once.
    let mut cands: Vec<(f64, f64, CycleSource)> = Vec::new();
    if let Some(t) = &s.two_state {
        if t.repeating_levels() {
            if let Some(p) = t.period_s {
                cands.push((p, 0.05, CycleSource::TwoState));
            }
        }
    }
    // In the pane, the autocorrelation only refines a cycle the period
    // search kept: its peaks carry no p-value for Holm's correction (#659).
    let ac = ac.filter(|_| cfg.acf_alone || s.period.is_some());
    if let Some(l) = ac.and_then(|a| a.repeat_lag_s) {
        // The lag is only as fine as one reading: try around it.
        cands.push((l, 0.2, CycleSource::Autocorrelation));
    }
    // Every later peak of the autocorrelation above three times its 95% band, even one
    // too weak to count as a repeat on its own: the fold below decides. The
    // band at a lag is [`band_at_lag`], from the pairs `band_by_lag` was read
    // from.
    if let Some(a) = ac {
        let ng = d.y.len() as f64;
        let band_at =
            |k: usize| band_at_lag((1.96 / a.band_by_lag[k]).powi(2), a.lag_s[k] / d.step, ng);
        for k in 2..a.r.len().saturating_sub(1) {
            if a.r[k] >= a.r[k - 1]
                && a.r[k] > a.r[k + 1]
                && band_at(k).is_some_and(|b| a.r[k] >= 3.0 * b)
            {
                cands.push((a.lag_s[k], 0.2, CycleSource::Autocorrelation));
            }
        }
    }
    if let Some(p) = &s.period {
        for &q in std::iter::once(&p.period_s).chain(&p.other_periods_s) {
            cands.push((q, 0.01, CycleSource::PeriodSearch));
        }
    }
    let refined = crate::cores::on_every_core(&cands, |&(p, rel, src)| {
        let q = refine(&d, p, rel);
        (q, src, fold(&d, q).0)
    });
    (d, refined)
}

/// The strongest of `cands` (periods on the detrended readings `d`, with
/// their sources) that explains enough, each first reduced to its
/// shortest whole fraction; `g` is the grid `d` came from.
pub(super) fn strongest_cycle(
    s: &SeriesCheck,
    g: &Grid,
    d: &Grid,
    cands: &[(f64, CycleSource)],
    wheels: &[Wheel],
    cfg: &Config,
) -> Option<Cycle> {
    let at = |p: f64, src: CycleSource| cycle_at(d, p, src, wheels);
    // Folding at five turns of a cycle explains as much as folding at one,
    // and the autocorrelation's lag grid can favour the multiple (a 48 s
    // cycle in 10 s readings lines up best at 240 s). Each candidate is
    // first reduced to its shortest whole fraction that explains at least
    // 85% as much; then the strongest wins. Reducing first keeps a slower,
    // stronger cycle from being swapped for a weaker fraction of another.
    let reduce = |best: Cycle| -> Cycle {
        // A fraction of a fraction too (540 s to 180 s to 60 s).
        let mut chosen = best;
        while let Some(c) = (2..=8u32)
            .rev()
            .map(|k| chosen.period_s / k as f64)
            .filter(|&p| p >= 3.0 * g.step)
            .map(|p| at(refine(d, p, 0.03), chosen.source))
            .find(|c| c.explained >= 0.85 * chosen.explained)
        {
            chosen = c;
        }
        // When the longer period still wins, the shorter cycle inside it
        // can be the stronger single cycle: its shape changes from turn to
        // turn and repeats every few turns. Name it when it explains at
        // least half as much on its own, fits three or more times (half the
        // period would catch the cycle's own second harmonic) and is at
        // least 30 s (faster lines are the timing search's, in ms).
        if let Some(inner) = (3..=8u32)
            .map(|k| chosen.period_s / k as f64)
            .filter(|&p| p >= 3.0 * g.step && p >= crate::longrun::TIMING_UNIT_BELOW_S)
            .map(|p| at(refine(d, p, 0.03), chosen.source))
            .max_by(|a, b| a.explained.total_cmp(&b.explained))
        {
            if inner.explained >= 0.5 * chosen.explained {
                chosen.inner_period_s = Some(inner.period_s);
                chosen.inner_explained = Some(inner.explained);
                chosen.turns = Some((chosen.period_s / inner.period_s).round() as u32);
            }
        }
        chosen
    };
    // Each candidate is tested on its own, several at once; the strongest
    // is kept as when they were tested in turn (the last of equals).
    let tested = crate::cores::on_every_core(cands, |&cand| {
        std::iter::once(cand)
            // At least three turns for the period search, which tests its own
            // significance; five for the others.
            .filter(|(p, src)| {
                let turns = if *src == CycleSource::PeriodSearch {
                    3.0
                } else {
                    5.0
                };
                *p >= 3.0 * g.step && *p <= g.span() / turns
            })
            .map(|(p, src)| at(p, src))
            .filter(|c| c.explained >= cfg.periodic_min_explained)
            .map(|c| {
                // A line the period search lists stays itself when reducing
                // would move it onto a fast line's alias or its turns: the
                // fraction then holds the alias, not the line.
                let r = reduce(c.clone());
                let listed = |p| slow_search_line(p, g.step, d.span(), s.period.as_ref());
                if listed(c.period_s)
                    && !listed(r.period_s)
                    && alias_of_fast_line(r.period_s, g.step, d.span(), s.period.as_ref()).is_some()
                {
                    c
                } else {
                    r
                }
            })
            .filter(|c| c.explained >= cfg.periodic_min_explained)
            .filter(|c| clears_alias(c.period_s, c.explained, d, s.period.as_ref()))
            .map(|mut c| {
                // At a turn and a half or more of an alias, the fold holds the
                // alias too: its share and size are given net of the alias's own
                // fold. On the alias itself the two cannot be parted.
                if let Some(a) = alias_of_fast_line(c.period_s, g.step, d.span(), s.period.as_ref())
                {
                    if c.period_s / a >= 1.25 {
                        let (rest, kept) = without_fold(d, refine(d, a, 0.03));
                        let (e, ptp) = sized(&rest, c.period_s);
                        c.explained = e * kept;
                        c.peak_to_peak = ptp;
                    }
                }
                c
            })
            .find(|c| c.explained >= cfg.periodic_min_explained)
    });
    tested
        .into_iter()
        .flatten()
        .max_by(|a, b| a.explained.total_cmp(&b.explained))
        .map(|c| half_cycle(&c, d, s.period.as_ref(), wheels, cfg).unwrap_or(c))
}

/// Candidates that refined onto the same peak, each `(period, source,
/// fold share)`, tested once (Norm's ruling on the cycle search): the one
/// with the best fold share is kept, and with it every other from the
/// same source within the fold's resolution at its period, `p²/(20·span)`
/// (the step [`refine`] stops at), with no chaining. Only the same source
/// merges, as the source sets how many turns the take must hold (a
/// 240 s cycle in 20 min passes as the period search's, not as the
/// autocorrelation's). The kept candidates stay in the order found.
pub(super) fn same_peaks_once(
    refined: &[(f64, CycleSource, f64)],
    span: f64,
) -> Vec<(f64, CycleSource)> {
    let mut by_share: Vec<usize> = (0..refined.len()).collect();
    // Best fold first; of equals the last found, as the strongest of
    // equal cycles was the last when each was tested.
    by_share.sort_by(|&a, &b| refined[b].2.total_cmp(&refined[a].2).then(b.cmp(&a)));
    let mut taken = vec![false; refined.len()];
    let mut kept = Vec::new();
    for &i in &by_share {
        if taken[i] {
            continue;
        }
        let p = refined[i].0;
        let res = p * p / (20.0 * span.max(p));
        for (j, t) in taken.iter_mut().enumerate() {
            if !*t && refined[j].1 == refined[i].1 && (refined[j].0 - p).abs() <= res {
                *t = true;
            }
        }
        kept.push(i);
    }
    kept.sort_unstable();
    kept.into_iter()
        .map(|i| (refined[i].0, refined[i].1))
        .collect()
}
