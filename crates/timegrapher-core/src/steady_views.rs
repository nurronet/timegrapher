//! What the Steadiness pane draws behind each headline, as data: the
//! readings the tests were made on, each series' period search, and each
//! series folded at a cycle. The Autocorrelation, Allan Deviation and CUSUM
//! views come with the statistics themselves ([`crate::steadiness`]); these
//! are the Readings, Periods and Cycle views, so a chart outside the app can
//! draw the same thing.
//!
//! The grids are the ones `steadiness::check` builds, from its own
//! functions (on true time when the take has pauses or a clock fit), and
//! [`grids_with_breaks`] builds them the same way for the app's own takes,
//! with no clock fit and only zero-length breaks. The searches and folds are the app's, and
//! only the output is thinned, so a day's take stays small.

use crate::chart_scale;
use crate::longrun::LongReport;
use crate::longterm::{self, Grid, LongConfig, Search};
use crate::steadiness::Report;
use crate::stream::BeatLog;
use crate::walk;
use serde::Serialize;

/// Most readings drawn per series, as on the app's Readings view.
pub const MAX_READINGS: usize = 3000;
/// Most periods kept from a period search.
pub const MAX_PERIODS: usize = 600;
/// Most folded cycles kept per series; the rest are counted.
pub const MAX_CYCLES: usize = 200;

/// The views behind one series' headline.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeriesViews {
    pub beat_error_source: Option<crate::session::BeatErrorSource>,
    pub beat_error_sources: crate::session::BeatErrorSources,
    pub caveats: Vec<crate::measure::Caveat>,
    /// "rate", "amplitude" or "beat_error".
    pub series: &'static str,
    pub readings: Readings,
    pub search: Option<SearchView>,
    pub fold: Option<FoldView>,
}

/// The readings the tests were made on, in time order on the tests' time
/// (the clock's, with each pause's gap): 10 s rate readings, and 2 s
/// windows for the amplitude and the beat error from the unlock.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Readings {
    /// Length of each reading, seconds.
    pub step_s: f64,
    /// Readings in the take, and how long each drawn point's chunk of
    /// them spans, seconds, on average ([`chunk_s`]).
    pub n: usize,
    pub chunk_s: f64,
    /// One point for each chunk of readings ([`thin_readings`]), with a
    /// value: the middle of its readings' times, seconds; their mean; the
    /// lowest and the highest; and how many readings it stands for (1
    /// keeps every reading as it is).
    pub t_s: Vec<f64>,
    pub values: Vec<f64>,
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
    pub counts: Vec<usize>,
    /// Whether any point stands for more than one reading ([`banded`]),
    /// so each is drawn over its band, and the key that says so
    /// ([`readings_key`]).
    pub banded: bool,
    pub key: String,
    /// The value scale they are drawn on ([`readings_range`]), null with
    /// fewer than two.
    pub y_range: Option<[f64; 2]>,
}

/// A chunk of readings as it is drawn: the mean time of its readings with
/// a value, their mean, lowest and highest, and how many there are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drawn {
    pub t: f64,
    pub mean: f64,
    pub lo: f64,
    pub hi: f64,
    pub count: usize,
}

/// Whether [`thin_readings`] drew any chunk of more than one reading, so
/// the series is drawn as bands.
pub fn banded(drawn: &[Drawn]) -> bool {
    drawn.iter().any(|d| d.count > 1)
}

/// What readings `step_s` long are: "10 s readings"; shorter than 10 s,
/// "2 s windows". Steps are seconds long, so whole seconds.
pub fn readings_kind(step_s: f64) -> String {
    let what = if step_s >= 10.0 - 1e-9 {
        "readings"
    } else {
        "windows"
    };
    format!("{step_s:.0} s {what}")
}

/// The Readings view's key: what a dot is and how many are drawn, and on
/// a long series ([`banded`]) how many readings the bands hold: "10 s
/// readings (179)", "10 s readings in 3000 bands (23400)".
pub fn readings_key(step_s: f64, drawn: usize, read: usize, banded: bool) -> String {
    let kind = readings_kind(step_s);
    if banded {
        format!("{kind} in {drawn} bands ({read})")
    } else {
        format!("{kind} ({drawn})")
    }
}

/// The value scale a series' readings are drawn on: core's
/// [`crate::chart_scale::spread`] of every finite reading, banded or not,
/// so a few stray readings stay outside it (#1034). A band reaching
/// past it is clipped at the plot's edge; its `lo` and `hi` stay true.
pub fn readings_range(g: &Grid) -> Option<(f64, f64)> {
    let finite: Vec<f64> = g.y.iter().copied().filter(|v| v.is_finite()).collect();
    chart_scale::spread(&finite)
}

/// How long a chunk of [`thin_readings`] spans, seconds, on average: a
/// reading's length while the series is within [`MAX_READINGS`].
pub fn chunk_s(g: &Grid) -> f64 {
    g.step * (g.y.len() as f64 / MAX_READINGS as f64).max(1.0)
}

/// A series' readings as they are drawn, at most [`MAX_READINGS`]
/// points: the readings cut into near-equal chunks by core's chunk rule
/// (chunk `i` starts at reading ⌊i·n/max⌋, as a fold's raster is
/// thinned, #512), each drawn as the band from its lowest reading to its
/// highest with its mean, so every reading stays in the picture. Picking
/// every k-th reading instead would alias a cycle near a multiple of k
/// readings away (#1034). A chunk with no reading with a value is left
/// out, never filled; a series within the limit is drawn as it is, one
/// reading a point.
pub fn thin_readings(g: &Grid) -> Vec<Drawn> {
    longterm::chunks(g.y.len(), MAX_READINGS)
        .into_iter()
        .filter_map(|[a, b]| {
            let finite: Vec<usize> = (a..b).filter(|&j| g.y[j].is_finite()).collect();
            let count = finite.len();
            let mean = crate::dsp::mean_of(finite.iter().map(|&j| g.y[j]))?;
            let t = crate::dsp::mean_of(finite.iter().map(|&j| g.time(j)))?;
            let (lo, hi) = finite
                .iter()
                .map(|&j| g.y[j])
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| {
                    (l.min(v), h.max(v))
                });
            Some(Drawn {
                t,
                mean,
                lo,
                hi,
                count,
            })
        })
        .collect()
}

/// The period search: how strongly the series repeats at each period.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchView {
    /// Periods searched, seconds, longest first, and the score at each
    /// (−log10 of the chance from noise).
    pub period_s: Vec<f64>,
    pub score: Vec<f64>,
    /// The score a period needs for the search to list it; whether the
    /// readings hold a cycle there is the tests' call, in the headline.
    pub threshold: f64,
}

/// The readings folded at one period: each cycle's readings by phase, and
/// their median shape.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FoldView {
    pub period_s: f64,
    /// "own" (the series' own cycle), "rate", "amplitude" or "beat_error"
    /// (another series' cycle), or "wheel" (a wheel's turn, `wheel`).
    pub from: &'static str,
    pub wheel: Option<String>,
    /// Median of the detrended readings in each phase bin; NaN as null.
    pub profile: Vec<f64>,
    /// Cycles folded, and up to [`MAX_CYCLES`] of them, evenly spread, one
    /// row per cycle and one value per phase bin (null where empty).
    pub cycles: usize,
    pub raster: Vec<Vec<f64>>,
    /// Each phase bin's quartiles (lower, median, upper) over every cycle
    /// folded, not only the rows kept; null for a bin with fewer than four
    /// readings. The app draws the middle half as a box.
    pub columns: Vec<Option<[f64; 3]>>,
    /// The median line's swing (highest less lowest), and the median of
    /// the columns' middle-half heights: null unless both can be given.
    pub swing: Option<f64>,
    pub middle_half: Option<f64>,
    /// The value scale the fold is drawn on ([`fold_range`]): every
    /// cycle's readings, not only those kept; null when none has one.
    pub y_range: Option<[f64; 2]>,
}

/// Each phase bin's quartiles over the fold's cycles (lower, median,
/// upper, interpolated), `None` for a bin with fewer than four readings.
pub fn fold_columns(f: &longterm::Fold) -> Vec<Option<[f64; 3]>> {
    (0..f.profile.len())
        .map(|b| {
            let mut v: Vec<f64> = f
                .raster
                .iter()
                .filter_map(|r| r.get(b).copied())
                .filter(|v| v.is_finite())
                .collect();
            if v.len() < 4 {
                return None;
            }
            v.sort_by(f64::total_cmp);
            let q = |p: f64| crate::dsp::quantile(&v, p);
            Some([q(0.25), q(0.5), q(0.75)])
        })
        .collect()
}

/// The value scale a fold is drawn on: [`chart_scale::spread`] of every
/// cycle's readings, not only the rows kept for drawing; `None` when no
/// cycle has a reading.
pub fn fold_range(f: &longterm::Fold) -> Option<(f64, f64)> {
    let dots: Vec<f64> = f.raster.iter().flatten().copied().collect();
    chart_scale::spread(&dots)
}

/// The median line's swing (highest less lowest) and the median of the
/// columns' middle-half heights, when both can be given.
pub fn fold_spread(f: &longterm::Fold, columns: &[Option<[f64; 3]>]) -> Option<(f64, f64)> {
    let shape: Vec<f64> = f
        .profile
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    let mut mids: Vec<f64> = columns.iter().flatten().map(|[a, _, c]| c - a).collect();
    if shape.len() < 2 || mids.is_empty() {
        return None;
    }
    mids.sort_by(f64::total_cmp);
    let n = mids.len();
    let middle = if n % 2 == 1 {
        mids[n / 2]
    } else {
        (mids[n / 2 - 1] + mids[n / 2]) / 2.0
    };
    let hi = shape.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let lo = shape.iter().copied().fold(f64::INFINITY, f64::min);
    Some((hi - lo, middle))
}

/// A fold as its view gives it: up to [`MAX_CYCLES`] of its cycles kept
/// for drawing, its columns and spread over every cycle, and its scale
/// ([`fold_range`]).
fn fold_view(f: &longterm::Fold, from: &'static str, wheel: Option<String>) -> FoldView {
    let columns = fold_columns(f);
    let spread_of = fold_spread(f, &columns);
    let raster: Vec<Vec<f64>> = spread(f.raster.len(), MAX_CYCLES)
        .into_iter()
        .map(|k| f.raster[k].clone())
        .collect();
    FoldView {
        period_s: f.period_s,
        from,
        wheel,
        profile: f.profile.clone(),
        cycles: f.raster.len(),
        raster,
        columns,
        swing: spread_of.map(|s| s.0),
        middle_half: spread_of.map(|s| s.1),
        y_range: fold_range(f).map(|(a, b)| [a, b]),
    }
}

/// The views for each series, in the report's order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Views {
    pub series: Vec<SeriesViews>,
    /// The periods every series' search covers between them, log10 seconds,
    /// so the Periods plots share one axis.
    pub period_span: Option<[f64; 2]>,
}

const NAMES: [&str; 3] = ["rate", "amplitude", "beat_error"];

/// The readings the tests are made on: `rate_s` rate readings, and the 2 s
/// amplitude windows for the amplitude and the beat error from the unlock.
pub fn grids(log: &BeatLog, rate_s: f64) -> [Grid; 3] {
    grids_with_breaks(log, rate_s, &[])
}

/// The same grids with `breaks` on the log's own clock (no clock fit),
/// built by `steadiness::check`'s own functions: no rate reading or
/// amplitude window reaches into a break's gap, and no rate reading lacks
/// beats in any of its 2 s stretches. A legacy entry point does not infer
/// breaks from the log's jumps; callers pass the ones their analysis used.
pub fn grids_with_breaks(log: &BeatLog, rate_s: f64, breaks: &[crate::breaks::Break]) -> [Grid; 3] {
    use crate::steadiness::{rate_grid, rate_windows, window_grids};
    let rate = rate_grid(&rate_windows(log, None, breaks, rate_s), rate_s);
    let [amplitude, beat_error] = window_grids(log, None, breaks);
    [rate, amplitude, beat_error]
}

/// The period search behind each series: the long-run analysis's for the
/// rate and the amplitude, and one over the beat error's readings when it
/// has at least 30.
pub fn searches(grids: &[Grid; 3], long: &LongReport, lc: &LongConfig) -> [Option<Search>; 3] {
    let readings = |g: &Grid| g.y.iter().filter(|v| v.is_finite()).count();
    let be_search = (readings(&grids[2]) >= 30).then(|| {
        let mut g = grids[2].clone();
        g.t0 = 0.0;
        longterm::analyse(&g, lc).search
    });
    [
        Some(long.timing.search.clone()),
        Some(long.amplitude.search.clone()),
        be_search,
    ]
}

/// The three series' readings as `steadiness::check` lays them out: on
/// true time (the clock's mapping, and each pause's gap when the take has
/// pauses), each gap left empty, and no rate reading that touches a gap or
/// lacks beats in any of its 2 s stretches.
/// Built with the tests' own functions, so the views draw, search and fold
/// the readings the tests were made on.
pub fn check_grids(log: &BeatLog, long: &LongReport, rate_s: f64) -> [Grid; 3] {
    use crate::steadiness::{rate_grid, rate_windows, window_grids};
    let clock = long.clock.as_ref();
    let rate = rate_grid(&rate_windows(log, clock, &long.breaks, rate_s), rate_s);
    let [amplitude, beat_error] = window_grids(log, clock, &long.breaks);
    [rate, amplitude, beat_error]
}

/// Where a series' fold period came from.
#[derive(Debug, Clone, PartialEq)]
pub enum FoldFrom {
    /// Its own cycle.
    Own,
    /// The cycle another series has (0 rate, 1 amplitude, 2 beat error).
    Series(usize),
    /// A wheel's turn, when no series has a cycle.
    Wheel(String),
}

/// Each series folded at the cycle it was found to have, else at another
/// series' cycle or the longest wheel's turn that fits, so its Cycle view
/// still shows whether that repeats in it.
pub fn folds(
    grids: &[Grid; 3],
    report: &Report,
    lc: &LongConfig,
) -> [Option<(longterm::Fold, FoldFrom)>; 3] {
    let own = |i: usize| {
        let s = report.series.get(i)?;
        s.cycle
            .as_ref()
            .map(|c| c.period_s)
            .or(s.period.as_ref().map(|p| p.period_s))
    };
    let fits = |g: &Grid, period: f64| period >= 3.0 * g.step && g.span() >= 2.0 * period;
    std::array::from_fn(|i| {
        let g = &grids[i];
        let (period, from) = own(i)
            .filter(|&p| fits(g, p))
            .map(|p| (p, FoldFrom::Own))
            .or_else(|| {
                (0..3).filter(|&j| j != i).find_map(|j| {
                    own(j)
                        .filter(|&p| fits(g, p))
                        .map(|p| (p, FoldFrom::Series(j)))
                })
            })
            .or_else(|| {
                lc.wheels
                    .iter()
                    .filter(|w| fits(g, w.period_s))
                    .max_by(|a, b| a.period_s.total_cmp(&b.period_s))
                    .map(|w| (w.period_s, FoldFrom::Wheel(w.name.clone())))
            })?;
        Some((longterm::fold(g, period, phase_bins(period, g.step)), from))
    })
}

/// Phase bins for the Cycle view: one per phase the readings fall on, so
/// the drawn cycle keeps its size (at a period of exactly a half whole
/// number of readings, rounding can still split a phase between two
/// neighbouring bins). A 45 s
/// cycle read every 10 s comes back to the same phase every 90 s, after 9
/// readings at 9 phases. A repeat is taken only within two turns: phases
/// that repeat later are too many for each bin to hold enough readings, so
/// the cycle is cut at one bin per reading, 6 to 40.
fn phase_bins(period: f64, step: f64) -> usize {
    let r = period / step;
    (1..=2)
        .map(|m| m as f64 * r)
        .filter(|x| x.round() <= 40.0)
        .find(|x| (x - x.round()).abs() < 0.05)
        .map_or((r.round() as usize).clamp(6, 40), |x| x.round() as usize)
}

/// What a reference line ([`crate::periodicity::reference_lines`]) says on
/// hover, on a period plot with no wheel named: it is a period alone, not a
/// wheel's (#1065).
pub fn reference_hover(label: &str) -> String {
    format!("Reference period, {label}. This calibre's wheels aren't known.")
}

/// The Periods plot's score axis (#1065, Norm): a period's score is
/// −log10 of the chance of a peak that tall at that one period under the
/// search's AR(1) noise model ([`Search::score`]), with no allowance for the
/// many periods tried. Where the model doesn't fit it can be far off, so the
/// axis is the score itself, in plain whole numbers, and this hover says what
/// it is. No title.
pub const SCORE_AXIS_HOVER: &str = "Score: how far a peak stands above the noise expected at its period. If the noise is as expected, a score of 3 means noise alone would reach it there 1 time in 1,000. Many periods are tried, so only peaks above the Threshold are named.";
/// The score axis' visible name, at the top of the axis above its top
/// tick, in the tick labels' size and colour (Hugh, #1069): plain "1, 2,
/// 5" don't say what they count. Its hover is [`SCORE_AXIS_HOVER`].
pub const SCORE_AXIS_NAME: &str = "Score";
/// The name of the line a period's score must pass to be named
/// ([`Search::threshold`]), beside the plot.
pub const THRESHOLD_NAME: &str = "Threshold";
/// That line's hover (Norm, worded by Hugh): the model's search-wide rate,
/// the search's [`LongConfig::false_alarm`] (1 in 100 by default), and the
/// rates measured on seeded noise, so it agrees with `period_may_be_noise`.
pub fn threshold_hover(false_alarm: f64) -> String {
    use longterm::{MEASURED_CROSSING_IN, SHORT_CORRELATED_CROSSING_IN};
    let n = thousands((1.0 / false_alarm).round() as u64);
    format!(
        "Peaks above this line are named. Expected noise would cross it somewhere in this search 1 time in {n}. \
         On test noise it was crossed about 1 time in {MEASURED_CROSSING_IN}, and up to 1 time in {SHORT_CORRELATED_CROSSING_IN} on short takes with correlated readings. \
         The more periods searched, the higher the line."
    )
}

/// A whole number with thousands separators: "1,000".
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The ticks of a score axis from `lo` to `hi`, `px` tall, at least
/// `min_px` apart: whole scores from 1 up, on a [`chart_scale::round_step`]
/// of at least 1, written as [`chart_scale::axis_number`] writes them.
pub fn score_ticks(lo: f64, hi: f64, px: f64, min_px: f64) -> Vec<(f64, String)> {
    if hi.partial_cmp(&lo) != Some(std::cmp::Ordering::Greater) {
        return Vec::new();
    }
    let want = (hi - lo) / (px / min_px).max(1.0);
    let step = chart_scale::round_step(want.max(1.0));
    let first = (lo.max(1.0) / step - 1e-9).ceil() as i64;
    (first..)
        .map(|k| k as f64 * step)
        .take_while(|v| *v <= hi + 1e-9)
        .map(|v| (v, chart_scale::axis_number(v, step)))
        .collect()
}

/// The Isochronism card's name (#741, Hugh's spec): the rate against the
/// amplitude as the take runs, from [`crate::walk`].
pub const WALK_TITLE: &str = "Isochronism";
/// The card's one-line hint.
pub const WALK_HINT: &str = "Rate at each amplitude as the watch runs down.";
/// The fewest points the card is drawn with.
pub const WALK_MIN_POINTS: usize = 3;
/// A point read from fewer readings than this says how many on hover.
pub const WALK_FEW_READINGS: usize = 10;
/// The one spelling of beat error's short name, for the names built on it.
macro_rules! beat_error_short {
    () => {
        "B.E."
    };
}
/// Each reading's short name, where its whole one has no room: the
/// strip's overlay axes and the walk's axis (#1052), and a narrow
/// readings card's title (#1061). The one place they are spelled.
pub const AMPLITUDE_SHORT: &str = "Amp.";
pub const BEAT_ERROR_SHORT: &str = beat_error_short!();
/// Beat error from the unlock and from the drop, on the strip's axes.
pub const BEAT_ERROR_UNLOCK_SHORT: &str = concat!(beat_error_short!(), " U");
pub const BEAT_ERROR_DROP_SHORT: &str = concat!(beat_error_short!(), " D");

/// A reading's short name for its whole one, as the cards and axes name
/// them: "Amplitude" is "Amp." and "Beat Error" "B.E."; none for a name
/// already short.
pub fn short_name(name: &str) -> Option<&'static str> {
    match name {
        "Amplitude" => Some(AMPLITUDE_SHORT),
        "Beat Error" => Some(BEAT_ERROR_SHORT),
        _ => None,
    }
}

/// The axes' short names and units, as the app's strip writes them (#1052).
pub const WALK_RATE_NAME: &str = "Rate";
pub const WALK_RATE_UNIT: &str = "s/d";
pub const WALK_AMP_NAME: &str = AMPLITUDE_SHORT;
pub const WALK_AMP_UNIT: &str = "°";
/// What the colour key's hours count from. Core can't yet tell a take
/// begun at full wind from one whose `wind_h` was left at its default, so
/// the hours are from the take's start (Hugh).
pub const WALK_SINCE: &str = "since start";
/// The colour key's left end: the take's start.
pub const WALK_KEY_START: &str = "0 h";
/// The name by the ring at the walk's first point (Hugh's look C).
pub const WALK_START: &str = "Start";

/// One point of the Isochronism card: hours since the take's start, its
/// amplitude and rate, and its hover.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WalkDot {
    pub hours: f64,
    /// Where it falls between the take's start and the last point, 0 to 1:
    /// where its colour is taken on the viridis map.
    pub when: f64,
    pub amplitude_deg: f64,
    pub rate_s_per_day: f64,
    pub hover: String,
    /// Whether the line joins it to the point before: only when that one
    /// is within [`WALK_JOIN_BINS`] merged bins, so a left-out stretch or
    /// a break is a gap in the line, never filled (Norm, #1083).
    pub joined: bool,
}

/// How far apart, in merged bins, two points may be and still be joined.
pub const WALK_JOIN_BINS: f64 = 1.5;

/// The Isochronism card as data: the walk merged to its default width
/// ([`walk::merged`], [`walk::default_merge`]), so every surface draws the
/// same points, and the colour key's end.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WalkView {
    pub dots: Vec<WalkDot>,
    /// The key's hours at its right end, the last point's; its left end is
    /// the take's start, 0 h.
    pub last_hours: f64,
    pub last_text: String,
    /// The hour beads along the trail ([`walk_beads`]).
    pub beads: Vec<WalkBead>,
}

/// A bead on the walk's trail marking a round hour: the dot nearest that
/// hour and the hour as the card writes it ("12 h").
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WalkBead {
    pub dot: usize,
    pub text: String,
}

/// The hour beads: one every 12 h (every 24 h on a take over 96 h), on
/// the dot nearest each, none within 3 h of the last point so a bead
/// never crowds the end's label (Hugh's look C, Peter 11-05). An hour
/// with no dot within an hour of it (in a break) gets none, so a bead
/// never sits on a dot of another hour.
pub fn walk_beads(dots: &[WalkDot], last_hours: f64) -> Vec<WalkBead> {
    let every = if last_hours > 96.0 { 24.0 } else { 12.0 };
    (1..)
        .map(|k| k as f64 * every)
        .take_while(|h| *h < last_hours - 3.0)
        .filter_map(|h| {
            let (dot, d) = dots
                .iter()
                .enumerate()
                .map(|(i, d)| (i, (d.hours - h).abs()))
                .min_by(|a, b| a.1.total_cmp(&b.1))?;
            (d <= 1.0).then(|| WalkBead {
                dot,
                text: hours_text(h),
            })
        })
        .collect()
}

/// A headline as short lines, one fact a line (Hugh's F4): each of its
/// sentences and clauses on a line of its own, from a capital, without
/// its full stop. What the Isochronism pane and card show over the walk,
/// from the curve's headline.
pub fn headline_lines(headline: &str) -> Vec<String> {
    headline
        .split(". ")
        .flat_map(|s| s.split("; "))
        .map(|s| s.trim().trim_end_matches('.'))
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        })
        .collect()
}

/// About how many steps each of the walk's axes is cut into, on a 1-2-5
/// step with round ends (Hugh's F3: 6 to 10 a side).
pub const WALK_AXIS_STEPS: f64 = 10.0;

/// Amplitudes from `lo` to `hi`, padded by 5% and at least `min`: the
/// isochronism curve's span (`min` its bin width) and the walk's
/// ([`crate::steadiness::ISO_BIN_DEG`]).
pub fn amp_padded(lo: f64, hi: f64, min: f64) -> (f64, f64) {
    let pad = ((hi - lo) * 0.05).max(min);
    (lo - pad, hi + pad)
}

/// Rates from `lo` to `hi`, padded by 15% as the charts are.
pub fn rate_padded(lo: f64, hi: f64) -> (f64, f64) {
    let pad = ((hi - lo) * 0.15).max(0.05);
    (lo - pad, hi + pad)
}

/// The isochronism curve's amplitude span from its bins' amplitudes, in
/// order: the first's to the last's, padded by the bin width
/// ([`amp_padded`]); none with no bins.
pub fn curve_amp(bin_amps: &[f64], bin_deg: f64) -> Option<(f64, f64)> {
    Some(amp_padded(*bin_amps.first()?, *bin_amps.last()?, bin_deg))
}

/// The walk's amplitude and rate spans: its dots' padded
/// ([`amp_padded`], [`rate_padded`]).
pub fn walk_spans(dots: &[WalkDot]) -> [(f64, f64); 2] {
    let ends = |f: fn(&WalkDot) -> f64| {
        dots.iter()
            .map(f)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| {
                (l.min(v), h.max(v))
            })
    };
    let (a_lo, a_hi) = ends(|d| d.amplitude_deg);
    let (r_lo, r_hi) = ends(|d| d.rate_s_per_day);
    [
        amp_padded(a_lo, a_hi, crate::steadiness::ISO_BIN_DEG),
        rate_padded(r_lo, r_hi),
    ]
}

/// One axis's `(lo, hi, step)` over `span`: round ends on a 1-2-5 step
/// cut into about [`WALK_AXIS_STEPS`] ([`chart_scale::round_ends_in`]).
pub fn walk_axis((lo, hi): (f64, f64)) -> (f64, f64, f64) {
    chart_scale::round_ends_in(lo, hi, WALK_AXIS_STEPS)
}

/// The walk's amplitude span ([`walk_spans`]) widened to the curve's
/// ([`curve_amp`]) when there is one, so a degree sits at one x in the
/// curve and the walk.
pub fn walk_amp(dots: &[WalkDot], curve_amp: Option<(f64, f64)>) -> (f64, f64) {
    let (a_lo, a_hi) = walk_spans(dots)[0];
    curve_amp.map_or((a_lo, a_hi), |(lo, hi)| (lo.min(a_lo), hi.max(a_hi)))
}

/// The walk's axes, amplitude ([`walk_amp`]) then rate ([`walk_spans`]),
/// each `(lo, hi, step)` ([`walk_axis`]).
pub fn walk_axes(dots: &[WalkDot], curve_amp: Option<(f64, f64)>) -> [(f64, f64, f64); 2] {
    [
        walk_axis(walk_amp(dots, curve_amp)),
        walk_axis(walk_spans(dots)[1]),
    ]
}

/// Hours as the card writes them: one decimal under 10 h, whole above.
pub fn hours_text(h: f64) -> String {
    let digits = if h < 9.95 { 1 } else { 0 };
    format!("{} h", crate::figure_text::fixed_text(h, digits))
}

/// The card's points, or `None` with fewer than [`WALK_MIN_POINTS`].
pub fn walk_view(w: &walk::IsochronismWalk) -> Option<WalkView> {
    use crate::figure_text::{base_digits, fixed_text, signed_text};
    let m = walk::merged(w, walk::default_merge(w));
    let mut dots: Vec<WalkDot> = m
        .points
        .iter()
        .map(|p| {
            let hours = p.t_s / 3600.0;
            let mut hover = format!(
                "{} after start · {}{WALK_AMP_UNIT} · {} {WALK_RATE_UNIT}",
                hours_text(hours),
                fixed_text(p.amplitude_deg, base_digits::AMPLITUDE),
                signed_text(p.rate_s_per_day, base_digits::RATE),
            );
            if p.readings < WALK_FEW_READINGS {
                hover += &format!(" · {} readings", p.readings);
            }
            WalkDot {
                joined: false,
                hours,
                when: 0.0,
                amplitude_deg: p.amplitude_deg,
                rate_s_per_day: p.rate_s_per_day,
                hover,
            }
        })
        .collect();
    if dots.len() < WALK_MIN_POINTS {
        return None;
    }
    for (d, pair) in dots.iter_mut().skip(1).zip(m.points.windows(2)) {
        d.joined = pair[1].t_s - pair[0].t_s <= WALK_JOIN_BINS * m.bin_s;
    }
    let last_hours = dots.last().map_or(0.0, |d| d.hours);
    if last_hours > 0.0 {
        for d in &mut dots {
            d.when = (d.hours / last_hours).clamp(0.0, 1.0);
        }
    }
    Some(WalkView {
        beads: walk_beads(&dots, last_hours),
        last_text: hours_text(last_hours),
        last_hours,
        dots,
    })
}

/// The periods the searches cover between them, log10 seconds.
pub fn period_span(searches: &[Option<Search>; 3]) -> Option<(f64, f64)> {
    searches
        .iter()
        .flatten()
        .flat_map(|x| x.period_s.iter())
        .filter(|p| **p > 0.0)
        .map(|p| p.log10())
        .fold(None, |r: Option<(f64, f64)>, x| {
            Some(r.map_or((x, x), |(a, b)| (a.min(x), b.max(x))))
        })
        .filter(|(a, b)| a < b)
}

/// Indices of a period search's curve thinned to at most `max`, first
/// and last kept: the axis is cut into equal slices of log period, as the
/// FFT's thinning cuts it, and each slice keeps its lowest and highest
/// score, in order. A narrow peak keeps its top unless a taller one shares
/// its slice (on a 12 h search each slice spans about 3% of period).
fn spread_extremes(period_s: &[f64], score: &[f64], max: usize) -> Vec<usize> {
    let n = period_s.len().min(score.len());
    if n <= max || max < 4 {
        return spread(n, max);
    }
    let slices = (max - 2) / 2;
    let (a, b) = (period_s[0].ln(), period_s[n - 1].ln());
    let mut lows: Vec<Option<usize>> = vec![None; slices];
    let mut highs: Vec<Option<usize>> = vec![None; slices];
    for i in 1..n - 1 {
        let f = if a == b {
            0.0
        } else {
            (period_s[i].ln() - a) / (b - a)
        };
        if !f.is_finite() {
            continue;
        }
        let k = ((f * slices as f64) as usize).min(slices - 1);
        if lows[k].is_none_or(|l| score[i] < score[l]) {
            lows[k] = Some(i);
        }
        if highs[k].is_none_or(|h| score[i] > score[h]) {
            highs[k] = Some(i);
        }
    }
    let mut keep: Vec<usize> = std::iter::once(0)
        .chain(lows.into_iter().flatten())
        .chain(highs.into_iter().flatten())
        .chain(std::iter::once(n - 1))
        .collect();
    keep.sort_unstable();
    keep.dedup();
    keep
}

/// Indices of `n` items thinned evenly to at most `max`, first and last kept.
fn spread(n: usize, max: usize) -> Vec<usize> {
    if n <= max {
        return (0..n).collect();
    }
    let last = (n - 1) as f64;
    (0..max)
        .map(|k| (k as f64 * last / (max - 1) as f64).round() as usize)
        .collect()
}

/// The views behind each headline in `report`, from the take `log` it was
/// made on and its long-run analysis `long` under `lc`.
pub fn views(
    log: &BeatLog,
    report: &Report,
    long: &LongReport,
    lc: &LongConfig,
    rate_s: f64,
) -> Views {
    let grids = check_grids(log, long, rate_s);
    // The beat error's search on the grid the tests searched, pauses and
    // all, so its strongest line is the one the label names.
    let readings = grids[2].y.iter().filter(|v| v.is_finite()).count();
    let searches = [
        Some(long.timing.search.clone()),
        Some(long.amplitude.search.clone()),
        (readings >= 30).then(|| {
            let mut g = grids[2].clone();
            g.t0 = 0.0;
            longterm::analyse(&g, lc).search
        }),
    ];
    let folds = folds(&grids, report, lc);
    let series = (0..3)
        .map(|i| {
            let search = searches[i]
                .as_ref()
                .filter(|s| !s.period_s.is_empty())
                .map(|s| {
                    let keep = spread_extremes(&s.period_s, &s.score, MAX_PERIODS);
                    SearchView {
                        period_s: keep.iter().map(|&k| s.period_s[k]).collect(),
                        score: keep.iter().map(|&k| s.score[k]).collect(),
                        threshold: s.threshold,
                    }
                });
            let fold = folds[i].as_ref().map(|(f, from)| {
                let (from, wheel) = match from {
                    FoldFrom::Own => ("own", None),
                    FoldFrom::Series(j) => (NAMES[*j], None),
                    FoldFrom::Wheel(w) => ("wheel", Some(w.clone())),
                };
                fold_view(f, from, wheel)
            });
            SeriesViews {
                beat_error_source: report.series[i].beat_error_source,
                beat_error_sources: report.series[i].beat_error_sources,
                caveats: report.series[i].caveats.clone(),
                series: NAMES[i],
                readings: readings_of(&grids[i]),
                search,
                fold,
            }
        })
        .collect();
    Views {
        series,
        period_span: period_span(&searches).map(|(a, b)| [a, b]),
    }
}

/// A series' readings as the views give them ([`thin_readings`]), on
/// their scale ([`readings_range`]).
fn readings_of(g: &Grid) -> Readings {
    let drawn = thin_readings(g);
    let pick = |f: fn(&Drawn) -> f64| drawn.iter().map(f).collect::<Vec<f64>>();
    let read = drawn.iter().map(|d| d.count).sum();
    Readings {
        banded: banded(&drawn),
        key: readings_key(g.step, drawn.len(), read, banded(&drawn)),
        step_s: g.step,
        n: g.y.len(),
        chunk_s: chunk_s(g),
        y_range: readings_range(g).map(|(a, b)| [a, b]),
        t_s: pick(|d| d.t),
        values: pick(|d| d.mean),
        counts: drawn.iter().map(|d| d.count).collect(),
        lo: pick(|d| d.lo),
        hi: pick(|d| d.hi),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_score_axis_is_marked_at_whole_scores() {
        // The MCP view's scoreTicks reads the same cases (score_ticks.test.cjs).
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/fixtures/score_ticks.json")).unwrap();
        assert!(!cases.is_empty());
        for c in cases {
            let f = |k: &str| c[k].as_f64().unwrap();
            let want: Vec<(f64, String)> = serde_json::from_value(c["ticks"].clone()).unwrap();
            assert_eq!(
                super::score_ticks(f("lo"), f("hi"), f("px"), f("min_px")),
                want,
                "{c}"
            );
        }
        assert_eq!(
            super::threshold_hover(super::LongConfig::default().false_alarm),
            "Peaks above this line are named. Expected noise would cross it somewhere in this search 1 time in 100. \
             On test noise it was crossed about 1 time in 100, and up to 1 time in 27 on short takes with correlated readings. \
             The more periods searched, the higher the line."
        );
        assert!(super::threshold_hover(0.001).contains("search 1 time in 1,000."));
    }

    #[test]
    fn a_reference_line_says_it_is_a_period_alone() {
        assert_eq!(
            super::reference_hover("1 min"),
            "Reference period, 1 min. This calibre's wheels aren't known."
        );
    }

    #[test]
    fn thinning_long_readings_bands_every_reading_by_chunk() {
        let mut g = super::Grid {
            t0: 7.0,
            step: 2.0,
            y: (0..super::MAX_READINGS * 2 + 1).map(|i| i as f64).collect(),
        };
        g.y[3] = f64::NAN;
        g.y[6] = f64::INFINITY;
        g.y[7] = f64::NEG_INFINITY;
        // A chunk of nothing but missing readings is left out: chunk 3
        // (readings 6 and 7) and chunk 1500 (readings 3000 and 3001).
        for j in 3000..3002 {
            g.y[j] = f64::NAN;
        }
        let drawn = super::thin_readings(&g);
        let n = g.y.len();
        let finite = g.y.iter().filter(|v| v.is_finite()).count();
        assert_eq!(drawn.iter().map(|d| d.count).sum::<usize>(), finite);
        assert_eq!(drawn.len(), super::MAX_READINGS - 2);
        // Chunk i holds readings ⌊i·n/max⌋ to ⌊(i+1)·n/max⌋, as core's
        // chunk rule cuts them.
        let expected: Vec<super::Drawn> = (0..super::MAX_READINGS)
            .filter_map(|i| {
                let (a, b) = (
                    i * n / super::MAX_READINGS,
                    (i + 1) * n / super::MAX_READINGS,
                );
                let js: Vec<usize> = (a..b).filter(|&j| g.y[j].is_finite()).collect();
                let k = js.len() as f64;
                (!js.is_empty()).then(|| super::Drawn {
                    t: js.iter().map(|&j| g.time(j)).sum::<f64>() / k,
                    mean: js.iter().map(|&j| g.y[j]).sum::<f64>() / k,
                    lo: g.y[js[0]],
                    hi: g.y[*js.last().unwrap()],
                    count: js.len(),
                })
            })
            .collect();
        assert_eq!(drawn.len(), expected.len());
        for (d, e) in drawn.iter().zip(&expected) {
            assert!((d.t - e.t).abs() < 1e-9 && (d.mean - e.mean).abs() < 1e-9);
            assert_eq!((d.lo, d.hi, d.count), (e.lo, e.hi, e.count));
        }
        assert_eq!(drawn.last().map(|d| d.hi), Some(6000.0));
    }

    /// The cases the MCP view's own key was tested on, now core's for the
    /// app and the view alike.
    #[test]
    fn the_readings_key_says_what_a_dot_is_and_how_many_are_drawn() {
        assert_eq!(readings_key(10.0, 179, 179, false), "10 s readings (179)");
        assert_eq!(readings_key(2.0, 912, 912, false), "2 s windows (912)");
        assert_eq!(readings_key(20.0, 90, 90, false), "20 s readings (90)");
        assert_eq!(readings_key(5.0, 40, 40, false), "5 s windows (40)");
        assert_eq!(
            readings_key(2.0, 3000, 9000, true),
            "2 s windows in 3000 bands (9000)"
        );
        assert_eq!(readings_kind(10.0), "10 s readings");
    }

    /// The JSON carries whether the readings are banded and the key, with
    /// the count the bands hold (a missing reading is in no band).
    #[test]
    fn the_readings_json_carries_banded_and_its_key() {
        let mut y = vec![1.0; 6001];
        y[17] = f64::NAN;
        let g = Grid {
            t0: 0.0,
            step: 2.0,
            y,
        };
        let r = readings_of(&g);
        assert!(r.banded);
        let read: usize = r.counts.iter().sum();
        assert_eq!(read, 6000);
        assert_eq!(
            r.key,
            format!("2 s windows in {} bands (6000)", r.t_s.len())
        );
        let short = readings_of(&Grid {
            t0: 0.0,
            step: 10.0,
            y: vec![1.0, 2.0, f64::NAN],
        });
        assert!(!short.banded);
        assert_eq!(short.key, "10 s readings (2)");
    }

    #[test]
    fn thinning_short_readings_keeps_all_finite_points() {
        let g = super::Grid {
            t0: 7.0,
            step: 2.0,
            y: vec![1.0, f64::NAN, 3.0, f64::NEG_INFINITY, 5.0],
        };
        let one = |j: usize| super::Drawn {
            t: g.time(j),
            mean: g.y[j],
            lo: g.y[j],
            hi: g.y[j],
            count: 1,
        };
        assert_eq!(super::thin_readings(&g), vec![one(0), one(2), one(4)]);
        let empty = super::Grid { y: vec![], ..g };
        assert_eq!(super::thin_readings(&empty), vec![]);
    }

    /// Walter's Pepsi case (#1037): 7 h of 2 s beat error windows between
    /// 0.2 and 0.6 ms, with a stray at ±2 ms in one chunk in twenty. Read
    /// from the readings, the scale stays on the 0.2-0.6 ms trace (from
    /// the band ends it ran −2 to +2 ms), and the bands still carry the
    /// strays, clipped where they are drawn and true in the JSON.
    #[test]
    fn stray_readings_in_a_few_chunks_leave_the_scale_alone() {
        let n = 7 * 3600 / 2;
        let mut y: Vec<f64> = (0..n)
            .map(|i| 0.4 + 0.2 * ((i as f64) * 0.37).sin())
            .collect();
        let per = n as f64 / super::MAX_READINGS as f64;
        let mut strays = 0;
        for c in (0..super::MAX_READINGS).step_by(20) {
            let i = (c as f64 * per).ceil() as usize;
            y[i] = if strays % 2 == 0 { 2.0 } else { -2.0 };
            strays += 1;
        }
        let g = super::Grid {
            t0: 0.0,
            step: 2.0,
            y,
        };
        let drawn = super::thin_readings(&g);
        assert!(super::banded(&drawn));
        let banded = drawn.iter().filter(|d| d.hi >= 2.0 || d.lo <= -2.0).count();
        assert_eq!(banded, strays);
        assert!(banded as f64 >= 0.05 * drawn.len() as f64);
        let (lo, hi) = super::readings_range(&g).unwrap();
        assert!(lo > 0.0 && lo < 0.2 && hi > 0.6 && hi < 0.8, "{lo} to {hi}");
        // The JSON keeps every band's true ends, strays and all, and its
        // scale is the same.
        let r = super::readings_of(&g);
        assert_eq!(r.y_range, Some([lo, hi]));
        let ends = |v: &[f64], x: f64| v.iter().filter(|&&e| e == x).count();
        assert_eq!(ends(&r.hi, 2.0) + ends(&r.lo, -2.0), strays);
    }

    /// A 60 s cycle in a 65 h take's 10 s readings: picking every k-th
    /// reading can land on one phase of it each time and draw a flat
    /// line; the chunks' bands keep its swing in every point, while their
    /// means stay on the level (#1034).
    #[test]
    fn a_long_takes_minute_cycle_shows_in_the_drawn_bands() {
        let n = 65 * 360;
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut noise = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        let g = super::Grid {
            t0: 0.0,
            step: 10.0,
            y: (0..n)
                .map(|i| {
                    let t = i as f64 * 10.0 + 5.0;
                    3.0 * (std::f64::consts::TAU * t / 60.0 + 0.4).sin() + 0.2 * noise()
                })
                .collect(),
        };
        // Picking every 6th reading, a minute apart, lands on one phase of
        // the cycle each time: its swing is gone.
        let picked: Vec<f64> = g.y.iter().step_by(6).copied().collect();
        let range = |v: &[f64]| {
            v.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))
                - v.iter().fold(f64::INFINITY, |a, &b| a.min(b))
        };
        assert!(range(&picked) < 1.0, "{}", range(&picked));
        let drawn = super::thin_readings(&g);
        assert!(drawn.len() <= super::MAX_READINGS);
        assert_eq!(drawn.iter().map(|d| d.count).sum::<usize>(), n);
        let mut widths: Vec<f64> = drawn.iter().map(|d| d.hi - d.lo).collect();
        widths.sort_by(f64::total_cmp);
        // Six readings make a turn: every chunk of seven or more spans
        // most of the 6 s/d swing.
        assert!(widths[0] > 4.0, "{}", widths[0]);
        assert!(drawn.iter().all(|d| d.mean.abs() < 1.0));
    }

    use super::*;
    use crate::live::{LiveAnalyzer, LiveConfig};
    use crate::longrun;
    use crate::periodicity::default_wheels;
    use crate::steadiness::{self, Config};
    use crate::synth::{generate, SynthConfig};

    #[test]
    fn derived_rate_grid_cuts_jumps_as_the_tests_do() {
        let mut log = BeatLog {
            sample_rate: 48000,
            duration_s: 40.0,
            bph: 28800,
            lift_deg: 52.0,
            beats: (0..320)
                .map(|index| crate::beats::Beat {
                    time: index as f64 * 0.125,
                    index,
                    quality: 1.0,
                })
                .collect(),
            amplitude_windows: vec![],
            clipped_beats: vec![],
            clipped_samples: 0,
            jumps: vec![15.0, 30.0],
            sided: Vec::new(),
            by_count: Vec::new(),
            snippets: Vec::new(),
        };
        let legacy = grids(&log, 10.0)[0].clone();
        let breaks: Vec<_> = log
            .jumps
            .iter()
            .map(|&audio_s| crate::breaks::Break {
                audio_s,
                gap_s: 0.0,
            })
            .collect();
        let baseline = grids_with_breaks(&log, 10.0, &breaks)[0].clone();
        let holes = crate::clean::Cuts::new(None, &breaks, &[]);
        let expected = holes.windows(&log.beats, log.bph, 0.0, log.duration_s, 10.0, 10.0, 2.0);
        // 0 to 10 s, 20 to 30 s and 30 to 40 s: the reading across the
        // jump at 15 s is left out, the one ending at the jump at 30 s is
        // whole, and so is the one ending at the end of the take, as the
        // tests' grids have them.
        assert_eq!(expected.len(), 3);
        assert_eq!(baseline.y.len(), 4, "time grid is not compacted");
        assert_eq!(baseline.t0, expected[0].start_s);
        assert_eq!(baseline.y[0], expected[0].fit.rate_s_per_day);
        assert!(baseline.y[1].is_nan());
        assert_eq!(baseline.y[2], expected[1].fit.rate_s_per_day);
        assert_eq!(baseline.y[3], expected[2].fit.rate_s_per_day);
        log.jumps.clear();
        let uncut = grids(&log, 10.0);
        assert_eq!(uncut[0].y.len(), 4);
        assert_eq!(uncut[0].t0, legacy.t0);
        assert_eq!(uncut[0].y, legacy.y, "legacy grids still ignores log jumps");
        assert_eq!(uncut[0].y[0], baseline.y[0], "unaffected reading unchanged");
        log.jumps = vec![15.0];
        let middle = grids_with_breaks(&log, 10.0, &breaks[..1]);
        assert_eq!(middle[0].t0, uncut[0].t0);
        assert_eq!(middle[0].y.len(), 4, "time grid is not compacted");
        assert_eq!(middle[0].y[0], uncut[0].y[0]);
        assert!(middle[0].y[1].is_nan());
        assert_eq!(middle[0].y[2], uncut[0].y[2]);
        assert_eq!(middle[0].y[3], uncut[0].y[3]);
    }

    #[test]
    fn the_cycle_is_folded_one_bin_per_reading_phase() {
        assert_eq!(phase_bins(45.0, 10.0), 9);
        assert_eq!(phase_bins(60.0, 10.0), 6);
        assert_eq!(phase_bins(30.0, 10.0), 3);
        assert_eq!(phase_bins(600.0, 2.0), 40);
        assert_eq!(phase_bins(42.7, 10.0), 6);
        assert_eq!(phase_bins(41.4, 10.0), 6);
        assert_eq!(phase_bins(58.0, 10.0), 6);
        // A 45 s cycle read every 10 s keeps its size: its readings' peak
        // to peak, at 9 phases 40 degrees apart. Each phase falls on a bin
        // edge, and its readings all go to one bin.
        let g = Grid {
            t0: 0.0,
            step: 10.0,
            y: (0..360)
                .map(|i| (std::f64::consts::TAU * (i as f64 * 10.0 + 2.5) / 45.0).sin())
                .collect(),
        };
        let f = longterm::fold(&g, 45.0, phase_bins(45.0, 10.0));
        assert_eq!(f.counts, vec![40; 9]);
        let max = f.profile.iter().copied().fold(f64::MIN, f64::max);
        let min = f.profile.iter().copied().fold(f64::MAX, f64::min);
        let want = 2.0 * (80f64.to_radians()).sin();
        assert!(((max - min) / want - 1.0).abs() < 0.01, "{}", max - min);
    }

    #[test]
    fn thinning_keeps_a_narrow_peak_anywhere_on_the_log_axis() {
        // A 12 h search's periods, longest first, with narrow lines at
        // about 2 h and 1 h, where an even split of the indices puts both
        // in one wide stretch.
        let n = 87_363;
        let period_s: Vec<f64> = (0..n).map(|i| 262_144.0 / (19 + i) as f64).collect();
        let mut score = vec![0.5; n];
        score[17] = 9.0;
        score[54] = 6.0;
        // And one among the short periods, where a slice holds thousands
        // of points.
        score[80_000] = 7.0;
        let keep = spread_extremes(&period_s, &score, MAX_PERIODS);
        assert!(keep.len() <= MAX_PERIODS, "{}", keep.len());
        assert!(keep.windows(2).all(|w| w[0] < w[1]));
        assert_eq!((keep[0], *keep.last().unwrap()), (0, n - 1));
        assert!(
            keep.contains(&17) && keep.contains(&54) && keep.contains(&80_000),
            "{keep:?}"
        );
        // A short curve is kept whole.
        assert_eq!(
            spread_extremes(&period_s[..10], &score[..10], MAX_PERIODS),
            (0..10).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_beat_errors_search_runs_on_the_tests_grid_with_its_pauses() {
        let cfg = SynthConfig {
            duration_s: 120.0,
            beat_error_ms: 0.8,
            beat_error_drift_ms_per_s: 0.01,
            ..Default::default()
        };
        let audio = generate(&cfg, |_| 270.0, |_| 0.0);
        let mut live = LiveAnalyzer::new(48000, LiveConfig::default());
        for b in audio.samples.chunks(4800) {
            live.push(b);
        }
        let log = BeatLog {
            sample_rate: live.sample_rate(),
            duration_s: live.duration_s(),
            bph: live.bph().expect("beat rate"),
            lift_deg: live.config().analysis.amplitude.lift_deg,
            beats: live.beats().to_vec(),
            amplitude_windows: live.amplitude_windows().to_vec(),
            clipped_beats: live.clipped_beats().to_vec(),
            clipped_samples: 0,
            jumps: Vec::new(),
            sided: Vec::new(),
            by_count: Vec::new(),
            snippets: Vec::new(),
        };
        let lc = LongConfig {
            wheels: default_wheels(log.bph),
            ..Default::default()
        };
        let aw = &log.amplitude_windows;
        let step = aw[0].end_s - aw[0].start_s;
        // A 6 s pause half way through, inside one window.
        let mid = aw[aw.len() / 2].start_s - 0.5 * step;
        let breaks = [crate::breaks::Break {
            audio_s: mid,
            gap_s: 3.0 * step,
        }];
        let plain = longrun::analyse(&log, None, &lc);
        let paused = longrun::analyse_with_breaks(&log, None, &breaks, &lc);
        let g0 = check_grids(&log, &plain, 10.0)[2].clone();
        let g1 = check_grids(&log, &paused, 10.0)[2].clone();
        assert_eq!(g1.y.len(), g0.y.len() + 3, "the pause is left empty");
        let k = aw.iter().position(|w| w.start_s > mid).unwrap();
        assert!(g1.y[k..k + 3].iter().all(|v| v.is_nan()));
        let same = |a: f64, b: f64| a == b || (a.is_nan() && b.is_nan());
        assert!(same(g1.y[k + 3], g0.y[k]));
        // The window the pause falls in holds sound from both sides, so
        // it is left out too; the one before it is kept.
        assert!(g1.y[k - 1].is_nan() && g0.y[k - 1].is_finite());
        assert!(same(g1.y[k - 2], g0.y[k - 2]));

        // The tests' own beat-error spectrum comes from this grid, and not
        // from the readings' grid without the pause.
        let check = Config::default();
        let report = steadiness::check(&log, None, &paused, &lc, &check);
        let tests = report
            .spectra
            .iter()
            .find(|s| s.series == "beat_error")
            .expect("beat error spectrum");
        let tests = serde_json::to_value(tests).unwrap();
        let spectrum_of = |mut g: Grid| {
            g.t0 = 0.0;
            let d = longterm::analyse(&g, &lc).detrended;
            let s =
                crate::spectrum::amplitude_spectrum(&d, "beat_error", "ms", 1.0, check.max_points);
            serde_json::to_value(s.expect("spectrum")).unwrap()
        };
        assert_eq!(spectrum_of(g1.clone()), tests);
        assert_ne!(spectrum_of(grids(&log, 10.0)[2].clone()), tests);

        // The views draw and fold these readings, on the tests' time: the
        // readings after the pause sit its gap later than the audio puts
        // them, as the tests' grid has them.
        let v = views(&log, &report, &paused, &lc, 10.0);
        let be = &v.series[2].readings;
        let want: Vec<f64> = (0..g1.y.len())
            .filter(|&j| g1.y[j].is_finite())
            .map(|j| g1.time(j))
            .collect();
        assert_eq!(be.t_s, want);
        let at = |t: f64| be.t_s.iter().any(|&x| (x - t).abs() < 1e-9);
        assert!(at(g0.time(k) + 3.0 * step) && !at(g0.time(k)));
    }

    #[test]
    fn the_rate_readings_are_the_tests_readings_across_a_pause() {
        let cfg = SynthConfig {
            duration_s: 300.0,
            ..Default::default()
        };
        let audio = generate(&cfg, |_| 270.0, |_| 0.0);
        let mut live = LiveAnalyzer::new(48000, LiveConfig::default());
        for b in audio.samples.chunks(4800) {
            live.push(b);
        }
        let mut log = BeatLog {
            sample_rate: live.sample_rate(),
            duration_s: live.duration_s(),
            bph: live.bph().expect("beat rate"),
            lift_deg: live.config().analysis.amplitude.lift_deg,
            beats: live.beats().to_vec(),
            amplitude_windows: live.amplitude_windows().to_vec(),
            clipped_beats: live.clipped_beats().to_vec(),
            clipped_samples: 0,
            jumps: Vec::new(),
            sided: Vec::new(),
            by_count: Vec::new(),
            snippets: Vec::new(),
        };
        let lc = LongConfig {
            wheels: default_wheels(log.bph),
            ..Default::default()
        };
        let plain = longrun::analyse(&log, None, &lc);
        // A 30 s pause 3 s into a rate reading, half way through.
        let g = &check_grids(&log, &plain, 10.0)[0];
        let mid = g.t0 + 15.0 * g.step + 3.0;
        // Beats lost for 3 s, 3 s into reading 5, with no break listed: the
        // tests leave that reading out, so the views must too.
        let lost = g.t0 + 5.0 * g.step + 3.0;
        log.beats.retain(|b| !(lost..lost + 3.0).contains(&b.time));
        let breaks = [crate::breaks::Break {
            audio_s: mid,
            gap_s: 30.0,
        }];
        let paused = longrun::analyse_with_breaks(&log, None, &breaks, &lc);
        let report = steadiness::check(&log, None, &paused, &lc, &Config::default());
        let tests = report
            .distributions
            .overall
            .iter()
            .find(|d| d.series == "rate" && d.values == "readings")
            .expect("rate readings");
        let v = views(&log, &report, &paused, &lc, 10.0);
        let r = &v.series[0].readings;
        assert!(r.counts.iter().all(|&c| c == 1), "none thinned");
        assert_eq!(r.t_s.len(), tests.n);
        let median = crate::dsp::median(&mut r.values.clone());
        assert!(
            (median - tests.median).abs() < 1e-9,
            "{median} {}",
            tests.median
        );
        // Cutting only at the pause's middle, as the tests did before,
        // keeps readings that reach into the pause from either side, so
        // this test bites.
        let holes = crate::clean::Cuts::new(None, &paused.breaks, &[]);
        // The old cut: halfway through each pause's gap.
        let cut: Vec<f64> = paused
            .breaks
            .iter()
            .map(|b| holes.true_time(b.audio_s) - 0.5 * b.gap_s - 1e-6)
            .collect();
        let beats = holes.beats(&log.beats);
        // Without the 2 s check (one part as long as the reading), on the
        // same tiling from the take's start (#636), reading 5 would be kept.
        let end = holes.true_time(log.duration_s);
        let no_check =
            |cuts: &crate::clean::Cuts| cuts.windows(&beats, log.bph, 0.0, end, 10.0, 10.0, 10.0);
        assert_eq!(no_check(&holes).len(), tests.n + 1);
        let mut wins = no_check(&crate::clean::Cuts::none());
        wins.retain(|w| !cut.iter().any(|&c| w.start_s < c && c < w.end_s));
        assert!(wins.len() > tests.n, "{} {}", wins.len(), tests.n);
    }

    /// The app's own test take: ten minutes of a watch whose rate swings
    /// 20 s/d once a minute (a fourth wheel fault), heard as the app hears
    /// it. Pins what each view carries, so the app can move its pane onto
    /// this module and show it draws the same thing.
    #[test]
    fn a_once_a_minute_swing_gives_every_view() {
        let cfg = SynthConfig {
            duration_s: 600.0,
            rate_s_per_day: 5.0,
            ..Default::default()
        };
        let audio = generate(
            &cfg,
            |_| 270.0,
            |t| 20.0 * (std::f64::consts::TAU * t / 60.0).sin(),
        );
        let mut live = LiveAnalyzer::new(48000, LiveConfig::default());
        for b in audio.samples.chunks(4800) {
            live.push(b);
        }
        let log = BeatLog {
            sample_rate: live.sample_rate(),
            duration_s: live.duration_s(),
            bph: live.bph().expect("beat rate"),
            lift_deg: live.config().analysis.amplitude.lift_deg,
            beats: live.beats().to_vec(),
            amplitude_windows: live.amplitude_windows().to_vec(),
            clipped_beats: live.clipped_beats().to_vec(),
            clipped_samples: 0,
            jumps: Vec::new(),
            sided: Vec::new(),
            by_count: Vec::new(),
            snippets: Vec::new(),
        };
        let lc = LongConfig {
            wheels: default_wheels(log.bph),
            ..Default::default()
        };
        let check = Config::default();
        let long = longrun::analyse(&log, None, &lc);
        let report = steadiness::check(&log, None, &long, &lc, &check);
        assert!(
            report.series[0].period.is_some(),
            "{}",
            report.series[0].headline
        );
        let v = views(&log, &report, &long, &lc, check.rate_reading_s);
        let names: Vec<&str> = v.series.iter().map(|s| s.series).collect();
        assert_eq!(names, ["rate", "amplitude", "beat_error"]);

        // Readings: 10 s rate readings and 2 s windows, none thinned.
        let r = &v.series[0].readings;
        // None thinned: each point a reading, as it is.
        let as_is = (vec![1; r.values.len()], r.values.clone(), r.values.clone());
        assert_eq!(
            (r.step_s, (r.counts.clone(), r.lo.clone(), r.hi.clone())),
            (10.0, as_is)
        );
        assert!((55..=60).contains(&r.t_s.len()), "{}", r.t_s.len());
        assert_eq!(r.t_s.len(), r.values.len());
        assert!(r.t_s.windows(2).all(|w| w[0] < w[1]));
        let a = &v.series[1].readings;
        assert_eq!(a.step_s, 2.0);
        assert!((280..=300).contains(&a.n), "{}", a.n);
        // The swing shows: the rate readings span most of its 40 s/d.
        let lo = r.values.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = r.values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(hi - lo > 20.0, "{lo} to {hi}");
        // Their scale is core's, and holds the swing.
        let [y0, y1] = r.y_range.expect("readings scale");
        assert_eq!(chart_scale::spread(&r.values), Some((y0, y1)));
        assert!(y0 < lo + 1.0 && y1 > hi - 1.0, "{y0} to {y1}");

        // Periods: the rate's search peaks near a minute, over its threshold.
        let s = v.series[0].search.as_ref().expect("rate search");
        assert_eq!(s.period_s.len(), s.score.len());
        let best = (0..s.score.len())
            .max_by(|&i, &j| s.score[i].total_cmp(&s.score[j]))
            .unwrap();
        assert!(
            (s.period_s[best] - 60.0).abs() < 3.0,
            "{}",
            s.period_s[best]
        );
        assert!(s.score[best] > s.threshold);
        let [a0, b0] = v.period_span.expect("span");
        assert!(a0 < 60f64.log10() && 60f64.log10() < b0);

        // Cycle: the rate folds at its own minute; the steady amplitude at
        // another series' cycle or a wheel's turn, as in the app.
        let f = v.series[0].fold.as_ref().expect("rate fold");
        assert_eq!(f.from, "own");
        assert!((f.period_s - 60.0).abs() < 3.0, "{}", f.period_s);
        assert_eq!(f.profile.len(), 6);
        assert_eq!(f.raster.len(), f.cycles);
        assert!(f.raster.iter().all(|row| row.len() == f.profile.len()));
        assert_eq!(f.columns.len(), f.profile.len());
        assert!(f.swing.is_some() && f.middle_half.is_some());
        let fa = v.series[1].fold.as_ref().expect("amplitude fold");
        assert_ne!(fa.from, "own");
    }

    #[test]
    fn a_folds_columns_are_its_quartiles_over_every_cycle() {
        let nan = f64::NAN;
        let f = longterm::Fold {
            period_s: 30.0,
            profile: vec![2.0, 5.0, nan],
            counts: vec![5, 5, 3],
            // Five cycles in the first two bins, three in the last.
            raster: vec![
                vec![1.0, 4.0, 1.0],
                vec![2.0, 5.0, nan],
                vec![3.0, 6.0, 2.0],
                vec![4.0, 5.0, 3.0],
                vec![0.0, 5.0, nan],
            ],
            t0: 0.0,
        };
        let c = fold_columns(&f);
        assert_eq!(c, vec![Some([1.0, 2.0, 3.0]), Some([5.0, 5.0, 5.0]), None]);
        // The line swings 5 - 2; the columns' middle halves are 2 and 0.
        assert_eq!(fold_spread(&f, &c), Some((3.0, 1.0)));
        assert_eq!(fold_spread(&f, &[None, None, None]), None);
    }

    /// A fold's scale is taken over every cycle, not only the rows kept
    /// for drawing, so it is the same however many cycles there are.
    #[test]
    fn a_folds_scale_takes_every_cycle() {
        let nan = f64::NAN;
        // 1000 cycles: those kept for drawing read 0, the rest 1, so the
        // rows kept and the whole fold have different scales.
        let kept = spread(1000, MAX_CYCLES);
        let raster: Vec<Vec<f64>> = (0..1000)
            .map(|r| vec![if kept.contains(&r) { 0.0 } else { 1.0 }, nan])
            .collect();
        let all: Vec<f64> = raster.iter().map(|r| r[0]).collect();
        let f = longterm::Fold {
            period_s: 30.0,
            profile: vec![1.0, nan],
            counts: vec![1000, 0],
            raster,
            t0: 0.0,
        };
        assert_eq!(fold_range(&f), chart_scale::spread(&all));
        // The view keeps 200 cycles to draw, and its scale is still every
        // cycle's.
        let v = fold_view(&f, "own", None);
        assert!(
            v.raster.len() < v.cycles,
            "{} of {}",
            v.raster.len(),
            v.cycles
        );
        assert_eq!(v.y_range, fold_range(&f).map(|(a, b)| [a, b]));
        let drawn: Vec<f64> = v.raster.iter().map(|r| r[0]).collect();
        assert_ne!(v.y_range, chart_scale::spread(&drawn).map(|(a, b)| [a, b]));
        assert_ne!(fold_range(&f), chart_scale::spread(&vec![0.0; kept.len()]));
        let empty = longterm::Fold {
            raster: vec![vec![nan, nan]],
            ..f
        };
        assert_eq!(fold_range(&empty), None);
    }

    #[test]
    fn thinning_keeps_both_ends() {
        assert_eq!(spread(5, 10), vec![0, 1, 2, 3, 4]);
        let k = spread(10_000, 200);
        assert_eq!(k.len(), 200);
        assert_eq!(k[0], 0);
        assert_eq!(*k.last().unwrap(), 9_999);
        assert!(k.windows(2).all(|w| w[0] < w[1]));
    }

    fn walk_of(points: &[(f64, f64, f64, usize)]) -> walk::IsochronismWalk {
        walk::IsochronismWalk {
            bin_s: walk::WALK_BIN_S,
            points: points
                .iter()
                .map(
                    |&(t_s, amplitude_deg, rate_s_per_day, readings)| walk::WalkPoint {
                        t_s,
                        wind_h: t_s / 3600.0,
                        rate_s_per_day,
                        amplitude_deg,
                        readings,
                        amp_windows: 5 * readings,
                    },
                )
                .collect(),
        }
    }

    #[test]
    fn the_isochronism_card_says_each_point_in_hours_degrees_and_signed_rate() {
        let w = walk_of(&[
            (150.0, 301.4, -0.04, 30),
            (12_600.0, 272.4, 4.06, 30),
            (137_000.0, 210.6, -2.26, 6),
        ]);
        let v = walk_view(&w).unwrap();
        let hovers: Vec<&str> = v.dots.iter().map(|d| d.hover.as_str()).collect();
        assert_eq!(
            hovers,
            [
                "0.0 h after start · 301° · 0.0 s/d",
                "3.5 h after start · 272° · +4.1 s/d",
                "38 h after start · 211° · -2.3 s/d · 6 readings",
            ]
        );
        assert_eq!(v.last_text, "38 h");
        assert_eq!(v.dots[2].when, 1.0);
        assert!((v.dots[1].when - 12_600.0 / 137_000.0).abs() < 1e-12);
        assert!((v.last_hours - 137_000.0 / 3600.0).abs() < 1e-12);
    }

    /// Dots every half hour from 0 to `last` h, but none in `gap`.
    fn dots_to(last: f64, gap: std::ops::Range<f64>) -> Vec<WalkDot> {
        (0..)
            .map(|k| k as f64 * 0.5)
            .take_while(|h| *h <= last)
            .filter(|h| !gap.contains(h))
            .map(|hours| WalkDot {
                hours,
                when: hours / last,
                amplitude_deg: 250.0,
                rate_s_per_day: 0.0,
                hover: String::new(),
                joined: true,
            })
            .collect()
    }

    /// A bead every 12 h on the dot at that hour, none within 3 h of the
    /// end or in a break; every 24 h over 96 h (Hugh's look C).
    #[test]
    fn the_walk_has_a_bead_at_each_round_hour() {
        let beads = |dots: &[WalkDot], last| {
            walk_beads(dots, last)
                .into_iter()
                .map(|b| (dots[b.dot].hours, b.text))
                .collect::<Vec<_>>()
        };
        let d = dots_to(58.0, 0.0..0.0);
        assert_eq!(
            beads(&d, 58.0),
            [
                (12.0, "12 h".to_string()),
                (24.0, "24 h".into()),
                (36.0, "36 h".into()),
                (48.0, "48 h".into())
            ]
        );
        // 48 h is within 3 h of a 50 h end: no bead there.
        assert_eq!(beads(&dots_to(50.0, 0.0..0.0), 50.0).len(), 3);
        // No dot within an hour of 24 h: no bead there.
        let gap = dots_to(58.0, 22.5..25.5);
        assert!(!beads(&gap, 58.0).iter().any(|b| b.1 == "24 h"));
        assert_eq!(beads(&gap, 58.0).len(), 3);
        let long = beads(&dots_to(120.0, 0.0..0.0), 120.0);
        let texts: Vec<&str> = long.iter().map(|b| b.1.as_str()).collect();
        assert_eq!(texts, ["24 h", "48 h", "72 h", "96 h"]);
    }

    #[test]
    fn a_headline_goes_one_fact_a_line() {
        assert_eq!(
            headline_lines("The rate falls 0.23 s/d per degree, 145° to 251°; flattest near 213°. Slope from 12 bins."),
            ["The rate falls 0.23 s/d per degree, 145° to 251°", "Flattest near 213°", "Slope from 12 bins"]
        );
        assert!(headline_lines("").is_empty());
    }

    #[test]
    fn the_walk_axes_end_round_on_six_to_ten_steps() {
        let dots: Vec<WalkDot> = [(211.4, -3.2), (272.0, 4.1), (301.3, 0.0)]
            .iter()
            .map(|&(a, r)| WalkDot {
                hours: 0.0,
                when: 0.0,
                amplitude_deg: a,
                rate_s_per_day: r,
                hover: String::new(),
                joined: true,
            })
            .collect();
        // Padded as the charts are (5% or 2°, 15%), then rounded.
        let [amp, rate] = walk_axes(&dots, None);
        assert_eq!(amp, (200.0, 310.0, 10.0));
        assert_eq!(rate, (-5.0, 6.0, 1.0));
        // A curve reaching further widens the amplitude; the rate is the
        // walk's own.
        let curve = curve_amp(&[150.0, 200.0, 250.0], 10.0);
        assert_eq!(curve, Some((140.0, 260.0)));
        let [amp, wide_rate] = walk_axes(&dots, curve);
        assert_eq!(amp, (140.0, 320.0, 20.0));
        assert_eq!(wide_rate, rate);
        assert_eq!(curve_amp(&[], 10.0), None);
    }

    #[test]
    fn the_isochronism_line_breaks_where_a_stretch_was_left_out() {
        // Bins at 150, 450 and 750 s, then none until 1650 s: a gap.
        let w = walk_of(&[
            (150.0, 300.0, 0.0, 30),
            (450.0, 299.0, 0.1, 30),
            (750.0, 298.0, 0.2, 30),
            (1650.0, 295.0, 0.5, 30),
            (1950.0, 294.0, 0.6, 30),
        ]);
        let joined: Vec<bool> = walk_view(&w)
            .unwrap()
            .dots
            .iter()
            .map(|d| d.joined)
            .collect();
        assert_eq!(joined, [false, true, true, false, true]);
    }

    #[test]
    fn the_isochronism_card_needs_three_points_and_takes_core_s_merge() {
        assert!(walk_view(&walk_of(&[
            (150.0, 300.0, 0.0, 30),
            (450.0, 299.0, 0.1, 30)
        ]))
        .is_none());
        // 600 base bins merge three to one: 200 points, as every surface gets.
        let many: Vec<_> = (0..600)
            .map(|i| {
                (
                    150.0 + 300.0 * i as f64,
                    300.0 - i as f64 / 10.0,
                    i as f64 / 100.0,
                    30,
                )
            })
            .collect();
        let w = walk_of(&many);
        let v = walk_view(&w).unwrap();
        let m = walk::merged(&w, walk::default_merge(&w));
        assert_eq!(v.dots.len(), m.points.len());
        assert_eq!(v.dots[1].amplitude_deg, m.points[1].amplitude_deg);
    }
}
