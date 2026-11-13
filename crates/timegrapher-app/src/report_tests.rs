//! Report a Problem as a person meets it: the form each kind opens, the
//! sound the dialog shows, what Save Report… waits on, the handles, and
//! what saving says (docs/reference/reports.md).

use super::*;
use eframe::egui::{Event, FullOutput, PointerButton, Pos2, Rect};
use timegrapher_core::synth::{generate, SynthConfig};

/// A folder of its own for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tg-report-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A watch on the stand, `secs` long, saved as `take.wav` in `dir`.
fn take(dir: &Path, secs: f64) -> Source {
    let wav = dir.join("take.wav");
    let cfg = SynthConfig {
        duration_s: secs,
        ..Default::default()
    };
    audio::write_wav(&wav, &generate(&cfg, |_| 280.0, |_| 0.0)).unwrap();
    Source {
        files: vec![wav],
        total_s: secs,
        name: "take.wav".into(),
        live: false,
        end_s: secs,
        about: Value::Null,
        cfg: None,
        kept: None,
    }
}

fn themed() -> egui::Context {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    ctx
}

/// One frame of the dialog on a 1280 × 820 window.
fn frame(
    d: &mut ReportDialog,
    ctx: &egui::Context,
    events: Vec<Event>,
) -> (Option<String>, FullOutput) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 820.0))),
        events,
        ..Default::default()
    };
    let mut message = None;
    let out = ctx.run(input, |ctx| message = d.ui(ctx));
    (message, out)
}

fn painted(out: &FullOutput) -> String {
    out.shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Where the text starting with `start` was painted.
fn text_at(out: &FullOutput, start: &str) -> Pos2 {
    out.shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text().starts_with(start) => {
                Some(t.galley.rect.translate(t.pos.to_vec2()).center())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("{start:?} not painted in {}", painted(out)))
}

/// The two handles' grips, left then right.
fn grips(out: &FullOutput) -> Vec<Pos2> {
    let mut at: Vec<Pos2> = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Rect(r) if r.rect.size() == Vec2::new(8.0, 22.0) => Some(r.rect.center()),
            _ => None,
        })
        .collect();
    at.sort_by(|a, b| a.x.total_cmp(&b.x));
    at
}

fn button(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn click(at: Pos2) -> Vec<Event> {
    vec![Event::PointerMoved(at), button(at, true), button(at, false)]
}

/// Drag from `from` to `to` over a few frames. egui starts a drag once
/// the pointer has moved a little, and the handle nearer to it then is
/// the one dragged, so the first step is short.
fn drag(d: &mut ReportDialog, ctx: &egui::Context, from: Pos2, to: Pos2) {
    frame(d, ctx, vec![Event::PointerMoved(from), button(from, true)]);
    let step = (to - from).normalized() * 10.0;
    frame(d, ctx, vec![Event::PointerMoved(from + step)]);
    for i in 1..=4 {
        let p = from + step + (to - from - step) * (i as f32 / 4.0);
        frame(d, ctx, vec![Event::PointerMoved(p)]);
    }
    frame(d, ctx, vec![button(to, false)]);
    frame(d, ctx, Vec::new());
}

/// Each kind opens a form the repository has, and Setup goes with Bug's.
#[test]
fn every_kind_opens_an_issue_form_the_repository_has() {
    let forms = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/ISSUE_TEMPLATE");
    let kinds = [Kind::Bug, Kind::Reading, Kind::Setup, Kind::Idea];
    for k in kinds {
        assert!(forms.join(k.form()).is_file(), "{:?}: no {}", k, k.form());
    }
    assert_eq!(Kind::Reading.form(), "bad_reading.yml");
    assert_eq!(Kind::Idea.form(), "feature.yml");
    assert_eq!(Kind::Setup.form(), Kind::Bug.form());
    let codes: std::collections::BTreeSet<_> = kinds.iter().map(|k| k.code()).collect();
    assert_eq!(
        codes.len(),
        kinds.len(),
        "each kind is told apart in the manifest"
    );
}

/// The waveform is the sound mixed to mono and scaled to ±1, in at most
/// 1000 columns; past the end there is nothing to show.
#[test]
fn the_waveform_is_the_sound_mixed_to_mono() {
    let dir = scratch("mono");
    let wav = dir.join("stereo.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&wav, spec).unwrap();
    for _ in 0..8000 * 3 {
        w.write_sample(16384i16).unwrap();
        w.write_sample(-8192i16).unwrap();
    }
    w.finalize().unwrap();
    let c = cut_clip(std::slice::from_ref(&wav), 1.0).unwrap();
    assert_eq!((c.from_s, c.rate), (1.0, 8000));
    assert!((c.len_s() - 2.0).abs() < 1e-9, "{}", c.len_s());
    assert!(
        c.mono.iter().all(|&s| (s - 0.125).abs() < 1e-6),
        "the two channels' mean, over full scale"
    );
    assert!(
        c.outline.len() <= COLUMNS && c.outline.len() >= COLUMNS - 1,
        "{}",
        c.outline.len()
    );
    assert!(c
        .outline
        .iter()
        .all(|&(lo, hi)| lo == 0.0 && (hi - 0.125).abs() < 1e-6));
    assert!(cut_clip(&[wav], 10.0).is_err(), "nothing after the end");
    let _ = std::fs::remove_dir_all(&dir);
}

/// With no sound, the dialog says why and Save Report… is offered at
/// once; it asks where once per press, and Cancel closes.
#[test]
fn with_no_sound_save_report_is_offered_at_once() {
    let ctx = themed();
    let mut d = ReportDialog {
        note: "draft".into(),
        ..Default::default()
    };
    d.show(Kind::Idea, None, Shown::default());
    assert!(d.open && !d.include);
    frame(&mut d, &ctx, Vec::new());
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    let text = painted(&out);
    assert!(text.contains("There is no sound to include"), "{text}");
    assert!(!text.contains("Include the Sound"), "{text}");
    frame(&mut d, &ctx, click(text_at(&out, "Save Report")));
    assert!(d.save_asked().is_some(), "Save Report… asks where");
    assert!(d.save_asked().is_none(), "once per press");
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    frame(&mut d, &ctx, click(text_at(&out, "Cancel")));
    assert!(!d.open);
    assert_eq!(
        d.note, "draft",
        "what was written waits for the next report"
    );
}

/// With sound, Save Report… waits until the person ticks that they
/// listened; leaving the sound out doesn't wait.
#[test]
fn save_report_waits_until_the_sound_is_listened_to() {
    let dir = scratch("listen");
    let ctx = themed();
    let mut d = ReportDialog::default();
    d.show(Kind::Bug, Some(take(&dir, 6.0)), Shown::default());
    assert!(d.include, "the sound is in at first");
    frame(&mut d, &ctx, Vec::new());
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    let text = painted(&out);
    assert!(text.contains("0:06 of take.wav."), "{text}");
    assert!(
        !text.contains("Ends At"),
        "a take under five minutes is all offered"
    );
    assert!(d.has_sound());
    frame(&mut d, &ctx, click(text_at(&out, "Save Report")));
    assert!(d.save_asked().is_none(), "not before the box is ticked");
    frame(&mut d, &ctx, click(text_at(&out, "I listened")));
    assert!(d.listened && !d.cc0);
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    frame(&mut d, &ctx, click(text_at(&out, "Save Report")));
    assert!(d.save_asked().is_some());

    // Leaving the sound out: nothing to listen to.
    d.show(Kind::Bug, Some(take(&dir, 6.0)), Shown::default());
    assert!(!d.listened, "each report is ticked afresh");
    frame(&mut d, &ctx, Vec::new());
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    frame(&mut d, &ctx, click(text_at(&out, "Include the Sound")));
    assert!(!d.include && !d.has_sound());
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    assert!(!painted(&out).contains("I listened"));
    frame(&mut d, &ctx, click(text_at(&out, "Save Report")));
    assert!(d.save_asked().is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A live session offers its last minutes; a recording over five minutes
/// gets Ends At to pick which five.
#[test]
fn a_long_recording_offers_ends_at() {
    let ctx = themed();
    let mut d = ReportDialog::default();
    let src = |live, total_s| Source {
        files: Vec::new(),
        total_s,
        name: "this session".into(),
        live,
        end_s: total_s,
        about: Value::Null,
        cfg: None,
        kept: None,
    };
    d.show(Kind::Setup, Some(src(true, 90.0)), Shown::default());
    frame(&mut d, &ctx, Vec::new());
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    let text = painted(&out);
    assert!(text.contains("The last 1:30 of this session"), "{text}");
    assert!(
        text.contains("Can't read the sound"),
        "no files, said: {text}"
    );
    assert!(!text.contains("Ends At"));
    d.show(Kind::Setup, Some(src(false, 900.0)), Shown::default());
    frame(&mut d, &ctx, Vec::new());
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    let text = painted(&out);
    assert!(
        text.contains("Ends At") && text.contains("5:00 of this session."),
        "{text}"
    );
    assert_eq!(d.end_s, 900.0, "the last five minutes at first");
}

/// The handles trim the sound, never closer than two seconds; moving
/// one asks for the sound to be listened to again.
#[test]
fn the_handles_trim_the_sound_and_untick_listened() {
    let dir = scratch("trim");
    let ctx = themed();
    let mut d = ReportDialog::default();
    d.show(Kind::Bug, Some(take(&dir, 10.0)), Shown::default());
    frame(&mut d, &ctx, Vec::new());
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    let len = d.trim.1;
    assert!(d.trim.0 == 0.0 && (len - 10.0).abs() < 0.01, "{:?}", d.trim);
    let g = grips(&out);
    assert_eq!(g.len(), 2, "two handles");
    let width = g[1].x - g[0].x;
    d.listened = true;
    // The right handle to the middle.
    drag(&mut d, &ctx, g[1], g[0] + Vec2::new(width / 2.0, 0.0));
    assert!((d.trim.1 - 5.0).abs() < 0.2, "{:?}", d.trim);
    assert!(!d.listened, "a new stretch is listened to again");
    // The left handle past it stops two seconds short.
    drag(&mut d, &ctx, g[0], g[1] + Vec2::new(40.0, 0.0));
    assert!((d.trim.1 - d.trim.0 - MIN_S).abs() < 1e-9, "{:?}", d.trim);
    assert!(d.dragging.is_none(), "let go");
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    assert!(painted(&out).contains("0:02 kept"), "{}", painted(&out));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Saving writes the trimmed part, then opens the zip's folder and the
/// kind's GitHub form and says where it went.
#[test]
fn saving_writes_the_trimmed_part_and_opens_the_form() {
    let dir = scratch("save");
    let ctx = themed();
    let mut d = ReportDialog::default();
    d.show(Kind::Reading, Some(take(&dir, 10.0)), Shown::default());
    d.note = "  rate jumps  ".into();
    frame(&mut d, &ctx, Vec::new());
    d.trim = (2.0, 6.0);
    d.listened = true;
    let zip = dir.join("r.zip");
    d.save_to(zip.clone());
    assert_eq!(
        d.save_dir.as_deref(),
        Some(dir.as_path()),
        "the next report asks here first"
    );
    let mut said = None;
    let mut urls = Vec::new();
    for _ in 0..600 {
        let (m, out) = frame(&mut d, &ctx, Vec::new());
        urls.extend(out.platform_output.commands.iter().filter_map(|c| match c {
            egui::OutputCommand::OpenUrl(u) => Some(u.url.clone()),
            _ => None,
        }));
        if m.is_some() {
            said = m;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let said = said.expect("saved");
    assert!(said.contains(&zip.display().to_string()), "{said}");
    assert_eq!(
        urls,
        [
            format!("file://{}", dir.display()),
            "https://github.com/peterhudson/timegrapher/issues/new?template=bad_reading.yml".into(),
        ]
    );
    assert!(matches!(d.result, Some(Saved::Done { .. })));
    let files = archive::read_zip(&zip).unwrap();
    let m: Value =
        serde_json::from_slice(&files.iter().find(|(p, _)| p == "manifest.json").unwrap().1)
            .unwrap();
    assert_eq!(m["note"], "rate jumps");
    assert_eq!(m["kind"], "reading");
    assert_eq!(m["recording"]["excerpt"]["from_s"], 2.0);
    let span = m["recording"]["excerpt"]["span_s"].as_f64().unwrap();
    assert!((span - 4.0).abs() < 0.01, "{span}");
    assert_eq!(m["consent"]["listened_back"], true);
    let _ = std::fs::remove_dir_all(&dir);
}

/// What saving came to: the size, a warning over GitHub's limit, or why
/// it failed.
#[test]
fn the_result_line_warns_over_githubs_limit() {
    let ctx = themed();
    let mut d = ReportDialog::default();
    d.show(Kind::Bug, None, Shown::default());
    let said = |d: &mut ReportDialog| painted(&frame(d, &ctx, Vec::new()).1);
    said(&mut d); // a new window's first frame only sizes it
    let zip = PathBuf::from("r.zip");
    d.result = Some(Saved::Done {
        zip: zip.clone(),
        bytes: 2_400_000,
    });
    let t = said(&mut d);
    assert!(
        t.contains("Saved r.zip (2.4 MB).") && !t.contains("25 MB limit"),
        "{t}"
    );
    d.result = Some(Saved::Done {
        zip,
        bytes: 26_000_000,
    });
    let t = said(&mut d);
    assert!(
        t.contains("(26.0 MB). That is over GitHub's 25 MB limit"),
        "{t}"
    );
    d.result = Some(Saved::Failed("disk full".into()));
    assert!(said(&mut d).contains("Couldn't save the report: disk full"));
    d.show(Kind::Bug, None, Shown::default());
    let t = said(&mut d);
    assert!(
        t.contains("There is no sound to include") && !t.contains("Saved"),
        "a new report starts without one: {t}"
    );
}

/// A job saving `source` with no excerpt, as Save Report… makes it: the
/// zip's files.
fn saved_without_excerpt(dir: &Path, source: Source) -> Vec<(String, Vec<u8>)> {
    let job = Job {
        zip: dir.join("r.zip"),
        created: "2026-11-12T19:00:00.000Z".into(),
        kind: Kind::Reading,
        note: String::new(),
        source: Some(source),
        sound: None,
        listened: false,
        cc0: false,
        shown: Shown {
            bph: Some(28800),
            lift_deg: 52.0,
            app: json!({}),
        },
    };
    job.write().unwrap();
    archive::read_zip(&job.zip).unwrap()
}

/// The analysis in a report's `files`, read as core reads a take's.
fn analysis_in(files: &[(String, Vec<u8>)], dir: &Path) -> analysis_take::Stored {
    for (p, d) in files.iter().filter(|(p, _)| p.starts_with("analysis/")) {
        let to = dir.join(p);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::write(to, d).unwrap();
    }
    analysis_take::read(dir).unwrap()
}

/// A report carries the take's beats and amplitude windows (#400) from
/// its whole sound, whole as it fits under GitHub's limit, even with the sound
/// left out.
#[test]
fn a_report_carries_the_whole_takes_beats() {
    let dir = scratch("beats");
    let files = saved_without_excerpt(&dir, take(&dir, 40.0));
    assert!(
        !files.iter().any(|(p, _)| p.starts_with("audio/")),
        "the sound was left out"
    );
    let s = analysis_in(&files, &dir.join("unpacked"));
    // 40 s at 8 beats a second.
    assert!(s.log.beats.len() > 250, "{} beats", s.log.beats.len());
    assert!(!s.log.amplitude_windows.is_empty());
    assert!(!s.profiles.is_empty(), "40 s fits whole");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A take kept as its analysis alone offers no sound, and its report
/// carries its own analysis, every beat it kept, at once.
#[test]
fn an_analysis_only_take_is_reported_with_its_analysis() {
    let dir = scratch("kept");
    let sound = take(&dir, 40.0);
    let mut cfg = timegrapher_core::stream::StreamConfig::default();
    cfg.analysis.bph = Some(28800);
    let paths: Vec<&Path> = sound.files.iter().map(PathBuf::as_path).collect();
    let stored = analysis_take::measure(&paths, &cfg, &[], software(), |_| {}).unwrap();
    let kept = dir.join("kept");
    let manifest = "audio = false\nbph = 28800\n\n[[recording]]\nfile = \".\"\n";
    analysis_take::save(
        &kept,
        &[(take::MANIFEST.into(), manifest.as_bytes().to_vec())],
        &stored,
    )
    .unwrap();
    let source = Source {
        files: Vec::new(),
        total_s: 0.0,
        name: "kept".into(),
        live: false,
        end_s: 0.0,
        about: Value::Null,
        cfg: None,
        kept: Some(kept),
    };
    let ctx = themed();
    let mut d = ReportDialog::default();
    d.show(Kind::Reading, Some(source.clone()), Shown::default());
    assert!(!d.include, "no sound to include");
    frame(&mut d, &ctx, Vec::new());
    let (_, out) = frame(&mut d, &ctx, Vec::new());
    let text = painted(&out);
    assert!(text.contains("kept as its analysis alone"), "{text}");
    assert!(!text.contains("Include the Sound"), "{text}");
    frame(&mut d, &ctx, click(text_at(&out, "Save Report")));
    assert!(d.save_asked().is_some(), "Save Report… is offered at once");

    let files = saved_without_excerpt(&dir, source);
    let s = analysis_in(&files, &dir.join("unpacked"));
    assert_eq!(s.log.beats.len(), stored.log.beats.len());
    assert_eq!(s.profiles.len(), stored.profiles.len(), "it fits whole");
    let m: Value =
        serde_json::from_slice(&files.iter().find(|(p, _)| p == "manifest.json").unwrap().1)
            .unwrap();
    assert!(
        m["left_out"].to_string().contains("analysis alone"),
        "{}",
        m["left_out"]
    );
    assert!(
        !m["left_out"]
            .to_string()
            .contains("whole recording's sound"),
        "no sound to leave an analysis of: {}",
        m["left_out"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}
