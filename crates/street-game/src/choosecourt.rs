//! Choose Court on the game's own layout (class 0x21 = 33, blob 10):
//! screen class 0x21's constructor `FUN_00267b00` (`FUN_002d7720(screen,
//! 0x21, 4)`, backdrop 4), build `FUN_00267f10`, update `FUN_002688f0`,
//! change `FUN_00268ce0`, input `FUN_002686e0`, confirm `FUN_00269610` and
//! the dialog's answer `FUN_002696a8`. The trace is in
//! `docs/FRONTEND_RECOVERY.md`, "Choose Court".
//!
//! The build registers twelve plain widgets (ids 0..11) on list 0x61 and
//! pairs them with its nodes in prop-34 order, so widget `k` sits on the
//! node of that number: three columns and four rows of tiles, 81 by 77
//! each (`XACOURTS` sheet `k`, image 0, frame 1), the twelfth the Random
//! Select tile (sheet 12). The court at the cursor shows on the right:
//! its name between two stars, its location, the big picture (frame 0,
//! 248 by 174) inside a 3-pixel frame, and "Locked" under a dimmed picture
//! when the court's reward card is not owned. The Random tile cycles the
//! big picture through the open courts four times a second
//! (`FUN_002688f0`, `rand() % 11` until an open court differs from the
//! last). Confirm on an open court stores it (`FUN_001dbc88`) and opens
//! the Play Game / My Rules dialog (`FUN_0024f508`, text 0xd05); cancel
//! returns to Choose Players (class 0x1d).

use bevy::prelude::*;
use street_sim::story::Story;

use crate::{
    art::Art,
    look::{self, Align, Look, Piece, fonts},
    setup::Disc,
};

/// The screen's backdrop (`FUN_002d7720(screen, 0x21, 4)`).
pub const BACKDROP: usize = 4;
/// The courts (`XDBCOURT` records 0..10) and the Random Select widget.
pub const COURTS: usize = 11;
pub const RANDOM: usize = 11;
pub const CELLS: usize = 12;
/// `XACOURTS` sheets: a court's own, then the Random tile's picture (12)
/// and the "?" shown for the locked court 10 (13; `FUN_00267f10`,
/// `FUN_00268ce0`).
pub const RANDOM_SHEET: usize = 12;
pub const UNKNOWN_SHEET: usize = 13;
/// The court whose tile, name and picture stay unknown until it is open
/// (Rucker Park '78).
pub const HIDDEN_COURT: usize = 10;

/// List 0x61's nodes by prop 34 (the widget they pair with), layout 33:
/// the tiles' top-left corners.
pub const NODES: [Vec2; CELLS] = [
    Vec2::new(230.0, 98.0),
    Vec2::new(147.0, 98.0),
    Vec2::new(66.0, 254.0),
    Vec2::new(66.0, 176.0),
    Vec2::new(147.0, 176.0),
    Vec2::new(230.0, 176.0),
    Vec2::new(66.0, 98.0),
    Vec2::new(147.0, 254.0),
    Vec2::new(230.0, 254.0),
    Vec2::new(66.0, 332.0),
    Vec2::new(147.0, 332.0),
    Vec2::new(230.0, 332.0),
];
/// The widget the list opens on (`FUN_0021f6a8(list, 0x5a)`: node 90,
/// prop 34 = 6, the top-left tile).
pub const FIRST: usize = 6;
/// A tile (frame 1 of a court sheet).
pub const TILE: Vec2 = Vec2::new(81.0, 77.0);
/// The big picture (element 0x15b3 at (321, 146), frame 0).
pub const PICTURE_AT: Vec2 = Vec2::new(321.0, 146.0);
pub const PICTURE: Vec2 = Vec2::new(248.0, 174.0);
/// The four 3-wide lines round the picture (elements 0x24..0x27,
/// `FUN_0021e350`, their ends in the constructor at +0x114..+0x150):
/// top, left, bottom, right.
pub const FRAME_LINES: [(Vec2, Vec2); 4] = [
    (Vec2::new(318.0, 144.0), Vec2::new(574.0, 144.0)),
    (Vec2::new(319.0, 144.0), Vec2::new(319.0, 323.0)),
    (Vec2::new(318.0, 323.0), Vec2::new(574.0, 323.0)),
    (Vec2::new(572.0, 144.0), Vec2::new(572.0, 323.0)),
];
pub const FRAME_WIDTH: f32 = 3.0;
/// The frame's colour (`FUN_00233e00(line, 0xff91adc0)`) and the locked
/// court's (`0xff3b45a2`).
const FRAME_OPEN: [u8; 4] = [0x91, 0xad, 0xc0, 0xff];
const FRAME_LOCKED: [u8; 4] = [0x3b, 0x45, 0xa2, 0xff];
/// The name (element 0x62: font 2, 0xffdcdcbb, left of and middle on
/// (338, 107)) and the location (0x60: font 1, 0xff7c83d4, (338, 128)).
const NAME_AT: Vec2 = Vec2::new(338.0, 107.0);
const LOCATION_AT: Vec2 = Vec2::new(338.0, 128.0);
const LOCATION: [u8; 4] = [0x7c, 0x83, 0xd4, 0xff];
/// The stars (elements 0x51 and 0x52, authored at y 106): the left one 20
/// left of the name, the right one 2 past its end (`FUN_00268ce0`).
const STAR_Y: f32 = 106.0;
const STAR_LEFT_GAP: f32 = 20.0;
const STAR_RIGHT_GAP: f32 = 2.0;
/// "Locked" under a locked court's picture (element 0x115d: font 5,
/// 0xffdcdcbb, centred on (450, 382)) and on a locked tile (elements
/// 0x28..0x32: font 1, 0xff838497 with a black shadow, right-aligned 12
/// in from the tile's right edge and 25 up from its bottom).
const LOCKED_AT: Vec2 = Vec2::new(450.0, 382.0);
const TILE_LOCKED_INSET: Vec2 = Vec2::new(12.0, 25.0);
const TILE_LOCKED: [u8; 4] = [0x83, 0x84, 0x97, 0xff];
/// How often the Random tile picks another court (`FUN_002688f0`: when
/// the frame count over 60 reaches `+0x10c`, 0.25).
pub const RANDOM_TICKS: u64 = 15;
/// A locked court's picture is disabled (`+0x114 (1, 1)`); the shade the
/// rewrite draws it in is its own.
const LOCKED_SHADE: f32 = 0.45;

/// Whether a court is open: its reward card (category 2, `XDBCARD`)
/// is owned, as `FUN_001d37b0` answers through `FUN_001e0398`; a court
/// no card names is open.
pub fn open(story: &Story, disc: &Disc, court: usize) -> bool {
    crate::rewards::court_unlocked(story, disc, court)
}

/// Which `XACOURTS` sheet a cell shows: a court's own, 13 for the hidden
/// court while locked, 12 for the Random tile.
pub fn sheet(cell: usize, open: bool) -> usize {
    match cell {
        RANDOM => RANDOM_SHEET,
        HIDDEN_COURT if !open => UNKNOWN_SHEET,
        c => c,
    }
}

/// The Random tile's court for a quarter-second window: a draw over the
/// eleven that lands on an open court other than `last` (`FUN_002688f0`
/// redraws until both hold); `roll` stands in for `rand()`.
pub fn random_court(
    is_open: impl Fn(usize) -> bool,
    last: Option<usize>,
    roll: u32,
) -> Option<usize> {
    let candidates: Vec<usize> = (0..COURTS)
        .filter(|&c| is_open(c) && last != Some(c))
        .collect();
    if candidates.is_empty() {
        return (0..COURTS).find(|&c| is_open(c));
    }
    // The original's rejection loop keeps rolling; the roll here picks
    // among the courts it could land on.
    Some(candidates[roll as usize % candidates.len()])
}

/// Which court the cell at `cursor` means: the court itself, or the Random
/// tile's current draw.
pub fn court_of(cursor: usize, random: Option<usize>) -> Option<usize> {
    if cursor == RANDOM {
        random
    } else {
        Some(cursor)
    }
}

/// The screen's pieces: `cursor` the highlighted cell, `random` the
/// court the Random tile currently shows, `is_open` whether a court is
/// unlocked.
#[allow(clippy::too_many_arguments)]
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: &Art,
    disc: &Disc,
    cursor: usize,
    random: Option<usize>,
    is_open: impl Fn(usize) -> bool,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(pm) = look.prematch.as_ref() else {
        return out;
    };
    let mut text = |out: &mut Vec<Piece>, font: usize, s: &str, colour, shadow, at: Vec2, align| {
        if let Some(p) = labels.text(look, images, font, s, colour, shadow, at, align) {
            out.push(p);
        }
    };
    // The tiles on their nodes, "Locked" on a locked court's.
    for (cell, &node) in NODES.iter().enumerate() {
        let court_open = cell == RANDOM || is_open(cell);
        if let Some(tile) = art
            .courts
            .get(sheet(cell, court_open))
            .and_then(|c| c.as_ref())
        {
            out.push(Piece::new(
                tile.tile.clone(),
                Rect::from_corners(node, node + TILE),
            ));
        }
        if !court_open {
            let label = look.text_id(0x012b).unwrap_or("Locked");
            let line = look
                .fonts
                .get(fonts::LIST_SMALL)
                .map_or(19.0, |f| f.text_height as f32);
            let _ = line;
            text(
                &mut out,
                fonts::LIST_SMALL,
                label,
                TILE_LOCKED,
                Some(([0, 0, 0, 0xff], [0, 0, 0, 0xff])),
                node + TILE - TILE_LOCKED_INSET,
                Align::Right,
            );
        }
    }
    // The cursor tag (element 0x15b4) on the highlighted tile.
    if let Some(&node) = NODES.get(cursor) {
        let mut p = Piece::new(
            pm.cursor_court.clone(),
            Rect::from_corners(node, node + pm.cursor_court_size),
        );
        p.colour = Color::WHITE.with_alpha(f32::from(look::row_pulse(ticks)) / 255.0);
        out.push(p);
    }
    // The court shown on the right.
    let shown = court_of(cursor, random);
    let shown_open = shown.is_some_and(&is_open);
    let hidden = shown == Some(HIDDEN_COURT) && !shown_open;
    let (name, location): (String, Option<String>) = if cursor == RANDOM {
        (
            look.text_id(0x0d08).unwrap_or("Random Select").to_owned(),
            None,
        )
    } else if hidden {
        ("?????".to_owned(), None)
    } else {
        let court = shown.and_then(|c| disc.courts.get(c));
        (
            court.map(|c| c.name.clone()).unwrap_or_default(),
            court.map(|c| c.location.clone()),
        )
    };
    let name_font = look.fonts.get(fonts::LIST);
    let name_height = name_font.map_or(21.0, |f| f.text_height as f32);
    let name_width = name_font.map_or(0.0, |f| f.measure(&name) as f32);
    text(
        &mut out,
        fonts::LIST,
        &name,
        look::colours::CREAM,
        None,
        NAME_AT - Vec2::new(0.0, name_height / 2.0),
        Align::Left,
    );
    let star_middle = STAR_Y - pm.name_star_size.y / 2.0;
    for x in [
        NAME_AT.x - STAR_LEFT_GAP,
        NAME_AT.x + name_width + STAR_RIGHT_GAP,
    ] {
        let min = Vec2::new(x, star_middle);
        out.push(Piece::new(
            pm.name_star.clone(),
            Rect::from_corners(min, min + pm.name_star_size),
        ));
    }
    if let Some(location) = location {
        let height = look
            .fonts
            .get(fonts::LIST_SMALL)
            .map_or(19.0, |f| f.text_height as f32);
        text(
            &mut out,
            fonts::LIST_SMALL,
            &location,
            LOCATION,
            None,
            LOCATION_AT - Vec2::new(0.0, height / 2.0),
            Align::Left,
        );
    }
    // The big picture in its frame; a locked court's dimmed and labelled.
    let picture_sheet = match shown {
        Some(c) => sheet(c, shown_open),
        None => RANDOM_SHEET,
    };
    let locked = cursor != RANDOM && !shown_open;
    if let Some(picture) = art
        .courts
        .get(picture_sheet)
        .and_then(|c| c.as_ref())
        .and_then(|c| c.picture.clone())
    {
        let mut p = Piece::new(
            picture,
            Rect::from_corners(PICTURE_AT, PICTURE_AT + PICTURE),
        );
        if locked {
            p.colour = Color::srgb(LOCKED_SHADE, LOCKED_SHADE, LOCKED_SHADE);
        }
        out.push(p);
    }
    let [r, g, b, _] = if locked { FRAME_LOCKED } else { FRAME_OPEN };
    for (from, to) in FRAME_LINES {
        let half = FRAME_WIDTH / 2.0;
        let rect = if from.y == to.y {
            Rect::new(from.x, from.y - half, to.x, to.y + half)
        } else {
            Rect::new(from.x - half, from.y, to.x + half, to.y)
        };
        let mut p = Piece::new(Handle::default(), rect);
        p.colour = Color::srgb_u8(r, g, b);
        out.push(p);
    }
    if locked {
        text(
            &mut out,
            fonts::TITLE_SMALL,
            look.text_id(0x0d07).unwrap_or("Locked"),
            look::colours::CREAM,
            None,
            LOCKED_AT,
            Align::Centre,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tiles_fill_three_columns_and_four_rows_in_registration_order() {
        // Widget k pairs with the node of prop 34 = k (`FUN_0021f230`), so
        // the courts read down the grid in no simple order: the top row is
        // courts 6, 1, 0; the bottom 9, 10 and the Random tile.
        let by_row = |y: f32| -> Vec<usize> {
            let mut cells: Vec<usize> = (0..CELLS).filter(|&k| NODES[k].y == y).collect();
            cells.sort_by(|&a, &b| NODES[a].x.partial_cmp(&NODES[b].x).unwrap());
            cells
        };
        assert_eq!(by_row(98.0), vec![6, 1, 0]);
        assert_eq!(by_row(176.0), vec![3, 4, 5]);
        assert_eq!(by_row(254.0), vec![2, 7, 8]);
        assert_eq!(by_row(332.0), vec![9, 10, RANDOM]);
        assert_eq!(NODES[FIRST], Vec2::new(66.0, 98.0));
        // The columns are a tile apart, the rows a tile and one more.
        assert_eq!(NODES[1].x - NODES[6].x, TILE.x);
        assert_eq!(NODES[3].y - NODES[6].y, TILE.y + 1.0);
    }

    #[test]
    fn the_hidden_court_and_the_random_tile_take_the_extra_sheets() {
        assert_eq!(sheet(0, true), 0);
        assert_eq!(sheet(0, false), 0);
        assert_eq!(sheet(HIDDEN_COURT, false), UNKNOWN_SHEET);
        assert_eq!(sheet(HIDDEN_COURT, true), HIDDEN_COURT);
        assert_eq!(sheet(RANDOM, true), RANDOM_SHEET);
    }

    #[test]
    fn the_random_tile_draws_an_open_court_other_than_the_last() {
        let open = |c: usize| matches!(c, 0 | 1 | 6);
        assert_eq!(random_court(open, Some(1), 0), Some(0));
        assert_eq!(random_court(open, Some(1), 1), Some(6));
        assert_eq!(random_court(open, Some(0), 0), Some(1));
        assert_eq!(random_court(open, None, 2), Some(6));
        // One open court: it stays.
        assert_eq!(random_court(|c| c == 4, Some(4), 7), Some(4));
        assert_eq!(random_court(|_| false, None, 7), None);
        assert_eq!(court_of(RANDOM, Some(6)), Some(6));
        assert_eq!(court_of(3, Some(6)), Some(3));
    }

    #[test]
    fn the_frame_lines_box_the_picture_three_wide() {
        for (from, to) in FRAME_LINES {
            assert!(from.x == to.x || from.y == to.y);
        }
        assert!(FRAME_LINES[0].0.y < PICTURE_AT.y);
        assert!(FRAME_LINES[2].0.y > PICTURE_AT.y + PICTURE.y);
        assert!(FRAME_LINES[1].0.x < PICTURE_AT.x);
        assert!(FRAME_LINES[3].0.x > PICTURE_AT.x + PICTURE.x);
        assert_eq!(FRAME_WIDTH, 3.0);
    }
}
