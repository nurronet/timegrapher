//! How strongly each line on a plot is drawn: every line takes one of
//! four strengths, weakest to strongest grid, axis, call-out and data,
//! each one colour and one width on every plot and at every window size
//! ("Line Strength on Plots"). A line's strength comes from what it is,
//! never from how far apart the lines are, so the plotting library's own
//! grid (which fades with spacing) is turned off and [`Grid`] draws the
//! grid and the axes instead. The colours are the core palette's, so the
//! MCP charts can draw the same.

use crate::theme::Palette;
use eframe::egui::{self, Color32, Pos2, Shape, Stroke, Ui};
use egui_plot::{
    GridInput, GridMark, Plot, PlotBounds, PlotGeometry, PlotItem, PlotItemBase, PlotTransform,
    PlotUi,
};
use std::rc::Rc;

/// What a line on a plot is, which sets how strongly it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    /// Every grid line, across and down.
    Grid,
    /// The plot's left and bottom edges, and a zero line where zero
    /// means something.
    Axis,
    /// A named or reference line (Unlock, Drop, Peak, the sounds, the
    /// wheels, a threshold, the hover line): one grey, told apart by its
    /// pattern.
    CallOut,
}

/// The fourth and strongest, data (anything drawn from the readings),
/// takes its reading's own colour, so it is not a [`Strength`]: its lines
/// are at least this wide.
pub const DATA_W: f32 = 1.5;

/// A call-out's width.
pub const CALL_OUT_W: f32 = 1.2;

/// The colour of a grid, axis or call-out line. Grid and axis are the
/// text colour see-through, so they keep their strength over a tint such
/// as the Paper Strip's wash.
pub fn colour(s: Strength, pal: &Palette) -> Color32 {
    match s {
        Strength::Grid => see_through(pal.text, pal.grid_opacity),
        Strength::Axis => see_through(pal.text, pal.axis_opacity),
        Strength::CallOut => pal.callout,
    }
}

/// The stroke of a grid, axis or call-out line: 1 pt for grid and axis,
/// [`CALL_OUT_W`] for a call-out.
pub fn stroke(s: Strength, pal: &Palette) -> Stroke {
    let w = match s {
        Strength::Grid | Strength::Axis => 1.0,
        Strength::CallOut => CALL_OUT_W,
    };
    Stroke::new(w, colour(s, pal))
}

fn see_through(c: Color32, opacity: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (opacity * 255.0).round() as u8)
}

/// The marks one axis of a plot is labelled at (and gridded at).
pub type Spacer = Rc<dyn Fn(GridInput) -> Vec<GridMark>>;

/// A plot's grid and axes: the marks along each axis (the same marks its
/// labels sit at) and whether zero across is a zero line.
#[derive(Clone)]
pub struct Grid {
    x: Spacer,
    y: Spacer,
    zero_y: bool,
}

impl Grid {
    pub fn new(
        x: impl Fn(GridInput) -> Vec<GridMark> + 'static,
        y: impl Fn(GridInput) -> Vec<GridMark> + 'static,
    ) -> Self {
        Grid {
            x: Rc::new(x),
            y: Rc::new(y),
            zero_y: false,
        }
    }

    /// Draw zero across at axis strength (0 s/d, 0 ms, a correlation of
    /// 0), where zero means something.
    pub fn zero_y(mut self) -> Self {
        self.zero_y = true;
        self
    }

    /// `plot` labelled at this grid's marks, with the library's grid off.
    pub fn on<'a>(&self, plot: Plot<'a>) -> Plot<'a> {
        let (x, y) = (self.x.clone(), self.y.clone());
        plot.x_grid_spacer(move |g| x(g))
            .y_grid_spacer(move |g| y(g))
            .show_grid(false)
    }

    /// Add the grid and axes to a plot, before anything else so they lie
    /// under it.
    pub fn draw(&self, p: &mut PlotUi<'_>, pal: &Palette) {
        p.add(GridItem {
            base: PlotItemBase::new(String::new()),
            grid: self.clone(),
            grid_c: colour(Strength::Grid, pal),
            axis_c: colour(Strength::Axis, pal),
        });
    }
}

/// The plot's minimum grid spacing, in points: what egui_plot hands a
/// spacer as its base step.
const BASE_SPACING: f32 = 8.0;

/// Within this many points of an axis a grid line is not drawn, so the
/// axis never doubles up or flickers as the range moves.
const ON_AXIS: f32 = 0.5;

/// The grid and the axes, as one item drawn under the plot's others.
struct GridItem {
    base: PlotItemBase,
    grid: Grid,
    grid_c: Color32,
    axis_c: Color32,
}

impl GridItem {
    /// Where each grid line falls along `axis` (0 across, 1 down), in
    /// screen points.
    fn lines(&self, t: &PlotTransform, axis: usize) -> Vec<f32> {
        let b = t.bounds();
        let (lo, hi) = if axis == 0 {
            (b.min()[0], b.max()[0])
        } else {
            (b.min()[1], b.max()[1])
        };
        let input = GridInput {
            bounds: (lo, hi),
            base_step_size: t.dvalue_dpos()[axis].abs() * f64::from(BASE_SPACING),
        };
        let spacer = if axis == 0 {
            &self.grid.x
        } else {
            &self.grid.y
        };
        spacer(input)
            .into_iter()
            .map(|m| {
                if axis == 0 {
                    t.position_from_point_x(m.value)
                } else {
                    t.position_from_point_y(m.value)
                }
            })
            .collect()
    }
}

/// The screen positions of the grid lines and the axes of one plot: grid
/// lines across (`xs`) and down (`ys`), less any on an axis.
fn layout(frame: egui::Rect, xs: &[f32], ys: &[f32], zero: Option<f32>) -> (Vec<f32>, Vec<f32>) {
    let clear = |v: f32, axes: &[f32]| axes.iter().all(|a| (v - a).abs() > ON_AXIS);
    let mut across_axes = vec![frame.bottom()];
    across_axes.extend(zero.filter(|z| frame.y_range().contains(*z)));
    let down = xs
        .iter()
        .copied()
        .filter(|&x| frame.x_range().contains(x) && clear(x, &[frame.left()]))
        .collect();
    let across = ys
        .iter()
        .copied()
        .filter(|&y| frame.y_range().contains(y) && clear(y, &across_axes))
        .collect();
    (down, across)
}

impl PlotItem for GridItem {
    fn shapes(&self, _ui: &Ui, t: &PlotTransform, shapes: &mut Vec<Shape>) {
        let f = *t.frame();
        let zero = self.grid.zero_y.then(|| t.position_from_point_y(0.0));
        let (down, across) = layout(f, &self.lines(t, 0), &self.lines(t, 1), zero);
        let grid = Stroke::new(1.0_f32, self.grid_c);
        let axis = Stroke::new(1.0_f32, self.axis_c);
        let seg = |a: Pos2, b: Pos2, s: Stroke| Shape::line_segment([a, b], s);
        for x in down {
            shapes.push(seg(Pos2::new(x, f.top()), Pos2::new(x, f.bottom()), grid));
        }
        for y in across {
            shapes.push(seg(Pos2::new(f.left(), y), Pos2::new(f.right(), y), grid));
        }
        if let Some(z) = zero.filter(|z| f.y_range().contains(*z)) {
            shapes.push(seg(Pos2::new(f.left(), z), Pos2::new(f.right(), z), axis));
        }
        shapes.push(seg(f.left_top(), f.left_bottom(), axis));
        shapes.push(seg(f.left_bottom(), f.right_bottom(), axis));
    }

    /// Nothing to point at: the grid has no values.
    fn allow_hover(&self) -> bool {
        false
    }

    fn initialize(&mut self, _x_range: std::ops::RangeInclusive<f64>) {}

    fn color(&self) -> Color32 {
        self.grid_c
    }

    fn geometry(&self) -> PlotGeometry<'_> {
        PlotGeometry::None
    }

    fn bounds(&self) -> PlotBounds {
        PlotBounds::NOTHING
    }

    fn base(&self) -> &PlotItemBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PlotItemBase {
        &mut self.base
    }
}

#[cfg(test)]
#[path = "strength_tests.rs"]
mod tests;
