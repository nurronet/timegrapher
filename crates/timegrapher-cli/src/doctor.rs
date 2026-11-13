//! `timegrapher doctor`: what the microphone input measures.
//!
//! It records a few seconds (or reads a file), measures the level,
//! clipping, the background between ticks, the signal above noise and
//! whether a regular beat is found, reads the sound card's mixer where it
//! can, and lists the mixer changes `--apply` would make. It changes
//! nothing on the computer unless `--apply` is given.

use crate::output;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use timegrapher_core::audio::{self, Audio};
use timegrapher_core::diagnose::{self, DiagnoseConfig, SignalCheck};
use timegrapher_core::figure_text::figure;
use timegrapher_core::library::{base_digits, fixed_text, signed_text};
use timegrapher_core::mixer::{self, Fix, Mixer};

pub struct Options {
    pub device: Option<String>,
    pub card: Option<String>,
    pub seconds: f64,
    pub file: Option<PathBuf>,
    pub save: Option<PathBuf>,
    pub apply: bool,
    pub json: bool,
    pub bph: Option<u32>,
}

#[derive(Serialize)]
struct Report {
    /// Where the audio came from: a device (with its configuration) or a file.
    source: Value,
    check: SignalCheck,
    /// The sound card's mixer controls (Linux), when they could be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    mixer: Option<Mixer>,
    /// Proposed changes; none are made without `--apply`.
    fixes: Vec<Fix>,
    /// True when `--apply` ran the fixes' commands.
    applied: bool,
    /// The check repeated after applying, when recording live.
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<SignalCheck>,
}

pub fn run(o: &Options) -> Result<(), String> {
    timegrapher_core::capture::checked_recording_deadline(o.seconds)
        .map_err(|e| format!("--seconds {e}"))?;
    let cfg = DiagnoseConfig {
        bph: o.bph,
        ..Default::default()
    };
    let (audio, source) = acquire(o)?;
    let check = diagnose::check(&audio, &cfg);

    let card = o.card.clone().or_else(|| {
        source["device"]["id"]
            .as_str()
            .and_then(mixer::card_from_device)
            .or_else(|| (o.file.is_none()).then(mixer::single_usb_card).flatten())
    });
    let os = std::env::consts::OS;
    let mixer = (os == "linux")
        .then(|| card.as_deref().and_then(mixer::read_alsa))
        .flatten();
    let fixes = if os == "linux" {
        mixer::linux_fixes(&check, mixer.as_ref())
    } else {
        mixer::other_fixes(&check, os)
    };

    if let Some(p) = &o.save {
        save(p, &audio, &source, mixer.as_ref())?;
    }

    let mut applied = false;
    let mut after = None;
    if o.apply && fixes.iter().any(|f| f.command.is_some()) {
        mixer::apply(&fixes)?;
        applied = true;
        if o.file.is_none() {
            let (audio, _) = acquire(o)?;
            after = Some(diagnose::check(&audio, &cfg));
        }
    }
    let rep = Report {
        source,
        check,
        mixer,
        fixes,
        applied,
        after,
    };
    if o.json {
        let files: Vec<PathBuf> = o.file.iter().cloned().collect();
        let settings =
            json!({ "seconds": o.seconds, "bph": o.bph, "device": o.device, "card": o.card });
        output::print("doctor", output::input(&files, settings), &rep)?;
    } else {
        print_report(&rep, o.apply);
    }
    Ok(())
}

fn acquire(o: &Options) -> Result<(Audio, Value), String> {
    if let Some(f) = &o.file {
        let a = audio::load(f).map_err(|e| format!("{}: {e}", f.display()))?;
        return Ok((a, json!({ "file": f.display().to_string() })));
    }
    live(o)
}

#[cfg(feature = "live")]
fn live(o: &Options) -> Result<(Audio, Value), String> {
    let (dev, info) = timegrapher_core::capture::find(o.device.as_deref())?;
    eprintln!(
        "listening to '{}' for {} s",
        info.name,
        fixed_text(o.seconds, 0)
    );
    let (a, rec) = timegrapher_core::capture::record(&dev, info, o.seconds)?;
    let v = serde_json::to_value(&rec).map_err(|e| e.to_string())?;
    Ok((a, v))
}

#[cfg(not(feature = "live"))]
fn live(_: &Options) -> Result<(Audio, Value), String> {
    Err("this build has no sound input support; pass --file, or build with --features live".into())
}

fn save(p: &Path, a: &Audio, source: &Value, mixer: Option<&Mixer>) -> Result<(), String> {
    audio::write_wav(p, a).map_err(|e| format!("{}: {e}", p.display()))?;
    let side = output::sidecar_path(p);
    let doc = json!({
        "schema": format!("timegrapher.recording/{}", output::SCHEMA_VERSION),
        "software": output::software(),
        "recording": { "source": source, "mixer": mixer },
    });
    std::fs::write(
        &side,
        serde_json::to_string_pretty(&doc).unwrap_or_default(),
    )
    .map_err(|e| format!("{}: {e}", side.display()))
}

/// One decimal place, or "-" with no value. A value that rounds to zero
/// prints as 0.0, not -0.0 (#810), by core's one rule
/// ([`timegrapher_core::library::fixed_text`], #815).
fn opt(v: Option<f64>, unit: &str) -> String {
    v.map_or("-".into(), |x| {
        format!("{}{unit}", timegrapher_core::library::fixed_text(x, 1))
    })
}

/// The text line for the signal figure. With no figure it says why
/// rather than printing "-" (#811).
fn signal_line(signal_x: Option<f64>) -> String {
    match signal_x {
        Some(x) => format!("Signal       {}× above the background", fixed_text(x, 0)),
        None => {
            "Signal       not measured: no beats, or the sound is mostly digital silence".into()
        }
    }
}

/// The text line for the gap rise. With too few gaps between ticks to
/// measure it says so rather than printing "-" (#833).
fn gap_rise_line(gap_rise_db: Option<f64>) -> String {
    match gap_rise_db {
        Some(_) => format!(
            "Gap rise     background rises {} between ticks",
            opt(gap_rise_db, " dB")
        ),
        None => "Gap rise     not measured: too few gaps between ticks".into(),
    }
}

/// The quick look's line: the rate signed as every surface signs it, so
/// one that rounds to zero reads 0.0, never -0.0 (#815), and both figures
/// at their place, as every figure is written (core's `figure`, #1470).
fn quick_look_line(rate_s_per_day: f64, beat_error_ms: f64) -> String {
    format!(
        "Quick look   rate {} s/d, beat error {} ms (on the sound card's clock)",
        figure(rate_s_per_day, None, base_digits::RATE, true),
        figure(beat_error_ms, None, base_digits::BEAT_ERROR, false)
    )
}

fn print_check(c: &SignalCheck) {
    println!(
        "Level        peak {} dBFS, RMS {} dBFS, {} clipped samples",
        fixed_text(c.peak_dbfs, 1),
        fixed_text(c.rms_dbfs, 1),
        c.clipped_samples
    );
    println!(
        "Ticks        {} of about {} beats at {} bph; {}, background {} dBFS",
        c.beats_found,
        c.beats_expected,
        c.bph,
        c.tick_level_dbfs
            .map_or("no tick to measure".into(), |t| format!(
                "tick {}",
                opt(Some(t), " dBFS")
            )),
        fixed_text(c.noise_level_dbfs, 1),
    );
    println!("{}", signal_line(c.signal_x));
    println!("{}", gap_rise_line(c.gap_rise_db));
    if let (Some(r), Some(b)) = (c.rate_s_per_day, c.beat_error_ms) {
        println!("{}", quick_look_line(r, b));
    }
    if c.issues.is_empty() {
        println!("None of the checked conditions detected.");
    }
    for i in &c.issues {
        println!("Detected     {} ({})", i.title, i.evidence);
    }
}

fn print_report(r: &Report, apply: bool) {
    match r.source.get("file").and_then(Value::as_str) {
        Some(f) => println!("Source       {f}"),
        None => println!(
            "Source       {} ({} Hz, {} channel(s), {})",
            r.source["device"]["name"].as_str().unwrap_or("?"),
            r.source["sample_rate"],
            r.source["channels"],
            r.source["sample_format"].as_str().unwrap_or("?")
        ),
    }
    print_check(&r.check);
    if let Some(m) = &r.mixer {
        let agc = m.agc().map(|c| {
            format!(
                "'{}' {}",
                c.name,
                if c.switch == Some(true) { "on" } else { "off" }
            )
        });
        let lvl = m.capture_level().map(|c| {
            let (cur, _, hi) = c.capture.unwrap_or_default();
            format!(
                "'{}' {cur}/{hi}{}",
                c.name,
                c.capture_db
                    .map_or(String::new(), |d| format!(" ({} dB)", signed_text(d, 1)))
            )
        });
        println!(
            "Mixer        card {}: {}",
            m.card,
            [lvl, agc]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !r.fixes.is_empty() {
        println!(
            "Changes --apply makes{}:",
            if r.applied { " (applied)" } else { "" }
        );
        for f in &r.fixes {
            println!("  - {}", f.description);
            if let Some(c) = &f.command {
                println!("      {}", mixer::shell_words(c));
            }
        }
        if !apply && r.fixes.iter().any(|f| f.command.is_some()) {
            println!("Nothing was changed; --apply makes these changes.");
        }
    }
    if let Some(a) = &r.after {
        println!("After the changes:");
        print_check(a);
    }
}

#[cfg(test)]
mod tests {
    use super::{gap_rise_line, opt, quick_look_line, signal_line};

    #[test]
    fn no_gap_rise_says_why_rather_than_a_dash() {
        assert_eq!(
            gap_rise_line(Some(-0.01)),
            "Gap rise     background rises 0.0 dB between ticks"
        );
        let none = gap_rise_line(None);
        assert!(none.contains("not measured"), "{none}");
        assert!(!none.contains(" - "), "{none}");
    }

    /// The quick look's rate reads as other surfaces read it: a plus on a
    /// gaining watch, and no sign where it rounds to zero (#815).
    #[test]
    fn the_quick_look_rate_has_no_minus_zero() {
        assert!(quick_look_line(-0.04, 0.3).starts_with("Quick look   rate 0.0 s/d, "));
        assert!(quick_look_line(2.36, 0.3).starts_with("Quick look   rate +2.4 s/d, "));
        assert!(quick_look_line(-2.36, 0.3).starts_with("Quick look   rate -2.4 s/d, "));
    }

    /// The quick look's beat error is at beat error's place, as the app's
    /// card and `analyze` write it (#1470).
    #[test]
    fn the_quick_look_beat_error_takes_its_base_decimals() {
        assert!(quick_look_line(1.0, 0.347).contains("beat error 0.35 ms"));
        assert!(quick_look_line(1.0, -0.001).contains("beat error 0.00 ms"));
    }

    #[test]
    fn a_value_that_rounds_to_zero_has_no_minus_sign() {
        assert_eq!(opt(Some(-0.01), " dB"), "0.0 dB");
        assert_eq!(opt(Some(-0.0), " dB"), "0.0 dB");
        assert_eq!(opt(Some(-0.06), " dB"), "-0.1 dB");
        assert_eq!(opt(Some(2.35), " dB"), "2.4 dB");
        assert_eq!(opt(None, " dB"), "-");
    }

    #[test]
    fn no_signal_figure_says_why_rather_than_a_dash() {
        assert_eq!(
            signal_line(Some(17.87)),
            "Signal       18× above the background"
        );
        let none = signal_line(None);
        assert!(none.contains("not measured"), "{none}");
        assert!(!none.contains(" - "), "{none}");
    }
}
