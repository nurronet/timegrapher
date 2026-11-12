//! Dynamic poise: the once-round wave of rate as the watch turns in its
//! own plane, and the heavy spot it points to (#1474; the math ruling is
//! the math standards guide's Poise entry).
//!
//! The positions are upright, at dial angle α: the hour mark at the top,
//! clockwise from 12H (12H 0°, 3H 90°, 6H 180°, 9H 270°, and the 45° steps
//! between). Two fits are made, and kept apart:
//!
//! - **The measured wave (a fact).** rate(α) = m + A·cos(α − φ): its size A
//!   each way from the mean (half the peak to peak) and the angle φ at
//!   which the watch ran fastest, each with its ±.
//! - **The heavy spot (an inference).** A poise error U changes the rate by
//!   U·f(θ)·cos(α − ψ), with f(θ) = 2·J1(θ)/θ of each position's own
//!   amplitude θ. f is 1 for a small swing, 0.18 at 180°, 0 at 219.5° and
//!   negative above, so the same heavy spot makes the watch fastest with
//!   it low below 219.5° and with it high above. The regressors are
//!   f(θ_i)·cos α_i and f(θ_i)·sin α_i and a constant, so ψ (the hour at the
//!   top when the heavy spot is lowest at rest) rests on the amplitude, and
//!   so on the lift angle. It is withheld when any position's amplitude is
//!   within reach of 219.5° ([`in_null_band`]).
//!
//! Both are weighted least squares, each position by 1/SE² of its rate,
//! and the ± comes from the fit; with any position lacking a ± the fit is
//! unweighted and gives none. The twice-round terms (cos 2α, sin 2α), fitted
//! beside the wave, are a check on the residual and no part of the answer:
//! pivots and the hairspring give them, poise does not.
//!
//! The amplitude itself can vary round the dial (friction differs by
//! position), and through the watch's isochronism that moves the rate in
//! the same once-round pattern. With a measured isochronism slope it is
//! taken off before the heavy spot is fitted; with none, a once-round
//! amplitude wave clear of zero by 2 SE is named as a caveat.

use crate::figure_text::{base_digits, figure_with_pm, fixed_text};
use serde::Serialize;

/// The fewest upright positions a wave is fitted from: it has three
/// unknowns.
pub const MIN_POSITIONS: usize = 3;

/// The amplitude at which poise does not move the rate, degrees: the first
/// zero of J1, 3.8317 rad.
pub const NULL_AMPLITUDE_DEG: f64 = 219.5;

/// The positions measured before a pane shows the wave: three fix it,
/// and five leave two to check its shape (Hugh's spec, 2026-11-12).
pub const MIN_SHOWN: usize = 5;

/// What a pane says until [`MIN_SHOWN`] positions are in.
pub fn pending_text() -> String {
    format!("Fitted once {MIN_SHOWN} positions are measured")
}

/// The share of each amplitude added either side of its 2 SE before asking
/// whether it reaches [`NULL_AMPLITUDE_DEG`]. Amplitude scales with the
/// lift angle, and a catalogue lift can be several percent out: the one
/// measured (DD3285) moved 7.3%, 55° to 59°. The math ruling of
/// 2026-11-12; a later change may drop it for a measured lift.
pub const NULL_BAND_SHARE: f64 = 0.08;

/// How many standard errors clear of zero a once-round amplitude wave must
/// be to be named, and how far either side of an amplitude the null band
/// reaches: the guide's 2 SE.
pub const CLEAR_SE: f64 = 2.0;

/// One upright position's figures for the fits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// The angle of the hour mark at the top, degrees ([`hour_code`]).
    pub angle_deg: f64,
    pub rate_s_per_day: f64,
    pub rate_se: Option<f64>,
    pub amplitude_deg: Option<f64>,
    pub amplitude_se: Option<f64>,
}

/// A watch's isochronism: how its rate moves with amplitude, s/d per
/// degree, with its ± (core's isochronism from a run-down).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Slope {
    pub s_per_day_per_deg: f64,
    pub se: f64,
}

/// A fitted figure and its standard error.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Term {
    pub value: f64,
    pub se: Option<f64>,
}

/// A once-round wave: its mean, its size each way from the mean, and the
/// angle of its peak, degrees 0 to 360.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Wave {
    pub mean: Term,
    pub size: Term,
    pub peak_deg: Term,
}

/// Why the heavy spot is not inferred.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum Withheld {
    /// A position's amplitude is within reach of 219.5°, where poise
    /// barely moves the rate: the first such, its angle and amplitude.
    NullBand { angle_deg: f64, amplitude_deg: f64 },
    /// A position has no amplitude.
    NoAmplitude,
    /// The amplitude-weighted fit cannot be solved.
    NoFit,
}

/// What limits the heavy spot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Caveat {
    /// The amplitude itself varies once round the dial, clear of zero by
    /// [`CLEAR_SE`]: through isochronism part of the rate wave may come
    /// from that. Its size each way, degrees.
    AmplitudeWave { size_deg: f64, se_deg: f64 },
    /// This isochronism was taken off the rates before the heavy spot was
    /// fitted.
    IsochronismRemoved { slope: Slope },
}

/// The two fits over the upright positions.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PoiseFit {
    pub positions: usize,
    /// The measured rate wave, s/d and degrees: a fact.
    pub wave: Wave,
    /// The twice-round terms beside it, s/d, where the positions leave
    /// room: cos 2α from four, sin 2α too from five.
    pub shape_cos2_s_per_day: Option<Term>,
    pub shape_sin2_s_per_day: Option<Term>,
    /// The inferred poise error: its size U (s/d at a small swing) and the
    /// hour at the top (degrees) when the heavy spot is lowest at rest.
    pub heavy_spot: Option<Wave>,
    pub withheld: Option<Withheld>,
    pub caveats: Vec<Caveat>,
    /// The amplitude's own once-round wave, degrees, where every position
    /// has one.
    pub amplitude_wave: Option<Wave>,
    pub amplitude_min_deg: Option<f64>,
    pub amplitude_max_deg: Option<f64>,
    /// The fit in words ([`PoiseFit::sentence`]) and in one line
    /// ([`PoiseFit::short`]).
    pub text: String,
    pub short_text: String,
    /// The heavy spot's line ([`PoiseFit::heavy_line`]).
    pub heavy_text: String,
}

/// f(θ) = 2·J1(θ)/θ of an amplitude in degrees: how much of a small
/// swing's poise effect is left at that amplitude.
pub fn amplitude_factor(amplitude_deg: f64) -> f64 {
    let x = amplitude_deg.to_radians();
    // 2·J1(x)/x = Σ (−1)^k (x/2)^(2k) / (k!(k+1)!), whose terms shrink
    // fast enough over any amplitude a balance reaches.
    let q = -(x * x) / 4.0;
    let mut term = 1.0;
    let mut sum = 1.0;
    for k in 1..60 {
        term *= q / (k as f64 * (k + 1) as f64);
        sum += term;
        if term.abs() < 1e-17 {
            break;
        }
    }
    sum
}

/// Whether an amplitude, with its ±, may sit where poise barely moves the
/// rate: [θ − 2·SE, θ + 2·SE] widened by [`NULL_BAND_SHARE`] of θ holds
/// [`NULL_AMPLITUDE_DEG`].
pub fn in_null_band(amplitude_deg: f64, se: Option<f64>) -> bool {
    let se = se.filter(|s| s.is_finite()).unwrap_or(0.0).abs();
    let reach = CLEAR_SE * se + NULL_BAND_SHARE * amplitude_deg.abs();
    (amplitude_deg - NULL_AMPLITUDE_DEG).abs() <= reach
}

/// Witschi's code of the hour nearest `deg` on the half hour: "12H" to
/// "11H", and "1H30" and so on for the 45° steps between.
pub fn hour_code(deg: f64) -> String {
    let half_hours = ((deg.rem_euclid(360.0) / 15.0).round() as i64).rem_euclid(24);
    let h = match half_hours / 2 {
        0 => 12,
        h => h,
    };
    if half_hours % 2 == 1 {
        format!("{h}H30")
    } else {
        format!("{h}H")
    }
}

/// Both fits over `points`, one per upright position, taking `slope` off
/// before the heavy spot where the watch's isochronism is known. None with
/// fewer than [`MIN_POSITIONS`] or angles that cannot fix a wave.
pub fn fit(points: &[Point], slope: Option<Slope>) -> Option<PoiseFit> {
    let pts: Vec<Point> = points
        .iter()
        .copied()
        .filter(|p| p.angle_deg.is_finite() && p.rate_s_per_day.is_finite())
        .collect();
    if pts.len() < MIN_POSITIONS {
        return None;
    }
    let rates: Vec<f64> = pts.iter().map(|p| p.rate_s_per_day).collect();
    let rate_ses = all_se(pts.iter().map(|p| p.rate_se));
    let ones = vec![1.0; pts.len()];
    let wave = wave_fit(&pts, &ones, &rates, rate_ses.as_deref())?;
    let (shape_cos2_s_per_day, shape_sin2_s_per_day) = shape_fit(&pts, &rates, rate_ses.as_deref());
    let amps: Option<Vec<f64>> = pts
        .iter()
        .map(|p| p.amplitude_deg.filter(|a| a.is_finite()))
        .collect();
    let amplitude_wave = amps.as_ref().and_then(|a| {
        let ses = all_se(pts.iter().map(|p| p.amplitude_se));
        wave_fit(&pts, &ones, a, ses.as_deref())
    });
    let mut caveats = Vec::new();
    let (heavy_spot, withheld) = match &amps {
        None => (None, Some(Withheld::NoAmplitude)),
        Some(a) => {
            let ses = rate_ses.as_deref();
            heavy_fit(&pts, a, ses, slope, amplitude_wave, &mut caveats)
        }
    };
    let flat = amps.iter().flatten().copied();
    let mut out = PoiseFit {
        positions: pts.len(),
        wave,
        shape_cos2_s_per_day,
        shape_sin2_s_per_day,
        heavy_spot,
        withheld,
        caveats,
        amplitude_wave,
        amplitude_min_deg: flat.clone().reduce(f64::min),
        amplitude_max_deg: flat.reduce(f64::max),
        text: String::new(),
        short_text: String::new(),
        heavy_text: String::new(),
    };
    out.text = out.sentence();
    out.short_text = out.short();
    out.heavy_text = out.heavy_line();
    Some(out)
}

/// The heavy spot, or why it is withheld; any caveat on it goes into
/// `caveats`.
fn heavy_fit(
    pts: &[Point],
    amps: &[f64],
    rate_ses: Option<&[f64]>,
    slope: Option<Slope>,
    amplitude_wave: Option<Wave>,
    caveats: &mut Vec<Caveat>,
) -> (Option<Wave>, Option<Withheld>) {
    if let Some((p, &a)) = pts
        .iter()
        .zip(amps)
        .find(|(p, &a)| in_null_band(a, p.amplitude_se))
    {
        let why = Withheld::NullBand {
            angle_deg: p.angle_deg,
            amplitude_deg: a,
        };
        return (None, Some(why));
    }
    let mut rates: Vec<f64> = pts.iter().map(|p| p.rate_s_per_day).collect();
    let mut ses = rate_ses.map(<[f64]>::to_vec);
    if let Some(s) = slope {
        let mean = amps.iter().sum::<f64>() / amps.len() as f64;
        for (i, a) in amps.iter().enumerate() {
            rates[i] -= s.s_per_day_per_deg * (a - mean);
            if let Some(ses) = ses.as_mut() {
                ses[i] = ses[i].hypot(s.se * (a - mean));
            }
        }
        caveats.push(Caveat::IsochronismRemoved { slope: s });
    } else if let Some(w) = amplitude_wave {
        if let Some(se) = w.size.se.filter(|se| w.size.value - CLEAR_SE * se > 0.0) {
            caveats.push(Caveat::AmplitudeWave {
                size_deg: w.size.value,
                se_deg: se,
            });
        }
    }
    let f: Vec<f64> = amps.iter().map(|&a| amplitude_factor(a)).collect();
    match wave_fit(pts, &f, &rates, ses.as_deref()) {
        Some(w) => (Some(w), None),
        None => (None, Some(Withheld::NoFit)),
    }
}

/// Every standard error, or none if any is missing or not positive.
fn all_se(ses: impl Iterator<Item = Option<f64>>) -> Option<Vec<f64>> {
    ses.map(|s| s.filter(|s| s.is_finite() && *s > 0.0))
        .collect()
}

/// y = m + scale_i·(a·cos α_i + b·sin α_i), weighted by 1/se² where every
/// point has one: the wave's mean, size √(a² + b²) and peak angle.
fn wave_fit(pts: &[Point], scale: &[f64], y: &[f64], ses: Option<&[f64]>) -> Option<Wave> {
    let rows: Vec<Vec<f64>> = pts
        .iter()
        .zip(scale)
        .map(|(p, k)| {
            let t = p.angle_deg.to_radians();
            vec![1.0, k * t.cos(), k * t.sin()]
        })
        .collect();
    let (beta, cov) = wls(&rows, y, ses)?;
    let (a, b) = (beta[1], beta[2]);
    let size = a.hypot(b);
    let (size_se, peak_se) = match &cov {
        Some(c) if size > 0.0 => {
            let quad = |g: [f64; 2]| {
                (g[0] * g[0] * c[1][1] + 2.0 * g[0] * g[1] * c[1][2] + g[1] * g[1] * c[2][2])
                    .max(0.0)
                    .sqrt()
            };
            let s2 = size * size;
            (
                Some(quad([a / size, b / size])),
                Some(quad([-b / s2, a / s2]).to_degrees()),
            )
        }
        _ => (None, None),
    };
    Some(Wave {
        mean: Term {
            value: beta[0],
            se: cov.as_ref().map(|c| c[0][0].max(0.0).sqrt()),
        },
        size: Term {
            value: size,
            se: size_se,
        },
        peak_deg: Term {
            value: b.atan2(a).to_degrees().rem_euclid(360.0),
            se: peak_se,
        },
    })
}

/// The twice-round terms, fitted beside the once-round wave: cos 2α where
/// the positions leave room for a fourth unknown, sin 2α where they leave
/// room for one more, each only where it is not zero at every position
/// (sin 2α is, at 90° steps).
fn shape_fit(pts: &[Point], y: &[f64], ses: Option<&[f64]>) -> (Option<Term>, Option<Term>) {
    let n = pts.len();
    let col = |f: fn(f64) -> f64| -> Vec<f64> {
        pts.iter()
            .map(|p| f(2.0 * p.angle_deg.to_radians()))
            .collect()
    };
    let readable = |c: &[f64]| c.iter().map(|x| x * x).sum::<f64>() > 1e-9 * n as f64;
    let (c2, s2) = (col(f64::cos), col(f64::sin));
    let use_c2 = n > MIN_POSITIONS && readable(&c2);
    let use_s2 = n > MIN_POSITIONS + usize::from(use_c2) && readable(&s2);
    if !use_c2 && !use_s2 {
        return (None, None);
    }
    let rows: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let t = pts[i].angle_deg.to_radians();
            let mut r = vec![1.0, t.cos(), t.sin()];
            r.extend(use_c2.then_some(c2[i]));
            r.extend(use_s2.then_some(s2[i]));
            r
        })
        .collect();
    let Some((beta, cov)) = wls(&rows, y, ses) else {
        return (None, None);
    };
    let term = |k: usize| Term {
        value: beta[k],
        se: cov.as_ref().map(|c| c[k][k].max(0.0).sqrt()),
    };
    let c2_term = use_c2.then(|| term(3));
    let s2_term = use_s2.then(|| term(3 + usize::from(use_c2)));
    (c2_term, s2_term)
}

type Matrix = Vec<Vec<f64>>;

/// Least squares of `y` on `rows`, weighted by 1/se² with `ses`, with its
/// covariance (XᵀWX)⁻¹; unweighted and with no covariance without.
fn wls(rows: &[Vec<f64>], y: &[f64], ses: Option<&[f64]>) -> Option<(Vec<f64>, Option<Matrix>)> {
    let k = rows.first()?.len();
    if rows.len() < k {
        return None;
    }
    let w = |i: usize| ses.map_or(1.0, |s| 1.0 / (s[i] * s[i]));
    let mut m = vec![vec![0.0; k]; k];
    let mut xy = vec![0.0; k];
    for (i, x) in rows.iter().enumerate() {
        for a in 0..k {
            xy[a] += w(i) * x[a] * y[i];
            for b in 0..k {
                m[a][b] += w(i) * x[a] * x[b];
            }
        }
    }
    let inv = invert(m)?;
    let beta = (0..k)
        .map(|a| (0..k).map(|b| inv[a][b] * xy[b]).sum())
        .collect();
    Some((beta, ses.map(|_| inv)))
}

/// The inverse of a small matrix by Gauss–Jordan with partial pivoting;
/// None when it is singular to working precision.
fn invert(mut m: Matrix) -> Option<Matrix> {
    let k = m.len();
    let scale = m.iter().flatten().map(|v| v.abs()).fold(0.0, f64::max);
    let mut inv: Matrix = (0..k)
        .map(|i| (0..k).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    for c in 0..k {
        let p = (c..k).max_by(|&a, &b| m[a][c].abs().total_cmp(&m[b][c].abs()))?;
        if m[p][c].abs() <= 1e-10 * scale || !m[p][c].is_finite() {
            return None;
        }
        m.swap(c, p);
        inv.swap(c, p);
        let d = m[c][c];
        for j in 0..k {
            m[c][j] /= d;
            inv[c][j] /= d;
        }
        for r in (0..k).filter(|&r| r != c) {
            let f = m[r][c];
            for j in 0..k {
                m[r][j] -= f * m[c][j];
                inv[r][j] -= f * inv[c][j];
            }
        }
    }
    Some(inv)
}

impl PoiseFit {
    /// The fit in words: the measured wave, its twice-round check and the
    /// amplitude, then the heavy spot or why it is not given, and any
    /// caveat on it.
    pub fn sentence(&self) -> String {
        let rate = |t: &Term, signed| figure_with_pm(t.value, t.se, base_digits::RATE, signed);
        let w = &self.wave;
        let mut s = format!(
            "Once-round wave over {} upright positions: {} s/d each way from the mean, \
             fastest at {}° (nearest {} at the top).",
            self.positions,
            rate(&w.size, false),
            figure_with_pm(w.peak_deg.value, w.peak_deg.se, 0, false),
            hour_code(w.peak_deg.value),
        );
        let shape: Vec<String> = [
            ("cos 2α", &self.shape_cos2_s_per_day),
            ("sin 2α", &self.shape_sin2_s_per_day),
        ]
        .iter()
        .filter_map(|(name, t)| t.as_ref().map(|t| format!("{name} {} s/d", rate(t, true))))
        .collect();
        if !shape.is_empty() {
            s.push_str(&format!(" Twice-round terms: {}.", shape.join(", ")));
        }
        if let Some(range) = self.amplitude_range() {
            s.push_str(&format!(" Amplitude {range}°."));
        }
        s.push(' ');
        s.push_str(&self.heavy_line());
        if self.heavy_spot.is_some() {
            s.push_str(
                " The heavy spot is from the fit at each position's own amplitude, so it rests \
                 on the lift angle.",
            );
        } else {
            s.push_str(&format!(
                " Poise moves the rate least at {}° amplitude, and the other way above it.",
                fixed_text(NULL_AMPLITUDE_DEG, 1)
            ));
        }
        s
    }

    /// The second line: the heavy spot, or why it is not stated, then
    /// any caveat on it. Inferred, where the title line is measured.
    pub fn heavy_line(&self) -> String {
        let mut s = match (&self.heavy_spot, self.withheld) {
            (Some(h), _) => format!(
                "Heavy spot hangs lowest with {} at the top ({}°).",
                hour_code(h.peak_deg.value),
                figure_with_pm(h.peak_deg.value, h.peak_deg.se, 0, false),
            ),
            (None, Some(Withheld::NullBand { .. })) => format!(
                "At {}°, poise barely moves the rate. Heavy spot not stated.",
                self.amplitude_range().unwrap_or_default()
            ),
            (None, Some(Withheld::NoAmplitude)) => {
                "Heavy spot not stated: a position has no amplitude.".into()
            }
            (None, _) => "Heavy spot not stated.".into(),
        };
        for c in &self.caveats {
            s.push(' ');
            s.push_str(&caveat_text(c));
        }
        s
    }

    /// The fit in one line: the wave's size, the hour nearest its peak,
    /// and the amplitude it was read at.
    pub fn short(&self) -> String {
        let size = &self.wave.size;
        let mut s = format!(
            "Once-round wave: {} s/d each way, fastest near {} at the top",
            figure_with_pm(size.value, size.se, base_digits::RATE, false),
            hour_code(self.wave.peak_deg.value)
        );
        if let Some(range) = self.amplitude_range() {
            s.push_str(&format!(" · at {range}°"));
        }
        s
    }

    /// The amplitude range as text, "159–186", or one figure when both
    /// ends round alike.
    fn amplitude_range(&self) -> Option<String> {
        let d = base_digits::AMPLITUDE;
        let lo = fixed_text(self.amplitude_min_deg?, d);
        let hi = fixed_text(self.amplitude_max_deg?, d);
        Some(if lo == hi { lo } else { format!("{lo}–{hi}") })
    }

    /// The measured wave at `steps` + 1 evenly spaced angles from 0° to
    /// 360°, (angle, rate s/d): the one source of the drawn curve, on a
    /// line or round a dial.
    pub fn curve(&self, steps: usize) -> Vec<(f64, f64)> {
        let steps = steps.max(1);
        let w = &self.wave;
        (0..=steps)
            .map(|i| {
                let deg = 360.0 * i as f64 / steps as f64;
                let r = w.mean.value + w.size.value * (deg - w.peak_deg.value).to_radians().cos();
                (deg, r)
            })
            .collect()
    }
}

fn caveat_text(c: &Caveat) -> String {
    match c {
        Caveat::AmplitudeWave { size_deg, se_deg } => format!(
            "Amplitude also changes once round, {}° each way, so part of the wave may be \
             isochronism.",
            figure_with_pm(*size_deg, Some(*se_deg), base_digits::AMPLITUDE, false)
        ),
        Caveat::IsochronismRemoved { slope } => format!(
            "Corrected for isochronism, {} s/d per degree.",
            figure_with_pm(
                slope.s_per_day_per_deg,
                Some(slope.se),
                base_digits::SLOPE,
                true
            )
        ),
    }
}

#[cfg(test)]
#[path = "poise_tests.rs"]
mod tests;
