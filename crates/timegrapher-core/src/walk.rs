//! The isochronism walk (#741): the take's rate against its amplitude, one
//! point per stretch of time, in time order, so a long run-down shows how
//! the watch walks through rate and amplitude as the mainspring lets down.
//! It sits beside the isochronism curve (rate binned by amplitude), not in
//! its place: a short take's wheel cycle moves rate and amplitude together
//! every few minutes, which the curve keeps and a time bin averages away.
//!
//! Core reads the points on a fine base of [`WALK_BIN_S`]; each surface
//! merges them to its own width with [`merged`], [`default_merge`] giving
//! 100 to 250 points a take. Bins of an hour or more smear steps and pull
//! the extremes in.

use crate::dsp::median_of;
use crate::longterm::Grid;
use serde::Serialize;

/// The walk's base bin, seconds.
pub const WALK_BIN_S: f64 = 300.0;
/// The most points [`default_merge`] leaves a take.
pub const WALK_MAX_POINTS: usize = 250;

/// One stretch of the take: the median rate and amplitude of the readings
/// in it where both were read.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WalkPoint {
    /// The stretch's middle, seconds from the take's start, and hours
    /// since full wind.
    pub t_s: f64,
    pub wind_h: f64,
    pub rate_s_per_day: f64,
    pub amplitude_deg: f64,
    /// Readings with both a rate and an amplitude.
    pub readings: usize,
}

/// The walk on its base bins, in time order. A bin with under half its
/// readings is left out, never filled.
#[derive(Debug, Clone, Serialize)]
pub struct IsochronismWalk {
    pub bin_s: f64,
    pub points: Vec<WalkPoint>,
}

/// The walk from the rate and the amplitude read at the same reading
/// length on the same grid (the steadiness report's readings), with
/// `wind_h` the hours since full wind at the take's start. `None` with
/// fewer than two bins.
pub fn walk(rate: &Grid, amp: &Grid, wind_h: f64) -> Option<IsochronismWalk> {
    let n = rate.y.len().min(amp.y.len());
    let per_bin = (WALK_BIN_S / rate.step).round().max(1.0) as usize;
    let points: Vec<WalkPoint> = (0..n)
        .step_by(per_bin)
        .filter_map(|i0| {
            let both: Vec<(f64, f64)> = (i0..(i0 + per_bin).min(n))
                .map(|i| (rate.y[i], amp.y[i]))
                .filter(|(r, a)| r.is_finite() && a.is_finite())
                .collect();
            if 2 * both.len() < per_bin {
                return None;
            }
            let t_s = rate.t0 + (i0 as f64 + per_bin as f64 / 2.0) * rate.step;
            Some(WalkPoint {
                t_s,
                wind_h: wind_h + t_s / 3600.0,
                rate_s_per_day: median_of(both.iter().map(|b| b.0))?,
                amplitude_deg: median_of(both.iter().map(|b| b.1))?,
                readings: both.len(),
            })
        })
        .collect();
    (points.len() >= 2).then_some(IsochronismWalk {
        bin_s: per_bin as f64 * rate.step,
        points,
    })
}

/// How many base bins to merge so the walk has at most
/// [`WALK_MAX_POINTS`] points.
pub fn default_merge(w: &IsochronismWalk) -> usize {
    w.points.len().div_ceil(WALK_MAX_POINTS).max(1)
}

/// The walk with every `k` base bins (by time, so a left-out bin leaves
/// its merged bin thinner) merged into one: each point is the
/// readings-weighted mean of its bins' medians and time.
pub fn merged(w: &IsochronismWalk, k: usize) -> IsochronismWalk {
    let k = k.max(1);
    let width = w.bin_s * k as f64;
    let mut out: Vec<WalkPoint> = Vec::new();
    let mut group: Vec<&WalkPoint> = Vec::new();
    let slot = |p: &WalkPoint| (p.t_s / width).floor() as i64;
    for p in &w.points {
        if group.last().is_some_and(|q| slot(q) != slot(p)) {
            out.push(merge(&group));
            group.clear();
        }
        group.push(p);
    }
    if !group.is_empty() {
        out.push(merge(&group));
    }
    IsochronismWalk {
        bin_s: width,
        points: out,
    }
}

fn merge(g: &[&WalkPoint]) -> WalkPoint {
    if let [p] = g {
        return (*p).clone();
    }
    let n: usize = g.iter().map(|p| p.readings).sum();
    let mean = |f: fn(&WalkPoint) -> f64| {
        g.iter().map(|p| f(p) * p.readings as f64).sum::<f64>() / n.max(1) as f64
    };
    WalkPoint {
        t_s: mean(|p| p.t_s),
        wind_h: mean(|p| p.wind_h),
        rate_s_per_day: mean(|p| p.rate_s_per_day),
        amplitude_deg: mean(|p| p.amplitude_deg),
        readings: n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run-down: amplitude falls 300° to 200° over `hours`, the rate
    /// rises 2 s/d per 10° lost, readings every 10 s.
    fn run_down(hours: f64) -> (Grid, Grid) {
        let n = (hours * 360.0) as usize;
        let amp: Vec<f64> = (0..n)
            .map(|i| 300.0 - 100.0 * i as f64 / n as f64)
            .collect();
        let rate = amp.iter().map(|a| 2.0 * (300.0 - a) / 10.0).collect();
        let g = |y| Grid {
            t0: 0.0,
            step: 10.0,
            y,
        };
        (g(rate), g(amp))
    }

    #[test]
    fn a_run_down_walks_down_the_amplitude_in_time_order() {
        let (rate, amp) = run_down(10.0);
        let w = walk(&rate, &amp, 1.0).unwrap();
        assert_eq!(w.bin_s, 300.0);
        assert_eq!(w.points.len(), 120);
        let p = &w.points[0];
        assert_eq!((p.t_s, p.readings), (150.0, 30));
        assert!((p.wind_h - (1.0 + 150.0 / 3600.0)).abs() < 1e-12);
        assert!((p.amplitude_deg - (300.0 - 100.0 * 14.5 / 3600.0)).abs() < 1e-9);
        assert!((p.rate_s_per_day - 2.0 * (300.0 - p.amplitude_deg) / 10.0).abs() < 1e-9);
        assert!(w.points.windows(2).all(|q| q[1].t_s > q[0].t_s
            && q[1].amplitude_deg < q[0].amplitude_deg
            && q[1].rate_s_per_day > q[0].rate_s_per_day));
    }

    #[test]
    fn a_bin_under_half_read_is_left_out_not_filled() {
        let (mut rate, mut amp) = run_down(1.0);
        // Bin 2 keeps 15 of 30 readings, bin 3 keeps 14; a rate without an
        // amplitude does not count.
        rate.y[60..75].fill(f64::NAN);
        rate.y[90..106].fill(f64::NAN);
        amp.y[30] = f64::NAN;
        let w = walk(&rate, &amp, 0.0).unwrap();
        let ts: Vec<f64> = w.points.iter().map(|p| p.t_s).collect();
        assert_eq!(ts.len(), 11);
        assert!(ts.contains(&750.0) && !ts.contains(&1050.0));
        assert_eq!(w.points[1].readings, 29);
        assert_eq!(w.points[2].readings, 15);
    }

    #[test]
    fn under_two_bins_is_no_walk() {
        let (rate, amp) = run_down(5.0 / 60.0);
        assert!(walk(&rate, &amp, 0.0).is_none());
    }

    #[test]
    fn merging_keeps_a_long_take_to_250_points_weighted_by_readings() {
        let (rate, amp) = run_down(65.0);
        let w = walk(&rate, &amp, 0.0).unwrap();
        assert_eq!(w.points.len(), 780);
        assert_eq!(default_merge(&w), 4);
        let m = merged(&w, default_merge(&w));
        assert_eq!((m.bin_s, m.points.len()), (1200.0, 195));
        assert_eq!(m.points[0].readings, 120);
        assert!((m.points[0].t_s - 600.0).abs() < 1e-9);
        let a: f64 = w.points[..4].iter().map(|p| p.amplitude_deg).sum::<f64>() / 4.0;
        assert!((m.points[0].amplitude_deg - a).abs() < 1e-9);
        // A thin bin weighs less.
        let mut thin = w.clone();
        thin.points[0].readings = 15;
        let m = merged(&thin, 2);
        let want =
            (15.0 * thin.points[0].rate_s_per_day + 30.0 * thin.points[1].rate_s_per_day) / 45.0;
        assert!((m.points[0].rate_s_per_day - want).abs() < 1e-12);
        assert_eq!(merged(&w, 1).points, w.points);
        let (rate, amp) = run_down(10.0);
        assert_eq!(default_merge(&walk(&rate, &amp, 0.0).unwrap()), 1);
    }
}
