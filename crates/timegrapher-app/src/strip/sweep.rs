//! Where the strip puts a time along it. On moving paper the newest beats
//! stay at one edge and the paper slides. On still paper a pen sweeps
//! along and starts again at the start every Length, like a scope: the
//! last lap stays ahead of the pen, wiped a little in front of it, so a
//! full Length of beats is always on show (Hugh's spec, option A).

use eframe::egui::{Pos2, Rect};

/// How the paper carries time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Sweep {
    /// The paper moves: the newest time at the right (across) or the top
    /// (down).
    Moving,
    /// The paper is still: a time lies (t mod Length) / Length of the way
    /// from the start (the left across, the top down), laps starting at
    /// take time 0.
    Still,
    /// One lap of still paper, the one starting at this time, laid out
    /// without wrapping: lines and stretches are drawn a lap at a time,
    /// each clipped to where its lap shows ([`Geom::laps`]).
    Lap(f64),
}

/// The blank paper ahead of the pen, as a share of the strip (Hugh).
const GAP_SHARE: f32 = 0.04;

/// The least blank paper ahead of the pen, points (Hugh).
const GAP_MIN: f32 = 8.0;

/// How near the pen, as a share of the strip, a point counts as at it.
const PEN_SLACK: f32 = 1e-4;

/// Places points on the strip from (lead in ms, time in s).
#[derive(Debug, Clone, Copy)]
pub(super) struct Geom {
    pub r: Rect,
    pub half: f64,
    pub span: f64,
    pub end: f64,
    pub horizontal: bool,
    pub sweep: Sweep,
}

impl Geom {
    /// The strip's length along time, points.
    fn len(&self) -> f32 {
        if self.horizontal {
            self.r.width()
        } else {
            self.r.height()
        }
        .max(1.0)
    }

    /// How far along the strip `p` is from its start (left or top), as a
    /// share of its length.
    fn share_at(&self, p: Pos2) -> f64 {
        let r = self.r;
        f64::from(if self.horizontal {
            (p.x - r.left()) / self.len()
        } else {
            (p.y - r.top()) / self.len()
        })
    }

    /// Where the pen is on still paper, as a share of the strip from its
    /// start.
    fn pen(&self) -> f64 {
        (self.end / self.span).rem_euclid(1.0)
    }

    /// The blank paper ahead of the pen, as a share of the strip: none on
    /// moving paper.
    fn gap(&self) -> f64 {
        match self.sweep {
            Sweep::Moving => 0.0,
            _ => f64::from(GAP_SHARE.max(GAP_MIN / self.len())),
        }
    }

    /// The oldest time on show: a Length back from the newest, less the
    /// blank ahead of the pen.
    pub fn oldest(&self) -> f64 {
        self.end - self.span * (1.0 - self.gap())
    }

    /// The time at a point on the strip.
    pub fn time_at(&self, p: Pos2) -> f64 {
        let share = self.share_at(p);
        match self.sweep {
            Sweep::Moving => {
                let back = if self.horizontal { 1.0 - share } else { share };
                self.end - back * self.span
            }
            Sweep::Still => {
                // A hair past the pen (points are f32) is the pen, not a
                // whole lap back: the blank ahead of the pen keeps the two
                // apart.
                let back = (self.pen() - share).rem_euclid(1.0);
                let back = if back > 1.0 - f64::from(PEN_SLACK) {
                    back - 1.0
                } else {
                    back
                };
                self.end - back * self.span
            }
            Sweep::Lap(start) => start + share * self.span,
        }
    }

    pub fn pos(&self, ms: f64, t: f64) -> Pos2 {
        let share = match self.sweep {
            Sweep::Moving => {
                let back = (self.end - t) / self.span;
                if self.horizontal {
                    1.0 - back
                } else {
                    back
                }
            }
            Sweep::Still => (t / self.span).rem_euclid(1.0),
            Sweep::Lap(start) => (t - start) / self.span,
        } as f32;
        let (r, across) = (self.r, (ms / self.half) as f32);
        if self.horizontal {
            Pos2::new(
                r.left() + share * r.width(),
                r.center().y - across * r.height() / 2.0,
            )
        } else {
            Pos2::new(
                r.center().x + across * r.width() / 2.0,
                r.top() + share * r.height(),
            )
        }
    }

    /// The laps to draw lines and stretches over, oldest first, each with
    /// the stretch of the strip it shows on (shares from the start). On
    /// moving paper, the strip itself over the whole of it. On still
    /// paper, the last lap from just past the blank ahead of the pen to
    /// the far end, then this lap from the start up to the pen, so no
    /// line runs across the wrap or the blank.
    pub fn laps(&self) -> Vec<(Geom, (f64, f64))> {
        if self.sweep == Sweep::Moving {
            return vec![(*self, (0.0, 1.0))];
        }
        let (pen, gap) = (self.pen(), self.gap());
        let start = (self.end / self.span).floor() * self.span;
        let lap = |s: f64| Geom {
            sweep: Sweep::Lap(s),
            ..*self
        };
        let mut out = Vec::with_capacity(2);
        if pen + gap < 1.0 {
            out.push((lap(start - self.span), (pen + gap, 1.0)));
        }
        out.push((lap(start), ((pen + gap - 1.0).max(0.0), pen)));
        out
    }

    /// The stretch of the strip from share `a` to share `b` along time as
    /// a rectangle over `outer`, the area round the strip: an end at the
    /// strip's own end reaches out to `outer`'s, so labels and the track
    /// there aren't cut.
    pub fn clip(&self, outer: Rect, (a, b): (f64, f64)) -> Rect {
        let r = self.r;
        let (lo, hi, out_lo, out_hi) = if self.horizontal {
            (r.left(), r.right(), outer.left(), outer.right())
        } else {
            (r.top(), r.bottom(), outer.top(), outer.bottom())
        };
        let at = |s: f64| lo + s as f32 * (hi - lo);
        let from = if a <= 0.0 { out_lo } else { at(a) };
        let to = if b >= 1.0 { out_hi } else { at(b) };
        if self.horizontal {
            Rect::from_x_y_ranges(from..=to, outer.y_range())
        } else {
            Rect::from_x_y_ranges(outer.x_range(), from..=to)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Vec2;

    fn geom(horizontal: bool, sweep: Sweep, end: f64) -> Geom {
        Geom {
            r: Rect::from_min_size(Pos2::new(50.0, 20.0), Vec2::new(400.0, 300.0)),
            half: 10.0,
            span: 60.0,
            end,
            horizontal,
            sweep,
        }
    }

    /// Moving paper keeps the newest beat at its edge: the right lying
    /// across, the top standing down.
    #[test]
    fn moving_paper_puts_the_newest_beat_at_its_edge() {
        let g = geom(true, Sweep::Moving, 100.0);
        assert!((g.pos(0.0, 100.0).x - 450.0).abs() < 1e-3);
        assert!((g.pos(0.0, 40.0).x - 50.0).abs() < 1e-3);
        let g = geom(false, Sweep::Moving, 100.0);
        assert!((g.pos(0.0, 100.0).y - 20.0).abs() < 1e-3);
        assert!((g.pos(0.0, 40.0).y - 320.0).abs() < 1e-3);
        assert_eq!(g.oldest(), 40.0);
    }

    /// On still paper a time sits at its place in its lap whatever the
    /// pen's time, laps starting at take time 0; the pen moves, the paper
    /// doesn't.
    #[test]
    fn still_paper_puts_a_time_at_its_place_in_the_lap() {
        for horizontal in [true, false] {
            let along = |p: Pos2| if horizontal { p.x - 50.0 } else { p.y - 20.0 };
            let len = if horizontal { 400.0 } else { 300.0 };
            for end in [100.0, 115.0, 130.0] {
                let g = geom(horizontal, Sweep::Still, end);
                // 75 s is 15 s into the lap from 60 s: a quarter along.
                assert!((along(g.pos(0.0, 75.0)) - len / 4.0).abs() < 1e-3);
                // 100 s is 40 s into its lap: two thirds along.
                assert!((along(g.pos(0.0, 100.0)) - len * 2.0 / 3.0).abs() < 1e-3);
            }
        }
    }

    /// The time read back at a point is where that time is drawn, on every
    /// kind of paper, both ways round, either side of the pen.
    #[test]
    fn the_time_under_the_pointer_is_where_it_is_drawn() {
        for horizontal in [false, true] {
            for sweep in [Sweep::Moving, Sweep::Still, Sweep::Lap(60.0)] {
                let g = geom(horizontal, sweep, 100.0);
                for t in [45.0, 55.5, 61.0, 99.0, 100.0] {
                    let back = g.time_at(g.pos(3.0, t));
                    assert!((back - t).abs() < 1e-3, "{sweep:?} {t} {back}");
                }
            }
        }
    }

    /// Ahead of the pen is a blank 4% of the strip, at least 8 points: the
    /// oldest time on show is that much less than a Length back.
    #[test]
    fn the_blank_ahead_of_the_pen_is_four_percent_or_eight_points() {
        let g = geom(true, Sweep::Still, 100.0);
        assert!((g.oldest() - (100.0 - 60.0 * 0.96)).abs() < 1e-6);
        let narrow = Geom {
            r: Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 50.0)),
            ..g
        };
        assert!((narrow.oldest() - (100.0 - 60.0 * 0.92)).abs() < 1e-6);
    }

    /// The last lap shows from past the blank to the far end and this one
    /// from the start to the pen; together with the blank, the whole strip.
    #[test]
    fn the_laps_meet_at_the_pen_and_the_blank() {
        let g = geom(true, Sweep::Still, 100.0);
        let laps = g.laps();
        assert_eq!(laps.len(), 2);
        let pen = 40.0 / 60.0;
        assert_eq!(laps[0].0.sweep, Sweep::Lap(0.0));
        assert!((laps[0].1 .0 - (pen + 0.04)).abs() < 1e-6 && laps[0].1 .1 == 1.0);
        assert_eq!(laps[1].0.sweep, Sweep::Lap(60.0));
        assert!(laps[1].1 .0 == 0.0 && (laps[1].1 .1 - pen).abs() < 1e-6);
        // The pen just short of the end: the blank wraps to the start, and
        // only this lap shows, from past the blank.
        let g = geom(true, Sweep::Still, 119.4);
        let laps = g.laps();
        assert_eq!(laps.len(), 1);
        assert!((laps[0].1 .0 - (119.4 / 60.0 - 1.0 + 0.04 - 1.0)).abs() < 1e-6);
        // Moving paper is one stretch over the whole strip.
        assert_eq!(geom(true, Sweep::Moving, 100.0).laps()[0].1, (0.0, 1.0));
    }

    /// A lap's stretch clips at the share it shows from and to, but an end
    /// at the strip's own end reaches out to the area round it, so labels
    /// and the track there aren't cut; across the strip, the whole area.
    #[test]
    fn a_lap_clips_inside_the_strip_but_not_at_its_ends() {
        let outer = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(500.0, 400.0));
        let g = geom(true, Sweep::Still, 100.0);
        // Strip x from 50 to 450: a quarter along is 150, three quarters 350.
        let c = g.clip(outer, (0.0, 0.25));
        assert_eq!((c.left(), c.right()), (0.0, 150.0));
        assert_eq!(c.y_range(), outer.y_range());
        let c = g.clip(outer, (0.75, 1.0));
        assert_eq!((c.left(), c.right()), (350.0, 500.0));
        let c = g.clip(outer, (0.25, 0.75));
        assert_eq!((c.left(), c.right()), (150.0, 350.0));
        // Standing down: strip y from 20 to 320.
        let g = geom(false, Sweep::Still, 100.0);
        let c = g.clip(outer, (0.0, 0.5));
        assert_eq!((c.top(), c.bottom()), (0.0, 170.0));
        assert_eq!(c.x_range(), outer.x_range());
        let c = g.clip(outer, (0.5, 1.0));
        assert_eq!((c.top(), c.bottom()), (170.0, 400.0));
    }
}
