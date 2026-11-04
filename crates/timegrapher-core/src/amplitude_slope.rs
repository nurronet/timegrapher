//! The rate's slope against a noisy amplitude (#651): the amplitude each
//! rate reading is paired with and its noise (#951), and a least-squares
//! slope corrected for it, with its standard error. The isochronism curve and
//! the rate against the amplitude in [`crate::steadiness`] read their
//! slopes here.

use crate::confidence::{self, Slow};
use crate::library::signed_text;
use crate::longterm::{self, Grid};
use crate::steadiness::{chi2_tail, drop_outliers, finite_or_nan, p_text, Config};
use serde::Serialize;

/// The least-squares rate's weight on the stretch of its span from `u0`
/// to `u1`, each a fraction of the span from 0 to 1. A line fitted
/// through evenly spaced beat times weights the rate beat by beat as
/// 6u(1 − u), so its weight on a stretch is the integral
/// F(u1) − F(u0), with F(u) = 3u² − 2u³ (#951). Five 2 s windows of a
/// 10 s reading take 0.104, 0.248, 0.296, 0.248 and 0.104.
pub(crate) fn rate_fit_weight(u0: f64, u1: f64) -> f64 {
    let f = |u: f64| {
        let u = u.clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    f(u1) - f(u0)
}

/// The amplitude each rate reading is paired with for the slopes, and its
/// noise variance (#951).
///
/// `medians` are the readings `steadiness` shows: one for each rate
/// reading, NaN where a reading is left out. The pairing reading is the
/// mean of the same windows of `fine`, each weighted by
/// [`rate_fit_weight`] over its span. A short event therefore moves it as
/// far as it moves the rate, which neither the median nor a plain mean
/// does.
///
/// The window σ is read once over the take as 1.4826·MAD/√2 of the
/// successive differences between windows within a reading, so an
/// event's edge inside a reading barely moves it. Each reading's noise is
/// σ²·Σw², with the weights summing to 1: about 0.232 for five windows.
/// The windows' noise is taken as independent from one window to the
/// next. Noise is NaN with fewer than four differences.
pub(crate) fn pairing_readings(fine: &Grid, medians: &Grid) -> (Grid, Vec<f64>) {
    let n = medians.y.len();
    let half = 0.5 * fine.step / medians.step;
    // Σw·v, Σw and Σw² of each reading.
    let mut acc = vec![(0.0, 0.0, 0.0); n];
    let mut diffs = Vec::new();
    let mut last: Option<(usize, f64)> = None;
    for (i, &v) in fine.y.iter().enumerate() {
        let u = (fine.time(i) - medians.t0) / medians.step;
        let b = u.floor() as usize;
        if !(v.is_finite() && u >= 0.0 && b < n && medians.y[b].is_finite()) {
            last = None;
            continue;
        }
        let f = u - b as f64;
        let w = rate_fit_weight(f - half, f + half);
        acc[b].0 += w * v;
        acc[b].1 += w;
        acc[b].2 += w * w;
        if let Some((_, lv)) = last.filter(|l| l.0 == b) {
            diffs.push(v - lv);
        }
        last = Some((b, v));
    }
    let var = if diffs.len() >= 4 {
        (crate::dsp::robust_sd(&diffs) / std::f64::consts::SQRT_2).powi(2)
    } else {
        f64::NAN
    };
    let ok = |a: &(f64, f64, f64)| a.1 > 0.0;
    let y = acc
        .iter()
        .map(|a| if ok(a) { a.0 / a.1 } else { f64::NAN })
        .collect();
    let noise = acc
        .iter()
        .map(|a| {
            if ok(a) {
                var * a.2 / (a.1 * a.1)
            } else {
                f64::NAN
            }
        })
        .collect();
    (
        Grid {
            t0: medians.t0,
            step: medians.step,
            y,
        },
        noise,
    )
}

/// A slope of y on x read from noisy x: the least-squares slope divided
/// by its reliability ratio λ = 1 − (mean noise variance of x) / (x's
/// variance), which undoes its shrinking towards 0 by the noise in x
/// (errors in variables, Fuller 1987), with its standard error.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EivSlope {
    pub slope: f64,
    pub se: f64,
    pub reliability: f64,
}

/// Below this reliability ratio the noise in the amplitude is too much
/// of its spread to read a slope from it (#651).
pub(crate) const MIN_RELIABILITY: f64 = 0.5;

/// Why no slope is read when the reliability is under [`MIN_RELIABILITY`],
/// in a watchmaker's words.
pub(crate) const TOO_NOISY: &str =
    "the amplitude readings' own noise is half or more of their variance";

/// A slope and its ± in s/d per degree to `digits` decimals, a slope
/// that rounds to zero given no sign: the one text every surface prints
/// for the slopes against the amplitude (#734).
pub fn slope_pm_text(slope: f64, se: f64, digits: usize) -> String {
    format!(
        "{} ± {se:.digits$} s/d per degree",
        signed_text(slope, digits)
    )
}

/// [`slope_pm_text`] of an [`EivSlope`], to two decimals.
pub(crate) fn slope_text(e: &EivSlope) -> String {
    slope_pm_text(e.slope, e.se, 2)
}

/// [`EivSlope`] of `(x, y, noise variance of x)`, one per reading in time
/// order, a missing reading's x or y NaN. The standard error is Fuller's
/// (1987, §1.2) for a slope with the noise variance σuu known (here the
/// mean of the readings'), from the residuals of the corrected slope and
/// the amplitude's own variance m̂XX = mxx − σuu. Its equation-error term
/// σee/m̂XX is widened by Bartlett's factor (1 + ρX·ρe)/(1 − ρX·ρe) for
/// the readings' lag-one correlation, ρX = ρx/λ of the amplitude and
/// ρe = ρv·σvv/σee of the equation error, each ρ read by
/// [`confidence::long_run_rho`] (its line out, only across neighbouring
/// readings, bias-corrected) and each held at 0.99 at most; the noise in
/// x is taken as independent from one reading to the next. `None` with
/// under 30 pairs, no noise known, or no spread in x.
pub(crate) fn eiv_slope(p: &[(f64, f64, f64)]) -> Option<EivSlope> {
    let present = |q: &&(f64, f64, f64)| q.0.is_finite() && q.1.is_finite();
    let kept: Vec<(f64, f64, f64)> = p.iter().filter(present).copied().collect();
    let noise: Vec<f64> = kept.iter().map(|q| q.2).filter(|v| v.is_finite()).collect();
    if kept.len() < 30 || noise.len() < kept.len() / 2 {
        return None;
    }
    let n = kept.len() as f64;
    let mx = kept.iter().map(|q| q.0).sum::<f64>() / n;
    let my = kept.iter().map(|q| q.1).sum::<f64>() / n;
    let sxx: f64 = kept.iter().map(|q| (q.0 - mx).powi(2)).sum();
    let sxy: f64 = kept.iter().map(|q| (q.0 - mx) * (q.1 - my)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let b = sxy / sxx;
    let suu = noise.iter().sum::<f64>() / noise.len() as f64;
    let mxx = sxx / (n - 1.0);
    let reliability = 1.0 - suu / mxx;
    let lam = reliability.max(f64::MIN_POSITIVE);
    let slope = b / lam;
    let mxx_true = (mxx - suu).max(f64::MIN_POSITIVE);
    // x and the residuals of the corrected slope in place, NaN where a
    // reading is missing, so ρ never pairs readings a gap separates.
    let (xs, v): (Vec<f64>, Vec<f64>) = p
        .iter()
        .map(|q| {
            if present(&q) {
                (q.0, q.1 - my - slope * (q.0 - mx))
            } else {
                (f64::NAN, f64::NAN)
            }
        })
        .unzip();
    let svv = v
        .iter()
        .filter(|e| e.is_finite())
        .map(|e| e * e)
        .sum::<f64>()
        / (n - 2.0);
    // The residual is the equation error less slope × the noise in x.
    let see = (svv - slope * slope * suu).max(0.0);
    let rho_x = (confidence::long_run_rho(&xs, Slow::Line) / lam).min(0.99);
    let rho_e = if see > 0.0 {
        (confidence::long_run_rho(&v, Slow::Line) * svv / see).min(0.99)
    } else {
        0.0
    };
    let r = rho_x * rho_e;
    let bartlett = (1.0 + r) / (1.0 - r);
    let var = (see * bartlett / mxx_true
        + slope * slope * suu / mxx_true
        + (suu * svv + (slope * suu).powi(2)) / mxx_true.powi(2))
        / (n - 1.0);
    Some(EivSlope {
        slope,
        se: var.sqrt(),
        reliability,
    })
}

/// How the rate moves with the amplitude, reading by reading.
#[derive(Debug, Clone, Serialize)]
pub struct CrossCorrelation {
    /// Length of each reading, seconds, and the pairs of readings used.
    pub step_s: f64,
    pub readings: usize,
    /// Correlation of the two, reading against the same reading.
    pub r: f64,
    /// Readings that are worth as much as independent ones, allowing for
    /// each series' memory.
    pub effective_readings: f64,
    /// Chance of an r this large from unrelated series.
    pub p_value: f64,
    /// Rate change per degree of amplitude within half an hour, s/d,
    /// corrected for the noise in the amplitude readings, and its standard
    /// error; `None` when [`Self::reliability`] is under 0.5.
    pub s_per_day_per_deg: Option<f64>,
    pub s_per_day_per_deg_se: Option<f64>,
    /// The share of the amplitude readings' spread that is the amplitude
    /// and not their noise (from each reading's own 2 s windows).
    pub reliability: Option<f64>,
    /// The plain correlation and slope over the whole take at no lag,
    /// slow change included (a shared drift counts here).
    pub take_r: f64,
    pub take_s_per_day_per_deg: f64,
    pub headline: String,
}

/// Correlate the rate with the amplitude, each reading against the same
/// reading (no lag: the rate answers the amplitude at once, and a lag
/// search on a shared cycle finds the half-turn where the two
/// anticorrelate). Both grids share `t0` and step.
/// The test is on the changes within half an hour (both series with
/// their slow change taken out): two series that both drift correlate
/// whether or not one drives the other, and their drift leaves too few
/// independent readings to tell. The whole-take slope is given beside it.
pub(crate) fn cross_correlation(
    rate: &Grid,
    amp: &Grid,
    noise: &[f64],
    cfg: &Config,
) -> Option<CrossCorrelation> {
    let (mut x, mut y) = (amp.clone(), rate.clone());
    drop_outliers(&mut x, cfg.outlier_sigma);
    drop_outliers(&mut y, cfg.outlier_sigma);
    let (xd, yd) = (
        longterm::detrend(&x, longterm::DETREND_S),
        longterm::detrend(&y, longterm::DETREND_S),
    );
    let pairs = |x: &Grid, y: &Grid| -> Vec<(f64, f64)> {
        (0..y.y.len().min(x.y.len()))
            .filter_map(|i| {
                let (a, b) = (x.y[i], y.y[i]);
                (a.is_finite() && b.is_finite()).then_some((a, b))
            })
            .collect()
    };
    let stats = |p: &[(f64, f64)]| {
        let n = p.len() as f64;
        let (ma, mb) = (
            p.iter().map(|q| q.0).sum::<f64>() / n,
            p.iter().map(|q| q.1).sum::<f64>() / n,
        );
        let sab: f64 = p.iter().map(|q| (q.0 - ma) * (q.1 - mb)).sum();
        let saa: f64 = p.iter().map(|q| (q.0 - ma).powi(2)).sum();
        let sbb: f64 = p.iter().map(|q| (q.1 - mb).powi(2)).sum();
        (sab / (saa * sbb).sqrt(), sab / saa)
    };
    let p = pairs(&xd, &yd);
    if p.len() < 30 {
        return None;
    }
    let (r, _) = stats(&p);
    if !r.is_finite() {
        return None;
    }
    // The slope read off the amplitude's noise, as the isochronism's is.
    let eiv = eiv_slope(
        &(0..yd.y.len().min(xd.y.len()))
            .map(|i| (xd.y[i], yd.y[i], noise.get(i).copied().unwrap_or(f64::NAN)))
            .collect::<Vec<_>>(),
    );
    let shown = eiv.filter(|e| e.reliability >= MIN_RELIABILITY);
    let (take_r, take_slope) = stats(&pairs(&x, &y));
    let n = p.len() as f64;
    // Bartlett's factor for a correlation, each ρ as the slope's is read
    // (#651): held at 0 or above, which drops the rare case of both
    // negative.
    let rho = |g: &Grid| confidence::long_run_rho(&g.y, Slow::Line);
    let n_eff = confidence::effective_readings(n, rho(&xd) * rho(&yd)).max(4.0);
    let z = r.clamp(-0.999_999, 0.999_999).atanh() * (n_eff - 3.0).sqrt();
    let p_value = chi2_tail(z * z, 1);
    let step = rate.step;
    let slope_text = match (shown, eiv) {
        (Some(e), _) => slope_text(&e),
        (None, Some(_)) => format!("no slope ({TOO_NOISY})"),
        (None, None) => "no slope (the amplitude's noise is not known)".into(),
    };
    let headline = format!(
        "Rate against amplitude from one {:.0} s reading to the next: {}, r {}, p {} over {} pairs (worth about {:.0} independent ones); over the whole take, {} s/d per degree (r {}).",
        step,
        slope_text,
        signed_text(r, 2),
        p_text(p_value),
        p.len(),
        n_eff,
        signed_text(take_slope, 2),
        signed_text(take_r, 2)
    );
    Some(CrossCorrelation {
        step_s: step,
        readings: p.len(),
        r,
        effective_readings: n_eff,
        p_value,
        s_per_day_per_deg: shown.map(|e| e.slope),
        s_per_day_per_deg_se: shown.map(|e| e.se),
        reliability: eiv.map(|e| e.reliability),
        take_r: finite_or_nan(take_r),
        take_s_per_day_per_deg: finite_or_nan(take_slope),
        headline,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::Rng;

    /// The rate's least-squares line weights five 2 s windows of a 10 s
    /// reading by 6u(1 − u) over each (#951): 0.104, 0.248, 0.296, 0.248
    /// and 0.104, summing to 1, with Σw²·5 about 1.16.
    #[test]
    fn the_rate_fit_weights_five_windows_as_its_kernel() {
        let w: Vec<f64> = (0..5)
            .map(|k| rate_fit_weight(k as f64 / 5.0, (k + 1) as f64 / 5.0))
            .collect();
        for (got, want) in w.iter().zip([0.104, 0.248, 0.296, 0.248, 0.104]) {
            assert!((got - want).abs() < 1e-9, "{w:?}");
        }
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        let factor = 5.0 * w.iter().map(|x| x * x).sum::<f64>();
        assert!((factor - 1.16).abs() < 0.005, "{factor}");
        // Clamped to the span.
        assert_eq!(rate_fit_weight(-0.5, 0.0), 0.0);
        assert!((rate_fit_weight(-0.5, 1.5) - 1.0).abs() < 1e-12);
    }

    /// A drop that fills the first four windows of every 18th reading
    /// moves the paired amplitude 0.896 of the way, as it moves the rate,
    /// and its edges inside the readings leave the noise read off the
    /// windows' scatter (#951).
    #[test]
    fn the_paired_amplitude_moves_with_the_rate_over_a_short_drop() {
        let mut z = Rng::new(951);
        let n = 720;
        let fine = Grid {
            t0: 0.0,
            step: 2.0,
            y: (0..5 * n)
                .map(|i| {
                    let t = 2.0 * i as f64 + 1.0;
                    let drop = if t % 180.0 < 8.0 { -15.0 } else { 0.0 };
                    270.0 + drop + z.normal()
                })
                .collect(),
        };
        let medians = Grid {
            t0: 0.0,
            step: 10.0,
            y: vec![270.0; n],
        };
        let (amp, noise) = pairing_readings(&fine, &medians);
        let mut moved = Vec::new();
        for b in (0..n).step_by(18) {
            moved.push(270.0 - amp.y[b]);
        }
        let mean = moved.iter().sum::<f64>() / moved.len() as f64;
        assert!((mean - 0.896 * 15.0).abs() < 0.2, "{mean}");
        // Unit window noise, five windows: about 0.232, not pulled up by
        // the 15° edges in one reading in 18.
        let m = noise.iter().sum::<f64>() / n as f64;
        assert!((m - 0.232).abs() < 0.03, "{m}");
        // A reading the medians leave out is left out here too.
        let mut gap = medians.clone();
        gap.y[3] = f64::NAN;
        let (amp, noise) = pairing_readings(&fine, &gap);
        assert!(amp.y[3].is_nan() && noise[3].is_nan() && amp.y[4].is_finite());
    }

    /// On the rub scenario, the slope of the rate against the paired
    /// amplitude reads the true coupling of 2.0 s/d per degree (#951);
    /// against the median it read about 1.9.
    #[test]
    fn a_rubs_slope_reads_its_true_coupling() {
        let log = crate::scenarios::beat_log(&crate::scenarios::find("rub").unwrap(), 1);
        let [rate, medians, _] = crate::steadiness::reading_grids(&log, None, &[], 10.0);
        let [fine, _] = crate::steadiness::window_grids(&log, None, &[]);
        let (amp, noise) = pairing_readings(&fine, &medians);
        let x = cross_correlation(&rate, &amp, &noise, &Config::default()).unwrap();
        let (s, se) = (
            x.s_per_day_per_deg.unwrap(),
            x.s_per_day_per_deg_se.unwrap(),
        );
        assert!((s - 2.0).abs() < 0.05 && se < 0.03, "{s} ± {se}");
    }

    /// Of `runs` seeded takes of `n` readings, amplitude (spread 5°) and
    /// the rate's residual each AR(1) at `rho`, the rate −0.5 s/d per
    /// degree, each amplitude reading off by independent noise of SD
    /// `noise`, how many the slope's ±1.96 SE holds the true slope in.
    /// `drop(i, z)` says whether reading i is missing.
    fn covered(
        rho: f64,
        noise: f64,
        n: usize,
        runs: usize,
        drop: impl Fn(usize, &mut Rng) -> bool,
    ) -> usize {
        let mut z = Rng::new(651);
        let mut hits = 0;
        for _ in 0..runs {
            let (mut a, mut e) = (z.normal(), z.normal());
            let k = (1.0 - rho * rho).sqrt();
            let p: Vec<(f64, f64, f64)> = (0..n)
                .map(|i| {
                    a = rho * a + k * z.normal();
                    e = rho * e + k * z.normal();
                    let x = 270.0 + 5.0 * a;
                    let seen = x + noise * z.normal();
                    if drop(i, &mut z) {
                        (f64::NAN, f64::NAN, f64::NAN)
                    } else {
                        (seen, -0.5 * (x - 270.0) + e, (noise * noise).max(1e-9))
                    }
                })
                .collect();
            let s = eiv_slope(&p).unwrap();
            hits += usize::from((s.slope + 0.5).abs() <= 1.96 * s.se);
        }
        hits
    }

    /// The ± holds the true slope about 95 times in 100 with real noise
    /// in the amplitude and memory in both series, gaps or none (#651,
    /// #728): over ρ 0, 0.5 and 0.9, amplitude noise of SD 0 to 3.5° on a
    /// 5° spread (λ down to about 0.67), and no gaps, blocks or half
    /// missing, 929 to 991 of 1000 (half missing at ρ 0.9 leaves fewer
    /// neighbours to carry the correlation, so it errs wide). With the
    /// least-squares SE over λ and Bartlett's factor on all of it, noise of
    /// 3.5° at ρ 0 held 869, and ρ 0.9 with no gaps 816 to 844.
    #[test]
    fn the_slopes_pm_holds_the_true_slope_in_noisy_correlated_readings_with_gaps() {
        let n = 720;
        let blocks = move |i: usize, z: &mut Rng| {
            (n / 6..n / 3).contains(&i) || (n / 2..n / 2 + n / 9).contains(&i) || z.next_f64() < 0.1
        };
        for rho in [0.0, 0.5, 0.9] {
            for noise in [0.0, 1.5, 2.5, 3.5] {
                for (gaps, hits) in [
                    ("no", covered(rho, noise, n, 1000, |_, _| false)),
                    ("block", covered(rho, noise, n, 1000, blocks)),
                    (
                        "half",
                        covered(rho, noise, n, 1000, |_, z| z.next_f64() < 0.5),
                    ),
                ] {
                    assert!(
                        (925..=995).contains(&hits),
                        "{hits} of 1000 at ρ {rho}, noise {noise}° and {gaps} gaps"
                    );
                }
            }
        }
    }

    /// A slope that rounds to zero takes no sign (#734).
    #[test]
    fn a_slope_of_about_zero_reads_without_a_sign() {
        let e = |slope| EivSlope {
            slope,
            se: 0.01,
            reliability: 0.97,
        };
        assert_eq!(slope_text(&e(-0.004)), "0.00 ± 0.01 s/d per degree");
        assert_eq!(slope_text(&e(-0.02)), "-0.02 ± 0.01 s/d per degree");
        assert_eq!(slope_text(&e(0.5)), "+0.50 ± 0.01 s/d per degree");
    }

    /// A figure in the rate-against-amplitude headline that rounds to zero
    /// reads without a sign (#734), the whole take's slope here.
    #[test]
    fn the_cross_correlations_figures_at_zero_read_without_a_sign() {
        let mut z = Rng::new(734);
        let n = 720;
        let grid = |y: Vec<f64>| Grid {
            t0: 0.0,
            step: 10.0,
            y,
        };
        // A rate that barely moves: both slopes round to 0.
        let amp: Vec<f64> = (0..n).map(|_| 270.0 + 5.0 * z.normal()).collect();
        let rate: Vec<f64> = (0..n).map(|_| 1e-4 * z.normal()).collect();
        let x =
            cross_correlation(&grid(rate), &grid(amp), &vec![0.01; n], &Config::default()).unwrap();
        assert!(!x.headline.contains("-0.00"), "{}", x.headline);
        assert!(
            x.headline
                .contains("over the whole take, 0.00 s/d per degree"),
            "{}",
            x.headline
        );
    }
}
