//! Be A Legend's map drawn the game's way (screen class 56, ctor
//! `0x29f200`, fill `FUN_002a03c0`, backdrop `XAFEBG` 19).
//!
//! The six cities are map nodes (`FUN_00225768`, ids 100..105 for cities
//! 0..5) standing at the layout's navigation points 1..6, each a
//! landmark from `XAFESCRN` sheet 0 (`FUN_00225d90`): the Empire State
//! Building (image 15), Independence Hall (15), the Sears Tower (12,
//! turned -90 degrees), the Space Needle (14), the Golden Gate (13) and a
//! Hollywood spotlight (13). A city with no open event shows only a blue
//! star (image 14, frame 5; `FUN_00225bc8`); the others their landmark
//! dark, and the highlighted one lit, with its glow (or the spotlight's
//! beam) and its name (`FUN_002266e0`; the names are the layout's text
//! elements 27..32, `mapscreen.txt` 26, 29..33). Under the map the
//! read-out (`FUN_002a0c00`): the baller's first and last names
//! (elements 10, 11), win-loss (12, "Win-Loss" before the first game),
//! progress (9), between two stars (44, 45: `XACOMMON` sheet 0 image 1,
//! frame 1). The reputation meter (element 20, kind 39) draws its five
//! 20x19 star pips 18 apart from the element's own left edge, lit per
//! level (frame 7 over frame 1, `FUN_00234d50`), the level's word below
//! it (the caption piece `FUN_00238a78`; `reputation.txt` #0..5).
//!
//! A city's events (`FUN_002a2548`) list on the right, one row per court
//! (the table at `0x489bd8`) at navigation points 21..25: the court's
//! name, a rule (333x4, 0x3c45a2) and two framed icons (`FUN_00224838`,
//! `FUN_002250b0`): pick-up a hoop, tournament a ball, street challenge
//! a red crown, the '78 challenge a gold crown, ticked once cleared;
//! dim (alpha 0x3f) while locked, half (0x7f) when open, full and in the
//! gold focus frame (image 0, frame 1) when picked (`FUN_00224e00`). The
//! map darkens to half under them (element 43's alpha +0x10d = 0x7f) with
//! a divider (element 42). The left of the screen shows the city's
//! landmark at 1.5 times its size with the larger offsets the node keeps
//! for it (+0xbc). The large landmark is the selected item in carousel
//! 115, at (155, 270); carousel 116 puts its city caption at (155, 285)
//! (`FUN_002a03c0`, `FUN_002a0fe0`, `FUN_00222420`).

use bevy::prelude::*;
use street_sim::story::{self as sim, EVENTS, EventKind};

use crate::{
    bracket_view::{ScreenArt, element, frame, place, text},
    look::{self, Look, Piece, colours, fonts},
    setup::Disc,
};

pub const CLASS: u32 = 56;
/// `FUN_002d7720(.., 0x38, 0x13)`.
pub const BACKDROP: usize = 19;

/// The Sears Tower's frames on image 12, dark and lit (stood up at load).
pub const TOWER: [[u32; 4]; 2] = [[13, 2, 136, 33], [11, 37, 137, 33]];

/// A city's landmark: its image, dark and lit frames, where they sit from
/// the node (+0xc4/+0xc8), its glow (+0xdc at +0xd4/+0xd8) and shadow
/// (child 1 at +0xcc/+0xd0); then the same for the larger picture.
struct Landmark {
    image: usize,
    dark: [u32; 4],
    lit: [u32; 4],
    at: [f32; 2],
    glow: (usize, [u32; 4], [f32; 2]),
    shadow: Option<(usize, [u32; 4], [f32; 2])>,
    big: [f32; 2],
    big_glow: [f32; 2],
    big_shadow: Option<(usize, [u32; 4], [f32; 2])>,
    /// The Sears Tower is drawn turned.
    turned: bool,
}

const GLOW_5: [u32; 4] = [135, 8, 19, 21];
const GLOW_6: [u32; 4] = [133, 36, 26, 23];
const GLOW_8: [u32; 4] = [136, 65, 21, 19];
const GLOW_9: [u32; 4] = [133, 94, 33, 27];

/// New York, Philly, Chicago, Seattle, Oakland, Los Angeles
/// (`FUN_00225d90`, `FUN_002266e0`).
const LANDMARKS: [Landmark; 6] = [
    Landmark {
        image: 15,
        dark: [59, 9, 25, 108],
        lit: [94, 12, 24, 107],
        at: [0.0, -80.0],
        glow: (15, GLOW_8, [2.0, -87.0]),
        shadow: Some((15, [198, 78, 38, 27], [20.0, 2.0])),
        big: [-18.0, -155.0],
        big_glow: [-15.0, -169.0],
        big_shadow: Some((15, [188, 9, 60, 63], [-3.0, -89.0])),
        turned: false,
    },
    Landmark {
        image: 15,
        dark: [2, 2, 45, 56],
        lit: [2, 66, 45, 56],
        at: [-23.0, -35.0],
        glow: (15, GLOW_8, [-13.0, -44.0]),
        shadow: None,
        big: [-30.0, -85.0],
        big_glow: [-14.0, -100.0],
        big_shadow: None,
        turned: false,
    },
    Landmark {
        image: 12,
        dark: TOWER[0],
        lit: TOWER[1],
        at: [-64.0, -43.0],
        glow: (15, GLOW_6, [-8.0, -103.0]),
        shadow: Some((12, [13, 78, 69, 43], [25.0, -7.0])),
        big: [-107.0, -105.0],
        big_glow: [-23.0, -195.0],
        big_shadow: Some((12, [153, 2, 73, 84], [19.0, -127.0])),
        turned: true,
    },
    Landmark {
        image: 14,
        dark: [120, 4, 62, 68],
        lit: [55, 4, 60, 68],
        at: [-27.0, -48.0],
        glow: (15, GLOW_5, [-11.0, -60.0]),
        shadow: None,
        big: [-39.0, -100.0],
        big_glow: [-14.0, -116.0],
        big_shadow: None,
        turned: false,
    },
    Landmark {
        image: 13,
        dark: [165, 47, 88, 80],
        lit: [74, 47, 91, 80],
        at: [-30.0, -33.0],
        glow: (15, GLOW_9, [-16.0, -43.0]),
        shadow: None,
        big: [-55.0, -95.0],
        big_glow: [-35.0, -110.0],
        big_shadow: None,
        turned: false,
    },
    Landmark {
        image: 13,
        dark: [5, 75, 40, 39],
        lit: [5, 75, 40, 39],
        at: [-6.0, -3.0],
        // The spotlight's beam.
        glow: (13, [3, 3, 83, 59], [-6.0, -18.0]),
        shadow: None,
        big: [-40.0, -62.0],
        big_glow: [-40.0, -85.0],
        big_shadow: None,
        turned: false,
    },
];

/// The city names' elements and their line spacing (+0xbc: New York's
/// and Los Angeles's run to two lines).
const LABELS: [(u32, f32); 6] = [
    (27, 33.0),
    (28, 0.0),
    (29, 0.0),
    (30, 0.0),
    (31, 0.0),
    (32, 25.0),
];

/// The map's courts as the table at `0x489bd8` groups them (one row per
/// court marker, ids 137..146): the city and the events in its two
/// slots (pick-up or tournament; street challenge).
const MARKERS: [(usize, [Option<usize>; 2]); 10] = [
    (0, [Some(0), None]),
    (0, [Some(2), Some(12)]),
    (0, [Some(4), None]),
    (0, [Some(6), None]),
    (0, [Some(9), Some(10)]),
    (1, [Some(1), Some(11)]),
    (2, [Some(3), Some(13)]),
    (3, [Some(5), Some(14)]),
    (4, [Some(8), Some(16)]),
    (5, [Some(7), Some(15)]),
];

/// `XAFESCRN` sheet 0 image 14's frames.
const STAR: [u32; 4] = [218, 97, 19, 19];
const SLOT_FRAME: [u32; 4] = [202, 2, 50, 48];
const CHECK: [u32; 4] = [211, 54, 33, 33];
/// Image 0, frame 1: the gold focus frame, drawn 50x48.
const FOCUS: [u32; 4] = [2, 70, 55, 53];
/// The rows' rule and the divider (0xff3c45a2).
const RULE: [u8; 4] = [0x3c, 0x45, 0xa2, 0xff];
/// The names' shadow (+0x4ac = 0xff040735).
const NAME_SHADOW: [u8; 4] = [0x04, 0x07, 0x35, 0xff];

/// A colour at half brightness (a disabled widget's +0x5a 0x7f).
fn half([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    [r / 2, g / 2, b / 2, a]
}

/// The reputation meter's level: reputation points over the game's
/// per-level size, capped at the five pips (`FUN_00234d10` refuses a
/// value past the meter's max).
fn reputation_level(story: &sim::Story) -> usize {
    (story.reputation / sim::REP_LEVEL_SIZE).clamp(0, 5) as usize
}

/// An event's icon and where it sits in its frame (`FUN_002250b0`).
fn icon(ev: usize) -> ([u32; 4], [f32; 2]) {
    match (EVENTS[ev].kind, ev) {
        (EventKind::Pickup, _) => ([3, 44, 45, 43], [3.0, 2.0]),
        (EventKind::Tournament, _) => ([3, 4, 35, 35], [8.0, 7.0]),
        (EventKind::StreetChallenge, 10) => ([75, 95, 44, 33], [3.0, 9.0]),
        (EventKind::StreetChallenge, _) => ([3, 95, 44, 33], [3.0, 9.0]),
    }
}

/// A city shows its landmark once one of its events is open.
fn available(progress: &sim::Story, city: usize) -> bool {
    sim::city_events(city)
        .into_iter()
        .any(|e| progress.visible(e) && progress.open(e))
}

/// A piece of `image`'s frame `f` at `at`, `scale` times its size; turned
/// pictures keep their middle.
fn art(
    art: &ScreenArt,
    image: usize,
    f: [u32; 4],
    at: Vec2,
    scale: f32,
    turned: Option<usize>,
) -> Option<Piece> {
    let size = Vec2::new(f[2] as f32, f[3] as f32) * scale;
    if let Some(k) = turned {
        let middle = at + size / 2.0;
        let half = Vec2::new(size.y, size.x) / 2.0;
        return Some(Piece::new(
            art.tower.get(k)?.clone(),
            Rect::from_corners(middle - half, middle + half),
        ));
    }
    let mut p = frame(art.sheet.get(image)?, f, at);
    p.rect = Rect::from_corners(at, at + size);
    Some(p)
}

/// One landmark: `big` the event page's larger picture.
fn landmark(out: &mut Vec<Piece>, a: &ScreenArt, city: usize, node: Vec2, lit: bool, big: bool) {
    let l = &LANDMARKS[city];
    let scale = if big { 1.5 } else { 1.0 };
    let off = |p: [f32; 2]| node + Vec2::new(p[0], p[1]);
    let shadow = if big { l.big_shadow } else { l.shadow };
    if let Some((image, f, at)) = shadow {
        out.extend(art(a, image, f, off(at), scale, None));
    }
    let f = if lit { l.lit } else { l.dark };
    let turned = l.turned.then_some(usize::from(lit));
    out.extend(art(
        a,
        l.image,
        f,
        off(if big { l.big } else { l.at }),
        scale,
        turned,
    ));
    if lit {
        let (image, f, at) = l.glow;
        let at = if big { l.big_glow } else { at };
        out.extend(art(a, image, f, off(at), scale, None));
    }
}

/// The cities in list 0's registration order (`FUN_002a03c0` registers
/// map nodes 100, 102, 101, 103, 104, 105 on it, so its row `k` pairs
/// with the node of prop 34 `k`): the map's cursor is a row.
pub const ROW_CITY: [usize; 6] = [0, 2, 1, 3, 4, 5];

/// Whether the map's row can take the cursor: its city shows its landmark
/// (a locked city's node, its blue star, is skipped, `FUN_0021f8f0`).
pub fn row_available(progress: &sim::Story, row: usize) -> bool {
    ROW_CITY.get(row).is_some_and(|&c| available(progress, c))
}

/// The reputation meter (element 20, kind 39: `FUN_00234f10` builds it,
/// `FUN_00234a50` places its children, `FUN_00234d50` and `FUN_002349c8`
/// fill it): the level's word (`reputation.txt`, font 1, 0xffffba00) at its
/// point, five stars 18 apart from 110 across (`XACOMMON` sheet 0, image 1,
/// frame 7 lit, 1 unlit), the bar (`XAFESCRN` sheet 0, image 7, frame 0x22,
/// 200 wide) 22 down, and over it a 3-high fill in 0xffffba00 at (5, 27),
/// 19 a point within the level (`FUN_001d8440`: field 15 less 11 a level).
const REP_WORD: [u8; 4] = [0xff, 0xba, 0x00, 0xff];
const REP_STARS_X: f32 = 110.0;
const REP_STAR_STEP: f32 = 18.0;
const REP_BAR_Y: f32 = 22.0;
const REP_BAR_WIDTH: f32 = 200.0;
const REP_FILL_AT: Vec2 = Vec2::new(5.0, 27.0);
const REP_FILL_HEIGHT: f32 = 3.0;
const REP_FILL_STEP: f32 = 19.0;
/// The US picture's place (`FUN_002a03c0`: (120, 113)).
const USA_AT: Vec2 = Vec2::new(120.0, 113.0);

/// The map's pieces (`Page::Story`): `cursor` the row of the city
/// highlighted ([`ROW_CITY`]).
#[allow(clippy::too_many_arguments)]
pub fn map(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    a: &ScreenArt,
    progress: &sim::Story,
    disc: &Disc,
    cursor: usize,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(layout) = look.layouts.iter().find(|l| l.class == CLASS) else {
        return out;
    };
    if let Some((usa, frame)) = &a.usa {
        out.push(Piece::part(
            usa.clone(),
            *frame,
            Rect::from_corners(USA_AT, USA_AT + frame.size()),
        ));
    }
    let focused_city = ROW_CITY.get(cursor).copied();
    for (city, &(id, spacing)) in LABELS.iter().enumerate() {
        let Some(node) = place(layout, city as u32 + 1) else {
            continue;
        };
        let focused = focused_city == Some(city);
        if !available(progress, city) {
            out.extend(art(a, 14, STAR, node, 1.0, None));
            continue;
        }
        landmark(&mut out, a, city, node, focused, false);
        if focused
            && let (Some(e), Some(at)) = (element(layout, id), place(layout, id))
            && let Some(&t) = e.texts().first()
            && let Some(name) = look.text("mapscreen.txt", usize::from(t & 0xff))
        {
            for (k, line) in name.split("\\n").enumerate() {
                let at = at + Vec2::new(0.0, spacing * k as f32);
                // The names are row widgets (kind 19), lit while their
                // city is: 0xfbac00 (`FUN_0021c590`).
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    fonts::TITLE_SMALL,
                    line.trim(),
                    colours::ROW_LIT,
                    Some((NAME_SHADOW, NAME_SHADOW)),
                    at,
                    e.align,
                    None,
                );
            }
        }
    }
    readout(&mut out, look, labels, images, a, layout, progress, disc);
    // The two prompts (`FUN_002a14f8` sets element 0x21's to "Select City",
    // 0x1c0b; element 0x22 keeps the layout's "Edit Baller"), each after its
    // button (the kind-29 icons: Cross, action 0, and Circle, 3).
    let select = look.text_id(0x1c0b).unwrap_or("Select City").to_owned();
    prompts(&mut out, look, labels, images, layout, &select);
    out
}

/// The prompts along the bottom: element 0x21's words (`select`), element
/// 0x22's own and the kind-29 button icons by their action (prop 9).
fn prompts(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    layout: &street_assets::font::Layout,
    select: &str,
) {
    for id in [33, 34] {
        let Some(e) = element(layout, id) else {
            continue;
        };
        let label = if id == 33 {
            select.to_owned()
        } else {
            e.texts()
                .first()
                .and_then(|&t| look.text_id(t))
                .unwrap_or("Edit Baller")
                .to_owned()
        };
        if let Some(at) = place(layout, id) {
            let font = e.props.get(&13).map_or(fonts::LIST_SMALL, |&f| f as usize);
            let colour = e.colour().unwrap_or(colours::PALE);
            text(
                out, look, labels, images, font, &label, colour, None, at, e.align, None,
            );
        }
    }
    for e in layout.elements.iter().filter(|e| e.kind == 29) {
        let (Some((x, y)), Some(&action)) = (e.position(), e.props.get(&9)) else {
            continue;
        };
        if let Some((atlas, frame)) = look.button_icon(action) {
            let middle = Vec2::new(f32::from(x), f32::from(y));
            out.push(Piece::part(
                atlas,
                frame,
                Rect::from_center_size(middle, frame.size()),
            ));
        }
    }
}

/// The read-out under the map and the city pages (`FUN_002a0c00`): the
/// baller's first and last names (elements 10 and 11; the first alone moves
/// to (65, 355) when the last is empty), the record (12: "Win-Loss" before
/// the first game, else "%1dW-%2dL"), the progress (9: "Progress: %1d/%2d"
/// of `FUN_001e73d8` and `FUN_001e74c0`), the two stars (44, 45) and the
/// reputation meter (20).
#[allow(clippy::too_many_arguments)]
fn readout(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    a: &ScreenArt,
    layout: &street_assets::font::Layout,
    progress: &sim::Story,
    disc: &Disc,
) {
    let pale = colours::PALE;
    let words = |i: usize, plain: &str| look.text("mapscreen.txt", i).unwrap_or(plain).to_owned();
    let star = progress
        .crew
        .first()
        .and_then(|&r| disc.players.get(usize::from(r)));
    let (first, last) = star.map_or((String::new(), String::new()), |p| {
        (p.first_name.clone(), p.last_name.clone())
    });
    let mut line = |out: &mut Vec<Piece>, id: u32, s: &str, at: Option<Vec2>| {
        if let (Some(e), Some(spot)) = (element(layout, id), at.or_else(|| place(layout, id))) {
            let font = e.props.get(&13).map_or(fonts::LIST, |&f| f as usize);
            text(
                out, look, labels, images, font, s, pale, None, spot, e.align, None,
            );
        }
    };
    if last.is_empty() {
        line(out, 10, &first, Some(Vec2::new(65.0, 355.0)));
    } else {
        line(out, 10, &first, None);
        line(out, 11, &last, None);
    }
    let record = if progress.wins + progress.losses == 0 {
        words(4, "Win-Loss")
    } else {
        words(3, "%1dW-%2dL")
            .replace("%1d", &progress.wins.to_string())
            .replace("%2d", &progress.losses.to_string())
    };
    line(out, 12, &record, None);
    let points = words(2, "Progress: %1d/%2d")
        .replace("%1d", &progress.overall_points().to_string())
        .replace("%2d", &sim::overall_total().to_string());
    line(out, 9, &points, None);
    // Elements 0x2c and 0x2d, disabled by `FUN_002a14f8` (+0x114): drawn at
    // half brightness (+0x5a 0x7f).
    if let Some(stars) = &a.stars {
        for id in [44, 45] {
            if let Some(at) = place(layout, id) {
                let mut p = frame(stars, [20, 1, 20, 19], at);
                p.colour = Color::srgb(0.5, 0.5, 0.5);
                out.push(p);
            }
        }
    }
    let Some((x, y)) = element(layout, 20).and_then(|e| e.position()) else {
        return;
    };
    let at = Vec2::new(f32::from(x), f32::from(y));
    let level = reputation_level(progress);
    let word = look.text("reputation.txt", level).unwrap_or(match level {
        0 => "REPUTATION",
        1 => "ROOKIE",
        2 => "BALLER",
        3 => "ALL STAR",
        4 => "SUPERSTAR",
        _ => "LEGEND",
    });
    text(
        out,
        look,
        labels,
        images,
        fonts::LIST_SMALL,
        word,
        REP_WORD,
        None,
        at,
        0,
        None,
    );
    if let Some(stars) = &a.stars {
        for i in 0..5usize {
            let star_at = at + Vec2::new(REP_STARS_X + REP_STAR_STEP * i as f32, 0.0);
            let f = if i < level {
                [20, 41, 20, 19]
            } else {
                [20, 1, 20, 19]
            };
            out.push(frame(stars, f, star_at));
        }
    }
    if let (Some(bar), Some(sheet)) = (a.rep_bar, a.sheet.get(7)) {
        let bar_at = at + Vec2::new(0.0, REP_BAR_Y);
        let mut p = frame(sheet, bar, bar_at);
        p.rect.max.x = bar_at.x + REP_BAR_WIDTH;
        out.push(p);
    }
    let points = (progress.reputation - sim::REP_LEVEL_SIZE * level as i16).clamp(0, 10);
    if points > 0 {
        let min = at + REP_FILL_AT;
        let mut fill = Piece::new(
            a.white.clone(),
            Rect::from_corners(
                min,
                min + Vec2::new(REP_FILL_STEP * f32::from(points), REP_FILL_HEIGHT),
            ),
        );
        fill.colour = Color::srgba_u8(REP_WORD[0], REP_WORD[1], REP_WORD[2], REP_WORD[3]);
        out.push(fill);
    }
}

/// One city's courts, each with the events in its two slots.
pub fn rows(city: usize) -> Vec<[Option<usize>; 2]> {
    MARKERS
        .iter()
        .filter(|(c, _)| *c == city)
        .map(|(_, slots)| *slots)
        .collect()
}

/// The city page's cursor after a press: list 0x1a's nodes (21..25) link
/// straight down and up a court at a time, the first's up and the last's
/// down disabled, keeping the slot the page is on where the court has one
/// (`FUN_002a2238`); left and right change the slot. `cursor` indexes
/// [`crate::story::city_rows`].
pub fn city_step(city: usize, cursor: usize, dx: i32, dy: i32) -> Option<usize> {
    let rows = rows(city);
    let flat: Vec<usize> = rows
        .iter()
        .flat_map(|slots| slots.iter().flatten().copied())
        .collect();
    let ev = *flat.get(cursor)?;
    let (r, s) = rows
        .iter()
        .enumerate()
        .find_map(|(r, slots)| Some((r, slots.iter().position(|&e| e == Some(ev))?)))?;
    let r2 = (r as i32 + dy).clamp(0, rows.len() as i32 - 1) as usize;
    let s2 = (s as i32 + dx).clamp(0, 1) as usize;
    let to = rows[r2][s2].or(rows[r2][s]).or(rows[r2][0])?;
    flat.iter().position(|&e| e == to)
}

/// A city's events (`Page::City`): `cursor` indexes the city's events as
/// [`crate::story::city_rows`] lists them.
#[allow(clippy::too_many_arguments)]
pub fn city(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    a: &ScreenArt,
    progress: &sim::Story,
    disc: &Disc,
    city: usize,
    cursor: usize,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(layout) = look.layouts.iter().find(|l| l.class == CLASS) else {
        return out;
    };
    // The map at half under the list, and the divider.
    for (id, colour) in [(43, [0, 0, 0, 0x7f]), (42, RULE)] {
        if let Some(e) = element(layout, id)
            && let (Some((x, y)), Some((w, h))) = (e.position(), e.size())
        {
            let (x, y, w, h) = (f32::from(x), f32::from(y), f32::from(w), f32::from(h));
            let mut p = Piece::new(a.white.clone(), Rect::new(x, y, x + w, y + h));
            p.colour = if colour[..3] == [0, 0, 0] {
                Color::srgba(
                    0.0,
                    0.0,
                    0.0,
                    look::black_alpha(f32::from(colour[3]) / 255.0),
                )
            } else {
                Color::srgba_u8(colour[0], colour[1], colour[2], colour[3])
            };
            out.push(p);
        }
    }
    // The city's landmark is a carousel item. Its navigation point is the
    // carousel's own position, copied by FUN_00222420; keep the authored
    // offsets from that point rather than centring each image separately.
    if city < LANDMARKS.len() {
        landmark(&mut out, a, city, Vec2::new(155.0, 270.0), true, true);
        let name = look
            .text("mapscreen.txt", 28 + city)
            .unwrap_or(sim::CITIES[city]);
        text(
            &mut out,
            look,
            labels,
            images,
            fonts::TITLE_SMALL,
            name,
            colours::HELP,
            None,
            Vec2::new(155.0, 285.0),
            1,
            None,
        );
    }
    let shown = crate::story::city_rows(progress, city);
    let picked = shown.get(cursor).copied();
    for (k, slots) in rows(city).into_iter().enumerate() {
        let Some(at) = place(layout, 21 + k as u32) else {
            continue;
        };
        let focused = picked.is_some_and(|ev| slots.contains(&Some(ev)));
        let any_open = slots.iter().flatten().any(|&e| progress.open(e));
        // The rule under the row.
        let mut rule = Piece::new(
            a.white.clone(),
            Rect::new(at.x - 25.0, at.y + 28.0, at.x + 308.0, at.y + 32.0),
        );
        rule.colour = Color::srgb_u8(RULE[0], RULE[1], RULE[2]);
        out.push(rule);
        // The court's name (from the first slot's event).
        let first = slots.iter().flatten().next().copied();
        if let Some(ev) = first
            && let Some(court) = disc.courts.get(usize::from(EVENTS[ev].court))
        {
            // Row widgets (`FUN_0021c590`): lit 0xfbac00, idle 0x88a0be, a
            // locked court's disabled at half brightness (+0x5a 0x7f).
            let colour = if focused {
                colours::ROW_LIT
            } else if any_open {
                colours::ROW
            } else {
                half(colours::ROW)
            };
            text(
                &mut out,
                look,
                labels,
                images,
                fonts::LIST,
                &court.name,
                colour,
                None,
                at + Vec2::new(-20.0, -20.0),
                0,
                Some(205.0),
            );
        }
        // The picked event's second line: its kind (`mapscreen.txt` 19..23),
        // or "Locked" (25), in the row colour.
        if focused && let Some(ev) = picked {
            let word = if progress.open(ev) {
                let (i, plain) = sim::kind_word(ev);
                look.text("mapscreen.txt", i).unwrap_or(plain).to_owned()
            } else {
                look.text("mapscreen.txt", 25)
                    .unwrap_or("Locked")
                    .to_owned()
            };
            text(
                &mut out,
                look,
                labels,
                images,
                fonts::LIST_SMALL,
                &word,
                colours::ROW,
                None,
                at + Vec2::new(-20.0, 5.0),
                0,
                Some(205.0),
            );
        }
        for (s, ev) in slots.iter().enumerate() {
            let Some(ev) = *ev else { continue };
            let spot = at + Vec2::new(if s == 0 { 195.0 } else { 250.0 }, -21.0);
            // `FUN_00224e00`'s alphas, which over the list's dark panel show
            // as brightness: a tint here, since the window composites a
            // translucent piece in linear light (see `look::black_alpha`).
            // A picked but locked event keeps its frame lit and its icon at
            // half, without the gold focus (measured on the original's
            // Soul in the Hole at 0 points).
            let open = progress.open(ev);
            let lit = picked == Some(ev);
            let alpha = if lit && open {
                1.0
            } else if open || lit {
                0x7f as f32 / 255.0
            } else {
                0x3f as f32 / 255.0
            };
            let tint = Color::srgb(alpha, alpha, alpha);
            let (f, inset) = icon(ev);
            if let Some(mut p) = art(a, 14, SLOT_FRAME, spot, 1.0, None) {
                p.colour = if lit { Color::WHITE } else { tint };
                out.push(p);
            }
            if let Some(mut p) = art(a, 14, f, spot + Vec2::new(inset[0], inset[1]), 1.0, None) {
                p.colour = tint;
                out.push(p);
            }
            if progress.cleared(ev)
                && let Some(mut p) = art(a, 14, CHECK, spot + Vec2::new(8.0, 7.0), 1.0, None)
            {
                let k = alpha.max(0x7f as f32 / 255.0);
                p.colour = Color::srgb(k, k, k);
                out.push(p);
            }
            if lit
                && open
                && let Some(mut p) = art(a, 0, FOCUS, spot, 1.0, None)
            {
                p.rect = Rect::from_corners(spot, spot + Vec2::new(50.0, 48.0));
                out.push(p);
            }
        }
    }
    // The read-out stays under the list; the first prompt reads "Play
    // Game" (0x1c0c, `FUN_002a14f8`).
    readout(&mut out, look, labels, images, a, layout, progress, disc);
    let play = look.text_id(0x1c0c).unwrap_or("Play Game").to_owned();
    prompts(&mut out, look, labels, images, layout, &play);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reputation_levels_light_the_meter_pips() {
        let mut story = sim::Story::default();
        assert_eq!(reputation_level(&story), 0);
        story.reputation = 10;
        assert_eq!(reputation_level(&story), 0);
        story.reputation = 11;
        assert_eq!(reputation_level(&story), 1);
        story.reputation = 54;
        assert_eq!(reputation_level(&story), 4);
        story.reputation = 62;
        assert_eq!(reputation_level(&story), 5);
    }

    #[test]
    fn every_map_event_has_one_marker_slot() {
        let mut seen: Vec<usize> = MARKERS
            .iter()
            .flat_map(|(_, s)| s.iter().flatten().copied())
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..EVENTS.len()).collect::<Vec<_>>());
        // Each slot's city is the event's own, and street challenges sit
        // in the second slot.
        for (city, slots) in MARKERS {
            for (s, ev) in slots.iter().enumerate() {
                if let Some(ev) = *ev {
                    assert_eq!(sim::city_of(ev), city);
                    assert_eq!(s == 1, EVENTS[ev].kind == EventKind::StreetChallenge);
                }
            }
        }
    }

    #[test]
    fn the_city_cursor_moves_by_court_and_keeps_its_slot() {
        // New York: NYC Rec Center (pick-up), Soul in the Hole (tournament
        // and street challenge), The Cage, Uptown, Rucker Park (tournament
        // and the '78 challenge).
        let flat = crate::story::city_rows(&sim::Story::default(), 0);
        assert_eq!(flat, [0, 2, 12, 4, 6, 9, 10]);
        // Down from the Rec Center lands on Soul in the Hole's tournament,
        // as the original's does; right moves to its challenge, and down
        // from there to The Cage's only slot.
        assert_eq!(city_step(0, 0, 0, 1), Some(1));
        assert_eq!(city_step(0, 1, 1, 0), Some(2));
        assert_eq!(city_step(0, 2, 0, 1), Some(3));
        // The ends stay put.
        assert_eq!(city_step(0, 0, 0, -1), Some(0));
        assert_eq!(city_step(0, 6, 0, 1), Some(6));
    }

    #[test]
    fn the_map_rows_pair_with_the_nodes_by_registration_order() {
        // `FUN_002a03c0` registers 100, 102, 101, 103, 104, 105 on list 0:
        // row k is the node of prop 34 k, which stands at node id city + 1.
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let layouts = street_assets::font::layouts(
            &mut street_assets::iso::Iso::open(path).unwrap(),
            "/FE/XLAY_FE.BIN",
        )
        .unwrap();
        let layout = layouts.iter().find(|l| l.class == CLASS).unwrap();
        for (row, &city) in ROW_CITY.iter().enumerate() {
            let node = layout
                .elements
                .iter()
                .find(|e| {
                    e.kind == 10
                        && e.props.get(&33) == Some(&0)
                        && e.props.get(&34) == Some(&(row as u32))
                })
                .unwrap();
            assert_eq!(node.id, city as u32 + 1, "row {row}");
        }
        // From New York, left walks the open cities in the original's order
        // (its grabs at 220 points: Philly, Chicago, Oakland, Los Angeles,
        // New York, Seattle skipped while it shows only its star).
        let open = |row: usize| ROW_CITY[row] != 3;
        let mut row = 0;
        let mut walked = Vec::new();
        for _ in 0..5 {
            row = look::screen_step_filtered(layout, row, 6, 2, open).unwrap();
            walked.push(ROW_CITY[row]);
        }
        assert_eq!(walked, [1, 2, 4, 5, 0]);
    }

    #[test]
    fn the_rows_keep_the_city_lists_order() {
        // With everything showing, a city's rows cover its events in the
        // map's order.
        let story = sim::Story::default();
        for city in 0..6 {
            let listed: Vec<usize> = crate::story::city_rows(&story, city);
            let in_rows: Vec<usize> = rows(city)
                .into_iter()
                .flat_map(|s| s.into_iter().flatten())
                .collect();
            let mut a = listed.clone();
            let mut b = in_rows.clone();
            a.sort_unstable();
            b.sort_unstable();
            assert_eq!(a, b, "city {city}");
        }
    }
}
