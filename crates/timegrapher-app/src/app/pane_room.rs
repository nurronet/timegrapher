//! Room for each pane: the least each one on show keeps when the panes
//! share a row or a column.

use super::*;

/// Give every pane on show at least [`MIN_PANE_H`] (or [`MIN_PANE_W`]
/// across), or the height `need` says its tab on show needs if more,
/// taken from the panes with room to spare, so opening a pane never
/// crushes the others into a sliver. Worked from the sizes of the last
/// frame; a window too small for them all shares its room in proportion.
pub(super) fn keep_room(
    tiles: &mut Tiles<Pane>,
    need: impl Fn(Pane) -> Option<f32>,
    dragged: &std::collections::HashSet<TileId>,
) {
    let strip_row = strip_row(tiles);
    let ids: Vec<TileId> = tiles.tile_ids().collect();
    for id in ids {
        // A divider the user dragged stands where they left it.
        if dragged.contains(&id) {
            continue;
        }
        let Some(rect) = tiles.rect(id) else { continue };
        let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(l))) = tiles.get(id)
        else {
            continue;
        };
        let shown: Vec<TileId> = l
            .children
            .iter()
            .copied()
            .filter(|&c| tiles.is_visible(c))
            .collect();
        let n = shown.len();
        let vertical = l.dir == LinearDir::Vertical;
        let room = if vertical {
            rect.height()
        } else {
            rect.width()
        };
        let room = room - 8.0 * (n as f32 - 1.0);
        if n < 2 || room <= 0.0 {
            continue;
        }
        let mut least = child_leasts(tiles, &shown, vertical, &need);
        squeeze_leasts(&mut least, &shown, &strip_row, room);
        let least: Vec<f32> = least.iter().map(|l| l / room).collect();
        let share: Vec<f32> = shown.iter().map(|&c| l.shares[c]).collect();
        let total: f32 = share.iter().sum();
        let Some(fracs) = raised_shares(&share, &least) else {
            continue;
        };
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(l))) =
            tiles.get_mut(id)
        {
            for (&c, f) in shown.iter().zip(fracs) {
                l.shares.set_share(c, f * total);
            }
        }
    }
}

/// The Paper Strip's row, whose least is kept first: its tile and every
/// container it sits in.
fn strip_row(tiles: &Tiles<Pane>) -> Vec<TileId> {
    tiles
        .find_pane(&Pane::Strip)
        .map(|mut id| {
            let mut up = vec![id];
            while let Some(p) = tiles.parent_of(id) {
                up.push(p);
                id = p;
            }
            up
        })
        .unwrap_or_default()
}

/// Each of `shown`'s least, in points: a tab bar's panes may need more
/// than the usual height, under its tab bar. The most any of its tabs
/// needs, not the one on show, so changing tabs never moves the divider.
fn child_leasts(
    tiles: &Tiles<Pane>,
    shown: &[TileId],
    vertical: bool,
    need: &impl Fn(Pane) -> Option<f32>,
) -> Vec<f32> {
    shown
        .iter()
        .map(|&c| {
            if !vertical {
                return MIN_PANE_W;
            }
            let more = match tiles.get(c) {
                Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(t))) => t
                    .children
                    .iter()
                    .filter(|&&a| tiles.is_visible(a))
                    .filter_map(|&a| match tiles.get(a) {
                        Some(egui_tiles::Tile::Pane(p)) => need(*p),
                        _ => None,
                    })
                    .reduce(f32::max)
                    .map(|h| h + TAB_BAR_H),
                _ => None,
            };
            more.unwrap_or(0.0).max(MIN_PANE_H)
        })
        .collect()
}

/// A window too small for every `least` in `room` keeps the Paper
/// Strip's least, and shares the rest out in proportion.
fn squeeze_leasts(least: &mut [f32], shown: &[TileId], strip_row: &[TileId], room: f32) {
    let sum: f32 = least.iter().sum();
    if sum <= room {
        return;
    }
    let first: f32 = shown
        .iter()
        .zip(least.iter())
        .filter(|(c, _)| strip_row.contains(c))
        .map(|(_, l)| *l)
        .sum();
    let (keep, k) = if first < room && sum > first {
        (first, (room - first) / (sum - first))
    } else {
        (0.0, room / sum)
    };
    for (c, l) in shown.iter().zip(least.iter_mut()) {
        if keep == 0.0 || !strip_row.contains(c) {
            *l *= k;
        }
    }
}

/// Each child's part of the room, from its `share` and its `least` as a
/// part of the room: raise the smallest to their least, then share the
/// rest out as before, until none of the rest falls under theirs. None
/// when none needed raising.
fn raised_shares(share: &[f32], least: &[f32]) -> Option<Vec<f32>> {
    let n = share.len();
    let mut small = vec![false; n];
    let rest_and_left = |small: &[bool]| {
        let rest: f32 = (0..n).filter(|&i| !small[i]).map(|i| share[i]).sum();
        let left = 1.0 - (0..n).filter(|&i| small[i]).map(|i| least[i]).sum::<f32>();
        (rest, left)
    };
    loop {
        let (rest, left) = rest_and_left(&small);
        let under: Vec<usize> = (0..n)
            .filter(|&i| !small[i] && share[i] / rest * left < least[i])
            .collect();
        if under.is_empty() {
            break;
        }
        for i in under {
            small[i] = true;
        }
    }
    if !small.contains(&true) {
        return None;
    }
    let (rest, left) = rest_and_left(&small);
    Some(
        (0..n)
            .map(|i| {
                if small[i] {
                    least[i]
                } else {
                    share[i] / rest * left
                }
            })
            .collect(),
    )
}
