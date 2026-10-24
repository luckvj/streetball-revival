//! Update Team (screen class 0x60, layout blob 54, backdrop 5): the
//! constructor `FUN_002d61a0`, the build `FUN_002d6248`, the input
//! `FUN_002d6c00`, the dialogs' answers `FUN_002d6fd0` and the read-outs
//! `FUN_002d66c0`. The trace is in `docs/FRONTEND_RECOVERY.md`, "Update
//! Team".
//!
//! After a won story game: two roster widgets (`FUN_002325e8`, five
//! portrait tiles two across, every tile at 0x9f and the focused one at
//! full): the career's team on the left (widget 0x55, at (113, 220); the
//! baller's tile at 0x3f, `FUN_00232e18`, as it keeps its place) and the
//! beaten team's on the right (0x56, at (457, 220)). The question comes
//! first (0x3d04, dialog 0x57, on Yes); then the beaten team's list takes
//! the focus ("Select a player to add to your team.", 0x3d05) and Cross
//! moves to the career's ("Select a roster position for your new
//! player.", 0x3d06), where Cross asks "Accept roster changes and
//! continue?" (0x3d02, dialog 0x59, on No) and Triangle goes back to the
//! beaten team; Triangle there asks "Exit without making changes?"
//! (0x3d01, 0x58, on No). While the beaten team's list is picked from the
//! career's read-out is blank. "Cancel Update" (0x3d08) and Triangle's
//! icon sit over the right panel (elements 3 and 2). The panels keep the
//! layout's palettes (prop 8: 1P's red on the left, 3P's blue on the
//! right) and their tabs no words.

use bevy::prelude::*;

use crate::{
    choose::{self, Tile},
    look::{self, Look, Piece, fonts},
    portraits::Portraits,
    setup::Disc,
};

pub const CLASS: u32 = 96;
/// The roster widgets' first tiles' middles (`FUN_002d6248`: 113, 220 and
/// 457, 220) and their five tiles.
const ORIGIN: [Vec2; 2] = [Vec2::new(113.0, 220.0), Vec2::new(457.0, 220.0)];
pub const SLOTS: usize = 5;
/// The read-outs' elements by side (`0x494620`, `0x49464c`) and their
/// meters' words.
const READOUT: [[u32; 11]; 2] = [
    [46, 47, 45, 44, 13, 6, 10, 5, 15, 7, 11],
    [50, 51, 49, 48, 33, 26, 30, 25, 35, 27, 31],
];
const LABELS: [[u32; 7]; 2] = [[21, 17, 19, 16, 22, 18, 20], [41, 37, 39, 36, 42, 38, 40]];
const FRAME: [[u32; 4]; 2] = [[53, 28, 8, 0], [54, 32, 12, 1]];
const CORNER: [u32; 2] = [14, 34];
const TAB: [u32; 2] = [23, 43];
/// "Cancel Update" (element 3, font 0) and its icon (element 2).
const CANCEL: u32 = 3;
const CANCEL_ICON: u32 = 2;

/// Which list has the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// A dialog is up.
    None,
    /// The beaten team's (`+0x104` 1).
    Beaten,
    /// The career's roster (`+0x104` 0).
    Roster,
}

/// What the screen shows.
pub struct View<'a> {
    pub roster: &'a [usize],
    pub beaten: &'a [usize],
    pub focus: Focus,
    /// The cursor on each list.
    pub roster_at: usize,
    pub beaten_at: usize,
}

/// The tiles' brightness on a list: 0x9f, the focused one full, the
/// career's baller 0x3f.
fn bright(roster: bool, index: usize, focused: bool) -> f32 {
    if roster && index == 0 {
        choose::EMPTY
    } else if focused {
        choose::FOCUSED
    } else {
        choose::UNFOCUSED
    }
}

/// The screen's pieces over its layout (the title, the help line and the
/// Help legend come with it).
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    portraits: Option<&mut Portraits>,
    disc: &Disc,
    view: &View,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let (Some(layout), Some(pm)) = (
        look.layouts.iter().find(|l| l.class == CLASS),
        look.prematch.as_ref(),
    ) else {
        return out;
    };
    let mut portraits = portraits;
    for side in 0..2 {
        // The layout's palette (prop 8 of the lines: 0 and 2).
        let variant = choose::element(layout, FRAME[side][0])
            .and_then(|e| e.props.get(&8))
            .map_or(0, |&k| k as usize + 1);
        choose::frame(
            &mut out,
            layout,
            pm,
            &FRAME[side],
            CORNER[side],
            TAB[side],
            variant,
        );
        let (list, at, focused) = if side == 0 {
            (view.roster, view.roster_at, view.focus == Focus::Roster)
        } else {
            (view.beaten, view.beaten_at, view.focus == Focus::Beaten)
        };
        let tiles: Vec<Tile> = (0..SLOTS)
            .map(|k| list.get(k).map_or(Tile::Empty, |&r| Tile::Player(r)))
            .collect();
        // `FUN_002d66c0`: the focused tile's player; the career's side is
        // blank while the beaten team's list is picked from.
        let shown = if side == 0 && view.focus != Focus::Roster {
            None
        } else {
            tiles.get(at).copied().filter(|t| *t != Tile::Empty)
        };
        choose::readout(
            &mut out,
            look,
            labels,
            images,
            layout,
            pm,
            disc,
            (&READOUT[side], &LABELS[side]),
            shown,
        );
        for (k, &tile) in tiles.iter().enumerate() {
            let centre = ORIGIN[side]
                + Vec2::new(
                    (k % 2) as f32 * pm.tile_size.x,
                    (k / 2) as f32 * pm.tile_size.y,
                );
            choose::tile_pieces(
                &mut out,
                images,
                portraits.as_deref_mut(),
                pm,
                disc,
                centre,
                (tile, false),
                bright(side == 0, k, focused && k == at),
            );
        }
        if focused {
            let centre = ORIGIN[side]
                + Vec2::new(
                    (at % 2) as f32 * pm.tile_size.x,
                    (at / 2) as f32 * pm.tile_size.y,
                );
            out.push(choose::cursor_piece(pm, centre, ticks));
        }
    }
    // Cancel Update (0x3d08) after Triangle's icon.
    if let Some(e) = choose::element(layout, CANCEL)
        && let Some(at) = choose::spot(e)
    {
        let word = look.text("updateteam.txt", 8).unwrap_or("Cancel Update");
        let colour = e
            .props
            .get(&10)
            .map_or(look::colours::PALE, |&c| street_assets::font::argb(c));
        let font = e.props.get(&13).map_or(fonts::SMALL, |&f| f as usize);
        choose::place(
            &mut out, labels, look, images, font, word, colour, at, e.align,
        );
    }
    if let (Some(e), Some((atlas, frames))) = (choose::element(layout, CANCEL_ICON), &look.icons)
        && let Some(at) = choose::spot(e)
        && let Some(icon) = look::legend_icon(1).and_then(|k| frames.get(k))
    {
        // Centred on its point (kind 29, anchor 5).
        let min = at - icon.size() / 2.0;
        out.push(Piece::part(
            atlas.clone(),
            *icon,
            Rect::from_corners(min, min + icon.size()),
        ));
    }
    out
}

/// A step on a five-tile roster widget: across in reading order, up and
/// down a row; a step off the tiles stays.
pub fn step(at: usize, dx: i32, dy: i32, count: usize) -> usize {
    let to = match (dx.signum(), dy.signum()) {
        (1, 0) => Some(at + 1),
        (-1, 0) => at.checked_sub(1),
        (0, 1) => Some(at + 2),
        (0, -1) => at.checked_sub(2),
        _ => None,
    };
    to.filter(|&t| t < count.min(SLOTS)).unwrap_or(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_roster_widget_steps_two_across() {
        assert_eq!(step(0, 1, 0, 5), 1);
        assert_eq!(step(1, 1, 0, 5), 2, "on to the next row");
        assert_eq!(step(0, -1, 0, 5), 0);
        assert_eq!(step(0, 0, 1, 5), 2);
        assert_eq!(step(2, 0, 1, 5), 4);
        assert_eq!(step(3, 0, 1, 5), 3, "no sixth tile");
        assert_eq!(step(4, 0, -1, 5), 2);
        assert_eq!(step(1, 0, 1, 3), 1, "a list of three");
    }

    #[test]
    fn the_baller_stays_dark_and_the_focus_lights() {
        assert_eq!(bright(true, 0, true), choose::EMPTY);
        assert_eq!(bright(true, 1, true), choose::FOCUSED);
        assert_eq!(bright(false, 0, false), choose::UNFOCUSED);
    }
}
