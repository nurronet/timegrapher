//! The Isochronism pane's facts box (Hugh's rule 13, Peter 11-05): core's
//! facts as a stack of rows on a plate of the plot's background, in the
//! corner the plot's points leave emptiest, chosen once per take so it
//! never jumps. Drawn as a plot item, under the curve or the walk.

use crate::marks;
use eframe::egui::{self, Color32, Rect};
use timegrapher_core::steady_views::ISO_SHORT_FACTS;

/// The box: its rows, what it keeps clear of, and where it keeps its
/// corner.
#[derive(Clone)]
pub(super) struct FactsBox {
    pub base: egui_plot::PlotItemBase,
    /// Core's facts, name and value ([`iso_facts`]).
    ///
    /// [`iso_facts`]: timegrapher_core::steady_views::iso_facts
    pub rows: Vec<(String, String)>,
    /// The points the plot draws, plot coordinates: the box takes the
    /// corner with the fewest.
    pub points: Vec<[f64; 2]>,
    /// Where the corner chosen for this take is kept, and the take.
    pub held: egui::Id,
    pub take: u64,
    pub fill: Color32,
    pub name_color: Color32,
    pub value_color: Color32,
}

/// The box's text size, row height, padding and distance from the plot's
/// edges, points: the Profile's.
fn metrics() -> (f32, f32, f32, f32) {
    (12.0, 16.0, 8.0, 4.0)
}

impl FactsBox {
    /// The rows a plot as tall as `frame` shows: all, or on a short plot
    /// Slope and Flattest, the rest left to the hover.
    pub fn shown(&self, frame: Rect) -> Vec<&(String, String)> {
        let short = frame.height() < marks::SHORT_PLOT;
        self.rows
            .iter()
            .filter(|(n, _)| !short || ISO_SHORT_FACTS.contains(&n.as_str()))
            .collect()
    }

    /// Where the box is on the plot `t` draws, and the rows in it: in
    /// the corner holding the fewest of the plot's points, the one first
    /// chosen for this take.
    pub fn place(&self, ctx: &egui::Context, t: &egui_plot::PlotTransform) -> (Rect, usize) {
        let frame = *t.frame();
        let rows = self.shown(frame);
        let (size, row, pad, inset) = metrics();
        let font = egui::FontId::proportional(size);
        let width = |text: &str| {
            ctx.fonts_mut(|f| {
                f.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE)
                    .size()
                    .x
            })
        };
        let names = rows.iter().map(|r| width(&r.0)).fold(0.0, f32::max);
        let values = rows.iter().map(|r| width(&r.1)).fold(0.0, f32::max);
        let gap = 2.0 * pad;
        let box_size = egui::vec2(
            names + gap + values + 2.0 * pad,
            rows.len() as f32 * row + 2.0 * pad,
        );
        let corners = [
            frame.left_top() + egui::vec2(inset, inset),
            frame.right_top() + egui::vec2(-inset - box_size.x, inset),
            frame.left_bottom() + egui::vec2(inset, -inset - box_size.y),
            frame.right_bottom() - egui::vec2(inset, inset) - box_size,
        ]
        .map(|min| Rect::from_min_size(min, box_size));
        let held = ctx.data(|d| d.get_temp::<(u64, usize)>(self.held));
        let corner = match held {
            Some((take, k)) if take == self.take => k,
            _ => {
                let inside = |r: &Rect| {
                    self.points
                        .iter()
                        .filter(|p| {
                            r.contains(
                                t.position_from_point(&egui_plot::PlotPoint::new(p[0], p[1])),
                            )
                        })
                        .count()
                };
                let k = (0..corners.len())
                    .min_by_key(|&k| inside(&corners[k]))
                    .unwrap_or(0);
                ctx.data_mut(|d| d.insert_temp(self.held, (self.take, k)));
                k
            }
        };
        (corners[corner], rows.len())
    }

    /// Over the box on a plot that keeps fewer rows, every fact.
    pub fn hover(&self, resp: &egui_plot::PlotResponse<()>) {
        let t = &resp.transform;
        let (r, n) = self.place(&resp.response.ctx, t);
        let over = resp.response.hover_pos().is_some_and(|p| r.contains(p));
        if over && n < self.rows.len() {
            let all: Vec<String> = self.rows.iter().map(|(n, v)| format!("{n}: {v}")).collect();
            resp.response
                .clone()
                .on_hover_text_at_pointer(all.join("\n"));
        }
    }
}

impl egui_plot::PlotItem for FactsBox {
    fn shapes(
        &self,
        ui: &egui::Ui,
        transform: &egui_plot::PlotTransform,
        shapes: &mut Vec<egui::Shape>,
    ) {
        let (r, _) = self.place(ui.ctx(), transform);
        let rows = self.shown(*transform.frame());
        if rows.is_empty() {
            return;
        }
        let (size, row, pad, _) = metrics();
        let look = marks::FactsLook {
            fill: self.fill,
            name: self.name_color,
            value: self.value_color,
        };
        marks::fact_rows(ui, shapes, r, rows.into_iter(), (size, row, pad), look);
    }

    marks::plot_item_plumbing!(|b| b.value_color);
}
