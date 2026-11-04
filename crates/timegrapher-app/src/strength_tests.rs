use super::*;
use crate::theme;
use eframe::egui::{pos2, Rect};
use std::path::Path;

/// Every source file of the app but the tests, with its text.
fn sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut dirs = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if p.is_dir() {
                dirs.push(p);
            } else if name.ends_with(".rs") && !name.ends_with("_tests.rs") {
                out.push((name, std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    out
}

#[test]
fn no_plot_draws_the_library_grid() {
    // A plot's marks are set only through a Grid, which turns the
    // library's grid off, and every plot is put on one.
    for (name, text) in sources() {
        if name == "strength.rs" {
            continue;
        }
        for call in [
            "x_grid_spacer(",
            "y_grid_spacer(",
            "grid_spacing(",
            "show_grid(",
        ] {
            assert!(!text.contains(call), "{name} calls {call}");
        }
        let plots = text.matches("Plot::new(").count();
        let on = text.matches(".on(").count();
        assert!(plots <= on, "{name}: {plots} plots, {on} put on a Grid");
    }
}

#[test]
fn only_this_module_has_the_grid_and_axis_greys() {
    for (name, text) in sources() {
        if name == "strength.rs" || name == "theme.rs" {
            continue;
        }
        for grey in ["grid_opacity", "axis_opacity"] {
            assert!(!text.contains(grey), "{name} uses {grey}");
        }
    }
}

#[test]
fn the_four_strengths_rise_from_grid_to_data() {
    for pal in [&theme::DARK, &theme::LIGHT] {
        let (g, a) = (colour(Strength::Grid, pal), colour(Strength::Axis, pal));
        assert!(g.a() < a.a(), "the grid is fainter than the axes");
        assert_eq!(colour(Strength::CallOut, pal), pal.callout);
        assert!(stroke(Strength::CallOut, pal).width > stroke(Strength::Axis, pal).width);
    }
    // The spec's: 8% and 27% of the text in dark, 9% and 33% in light.
    assert_eq!(colour(Strength::Grid, &theme::DARK).a(), 20);
    assert_eq!(colour(Strength::Axis, &theme::DARK).a(), 69);
    assert_eq!(colour(Strength::Grid, &theme::LIGHT).a(), 23);
    assert_eq!(colour(Strength::Axis, &theme::LIGHT).a(), 84);
}

#[test]
fn a_grid_line_on_an_axis_is_not_drawn() {
    let frame = Rect::from_min_max(pos2(10.0, 0.0), pos2(110.0, 100.0));
    let (down, across) = layout(
        frame,
        &[10.0, 10.3, 50.0, 120.0],
        &[0.0, 50.0, 70.2, 99.8],
        Some(70.0),
    );
    // On the left axis, or off the plot: gone.
    assert_eq!(down, vec![50.0]);
    // On the bottom axis or the zero line: gone. At the top: kept.
    assert_eq!(across, vec![0.0, 50.0]);
}

#[test]
fn a_zero_line_off_the_plot_hides_no_grid_line() {
    let frame = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
    let (_, across) = layout(frame, &[], &[40.0], Some(-500.0));
    assert_eq!(across, vec![40.0]);
}

#[test]
fn the_axes_lie_inside_the_frame_the_plot_clips_to() {
    let frame = Rect::from_min_max(pos2(10.0, 0.0), pos2(110.0, 100.0));
    let (left, bottom) = axes(frame);
    // A 1 pt line centred here is wholly inside the frame.
    assert_eq!(left - 0.5, frame.left());
    assert_eq!(bottom + 0.5, frame.bottom());
}
