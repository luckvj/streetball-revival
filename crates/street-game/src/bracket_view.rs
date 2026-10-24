//! Be A Legend's tournament brackets drawn on the game's own layouts:
//! single elimination (screen class 88, ctor `0x2cbed8`, fill
//! `FUN_002cc050`), double elimination (class 89, ctor `0x285050`, fill
//! `FUN_002857f8`) and best of three (class 94, ctor `0x265f48`, fill
//! `FUN_00266080`); their backdrops are `XAFEBG` 24, 11 and 1
//! (`FUN_002d7720(.., 0x18 / 0xb / 1)`).
//!
//! The fills name every element by its place in the bracket: round `r`,
//! match `m` (its order in the round), side `s` — the name is text
//! element `(r+1)*100 + (m+1)*10 + s`, its plate the image element 1000
//! above it (`XAFESCRN` sheet 0, image 2, frame 0: a 125x32 outline,
//! `FUN_00210048(w, 2, 0, 2, 0)`), and a W!/L badge for your matches
//! `(r+5)*100 + (m+1)*10 + s` (frames 2 and 1; best of three has one per
//! game, the last digit the game). The plates are recoloured by palette
//! swaps (`FUN_00210368(w, 3, k)`: `XPAL_FE` sheet 3, palette `k`, which
//! changes only the outline's colour), and the names by the colours each
//! ctor keeps at +0xf0..+0x104:
//!
//! | State | Plate | Name |
//! |---|---|---|
//! | not played | own blue | the layout's grey-blue |
//! | played: winner, loser | own, palette 1 (navy) | +0xf8 blue, +0xf4 navy |
//! | yours, played | palette 2 (orange) | +0x100 orange |
//! | your next match: you, them | palette 3 (yellow), palette 0 (grey-blue) | +0xfc yellow, +0x104 grey-blue |
//!
//! The round headings light yellow (+0xf0) on your round and turn cream
//! (0xdcdcbb) behind you. Your way through is drawn in orange: single
//! elimination's three connectors (frames 5, 6, 7 at (190, 152),
//! (319, 186), (447, 262), the static block `FUN_002cd618`), best of
//! three's two elbows (frames 3, 4 at (205, 163), (383, 236),
//! `FUN_00267370`), and double elimination's 3-pixel lines (28 segments
//! in its ctor at +0x138, shown in sets by where you went). Generic crews
//! are "Team %1d" (`common.txt` 42, the crew's number counted from the
//! first), a boss crew its team's nickname ("Stretch"); your crew the game
//! calls by your story team's name (story +0xca0, `Story::crew_team`).

use bevy::prelude::*;
use street_assets::{
    fe,
    font::{Element, Layout},
    ssh,
};
use street_sim::bracket::{Bracket, Entrant};

use crate::look::{self, Align, Look, Piece, colours, fonts};

/// The three screens.
pub const SINGLE: u32 = 88;
pub const DOUBLE: u32 = 89;
pub const BEST_OF_THREE: u32 = 94;

/// The screen a bracket shape is shown on (`singleko`, `doubleko`,
/// `bestof3`).
pub fn class_of(kind: u8) -> u32 {
    match kind {
        0 | 1 => SINGLE,
        2 => DOUBLE,
        _ => BEST_OF_THREE,
    }
}

/// The screen's backdrop (`XAFEBG`, from each ctor's call to the base
/// screen ctor).
pub fn backdrop_of(class: u32) -> usize {
    match class {
        SINGLE => 24,
        DOUBLE => 11,
        _ => 1,
    }
}

/// The ctors' colours (+0xf0..+0x104), RGBA.
const YELLOW: [u8; 4] = [0xff, 0xd0, 0x3f, 0xff];
const NAVY: [u8; 4] = [0x1a, 0x20, 0x5d, 0xff];
const BLUE: [u8; 4] = [0x3e, 0x46, 0x99, 0xff];
const ORANGE: [u8; 4] = [0xf8, 0x71, 0x06, 0xff];
const GREY: [u8; 4] = [0x88, 0xa3, 0xb7, 0xff];
/// A passed heading, the champion's name (0xffdcdcbb in the fills).
const CREAM: [u8; 4] = [0xdc, 0xdc, 0xbb, 0xff];

/// A plate's outline: its own colour or one of `XPAL_FE` sheet 3's four
/// palettes (index 0..3 = `Grey`..`Yellow`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Own,
    Grey,
    Navy,
    Orange,
    Yellow,
}

impl Tint {
    fn index(self) -> usize {
        match self {
            Tint::Own => 0,
            Tint::Grey => 1,
            Tint::Navy => 2,
            Tint::Orange => 3,
            Tint::Yellow => 4,
        }
    }
}

/// The outline colour of `XAFESCRN` sheet 0 image 2 (palette entry 32)
/// and its replacement in `XPAL_FE` sheet 3's palettes 0..3: the only
/// entry they change.
const OUTLINE: [u8; 3] = [66, 74, 156];
const PALETTES: [[u8; 3]; 4] = [[140, 165, 181], [25, 33, 91], [255, 115, 0], [255, 214, 58]];

/// A W! or L badge (image 2, frames 2 and 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Badge {
    Won,
    Lost,
}

/// A round heading's colour: the layout's, yours now, or behind you.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heading {
    Plain,
    Current,
    Passed,
}

/// One side of one match on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotPlan {
    /// The name's element; the plate is 1000 above.
    pub id: u32,
    pub entrant: Entrant,
    /// The name's colour; `None` keeps the layout's.
    pub colour: Option<[u8; 4]>,
    pub tint: Tint,
}

/// What a bracket screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub class: u32,
    pub slots: Vec<SlotPlan>,
    pub badges: Vec<(u32, Badge)>,
    pub headings: Vec<(u32, Heading)>,
    /// The champion, once the final is played, and whether it is you.
    pub champion: Option<Entrant>,
    /// Your way through: connectors (single elimination, best of three)
    /// or line segments (double elimination), by element id.
    pub paths: Vec<u32>,
}

/// The headings: (element, round, the one row it names if not the whole
/// round), then the champion's heading.
fn headings_of(class: u32) -> (&'static [(u32, usize, Option<usize>)], u32) {
    match class {
        SINGLE => (&[(20, 0, None), (21, 1, None), (22, 2, None)], 23),
        // Winner's (right) and Loser's (left) brackets, the losers'
        // final ("Semis") and the final.
        DOUBLE => (
            &[
                (28, 1, Some(8)),
                (27, 1, Some(2)),
                (22, 2, None),
                (23, 3, None),
            ],
            20,
        ),
        _ => (&[(20, 0, Some(0)), (21, 0, Some(1)), (22, 1, None)], 23),
    }
}

/// The champion's elements: name, plate, its own W! badge (single
/// elimination's 810), and the "Final Standings." line.
fn champion_of(class: u32) -> (u32, u32, Option<u32>, u32) {
    match class {
        SINGLE => (410, 47, Some(810), 24),
        DOUBLE => (26, 29, None, 37),
        _ => (24, 25, None, 26),
    }
}

/// The prompts: (text, icon, `XACOMMON` sheet 1 image 0 frame) for our
/// rows 0 Continue, 1 Forfeit Tournament, 2 Back. The icons as the fills
/// set them for the American pad (`FUN_00210048(w, 0, 1, 0, n)`: frame 3
/// Cross to continue, 8 for the forfeit, 2 Triangle for back).
fn prompts_of(class: u32) -> [(u32, u32, usize); 3] {
    match class {
        SINGLE => [(44, 42, 3), (43, 41, 8), (45, 46, 2)],
        DOUBLE => [(35, 34, 3), (21, 32, 8), (25, 33, 2)],
        _ => [(33, 1, 3), (2, 0, 8), (34, 35, 2)],
    }
}

/// The frames of `XACOMMON` sheet 1, image 0 (the pad's buttons) the
/// prompts use: 0 Circle, 2 Triangle, 3 Cross, 8 the forfeit's.
fn icon_frame(n: usize) -> [u32; 4] {
    match n {
        0 => [35, 83, 25, 25],
        2 => [3, 51, 25, 25],
        3 => [35, 51, 25, 25],
        _ => [128, 51, 32, 25],
    }
}

/// The screen's subtitle element ("Single Elimination", ...).
fn subtitle_of(class: u32) -> u32 {
    match class {
        SINGLE => 33,
        DOUBLE => 36,
        _ => 3,
    }
}

/// Double elimination's line segments (ctor `0x285050`, +0x138..+0x2f4,
/// by the element id each is created as in `FUN_002857f8`).
const LINES: [(u32, [f32; 4]); 28] = [
    (0xd06, [370.0, 156.0, 390.0, 156.0]),
    (0xd07, [391.0, 155.0, 391.0, 188.0]),
    (0xd08, [419.0, 187.0, 390.0, 187.0]),
    (0xd09, [521.0, 185.0, 554.0, 185.0]),
    (0xd0a, [555.0, 184.0, 555.0, 310.0]),
    (0xd0b, [553.0, 310.0, 410.0, 310.0]),
    (0xd0c, [375.0, 358.0, 410.0, 358.0]),
    (0xd0d, [410.0, 358.0, 410.0, 384.0]),
    (0xd10, [520.0, 190.0, 553.0, 190.0]),
    (0xd11, [554.0, 188.0, 554.0, 318.0]),
    (0xd12, [375.0, 318.0, 554.0, 318.0]),
    (0xd13, [410.0, 384.0, 445.0, 384.0]),
    (0xd14, [250.0, 325.0, 222.0, 325.0]),
    (0xd15, [254.0, 318.0, 241.0, 318.0]),
    (0xd16, [241.0, 315.0, 241.0, 385.0]),
    (0xd17, [241.0, 384.0, 254.0, 384.0]),
    (0xd18, [378.0, 348.0, 410.0, 348.0]),
    (0xd0e, [410.0, 308.0, 410.0, 348.0]),
    (0xd0f, [375.0, 384.0, 445.0, 384.0]),
    (0xd1c, [235.0, 156.0, 255.0, 156.0]),
    (0xd1d, [237.0, 154.0, 237.0, 190.0]),
    (0xd1e, [236.0, 190.0, 219.0, 190.0]),
    (0xd1f, [85.0, 190.0, 95.0, 190.0]),
    (0xd20, [85.0, 188.0, 85.0, 286.0]),
    (0xd21, [85.0, 285.0, 254.0, 285.0]),
    (0xd19, [254.0, 297.0, 241.0, 297.0]),
    (0xd1a, [241.0, 296.0, 241.0, 384.0]),
    (0xd1b, [241.0, 384.0, 254.0, 384.0]),
];

/// Single elimination's and best of three's connectors: (id, frame of
/// image 2, top-left).
const CONNECTORS: [(u32, usize, [f32; 2]); 5] = [
    (0xd06, 5, [190.0, 152.0]),
    (0xd07, 6, [319.0, 186.0]),
    (0xd08, 7, [447.0, 262.0]),
    (0xd0b, 3, [205.0, 163.0]),
    (0xd0c, 4, [383.0, 236.0]),
];

/// The frames of `XAFESCRN` sheet 0, image 2 (its `|` block): x, y, w, h.
const PLATE_FRAMES: [[u32; 4]; 10] = [
    [4, 6, 125, 32],
    [136, 7, 30, 30],
    [171, 7, 30, 30],
    [4, 43, 55, 76],
    [208, 7, 41, 33],
    [74, 43, 9, 22],
    [90, 43, 8, 53],
    [102, 44, 6, 4],
    [171, 46, 30, 30],
    [137, 47, 28, 28],
];

/// The bracket screen's state from a bracket.
pub fn plan(b: &Bracket) -> Plan {
    let class = class_of(b.kind);
    let current = b.current();
    let mut slots = Vec::new();
    let mut badges = Vec::new();
    let mut paths = Vec::new();
    for round in 0..b.rounds() {
        for (m, &row) in b.round_rows(round).iter().enumerate() {
            let row = usize::from(row);
            let sides = b.rows[row].sides;
            let decided = b.decided(row);
            let mine = sides.iter().position(|&e| e == Entrant::You);
            for (s, &entrant) in sides.iter().enumerate() {
                let id = ((round + 1) * 100 + (m + 1) * 10 + s) as u32;
                let (mut colour, mut tint) = (None, Tint::Own);
                if let Some(w) = decided {
                    (colour, tint) = if s == w {
                        (Some(BLUE), Tint::Own)
                    } else {
                        (Some(NAVY), Tint::Navy)
                    };
                    if mine == Some(s) {
                        (colour, tint) = (Some(ORANGE), Tint::Orange);
                    }
                }
                if let Some((_, r, side)) = current
                    && r == row
                {
                    (colour, tint) = if s == side {
                        (Some(YELLOW), Tint::Yellow)
                    } else {
                        (Some(GREY), Tint::Grey)
                    };
                }
                slots.push(SlotPlan {
                    id,
                    entrant,
                    colour,
                    tint,
                });
            }
            let Some(side) = mine else { continue };
            let base = ((round + 5) * 100 + (m + 1) * 10) as u32;
            if class == BEST_OF_THREE {
                // A badge a game played (`FUN_00266080`'s per-game loop).
                let m = b.rows[row];
                let played = (m.wins[0] | m.wins[1]).count_ones();
                for g in 0..played {
                    let won = m.wins[side] & (1 << g) != 0;
                    badges.push((base + g, if won { Badge::Won } else { Badge::Lost }));
                }
            } else if let Some(w) = decided {
                let badge = if w == side { Badge::Won } else { Badge::Lost };
                badges.push((base + side as u32, badge));
            }
            if decided == Some(side) {
                paths.extend(won_path(class, b.kind, row, side));
            } else if decided.is_some() && class == DOUBLE && row == 0 {
                // Into the losers' bracket.
                paths.extend([0xd1c, 0xd1d, 0xd1e]);
            } else if decided.is_some() && class == DOUBLE && row == 8 {
                paths.extend([0xd10, 0xd11, 0xd12]);
            }
        }
    }
    let champion = Some(b.champion()).filter(|&e| e != Entrant::Empty);
    let (_, _, champ_badge, _) = champion_of(class);
    if champion == Some(Entrant::You) {
        if let Some(id) = champ_badge {
            badges.push((id, Badge::Won));
        }
        match class {
            SINGLE => paths.push(0xd08),
            BEST_OF_THREE => paths.push(0xd0c),
            // Straight from the winners' final, or up from the losers'.
            _ if b.decided(8) == Some(0) && b.rows[8].sides[0] == Entrant::You => {
                paths.extend([0xd0c, 0xd0d, 0xd13])
            }
            _ => paths.push(0xd0f),
        }
    }
    paths.sort_unstable();
    paths.dedup();
    // The headings: yours now in yellow, the ones behind you cream.
    let (list, champ_heading) = headings_of(class);
    let over = champion.is_some();
    let mut headings: Vec<(u32, Heading)> = list
        .iter()
        .map(|&(id, round, row)| {
            let rows: Vec<usize> = match row {
                Some(r) => vec![r],
                None => b
                    .round_rows(round)
                    .iter()
                    .map(|&r| usize::from(r))
                    .collect(),
            };
            let now = current.is_some_and(|(r, row, _)| r == round && rows.contains(&row));
            let done = !rows.is_empty() && rows.iter().all(|&r| b.decided(r).is_some());
            let state = if now {
                Heading::Current
            } else if done || over {
                Heading::Passed
            } else {
                Heading::Plain
            };
            (id, state)
        })
        .collect();
    headings.push((
        champ_heading,
        match champion {
            Some(Entrant::You) => Heading::Current,
            Some(_) => Heading::Passed,
            None => Heading::Plain,
        },
    ));
    Plan {
        class,
        slots,
        badges,
        headings,
        champion,
        paths,
    }
}

/// The connectors or lines a won match of yours lights.
fn won_path(class: u32, kind: u8, row: usize, side: usize) -> Vec<u32> {
    match (class, row) {
        (SINGLE, 0) => vec![0xd06],
        (SINGLE, 8) => vec![0xd07],
        (SINGLE, 12) => vec![0xd08],
        (BEST_OF_THREE, 0) if kind == 4 => vec![0xd0b],
        (DOUBLE, 0) => vec![0xd06, 0xd07, 0xd08],
        (DOUBLE, 8) => vec![0xd09, 0xd0a, 0xd0b, 0xd18, 0xd0e],
        (DOUBLE, 2) => vec![0xd1f, 0xd20, 0xd21],
        // The losers' final: up from the winners' final (side 1) or the
        // losers' bracket (side 0).
        (DOUBLE, 9) if side == 1 => vec![0xd15, 0xd16, 0xd17],
        (DOUBLE, 9) => vec![0xd19, 0xd1a, 0xd1b],
        _ => Vec::new(),
    }
}

/// `XAFESCRN` sheet 0's images the bracket and map screens draw, cut and
/// recoloured once.
#[derive(Resource)]
pub struct ScreenArt {
    /// The sheet's images by number.
    pub sheet: Vec<Handle<Image>>,
    /// The bracket's plate outline (image 2, frame 0) by [`Tint`].
    plates: Vec<Handle<Image>>,
    /// Chicago's tower stood up (image 12, frames 0 and 1, which the map
    /// turns -90 degrees, +0xec).
    pub tower: Vec<Handle<Image>>,
    /// `XACOMMON` sheet 0 image 1: the read-out's stars.
    pub stars: Option<Handle<Image>>,
    /// A white pixel for lines and panels.
    pub white: Handle<Image>,
    /// The map's United States (`XAFEBGEX` entry 7, 512x256, and its frame
    /// 0, (5, 0) 387x229): the picture widget `FUN_002a03c0` puts at (120,
    /// 113), shown on the map only (`FUN_002a14f8`).
    pub usa: Option<(Handle<Image>, Rect)>,
    /// The reputation meter's bar (image 7, frame 0x22: `FUN_00234f10`'s
    /// child 2), its frame on the image.
    pub rep_bar: Option<[u32; 4]>,
}

/// Startup: cut the screens' art from the disc.
pub fn start(
    mut commands: Commands,
    mut disc: ResMut<crate::setup::Disc>,
    mut images: ResMut<Assets<Image>>,
) {
    let sheet = match fe::read(&mut disc.iso, "/FE/XAFESCRN.BIN") {
        Ok(mut s) if !s.is_empty() => s.swap_remove(0),
        Ok(_) => return warn!("bracket art: no sheets"),
        Err(e) => return warn!("bracket art: {e}"),
    };
    let stars = fe::read(&mut disc.iso, "/FE/XACOMMON.BIN")
        .ok()
        .and_then(|s| s.into_iter().next())
        .and_then(|s| s.textures.into_iter().nth(1))
        .map(|t| images.add(street_render::convert::image(&t)));
    let by_name = |n: usize| {
        let name = format!("{n:04x}");
        sheet.textures.iter().find(|t| t.name == name)
    };
    let plates = by_name(2)
        .map(|t| {
            let [x, y, w, h] = PLATE_FRAMES[0];
            let plate = crop(t, x, y, w, h);
            let mut out = vec![images.add(street_render::convert::image(&plate))];
            for colour in PALETTES {
                let mut p = plate.clone();
                for px in p.rgba.as_chunks_mut::<4>().0 {
                    if px[..3] == OUTLINE {
                        px[..3].copy_from_slice(&colour);
                    }
                }
                out.push(images.add(street_render::convert::image(&p)));
            }
            out
        })
        .unwrap_or_default();
    let tower = by_name(12)
        .map(|t| {
            crate::map_view::TOWER
                .iter()
                .map(|&[x, y, w, h]| {
                    images.add(street_render::convert::image(&stand_up(&crop(
                        t, x, y, w, h,
                    ))))
                })
                .collect()
        })
        .unwrap_or_default();
    let all = (0..sheet.textures.len())
        .map(|n| {
            by_name(n)
                .map(|t| images.add(street_render::convert::image(t)))
                .unwrap_or_default()
        })
        .collect();
    let white = images.add(street_render::convert::image(&ssh::Texture {
        name: "white".into(),
        width: 1,
        height: 1,
        rgba: vec![255; 4],
        indices: None,
    }));
    let usa = fe::read(&mut disc.iso, "/FE/XAFEBGEX.BIN")
        .ok()
        .and_then(|s| s.into_iter().nth(7))
        .and_then(|s| {
            let [x, y, w, h] = s.frames.first()?.first()?.bounds();
            let t = s.textures.into_iter().next()?;
            let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
            Some((
                images.add(street_render::convert::image(&t)),
                Rect::new(x, y, x + w, y + h),
            ))
        });
    let rep_bar = sheet
        .textures
        .iter()
        .position(|t| t.name == "0007")
        .and_then(|k| sheet.frames.get(k))
        .and_then(|f| f.get(0x22))
        .map(|f| f.bounds())
        .and_then(|[x, y, w, h]| {
            Some([
                u32::try_from(x).ok()?,
                u32::try_from(y).ok()?,
                u32::try_from(w).ok()?,
                u32::try_from(h).ok()?,
            ])
        });
    commands.insert_resource(ScreenArt {
        sheet: all,
        plates,
        tower,
        stars,
        white,
        usa,
        rep_bar,
    });
}

fn crop(t: &ssh::Texture, x: u32, y: u32, w: u32, h: u32) -> ssh::Texture {
    let (w, h) = (
        w.min(t.width.saturating_sub(x)),
        h.min(t.height.saturating_sub(y)),
    );
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let at = ((row * t.width + x) * 4) as usize;
        rgba.extend_from_slice(&t.rgba[at..at + (w * 4) as usize]);
    }
    ssh::Texture {
        name: t.name.clone(),
        width: w,
        height: h,
        rgba,
        indices: None,
    }
}

/// Turned a quarter turn: the picture's right edge becomes its top.
fn stand_up(t: &ssh::Texture) -> ssh::Texture {
    let (w, h) = (t.width, t.height);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let src = ((y * w + x) * 4) as usize;
            // (x, y) to (y, w - 1 - x) in an h-wide picture.
            let dst = (((w - 1 - x) * h + y) * 4) as usize;
            rgba[dst..dst + 4].copy_from_slice(&t.rgba[src..src + 4]);
        }
    }
    ssh::Texture {
        name: t.name.clone(),
        width: h,
        height: w,
        rgba,
        indices: None,
    }
}

/// A layout's element by id.
pub fn element(layout: &Layout, id: u32) -> Option<&Element> {
    layout.elements.iter().find(|e| e.id == id && e.kind != 0)
}

/// An element's place on the 640x480 screen.
pub fn place(layout: &Layout, id: u32) -> Option<Vec2> {
    element(layout, id)
        .and_then(|e| e.position())
        .map(|(x, y)| Vec2::new(f32::from(x), f32::from(y)))
}

/// A frame of an atlas at `at` (top-left), its own size.
pub fn frame(image: &Handle<Image>, [x, y, w, h]: [u32; 4], at: Vec2) -> Piece {
    let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
    Piece::part(
        image.clone(),
        Rect::new(x, y, x + w, y + h),
        Rect::new(at.x, at.y, at.x + w, at.y + h),
    )
}

/// A line of text placed by a layout element's alignment (bit 0 centred
/// across, bit 2 centred up and down), squeezed to `room` if wider.
#[allow(clippy::too_many_arguments)]
pub fn text(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    font: usize,
    s: &str,
    colour: [u8; 4],
    shadow: Option<([u8; 4], [u8; 4])>,
    at: Vec2,
    align: u8,
    room: Option<f32>,
) {
    let across = if align & 1 != 0 {
        Align::Centre
    } else {
        Align::Left
    };
    let Some(mut p) = labels.text(look, images, font, s, colour, shadow, at, across) else {
        return;
    };
    if align & 4 != 0 {
        // Middle-anchored by the font's text height, as the text widget
        // places it (`choose::place`).
        let half = look
            .fonts
            .get(font)
            .map_or(0.0, |f| f.text_height as f32 / 2.0);
        p.rect.min.y -= half;
        p.rect.max.y -= half;
    }
    if let Some(room) = room
        && p.rect.width() > room
    {
        let s = room / p.rect.width();
        let pivot = if across == Align::Centre {
            at.x
        } else {
            p.rect.min.x
        };
        p.rect.min.x = pivot + (p.rect.min.x - pivot) * s;
        p.rect.max.x = pivot + (p.rect.max.x - pivot) * s;
    }
    out.push(p);
}

/// The layout's colour of an element (prop 10), or `plain`.
fn colour_of(layout: &Layout, id: u32, plain: [u8; 4]) -> [u8; 4] {
    element(layout, id)
        .and_then(|e| e.colour())
        .unwrap_or(plain)
}

/// A bracket screen's pieces: the plates, names, badges, headings, your
/// way through, the champion and the three prompts (`cursor` lights one:
/// 0 Continue, 1 Forfeit Tournament, 2 Back).
#[allow(clippy::too_many_arguments)]
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: &ScreenArt,
    b: &Bracket,
    name: &dyn Fn(Entrant) -> String,
    cursor: usize,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let plan = plan(b);
    let Some(layout) = look.layouts.iter().find(|l| l.class == plan.class) else {
        return out;
    };
    let file = match plan.class {
        SINGLE => "singleko.txt",
        DOUBLE => "doubleko.txt",
        _ => "bestof3.txt",
    };
    let Some(sheet) = art.sheet.get(2) else {
        return out;
    };
    let plate = |tint: Tint| art.plates.get(tint.index()).cloned();
    // Your way through, under the plates.
    for &id in &plan.paths {
        if plan.class == DOUBLE {
            if let Some(&(_, [x0, y0, x1, y1])) = LINES.iter().find(|l| l.0 == id) {
                let r = Rect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
                let mut p = Piece::new(
                    art.white.clone(),
                    Rect::new(r.min.x - 1.5, r.min.y - 1.5, r.max.x + 1.5, r.max.y + 1.5),
                );
                p.colour = Color::srgb_u8(ORANGE[0], ORANGE[1], ORANGE[2]);
                out.push(p);
            }
        } else if let Some(&(_, f, [x, y])) = CONNECTORS.iter().find(|c| c.0 == id) {
            out.push(frame(sheet, PLATE_FRAMES[f], Vec2::new(x, y)));
        }
    }
    // The plates and names.
    for slot in &plan.slots {
        if let (Some(at), Some(image)) = (place(layout, slot.id + 1000), plate(slot.tint)) {
            out.push(Piece::new(
                image,
                Rect::new(at.x, at.y, at.x + 125.0, at.y + 32.0),
            ));
        }
        if slot.entrant == Entrant::Empty {
            continue;
        }
        let Some(e) = element(layout, slot.id) else {
            continue;
        };
        let Some(at) = place(layout, slot.id) else {
            continue;
        };
        let colour = slot
            .colour
            .unwrap_or_else(|| colour_of(layout, slot.id, colours::DIM));
        let font = e.props.get(&13).map_or(fonts::LIST_SMALL, |&f| f as usize);
        text(
            &mut out,
            look,
            labels,
            images,
            font,
            &name(slot.entrant),
            colour,
            None,
            at,
            e.align,
            Some(117.0),
        );
    }
    // The champion: the name, its plate yellow if it is you.
    let (champ_text, champ_plate, _, standings) = champion_of(plan.class);
    let you = plan.champion == Some(Entrant::You);
    if let (Some(at), Some(image)) = (
        place(layout, champ_plate),
        plate(if you { Tint::Yellow } else { Tint::Own }),
    ) {
        out.push(Piece::new(
            image,
            Rect::new(at.x, at.y, at.x + 125.0, at.y + 32.0),
        ));
    }
    if let (Some(winner), Some(e), Some(at)) = (
        plan.champion,
        element(layout, champ_text),
        place(layout, champ_text),
    ) {
        let font = e.props.get(&13).map_or(fonts::LIST_SMALL, |&f| f as usize);
        let colour = if you { YELLOW } else { CREAM };
        text(
            &mut out,
            look,
            labels,
            images,
            font,
            &name(winner),
            colour,
            None,
            at,
            e.align,
            Some(117.0),
        );
    }
    // The badges.
    for &(id, badge) in &plan.badges {
        if let Some(at) = place(layout, id) {
            let f = if badge == Badge::Won { 2 } else { 1 };
            out.push(frame(sheet, PLATE_FRAMES[f], at));
        }
    }
    // The headings and the subtitle.
    for &(id, state) in &plan.headings {
        let (Some(e), Some(at)) = (element(layout, id), place(layout, id)) else {
            continue;
        };
        let words = e.texts().first().and_then(|&t| text_of(look, file, t));
        let Some(words) = words else { continue };
        let colour = match state {
            Heading::Current => YELLOW,
            Heading::Passed => CREAM,
            Heading::Plain => colour_of(layout, id, colours::PALE),
        };
        let font = e.props.get(&13).map_or(fonts::LIST, |&f| f as usize);
        text(
            &mut out, look, labels, images, font, &words, colour, None, at, e.align, None,
        );
    }
    let sub = subtitle_of(plan.class);
    if let (Some(e), Some(at)) = (element(layout, sub), place(layout, sub))
        && let Some(words) = e.texts().first().and_then(|&t| text_of(look, file, t))
    {
        let font = e.props.get(&13).map_or(fonts::LIST, |&f| f as usize);
        let colour = colour_of(layout, sub, GREY);
        text(
            &mut out, look, labels, images, font, &words, colour, None, at, e.align, None,
        );
    }
    // The prompts: Continue, Forfeit Tournament, Back; once the final is
    // played the forfeit gives way to "Final Standings." (the fills hide
    // it and show the standings line).
    for (row, &(id, icon, frame_no)) in prompts_of(plan.class).iter().enumerate() {
        if row == 1 && plan.champion.is_some() {
            continue;
        }
        let (Some(e), Some(at)) = (element(layout, id), place(layout, id)) else {
            continue;
        };
        if let (Some([icons, _]), Some(spot)) = (look.pad.as_ref(), place(layout, icon)) {
            let f = icon_frame(frame_no);
            let size = Vec2::new(f[2] as f32, f[3] as f32);
            out.push(frame(icons, f, spot - size / 2.0));
        }
        let Some(words) = e.texts().first().and_then(|&t| text_of(look, file, t)) else {
            continue;
        };
        let colour = if row == cursor {
            colours::SELECTED
        } else {
            colour_of(layout, id, GREY)
        };
        let font = e.props.get(&13).map_or(fonts::LIST_SMALL, |&f| f as usize);
        text(
            &mut out, look, labels, images, font, &words, colour, None, at, e.align, None,
        );
    }
    if plan.champion.is_some()
        && let (Some(e), Some(at)) = (element(layout, standings), place(layout, standings))
        && let Some(words) = e.texts().first().and_then(|&t| text_of(look, file, t))
    {
        let font = e.props.get(&13).map_or(fonts::LIST_SMALL, |&f| f as usize);
        let colour = colour_of(layout, standings, colours::HELP);
        text(
            &mut out, look, labels, images, font, &words, colour, None, at, e.align, None,
        );
    }
    out
}

/// The bracket a tournament's screen shows: the one under way or just
/// finished, else the one a start would make.
pub fn shown(story: &street_sim::story::Story, ev: usize) -> Bracket {
    story
        .brackets
        .iter()
        .find(|(e, _)| usize::from(*e) == ev)
        .map(|(_, b)| b.clone())
        .unwrap_or_else(|| story.tournament(ev))
}

/// The pieces the bracket and map pages draw on their layouts (none for
/// other pages).
#[allow(clippy::too_many_arguments)]
pub fn page_pieces(
    page: crate::menu::Page,
    cursor: usize,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: &ScreenArt,
    story: &street_sim::story::Story,
    disc: &crate::setup::Disc,
) -> Vec<Piece> {
    use crate::menu::Page;
    match page {
        Page::Bracket(ev) => {
            let b = shown(story, ev);
            // Your crew by its team's name (story +0xca0's, as the crew's
            // keyboards named it); a career from before the rewrite kept
            // it goes by its baller's.
            let you = if story.crew_team.name.is_empty() {
                story
                    .crew
                    .first()
                    .and_then(|&r| disc.players.get(usize::from(r)))
                    .map_or_else(String::new, |p| p.name())
            } else {
                story.crew_team.name.clone()
            };
            let name = |e: Entrant| entrant_name(e, &b, look, &you, &disc.teams);
            pieces(look, labels, images, art, &b, &name, cursor)
        }
        Page::Story => crate::map_view::map(look, labels, images, art, story, disc, cursor),
        Page::City(c) => crate::map_view::city(look, labels, images, art, story, disc, c, cursor),
        _ => Vec::new(),
    }
}

/// A text id of a bracket screen (`file << 8 | index`; the screens'
/// files are 0x3a..0x3c, past [`Look::text_id`]'s table).
fn text_of(look: &Look, file: &str, id: u16) -> Option<String> {
    look.text(file, usize::from(id & 0xff))
        .map(|s| s.replace("\\n", " "))
}

/// An entrant's name on the bracket: a generic crew is "Team %1d"
/// (`common.txt` 42), numbered from the bracket's first crew.
pub fn entrant_name(
    e: Entrant,
    b: &Bracket,
    look: &Look,
    you: &str,
    teams: &[street_assets::db::Team],
) -> String {
    match e {
        Entrant::Empty => String::new(),
        Entrant::You => you.to_owned(),
        // A boss crew by its team's nickname ("Stretch", "Osmosis" in the
        // grabs; the fill is cut in the decompile at `FUN_0030fa50`).
        Entrant::Boss(t) => teams
            .get(usize::from(t))
            .map_or_else(|| format!("Team {t}"), |t| t.nickname.clone()),
        Entrant::Crew(n) => {
            let first = b
                .rows
                .iter()
                .flat_map(|m| m.sides)
                .filter_map(|e| match e {
                    Entrant::Crew(n) => Some(n),
                    _ => None,
                })
                .min()
                .unwrap_or(n);
            look.text("common.txt", 42)
                .unwrap_or("Team %1d")
                .replace("%1d", &(n.wrapping_sub(first) + 1).to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(p: &Plan, id: u32) -> SlotPlan {
        *p.slots.iter().find(|s| s.id == id).unwrap()
    }

    #[test]
    fn a_fresh_single_elimination_lights_your_first_match() {
        let b = Bracket::new(0, Some(40), 100);
        let p = plan(&b);
        assert_eq!(p.class, SINGLE);
        assert_eq!(slot(&p, 110).entrant, Entrant::You);
        assert_eq!(slot(&p, 110).tint, Tint::Yellow);
        assert_eq!(slot(&p, 111).tint, Tint::Grey);
        assert_eq!(slot(&p, 140).entrant, Entrant::Boss(40));
        assert_eq!(slot(&p, 140).tint, Tint::Own);
        assert_eq!(p.headings[0], (20, Heading::Current));
        assert!(p.paths.is_empty() && p.badges.is_empty());
    }

    #[test]
    fn a_win_moves_you_on_with_its_badge_and_connector() {
        let mut b = Bracket::new(0, Some(40), 100);
        b.play(true);
        let p = plan(&b);
        assert_eq!(slot(&p, 110).tint, Tint::Orange);
        assert_eq!(slot(&p, 111).tint, Tint::Navy);
        assert_eq!(slot(&p, 210).tint, Tint::Yellow);
        assert!(p.badges.contains(&(510, Badge::Won)));
        assert_eq!(p.paths, vec![0xd06]);
        assert_eq!(p.headings[0], (20, Heading::Passed));
        assert_eq!(p.headings[1], (21, Heading::Current));
        b.play(true);
        b.play(true);
        let p = plan(&b);
        assert_eq!(p.champion, Some(Entrant::You));
        assert!(p.badges.contains(&(810, Badge::Won)));
        assert_eq!(p.paths, vec![0xd06, 0xd07, 0xd08]);
    }

    #[test]
    fn double_elimination_draws_the_losers_way() {
        let mut b = Bracket::new(2, Some(44), 1);
        b.play(false);
        let p = plan(&b);
        assert_eq!(p.class, DOUBLE);
        assert!(p.badges.contains(&(510, Badge::Lost)));
        assert_eq!(p.paths, vec![0xd1c, 0xd1d, 0xd1e]);
        // The losers' bracket is the left one, row 2: element 220.
        assert_eq!(slot(&p, 220).tint, Tint::Yellow);
        assert!(p.headings.contains(&(27, Heading::Current)));
        b.play(true);
        b.play(true);
        b.play(true);
        let p = plan(&b);
        assert_eq!(p.champion, Some(Entrant::You));
        assert!(p.paths.contains(&0xd0f));
    }

    #[test]
    fn best_of_three_badges_each_game() {
        let mut b = Bracket::new(4, Some(43), 9);
        b.play(true);
        b.play(false);
        let p = plan(&b);
        assert_eq!(p.class, BEST_OF_THREE);
        assert_eq!(p.badges, vec![(510, Badge::Won), (511, Badge::Lost)]);
        b.play(true);
        assert!(plan(&b).paths.contains(&0xd0b));
    }

    #[test]
    fn a_turned_picture_stands_up() {
        let t = ssh::Texture {
            name: String::new(),
            width: 2,
            height: 1,
            rgba: vec![1, 1, 1, 1, 2, 2, 2, 2],
            indices: None,
        };
        let u = stand_up(&t);
        assert_eq!((u.width, u.height), (1, 2));
        // The right-hand pixel is now on top.
        assert_eq!(u.rgba[0], 2);
    }
}
