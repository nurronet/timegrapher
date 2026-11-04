//! The readings of a whole take that rest on its masks, clock and
//! settings: the whole take beside the cards, the Positions Test table
//! and the charts' run of readings. Each is read at once where a caller
//! needs it now, and in the background for the window, so that a mask
//! on a take of many hours never holds the window while they are read
//! again (#978).

use super::*;

/// What [`TimegrapherApp::whole_take`] was read from: the beats heard,
/// the stretches, the masks and what the readings rest on.
pub(super) type WholeKey = (usize, Vec<(f64, f64)>, Vec<(f64, f64)>, Revision);

/// What [`TimegrapherApp::free_table`] was read from: the beats heard,
/// each position's settled stretches, the masks and the True Clock.
pub(super) type FreeKey = (
    usize,
    Vec<(core_session::Position, f64, f64)>,
    Vec<(f64, f64)>,
    Revision,
);

/// What [`TimegrapherApp::trend`] was read from: what the readings rest
/// on and Average Over.
pub(super) type TrendKey = (Revision, u64);

/// How often the Positions Test table is read again while listening.
const FREE_TABLE_STEP_S: f64 = 1.0;

/// A reading under way on a thread of its own, and what it is read from.
pub(super) struct Job<K, T> {
    pub(super) key: K,
    rx: std::sync::mpsc::Receiver<T>,
}

impl<K, T: Send + 'static> Job<K, T> {
    /// Run `read` on a thread of its own; the window is drawn again when
    /// it is done.
    fn start(key: K, ctx: &egui::Context, read: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(read());
            ctx.request_repaint();
        });
        Job { key, rx }
    }

    /// What it read, once it is done; `Err(true)` while it runs, and
    /// `Err(false)` when it ended with nothing.
    fn done(&self) -> Result<T, bool> {
        match self.rx.try_recv() {
            Ok(t) => Ok(t),
            Err(TryRecvError::Empty) => Err(true),
            Err(TryRecvError::Disconnected) => Err(false),
        }
    }
}

/// A Positions Test table being read in the background, and what from.
pub(super) type FreeJob = Job<FreeKey, Option<guided::FreeTable>>;

/// The whole take in one position, and the seconds it covers.
#[derive(Clone)]
pub(super) struct WholeTake {
    pub(super) m: timegrapher_core::session::Measurement,
    pub(super) span: f64,
}

/// A reading of the whole take, ready to run here or on a thread of its
/// own.
type WholeRead = Box<dyn FnOnce() -> Option<WholeTake> + Send>;

impl TimegrapherApp {
    /// The take in the position the watch is in now, once it is stopped:
    /// its settled stretches, at whatever length session reads, read as
    /// `timegrapher session` reads a recording (core
    /// `session::measure_masked`), the time between them masked, and the
    /// seconds they cover. A take opened from disk is read to its end, as
    /// session reads it; one just listened to, to its last reading with a
    /// watch heard. Read here and now when it is not kept, as the tests
    /// read it; the window reads it with [`Self::whole_take_shown`].
    #[cfg(test)]
    pub(super) fn whole_take(&self) -> Option<WholeTake> {
        match self.whole_read()? {
            Ok(w) => w,
            Err((key, read)) => {
                let w = read();
                *self.whole_take.borrow_mut() = Some((key, w.clone()));
                w
            }
        }
    }

    /// [`Self::whole_take`] for the window: when it is not kept, read in
    /// the background and none until it comes.
    pub(super) fn whole_take_shown(&self, ctx: &egui::Context) -> Option<WholeTake> {
        // A take just listened to is read to its last reading with a
        // watch heard: once the charts are read on what is masked now.
        let opened = self.take_breaks.is_some();
        if !opened && self.trend_key != (self.rev, self.average_s.to_bits()) {
            return None;
        }
        let (key, read) = match self.whole_read()? {
            Ok(w) => return w,
            Err(r) => r,
        };
        let mut job = self.whole_job.borrow_mut();
        if job.as_ref().is_none_or(|j| j.key != key) {
            *job = Some(Job::start(key, ctx, read));
            return None;
        }
        let done = job.as_ref().map(Job::done)?;
        match done {
            Ok(w) => {
                let key = job.take().map(|j| j.key)?;
                *self.whole_take.borrow_mut() = Some((key, w.clone()));
                w
            }
            Err(true) => None,
            Err(false) => {
                *job = None;
                None
            }
        }
    }

    /// The whole take as kept (`Ok`), or the key and the reading that
    /// gives it (`Err`); none while listening or with nothing to read.
    fn whole_read(&self) -> Option<Result<Option<WholeTake>, (WholeKey, WholeRead)>> {
        if self.running() {
            return None;
        }
        let live = self.live.as_ref()?;
        // To the take's end, as session reads it, not its last beat.
        let opened = self.take_breaks.is_some();
        let end = if opened {
            live.duration_s()
        } else {
            self.last_heard_s().min(live.duration_s())
        };
        let spans: Vec<(f64, f64)> = self
            .current_spans()
            .into_iter()
            .map(|(a, b)| (a.max(0.0), b.min(end)))
            .filter(|(a, b)| a < b)
            .collect();
        let key: WholeKey = (
            live.beats().len(),
            spans.clone(),
            self.masked.clone(),
            self.rev,
        );
        if let Some((k, w)) = self.whole_take.borrow().as_ref() {
            if *k == key {
                return Some(Ok(w.clone()));
            }
        }
        let log = steady::log_of(live);
        let masks = self.masked.clone();
        let (take_clock, take_breaks, ppm) = (
            self.take_clock.clone(),
            self.take_breaks.clone(),
            self.clock.as_ref().map(|c| c.ppm),
        );
        let (lift_known, unlisted) = (self.lift_known(), self.take_unlisted);
        let read = move || {
            let span: f64 = spans.iter().map(|(a, b)| b - a).sum();
            let (&(from, _), &(_, to)) = (spans.first()?, spans.last()?);
            let mut log = log?;
            let clock = reading_clock(take_clock.as_ref(), opened, ppm, log.duration_s);
            // The cuts the app adds (a settling's end, a move, a mask's ends)
            // at the reading's or a mask's edges change nothing it reads, and
            // session has none there (#682).
            let edge = |j: f64| j <= from || j >= to || masks.iter().any(|m| j == m.0 || j == m.1);
            log.jumps.retain(|&j| !edge(j));
            let breaks = steady::reading_breaks(take_breaks.unwrap_or_default(), &log);
            // The time between the stretches, another position or its
            // settling, is left out as a mask is.
            let mut masked = masks;
            masked.extend(spans.windows(2).map(|w| (w[0].1, w[1].0)));
            let mut m = timegrapher_core::session::measure_masked(
                &log,
                clock.as_ref(),
                &breaks,
                &masked,
                from,
                to,
            );
            m.take_doubts(lift_known, unlisted);
            Some(WholeTake { m, span })
        };
        Some(Err((key, Box::new(read))))
    }

    /// The Positions Test table of the take on show or under way, when no
    /// guided test is: each position's settled stretches read by core
    /// `session::positions_table`. While listening it is read afresh every
    /// second, in the background, the last table showing until the next is
    /// ready; a take on show is read at once, as the tests read it. The
    /// window reads it with [`Self::free_table_shown`].
    #[cfg(test)]
    pub(super) fn free_table(&self, ctx: &egui::Context) -> Option<guided::FreeTable> {
        self.table_of(ctx, self.free_stretches(), false)
    }

    /// [`Self::free_table`] for the window: a take on show's is read in
    /// the background when it is not kept, and none until it comes.
    pub(super) fn free_table_shown(&self, ctx: &egui::Context) -> Option<guided::FreeTable> {
        self.table_of(ctx, self.free_stretches(), true)
    }

    /// Whether a Positions Test table of the take on show is being read
    /// in the background.
    pub(super) fn free_table_reading(&self) -> bool {
        !self.running() && self.free_job.borrow().is_some()
    }

    /// Each position's settled stretches, in the table's order.
    fn free_stretches(&self) -> Vec<(core_session::Position, f64, f64)> {
        guided::ORDER
            .iter()
            .filter_map(|&p| Some((p, core_session::Position::parse(p.code())?)))
            .flat_map(|(p, sp)| {
                self.session
                    .settled_spans(self.position, p)
                    .into_iter()
                    .map(move |(a, b)| (sp, a, b))
            })
            .collect()
    }

    /// The guided test's figures for the position being measured, so far:
    /// its measuring up to now read as [`Self::free_table`] reads a
    /// position, every second. None until it has 10 s.
    pub(super) fn guided_live(&self, ctx: &egui::Context) -> Option<guided::FreeCell> {
        let t = self.guided.as_ref()?;
        let now = self.live.as_ref()?.duration_s();
        let &(from, to) = t.measured(now).get(t.stretches().len())?;
        let p = core_session::Position::parse(t.position().code())?;
        let table = self.table_of(ctx, vec![(p, from, to)], false)?;
        let label = guided::position_label(p);
        let row = table.rows.into_iter().find(|r| r.0 == label)?;
        matches!(row.2, guided::FreeCell::Read(..)).then_some(row.2)
    }

    /// A Positions Test table of `stretches` (position, start, end), each
    /// cut at the take's end so far: read afresh every second, in the
    /// background, while listening; for a take on show at once, or in the
    /// background with none until it comes when `later`.
    fn table_of(
        &self,
        ctx: &egui::Context,
        stretches: Vec<(core_session::Position, f64, f64)>,
        later: bool,
    ) -> Option<guided::FreeTable> {
        let live = self.live.as_ref()?;
        let latest = self.latest_s();
        let running = self.running();
        // A take on show reads to the end of its sound, as `timegrapher
        // session` reads its last position.
        let end = if running {
            (latest / FREE_TABLE_STEP_S).floor() * FREE_TABLE_STEP_S
        } else {
            latest.max(live.duration_s())
        };
        let stretches: Vec<(core_session::Position, f64, f64)> = stretches
            .into_iter()
            .map(|(p, a, b)| (p, a.max(0.0), b.min(end)))
            .filter(|s| s.1 < s.2)
            .collect();
        let masked = core_session::masked_spans(
            &self.masked.iter().map(|&(a, b)| [a, b]).collect::<Vec<_>>(),
        );
        let beats = if running {
            (end / FREE_TABLE_STEP_S) as usize
        } else {
            live.beats().len()
        };
        let key: FreeKey = (beats, stretches.clone(), masked.clone(), self.rev);
        let mut job = self.free_job.borrow_mut();
        // A table being read on what the readings rested on before is let
        // go; so is one of a take on show read from anything else.
        if job
            .as_ref()
            .is_some_and(|j| j.key.3 != self.rev || !running && j.key != key)
        {
            *job = None;
        }
        if let Some(done) = job.as_ref().map(Job::done) {
            match done {
                Ok(t) => {
                    let key = job.take().map(|j| j.key)?;
                    *self.free_table.borrow_mut() = Some((key, t));
                }
                Err(running) => {
                    if !running {
                        *job = None;
                    }
                }
            }
        }
        if let Some((k, t)) = self.free_table.borrow().as_ref() {
            if *k == key || running && job.is_some() {
                return t.clone();
            }
        }
        if !running && job.is_some() {
            return None;
        }
        let (take_clock, opened, ppm) = (
            self.take_clock.clone(),
            self.take_breaks.clone(),
            self.clock.as_ref().map(|c| c.ppm),
        );
        let read = move |log: BeatLog| {
            let clock = reading_clock(take_clock.as_ref(), opened.is_some(), ppm, log.duration_s);
            let breaks = steady::reading_breaks(opened.unwrap_or_default(), &log);
            let t = core_session::positions_table(
                &log,
                clock.as_ref(),
                &breaks,
                &masked,
                &stretches,
                core_session::POSITION_MIN_S,
            );
            guided::free_table(&t)
        };
        let log = steady::log_of(live);
        let shown = self.free_table.borrow().as_ref().map(|(_, t)| t.clone());
        match (running, shown, log) {
            // Listening, with a table on show: read the next one in the
            // background and keep this one until it comes.
            (true, Some(shown), Some(log)) => {
                *job = Some(Job::start(key, ctx, move || Some(read(log))));
                shown
            }
            // A take on show, for the window: none until it comes.
            (false, _, Some(log)) if later => {
                *job = Some(Job::start(key, ctx, move || Some(read(log))));
                None
            }
            (_, _, log) => {
                let t = log.map(read);
                *self.free_table.borrow_mut() = Some((key, t.clone()));
                t
            }
        }
    }

    /// For the window: the charts' run of readings read again when what
    /// it rests on changed. While listening, here and now
    /// ([`Self::refresh_trend`]); for a take on show, in the background,
    /// the run on show kept until the new one comes.
    pub(super) fn refresh_trend_shown(&mut self, ctx: &egui::Context) {
        let key: TrendKey = (self.rev, self.average_s.to_bits());
        if self.running() || self.live.is_none() {
            self.trend_job = None;
            self.refresh_trend();
            return;
        }
        if self.trend_key == key {
            self.trend_job = None;
            return;
        }
        if self.trend_job.as_ref().is_some_and(|j| j.key != key) {
            self.trend_job = None;
        }
        let Some(job) = &self.trend_job else {
            let (live, average_s) = (self.live.clone(), self.average_s);
            self.trend_job = Some(Job::start(key, ctx, move || {
                live.map_or_else(Vec::new, |l| l.trend(average_s))
            }));
            return;
        };
        match job.done() {
            Ok(points) => {
                self.trend_job = None;
                self.put_trend(points);
            }
            Err(true) => {}
            Err(false) => self.trend_job = None,
        }
    }
}
