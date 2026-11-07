//! The white null the Ljung–Box test is held to (#1199): white readings,
//! detrended exactly as the take is. Their mean correlation at each lag is
//! what the detrend (and the mean) take out of independent readings, and
//! their Q's mean, variance and skew give the shifted, scaled χ² the
//! take's Q is read against. On 4,000 white takes of 30 min in 10 s
//! readings the plain χ² named 73 at p 0.01 (40 expected): a running
//! median's lag bias, about −0.7 over its window, and Q's own excess
//! spread on short takes.
//!
//! The nulls are drawn once, offline, into `data/white-null.json`
//! ([`write_white_null_table`](super::tests) writes it), on gap-free white
//! readings: a take with gaps is read against the null of its present
//! readings, which held on fresh seeds for scattered, single and block
//! gaps. The table is read at the detrend the take's own grid got and its
//! present readings, interpolated in log n (and in their share of the
//! window); outside it, or for another number of Ljung–Box lags, the
//! plain χ².

#[cfg(test)]
use super::acf_pairs;
use super::{gamma_q, MIN_PAIRS};
use crate::longterm::{self, Grid};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The Ljung–Box lags the table's Q is read over.
#[cfg(test)]
pub(super) const TABLE_LB_LAGS: usize = 10;

/// The lags each point's bias is kept to: past it a lag is read as it is.
#[cfg(test)]
pub(super) const TABLE_LAGS: usize = 128;

/// Past this span a take's null is the one of its first readings: the
/// running median's window is fixed from 2 h on, and past 720 readings
/// the plain χ² holds.
pub(super) const DRAW_SPAN_S: f64 = 4.0 * 3600.0;

/// What white readings read once detrended: the mean correlation at each
/// lag (0 at lag 0), and the mean, variance and skew of their Ljung–Box Q.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct WhiteNull {
    pub bias: Vec<f64>,
    pub q_mean: f64,
    pub q_var: f64,
    pub q_skew: f64,
}

impl WhiteNull {
    /// P of a Q this large, from a shifted, scaled χ² matched to the draws'
    /// mean, variance and skew (Pearson's three moments): ν = 8/skew²
    /// degrees of freedom, Q = a + b·χ²_ν. Two moments alone left the tail
    /// at p 0.004 twice too heavy on 20 min takes. Without a positive skew,
    /// two moments (Satterthwaite).
    pub fn p(&self, q: f64) -> f64 {
        if self.q_mean <= 0.0 || self.q_var <= 0.0 {
            return 1.0;
        }
        let (nu, b, a) = if self.q_skew > 0.0 {
            let nu = 8.0 / (self.q_skew * self.q_skew);
            let b = (self.q_var / (2.0 * nu)).sqrt();
            (nu, b, self.q_mean - b * nu)
        } else {
            let nu = 2.0 * self.q_mean * self.q_mean / self.q_var;
            (nu, self.q_var / (2.0 * self.q_mean), 0.0)
        };
        if q <= a {
            return 1.0;
        }
        gamma_q(nu / 2.0, (q - a) / b / 2.0)
    }

    /// `self` and `other` blended, `t` of the way to `other`; the bias to
    /// the lags both hold.
    fn blend(&self, other: &WhiteNull, t: f64) -> WhiteNull {
        let mix = |a: f64, b: f64| a + (b - a) * t;
        WhiteNull {
            bias: self
                .bias
                .iter()
                .zip(&other.bias)
                .map(|(&a, &b)| mix(a, b))
                .collect(),
            q_mean: mix(self.q_mean, other.q_mean),
            q_var: mix(self.q_var, other.q_var),
            q_skew: mix(self.q_skew, other.q_skew),
        }
    }
}

/// One drawn null: `n` gap-free readings detrended with a quadratic
/// (`window` 0) or a running median of `window` readings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Point {
    pub n: usize,
    pub window: usize,
    pub draws: usize,
    pub null: WhiteNull,
}

/// `data/white-null.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Table {
    pub note: String,
    pub lb_lags: usize,
    /// By n.
    pub quadratic: Vec<Point>,
    /// By window, then n.
    pub median: Vec<Point>,
}

const DATA: &str = include_str!("../data/white-null.json");

pub(super) fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(DATA).expect("white-null.json"))
}

/// The white null for a take of `n` present readings on a grid of `len`
/// readings `step` apart, with Q over `lb_lags`: the table's,
/// interpolated; None outside it. The detrend (quadratic, or the running
/// median's window) is the one the take's own grid got, gaps included;
/// the point along it is the present readings' (#1199).
pub(super) fn white_null(n: usize, len: usize, step: f64, lb_lags: usize) -> Option<WhiteNull> {
    lookup(table(), n, len, step, lb_lags)
}

/// [`white_null`] from `t`.
pub(super) fn lookup(
    t: &Table,
    n: usize,
    len: usize,
    step: f64,
    lb_lags: usize,
) -> Option<WhiteNull> {
    if lb_lags != t.lb_lags || n < 2 {
        return None;
    }
    let cap = ((DRAW_SPAN_S / step).round() as usize).max(1);
    let n = n.min(len).min(cap);
    let g = Grid {
        t0: 0.0,
        step,
        y: vec![0.0; len.min(cap)],
    };
    match longterm::detrend_window(&g, longterm::DETREND_S) {
        None => along(&t.quadratic, (n as f64).ln(), false, |p| (p.n as f64).ln()),
        Some(w) => {
            // Each window's row by the present readings' share of the
            // window (4 to 8 without gaps: a quarter of the take up to 2 h,
            // then 30 min of up to 4 h; a gapped take's share, under 4, reads
            // the row's end), then across the windows either side in log
            // window.
            let ratio = n as f64 / w as f64;
            let rows: Vec<(f64, WhiteNull)> = windows(&t.median)
                .into_iter()
                .filter_map(|wr| {
                    let row: Vec<Point> = t
                        .median
                        .iter()
                        .filter(|p| p.window == wr)
                        .cloned()
                        .collect();
                    along(&row, ratio, true, |p| p.n as f64 / p.window as f64)
                        .map(|v| ((wr as f64).ln(), v))
                })
                .collect();
            let lw = (w as f64).ln();
            let i = rows.iter().position(|r| r.0 >= lw - 1e-12)?;
            if (rows[i].0 - lw).abs() < 1e-12 {
                return Some(rows[i].1.clone());
            }
            let (a, b) = (rows.get(i.checked_sub(1)?)?, &rows[i]);
            Some(a.1.blend(&b.1, (lw - a.0) / (b.0 - a.0)))
        }
    }
}

/// The distinct windows of `points`, in order.
fn windows(points: &[Point]) -> Vec<usize> {
    let mut w: Vec<usize> = points.iter().map(|p| p.window).collect();
    w.sort_unstable();
    w.dedup();
    w
}

/// The null at `x` along `points` (in order of `key`), blended between
/// the two either side; outside them the end's when `clamp`, else None.
fn along(points: &[Point], x: f64, clamp: bool, key: impl Fn(&Point) -> f64) -> Option<WhiteNull> {
    let (first, last) = (points.first()?, points.last()?);
    if x <= key(first) || x >= key(last) {
        let end = if x <= key(first) { first } else { last };
        return ((key(end) - x).abs() < 1e-12 || clamp).then(|| end.null.clone());
    }
    let i = points.iter().position(|p| key(p) >= x)?;
    let (a, b) = (&points[i - 1], &points[i]);
    Some(a.null.blend(&b.null, (x - key(a)) / (key(b) - key(a))))
}

/// The Ljung–Box Q over `lags` of correlations `r` read from `pairs`
/// pairs among `n` readings: each lag weighed by its pairs (#828).
pub(super) fn ljung_q(r: &[f64], pairs: &[usize], n: usize, lags: &[usize]) -> f64 {
    let np = n as f64;
    lags.iter()
        .map(|&k| r[k] * r[k] * pairs[k] as f64 * (np + 2.0) / np)
        .sum()
}

/// The lags Q is read over: 1 to `most`, each finite and read from at
/// least [`MIN_PAIRS`] pairs.
pub(super) fn q_lags(r: &[f64], pairs: &[usize], most: usize) -> Vec<usize> {
    (1..=most)
        .filter(|&k| {
            r.get(k).is_some_and(|v| v.is_finite()) && pairs.get(k).is_some_and(|&m| m >= MIN_PAIRS)
        })
        .collect()
}

/// Readings drawn for each point of the table: the series number is this
/// over the readings each carries, from [`MIN_DRAWS`] to [`MAX_DRAWS`].
/// 200 series (about 12% in Q's variance) read 71 of 1,000 white takes of
/// 1 h at p 0.05, as every take of one shape shares its null; 15,873 (at
/// 8 million readings) left the point at 84 min 4% low in variance and
/// 0.1 in skew, and takes of 83 min read 1,106 of 20,000 at p 0.05.
#[cfg(test)]
pub(super) const DRAW_BUDGET: usize = 40_000_000;
#[cfg(test)]
pub(super) const MIN_DRAWS: usize = 1_000;
#[cfg(test)]
pub(super) const MAX_DRAWS: usize = 40_000;

/// One point of the table, drawn: white gap-free readings, `n` of them
/// `step` apart, detrended as a take on that grid is, from `seed`.
#[cfg(test)]
pub(super) fn draw_point(n: usize, step: f64, seed: u64) -> Point {
    let count = (DRAW_BUDGET / n.max(1)).clamp(MIN_DRAWS, MAX_DRAWS);
    let max_lag = TABLE_LAGS.min(n / 3);
    let draws: Vec<(Vec<f64>, Vec<usize>)> = (0..count as u64)
        .map(|s| {
            let mut z = crate::synth::Rng::new(seed.wrapping_add(s));
            let y = (0..n).map(|_| z.normal()).collect();
            let d = longterm::detrend(&Grid { t0: 0.0, step, y }, longterm::DETREND_S);
            acf_pairs(&d.y, max_lag)
        })
        .collect();
    let mut bias = vec![0.0; max_lag + 1];
    for (r, _) in &draws {
        for (b, v) in bias.iter_mut().zip(r).skip(1) {
            *b += v / count as f64;
        }
    }
    let qs: Vec<f64> = draws
        .iter()
        .map(|(r, pairs)| {
            let r: Vec<f64> = r.iter().zip(&bias).map(|(v, b)| v - b).collect();
            ljung_q(&r, pairs, n, &q_lags(&r, pairs, TABLE_LB_LAGS))
        })
        .collect();
    let q_mean = qs.iter().sum::<f64>() / qs.len() as f64;
    let q_var = qs.iter().map(|q| (q - q_mean).powi(2)).sum::<f64>() / (qs.len() - 1) as f64;
    // Under 31 readings no lag has the pairs Q needs, and Q is 0.
    let q_skew = if q_var > 0.0 {
        qs.iter().map(|q| (q - q_mean).powi(3)).sum::<f64>() / qs.len() as f64 / q_var.powf(1.5)
    } else {
        0.0
    };
    let g = Grid {
        t0: 0.0,
        step,
        y: vec![0.0; n],
    };
    Point {
        n,
        window: longterm::detrend_window(&g, longterm::DETREND_S).unwrap_or(0),
        draws: count,
        null: WhiteNull {
            bias,
            q_mean,
            q_var,
            q_skew,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table the app ships is what this code draws: a few of its
    /// cheapest points, drawn again from their seeds as
    /// `write_white_null_table` drew them, match it. A change to the
    /// detrend, the correlations or Q that would move the null shows here,
    /// not only when the table is next written.
    #[test]
    fn the_shipped_white_null_is_what_its_points_draw() {
        let seed = |n: usize, step: f64| (n as u64) << 20 ^ step.to_bits().rotate_left(7) ^ 0x1199;
        let t = table();
        let shipped = |window: usize, n: usize| {
            let rows = if window == 0 { &t.quadratic } else { &t.median };
            rows.iter()
                .find(|p| p.window == window && p.n == n)
                .unwrap()
        };
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * a.abs().max(1.0);
        for (n, step, window) in [(33, 1.0, 0), (40, 1.0, 0), (20, 360.0, 5)] {
            let p = draw_point(n, step, seed(n, step));
            let s = shipped(window, n);
            assert_eq!((p.window, p.draws), (s.window, s.draws), "{n}");
            let (a, b) = (&p.null, &s.null);
            assert!(close(a.q_mean, b.q_mean), "{n}: {} {}", a.q_mean, b.q_mean);
            assert!(close(a.q_var, b.q_var), "{n}: {} {}", a.q_var, b.q_var);
            assert!(close(a.q_skew, b.q_skew), "{n}: {} {}", a.q_skew, b.q_skew);
            assert_eq!(a.bias.len(), b.bias.len());
            assert!(
                a.bias.iter().zip(&b.bias).all(|(&x, &y)| close(x, y)),
                "{n}"
            );
        }
        // Under 31 readings no lag has the pairs Q needs: Q is 0, with
        // no spread, and any Q is read as likely as can be.
        let p = draw_point(30, 1.0, seed(30, 1.0));
        assert_eq!(
            (p.null.q_mean, p.null.q_var, p.null.q_skew),
            (0.0, 0.0, 0.0)
        );
        assert_eq!(p.null.p(50.0), 1.0);
    }

    /// A take whose grid gets a window the table holds reads that window's
    /// row as it is, not a blend with its neighbours.
    #[test]
    fn a_window_the_table_holds_is_read_as_it_is() {
        let t = table();
        let row = t
            .median
            .iter()
            .find(|p| p.window == 5 && p.n == 20)
            .unwrap();
        let got = lookup(t, 20, 20, 360.0, t.lb_lags).unwrap();
        assert_eq!(got.q_mean.to_bits(), row.null.q_mean.to_bits());
        assert_eq!(got.q_var.to_bits(), row.null.q_var.to_bits());
        assert_eq!(got.bias, row.null.bias);
    }
}
