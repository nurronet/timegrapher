//! The white null the Ljung–Box test is held to (#1199): white readings
//! with the take's own gaps, detrended exactly as the take is. Their mean
//! correlation at each lag is what the detrend (and the mean) take out of
//! independent readings, and their Q's mean and variance give the scaled
//! χ² the take's Q is read against. On 4,000 white takes of 30 min in 10 s
//! readings the plain χ² named 73 at p 0.01 (40 expected): a running
//! median's lag bias, about −0.7 over its window, and Q's own excess
//! spread on short takes.

use super::{acf_pairs, gamma_q, MIN_PAIRS};
use crate::longterm::{self, Grid};

/// Readings drawn for each null, shared among its white series: the
/// series number is this over the readings each carries, from
/// [`MIN_DRAWS`] to [`MAX_DRAWS`]. Every take of one length and gap
/// pattern shares one null, so the error in its Q's variance is the
/// same for all of them: 200 series (about 12% in the variance) read 71 of
/// 1,000 white takes of 1 h at p 0.05; 1,000 or more hold it to about
/// 4%. p is read from the scaled χ² they fit, not counted from them.
const DRAW_BUDGET: usize = 1_500_000;
const MIN_DRAWS: usize = 200;
const MAX_DRAWS: usize = 4000;

/// Past this span the draws carry only the first readings (and their
/// gaps): the running median's window is fixed from 2 h on, and past
/// 720 readings the plain χ² holds, so a longer draw changes nothing but
/// its cost.
const DRAW_SPAN_S: f64 = 4.0 * 3600.0;

/// What white readings read once detrended: the mean correlation at each
/// lag (0 at lag 0), and the mean and variance of their Ljung–Box Q.
#[derive(Debug, Clone)]
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

/// [`WhiteNull`] for `g`, to `max_lag` lags, with Q over up to `lb_lags`.
/// The seed comes only from the readings' count and where the gaps are, so
/// one take always gives one p. Kept per thread ([`CACHED`]).
pub(super) fn white_null(g: &Grid, max_lag: usize, lb_lags: usize) -> WhiteNull {
    let len =
        g.y.len()
            .min(((DRAW_SPAN_S / g.step).round() as usize).max(1));
    let present: Vec<bool> = g.y[..len].iter().map(|v| v.is_finite()).collect();
    let mut seed = 0xcbf2_9ce4_8422_2325u64 ^ len as u64;
    for (i, _) in present.iter().enumerate().filter(|(_, p)| !**p) {
        seed = (seed ^ i as u64).wrapping_mul(0x0100_0000_01b3);
    }
    let key = (len, g.step.to_bits(), seed, max_lag, lb_lags);
    let hit = CACHE.with(|c| c.borrow().iter().find(|e| e.0 == key).map(|e| e.1.clone()));
    if let Some(w) = hit {
        return w;
    }
    let w = drawn_null(&present, g.step, seed, max_lag, lb_lags);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= CACHED {
            c.remove(0);
        }
        c.push((key, w.clone()));
    });
    w
}

/// Nulls kept per thread: the rate, amplitude and beat error of one take
/// share their readings' count and gaps.
const CACHED: usize = 16;

type Key = (usize, u64, u64, usize, usize);

thread_local! {
    static CACHE: std::cell::RefCell<Vec<(Key, WhiteNull)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// [`white_null`], drawn: white readings where `present`, detrended as a
/// take on that grid is.
fn drawn_null(present: &[bool], step: f64, seed: u64, max_lag: usize, lb_lags: usize) -> WhiteNull {
    let len = present.len();
    let n = present.iter().filter(|&&p| p).count();
    let count = (DRAW_BUDGET / len.max(1)).clamp(MIN_DRAWS, MAX_DRAWS);
    let draws: Vec<(Vec<f64>, Vec<usize>)> = (0..count as u64)
        .map(|s| {
            let mut z = crate::synth::Rng::new(seed.wrapping_add(s));
            let y = present
                .iter()
                .map(|&p| if p { z.normal() } else { f64::NAN })
                .collect();
            let d = longterm::detrend(&Grid { t0: 0.0, step, y }, longterm::DETREND_S);
            acf_pairs(&d.y, max_lag)
        })
        .collect();
    let mut bias = vec![0.0; max_lag + 1];
    for (r, _) in &draws {
        for (b, v) in bias.iter_mut().zip(r).skip(1) {
            if v.is_finite() {
                *b += v / count as f64;
            }
        }
    }
    let qs: Vec<f64> = draws
        .iter()
        .map(|(r, pairs)| {
            let r: Vec<f64> = r.iter().zip(&bias).map(|(v, b)| v - b).collect();
            ljung_q(&r, pairs, n, &q_lags(&r, pairs, lb_lags))
        })
        .collect();
    let q_mean = qs.iter().sum::<f64>() / qs.len() as f64;
    let q_var = qs.iter().map(|q| (q - q_mean).powi(2)).sum::<f64>() / (qs.len() - 1) as f64;
    let q_skew =
        qs.iter().map(|q| (q - q_mean).powi(3)).sum::<f64>() / qs.len() as f64 / q_var.powf(1.5);
    WhiteNull {
        bias,
        q_mean,
        q_var,
        q_skew,
    }
}
