//! Live builds its beat template again as a file's reading does (#1327):
//! from the sound of the first chunk, which it then reads again with it.

use super::*;

/// Where live is in building its template again ([`LiveAnalyzer::rebuild_template`]).
#[derive(Debug, Clone, Default)]
pub(super) struct Rebuild {
    /// The sound from the window the watch was first heard in, and the
    /// sample it starts at, gathered until it holds a file's first chunk.
    /// None before the watch is heard and once built.
    gather: Option<(u64, Vec<f32>)>,
    /// Whether the template has been built again since the watch was
    /// first heard (or that was given up).
    done: bool,
    /// When the sound read again took over, seconds.
    switch_s: Option<f64>,
    /// The gathered sound being read again with the template built from
    /// it, a little faster than it comes in, until it catches up.
    shadow: Option<Box<Shadow>>,
}

/// The sound gathered read again: a fresh analyzer and the sound it has
/// still to read.
#[derive(Debug, Clone)]
struct Shadow {
    live: LiveAnalyzer,
    pending: Vec<f32>,
    at: usize,
}

/// How much faster than it comes in the gathered sound is read again: 47 s
/// is caught up about 12 s after the template is built, at most this many
/// times a push's work in any one push, so the screen never waits on all
/// of it at once.
const CATCH_UP: usize = 4;

impl LiveAnalyzer {
    /// When the template built again from a file's first chunk of sound,
    /// and that sound read again with it, took over (#1327), seconds; None
    /// until then.
    pub fn template_switch_s(&self) -> Option<f64> {
        self.rebuild.switch_s
    }

    /// Whether sound is held to build the template again or being read
    /// again with it.
    pub fn gathering(&self) -> bool {
        self.rebuild.gather.is_some() || self.rebuild.shadow.is_some()
    }

    /// Start again: the next watch heard has its template built again.
    pub(super) fn forget_rebuild(&mut self) {
        self.rebuild = Rebuild::default();
    }

    /// Stop building the template again: what the caller changed (a new
    /// stretch, a pause, its own breaks) has no place in the sound
    /// gathered. The template stays as first built.
    pub(super) fn give_up_rebuild(&mut self) {
        self.rebuild.gather = None;
        self.rebuild.shadow = None;
        self.rebuild.done = true;
    }

    /// The same change made to the analyzer reading the sound again.
    pub(super) fn in_shadow(&mut self, f: impl FnOnce(&mut LiveAnalyzer)) {
        if let Some(sh) = &mut self.rebuild.shadow {
            f(&mut sh.live);
        }
    }

    /// After a push of `samples`, `passed` when a pass ran: gathers them,
    /// builds the template again once a chunk is gathered, and reads the
    /// sound again a little further. True when the readings may have
    /// changed.
    pub(super) fn rebuild_after_push(&mut self, samples: &[f32], passed: bool) -> bool {
        if let Some((_, g)) = &mut self.rebuild.gather {
            g.extend_from_slice(samples);
        }
        if let Some(sh) = &mut self.rebuild.shadow {
            sh.pending.extend_from_slice(samples);
            self.catch_up(samples.len());
            return true;
        }
        if passed {
            self.rebuild_template();
        }
        false
    }

    /// Gathers the sound from the window the watch was first heard in
    /// until it holds a file's first chunk, then builds the template again
    /// from it as a file's reading does ([`crate::stream::first_beats_in`])
    /// and reads that sound again with it ([`Self::reread`]), so live and
    /// a file read the beats with the same template from the start, with
    /// no cut between two (#1327). The sound gathered is let go. Only a
    /// watch first heard with no beats yet: one heard again after a long
    /// silence keeps its template, as a file's reading keeps its first.
    fn rebuild_template(&mut self) {
        let Some(bph) = self
            .bph
            .filter(|_| self.template.is_some() && !self.rebuild.done)
        else {
            return;
        };
        if self.rebuild.gather.is_none() {
            let win = ((self.cfg.window_s * self.fs).ceil() as usize).min(self.buf.len());
            let from = self.buf.len() - win;
            let g0 = self.buf0 + from as u64;
            if self
                .beats
                .first()
                .is_some_and(|b| b.time < g0 as f64 / self.fs)
            {
                self.rebuild.done = true;
                return;
            }
            self.rebuild.gather = Some((g0, self.buf[from..].to_vec()));
        }
        let chunk = (crate::stream::StreamConfig::default().chunk_s * self.fs).round() as usize;
        if self
            .rebuild
            .gather
            .as_ref()
            .is_some_and(|(_, g)| g.len() < chunk)
        {
            return;
        }
        let Some((g0, g)) = self.rebuild.gather.take() else {
            return;
        };
        self.rebuild.done = true;
        let a = &self.cfg.analysis;
        let env = envelope(&g[..chunk], self.fs, &a.envelope);
        let Some(t) = crate::stream::first_beats_in(&env, self.fs, Some(bph), a.amplitude_window_s)
            .and_then(|f| f.template)
        else {
            return;
        };
        drop(env);
        self.reread(g0, g, t);
    }

    /// Starts reading the sound `g`, from sample `g0` to now, again with
    /// `template` from a fresh start, as a file reads its first chunk: a
    /// copy of this analyzer as it was at `g0`, with the masks kept, finds
    /// the beats, amplitude windows, profiles and cuts since `g0` again
    /// through the same passes ([`Self::catch_up`]).
    fn reread(&mut self, g0: u64, g: Vec<f32>, template: Vec<f32>) {
        let mut l = self.clone();
        let since = g0 as f64 / self.fs;
        let before = |v: &[f64]| -> Vec<f64> { v.iter().copied().filter(|&t| t < since).collect() };
        let (breaks, jumps, by_count) = (
            before(&self.breaks),
            before(&self.intrinsic_breaks),
            before(&self.by_count),
        );
        l.restart();
        (l.breaks, l.intrinsic_breaks, l.by_count) = (breaks, jumps, by_count);
        (l.bph, l.found_bph, l.logged_bph) = (self.bph, self.found_bph, self.logged_bph);
        l.template = Some(template);
        l.rebuild.done = true;
        l.buf.clear();
        (l.buf0, l.total, l.next_pass) = (g0, g0, g0);
        l.settled_to = since;
        self.rebuild.shadow = Some(Box::new(Shadow {
            live: l,
            pending: g,
            at: 0,
        }));
    }

    /// Reads up to [`CATCH_UP`] times `n` samples more of the sound being
    /// read again, and once it has caught up, takes its reading over: the
    /// first chunk's readings change once, then.
    fn catch_up(&mut self, n: usize) {
        let step = (self.cfg.update_s * self.fs).round().max(1.0) as usize;
        let Some(sh) = self.rebuild.shadow.as_mut() else {
            return;
        };
        let to = (sh.at + CATCH_UP * n).min(sh.pending.len());
        while sh.at < to {
            let end = (sh.at + step).min(to);
            sh.live.push(&sh.pending[sh.at..end]);
            sh.at = end;
        }
        if sh.at < sh.pending.len() {
            // Let go of what has been read, now and then.
            if sh.at > sh.pending.len() / 2 {
                sh.pending.drain(..sh.at);
                sh.at = 0;
            }
            return;
        }
        let Some(sh) = self.rebuild.shadow.take() else {
            return;
        };
        let mut l = sh.live;
        l.rebuild.switch_s = Some(self.duration_s());
        *self = l;
    }
}
