//! A long run's memory stays inside a stated budget (#483).
//!
//! A run of days can't be played in a test, so this one plays ten minutes
//! of a synthetic watch through the live engine and checks that each of
//! its stores grows no faster than its rule allows. It then works out
//! what each store holds after [`RUN_S`] by that rule and checks that the
//! total stays under [`BUDGET_BYTES`]. The stores are the beats (capped),
//! the amplitude windows (one a window step), the tick and tock sound
//! kept for the Tick Tock Profile (thinned by core's `thin_sound`), and
//! the capture queue (bounded at `QUEUE_BLOCKS`).

use std::mem::size_of;
use timegrapher_core::amplitude::AmplitudeWindow;
use timegrapher_core::analysis_take::PROFILE_EVERY_S;
use timegrapher_core::beats::Beat;
use timegrapher_core::capture::QUEUE_BLOCKS;
use timegrapher_core::live::{LiveAnalyzer, LiveConfig};
use timegrapher_core::profile::{thin_sound, TickProfile};
use timegrapher_core::synth::{generate, SynthConfig};

/// The run the budget is for: an 80 h Power Reserve test.
const RUN_S: f64 = 80.0 * 3600.0;
/// What the stores above may hold together after [`RUN_S`].
const BUDGET_BYTES: f64 = 128e6;
/// Ten minutes of sound, played in 10 ms blocks as inputs commonly send.
const PLAYED_S: f64 = 600.0;
const BLOCK_S: f64 = 0.01;

/// The bytes one tick or tock profile holds: its fields and its curves.
fn profile_bytes(p: &TickProfile) -> usize {
    size_of::<TickProfile>() + 4 * (p.median.len() + p.p10.len() + p.p90.len())
}

#[test]
fn a_multi_day_run_stays_inside_its_memory_budget() {
    let synth = SynthConfig {
        duration_s: PLAYED_S,
        ..Default::default()
    };
    let audio = generate(&synth, |_| 280.0, |_| 0.0);
    let fs = audio.sample_rate as f64;
    let beats_per_s = synth.bph as f64 / 3600.0;
    // A cap the ten minutes reach, so the test sees it hold.
    let cap = (beats_per_s * PLAYED_S / 2.0) as usize;
    let cfg = LiveConfig {
        max_beats: cap,
        ..Default::default()
    };
    let max_beats = LiveConfig::default().max_beats;
    let mut live = LiveAnalyzer::new(audio.sample_rate, cfg);
    let block = (BLOCK_S * fs).round() as usize;
    for chunk in audio.samples.chunks(block) {
        live.push(chunk);
        // Beats never pass the cap, at any moment.
        assert!(live.beats().len() <= cap, "{} > {cap}", live.beats().len());
    }

    // Each store as the rule allows. A steady watch has no breaks or jumps,
    // and candidates wait only for the next pass.
    assert_eq!(live.beats().len(), cap);
    assert!(live.breaks().is_empty() && live.by_count().is_empty() && live.jumps().is_empty());
    assert!(live.candidates().len() < 4 * beats_per_s as usize);
    let windows = live.amplitude_windows().len();
    let window_every_s = PLAYED_S / windows as f64;
    assert!(window_every_s >= 1.9, "a window every {window_every_s} s");

    // The tick and tock sound kept every PROFILE_EVERY_S over the run,
    // thinned as it grows, as the app keeps it.
    let sides = live.tick_profiles();
    let entry_bytes: usize = sides.iter().flatten().map(profile_bytes).sum();
    assert!(sides.iter().all(Option::is_some), "no profile to measure");
    let mut history: Vec<(f64, ())> = Vec::new();
    let mut t = 0.0;
    while t <= RUN_S {
        history.push((t, ()));
        thin_sound(&mut history);
        t += PROFILE_EVERY_S;
    }

    let stores = [
        ("beats", max_beats * size_of::<Beat>()),
        (
            "amplitude windows",
            (RUN_S / window_every_s).ceil() as usize * size_of::<AmplitudeWindow>(),
        ),
        ("tick and tock sound", history.len() * entry_bytes),
        (
            "capture queue",
            QUEUE_BLOCKS * (BLOCK_S * 48_000.0) as usize * size_of::<f32>(),
        ),
    ];
    let total: usize = stores.iter().map(|s| s.1).sum();
    let text: Vec<String> = stores
        .iter()
        .map(|(name, b)| format!("{name} {:.1} MB", *b as f64 / 1e6))
        .collect();
    assert!(
        (total as f64) < BUDGET_BYTES,
        "{:.1} MB after {} h, over {:.0} MB: {}",
        total as f64 / 1e6,
        RUN_S / 3600.0,
        BUDGET_BYTES / 1e6,
        text.join(", ")
    );
    eprintln!("after {} h: {}", RUN_S / 3600.0, text.join(", "));
}
