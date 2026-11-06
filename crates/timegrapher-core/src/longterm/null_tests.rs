use super::*;
use crate::longterm::{detrend, detrend_window, search, spectrum, x_scale, Band, Grid, DETREND_S};
use crate::synth::Rng;

/// The table's take lengths (s) and grid steps, by model: the steps the
/// pipeline reads each at (timing at `longrun::timing_step`, readings on
/// 2 s windows, or 4 s over 48 h).
fn rows(model: NullModel) -> Vec<(f64, f64)> {
    const SHORT: [f64; 5] = [120.0, 300.0, 600.0, 900.0, 1199.0];
    const TO_12H: [f64; 11] = [
        1200.0, 1800.0, 2700.0, 3600.0, 5400.0, 7200.0, 10800.0, 16200.0, 21600.0, 32400.0, 43200.0,
    ];
    const TO_48H: [f64; 5] = [43200.0, 64800.0, 86400.0, 129600.0, 172800.0];
    const TO_144H: [f64; 4] = [172800.0, 259200.0, 345600.0, 518400.0];
    let mut out = Vec::new();
    match model {
        NullModel::Timing => {
            out.extend(SHORT.iter().chain(&TO_12H).map(|&s| (s, 0.25)));
            out.extend(TO_48H.iter().map(|&s| (s, 0.5)));
            out.extend(TO_144H.iter().map(|&s| (s, 4.0)));
        }
        NullModel::Readings => {
            out.extend(
                SHORT
                    .iter()
                    .chain(&TO_12H)
                    .chain(&TO_48H[1..])
                    .chain(&TO_144H[1..])
                    .map(|&s| (s, 2.0)),
            );
            out.extend(TO_144H.iter().map(|&s| (s, 4.0)));
        }
    }
    out
}

fn mixes(model: NullModel, scale: f64, step: f64) -> Vec<f64> {
    match model {
        NullModel::Readings => vec![0.0, 0.3, 0.5, 0.7, 0.8, 0.9, 0.95, 0.99],
        // Up to a crossover of 0.35 of the sampling rate: the fit can't
        // see a floor above that.
        NullModel::Timing => [
            0.0, 1.0, 3.0, 10.0, 30.0, 100.0, 300.0, 1000.0, 3000.0, 10000.0,
        ]
        .into_iter()
        .filter(|&m| m / scale < 0.35 / step)
        .chain([WALK_MIX])
        .collect(),
    }
}

/// One null series: AR(1) at ρ `mix`, or a random walk (1 s^-½) plus
/// white noise with the crossover at `mix / scale`.
fn null_grid(model: NullModel, span: f64, step: f64, mix: f64, scale: f64, seed: u64) -> Grid {
    let n = (span / step).round() as usize;
    let mut rng = Rng::new(seed);
    let y = match model {
        NullModel::Readings => {
            let innov = (1.0 - mix * mix).sqrt();
            let mut x = rng.normal();
            (0..n)
                .map(|_| {
                    let v = x;
                    x = mix * x + innov * rng.normal();
                    v
                })
                .collect()
        }
        NullModel::Timing if mix == 0.0 => (0..n).map(|_| rng.normal()).collect(),
        NullModel::Timing if mix >= WALK_MIX => {
            let mut x = 0.0;
            (0..n)
                .map(|_| {
                    x += step.sqrt() * rng.normal();
                    x
                })
                .collect()
        }
        NullModel::Timing => {
            // a = step (the walk's variance a step), b = r²: the crossover
            // has sin(π f_c Δ) = √a / (2r).
            let f_c = mix / scale;
            let r = step.sqrt() / (2.0 * (std::f64::consts::PI * f_c * step).sin());
            let mut x = 0.0;
            (0..n)
                .map(|_| {
                    x += step.sqrt() * rng.normal();
                    x + r * rng.normal()
                })
                .collect()
        }
    };
    Grid { t0: 0.0, step, y }
}

fn seed_of(parts: &[u64]) -> u64 {
    parts.iter().fold(0x9e37_79b9_7f4a_7c15u64, |h, &p| {
        (h ^ p).wrapping_mul(0x0100_0000_01b3).rotate_left(17)
    })
}

/// One cell as measured, before the row's shape is fitted.
#[derive(Serialize, Deserialize)]
struct Raw {
    model: NullModel,
    span_s: f64,
    step_s: f64,
    mix: f64,
    /// The Gumbel's decades per unit of score, and its location less
    /// log10(trials)/slope.
    slope: f64,
    loc0: f64,
    mean: Vec<Option<f64>>,
}

#[derive(Serialize, Deserialize)]
struct Measured {
    cell: Raw,
    fitted_mix: f64,
    smax: Vec<f64>,
    /// The fitted Gumbel's location and scale, score units.
    mu: f64,
    sigma: f64,
}

/// Maximum-likelihood Gumbel (location, scale) of the maxima `x`.
fn gumbel_fit(x: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let mean = x.iter().sum::<f64>() / n;
    let top = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    // The scale solves σ = mean − Σ x e^(−x/σ) / Σ e^(−x/σ).
    let g = |sigma: f64| {
        let (mut a, mut b) = (0.0, 0.0);
        for &v in x {
            let e = (-(v - top) / sigma).exp();
            a += v * e;
            b += e;
        }
        sigma - mean + a / b
    };
    let (mut lo, mut hi) = (1e-3, 20.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if g(mid) > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let sigma = 0.5 * (lo + hi);
    let m = x.iter().map(|&v| (-(v - top) / sigma).exp()).sum::<f64>() / n;
    (top - sigma * m.ln(), sigma)
}

fn x_index(x: f64) -> Option<usize> {
    let k = ((x / 0.01).log2() * 4.0).round();
    (k >= 0.0 && k < X_POINTS as f64).then_some(k as usize)
}

/// The whitened null's mean at each `x_at(k)` over the band from half
/// the span to the Nyquist frequency, so that a take between two lengths
/// finds both cells measured over all it searches; and the median mix the
/// take's own fit reads.
fn null_mean(
    model: NullModel,
    span: f64,
    step: f64,
    mix: f64,
    draws: usize,
) -> (Vec<Option<f64>>, f64) {
    let mi = (mix.min(1e15) * 1000.0).round() as u64;
    let id = [model as u64, span as u64, (step * 100.0) as u64, mi];
    let g0 = null_grid(NullModel::Readings, span, step, 0.0, 1.0, 1);
    let w = detrend_window(&g0, DETREND_S).map(|w| w as f64 * step);
    let scale = x_scale(&g0, w);
    let mut sum = vec![0.0; X_POINTS];
    let mut cnt = vec![0usize; X_POINTS];
    let mut fitted = Vec::new();
    for d in 0..draws {
        let g = null_grid(
            model,
            span,
            step,
            mix,
            scale,
            seed_of(&[id[0], id[1], id[2], id[3], 1, d as u64]),
        );
        let det = detrend(&g, DETREND_S);
        let sp = spectrum(&det, w);
        let jmin = ((2.0 / span) / sp.df).ceil().max(1.0) as usize;
        for j in jmin..sp.white.len() {
            if let Some(k) = x_index(j as f64 * sp.df * scale) {
                sum[k] += sp.white[j];
                cnt[k] += 1;
            }
        }
        fitted.push(match model {
            NullModel::Readings => sp.rho,
            NullModel::Timing => {
                let from_x = if w.is_some() { 2.0 } else { 6.0 };
                let from = (from_x / (sp.df * scale)).ceil() as usize;
                timing_mix(&sp.power, sp.df, step, from, scale)
            }
        });
    }
    let mean = sum
        .iter()
        .zip(&cnt)
        .map(|(&s, &c)| (c >= 20).then(|| (s / c as f64 * 1e4).round() / 1e4))
        .collect();
    fitted.sort_by(f64::total_cmp);
    (mean, fitted[fitted.len() / 2])
}

fn measure(model: NullModel, span: f64, step: f64, mix: f64, draws: usize) -> Measured {
    let mi = (mix.min(1e15) * 1000.0).round() as u64;
    let id = [model as u64, span as u64, (step * 100.0) as u64, mi];
    let g0 = null_grid(NullModel::Readings, span, step, 0.0, 1.0, 1);
    let w = detrend_window(&g0, DETREND_S).map(|w| w as f64 * step);
    let scale = x_scale(&g0, w);
    let min_p = 3f64.max(8.0 * step);
    let (mean, fitted_mix) = null_mean(model, span, step, mix, draws);
    // The plain trials count's tail; the maxima don't depend on it.
    let lev = Levelling {
        mean: mean.clone(),
        mu: 0.0,
        sigma: 1.0 / std::f64::consts::LN_10,
        xi: 0.0,
        log_trials: 0.0,
    };
    // The maximum score over the searched band, levelled by that mean.
    let mut smax = Vec::new();
    let mut log_trials = 0.0;
    for d in 0..draws {
        let g = null_grid(
            model,
            span,
            step,
            mix,
            scale,
            seed_of(&[id[0], id[1], id[2], id[3], 2, d as u64]),
        );
        let det = detrend(&g, DETREND_S);
        let band = Band {
            min_p,
            max_p: span / 3.0,
            false_alarm: 0.01,
            detrend_w: w,
            model,
            given: Some(&lev),
        };
        let s = search(&det, &band, &[]);
        if !s.null.calibrated {
            let def: Vec<usize> = (0..X_POINTS).filter(|&k| mean[k].is_some()).collect();
            panic!("{model:?} {span} {step} {mix}: band outside the mean; defined {def:?} scale {scale} min_p {min_p}");
        }
        log_trials = s.trials.log10();
        smax.push(s.score.iter().copied().fold(0.0, f64::max));
    }
    let (mu, sigma) = gumbel_fit(&smax);
    let slope = 1.0 / (sigma * std::f64::consts::LN_10);
    Measured {
        cell: Raw {
            model,
            span_s: span,
            step_s: step,
            mix,
            slope: (slope * 1e4).round() / 1e4,
            loc0: ((mu - log_trials / slope) * 1e4).round() / 1e4,
            mean,
        },
        fitted_mix,
        smax,
        mu,
        sigma,
    }
}

/// Writes `data/period-null.json`. About an hour on 4 cores:
/// `cargo test --release -p timegrapher-core --lib write_period_null_table -- --ignored --nocapture`.
#[test]
#[ignore = "writes the table; run by hand"]
fn write_period_null_table() {
    let draws: usize = std::env::var("NULL_DRAWS").map_or(400, |s| s.parse().unwrap());
    let only: Option<String> = std::env::var("NULL_ONLY").ok();
    let cache: Option<String> = std::env::var("NULL_CACHE").ok();
    let remean = std::env::var("NULL_REMEAN").is_ok();
    let mut jobs = Vec::new();
    for model in [NullModel::Timing, NullModel::Readings] {
        for (span, step) in rows(model) {
            let g = null_grid(NullModel::Readings, span, step, 0.0, 1.0, 1);
            let w = detrend_window(&g, DETREND_S).map(|w| w as f64 * step);
            let scale = x_scale(&g, w);
            for mix in mixes(model, scale, step) {
                if only
                    .as_deref()
                    .is_some_and(|o| !format!("{model:?} {span} {step}").starts_with(o))
                {
                    continue;
                }
                jobs.push((model, span, step, mix));
            }
        }
    }
    // Longest first, so the threads finish together.
    jobs.sort_by(|a, b| (b.1 / b.2).total_cmp(&(a.1 / a.2)));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut done: Vec<Measured> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|_| {
                sc.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&(model, span, step, mix)) = jobs.get(i) else { break };
                        // A cell already measured (the run was cut short) is
                        // read back rather than run again.
                        let kept = cache.as_ref().map(|d| format!("{d}/{model:?}_{span}_{step}_{mix}_{draws}.json"));
                        let old = kept.as_ref().and_then(|f| std::fs::read_to_string(f).ok());
                        let m = match old.and_then(|t| serde_json::from_str::<Measured>(&t).ok()) {
                            // The mean measured again over its band, the
                            // maxima kept (they use the band's mean only,
                            // the same draws).
                            Some(mut m) if remean => {
                                m.cell.mean = null_mean(model, span, step, mix, draws).0;
                                if let Some(f) = &kept {
                                    std::fs::write(f, serde_json::to_string(&m).unwrap()).unwrap();
                                }
                                m
                            }
                            Some(m) => m,
                            None => {
                                let m = measure(model, span, step, mix, draws);
                                if let Some(f) = &kept {
                                    std::fs::write(f, serde_json::to_string(&m).unwrap()).unwrap();
                                }
                                m
                            }
                        };
                        eprintln!(
                            "{model:?} span {span} step {step} mix {mix}: fitted {:.3} slope {:.3} loc0 {:.3}",
                            m.fitted_mix, m.cell.slope, m.cell.loc0
                        );
                        out.push(m);
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    done.sort_by(|a, b| {
        (a.cell.model as u8, a.cell.step_s, a.cell.span_s, a.cell.mix)
            .partial_cmp(&(b.cell.model as u8, b.cell.step_s, b.cell.span_s, b.cell.mix))
            .unwrap()
    });
    let cells = fit_shapes(&done);
    // The tail check: with each cell's fit, how often a null maximum reads
    // under 1% and 0.1% (pooled over every cell).
    let (mut n, mut p1, mut p01) = (0usize, 0usize, 0usize);
    for (m, c) in done.iter().zip(&cells) {
        for &s in &m.smax {
            let p = gev_p(s, c.mu, c.sigma, c.xi);
            n += 1;
            p1 += usize::from(p < 0.01);
            p01 += usize::from(p < 0.001);
        }
    }
    let note = format!(
        "Generated by write_period_null_table (longterm/null_tests.rs), fixed seeds. {draws} draws a cell for the mean and as many for the maximum. The maximum's generalised extreme value by maximum likelihood: location and scale for each cell (the scale's relative error about {:.0}%), the shape for each take length and step, profiled over its cells on 0 to 0.2 in steps of 0.02. Pooled tail check: {p1} of {n} null maxima under p 0.01 (expected {:.0}), {p01} under 0.001 (expected {:.0}).",
        78.0 / (draws as f64).sqrt(),
        n as f64 * 0.01,
        n as f64 * 0.001
    );
    eprintln!("{note}");
    let mut text = format!(
        "{{\"note\": {},\n\"draws_mean\": {draws},\n\"draws_max\": {draws},\n\"cells\": [\n",
        serde_json::to_string(&note).unwrap()
    );
    for (i, c) in cells.iter().enumerate() {
        text += &serde_json::to_string(c).unwrap();
        text += if i + 1 < cells.len() { ",\n" } else { "\n" };
    }
    text += "]}\n";
    let path = std::env::var("NULL_OUT").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/data/period-null.json").to_string()
    });
    std::fs::write(path, text).unwrap();
}

/// The exact chance a GEV(μ, σ, ξ) maximum exceeds `s`.
fn gev_p(s: f64, mu: f64, sigma: f64, xi: f64) -> f64 {
    let z = (s - mu) / sigma;
    let h = if xi.abs() < 1e-9 {
        (-z).exp()
    } else {
        let t = 1.0 + xi * z;
        if t <= 0.0 {
            return if xi > 0.0 { 1.0 } else { 0.0 };
        }
        t.powf(-1.0 / xi)
    };
    -(-h).exp_m1()
}

/// The GEV log-likelihood of the maxima `x`.
fn gev_ll(x: &[f64], mu: f64, sigma: f64, xi: f64) -> f64 {
    if sigma <= 0.0 {
        return f64::NEG_INFINITY;
    }
    x.iter()
        .map(|&v| {
            let z = (v - mu) / sigma;
            if xi.abs() < 1e-9 {
                return -sigma.ln() - z - (-z).exp();
            }
            let t = 1.0 + xi * z;
            if t <= 0.0 {
                return f64::NEG_INFINITY;
            }
            -sigma.ln() - (1.0 + 1.0 / xi) * t.ln() - t.powf(-1.0 / xi)
        })
        .sum()
}

/// μ and σ at the shape `xi` by maximum likelihood (a pattern search
/// from the Gumbel fit), and the likelihood.
fn gev_fit(x: &[f64], xi: f64, mu0: f64, sigma0: f64) -> (f64, f64, f64) {
    let (mut mu, mut sigma) = (mu0, sigma0);
    let mut best = gev_ll(x, mu, sigma, xi);
    let mut step = (0.1, 0.05);
    while step.0 > 1e-5 {
        let mut moved = false;
        for (dm, ds) in [(step.0, 0.0), (-step.0, 0.0), (0.0, step.1), (0.0, -step.1)] {
            let l = gev_ll(x, mu + dm, sigma + ds, xi);
            if l > best {
                (best, mu, sigma, moved) = (l, mu + dm, sigma + ds, true);
            }
        }
        if !moved {
            step = (step.0 / 2.0, step.1 / 2.0);
        }
    }
    (mu, sigma, best)
}

/// The table's cells: for each take length and step, the shape that
/// fits all its cells' maxima best, and each cell's location and scale at
/// it.
fn fit_shapes(done: &[Measured]) -> Vec<Cell> {
    let round = |v: f64| (v * 1e4).round() / 1e4;
    let mut out = Vec::new();
    let mut i = 0;
    while i < done.len() {
        let r = &done[i].cell;
        let row: Vec<&Measured> = done[i..]
            .iter()
            .take_while(|m| {
                m.cell.model == r.model && m.cell.span_s == r.span_s && m.cell.step_s == r.step_s
            })
            .collect();
        i += row.len();
        let best = (0..=10)
            .map(|k| {
                let xi = 0.02 * k as f64;
                let fits: Vec<(f64, f64, f64)> = row
                    .iter()
                    .map(|m| gev_fit(&m.smax, xi, m.mu, m.sigma))
                    .collect();
                (fits.iter().map(|f| f.2).sum::<f64>(), xi, fits)
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap();
        let (_, xi, fits) = best;
        eprintln!(
            "{:?} span {} step {}: shape {xi}",
            r.model, r.span_s, r.step_s
        );
        for (m, (mu, sigma, _)) in row.iter().zip(fits) {
            let c = &m.cell;
            let slope = c.slope;
            out.push(Cell {
                model: c.model,
                span_s: c.span_s,
                step_s: c.step_s,
                mix: c.mix,
                mu: round(mu),
                sigma: round(sigma),
                xi: round(xi),
                // The Gumbel's location less log10(trials)/slope gave it.
                log_trials: round((m.mu - c.loc0) * slope),
                mean: c.mean.clone(),
            });
        }
    }
    out
}

/// Outside the table there is no levelling, never a clamped cell: a
/// take shorter than its shortest, longer than its longest, a ρ past its
/// last, a step it has no cells for, a step it has only at other lengths,
/// a span between the two detrend branches' cells, or no fitted mix
/// (#1195, Norm's condition 2).
#[test]
fn levelling_is_none_outside_the_table() {
    use NullModel::{Readings, Timing};
    for (model, span, step, mix) in [
        (Readings, 60.0, 2.0, 0.3),
        (Readings, 1.0e6, 2.0, 0.3),
        (Readings, 1800.0, 2.0, 0.995),
        (Readings, 1800.0, 1.0, 0.3),
        (Readings, 3600.0, 4.0, 0.3),
        (Readings, 1199.5, 2.0, 0.3),
        (Timing, 60.0, 0.25, 10.0),
        (Timing, 1.0e6, 4.0, 10.0),
        (Timing, 1800.0, 0.1, 10.0),
        (Timing, 1800.0, 0.25, f64::NAN),
    ] {
        assert!(
            levelling(model, span, step, mix).is_none(),
            "{model:?} {span} s at {step} s, mix {mix}"
        );
    }
    // Each side's own edge cells are inside.
    assert!(levelling(Readings, 1199.0, 2.0, 0.3).is_some());
    assert!(levelling(Readings, 1200.0, 2.0, 0.3).is_some());
    assert!(levelling(Readings, 1800.0, 2.0, 0.99).is_some());
}

/// A lookup on a cell returns that cell; one between two take lengths or
/// two mixes is the blend of the cells either side, linear in log span
/// and on the mix axis.
#[test]
fn a_lookup_between_cells_interpolates() {
    use NullModel::Readings;
    let cell = |span: f64, mix: f64| {
        table()
            .cells
            .iter()
            .find(|c| c.model == Readings && c.span_s == span && c.step_s == 2.0 && c.mix == mix)
            .unwrap_or_else(|| panic!("no cell {span} {mix}"))
    };
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9 * (1.0 + b.abs());
    let on = levelling(Readings, 1800.0, 2.0, 0.5).unwrap();
    let c = cell(1800.0, 0.5);
    assert_eq!(
        (on.mu, on.sigma, on.xi, on.log_trials),
        (c.mu, c.sigma, c.xi, c.log_trials)
    );
    assert_eq!(on.mean, c.mean);
    // Halfway in log span between 1800 and 2700 s.
    let (a, b) = (cell(1800.0, 0.5), cell(2700.0, 0.5));
    let mid = levelling(Readings, (1800.0f64 * 2700.0).sqrt(), 2.0, 0.5).unwrap();
    assert!(
        close(mid.mu, (a.mu + b.mu) / 2.0),
        "{} {} {}",
        mid.mu,
        a.mu,
        b.mu
    );
    assert!(close(mid.sigma, (a.sigma + b.sigma) / 2.0));
    for k in 0..X_POINTS {
        match (a.mean[k], b.mean[k], mid.mean[k]) {
            (Some(x), Some(y), Some(m)) => assert!(close(m, (x + y) / 2.0)),
            (_, _, m) => assert!(m.is_none()),
        }
    }
    // A quarter of the way from ρ 0.5 to 0.7 at 1800 s.
    let (a, b) = (cell(1800.0, 0.5), cell(1800.0, 0.7));
    let q = levelling(Readings, 1800.0, 2.0, 0.55).unwrap();
    assert!(
        close(q.mu, a.mu + (b.mu - a.mu) * 0.25),
        "{} {} {}",
        q.mu,
        a.mu,
        b.mu
    );
    assert!(close(q.xi, a.xi + (b.xi - a.xi) * 0.25));
    assert!(q.mu != a.mu && q.mu != b.mu);
}

/// Out of sample: fresh seeds (none the table was fitted on) through the
/// whole search, its own mix fitted and looked up, at every cell; the
/// count of maxima past p 0.01 and 0.001 against the expected. Run by
/// hand: `FRESH_DRAWS` draws a cell (default 100).
#[test]
#[ignore = "out-of-sample tail check; run by hand"]
fn fresh_seeds_hold_the_tail() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    let draws: usize = std::env::var("FRESH_DRAWS").map_or(100, |s| s.parse().unwrap());
    let only: Option<String> = std::env::var("FRESH_MODEL").ok();
    let mut cells: Vec<&Cell> = table()
        .cells
        .iter()
        .filter(|c| only.as_ref().is_none_or(|m| format!("{:?}", c.model) == *m))
        .collect();
    cells.sort_by(|a, b| (b.span_s / b.step_s).total_cmp(&(a.span_s / a.step_s)));
    let next = AtomicUsize::new(0);
    // Per model: maxima, uncalibrated, past 0.01, past 0.001.
    let totals = Mutex::new([[0usize; 4]; 2]);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|sc| {
        for _ in 0..workers {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(c) = cells.get(i) else { break };
                let g0 = null_grid(NullModel::Readings, c.span_s, c.step_s, 0.0, 1.0, 1);
                let w = detrend_window(&g0, DETREND_S).map(|w| w as f64 * c.step_s);
                let scale = x_scale(&g0, w);
                let mi = (c.mix.min(1e15) * 1000.0).round() as u64;
                let id = [
                    c.model as u64,
                    c.span_s as u64,
                    (c.step_s * 100.0) as u64,
                    mi,
                ];
                let mut n = [0usize; 4];
                for d in 0..draws {
                    let seed = seed_of(&[id[0], id[1], id[2], id[3], 7, d as u64]);
                    let g = null_grid(c.model, c.span_s, c.step_s, c.mix, scale, seed);
                    let det = detrend(&g, DETREND_S);
                    let band = Band {
                        min_p: 3f64.max(8.0 * c.step_s),
                        max_p: c.span_s / 3.0,
                        false_alarm: 0.01,
                        detrend_w: w,
                        model: c.model,
                        given: None,
                    };
                    let s = search(&det, &band, &[]);
                    n[0] += 1;
                    if !s.null.calibrated {
                        n[1] += 1;
                        continue;
                    }
                    let sig = s
                        .tail
                        .significance(s.score.iter().copied().fold(0.0, f64::max));
                    n[2] += usize::from(sig > 2.0);
                    n[3] += usize::from(sig > 3.0);
                }
                eprintln!(
                    "cell {:?} {} {} {}: {} {} {} {}",
                    c.model, c.span_s, c.step_s, c.mix, n[0], n[1], n[2], n[3]
                );
                let mut t = totals.lock().unwrap();
                for (tk, nk) in t[c.model as usize].iter_mut().zip(n) {
                    *tk += nk;
                }
            });
        }
    });
    let t = totals.into_inner().unwrap();
    for (m, name) in [(0, "readings"), (1, "timing")] {
        let held = (t[m][0] - t[m][1]) as f64;
        eprintln!(
            "{name}: {} maxima, {} uncalibrated; past p 0.01 {} (expected {:.0}), past 0.001 {} (expected {:.1})",
            t[m][0],
            t[m][1],
            t[m][2],
            held * 0.01,
            t[m][3],
            held * 0.001
        );
    }
}

/// The plain trials count's tail is the old Bonferroni: significance
/// score − log10 N, threshold log10 N − log10 p.
#[test]
fn the_trials_tail_is_bonferroni() {
    for n in [1.0, 37.0, 4096.0] {
        let t = Tail::trials(n);
        for s in [0.5, 3.0, 9.25] {
            assert!((t.significance(s) - (s - f64::log10(n))).abs() < 1e-12);
        }
        for p in [0.01, 0.001] {
            assert!((t.score_at(p) - (f64::log10(n) - f64::log10(p))).abs() < 1e-12);
        }
    }
}

/// `score_at` and `significance` invert each other, with no shape and
/// with one.
#[test]
fn score_at_and_significance_invert() {
    for xi in [0.0, 0.08, 0.2] {
        let t = Tail {
            mu: 3.1,
            sigma: 0.52,
            xi,
        };
        for p in [0.05, 0.01, 0.001, 1e-6] {
            let s = t.score_at(p);
            assert!((t.significance(s) + p.log10()).abs() < 1e-9, "ξ {xi} p {p}");
        }
    }
}

/// A cell's levelling moved to its own trials count is its own tail;
/// to ten times as many, its maximum's location rises as ten draws'.
#[test]
fn a_levellings_tail_at_its_own_trials_is_its_own() {
    for xi in [0.0, 0.12] {
        let l = Levelling {
            mean: vec![Some(1.0); X_POINTS],
            mu: 3.4,
            sigma: 0.47,
            xi,
            log_trials: 2.7,
        };
        let t = l.tail(10f64.powf(2.7));
        assert!((t.mu - 3.4).abs() < 1e-12 && (t.sigma - 0.47).abs() < 1e-12 && t.xi == xi);
        // The maximum of ten: P(max ≤ s)^10, so −ln P, the chance the
        // tail states, is ten times as large at every score.
        let ten = l.tail(10f64.powf(3.7));
        let s = t.score_at(0.01);
        assert!(
            (ten.significance(s) - (t.significance(s) - 1.0)).abs() < 1e-9,
            "ξ {xi}"
        );
    }
}

/// A step within 0.1% of the table's is the table's: real takes read a
/// few ppm off the nominal 2 s.
#[test]
fn a_step_a_few_ppm_off_finds_its_cells() {
    let on = levelling(NullModel::Readings, 1800.0, 2.0, 0.5).unwrap();
    let off = levelling(NullModel::Readings, 1800.0, 2.0000378, 0.5).unwrap();
    assert_eq!(on.mu, off.mu);
    assert!(levelling(NullModel::Readings, 1800.0, 2.01, 0.5).is_none());
}

/// The timing's crossover fit on the null itself: about none on white
/// noise, the pure walk's mark on a walk, and close to the crossover a
/// walk plus white noise was made with (the median of 21 draws).
#[test]
fn timing_mix_recovers_the_crossover() {
    let (span, step) = (7200.0, 0.25);
    let g0 = null_grid(NullModel::Readings, span, step, 0.0, 1.0, 1);
    let w = detrend_window(&g0, DETREND_S).map(|w| w as f64 * step);
    let scale = x_scale(&g0, w);
    let fit = |mix: f64| {
        let mut v: Vec<f64> = (0..21)
            .map(|d| {
                let g = null_grid(NullModel::Timing, span, step, mix, scale, seed_of(&[9, d]));
                let sp = spectrum(&detrend(&g, DETREND_S), w);
                let from = (2.0 / (sp.df * scale)).ceil() as usize;
                timing_mix(&sp.power, sp.df, step, from, scale)
            })
            .collect();
        v.sort_by(f64::total_cmp);
        v[10]
    };
    assert!(fit(0.0) < 1.0, "white: {}", fit(0.0));
    assert_eq!(fit(WALK_MIX), WALK_MIX);
    for mix in [30.0, 300.0, 1000.0] {
        let got = fit(mix);
        assert!((got / mix).ln().abs() < 0.25f64.ln_1p(), "{mix}: {got}");
    }
}

/// The table as the generator writes it: every model, span and step row
/// with its full mix list, a shape on 0 to 0.2, a positive scale, and
/// positive means (a truncated or hand-edited table fails here).
#[test]
fn the_null_table_has_the_generators_shape() {
    let t = table();
    for model in [NullModel::Readings, NullModel::Timing] {
        for (span, step) in rows(model) {
            let g0 = null_grid(NullModel::Readings, span, step, 0.0, 1.0, 1);
            let w = detrend_window(&g0, DETREND_S).map(|w| w as f64 * step);
            let want = mixes(model, x_scale(&g0, w), step);
            let mut got: Vec<f64> = t
                .cells
                .iter()
                .filter(|c| c.model == model && c.span_s == span && c.step_s == step)
                .map(|c| c.mix)
                .collect();
            got.sort_by(f64::total_cmp);
            assert_eq!(got, want, "{model:?} {span} s at {step} s");
        }
    }
    for c in &t.cells {
        let at = format!("{:?} {} {} {}", c.model, c.span_s, c.step_s, c.mix);
        assert!((0.0..=0.2).contains(&c.xi), "{at}: ξ {}", c.xi);
        assert!(
            c.sigma > 0.0 && c.mu.is_finite() && c.log_trials > 0.0,
            "{at}"
        );
        assert_eq!(c.mean.len(), X_POINTS, "{at}");
        assert!(c.mean.iter().flatten().all(|&m| m > 0.0), "{at}");
        assert!(c.mean.iter().flatten().count() >= 10, "{at}");
    }
}
