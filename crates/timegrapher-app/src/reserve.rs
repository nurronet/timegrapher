//! The Power Reserve Test: the watch wound fully, set on the stand in one
//! position, and listened to until it stops. The app finds the stop
//! (core [`power_reserve`]: the last beat heard before ten minutes with
//! none), keeps the take in the watch's history with the run's results
//! beside it in `power-reserve.json`, and shows the run time. No verdict
//! is given.
//!
//! While it runs the computer is kept awake, the sound is kept, and the
//! position can't change and the listening can't pause: a run is one
//! position from wind to stop.

use crate::app::Position;
use crate::strength::{self, Grid};
use crate::theme;
use eframe::egui::{self, RichText};
use std::path::PathBuf;
use timegrapher_core::beats::Beat;
use timegrapher_core::chart_scale::axis_number;
use timegrapher_core::library::length_text;
use timegrapher_core::power_reserve::{self, Ending, Heard, Results};

/// Bytes of sound written an hour at `sample_rate` and `bits`, one
/// channel, as the recorder keeps it.
pub fn bytes_per_hour(sample_rate: u32, bits: u16) -> u64 {
    u64::from(sample_rate) * u64::from(bits.div_ceil(8)) * 3600
}

/// The sound a run of [`power_reserve::LIMIT_S`] could write at most.
pub fn bytes_needed(sample_rate: u32, bits: u16) -> u64 {
    bytes_per_hour(sample_rate, bits) * (power_reserve::LIMIT_S / 3600.0) as u64
}

/// Bytes free to this user on the disk holding `dir`.
#[cfg(unix)]
pub fn free_bytes(dir: &std::path::Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is a valid C string and `st` a place for the answer.
    if unsafe { libc::statvfs(path.as_ptr(), &mut st) } != 0 {
        return None;
    }
    #[allow(clippy::unnecessary_cast, clippy::useless_conversion)]
    Some(u64::from(st.f_bavail as u64) * u64::from(st.f_frsize as u64))
}

/// Bytes free to this user on the disk holding `dir`.
#[cfg(windows)]
pub fn free_bytes(dir: &std::path::Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
    let mut free = 0u64;
    // SAFETY: `wide` ends in a nul; the other answers aren't asked for.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(free)
}

/// A size in GB, one decimal: "17.3 GB".
pub fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

/// Beats a run keeps, from its first to its limit at 43,200 bph, the
/// fastest beat rate, so its start and its hours are read from every beat
/// of it, not only the day the Bench keeps (about 80 MB at most).
pub const RUN_BEATS_KEPT: usize =
    (power_reserve::LIMIT_S as usize + power_reserve::STOP_AFTER_S as usize) * 12;

/// A test under way or finished.
#[derive(Debug, Clone)]
pub struct Test {
    pub position: Position,
    /// Keep the sound in the take, or keep its analysis only.
    pub keep_sound: bool,
    /// When it started, UTC, RFC 3339.
    pub started_utc: String,
    /// The session's audio seconds when it started: beats before don't
    /// count.
    pub began_s: f64,
    /// The beats heard since.
    pub heard: Heard,
    /// The session's audio seconds when last looked at.
    pub now_s: f64,
    /// How it ended, once it has.
    pub ending: Option<Ending>,
    /// Its results, once it has ended and the stop was found.
    pub results: Option<Results>,
    /// While it runs, its hours so far, the newest still filling.
    pub so_far: Vec<power_reserve::Hour>,
    /// The take it was saved as.
    pub saved: Option<PathBuf>,
    /// Its take is being written.
    pub saving: bool,
}

impl Test {
    pub fn new(position: Position, keep_sound: bool, started_utc: String, began_s: f64) -> Test {
        Test {
            position,
            keep_sound,
            started_utc,
            began_s,
            heard: power_reserve::heard(&[], 1.0),
            now_s: began_s,
            ending: None,
            results: None,
            so_far: Vec::new(),
            saved: None,
            saving: false,
        }
    }

    pub fn running(&self) -> bool {
        self.ending.is_none()
    }

    /// Read the session's `beats` up to `now_s` at `beat_s` seconds a
    /// beat: how it ended, when it just has (the watch stopped, or the
    /// time limit).
    pub fn update(&mut self, beats: &[Beat], beat_s: f64, now_s: f64) -> Option<Ending> {
        if !self.running() {
            return None;
        }
        let from = beats.partition_point(|b| b.time < self.began_s);
        self.heard = power_reserve::heard(&beats[from..], beat_s);
        self.now_s = now_s;
        let ending = self.heard.ending(now_s, self.began_s)?;
        self.ending = Some(ending);
        Some(ending)
    }

    /// End it by hand.
    pub fn stop(&mut self) {
        if self.running() {
            self.ending = Some(Ending::ByHand);
        }
    }

    /// Its results, with `to_true` mapping audio seconds to true seconds:
    /// `None` until it ends, or when no beat was heard.
    pub fn finish(&mut self, to_true: impl Fn(f64) -> f64) -> Option<&Results> {
        let ending = self.ending?;
        self.results = Results::of(
            &self.heard,
            ending,
            &self.started_utc,
            self.position.code(),
            self.keep_sound,
            to_true,
        );
        self.results.as_ref()
    }

    /// Seconds from the first beat heard to now, or to the last beat once
    /// ended, in audio seconds.
    pub fn run_so_far_s(&self) -> f64 {
        match (self.heard.first_s, self.heard.last_s) {
            (Some(a), Some(b)) if !self.running() => b - a,
            (Some(a), _) => (self.now_s - a).max(0.0),
            _ => 0.0,
        }
    }
}

/// The pane's body, in no more height than it shows: before a run one
/// line of what to do (the steps are in its "?"); while it runs the run
/// time and one line of how the beats stand; once ended one line of the
/// result over the hour charts and table, which take the rest of the pane
/// and scroll in it. `card` is the position card's title as drawn,
/// `watch` the watch the take is kept in (none: Unsorted), and `ready`
/// whether a run is set up, waiting for Start Test on the Listen card.
pub fn pane(
    ui: &mut egui::Ui,
    test: Option<&Test>,
    free: Option<(u64, u64)>,
    card: &str,
    watch: &str,
    ready: bool,
) {
    ui.spacing_mut().item_spacing.y = 6.0;
    let Some(t) = test else {
        before_line(ui, free, card, ready);
        return;
    };
    let pauses = (!t.heard.pauses.is_empty()).then(|| {
        format!(
            " Beats stopped and came back {} time{} on the way.",
            t.heard.pauses.len(),
            if t.heard.pauses.len() == 1 { "" } else { "s" }
        )
    });
    let pause_hint = format!(
        "Silences of {:.0} s or more inside the run, after which the beats came back: a watch \
         near the end can stop and start again with a knock",
        power_reserve::PAUSE_S
    );
    let pauses = pauses.map(|words| (words, pause_hint));
    if t.running() {
        running_line(ui, t, pauses.as_ref());
    } else {
        ended_line(ui, t, watch, pauses.as_ref());
    }
}

/// Before a run: one line of what to do, with a warning when the disk
/// where the take is kept has too little room for a long run.
fn before_line(ui: &mut egui::Ui, free: Option<(u64, u64)>, card: &str, ready: bool) {
    let pal = theme::pal(ui);
    let words = if ready {
        format!("Wind fully, set the position on the {card} card, then press Start Test.")
    } else {
        "New Test sets a run up on a clear Bench.".to_string()
    };
    let mut line =
        ui.add(egui::Label::new(RichText::new(words).color(pal.text_secondary)).truncate());
    if let Some((have, need)) = free {
        let room = format!(
            "{} free where the take is kept; a run of up to {} writes up to {} of sound.",
            gb(have),
            timegrapher_core::power_reserve::limit_text(),
            gb(need)
        );
        if have >= need {
            line = line.on_hover_text(room);
        } else {
            theme::alert_label(
                ui,
                theme::Alert::Warning,
                &format!(
                    "Only {} free where the take is kept: a long run could fill the disk.",
                    gb(have)
                ),
                true,
            )
            .on_hover_text(room);
        }
    }
    let _ = line;
}

/// While a run goes on: the run time and how the beats stand, then the
/// hours so far as when it ends. `pauses`: the words for the beats'
/// pauses on the way and their hover.
fn running_line(ui: &mut egui::Ui, t: &Test, pauses: Option<&(String, String)>) {
    let pal = theme::pal(ui);
    let state = match t.heard.silent_s(t.now_s) {
        None => "Waiting for the first beats…".to_string(),
        Some(s) if s >= power_reserve::PAUSE_S => format!(
            "No beat heard for {}; the run ends after {} min with none.",
            length_text(s),
            power_reserve::STOP_AFTER_S / 60.0
        ),
        Some(_) => format!("Running, in {}.", t.position.label()),
    };
    let line = format!(
        "{} · {state}{}",
        power_reserve::run_text(t.run_so_far_s()),
        pauses.map_or("", |p| p.0.as_str())
    );
    let mut hover = "The run so far, from the first beat heard to now".to_string();
    if let Some((_, pause_hint)) = pauses {
        hover.push_str(&format!(". {pause_hint}"));
    }
    ui.add(egui::Label::new(RichText::new(line).color(pal.text)).truncate())
        .on_hover_text(hover);
    if !t.so_far.is_empty() {
        hour_area(ui, &t.so_far, true);
    }
}

/// Once a run has ended: one line of the result, over the hour charts
/// and table. `pauses`: as for [`running_line`].
fn ended_line(ui: &mut egui::Ui, t: &Test, watch: &str, pauses: Option<&(String, String)>) {
    let pal = theme::pal(ui);
    // As core words the run: never a power reserve for a long take.
    let ran = t.results.as_ref().map_or_else(
        || format!("Ran {}", power_reserve::run_text(t.run_so_far_s())),
        Results::ran,
    );
    let ending = t
        .ending
        .map(|e| format!(" · {}, in {}", e.text(), t.position.label()))
        .unwrap_or_default();
    // Read from a long take, not a test: said where it came from.
    let source = t
        .results
        .as_ref()
        .and_then(|r| r.source.text())
        .map_or_else(String::new, |w| format!(" · {w}"));
    let kept = if t.saving {
        " · Saving the take…".to_string()
    } else if t.saved.is_some() {
        if watch.is_empty() {
            " · Kept in Unsorted".to_string()
        } else {
            format!(" · Kept in {watch}")
        }
    } else {
        String::new()
    };
    let mut hover = "The run, from the first beat heard to the last, in true time".to_string();
    if let Some(p) = &t.saved {
        hover.push_str(&format!(
            ". Kept as \"{}\"{}; every pane reads it as any take",
            p.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into()),
            if t.keep_sound {
                ""
            } else {
                ", as its analysis only once that is written"
            }
        ));
    }
    if let Some((_, pause_hint)) = pauses {
        hover.push_str(&format!(". {pause_hint}"));
    }
    ui.add(
        egui::Label::new(
            RichText::new(format!(
                "{ran}{ending}{source}{kept}.{}",
                pauses.map_or("", |p| p.0.as_str())
            ))
            .color(pal.text),
        )
        .truncate(),
    )
    .on_hover_text(hover);
    if let Some(r) = t.results.as_ref().filter(|r| !r.hours.is_empty()) {
        hour_area(ui, &r.hours, false);
    }
}

/// The hour charts and table, taking the rest of the pane and scrolling
/// in it, their bar drawn at rest, from the top each time the pane is
/// shown; `so_far` while the run goes on, its newest hour still filling.
fn hour_area(ui: &mut egui::Ui, hours: &[power_reserve::Hour], so_far: bool) {
    ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
    let viewport_h = ui.available_height();
    let id = egui::Id::new("reserve run scroll");
    let pass = ui.ctx().cumulative_pass_nr();
    let last = ui.ctx().data_mut(|d| {
        let last = d.get_temp::<u64>(id);
        d.insert_temp(id, pass);
        last
    });
    // Never taller than the room left, however little: egui's least
    // scrolled height of 64 would run the foot past a short pane, out of
    // reach of the wheel.
    let mut area = egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .min_scrolled_height(0.0)
        .max_height(viewport_h.max(0.0))
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded);
    if last.is_none_or(|l| l + 1 < pass) {
        area = area.vertical_scroll_offset(0.0);
    }
    area.show(ui, |ui| {
        // The charts take the height left after the hours' table (or a
        // third of it, for a long table); when that leaves them short of a
        // readable height, all of it, the table under them. What doesn't
        // fit scrolls.
        let row_h = ui
            .text_style_height(&egui::TextStyle::Body)
            .max(ui.spacing().interact_size.y)
            + 3.0;
        let gap = ui.spacing().item_spacing.y;
        let room = viewport_h - 8.0 - 3.0 * gap;
        let table_h = ((hours.len() as f32 + 1.0) * row_h + 4.0).min(room / 3.0);
        let with_table = (room - table_h - gap) / 2.0;
        let stacked = if with_table >= CHART_READ_H {
            with_table
        } else {
            ((room - gap) / 2.0).max(CHART_MIN_H)
        };
        // Too short to stack them at a height that reads, but wide enough:
        // side by side, each the whole height.
        let side = stacked < CHART_READ_H && ui.available_width() >= 2.0 * SIDE_MIN_W;
        // Side by side, they fill the pane, the table wholly under them.
        let chart_h = if side {
            (viewport_h - 8.0).max(CHART_MIN_H)
        } else {
            stacked
        };
        // While it runs the charts gain a point as each hour is whole.
        let whole = if so_far {
            hours.iter().take_while(|h| h.whole()).count()
        } else {
            hours.len()
        };
        if whole > 0 {
            hour_chart(ui, &hours[..whole], chart_h, side);
            ui.add_space(8.0);
        }
        hour_table(ui, hours);
    });
}

/// The least height of each of the Power Reserve charts, points.
const CHART_MIN_H: f32 = 56.0;

/// The least width of each Power Reserve chart side by side, points.
const SIDE_MIN_W: f32 = 360.0;

/// The height each Power Reserve chart reads well at, points: its hours'
/// labels and a plot that names a few values.
const CHART_READ_H: f32 = 100.0;

/// The least height the pane's run needs under its toolbar once it has
/// charts: its line and both charts at a height that reads, the table
/// scrolling under them; none before then. The Paper Strip above gives
/// room for it, down to its own least.
pub fn least_h(test: Option<&Test>) -> Option<f32> {
    let t = test?;
    let charts = match t.results.as_ref() {
        Some(r) => !r.hours.is_empty(),
        None => t.so_far.iter().any(|h| h.whole()),
    };
    charts.then_some(22.0 + 2.0 * CHART_READ_H + 8.0 + 12.0)
}

/// A run stopped by hand after `hours` whole hours, for the tests.
#[cfg(test)]
pub fn finished_run(hours: u32) -> Test {
    let beats: Vec<Beat> = (0..160)
        .map(|i| Beat {
            index: i,
            time: i as f64 * 0.2,
            quality: 1.0,
        })
        .collect();
    let mut t = Test::new(Position::DialUp, true, String::new(), 0.0);
    t.update(&beats, 0.2, 32.0);
    t.stop();
    t.finish(|s| s);
    if let Some(r) = t.results.as_mut() {
        r.hours = (0..hours)
            .map(|h| power_reserve::Hour {
                hour: h,
                span_s: 3600.0,
                rate_s_per_day: Some(6.5),
                amplitude_deg: Some(281.0),
                beat_error_ms: Some(0.4),
                beat_error_source: None,
                caveats: Vec::new(),
                read_s: 3500.0,
            })
            .collect();
    }
    t
}

/// The height a run of `hours` rows reads well in: the toolbar and the
/// run's line, two charts of a readable height, and the table.
pub fn run_h(hours: usize) -> f32 {
    let row_h = 22.0;
    60.0 + 2.0 * 160.0 + 8.0 + (hours as f32 + 1.0) * row_h
}

/// A value or a dash.
fn cell(v: Option<f64>, d: usize, unit: &str) -> String {
    v.map_or_else(|| "–".to_string(), |x| format!("{x:.d$}{unit}"))
}

/// The hour column's words: "0–1 h", and the last part hour's length.
fn hour_label(h: &power_reserve::Hour) -> String {
    if h.whole() {
        format!("{}–{} h", h.hour, h.hour + 1)
    } else {
        // Whole seconds, as the run's own words count them.
        format!(
            "{} h + {}",
            h.hour,
            length_text(power_reserve::whole_s(h.span_s))
        )
    }
}

/// The run hour by hour as a table, the hour in progress read like a
/// finished run's last, part hour.
fn hour_table(ui: &mut egui::Ui, hours: &[power_reserve::Hour]) {
    let pal = theme::pal(ui);
    egui::Grid::new("reserve hours")
        .striped(true)
        .spacing([18.0, 3.0])
        .show(ui, |ui| {
            for (head, hint) in [
                ("Hours", "Hours since the first beat heard"),
                (
                    "Rate",
                    "The hour's rate, s/d, as the command line reads a stretch",
                ),
                ("Amplitude", "The hour's median amplitude"),
                (
                    "Beat Error",
                    "The hour's beat error from the unlock, ms (its size)",
                ),
                ("Read", "How much of the hour its whole 10 s readings cover"),
            ] {
                ui.label(RichText::new(head).strong().color(pal.text_secondary))
                    .on_hover_text(hint);
            }
            ui.end_row();
            for h in hours {
                ui.label(hour_label(h));
                ui.label(cell(h.rate_s_per_day, 1, " s/d"));
                ui.label(cell(h.amplitude_deg, 0, "°"));
                ui.label(cell(h.beat_error_ms.map(f64::abs), 1, " ms"));
                ui.label(format!("{:.0}%", 100.0 * h.read_s / h.span_s.max(1.0)));
                ui.end_row();
            }
        });
}

/// The steps, in hours, the Power Reserve charts' hours go up in.
const HOUR_STEPS: [f64; 7] = [1.0, 2.0, 3.0, 6.0, 12.0, 24.0, 48.0];

/// The Power Reserve charts' hour marks: none before the run, in the
/// margin left of 0 h.
fn hour_marks(g: egui_plot::GridInput) -> Vec<egui_plot::GridMark> {
    // 80 points apart at least: egui_plot names hours only from 60 apart,
    // and fades them in up to 80.
    let mut marks = theme::even_grid(g, 80.0, &HOUR_STEPS);
    marks.retain(|m| m.value >= 0.0);
    marks
}

/// The least width of the Power Reserve charts' value axis, points.
const GUTTER: f32 = 64.0;

/// The Power Reserve charts' two figures: name, unit and reading.
type Series = (
    &'static str,
    &'static str,
    fn(&power_reserve::Hour) -> Option<f64>,
);

/// Rate, then amplitude.
const SERIES: [Series; 2] = [
    ("rate", " s/d", |h| h.rate_s_per_day),
    ("amplitude", "°", |h| h.amplitude_deg),
];

/// One width for both charts' value axes, so their plots start at the
/// same x: the least gutter, or the widest label either chart may draw,
/// read from each figure's whole-number ends with a decimal to spare.
fn shared_gutter(ui: &egui::Ui, hours: &[power_reserve::Hour]) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let wide = |t: String| {
        ui.fonts_mut(|f| {
            f.layout_no_wrap(t, font.clone(), egui::Color32::WHITE)
                .size()
                .x
        })
    };
    let mut w = GUTTER;
    for (_, unit, get) in SERIES {
        for v in hours.iter().filter_map(get) {
            for end in [v.floor(), v.ceil()] {
                w = w.max(wide(format!("{end:.1}{unit}")) + 8.0);
            }
        }
    }
    w
}

/// Where the run's hours end: the last hour's end, an hour at least, so a
/// short run reads 0 h to 1 h rather than the one hour over and over.
fn run_end_h(hours: &[power_reserve::Hour]) -> f64 {
    hours
        .iter()
        .map(|h| f64::from(h.hour) + h.span_s / 3600.0)
        .fold(1.0, f64::max)
}

/// Rate and amplitude against hours since the first beat, one above the
/// other on the same hours, or `side` by side in a short, wide pane: both
/// span 0 h to the run's end, on one width of value axis, so an hour sits
/// at the same x in each when stacked.
fn hour_chart(ui: &mut egui::Ui, hours: &[power_reserve::Hour], height: f32, side: bool) {
    let end = run_end_h(hours);
    // A 5% margin each side, as egui's own, the same in both, so 0 h
    // clears the value axis.
    let x_range = (-end * 0.05, end * 1.05);
    let gutter = shared_gutter(ui, hours);
    if side {
        ui.columns(2, |cols| {
            for (col, series) in cols.iter_mut().zip(SERIES) {
                one_chart(col, hours, series, height, x_range, gutter);
            }
        });
    } else {
        for series in SERIES {
            one_chart(ui, hours, series, height, x_range, gutter);
        }
    }
}

/// One of the Power Reserve charts: `series` against hours since the first
/// beat, `height` tall, over `x_range` hours, its value axis `gutter` wide.
fn one_chart(
    ui: &mut egui::Ui,
    hours: &[power_reserve::Hour],
    (name, unit, get): Series,
    height: f32,
    (x_lo, x_hi): (f64, f64),
    gutter: f32,
) {
    use egui_plot::{Line, Plot, PlotPoints};
    let pal = theme::pal(ui);
    let x = |h: &power_reserve::Hour| f64::from(h.hour) + h.span_s / 7200.0;
    {
        let colour = if name == "rate" {
            pal.trace_rate
        } else {
            pal.trace_amplitude
        };
        let pts: Vec<[f64; 2]> = hours
            .iter()
            .filter_map(|h| get(h).map(|v| [x(h), v]))
            .collect();
        let mut grid = Grid::new(hour_marks, |g| theme::even_grid(g, 24.0, &[]));
        if name == "rate" {
            grid = grid.zero_y();
        }
        grid.on(Plot::new(format!("reserve {name}")))
            .height(height)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .default_x_bounds(x_lo, x_hi)
            .y_axis_min_width(gutter)
            .x_axis_formatter(|m, _| format!("{} h", axis_number(m.value, m.step_size)))
            // The point under the pointer in words: hours into the run and
            // the hour's figure.
            .label_formatter(move |name, v| {
                if name.is_empty() {
                    return String::new();
                }
                format!(
                    "{} into the run\n{:.d$}{unit}",
                    length_text((v.x * 3600.0).max(0.0)),
                    v.y,
                    d = usize::from(unit != "°")
                )
            })
            // Never "-0 s/d" on the zero line.
            .y_axis_formatter(move |m, _| format!("{}{unit}", axis_number(m.value, m.step_size)))
            .show(ui, |p| {
                grid.draw(p, pal);
                p.line(
                    Line::new(name, PlotPoints::from(pts))
                        .color(colour)
                        .width(strength::DATA_W),
                );
            });
    }
}

/// The dialog for saving the run as a CSV file, starting in `dir`.
pub fn csv_dialog(dir: &std::path::Path) -> rfd::FileDialog {
    let stamp: String = timegrapher_core::recorder::utc(std::time::SystemTime::now())
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'T')
        .take(15)
        .collect();
    let mut d = rfd::FileDialog::new()
        .set_title("Save the Power Reserve Test as CSV")
        .set_file_name(format!("power-reserve-{stamp}Z.csv"))
        .add_filter("CSV", &["csv"]);
    if dir.is_dir() {
        d = d.set_directory(dir);
    }
    d
}

/// The run as text with `sep` between columns, for Copy and a CSV file:
/// the run, then the hours.
pub fn results_text(r: &Results, watch: &str, sep: char) -> String {
    let mut out = String::new();
    let line = |cells: &[String]| {
        cells
            .iter()
            .map(|c| {
                if sep == ',' && c.contains(',') {
                    format!("\"{c}\"")
                } else {
                    c.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(&sep.to_string())
            + "\n"
    };
    out += &line(&["Watch".into(), watch.into()]);
    if !r.started_utc.is_empty() {
        out += &line(&["Started (UTC)".into(), r.started_utc.clone()]);
    }
    out += &line(&["Position".into(), r.position.clone()]);
    out += &line(&[r.run_label().into(), r.run_words()]);
    out += &line(&["Ended".into(), r.ending.text()]);
    if let Some(w) = r.source.text() {
        out += &line(&["Read from".into(), w.into()]);
    }
    out += "\n";
    out += &line(&[
        "Hour".into(),
        "Length (s)".into(),
        "Rate (s/d)".into(),
        "Amplitude (deg)".into(),
        "Beat error (ms)".into(),
        "Read (s)".into(),
    ]);
    let num = |v: Option<f64>, d: usize| v.map_or_else(String::new, |x| format!("{x:.d$}"));
    for h in &r.hours {
        out += &line(&[
            h.hour.to_string(),
            format!("{:.0}", h.span_s),
            num(h.rate_s_per_day, 1),
            num(h.amplitude_deg, 0),
            num(h.beat_error_ms.map(f64::abs), 2),
            format!("{:.0}", h.read_s),
        ]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beats(from: f64, to: f64, step: f64) -> Vec<Beat> {
        let n = ((to - from) / step).round() as i64;
        (0..n)
            .map(|i| Beat {
                index: i,
                time: from + i as f64 * step,
                quality: 1.0,
            })
            .collect()
    }

    #[test]
    fn a_run_ends_ten_minutes_after_the_last_beat_and_keeps_its_results() {
        let step = 0.125;
        // Beats before the test started don't count.
        let mut b = beats(0.0, 50.0, step);
        b.extend(beats(100.0, 4000.0, step));
        let mut t = Test::new(
            Position::CrownLeft,
            false,
            "2026-10-22T09:15:00Z".into(),
            90.0,
        );
        assert_eq!(t.update(&b, step, 4100.0), None);
        assert!(t.running());
        let last = 4000.0 - step;
        assert_eq!(t.update(&b, step, last + 600.0), Some(Ending::Stopped));
        assert!(!t.running());
        assert!((t.run_so_far_s() - (last - 100.0)).abs() < 1e-9);
        let r = t.finish(|s| s).unwrap();
        assert_eq!(r.position, "CL");
        assert!(!r.sound_kept);
        assert!((r.first_beat_s - 100.0).abs() < 1e-9);
        // Nothing moves once it has ended.
        assert_eq!(t.update(&b, step, 1e6), None);
    }

    #[test]
    fn stopped_by_hand_or_at_the_limit() {
        let step = 0.2;
        let b = beats(0.0, 1000.0, step);
        let mut t = Test::new(Position::DialUp, true, String::new(), 0.0);
        t.update(&b, step, 1000.0);
        t.stop();
        assert_eq!(t.finish(|s| s).unwrap().ending, Ending::ByHand);
        let mut t = Test::new(Position::DialUp, true, String::new(), 0.0);
        let far = beats(power_reserve::LIMIT_S - 10.0, power_reserve::LIMIT_S, step);
        let all: Vec<Beat> = b.into_iter().chain(far).collect();
        assert_eq!(
            t.update(&all, step, power_reserve::LIMIT_S),
            Some(Ending::Limit)
        );
        assert_eq!(gb(bytes_needed(48_000, 16)), "27.6 GB");
        assert_eq!(bytes_per_hour(48_000, 24), 48_000 * 3 * 3600);
        assert!(free_bytes(&std::env::temp_dir()).is_some_and(|b| b > 0));
    }

    #[test]
    fn the_results_copy_as_a_table() {
        let mut t = Test::new(Position::DialUp, true, "2026-10-22T16:15:00Z".into(), 0.0);
        t.update(&beats(0.0, 100.0, 0.2), 0.2, 100.0);
        t.stop();
        let mut r = t.finish(|s| s).unwrap().clone();
        r.hours = vec![power_reserve::Hour {
            hour: 0,
            span_s: 99.8,
            rate_s_per_day: Some(4.25),
            amplitude_deg: Some(281.4),
            beat_error_ms: Some(-0.4),
            beat_error_source: None,
            caveats: Vec::new(),
            read_s: 90.0,
        }];
        let text = results_text(&r, "Pepsi, 1972", ',');
        assert!(text.starts_with("Watch,\"Pepsi, 1972\"\n"), "{text}");
        assert!(
            text.contains("Ended,Stopped by hand while the watch still ran"),
            "{text}"
        );
        assert!(
            // Beat error as its size, as every text gives it.
            text.ends_with("0,100,4.2,281,0.40,90\n") || text.ends_with("0,100,4.3,281,0.40,90\n"),
            "{text}"
        );
        // Whole seconds, as the run's own words count them: 99.8 s is
        // "1 min 39 s", never a second over.
        assert_eq!(hour_label(&r.hours[0]), "0 h + 1:39");
    }

    /// After a run, its two charts take the pane's height and nothing is
    /// cut at its foot: a taller pane gives taller charts, and the hours'
    /// table ends inside the pane.
    #[test]
    fn a_finished_runs_charts_take_the_panes_height() {
        let mut t = Test::new(Position::DialUp, true, String::new(), 0.0);
        t.update(&beats(0.0, 32.0, 0.2), 0.2, 32.0);
        t.stop();
        t.finish(|s| s).unwrap();
        if let Some(r) = t.results.as_mut() {
            r.hours = (0..3)
                .map(|h| power_reserve::Hour {
                    hour: h,
                    span_s: 3600.0,
                    rate_s_per_day: Some(6.5),
                    amplitude_deg: Some(281.0),
                    beat_error_ms: Some(0.4),
                    beat_error_source: None,
                    caveats: Vec::new(),
                    read_s: 3500.0,
                })
                .collect();
        }
        let ctx = egui::Context::default();
        theme::install(&ctx);
        // Each "0 h" axis label's height, and the foot of the last text.
        let drawn = |h: f32| {
            let mut texts = Vec::new();
            for _ in 0..2 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700.0, h),
                    )),
                    ..Default::default()
                };
                let out = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        pane(ui, Some(&t), None, "Position", "Pepsi", true);
                    });
                });
                texts = out
                    .shapes
                    .iter()
                    .filter(|c| {
                        c.clip_rect.intersects(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(700.0, h),
                        ))
                    })
                    .filter_map(|c| match &c.shape {
                        egui::Shape::Text(t) => Some((
                            t.galley.text().to_string(),
                            t.galley.rect.translate(t.pos.to_vec2()),
                        )),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
            }
            let zero: Vec<f32> = texts
                .iter()
                .filter(|t| t.0 == "0 h")
                .map(|t| t.1.center().y)
                .collect();
            assert_eq!(zero.len(), 2, "{texts:?}");
            // Nothing drawn starts below the pane: what doesn't fit is in
            // the table, which scrolls.
            assert!(texts.iter().all(|t| t.1.top() < h), "{texts:?}");
            let last = texts.iter().find(|t| t.0 == "2–3 h").map(|t| t.1.bottom());
            (zero[1] - zero[0], last)
        };
        let (short, _) = drawn(380.0);
        let (tall, last) = drawn(760.0);
        // Room enough: the whole table shows.
        assert!(last.is_some_and(|b| b <= 760.0), "{last:?}");
        assert!(tall > short + 120.0, "{short} {tall}");
    }

    /// A finished run in a short pane scrolls, by the wheel, to its
    /// Amplitude chart and its last hour, and opens again at its top; in a
    /// pane with room, its one hour's row shows whole.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "over the limit before it was set; split it (architecture guide, section 12)"
    )]
    fn a_short_panes_run_scrolls_to_its_last_hour_and_opens_at_its_top() {
        let run = |hours: u32| {
            let mut t = Test::new(Position::DialUp, true, String::new(), 0.0);
            t.update(&beats(0.0, 32.0, 0.2), 0.2, 32.0);
            t.stop();
            t.finish(|s| s).unwrap();
            if let Some(r) = t.results.as_mut() {
                r.hours = (0..hours)
                    .map(|h| power_reserve::Hour {
                        hour: h,
                        span_s: 3600.0,
                        rate_s_per_day: Some(6.5),
                        amplitude_deg: Some(281.0),
                        beat_error_ms: Some(0.4),
                        beat_error_source: None,
                        caveats: Vec::new(),
                        read_s: 3500.0,
                    })
                    .collect();
            }
            t
        };
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let (w, h) = (600.0, 240.0);
        let pass = |t: Option<&Test>, h: f32, wheel: bool| {
            let at = egui::pos2(w / 2.0, h / 2.0);
            let mut events = vec![egui::Event::PointerMoved(at)];
            if wheel {
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -300.0),
                    modifiers: Default::default(),
                });
            }
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(w, h),
                )),
                events,
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if let Some(t) = t {
                        pane(ui, Some(t), None, "Position", "Pepsi", true);
                    }
                });
            });
            out.shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(s) => {
                        let r = s.galley.rect.translate(s.pos.to_vec2());
                        c.clip_rect
                            .contains_rect(r)
                            .then(|| (s.galley.text().to_string(), r))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let t = run(5);
        let shown = |texts: &[(String, egui::Rect)], what: &str| texts.iter().any(|t| t.0 == what);
        // The Rate chart's own scale, "7 s/d", at the top of what scrolls.
        let rate_scale = |texts: &[(String, egui::Rect)]| {
            texts
                .iter()
                .any(|t| t.0.ends_with(" s/d") && !t.0.contains('.'))
        };
        let mut texts = pass(Some(&t), h, false);
        texts = pass(Some(&t), h, false).into_iter().chain(texts).collect();
        assert!(rate_scale(&texts), "the top first: {texts:?}");
        assert!(
            texts
                .iter()
                .any(|t| t.0.starts_with("Ran ") && t.0.contains(" s · ")),
            "one line of the result: {texts:?}"
        );
        assert!(!shown(&texts, "4–5 h"), "{texts:?}");
        for _ in 0..12 {
            texts = pass(Some(&t), h, true);
        }
        assert!(
            shown(&texts, "4–5 h"),
            "the last hour, scrolled to: {texts:?}"
        );
        // A pane with under 64 points left under its line: the last hour
        // still scrolls whole into it.
        let low = 84.0;
        let mut texts = Vec::new();
        for _ in 0..30 {
            texts = pass(Some(&t), low, true);
        }
        let row = texts.iter().find(|t| t.0 == "4–5 h");
        assert!(
            row.is_some_and(|r| r.1.bottom() <= low),
            "{row:?} {texts:?}"
        );
        for _ in 0..60 {
            pass(Some(&t), h, false);
        }
        // The wheel's glide spent, away a pass and back: at the top again.
        for _ in 0..60 {
            pass(Some(&t), h, false);
        }
        assert!(!rate_scale(&pass(Some(&t), h, false)));
        pass(None, h, false);
        pass(Some(&t), h, false);
        let texts = pass(Some(&t), h, false);
        assert!(rate_scale(&texts), "{texts:?}");
        // Room for it all: the one hour's row whole, nothing to scroll.
        let one = run(1);
        pass(Some(&one), 520.0, false);
        let texts = pass(Some(&one), 520.0, false);
        let row = texts
            .iter()
            .find(|t| t.0 == "0–1 h")
            .unwrap_or_else(|| panic!("{texts:?}"));
        assert!(row.1.bottom() <= 520.0, "{row:?}");
        assert!(shown(&texts, "280°") || shown(&texts, "281°"), "{texts:?}");
    }

    /// The pane holds no more height than it shows: one line before a run,
    /// and one line while it runs until its first hours are measured.
    #[test]
    fn the_pane_holds_no_more_than_it_shows() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let height = |t: Option<&Test>, free: Option<(u64, u64)>| {
            let mut h = 0.0;
            let mut texts = Vec::new();
            for _ in 0..2 {
                let out = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let top = ui.cursor().top();
                        pane(ui, t, free, "Position", "", true);
                        h = ui.cursor().top() - top - ui.spacing().item_spacing.y;
                    });
                });
                texts = out
                    .shapes
                    .iter()
                    .filter_map(|c| match &c.shape {
                        egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
            }
            (h, texts)
        };
        // Ready: the toolbar's 28 points and one line in 64.
        let (h, texts) = height(None, Some((500_000_000_000, 100_000_000_000)));
        assert!(h <= 64.0 - 28.0, "{h}");
        assert_eq!(
            texts,
            ["Wind fully, set the position on the Position card, then press Start Test."]
        );
        // Running, before its first hours are measured: one line of run
        // time and state, with the toolbar, in 64.
        let mut t = Test::new(Position::DialUp, true, String::new(), 0.0);
        t.update(&beats(0.0, 32.0, 0.2), 0.2, 32.0);
        assert!(t.running());
        let (h, texts) = height(Some(&t), None);
        assert!(h <= 64.0 - 28.0, "{h}");
        assert_eq!(texts.len(), 1, "{texts:?}");
        assert!(texts[0].starts_with("32 s · "), "{texts:?}");
        // Ended with no hours yet: its one line.
        t.stop();
        let (h, texts) = height(Some(&t), None);
        assert!(h <= 64.0 - 28.0, "{h} {texts:?}");
        assert!(texts[0].starts_with("Ran "), "{texts:?}");
    }

    /// While a run goes on, its hours show as they come: a row for each,
    /// the hour in progress read like a finished run's last, and a chart
    /// point for each whole hour only; before its first whole hour, the
    /// table alone.
    #[test]
    fn a_running_test_shows_its_hours_so_far() {
        let hour = |h: u32, span_s: f64| power_reserve::Hour {
            hour: h,
            span_s,
            rate_s_per_day: Some(6.5),
            amplitude_deg: Some(281.0),
            beat_error_ms: Some(0.4),
            beat_error_source: None,
            caveats: Vec::new(),
            read_s: span_s - 100.0,
        };
        let mut t = Test::new(Position::DialUp, true, String::new(), 0.0);
        t.update(&beats(0.0, 32.0, 0.2), 0.2, 32.0);
        assert!(t.running());
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let texts = |t: &Test| {
            let mut texts = Vec::new();
            for _ in 0..2 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700.0, 700.0),
                    )),
                    ..Default::default()
                };
                let out = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        pane(ui, Some(t), None, "Position", "Pepsi", true);
                    });
                });
                texts = out
                    .shapes
                    .iter()
                    .filter_map(|c| match &c.shape {
                        egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
            }
            texts
        };
        // The first hour filling: its row, no chart yet.
        t.so_far = vec![hour(0, 1588.0)];
        let shown = texts(&t);
        assert!(shown.iter().any(|s| s == "0 h + 26:28"), "{shown:?}");
        assert!(!shown.iter().any(|s| s == "0 h"), "no chart: {shown:?}");
        // Two whole hours and the third filling: three rows, and the
        // charts reach the second hour's point only.
        t.so_far = vec![hour(0, 3600.0), hour(1, 3600.0), hour(2, 1588.0)];
        let shown = texts(&t);
        for row in ["0–1 h", "1–2 h", "2 h + 26:28"] {
            assert!(shown.iter().any(|s| s == row), "{row}: {shown:?}");
        }
        assert_eq!(shown.iter().filter(|s| *s == "0 h").count(), 2, "{shown:?}");
        assert!(!shown.iter().any(|s| s == "3 h"), "{shown:?}");
        assert!(shown[0].starts_with("32 s · "), "{shown:?}");
    }

    /// A run of half a minute charts 0 h to 1 h, each hour named once,
    /// none "-0 h".
    #[test]
    fn a_short_runs_chart_names_each_hour_once() {
        let mut t = Test::new(Position::DialUp, true, String::new(), 0.0);
        t.update(&beats(0.0, 32.0, 0.2), 0.2, 32.0);
        t.stop();
        let mut r = t.finish(|s| s).unwrap().clone();
        r.hours = vec![power_reserve::Hour {
            hour: 0,
            span_s: 32.0,
            rate_s_per_day: Some(7.0),
            amplitude_deg: Some(281.0),
            beat_error_ms: Some(0.4),
            beat_error_source: None,
            caveats: Vec::new(),
            read_s: 30.0,
        }];
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| hour_chart(ui, &r.hours, 110.0, false));
            });
            texts = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>();
        }
        let hours: Vec<_> = texts.iter().filter(|t| t.ends_with(" h")).collect();
        assert!(!hours.is_empty(), "{texts:?}");
        assert!(texts.iter().all(|t| !t.starts_with("-0")), "{texts:?}");
        // Two charts, each naming 0 h and 1 h once.
        for h in ["0 h", "1 h"] {
            assert_eq!(hours.iter().filter(|t| **t == h).count(), 2, "{texts:?}");
        }
        assert_eq!(hours.len(), 4, "{texts:?}");
    }

    /// An 8.5 h run's two charts put each hour at the same x, though the
    /// rate's labels are wider than the amplitude's and the last part hour
    /// has a rate but no amplitude.
    #[test]
    fn the_two_charts_put_each_hour_at_the_same_x() {
        let rate = [3.2, 2.1, 1.4, 0.6, -0.8, -12.3, -44.1, -96.8, -150.5];
        let hours: Vec<_> = (0..9u32)
            .map(|h| power_reserve::Hour {
                hour: h,
                span_s: if h == 8 { 1800.0 } else { 3600.0 },
                rate_s_per_day: Some(rate[h as usize]),
                amplitude_deg: (h < 8).then(|| 292.0 - 9.0 * f64::from(h)),
                beat_error_ms: Some(0.4),
                beat_error_source: None,
                caveats: Vec::new(),
                read_s: 3500.0,
            })
            .collect();
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut texts = Vec::new();
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 600.0),
                )),
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| hour_chart(ui, &hours, 200.0, false));
            });
            texts = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) => Some((
                        t.galley.text().to_string(),
                        t.galley.rect.translate(t.pos.to_vec2()).center().x,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
        }
        for h in 0..=8 {
            let name = format!("{h} h");
            let xs: Vec<f32> = texts.iter().filter(|t| t.0 == name).map(|t| t.1).collect();
            assert_eq!(xs.len(), 2, "{name}: {texts:?}");
            assert!((xs[0] - xs[1]).abs() < 0.5, "{name}: {xs:?}");
        }
    }

    /// A long run's margin left of 0 h, wider than an hour step, marks no
    /// hour before the run.
    #[test]
    fn no_hour_is_marked_before_the_run() {
        let marks = hour_marks(egui_plot::GridInput {
            bounds: (-2.3, 48.8),
            base_step_size: 0.2,
        });
        assert!(!marks.is_empty());
        assert!(marks.iter().all(|m| m.value >= 0.0), "{marks:?}");
        assert_eq!(marks[0].value, 0.0);
    }
}
