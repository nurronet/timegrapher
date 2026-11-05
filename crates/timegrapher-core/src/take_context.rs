//! One measurement context per take (#294): the take opened once, with
//! its settings, clock, breaks and masks, each sound file's place on the
//! take's timeline and each session entry's window, all in take seconds.
//!
//! A take's sound files are read end to end as one recording, so a time
//! in the take is a time in its sound counted from the first file's
//! start. Each entry of its session file names a recording (one of the
//! files, or the take's folder) and gives its stretch in seconds into
//! that recording; here every such stretch is moved by its recording's
//! offset, so every surface reads the same windows, cuts and masks.
//!
//! This first step builds the context; no surface reads it yet.

use crate::breaks::{self, Break};
use crate::clock::ClockFit;
use crate::session::{Manifest, Position, RecordingEntry};
use crate::stream::BeatLog;
use crate::take::{self, ClockFrom, EntrySettings, NoReading, Window};
use std::ops::Range;
use std::path::{Path, PathBuf};

/// One sound file's place on its take's timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct FilePart {
    pub path: PathBuf,
    /// Where it starts in the take and how long it is, seconds.
    pub offset_s: f64,
    pub duration_s: f64,
}

/// The stretch one session entry reads, in take seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryWindow {
    /// Its place in the session file's list, from 0.
    pub entry: usize,
    pub position: Option<Position>,
    /// The take's sound files its recording is: indexes into
    /// [`TakeContext::files`]; empty for a take kept as its analysis alone.
    pub parts: Range<usize>,
    /// Where the recording it names starts in the take, seconds: its
    /// window less this is the window `session` reads in that recording.
    pub offset_s: f64,
    /// [`take::reading_window`] in the recording it names, moved by
    /// `offset_s`; or why it reads nothing.
    pub window: Result<Window, NoReading>,
}

/// What opening a take does with the stretches masked by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Masks {
    /// Cut from the beats, as every reading of a take does.
    Cut,
    /// Kept as data only, the beats read whole: for an editor that
    /// applies them itself and lets them be changed.
    Kept,
}

/// A take, opened once: what every surface reads it from.
pub struct TakeContext {
    /// 0 when opened. Nothing here changes a context once opened; a
    /// holder that changes it (the app editing masks, #294 step 5) bumps
    /// this, and a cache of anything read from it keys on it.
    pub rev: u64,
    /// How the take is read: beat rate, lift, escape teeth, the card's
    /// clock or not, and a settling time given to the reader.
    pub settings: EntrySettings,
    /// The clock the readings are on, as [`take::open_entry`] picks it.
    pub clock: Option<ClockFit>,
    pub clock_from: Option<ClockFrom>,
    pub clock_ppm: Option<f64>,
    pub clock_source: Option<String>,
    pub breaks: Vec<Break>,
    pub unlisted: Option<breaks::Unlisted>,
    /// The take's sound files in the order read; empty for a take kept as
    /// its analysis alone.
    pub files: Vec<FilePart>,
    /// The stretches masked by hand, in take seconds: each entry's masks
    /// moved by its recording's offset.
    pub masked: Vec<(f64, f64)>,
    /// Each session entry's window, in the session file's order.
    pub entries: Vec<EntryWindow>,
}

/// The recording entry `e` names, as a run of `parts`: which they are,
/// where it starts in the take and how long it is. `rate` is the files' sample rate and
/// `frames` each part's length in frames, so lengths come out exactly as
/// the beat log counts them (frames over the rate).
fn placed(
    base: &Path,
    e: &RecordingEntry,
    parts: &[FilePart],
    frames: &[u64],
    rate: u32,
) -> Result<Place, String> {
    let files = take::sound_files(&[base.join(&e.file)])?;
    let first = files
        .first()
        .and_then(|f| parts.iter().position(|p| same_file(&p.path, f)))
        .ok_or_else(|| format!("{}: not one of the take's recordings", e.file))?;
    let run = parts.get(first..first + files.len());
    if run.is_none_or(|run| run.iter().zip(&files).any(|(p, f)| !same_file(&p.path, f))) {
        return Err(format!(
            "{}: its recordings are not in the take's order",
            e.file
        ));
    }
    let before: u64 = frames[..first].iter().sum();
    let own: u64 = frames[first..first + files.len()].iter().sum();
    Ok(Place {
        parts: first..first + files.len(),
        offset_s: before as f64 / rate as f64,
        length_s: own as f64 / rate as f64,
    })
}

/// Whether two paths name one file, however each is spelt (`./a.wav`,
/// a link): as the paths themselves when either can't be resolved.
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// The take's sound files with their places, and each one's length in
/// frames and their sample rate.
fn file_parts(files: &[PathBuf]) -> Result<(Vec<FilePart>, Vec<u64>, u32), String> {
    let mut parts = Vec::new();
    let mut frames = Vec::new();
    let mut rate = None;
    let mut at = 0u64;
    for f in files {
        let i = crate::audio::info(f).map_err(|err| format!("{}: {err}", f.display()))?;
        let n = i
            .frames
            .ok_or_else(|| format!("{}: its header gives no length", f.display()))?;
        if *rate.get_or_insert(i.sample_rate) != i.sample_rate {
            return Err(format!(
                "{}: recorded at {} Hz, unlike the take's other recordings",
                f.display(),
                i.sample_rate
            ));
        }
        let r = i.sample_rate as f64;
        parts.push(FilePart {
            path: f.clone(),
            offset_s: at as f64 / r,
            duration_s: n as f64 / r,
        });
        frames.push(n);
        at += n;
    }
    Ok((parts, frames, rate.unwrap_or(1)))
}

/// A window moved later by `by` seconds.
fn moved(w: Window, by: f64) -> Window {
    Window {
        start_s: w.start_s + by,
        stop_s: w.stop_s + by,
        from_s: w.from_s + by,
        to_s: w.to_s + by,
    }
}

/// Each entry of `m` placed on the take's timeline at `places` (its
/// recording's offset and length): its window there, as `session` reads
/// it in that recording, moved by the offset.
fn entry_windows(m: &Manifest, given_settle_s: Option<f64>, places: &[Place]) -> Vec<EntryWindow> {
    m.recordings
        .iter()
        .zip(places)
        .enumerate()
        .map(|(i, (e, p))| EntryWindow {
            entry: i,
            position: e.position.as_deref().and_then(Position::parse),
            parts: p.parts.clone(),
            offset_s: p.offset_s,
            window: take::reading_window(e, m, given_settle_s, p.length_s)
                .map(|w| moved(w, p.offset_s)),
        })
        .collect()
}

impl TakeContext {
    /// Open the take described by `m` in the folder `base` once, as one
    /// recording over all its sound ([`take::open_entry`]), with each
    /// entry's masks moved to take seconds, and place each entry's window
    /// on it. `masks` says whether the masks are cut from the beats or
    /// only kept as data; [`TakeContext::masked`] holds them either way.
    /// `usb`, `correction` and `progress` are as
    /// [`take::open_entry`] takes them. Gives the context and the take's
    /// beats.
    pub fn open(
        base: &Path,
        m: &Manifest,
        settings: EntrySettings,
        masks: Masks,
        usb: impl FnOnce(&[PathBuf]) -> Result<Option<(f64, String)>, String>,
        correction: impl FnOnce(&Path) -> Result<Option<(f64, String)>, String>,
        progress: impl FnMut(f64, f64),
    ) -> Result<(TakeContext, BeatLog), String> {
        let whole = whole_recording(m)?;
        let kept = crate::analysis_take::is_analysis_only(&base.join(&whole.file));
        let (files, mut places) = if kept {
            (Vec::new(), kept_places(m, f64::INFINITY)?)
        } else {
            layout(base, m, &whole)?
        };
        // The places come from the files' headers, so the masks are put
        // in take seconds before the beats are read.
        let masked: Vec<[f64; 2]> = m
            .recordings
            .iter()
            .zip(&places)
            .flat_map(|(e, p)| {
                let at = p.offset_s;
                e.masked.iter().map(move |[a, b]| [a + at, b + at])
            })
            .collect();
        let entry = RecordingEntry {
            masked: match masks {
                Masks::Cut => masked.clone(),
                Masks::Kept => Vec::new(),
            },
            ..whole
        };
        let opened = take::open_entry(base, m, &entry, &settings, usb, correction, progress)?;
        if kept {
            for p in &mut places {
                p.length_s = opened.log.duration_s;
            }
        }
        let entries = entry_windows(m, settings.settle_s, &places);
        let ctx = TakeContext {
            rev: 0,
            settings,
            clock: opened.clock,
            clock_from: opened.clock_from,
            clock_ppm: opened.clock_ppm,
            clock_source: opened.clock_source,
            breaks: opened.breaks,
            unlisted: opened.unlisted,
            files,
            masked: masked.iter().map(|&[a, b]| (a, b)).collect(),
            entries,
        };
        Ok((ctx, opened.log))
    }
}

/// The one recording a take is opened as: [`take::whole_take`]'s, which
/// may hold every entry's masks only while they all name one recording;
/// here the masks are placed in take seconds by the caller.
fn whole_recording(m: &Manifest) -> Result<RecordingEntry, String> {
    let unmasked = Manifest {
        recordings: m
            .recordings
            .iter()
            .map(|e| RecordingEntry {
                masked: Vec::new(),
                ..e.clone()
            })
            .collect(),
        ..m.clone()
    };
    take::whole_take(&unmasked)
}

/// An entry of a take kept as its analysis alone, which is one recording:
/// at the take's start, the take's length long. An entry naming one of
/// the files it was made from can't be placed.
fn kept_place(e: &RecordingEntry, length_s: f64) -> Result<Place, String> {
    match e.file.as_str() {
        "." | "" | "./" => Ok(Place {
            parts: 0..0,
            offset_s: 0.0,
            length_s,
        }),
        f => Err(format!(
            "{f}: the take is kept as its analysis alone, so its entries read it whole"
        )),
    }
}

/// Every entry of a take kept as its analysis alone, `length_s` long.
fn kept_places(m: &Manifest, length_s: f64) -> Result<Vec<Place>, String> {
    m.recordings
        .iter()
        .map(|e| kept_place(e, length_s))
        .collect()
}

/// The session entries of a take, laid on its timeline as [`TakeContext`]
/// lays them, from the sound files' headers alone (or, for a take kept as
/// its analysis alone, its kept length): for a reader that needs the
/// windows before (or without) finding the beats.
pub fn entry_layout(
    base: &Path,
    m: &Manifest,
    given_settle_s: Option<f64>,
) -> Result<(Vec<FilePart>, Vec<EntryWindow>), String> {
    let whole = whole_recording(m)?;
    let path = base.join(&whole.file);
    let (files, places) = if crate::analysis_take::is_analysis_only(&path) {
        let length_s = crate::analysis_take::read(&path)
            .map_err(|err| format!("{}: {err}", path.display()))?
            .log
            .duration_s;
        (Vec::new(), kept_places(m, length_s)?)
    } else {
        layout(base, m, &whole)?
    };
    Ok((files, entry_windows(m, given_settle_s, &places)))
}

/// An entry's recording on the take's timeline: the sound files it is,
/// where it starts in the take and its length, seconds.
struct Place {
    parts: Range<usize>,
    offset_s: f64,
    length_s: f64,
}

/// The stretch `a` to `b` of the take, in take seconds, as it lies in
/// each sound file it crosses: the file's index in `files` and the
/// stretch in that file's own seconds, split where one file ends and the
/// next begins. What a mask drawn on the take is saved as.
pub fn in_files(files: &[FilePart], a: f64, b: f64) -> Vec<(usize, f64, f64)> {
    files
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            let from = a.max(f.offset_s);
            let to = b.min(f.offset_s + f.duration_s);
            (from < to).then_some((i, from - f.offset_s, to - f.offset_s))
        })
        .collect()
}

/// The take's sound files, as `whole` reads them, with each entry's
/// place among them (its recording's offset and length).
fn layout(
    base: &Path,
    m: &Manifest,
    whole: &RecordingEntry,
) -> Result<(Vec<FilePart>, Vec<Place>), String> {
    let (files, frames, rate) = file_parts(&take::sound_files(&[base.join(&whole.file)])?)?;
    let places = m
        .recordings
        .iter()
        .map(|e| placed(base, e, &files, &frames, rate))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((files, places))
}
