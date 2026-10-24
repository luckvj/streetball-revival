//! The completion boards on their layouts: the League Challenge's (screen
//! class 0x3b, `FUN_002a46b8`: `FUN_002d7720(screen, 0x3b, 0x14)`, layout
//! 59, backdrop 20) and Be A Legend's (class 0x3c, `FUN_00261c48`:
//! `FUN_002d7720(screen, 0x3c, 0x10)`, layout 60, backdrop 16, filled by
//! `FUN_00261cc0`). Neither was grabbed from the original (no save state
//! reaches them), so they are built from the constructors alone.
//!
//! - The League Challenge's: "Congratulations!" (`nbatcend.txt` 0) as the
//!   title, "You have completed / League Challenge Mode." (1) and "%1d Reward
//!   Points" (2, element 1) of the reward (`setup + 0x1c0c`, the results'
//!   first entry) between two stars (elements 2, 3: `XACOMMON` sheet 0,
//!   image 1, frame 0), and Continue. Cross restarts the challenge
//!   (`FUN_001de8d0`) and leaves (`FUN_002d8460(.., 0x3f)`; to class 0x4d
//!   when the challenge's team holds a created player, `+0xf0`).
//! - Be A Legend's: "Be A Legend" (`bealegendend.txt` 10) as the title,
//!   "Congratulations!" (0, font 5) and "You have completed / Be A Legend
//!   Mode." (1, font 2); the baller's name on element 0 (the first name
//!   alone in font 3 when the last is empty, else "first last" in font 1)
//!   with a star 14 beyond each end (`FUN_00262100`); "Win / Loss Record:"
//!   (4) over "%1d - %2d" (7, element 3) of the career's wins and losses
//!   (`+0x28`, `+0x2c`); "Reputation Level:" (5) over its name
//!   (`reputation.txt` 1..5 by `FUN_001d83c8`, element 1); "Street
//!   Challenges:" (6) over "%1d / %2d COMPLETE" (9, element 2) of the
//!   street challenges cleared; Continue. Cross marks the career complete
//!   (`+0xf28 | 2`) and leaves.

use bevy::prelude::*;
use street_assets::font::Layout;

use crate::{
    choose::{element, place, spot},
    look::{self, Look, Piece, colours, fonts},
};

pub const LEAGUE_CLASS: u32 = 59;
pub const LEAGUE_BACKDROP: usize = 20;
pub const LEGEND_CLASS: u32 = 60;
pub const LEGEND_BACKDROP: usize = 16;

/// The screen class for a completion (0 the League Challenge, 1 Be A Legend).
pub fn class_of(mode: usize) -> u32 {
    if mode == 0 {
        LEAGUE_CLASS
    } else {
        LEGEND_CLASS
    }
}

pub fn backdrop_of(mode: usize) -> usize {
    if mode == 0 {
        LEAGUE_BACKDROP
    } else {
        LEGEND_BACKDROP
    }
}

/// What the boards read out.
pub struct Board {
    /// The League Challenge's reward.
    pub reward_points: u32,
    /// Be A Legend's baller: first and last names.
    pub baller: (String, String),
    pub wins: u32,
    pub losses: u32,
    /// The reputation level, 1..5 (`reputation.txt`).
    pub reputation: u8,
    /// Street challenges cleared, of all.
    pub streets: (usize, usize),
}

/// A layout's static words (kind 4 with a text id), each by its element's
/// font, colour and anchor.
fn static_words(
    out: &mut Vec<Piece>,
    labels: &mut look::Labels,
    look: &Look,
    images: &mut Assets<Image>,
    layout: &Layout,
    file: &str,
    file_id: u16,
) {
    for e in layout.elements.iter().filter(|e| e.kind == 4) {
        let Some(&id) = e.texts().first() else {
            continue;
        };
        if id >> 8 != file_id {
            continue;
        }
        let Some(words) = look.text(file, usize::from(id & 0xff)) else {
            continue;
        };
        write(out, labels, look, images, e, &words.replace("\\n", "\n"));
    }
}

/// `words` on element `e` by its font, colour (prop 10) and anchor, a line
/// at a time.
fn write(
    out: &mut Vec<Piece>,
    labels: &mut look::Labels,
    look: &Look,
    images: &mut Assets<Image>,
    e: &street_assets::font::Element,
    words: &str,
) {
    let Some(at) = spot(e) else { return };
    let font = e.props.get(&13).map_or(fonts::LIST, |&f| f as usize);
    let colour = e
        .props
        .get(&10)
        .map_or(colours::CREAM, |&c| street_assets::font::argb(c));
    let line = look.fonts.get(font).map_or(20.0, |f| f.line_height as f32);
    for (k, l) in words.split('\n').enumerate() {
        place(
            out,
            labels,
            look,
            images,
            font,
            l.trim_start(),
            colour,
            at + Vec2::new(0.0, line * k as f32),
            e.align,
        );
    }
}

/// A star (`XACOMMON` sheet 0, image 1, frame 0) centred on `at`.
fn star(out: &mut Vec<Piece>, look: &Look, at: Vec2) {
    let (image, size) = look.row_star();
    out.push(Piece::new(image, Rect::from_center_size(at, size)));
}

/// The board's pieces (the title and the Continue legend come with the
/// layout).
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    mode: usize,
    board: &Board,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(layout) = look.layouts.iter().find(|l| l.class == class_of(mode)) else {
        return out;
    };
    if mode == 0 {
        static_words(&mut out, labels, look, images, layout, "nbatcend.txt", 0x1e);
        if let Some(e) = element(layout, 1) {
            let words = look
                .text("nbatcend.txt", 2)
                .unwrap_or("%1d Reward Points")
                .replace("%1d", &board.reward_points.to_string());
            write(&mut out, labels, look, images, e, &words);
        }
        for id in [2, 3] {
            if let Some(at) = element(layout, id).and_then(spot) {
                star(&mut out, look, at);
            }
        }
        return out;
    }
    static_words(
        &mut out,
        labels,
        look,
        images,
        layout,
        "bealegendend.txt",
        0x1f,
    );
    if let Some(e) = element(layout, 0)
        && let Some(at) = spot(e)
    {
        let (first, last) = &board.baller;
        let own = e.props.get(&13).map_or(3, |&f| f as usize);
        let (font, name) = if last.trim().is_empty() {
            (own, first.clone())
        } else {
            (fonts::LIST_SMALL, format!("{first} {last}"))
        };
        let colour = e
            .props
            .get(&10)
            .map_or(colours::CREAM, |&c| street_assets::font::argb(c));
        place(
            &mut out, labels, look, images, font, &name, colour, at, e.align,
        );
        let width = look
            .fonts
            .get(font)
            .map_or(0.0, |f| f.measure(&name) as f32);
        star(&mut out, look, at + Vec2::new(width / 2.0 + 14.0, 0.0));
        star(&mut out, look, at - Vec2::new(width / 2.0 + 14.0, 0.0));
    }
    let text = |k: usize, or: &str| look.text("bealegendend.txt", k).unwrap_or(or).to_owned();
    let values = [
        (
            3,
            text(7, "%1d - %2d")
                .replace("%1d", &board.wins.to_string())
                .replace("%2d", &board.losses.to_string()),
        ),
        (
            1,
            look.text("reputation.txt", usize::from(board.reputation.clamp(1, 5)))
                .unwrap_or("ROOKIE")
                .to_owned(),
        ),
        (
            2,
            text(9, "%1d / %2d COMPLETE")
                .replace("%1d", &board.streets.0.to_string())
                .replace("%2d", &board.streets.1.to_string()),
        ),
    ];
    for (id, words) in values {
        if let Some(e) = element(layout, id) {
            write(&mut out, labels, look, images, e, &words);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_boards_take_their_constructors_layouts_and_backdrops() {
        assert_eq!((class_of(0), backdrop_of(0)), (0x3b, 0x14));
        assert_eq!((class_of(1), backdrop_of(1)), (0x3c, 0x10));
    }
}
