//! The period search's null (#1195): what the search reads from noise
//! alone, simulated through the same steps (detrend, spectrum, whitening,
//! oversampling, harmonic sums) and held in `data/period-null.json`, so
//! that its stated false-alarm chance holds.
//!
//! Two noise models, one for each kind of series the search reads:
//!
//! - **Readings** (amplitude, beat error): AR(1), indexed by the lag-one ρ
//!   the spectrum is whitened with.
//! - **Timing** (the rate's search runs on the beats' timing): white rate
//!   noise integrates to a random walk, and the beats' own jitter adds
//!   white noise, so random walk plus white. The AR(1) whitening doesn't fit
//!   that shape, and below the running median's corner the walk leaves a
//!   hump that read false cycles at 2 to 10 times the stated rate. The mix
//!   is indexed by the crossover frequency (where the walk's power meets
//!   the white floor) times the detrend's window: [`timing_mix`].
//!
//! For each take length, grid step and mix, the table holds
//! - the whitened null spectrum's mean against f·w (f·span under the
//!   quadratic detrend), which the search divides out, and
//! - the distribution of the maximum score over the searched band, as a
//!   generalised extreme value ([`Tail`]), so a maximum `s` from noise has
//!   a known chance. One effective number of trials did not hold: the
//!   score's tail falls about 0.75 to 0.95 decades for each, so a count
//!   right at p 0.5 was 3 to 6 times short at p 0.01; and on takes under
//!   an hour the tail is heavier still than a Gumbel's, so the shape ξ is
//!   fitted for each take length.
//!
//! The table comes from fixed seeds (`write_period_null_table`, an
//! ignored test). A take outside its range of length, step or mix is
//! searched as before, by the trials count alone, and the search says it
//! is uncalibrated ([`Search::calibrated`](super::Search)).

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Which simulated noise a series is searched against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NullModel {
    /// AR(1) readings: amplitude, beat error.
    Readings,
    /// Random walk plus white: the beats' timing.
    Timing,
}

/// Where a search's null came from: the model, the take's mix, and
/// whether the table covered it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct NullFit {
    pub model: NullModel,
    /// ρ for readings; the crossover times the detrend's window (the
    /// take's span under the quadratic) for timing.
    pub mix: f64,
    /// Whether the table covered this take's length, step and mix; when
    /// false the search used the plain trials count and its false-alarm
    /// chance is not calibrated.
    pub calibrated: bool,
}

/// The table's f·w axis: 0.01 times 2^(k/4), k from 0 to 68.
pub const X_POINTS: usize = 69;

pub fn x_at(k: usize) -> f64 {
    0.01 * 2f64.powf(k as f64 / 4.0)
}

/// One simulated cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub model: NullModel,
    pub span_s: f64,
    pub step_s: f64,
    pub mix: f64,
    /// The maximum score's extreme-value fit: location, scale, shape, and
    /// the log10 of the trials count the search made for it.
    pub mu: f64,
    pub sigma: f64,
    pub xi: f64,
    pub log_trials: f64,
    /// The whitened null's mean at each `x_at(k)`; null where the cell
    /// has no searched bins.
    pub mean: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    /// What the simulation ran, for the record.
    pub note: String,
    pub draws_mean: usize,
    pub draws_max: usize,
    pub cells: Vec<Cell>,
}

const DATA: &str = include_str!("../../data/period-null.json");

pub fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(DATA).expect("period-null.json"))
}

/// What a search divides its whitened spectrum by, and its maximum
/// score's tail.
#[derive(Debug, Clone)]
pub struct Levelling {
    pub mean: Vec<Option<f64>>,
    pub mu: f64,
    pub sigma: f64,
    pub xi: f64,
    pub log_trials: f64,
}

/// The distribution of a search's maximum score from noise alone: a
/// generalised extreme value, P(max ≤ s) = exp(−t^(−1/ξ)), t = 1 + ξ(s −
/// μ)/σ (a Gumbel at ξ = 0). Its tail, t^(−1/ξ), is the chance a score is
/// read as.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Tail {
    pub mu: f64,
    pub sigma: f64,
    pub xi: f64,
}

impl Tail {
    /// The plain trials count's tail, `trials` · 10^−s: the search's rate
    /// where the table doesn't reach.
    pub fn trials(trials: f64) -> Tail {
        Tail {
            mu: trials.log10(),
            sigma: 1.0 / std::f64::consts::LN_10,
            xi: 0.0,
        }
    }

    /// −log10 of the chance noise's maximum reaches `score`.
    pub fn significance(&self, score: f64) -> f64 {
        let z = (score - self.mu) / self.sigma;
        if self.xi.abs() < 1e-9 {
            return z / std::f64::consts::LN_10;
        }
        let t = 1.0 + self.xi * z;
        if t <= 0.0 {
            return 0.0;
        }
        t.log10() / self.xi
    }

    /// The score noise's maximum reaches with the chance `p`.
    pub fn score_at(&self, p: f64) -> f64 {
        if self.xi.abs() < 1e-9 {
            return self.mu - self.sigma * p.ln();
        }
        self.mu + self.sigma * (p.powf(-self.xi) - 1.0) / self.xi
    }
}

impl Levelling {
    /// The maximum's tail over `trials`: the fit's, moved from the trials
    /// count it was made with as the maximum of that many more draws.
    pub fn tail(&self, trials: f64) -> Tail {
        let k = trials / 10f64.powf(self.log_trials);
        if self.xi.abs() < 1e-9 {
            return Tail {
                mu: self.mu + self.sigma * k.ln(),
                sigma: self.sigma,
                xi: 0.0,
            };
        }
        let kx = k.powf(self.xi);
        Tail {
            mu: self.mu + self.sigma * (kx - 1.0) / self.xi,
            sigma: self.sigma * kx,
            xi: self.xi,
        }
    }

    /// The null's mean at `x`, from the two table points either side (on
    /// log x); None outside the points the cell measured.
    pub fn at(&self, x: f64) -> Option<f64> {
        let k = (x / 0.01).log2() * 4.0;
        // NaN (x not above 0) falls outside too.
        if !(0.0..=(X_POINTS - 1) as f64).contains(&k) {
            return None;
        }
        let k0 = k.floor() as usize;
        let k1 = (k0 + 1).min(X_POINTS - 1);
        let (a, b) = (self.mean[k0]?, self.mean[k1]?);
        Some(a + (b - a) * (k - k0 as f64))
    }
}

/// Timing's mix for a walk with no white floor in the searched band:
/// the table's pure-walk cells.
pub const WALK_MIX: f64 = 1e12;

/// The mix axis the table interpolates on: ρ as it is, timing's
/// crossover as f_c·w / (1 + f_c·w), so white noise (0) and a walk with no
/// floor (1) are both points on it.
fn axis(model: NullModel, mix: f64) -> f64 {
    match model {
        NullModel::Readings => mix,
        NullModel::Timing if mix >= WALK_MIX => 1.0,
        NullModel::Timing => mix / (1.0 + mix),
    }
}

fn blend(a: &Levelling, b: &Levelling, t: f64) -> Levelling {
    Levelling {
        mean: a
            .mean
            .iter()
            .zip(&b.mean)
            .map(|(&x, &y)| Some(x? + (y? - x?) * t))
            .collect(),
        mu: a.mu + (b.mu - a.mu) * t,
        sigma: a.sigma + (b.sigma - a.sigma) * t,
        xi: a.xi + (b.xi - a.xi) * t,
        log_trials: a.log_trials + (b.log_trials - a.log_trials) * t,
    }
}

/// The two entries of sorted `keys` either side of `v` and how far
/// between; None outside them.
fn bracket(keys: &[f64], v: f64) -> Option<(usize, usize, f64)> {
    let i = keys.iter().position(|&k| k >= v - 1e-9)?;
    if (keys[i] - v).abs() <= 1e-9 {
        return Some((i, i, 0.0));
    }
    let j = i.checked_sub(1)?;
    Some((j, i, (v - keys[j]) / (keys[i] - keys[j])))
}

fn at_span(cells: &[&Cell], model: NullModel, mix: f64) -> Option<Levelling> {
    let mut row: Vec<&&Cell> = cells.iter().collect();
    row.sort_by(|a, b| a.mix.total_cmp(&b.mix));
    let keys: Vec<f64> = row.iter().map(|c| axis(model, c.mix)).collect();
    let (i, j, t) = bracket(&keys, axis(model, mix))?;
    let l = |c: &Cell| Levelling {
        mean: c.mean.clone(),
        mu: c.mu,
        sigma: c.sigma,
        xi: c.xi,
        log_trials: c.log_trials,
    };
    Some(blend(&l(row[i]), &l(row[j]), t))
}

/// The table's levelling for a take of `span_s` on a `step_s` grid at
/// `mix`, interpolated between the cells around it (log span, mix axis);
/// None outside the table, or across a change of step or of detrend.
pub fn levelling(model: NullModel, span_s: f64, step_s: f64, mix: f64) -> Option<Levelling> {
    levelling_in(&table().cells, model, span_s, step_s, mix)
}

pub fn levelling_in(
    cells: &[Cell],
    model: NullModel,
    span_s: f64,
    step_s: f64,
    mix: f64,
) -> Option<Levelling> {
    if mix.is_nan() {
        return None;
    }
    let median = span_s >= super::MEDIAN_FROM_S;
    let ours: Vec<&Cell> = cells
        .iter()
        .filter(|c| {
            c.model == model
                // Readings come at the sound card's clock, a few ppm off
                // the nominal step.
                && (c.step_s - step_s).abs() < 1e-3 * c.step_s
                && (c.span_s >= super::MEDIAN_FROM_S) == median
        })
        .collect();
    let mut spans: Vec<f64> = ours.iter().map(|c| c.span_s).collect();
    spans.sort_by(f64::total_cmp);
    spans.dedup();
    let logs: Vec<f64> = spans.iter().map(|s| s.ln()).collect();
    let (i, j, t) = bracket(&logs, span_s.ln())?;
    let row = |s: f64| -> Vec<&Cell> {
        ours.iter()
            .copied()
            .filter(|c| (c.span_s - s).abs() < 1e-9)
            .collect()
    };
    let a = at_span(&row(spans[i]), model, mix)?;
    let b = at_span(&row(spans[j]), model, mix)?;
    Some(blend(&a, &b, t))
}

/// Timing's mix from its power spectrum `power` (bin j at j·df, on a
/// `step_s` grid): the crossover f_c where a random walk's power a/(4
/// sin²(πfΔ)) meets the white floor b, times `scale` (the detrend's
/// window, or the span under the quadratic). Fitted to the median power
/// in quarter-octave bands from `from` up, so that a real line, which
/// lifts a few bins, does not move it; least squares relative to the
/// model, twice. 0 for white noise; [`WALK_MIX`] when no floor shows
/// (the walk reaches past the Nyquist frequency, or the floor fits at or
/// under nothing).
pub fn timing_mix(power: &[f64], df: f64, step_s: f64, from: usize, scale: f64) -> f64 {
    let u = |j: usize| {
        let s = (std::f64::consts::PI * j as f64 * df * step_s).sin();
        1.0 / (4.0 * s * s)
    };
    let top = power.len().saturating_sub(1);
    let mut bands: Vec<(f64, f64)> = Vec::new();
    let mut a = from.max(1);
    while a < top {
        let b = ((a as f64 * 2f64.powf(0.25)).ceil() as usize)
            .max(a + 4)
            .min(top);
        let mut v: Vec<f64> = power[a..b].to_vec();
        let mid = (a + b) / 2;
        bands.push((u(mid), crate::dsp::median(&mut v) / std::f64::consts::LN_2));
        a = b;
    }
    if bands.len() < 3 {
        return f64::NAN;
    }
    let mut w: Vec<f64> = bands.iter().map(|b| 1.0 / b.1.max(1e-300)).collect();
    let (mut ca, mut cb) = (0.0, 0.0);
    for _ in 0..3 {
        // Weighted least squares of p = a·u + b.
        let (mut s_uu, mut s_u, mut s_1, mut s_up, mut s_p) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (&(uu, p), &wt) in bands.iter().zip(&w) {
            let w2 = wt * wt;
            s_uu += w2 * uu * uu;
            s_u += w2 * uu;
            s_1 += w2;
            s_up += w2 * uu * p;
            s_p += w2 * p;
        }
        let det = s_uu * s_1 - s_u * s_u;
        if det.abs() < 1e-300 {
            return f64::NAN;
        }
        ca = (s_up * s_1 - s_u * s_p) / det;
        cb = (s_uu * s_p - s_u * s_up) / det;
        if ca <= 0.0 {
            // No walk: a white floor alone.
            return 0.0;
        }
        if cb <= 0.0 {
            return WALK_MIX;
        }
        w = bands.iter().map(|&(uu, _)| 1.0 / (ca * uu + cb)).collect();
    }
    let s2 = ca / (4.0 * cb);
    if s2 >= 1.0 {
        return WALK_MIX;
    }
    s2.sqrt().asin() / (std::f64::consts::PI * step_s) * scale
}

#[cfg(test)]
#[path = "null_tests.rs"]
mod tests;
