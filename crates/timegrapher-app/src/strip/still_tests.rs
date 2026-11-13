//! Still paper drawn whole: what a lap cut at the wrap and the blank ahead
//! of the pen does to what's written on it.

use super::*;

/// A position's name on the track of still paper is written inside the
/// part of its lap that shows, never cut by the wrap or the blank ahead
/// of the pen (#1511: "Dial U" | "(DU)"), and each lap on show names it.
#[test]
fn still_paper_writes_a_positions_name_whole_in_each_lap() {
    for horizontal in [true, false] {
        let ctx = egui::Context::default();
        let view = StripView {
            half_width_ms: 10.0,
            span_s: 100.0,
            horizontal,
            still: true,
        };
        let track = [(0.0, f64::INFINITY, "Dial Up (DU)", "DU")];
        let extras = Extras {
            track: &track,
            ..Default::default()
        };
        // The pen 30% along: the last lap shows from past the blank to the
        // far end, this one from the start to the pen.
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 500.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    draw_strip(
                        ui,
                        &[],
                        0.125,
                        None,
                        130.0,
                        &view,
                        &extras,
                        Vec2::new(460.0, 460.0),
                    );
                });
            },
        );
        let mut names = 0;
        for c in &out.shapes {
            if let egui::Shape::Text(t) = &c.shape {
                let text = t.galley.text();
                if text == "Dial Up (DU)" || text == "DU" {
                    names += 1;
                    let r = c.shape.visual_bounding_rect();
                    assert!(
                        c.clip_rect.expand(0.5).contains_rect(r),
                        "{text} at {r:?} cut by {:?} (horizontal {horizontal})",
                        c.clip_rect
                    );
                }
            }
        }
        assert!(
            names >= 2,
            "both laps name it: {names} (horizontal {horizontal})"
        );
    }
}
