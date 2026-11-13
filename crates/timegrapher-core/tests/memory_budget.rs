//! A long run's memory stays inside a stated budget (#483).
//!
//! A run of days can't be played in a test, so this one plays ten minutes
//! of a synthetic watch through the live engine and checks that each of
//! its stores grows no faster than its rule allows. It then works out
//! what each store holds after [`RUN_S`] by that rule and checks that the
//! total stays under [`BUDGET_BYTES`], at each of [`RATES`]. Memory is
//! counted as each list's capacity, not its length. The stores are the beats (capped),
//! the amplitude windows (one a window step), the tick and tock sound
//! kept for the Tick Tock Profile (thinned by core's `thin_sound`), and
//! the capture queue (bounded at `QUEUE_BLOCKS`).
//!
//! One store is transient and not in the total: the sound live gathers
//! from when a watch is first heard until it holds a file's first chunk
//! (47 s), to build its beat template again as a file's reading does
//! (#1327), about 9 MB at 48 kHz and 18 MB at 96 kHz. It is let go once
//! the template is built and that sound read again, which the test checks.

use std::mem::size_of;
use timegrapher_core::amplitude::AmplitudeWindow;
use timegrapher_core::analysis_take::PROFILE_EVERY_S;
use timegrapher_core::beats::Beat;
use timegrapher_core::capture::QUEUE_BLOCKS;
use timegrapher_core::live::{LiveAnalyzer, LiveConfig};
use timegrapher_core::profile::{thin_sound, TickProfile};
use timegrapher_core::sound_sample::SOUND_SAMPLE_BEATS;
use timegrapher_core::synth::{generate, SynthConfig};

/// The run the budget is for: an 80 h Power Reserve test.
const RUN_S: f64 = 80.0 * 3600.0;
/// What the stores above may hold together after [`RUN_S`].
const BUDGET_BYTES: f64 = 160e6;
/// The input rates the budget holds at (decided on #1115).
const RATES: [u32; 2] = [48_000, 96_000];
/// The shortest amplitude window step the test allows: the budget counts
/// windows at this step, the most windows the rule allows.
const WINDOW_STEP_S: f64 = 1.9;
/// Ten minutes of sound, played in 10 ms blocks as inputs commonly send.
const PLAYED_S: f64 = 600.0;
const BLOCK_S: f64 = 0.01;

/// The bytes one tick or tock profile's curves hold on the heap: an entry
/// the history drops frees them, so only kept entries count them.
fn curve_bytes(p: &TickProfile) -> usize {
    4 * (p.median.len() + p.p10.len() + p.p90.len())
}

/// One slot of the app's sound history: the time, the span and both
/// sides' profiles (the app's `(f64, Kept)`), inline in the list.
type SoundSlot = (f64, f64, [Option<TickProfile>; 2]);

#[test]
fn a_multi_day_run_stays_inside_its_memory_budget() {
    for rate in RATES {
        let (total, text) = run_at(rate);
        assert!(
            (total as f64) < BUDGET_BYTES,
            "{:.1} MB after {} h at {rate} Hz, over {:.0} MB: {text}",
            total as f64 / 1e6,
            RUN_S / 3600.0,
            BUDGET_BYTES / 1e6,
        );
        eprintln!("after {} h at {rate} Hz: {text}", RUN_S / 3600.0);
    }
}

/// Play [`PLAYED_S`] at `rate` through the live engine, check each store
/// grows only as its rule allows, and give the bytes the stores hold
/// after [`RUN_S`] by those rules, with a line naming each.
fn run_at(rate: u32) -> (usize, String) {
    let synth = SynthConfig {
        duration_s: PLAYED_S,
        sample_rate: rate,
        ..Default::default()
    };
    let audio = generate(&synth, |_| 280.0, |_| 0.0);
    let beats_per_s = synth.bph as f64 / 3600.0;
    // A cap the ten minutes reach, so the test sees it hold.
    let cap = (beats_per_s * PLAYED_S / 2.0) as usize;
    let cfg = LiveConfig {
        max_beats: cap,
        ..Default::default()
    };
    let mut live = LiveAnalyzer::new(rate, cfg);
    let block = (BLOCK_S * rate as f64).round() as usize;
    for chunk in audio.samples.chunks(block) {
        live.push(chunk);
        // Beats never pass the cap, at any moment.
        assert!(live.beats().len() <= cap, "{} > {cap}", live.beats().len());
    }

    // Each store as the rule allows. A steady watch has no breaks or jumps,
    // and candidates wait only for the next pass. The sound gathered to
    // build the template again is let go once it is built.
    assert_eq!(live.beats().len(), cap);
    assert!(
        live.template_switch_s().is_some(),
        "the template built again"
    );
    assert!(
        !live.gathering(),
        "the sound gathered for the template is kept"
    );
    assert!(live.breaks().is_empty() && live.by_count().is_empty() && live.jumps().is_empty());
    assert!(live.candidates().len() < 4 * beats_per_s as usize);
    // The beat sound the verdict reads, sampled across the take within
    // its budget.
    let sample = live.snippets();
    assert!(sample.len() <= SOUND_SAMPLE_BEATS, "{}", sample.len());
    let window_bytes = sample.first().map_or(0, |s| s.1.len() * size_of::<f32>());
    let window_every_s = PLAYED_S / live.amplitude_windows().len() as f64;
    assert!(
        window_every_s >= WINDOW_STEP_S,
        "a window every {window_every_s} s"
    );

    // The tick and tock sound kept every PROFILE_EVERY_S over the run,
    // thinned as it grows, as the app keeps it.
    let sides = live.tick_profiles();
    assert!(sides.iter().all(Option::is_some), "no profile to measure");
    let curves: usize = sides.iter().flatten().map(curve_bytes).sum();
    let mut history: Vec<(f64, ())> = Vec::new();
    let mut peak = 0;
    let mut t = 0.0;
    while t <= RUN_S {
        history.push((t, ()));
        peak = peak.max(history.len());
        thin_sound(&mut history);
        t += PROFILE_EVERY_S;
    }

    // Memory is a list's capacity, not its length: a list grown by push
    // doubles, and trimming or thinning it keeps its slots, though not
    // the curves of the entries it drops (#1115).
    let held = |len: usize| len.next_power_of_two();
    let stores = [
        (
            "beats",
            held(LiveConfig::default().max_beats + 1) * size_of::<Beat>(),
        ),
        (
            "amplitude windows",
            held((RUN_S / WINDOW_STEP_S).ceil() as usize) * size_of::<AmplitudeWindow>(),
        ),
        (
            "tick and tock sound",
            held(peak) * size_of::<SoundSlot>() + peak * curves,
        ),
        ("capture queue", QUEUE_BLOCKS * block * size_of::<f32>()),
        (
            "beat sound sample",
            held(SOUND_SAMPLE_BEATS + 1) * size_of::<(f64, bool, Vec<f32>)>()
                + SOUND_SAMPLE_BEATS * window_bytes,
        ),
    ];
    let text: Vec<String> = stores
        .iter()
        .map(|(name, b)| format!("{name} {:.1} MB", *b as f64 / 1e6))
        .collect();
    (stores.iter().map(|s| s.1).sum(), text.join(", "))
}
