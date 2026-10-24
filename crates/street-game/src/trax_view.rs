//! Options → Trax (class 0x30, layout 48, backdrop 0x1c;
//! `FUN_00265848`, "BIG RADIO").
//!
//! The screen's build (`FUN_002658c0`) makes one widget, the radio
//! (`FUN_00218e68(.., 0, ..)`), which lays its rows out of the table at
//! `0x467710` (eight shorts a row: the label's text, the kind, the bit,
//! the two choices' texts and the help): Music Experience (kind 1, option
//! bit 0, Playlist or Interactive), the nine Playlist songs (kind 6, bits
//! 3..11), the fifteen Interactive ones (kind 5, bits 12..26), Song
//! Rotation (kind 2, bit 1, Sequential or Random) and Exit (kind 0).
//! `FUN_00219da8` keeps on the list only the songs of the experience
//! chosen (Playlist the kind-6 ones, Interactive the kind-5 ones); the
//! table has no Track Info row (the other table, `0x4678d0`, which this
//! screen does not use, has).
//!
//! The radio's list shows five rows from (92, 128), 52 apart: the label
//! left at x 92 (songs in font 1 on two lines, title and artist; the other
//! rows in font 2), the choice right-anchored at 510 (font 2), both on the
//! row's middle. The focused row turns gold, its choice between the
//! list's arrows: the option widget is 40 wider than its wider choice
//! (`FUN_0023ef70`), its left arrow 18 left of that choice's width
//! (`FUN_0023efa8`). Left and right change the focused row's choice;
//! Cross is Preview, Triangle cancels (asking "Do you want to cancel your
//! changes?", `bigradio.txt` 47, when something changed), Start keeps
//! them (`FUN_00265978`). The help line is the row's (`FUN_00219bf0`:
//! `bigradio.txt` 40, 39, 41, 45).

use bevy::prelude::*;
use street_sim::records::{FRONT_END_BIT, Settings};

use crate::{
    choose::place,
    look::{self, Look, Piece, colours},
};

/// The screen's layout and backdrop (`FUN_002d7720(.., 0x30, 0x1c)`).
pub const CLASS: u32 = 48;
pub const BACKDROP: usize = 0x1c;
/// Rows the radio's list shows.
pub const SHOWN: usize = 5;

/// The rows' first line and spacing, and the two columns
/// (`FUN_00218e68`'s nodes: 92 and 510, 128 + 52k).
const FIRST_Y: f32 = 128.0;
const STEP: f32 = 52.0;
const LABEL_X: f32 = 92.0;
const VALUE_X: f32 = 510.0;
/// The option widget's arrows: the left one this far left of its wider
/// choice's left edge, the right one this far right of the column
/// (measured on `ea_trax.png` and `ea_trax_row2.png`).
const ARROW_IN: f32 = 18.0;
const ARROW_OUT: f32 = 20.0;
/// The list's "more" arrows (the down one measured on `ea_trax.png`; the
/// up one at the list's top, layout 48's list 6 at (76, 101)).
const MORE_UP: Vec2 = Vec2::new(78.0, 105.0);
const MORE_DOWN: Vec2 = Vec2::new(78.0, 366.0);

/// A row of the radio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Experience,
    /// A song: its label in `bigradio.txt` and its bit in the rewrite's
    /// `songs_off` (the front end's songs at [`FRONT_END_BIT`] on, the
    /// in-game ones from 0; `original_save` maps the original's bits).
    Song {
        label: usize,
        bit: u32,
    },
    Rotation,
    Exit,
}

/// `0x467710`'s kind-6 rows (Playlist): the labels (`bigradio.txt` 11, 10,
/// 12..17, 20) and the rewrite's front-end songs they are
/// (`story::FRONT_END_ORDER`).
const PLAYLIST: [(usize, u32); 9] = [
    (11, 1),
    (10, 0),
    (12, 2),
    (13, 3),
    (14, 4),
    (15, 5),
    (16, 6),
    (17, 7),
    (20, 8),
];
/// The kind-5 rows (Interactive): the labels (21..33, 36, 37), the
/// in-game songs 0..14 in `PATHSONG.CFG`'s order.
const INTERACTIVE: [usize; 15] = [21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 36, 37];

/// The rows the list holds for the experience chosen (`FUN_00219da8`).
pub fn rows(o: &Settings) -> Vec<Row> {
    let mut out = vec![Row::Experience];
    if o.interactive {
        out.extend(INTERACTIVE.iter().enumerate().map(|(k, &label)| Row::Song {
            label,
            bit: k as u32,
        }));
    } else {
        out.extend(PLAYLIST.iter().map(|&(label, k)| Row::Song {
            label,
            bit: FRONT_END_BIT + k,
        }));
    }
    out.push(Row::Rotation);
    out.push(Row::Exit);
    out
}

/// Left or right on a row: its other choice (each row has two).
pub fn step(o: &mut Settings, row: Row) {
    match row {
        Row::Experience => o.interactive = !o.interactive,
        Row::Song { bit, .. } => o.songs_off ^= 1 << bit,
        Row::Rotation => o.sequential = !o.sequential,
        Row::Exit => {}
    }
}

/// The row's help line in `bigradio.txt` (the table's last short).
pub fn help(row: Row) -> usize {
    match row {
        Row::Experience => 40,
        Row::Song { .. } => 39,
        Row::Rotation => 41,
        Row::Exit => 45,
    }
}

/// The row's label.
fn label(look: &Look, row: Row) -> (String, usize) {
    let text = |k: usize, or: &str| look.text("bigradio.txt", k).unwrap_or(or).to_owned();
    match row {
        Row::Experience => (text(2, "Music Experience"), 2),
        Row::Song { label, .. } => (text(label, "Song"), 1),
        Row::Rotation => (text(3, "Song Rotation"), 2),
        Row::Exit => (look.text("common.txt", 19).unwrap_or("Exit").to_owned(), 2),
    }
}

/// The row's two choices (the one shown first) and which is shown.
fn choices(look: &Look, o: &Settings, row: Row) -> Option<([String; 2], usize)> {
    let radio = |k: usize, or: &str| look.text("bigradio.txt", k).unwrap_or(or).to_owned();
    let common = |k: usize, or: &str| look.text("common.txt", k).unwrap_or(or).to_owned();
    Some(match row {
        Row::Experience => (
            [radio(8, "Playlist"), radio(9, "Interactive")],
            usize::from(o.interactive),
        ),
        Row::Song { bit, .. } => (
            [common(17, "On"), common(18, "Off")],
            usize::from(o.songs_off & (1 << bit) != 0),
        ),
        Row::Rotation => (
            [radio(7, "Sequential"), radio(6, "Random")],
            usize::from(!o.sequential),
        ),
        Row::Exit => return None,
    })
}

/// The screen's pieces: five rows from `top`, the focused one lit.
#[allow(clippy::too_many_arguments)]
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    o: &Settings,
    cursor: usize,
    top: usize,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let all = rows(o);
    let mut lit_colour = colours::ROW_LIT;
    lit_colour[3] = look::row_pulse(ticks);
    for (k, &row) in all.iter().enumerate().skip(top).take(SHOWN) {
        let y = FIRST_Y + (k - top) as f32 * STEP;
        let lit = k == cursor;
        let colour = if lit { lit_colour } else { colours::ROW };
        let (words, font) = label(look, row);
        let lines: Vec<&str> = words.split("\\n").map(str::trim).collect();
        let height = look.fonts.get(font).map_or(20.0, |f| f.line_height as f32);
        let block = height * (lines.len() as f32 - 1.0);
        for (j, line) in lines.iter().enumerate() {
            let at = Vec2::new(LABEL_X, y - block / 2.0 + j as f32 * height);
            place(&mut out, labels, look, images, font, line, colour, at, 4);
        }
        let Some((both, shown)) = choices(look, o, row) else {
            continue;
        };
        place(
            &mut out,
            labels,
            look,
            images,
            2,
            &both[shown],
            colour,
            Vec2::new(VALUE_X, y),
            2 | 4,
        );
        if lit && let (Some(pm), Some(f)) = (look.prematch.as_ref(), look.fonts.get(2)) {
            let wide = both.iter().map(|s| f.measure(s) as f32).fold(0.0, f32::max);
            for (image, x) in [
                (&pm.arrow_left, VALUE_X - wide - ARROW_IN),
                (&pm.arrow_right, VALUE_X + ARROW_OUT),
            ] {
                out.push(Piece::new(
                    image.clone(),
                    Rect::from_center_size(Vec2::new(x, y), pm.arrow_side_size),
                ));
            }
        }
    }
    if let Some(pm) = look.prematch.as_ref() {
        if top > 0 {
            out.push(Piece::new(
                pm.arrow_up.clone(),
                Rect::from_center_size(MORE_UP, pm.arrow_size),
            ));
        }
        if top + SHOWN < all.len() {
            out.push(Piece::new(
                pm.arrow_down.clone(),
                Rect::from_center_size(MORE_DOWN, pm.arrow_size),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_holds_the_chosen_experiences_songs() {
        let mut o = Settings {
            interactive: true,
            ..Default::default()
        };
        let r = rows(&o);
        // Music Experience, fifteen in-game songs, Song Rotation, Exit.
        assert_eq!(r.len(), 18);
        assert_eq!(r[1], Row::Song { label: 21, bit: 0 });
        assert_eq!(r[16], Row::Rotation);
        o.interactive = false;
        let r = rows(&o);
        assert_eq!(r.len(), 12);
        // Nelly's first (`0x467710`'s kind-6 rows), Pete Rock second.
        assert_eq!(
            r[1],
            Row::Song {
                label: 11,
                bit: FRONT_END_BIT + 1
            }
        );
        assert!(!r.contains(&Row::Exit) || r.last() == Some(&Row::Exit));
    }

    #[test]
    fn left_and_right_change_the_rows_choice() {
        let mut o = Settings::default();
        let before = o.interactive;
        step(&mut o, Row::Experience);
        assert_ne!(o.interactive, before);
        step(&mut o, Row::Song { label: 21, bit: 3 });
        assert_eq!(o.songs_off & 8, 8 & !Settings::default().songs_off);
        assert_eq!(help(Row::Rotation), 41);
    }
}
