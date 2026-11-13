//! A plot's facts box: a few results read against the plot, a row each,
//! in a corner the trace leaves clear (#1528). The Tick Tock Profile's
//! and the Distributions' box.

use crate::marks::SHORT_PLOT;
use eframe::egui::{self, Color32};

/// A plot's facts in its top-left corner, as a plot item so they sit
/// over the grid and under the trace: a spike that runs into them still
/// shows. The Tick Tock Profile's box, and the Distributions' (#1528).
#[derive(Clone)]
pub(crate) struct FactBox {
    pub(crate) base: egui_plot::PlotItemBase,
    /// The four facts, or the one line saying there are none yet.
    pub(crate) rows: Vec<(String, String)>,
    pub(crate) plot_h: f32,
    /// The trace at each time, plot coordinates: the box takes the top
    /// right corner where it would cover the trace in the left and not
    /// there.
    pub(crate) tops: Vec<[f64; 2]>,
    /// Where `held` keeps the rows and corner last drawn, and those rows'
    /// count and whether in the right corner: another layout must clear
    /// the trace by `FACT_HOLD` more, so a trace on the edge doesn't make
    /// the box flick between two from frame to frame.
    pub(crate) held: egui::Id,
    pub(crate) was: Option<(usize, bool)>,
    /// The values' column, right-aligned, as wide as the widest value on
    /// either side; and the names' column likewise.
    pub(crate) name_w: f32,
    pub(crate) value_w: f32,
    /// The same for the two rows a short plot keeps.
    pub(crate) short_name_w: f32,
    pub(crate) short_value_w: f32,
    pub(crate) fill: Color32,
    pub(crate) name_color: Color32,
    pub(crate) value_color: Color32,
    /// Which rows a plot `plot_h` tall with room for `fit` rows keeps,
    /// top to bottom, for a box with a short form: fewer rows, tight
    /// around them, as narrow as `short_name_w` and `short_value_w`,
    /// shedding rows from the top. `None`: the first rows that fit, at
    /// the full spacing, shedding rows from the bottom.
    pub(crate) keep: Option<fn(f32, usize) -> Vec<usize>>,
}

/// The box's distance from the plot's corner, points.
pub(crate) const FACT_INSET: f32 = 4.0;
/// Room inside the box around its rows, and each row's height, points.
pub(crate) const FACT_PAD: f32 = 8.0;
pub(crate) const FACT_ROW: f32 = 16.0;
/// The room around the rows in a short plot's box, and each row's height.
pub(crate) const FACT_PAD_SHORT: f32 = 3.0;
pub(crate) const FACT_ROW_SHORT: f32 = 14.0;
/// Between the names' column and the values'.
pub(crate) const FACT_GAP: f32 = 16.0;
/// How much further from the trace a facts box must be to take a layout
/// other than the one last drawn, points.
pub(crate) const FACT_HOLD: f32 = 6.0;
/// The facts' type size.
pub(crate) const FACT_SIZE: f32 = 12.0;

impl FactBox {
    /// Where the box goes in a plot whose frame is `frame`, `rows` tall.
    /// Fewer than four rows of facts make a short box, as narrow as its
    /// rows and tight around them.
    pub(crate) fn rect(&self, frame: egui::Rect, rows: usize) -> egui::Rect {
        let short = self.is_short(rows);
        let (name_w, value_w) = if short {
            (self.short_name_w, self.short_value_w)
        } else {
            (self.name_w, self.value_w)
        };
        let pad = self.pad(rows);
        egui::Rect::from_min_size(
            frame.min + egui::vec2(FACT_INSET, FACT_INSET),
            egui::vec2(
                name_w + FACT_GAP + value_w + 2.0 * pad,
                rows as f32 * self.row_h(rows) + 2.0 * pad,
            ),
        )
    }

    /// Whether `rows` of the box take its short form.
    fn is_short(&self, rows: usize) -> bool {
        self.keep.is_some() && rows < self.rows.len()
    }

    /// The room around `rows` of the box.
    fn pad(&self, rows: usize) -> f32 {
        if self.is_short(rows) {
            FACT_PAD_SHORT
        } else {
            FACT_PAD
        }
    }

    /// Each row's height when the box shows `rows`.
    fn row_h(&self, rows: usize) -> f32 {
        if self.is_short(rows) {
            FACT_ROW_SHORT
        } else {
            FACT_ROW
        }
    }

    /// The rows shown in the plot as `t` draws it: inside the plot.
    pub(crate) fn shown(&self, t: &egui_plot::PlotTransform) -> Vec<usize> {
        let frame = *t.frame();
        let top = frame.top() + FACT_INSET;
        let room = |bottom: f32, pad: f32, row: f32| {
            ((bottom - top - 2.0 * pad) / row).floor().max(0.0) as usize
        };
        // A short plot's box is tight around its rows.
        let full = self.plot_h >= SHORT_PLOT
            && room(frame.bottom() - FACT_INSET, FACT_PAD, FACT_ROW) >= self.rows.len();
        let (pad, row) = if full || self.keep.is_none() {
            (FACT_PAD, FACT_ROW)
        } else {
            (FACT_PAD_SHORT, FACT_ROW_SHORT)
        };
        let fit_frame = room(frame.bottom() - FACT_INSET, pad, row);
        match self.keep {
            Some(keep) => keep(self.plot_h, fit_frame),
            None => (0..fit_frame.min(self.rows.len())).collect(),
        }
    }

    /// The box of `rows` in the top left corner and in the top right.
    pub(crate) fn corners(
        &self,
        t: &egui_plot::PlotTransform,
        rows: usize,
    ) -> (egui::Rect, egui::Rect) {
        let frame = *t.frame();
        let r = self.rect(frame, rows);
        (
            r,
            r.translate(egui::vec2(frame.right() - FACT_INSET - r.right(), 0.0)),
        )
    }

    /// Whether the trace runs under `r`, or within `more` points under it
    /// short of the plot's foot, where a trace at rest lies.
    pub(crate) fn covers(&self, t: &egui_plot::PlotTransform, r: egui::Rect, more: f32) -> bool {
        let edge = r.bottom() + 2.0;
        let edge = edge + more.min(t.frame().bottom() - 0.5 - edge).max(0.0);
        self.tops.iter().any(|p| {
            let x = t.position_from_point_x(p[0]);
            x >= r.left() && x <= r.right() && t.position_from_point_y(p[1]) < edge
        })
    }

    /// The rows drawn and where: those [`Self::shown`] keeps, in the top
    /// left corner, or the top right where the trace runs under the left
    /// and not the right. Where the trace runs under both corners the
    /// box keeps fewer rows, Unlock to Drop last, and none where even one row
    /// would sit on it: the picture comes first, and the box's hover has
    /// all four. A layout other than the one last drawn must clear the
    /// trace by [`FACT_HOLD`] more.
    pub(crate) fn layout(&self, t: &egui_plot::PlotTransform) -> (Vec<usize>, egui::Rect) {
        let shown = self.shown(t);
        if self.tops.is_empty() || shown.is_empty() {
            let r = self.corners(t, shown.len().max(1)).0;
            return (shown, r);
        }
        for n in (1..=shown.len()).rev() {
            let (left, right) = self.corners(t, n);
            for (r, right) in [(left, false), (right, true)] {
                let more = if self.was.is_none_or(|w| w == (n, right)) {
                    0.0
                } else {
                    FACT_HOLD
                };
                if !self.covers(t, r, more) {
                    let kept = if self.keep.is_some() {
                        &shown[shown.len() - n..]
                    } else {
                        &shown[..n]
                    };
                    return (kept.to_vec(), r);
                }
            }
        }
        (Vec::new(), self.corners(t, 1).0)
    }

    /// The span of x the box takes, as `t` draws it, its sides 0 wide at
    /// the plot's left edge with no rows shown: nothing names itself over
    /// it.
    pub(crate) fn span(&self, t: &egui_plot::PlotTransform) -> (f32, f32) {
        let (shown, r) = self.layout(t);
        if shown.is_empty() {
            (t.frame().left(), t.frame().left())
        } else {
            (r.left() - FACT_INSET, r.right())
        }
    }
}

impl egui_plot::PlotItem for FactBox {
    fn shapes(
        &self,
        ui: &egui::Ui,
        transform: &egui_plot::PlotTransform,
        shapes: &mut Vec<egui::Shape>,
    ) {
        let (shown, r) = self.layout(transform);
        // Kept for the next frame's layout, as `held` reads it.
        let right = r.right() >= transform.frame().right() - FACT_INSET - 0.5;
        ui.ctx()
            .data_mut(|d| d.insert_temp(self.held, (shown.len(), right)));
        if shown.is_empty() {
            return;
        }
        let pad = self.pad(shown.len());
        let row = self.row_h(shown.len());
        shapes.push(egui::Shape::rect_filled(r, 6.0, self.fill));
        let font = egui::FontId::proportional(FACT_SIZE);
        for (i, (name, value)) in shown.iter().map(|&k| &self.rows[k]).enumerate() {
            let y = r.top() + pad + (i as f32 + 0.5) * row;
            ui.fonts_mut(|f| {
                let g = f.layout_no_wrap(name.clone(), font.clone(), self.name_color);
                let at = egui::pos2(r.left() + pad, y - g.size().y / 2.0);
                shapes.push(egui::Shape::galley(at, g, self.name_color));
                let g = f.layout_no_wrap(value.clone(), font.clone(), self.value_color);
                let at = egui::pos2(r.right() - pad - g.size().x, y - g.size().y / 2.0);
                shapes.push(egui::Shape::galley(at, g, self.value_color));
            });
        }
    }

    fn initialize(&mut self, _x_range: std::ops::RangeInclusive<f64>) {}

    fn color(&self) -> Color32 {
        self.value_color
    }

    fn geometry(&self) -> egui_plot::PlotGeometry<'_> {
        egui_plot::PlotGeometry::None
    }

    fn bounds(&self) -> egui_plot::PlotBounds {
        egui_plot::PlotBounds::NOTHING
    }

    fn base(&self) -> &egui_plot::PlotItemBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut egui_plot::PlotItemBase {
        &mut self.base
    }
}

/// The widest of `texts` in the facts' type, points.
pub(crate) fn widest<'a>(ui: &egui::Ui, texts: impl Iterator<Item = &'a str>) -> f32 {
    texts
        .map(|t| {
            ui.fonts_mut(|f| {
                f.layout_no_wrap(
                    t.to_owned(),
                    egui::FontId::proportional(FACT_SIZE),
                    Color32::WHITE,
                )
                .size()
                .x
            })
        })
        .fold(0.0, f32::max)
}
