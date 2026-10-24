//! Choose Players (screen class 0x1d, layout blob 9, backdrop 5): the
//! constructor `FUN_00269b28`, the build `FUN_00269c08`, the screen's
//! input `FUN_0026a860`, the read-outs `FUN_0026a198` and the frames'
//! colours `FUN_0026a068`. The trace is in `docs/FRONTEND_RECOVERY.md`,
//! "Choose Players' portrait grids".
//!
//! One screen for both sides: AWAY on the left, HOME on the right, each a
//! picker widget (`FUN_0022ff80`, at (118, 218) and (450, 218)) of
//! portrait tiles two across and three rows down, scrolling a row at a
//! time, with the focused player's name, size and seven skill meters
//! beside it. A side with people picks on its own (both sides at once
//! with people on both); a side nobody holds shows its three already
//! picked (`FUN_002323b8`). A pick checks its tile (two people on a side:
//! their numbers, the third "CPU"); the third readies the side ("Ready",
//! and in the League Challenge and Be A Legend a Continue to press); Triangle
//! takes the last pick back.

use bevy::prelude::*;
use street_assets::{
    db::Skill,
    font::{Element, Layout},
};

use crate::{
    controls::Device,
    look::{self, Align, Look, Piece, colours, fonts},
    portraits::Portraits,
    setup::Disc,
};

/// The layout's class.
pub const CLASS: u32 = 29;
/// Tiles across and rows shown.
pub const COLUMNS: usize = 2;
pub const ROWS_SHOWN: usize = 3;
/// Players a side picks.
pub const TEAM: usize = 3;

/// The game's mode as the screen tells it apart (game data `+0x78`): 2 and
/// 3 a Pick Up Game, 4 the League Challenge, 5 Be A Legend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    PickUp,
    League,
    Legend,
}

/// A tile: a player record, one not unlocked yet (dimmed, "Locked"), or
/// Be A Legend's empty places up to five (`0xb`, `0xc`: the portrait
/// hidden, the tile disabled).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Player(usize),
    Locked(usize),
    Empty,
}

impl Tile {
    pub fn record(self) -> Option<usize> {
        match self {
            Tile::Player(r) | Tile::Locked(r) => Some(r),
            Tile::Empty => None,
        }
    }
}

/// All-Stars' 25 (`0x46ba28`), the list team 39 shows to a person.
pub const LEAGUE_STARS: [usize; 25] = [
    100, 55, 56, 130, 95, 80, 115, 75, 82, 40, 5, 21, 22, 20, 141, 15, 50, 105, 32, 120, 60, 86,
    116, 45, 42,
];
/// Old School's legends (records 0x9e..=0xb7) and the one among them who
/// shows a question mark until unlocked (0xa7, `FUN_00223dd0`).
pub const OLD_SCHOOL: std::ops::RangeInclusive<usize> = 0x9e..=0xb7;
pub const SECRET: usize = 0xa7;
const OLD_SCHOOL_TEAM: usize = 38;
const LEAGUE_STARS_TEAM: usize = 39;

/// A side's tiles (`FUN_00231b40`): a side nobody holds shows only its
/// three when its team is a legends' team (38, 39); All-Stars shows its
/// 25 to a person, Old School its legends, the unlocked first and the
/// locked after them with the secret one last (`FUN_002320d8`); other
/// teams their roster, up to six; Be A Legend's crew pads to five with
/// empty places.
pub fn tiles(
    team: usize,
    roster: &[usize],
    lineup: [usize; TEAM],
    held: bool,
    mode: Mode,
    locked: &dyn Fn(usize) -> bool,
) -> Vec<Tile> {
    let mut out: Vec<Tile> = if mode == Mode::Legend && held {
        // Be A Legend's crew (`FUN_001ebf80` over the career's team).
        roster.iter().take(6).map(|&r| Tile::Player(r)).collect()
    } else if !held && matches!(team, OLD_SCHOOL_TEAM | LEAGUE_STARS_TEAM) {
        lineup.iter().map(|&r| Tile::Player(r)).collect()
    } else if team == LEAGUE_STARS_TEAM {
        LEAGUE_STARS.iter().map(|&r| Tile::Player(r)).collect()
    } else if team == OLD_SCHOOL_TEAM {
        let open = OLD_SCHOOL.filter(|&r| !locked(r)).map(Tile::Player);
        let shut = OLD_SCHOOL
            .filter(|&r| locked(r) && r != SECRET)
            .chain(std::iter::once(SECRET).filter(|&r| locked(r)))
            .map(Tile::Locked);
        open.chain(shut).collect()
    } else {
        roster.iter().take(6).map(|&r| Tile::Player(r)).collect()
    };
    // `FUN_002321b0`: a side nobody holds lists only its line-up's players
    // (a line-up from outside its roster, as a generic crew's, all three).
    if !held {
        out.retain(|t| t.record().is_some_and(|r| lineup.contains(&r)));
        for &r in &lineup {
            if !out.iter().any(|t| t.record() == Some(r)) {
                out.push(Tile::Player(r));
            }
        }
    }
    if mode == Mode::Legend && held {
        while out.len() < 5 {
            out.push(Tile::Empty);
        }
    }
    out
}

/// One side's picker widget.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picker {
    pub tiles: Vec<Tile>,
    /// The focused tile, and the first row shown.
    pub cursor: usize,
    pub first: usize,
    /// The picked tiles in order (`+0xc0`).
    pub picks: Vec<usize>,
    /// The side's people, 1P first (`+0xba`, `+0xbb`), and whose turn it is
    /// (`+0xb8`).
    pub people: Vec<Device>,
    pub turn: usize,
    /// Three picked in the League Challenge or Be A Legend: "Ready" over a
    /// Continue (`+0xcc`).
    pub ready: bool,
    /// The side is finished (`+200`): the third pick in a Pick Up Game, the
    /// Continue otherwise, and a side nobody holds from the start.
    pub done: bool,
}

/// What a press on a picker did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Picked,
    /// The third pick: Ready.
    Ready,
    /// The side is finished.
    Done,
    /// A picked, locked or empty tile: sound 6 (`FUN_001a1908(.., 6, 1)`).
    Refused,
    Undone,
    /// Nothing to take back: the screen goes back.
    Leave,
    Nothing,
}

impl Picker {
    /// A side with people. In Be A Legend the career's baller, the first
    /// tile, is picked from the start (`FUN_00230938`) and stays.
    pub fn held(tiles: Vec<Tile>, people: Vec<Device>, mode: Mode) -> Self {
        let mut p = Picker {
            tiles,
            people,
            ..default()
        };
        if mode == Mode::Legend && matches!(p.tiles.first(), Some(Tile::Player(_))) {
            p.pick(mode);
        }
        p
    }

    /// A side nobody holds: its line-up picked (`FUN_002323b8`), the focus
    /// on the last.
    pub fn computer(tiles: Vec<Tile>, lineup: [usize; TEAM]) -> Self {
        let picks: Vec<usize> = lineup
            .iter()
            .filter_map(|&r| tiles.iter().position(|t| t.record() == Some(r)))
            .collect();
        let cursor = picks.last().copied().unwrap_or(0);
        Picker {
            first: scroll(0, cursor),
            cursor,
            tiles,
            picks,
            done: true,
            ready: true,
            ..default()
        }
    }

    /// The player the side's read-out shows: the focused tile's.
    pub fn shown(&self) -> Option<Tile> {
        self.tiles.get(self.cursor).copied()
    }

    /// The picker as shown `ticks` (60 a second) after the screen opened:
    /// a side nobody holds moves its highlight through its picks, the
    /// last pick dimmed (0x5f) and the next lit (`FUN_002324d8`, the timer
    /// `FUN_002323b8` sets with `FUN_002da198(.., 1, 0)`, a second), the
    /// next by a counter every picker shares (`0x3f68ac`, 0..2), here from
    /// 0 when the screen opens; before the first second its last pick.
    pub fn cycled(&self, ticks: u64) -> Self {
        let mut p = self.clone();
        let seconds = ticks / 60;
        if p.people.is_empty() && seconds > 0 && !p.picks.is_empty() {
            p.cursor = p.picks[(seconds as usize - 1) % p.picks.len()];
        }
        p
    }

    /// The picked records, once there are three.
    pub fn lineup(&self) -> Option<[usize; TEAM]> {
        let records: Vec<usize> = self
            .picks
            .iter()
            .filter_map(|&k| self.tiles.get(k)?.record())
            .collect();
        records.try_into().ok()
    }

    /// The person whose turn it is, or every person on the side once it is
    /// ready (`FUN_002318b8` gives them all the focus).
    pub fn takes(&self, device: Device) -> bool {
        if self.ready || self.done {
            self.people.contains(&device)
        } else {
            self.people.get(self.turn) == Some(&device)
        }
    }

    /// The links `FUN_0022ff80` sets between the six shown tiles: across in
    /// reading order (the first tile's left and the last's right stop),
    /// up and down a row, the top row's up and the bottom row's down
    /// scrolling the list a row.
    pub fn step(&mut self, dx: i32, dy: i32) -> bool {
        if self.ready || self.done {
            return false;
        }
        let at = self.cursor;
        let (row, col) = (at / COLUMNS, at % COLUMNS);
        let to = match (dx.signum(), dy.signum()) {
            (1, 0) if col + 1 < COLUMNS || row + 1 < self.first + ROWS_SHOWN => Some(at + 1),
            (-1, 0) if col > 0 || row > self.first => at.checked_sub(1),
            (0, 1) => Some(at + COLUMNS),
            (0, -1) => at.checked_sub(COLUMNS),
            _ => None,
        };
        match to {
            Some(to) if to < self.tiles.len() => {
                self.cursor = to;
                self.first = scroll(self.first, to);
                true
            }
            _ => false,
        }
    }

    /// Cross (`FUN_002315b8`).
    pub fn pick(&mut self, mode: Mode) -> Press {
        if self.done {
            return Press::Nothing;
        }
        if self.picks.len() >= TEAM {
            // The Continue under Ready.
            self.done = true;
            return Press::Done;
        }
        let at = self.cursor;
        if self.picks.contains(&at) || !matches!(self.tiles.get(at), Some(Tile::Player(_))) {
            return Press::Refused;
        }
        self.picks.push(at);
        if self.picks.len() == TEAM {
            if mode == Mode::PickUp {
                self.done = true;
                return Press::Done;
            }
            self.ready = true;
            return Press::Ready;
        }
        // Two people on a side take turns (`+0xb8`).
        if self.people.len() > 1 {
            self.turn ^= 1;
        }
        // `FUN_00220dd0`: on to the next tile not picked.
        if let Some(next) = (at + 1..self.tiles.len())
            .chain(0..at)
            .find(|k| !self.picks.contains(k) && matches!(self.tiles[*k], Tile::Player(_)))
        {
            self.cursor = next;
            self.first = scroll(self.first, next);
        }
        Press::Picked
    }

    /// Triangle (`FUN_002312e8`): the last pick back, the focus on it; Be A
    /// Legend's baller stays. Nothing to take back leaves the screen
    /// (`FUN_0026a860`, key 0x25).
    pub fn undo(&mut self, mode: Mode) -> Press {
        let keep = usize::from(mode == Mode::Legend);
        if self.picks.len() <= keep {
            return Press::Leave;
        }
        let Some(at) = self.picks.pop() else {
            return Press::Leave;
        };
        if !self.ready && !self.done && self.people.len() > 1 {
            self.turn ^= 1;
        }
        self.ready = false;
        self.done = false;
        self.cursor = at;
        self.first = scroll(self.first, at);
        Press::Undone
    }
}

/// The first row shown so that `cursor`'s row is in view.
pub fn scroll(first: usize, cursor: usize) -> usize {
    let row = cursor / COLUMNS;
    if row < first {
        row
    } else if row >= first + ROWS_SHOWN {
        row + 1 - ROWS_SHOWN
    } else {
        first
    }
}

/// The two sides' pickers, by the original's side: 0 AWAY (left), 1 HOME
/// (right).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Screen {
    pub mode: Mode,
    pub sides: [Picker; 2],
    /// The rewrite's home side (its match side 0) on the left. A Pick Up
    /// Game puts home on the right; Be A Legend's crew, which the rewrite
    /// plays as home, is the original's side 0, on the left (`FUN_001db308`
    /// in mode 5).
    pub home_left: bool,
}

/// The panel a match side sits in with home on the right.
pub fn panel(side: usize) -> usize {
    if side == 0 { 1 } else { 0 }
}

impl Screen {
    /// The panel a match side (0 home, 1 away) sits in.
    pub fn panel(&self, side: usize) -> usize {
        if self.home_left { side } else { panel(side) }
    }

    /// The picker for a match side.
    pub fn side(&self, side: usize) -> &Picker {
        &self.sides[self.panel(side)]
    }

    /// Both sides finished: Cross goes on (`FUN_0026a860`, key 0x23).
    pub fn finished(&self) -> bool {
        self.sides.iter().all(|p| p.done)
    }
}

/// Picker geometry (`FUN_0022ff80`): the first tile's middle, and the
/// tiles 74 by 73 (the frame, `XAFESCRN` sheet 0, image 5).
const ORIGIN: [Vec2; 2] = [Vec2::new(118.0, 218.0), Vec2::new(450.0, 218.0)];
/// The read-out's elements by side (`0x476940`, `0x47696c`): the line over
/// the name ("?" for the secret legend), the first name, the last name,
/// the size, then the seven meters (Shots, Dunks, Power, Blocks, Steals,
/// Handles, Rebounds).
const READOUT: [[u32; 11]; 2] = [
    [55, 62, 54, 53, 15, 7, 12, 6, 17, 8, 13],
    [58, 63, 57, 56, 37, 29, 34, 28, 39, 30, 35],
];
/// The meters' labels by side (Shots .. Rebounds).
const LABELS: [[u32; 7]; 2] = [[23, 19, 21, 18, 24, 20, 22], [45, 41, 43, 40, 46, 42, 44]];
pub(crate) const SKILLS: [Skill; 7] = [
    Skill::Shots,
    Skill::Dunks,
    Skill::Power,
    Skill::Blocks,
    Skill::Steals,
    Skill::Handles,
    Skill::Rebounds,
];
/// The pieces `FUN_0026a068` colours by the side's controller (`0x4768e0`):
/// four lines, the corner and the tab; and the tab's word (`0x476910`).
const FRAME: [[u32; 4]; 2] = [[60, 31, 9, 0], [61, 36, 14, 1]];
const CORNER: [u32; 2] = [16, 38];
const TAB: [u32; 2] = [27, 49];
const WORD: [u32; 2] = [11, 33];
/// A tile's brightness (`+0x134`): the picker's list dims the tiles it
/// does not focus (its `+0x124`, set by `FUN_0022ff80`: `FUN_00222420`
/// and `FUN_00222368` give 0x9f, `FUN_002222c8` the focused 0xff); a
/// picked or locked tile 0x5f (`FUN_002310a0`), a disabled one 0x7f (the
/// tiles not picked once the challenges' sides are ready, `FUN_00230938`),
/// an empty place and a ready side's checks 0x3f (`FUN_00230dc8`).
pub(crate) const FOCUSED: f32 = 1.0;
pub(crate) const UNFOCUSED: f32 = 0x9f as f32 / 255.0;
pub(crate) const PICKED: f32 = 0x5f as f32 / 255.0;
const DISABLED: f32 = 0x7f as f32 / 255.0;
pub(crate) const EMPTY: f32 = 0x3f as f32 / 255.0;
/// A tile's label (`FUN_00223220`'s text 0x452: font 1, 0xfbac00, a 2-pixel
/// black outline): its right edge and top from the tile's middle; the
/// check (0x453) right-anchored 13 pixels higher.
const LABEL_AT: Vec2 = Vec2::new(27.0, 11.5);
const CHECK_AT: Vec2 = Vec2::new(27.0, -1.5);
/// "Locked" (0x451's 0x12b) in 0xff838497.
const LOCKED_LABEL: [u8; 4] = [0x83, 0x84, 0x97, 0xff];

/// One side as drawn: its picker, the controller its frame shows (the
/// person whose turn it is; none for the computer's, `FUN_002103e8`), and
/// each person's number for two people's labels.
pub struct SideLook<'a> {
    pub picker: &'a Picker,
    pub controller: Option<usize>,
    pub numbers: Vec<usize>,
}

/// Places one line of text by a layout element's anchor (`align & 3`
/// across, `(align >> 2) & 3` down, by the font's text height).
#[allow(clippy::too_many_arguments)]
pub(crate) fn place(
    out: &mut Vec<Piece>,
    labels: &mut look::Labels,
    look: &Look,
    images: &mut Assets<Image>,
    font: usize,
    s: &str,
    colour: [u8; 4],
    at: Vec2,
    align: u8,
) {
    let Some(f) = look.fonts.get(font) else {
        return;
    };
    let across = match align & 3 {
        1 => Align::Centre,
        2 => Align::Right,
        _ => Align::Left,
    };
    let height = f.text_height as f32;
    let top = at.y
        - match (align >> 2) & 3 {
            1 => height / 2.0,
            2 => height,
            _ => 0.0,
        };
    if let Some(p) = labels.text(
        look,
        images,
        font,
        s,
        colour,
        None,
        Vec2::new(at.x, top),
        across,
    ) {
        out.push(p);
    }
}

/// A layout element by its id, not the screen's own (kind 0, whose id is
/// the class, 29, which a meter shares).
pub(crate) fn element(layout: &Layout, id: u32) -> Option<&Element> {
    layout.elements.iter().find(|e| e.id == id && e.kind != 0)
}

pub(crate) fn spot(e: &Element) -> Option<Vec2> {
    e.position()
        .map(|(x, y)| Vec2::new(f32::from(x as i16), f32::from(y as i16)))
}

/// The screen's pieces over its layout (the title, the help and the Help
/// legend come with the layout): `sides` by the original's side (AWAY,
/// HOME).
#[allow(clippy::too_many_arguments)]
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    portraits: Option<&mut Portraits>,
    disc: &Disc,
    sides: [SideLook; 2],
    mode: Mode,
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
    for (s, side) in sides.iter().enumerate() {
        let picker = side.picker.cycled(ticks);
        let side = &SideLook {
            picker: &picker,
            controller: side.controller,
            numbers: side.numbers.clone(),
        };
        // The frame in the controller's palette (`XPAL_FE` sheet 0) or the
        // sheet's own grey for the computer's side.
        let variant = side.controller.map_or(0, |c| c.min(3) + 1);
        frame(&mut out, layout, pm, &FRAME[s], CORNER[s], TAB[s], variant);
        // AWAY / HOME (0xc03, 0xc04), black on 2P's yellow tab.
        if let Some(e) = element(layout, WORD[s]) {
            let word =
                look.text_id(0xc03 + s as u16)
                    .unwrap_or(if s == 0 { "AWAY" } else { "HOME" });
            let colour = if side.controller == Some(1) {
                [0, 0, 0, 0xff]
            } else {
                colours::CREAM
            };
            if let Some(at) = spot(e) {
                let font = e.props.get(&13).map_or(fonts::SMALL, |&f| f as usize);
                place(
                    &mut out, labels, look, images, font, word, colour, at, e.align,
                );
            }
        }
        readout(
            &mut out,
            look,
            labels,
            images,
            layout,
            pm,
            disc,
            (&READOUT[s], &LABELS[s]),
            side.picker.shown(),
        );
        grid(
            &mut out,
            look,
            labels,
            images,
            portraits.as_deref_mut(),
            pm,
            disc,
            s,
            side,
            mode,
            ticks,
        );
    }
    out
}

/// A panel's frame (kind 32 lines, the corner and the tab, kind 3) in a
/// controller's palette variant (0 the sheet's own, `1 + k` for `k`P).
pub(crate) fn frame(
    out: &mut Vec<Piece>,
    layout: &Layout,
    pm: &look::PrematchArt,
    lines: &[u32],
    corner: u32,
    tab: u32,
    variant: usize,
) {
    for &id in lines {
        if let Some(e) = element(layout, id)
            && let Some(at) = spot(e)
        {
            let size = e
                .size()
                .map_or(Vec2::ZERO, |(w, h)| Vec2::new(f32::from(w), f32::from(h)));
            out.push(Piece::new(
                pm.line[variant].clone(),
                Rect::from_corners(at, at + size),
            ));
        }
    }
    if let Some(at) = element(layout, corner).and_then(spot) {
        out.push(Piece::new(
            pm.corner[variant].clone(),
            Rect::from_corners(at, at + pm.corner_size),
        ));
    }
    if let Some(at) = element(layout, tab).and_then(spot) {
        out.push(Piece::new(
            pm.tab_wide[variant].clone(),
            Rect::from_corners(at, at + pm.tab_wide_size),
        ));
    }
}

/// A player's read-out (`FUN_0026a198`, Update Team's `FUN_002d66c0`):
/// `ids` the line over the name, the first and last names, the size and
/// the seven meters; `labels` the meters' words (their elements' own
/// text). Nothing shown blanks the lines and empties the meters.
#[allow(clippy::too_many_arguments)]
pub(crate) fn readout(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    layout: &Layout,
    pm: &look::PrematchArt,
    disc: &Disc,
    (ids, label_ids): (&[u32; 11], &[u32; 7]),
    shown: Option<Tile>,
) {
    let player = shown
        .and_then(|t| t.record())
        .and_then(|r| disc.players.get(r));
    let secret = matches!(shown, Some(Tile::Locked(SECRET)));
    let lines: [String; 4] = match player {
        _ if secret => ["?".into(), "?".into(), "?".into(), String::new()],
        Some(p) => [
            String::new(),
            p.first_name.clone(),
            p.last_name.clone(),
            format!(
                "{}'{}\" {} lbs",
                p.height_in / 12,
                p.height_in % 12,
                p.weight_lb
            ),
        ],
        None => Default::default(),
    };
    for (k, line) in lines.iter().enumerate() {
        let Some(e) = element(layout, ids[k]) else {
            continue;
        };
        let (Some(at), font) = (
            spot(e),
            e.props.get(&13).map_or(fonts::LIST, |&f| f as usize),
        ) else {
            continue;
        };
        // The name lines in 0xfbac00 (`+0x4a4`), the size in the layout's
        // 0x98a8b9.
        let colour = if k < 3 {
            colours::HELP
        } else {
            e.props
                .get(&10)
                .map_or(colours::LEGEND, |&c| street_assets::font::argb(c))
        };
        place(out, labels, look, images, font, line, colour, at, e.align);
    }
    for (k, skill) in SKILLS.iter().enumerate() {
        if let Some(at) = element(layout, ids[4 + k]).and_then(spot) {
            let level = if secret {
                0
            } else {
                player.map_or(0, |p| {
                    crate::progression_trace::skill_bar(*skill, p.ratings[skill.rating()])
                })
            };
            out.extend(crate::chooseteam::meter(pm, at, level));
        }
        if let Some(e) = element(layout, label_ids[k]) {
            let name = e
                .props
                .get(&14)
                .and_then(|&id| look.text_id(id as u16))
                .or(look.text("common.txt", 10 + k))
                .unwrap_or("");
            let colour = e
                .props
                .get(&10)
                .map_or(colours::LEGEND, |&c| street_assets::font::argb(c));
            let font = e.props.get(&13).map_or(fonts::SMALL, |&f| f as usize);
            if let Some(at) = spot(e) {
                place(out, labels, look, images, font, name, colour, at, e.align);
            }
        }
    }
}

/// A side's grid: the tiles shown, the cursor, the checks and labels, the
/// scroll arrows, and Ready with its Continue.
#[allow(clippy::too_many_arguments)]
fn grid(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    mut portraits: Option<&mut Portraits>,
    pm: &look::PrematchArt,
    disc: &Disc,
    s: usize,
    side: &SideLook,
    mode: Mode,
    ticks: u64,
) {
    let p = side.picker;
    let origin = ORIGIN[s];
    let pitch = pm.tile_size;
    let held = !p.people.is_empty();
    let mut marks = Vec::new();
    let mut cursor = None;
    for row in 0..ROWS_SHOWN {
        for col in 0..COLUMNS {
            let index = (p.first + row) * COLUMNS + col;
            let Some(&tile) = p.tiles.get(index) else {
                continue;
            };
            let centre = origin + Vec2::new(col as f32 * pitch.x, row as f32 * pitch.y);
            let picked = p.picks.iter().position(|&k| k == index);
            let finished = p.ready || p.done;
            let focused = index == p.cursor && held && !finished;
            let bright = match tile {
                Tile::Empty => EMPTY,
                Tile::Locked(_) => PICKED,
                // The computer's side keeps its last pick lit.
                Tile::Player(_) if picked.is_some() && !held && index == p.cursor => FOCUSED,
                Tile::Player(_) if picked.is_some() => PICKED,
                Tile::Player(_) if focused => FOCUSED,
                Tile::Player(_) if held && finished && mode != Mode::PickUp => DISABLED,
                Tile::Player(_) => UNFOCUSED,
            };
            tile_pieces(
                out,
                images,
                portraits.as_deref_mut(),
                pm,
                disc,
                centre,
                (tile, false),
                bright,
            );
            // A person's pick: the check, or with two people on the side
            // their numbers and "CPU" (`FUN_002315b8`); the computer's
            // side shows neither. "Locked" on a locked tile.
            if let Some(n) = picked
                && held
            {
                if side.numbers.len() > 1 {
                    let label = match n {
                        0 | 1 => look
                            .text(
                                "common_fe.txt",
                                38 + side.numbers.get(n).copied().unwrap_or(0),
                            )
                            .map(str::to_owned),
                        _ => look.text("common_fe.txt", 42).map(str::to_owned),
                    };
                    if let Some(label) = label {
                        marks.push((centre, Some((label, colours::HELP))));
                    }
                } else {
                    marks.push((centre, None));
                }
            } else if matches!(tile, Tile::Locked(_))
                && let Some(label) = look.text("common_fe.txt", 43)
            {
                marks.push((centre, Some((label.to_owned(), LOCKED_LABEL))));
            }
            // The cursor: on the person's tile while the side picks; on the
            // computer's last pick (`FUN_002323b8` puts it back,
            // `FUN_0021b500`); gone once a person's side is ready
            // (`FUN_002318b8`).
            if index == p.cursor && (focused || !held) {
                cursor = Some(centre);
            }
        }
    }
    for (centre, label) in marks {
        match label {
            None => {
                let right_top = centre + CHECK_AT;
                let min = Vec2::new(right_top.x - pm.check_size.x, right_top.y);
                let mut check = Piece::new(
                    pm.check.clone(),
                    Rect::from_corners(min, min + pm.check_size),
                );
                if p.ready || p.done {
                    check.colour = Color::srgb(EMPTY, EMPTY, EMPTY);
                }
                out.push(check);
            }
            Some((text, colour)) => {
                let at = centre + LABEL_AT;
                if let Some(piece) = labels.text(
                    look,
                    images,
                    fonts::LIST_SMALL,
                    &text,
                    colour,
                    Some(([0, 0, 0, 0xff], [0, 0, 0, 0xff])),
                    at,
                    Align::Right,
                ) {
                    out.push(piece);
                }
            }
        }
    }
    // The cursor (`FUN_0021b550` case 0: image 0, frame 1, stretched to the
    // tile and 2), with the rows' pulse.
    if let Some(centre) = cursor {
        out.push(cursor_piece(pm, centre, ticks));
    }
    // The scroll arrows (`FUN_0021eed8`): 48 left of and 26 above the
    // first tile on the left, 2 tiles less 27 right of it on the right;
    // the down arrow three rows less 20 below.
    if held && !(p.ready || p.done) {
        let rows = p.tiles.len().div_ceil(COLUMNS);
        let up = if s == 0 {
            origin + Vec2::new(-48.0, -26.0)
        } else {
            origin + Vec2::new(2.0 * pitch.x - 27.0, -26.0)
        };
        let down = up + Vec2::new(0.0, 3.0 * pitch.y - 20.0);
        let half = pm.arrow_size / 2.0;
        if p.first > 0 {
            out.push(Piece::new(
                pm.arrow_up.clone(),
                Rect::from_corners(up - half, up + half),
            ));
        }
        if p.first + ROWS_SHOWN < rows {
            out.push(Piece::new(
                pm.arrow_down.clone(),
                Rect::from_corners(down - half, down + half),
            ));
        }
    }
    // Ready (element 8: font 6, 0xffdcdcbb, centred 35 right of and 74
    // below the first tile's middle), and in the League Challenge and Be A
    // Legend the Continue under it (element 9, `FUN_0021a530`: Cross and
    // 0x14, top-anchored).
    if held && (p.ready || p.done) {
        let ready = origin + Vec2::new(35.0, 3.0 * pitch.y - 145.0);
        let word = look.text("common_fe.txt", 10).unwrap_or("Ready");
        place(
            out,
            labels,
            look,
            images,
            fonts::TITLE,
            word,
            colours::CREAM,
            ready,
            1 | (1 << 2),
        );
        if mode != Mode::PickUp && !p.done {
            let height = look
                .fonts
                .get(fonts::TITLE)
                .map_or(30.0, |f| f.text_height as f32);
            // `FUN_0022ff80`: Ready's point (its middle) plus its height and
            // 4, the legend's top.
            let at = ready + Vec2::new(0.0, height + 4.0);
            let mut e = Element {
                kind: 34,
                id: 9,
                align: 1,
                props: Default::default(),
            };
            e.props.insert(
                3,
                (at.x as i16 as u16 as u32) | ((at.y as i16 as u16 as u32) << 16),
            );
            e.props.insert(13, fonts::LIST_SMALL as u32);
            e.props.insert(10, 0xffdcdcbb);
            e.props.insert(56, 0x14);
            e.props.insert(60, 0);
            look::legend(out, labels, look, images, &e, &|id| {
                look.text_id(id).map(str::to_owned)
            });
        }
    }
}

/// A portrait tile (`FUN_00223698`) centred on `centre` at `bright`: the
/// back in the portrait's palette, the portrait cut to its frame at the
/// back's size (the secret legend's question mark over the whole tile, an
/// empty place none), the frame over them; `small` the tile `+0xd0` clear
/// makes (Create Team's grid).
#[allow(clippy::too_many_arguments)]
pub(crate) fn tile_pieces(
    out: &mut Vec<Piece>,
    images: &mut Assets<Image>,
    portraits: Option<&mut Portraits>,
    pm: &look::PrematchArt,
    disc: &Disc,
    centre: Vec2,
    (tile, small): (Tile, bool),
    bright: f32,
) {
    let (backs, back_size, frame_image, frame_size) = if small {
        (
            &pm.tile_back_small,
            pm.tile_back_small_size,
            &pm.tile_frame_small,
            pm.tile_small_size,
        )
    } else {
        (
            &pm.tile_back,
            pm.tile_back_size,
            &pm.tile_frame,
            pm.tile_size,
        )
    };
    let tint = Color::srgb(bright, bright, bright);
    let record = tile.record();
    let player = record.and_then(|r| disc.players.get(r));
    // The back in the portrait's palette (`FUN_00223698`).
    let palette = match (tile, player) {
        (Tile::Locked(SECRET), _) => None,
        (_, Some(p)) if p.team == 37 => Some(11),
        (Tile::Empty, _) => Some(0),
        (_, Some(p)) => Some(back_palette(i32::from(p.portrait))),
        (_, None) => Some(8),
    };
    if let Some(k) = palette
        && let Some(back) = backs.get(k).or(backs.first())
    {
        let mut b = Piece::new(back.clone(), Rect::from_center_size(centre, back_size));
        b.colour = tint;
        out.push(b);
    }
    // The portrait at the back's size; the secret legend's
    // question mark over the whole tile; an empty place none.
    let picture = match (tile, player) {
        (Tile::Locked(SECRET), _) => Some((
            pm.tile_mystery.clone(),
            if small {
                frame_size
            } else {
                pm.tile_mystery_size
            },
        )),
        (Tile::Empty, _) => None,
        (_, Some(p)) => {
            let sheet = if p.team == 37 {
                crate::portraits::ICONS + usize::try_from(p.portrait).unwrap_or(0).min(6)
            } else {
                usize::try_from(p.portrait).unwrap_or(crate::portraits::NONE)
            };
            portraits
                .and_then(|ps| ps.get(images, sheet))
                .map(|h| (h, back_size))
        }
        _ => None,
    };
    if let Some((image, size)) = picture {
        let mut pic = Piece::new(image, Rect::from_center_size(centre, size));
        pic.colour = tint;
        out.push(pic);
    }
    let mut frame = Piece::new(
        frame_image.clone(),
        Rect::from_center_size(centre, frame_size),
    );
    frame.colour = tint;
    out.push(frame);
}

/// The cursor (`FUN_0021b550` case 0: `XAFESCRN` image 0, frame 1,
/// stretched to the tile and 2) on `centre`, with the rows' pulse.
pub(crate) fn cursor_piece(pm: &look::PrematchArt, centre: Vec2, ticks: u64) -> Piece {
    let mut c = Piece::new(
        pm.cursor.clone(),
        Rect::from_center_size(centre, pm.tile_size + Vec2::splat(2.0)),
    );
    c.colour = Color::WHITE.with_alpha(f32::from(look::row_pulse(ticks)) / 255.0);
    c
}

/// The back's palette by the portrait sheet (`FUN_00223698`): 2, 9 and 10
/// for the legends' and streetballers' sheets, 0 for the league's.
fn back_palette(sheet: i32) -> usize {
    match sheet {
        -1 => 8,
        0x91..=0xa9 | 0xe9 => 2,
        0xaa..=0xaf | 0xea..=0xf0 => 9,
        0xb7..=0xe8 => 10,
        _ => 0,
    }
}

// The layout's sides and the picker's geometry, checked when compiling.
const _: () = {
    assert!(ORIGIN[1].x - ORIGIN[0].x == 332.0);
    assert!(READOUT[0].len() == 4 + SKILLS.len());
};

#[cfg(test)]
mod tests {
    use super::*;

    fn people() -> Vec<Device> {
        vec![Device::Keyboard]
    }

    fn players(n: usize) -> Vec<Tile> {
        (0..n).map(Tile::Player).collect()
    }

    #[test]
    fn home_stands_right_and_away_left() {
        assert_eq!(panel(0), 1);
        assert_eq!(panel(1), 0);
    }

    #[test]
    fn the_cursor_reads_across_and_scrolls_a_row() {
        let mut p = Picker::held(players(8), people(), Mode::PickUp);
        assert!(!p.step(-1, 0), "the first tile's left stops");
        assert!(p.step(1, 0));
        assert_eq!(p.cursor, 1);
        assert!(p.step(1, 0), "the row's end goes on to the next row");
        assert_eq!(p.cursor, 2);
        assert!(p.step(-1, 0));
        assert_eq!(p.cursor, 1);
        assert!(p.step(0, 1) && p.step(0, 1));
        assert_eq!((p.cursor, p.first), (5, 0));
        assert!(!p.step(1, 0), "the last shown tile's right stops");
        assert!(p.step(0, 1), "down from the bottom row scrolls");
        assert_eq!((p.cursor, p.first), (7, 1));
        assert!(!p.step(0, 1), "no ninth tile");
        assert!(p.step(0, -1) && p.step(0, -1) && p.step(0, -1));
        assert_eq!((p.cursor, p.first), (1, 0));
    }

    #[test]
    fn three_picks_ready_a_side_and_triangle_takes_them_back() {
        let mut p = Picker::held(players(5), people(), Mode::PickUp);
        assert_eq!(p.pick(Mode::PickUp), Press::Picked);
        assert_eq!(p.cursor, 1, "on to the next tile not picked");
        p.cursor = 0;
        assert_eq!(p.pick(Mode::PickUp), Press::Refused, "picked already");
        p.cursor = 3;
        assert_eq!(p.pick(Mode::PickUp), Press::Picked);
        assert_eq!(p.cursor, 4);
        assert_eq!(p.pick(Mode::PickUp), Press::Done);
        assert_eq!(p.lineup(), Some([0, 3, 4]));
        assert_eq!(p.pick(Mode::PickUp), Press::Nothing);
        assert_eq!(p.undo(Mode::PickUp), Press::Undone);
        assert!(!p.done);
        assert_eq!(p.cursor, 4, "the focus goes to the pick taken back");
        assert_eq!(p.undo(Mode::PickUp), Press::Undone);
        assert_eq!(p.undo(Mode::PickUp), Press::Undone);
        assert_eq!(p.undo(Mode::PickUp), Press::Leave);
    }

    #[test]
    fn the_challenges_wait_for_continue_and_the_legend_keeps_the_baller() {
        let mut tiles = players(3);
        tiles.extend([Tile::Empty, Tile::Empty]);
        let mut p = Picker::held(tiles, people(), Mode::Legend);
        assert_eq!(p.picks, [0], "the baller is picked from the start");
        p.cursor = 3;
        assert_eq!(p.pick(Mode::Legend), Press::Refused, "an empty place");
        p.cursor = 1;
        assert_eq!(p.pick(Mode::Legend), Press::Picked);
        assert_eq!(p.pick(Mode::Legend), Press::Ready);
        assert!(p.ready && !p.done);
        assert_eq!(p.pick(Mode::Legend), Press::Done, "Continue");
        assert_eq!(p.undo(Mode::Legend), Press::Undone);
        assert_eq!(p.undo(Mode::Legend), Press::Undone);
        assert_eq!(p.undo(Mode::Legend), Press::Leave, "the baller stays");
    }

    #[test]
    fn two_people_on_a_side_take_turns() {
        let pad = Device::Pad(Entity::from_raw_u32(7).unwrap());
        let mut p = Picker::held(players(6), vec![Device::Keyboard, pad], Mode::PickUp);
        assert!(p.takes(Device::Keyboard) && !p.takes(pad));
        p.pick(Mode::PickUp);
        assert!(p.takes(pad) && !p.takes(Device::Keyboard));
        p.undo(Mode::PickUp);
        assert!(p.takes(Device::Keyboard));
    }

    #[test]
    fn the_computer_sides_highlight_moves_through_its_picks_a_second_at_a_time() {
        let t = tiles(
            3,
            &[15, 16, 17, 18, 19],
            [15, 16, 17],
            false,
            Mode::League,
            &|_| false,
        );
        let p = Picker::computer(t, [15, 16, 17]);
        let at = |ticks| p.cycled(ticks).shown().and_then(|t| t.record());
        assert_eq!(at(0), Some(17));
        assert_eq!(at(59), Some(17));
        assert_eq!(at(60), Some(15));
        assert_eq!(at(120), Some(16));
        assert_eq!(at(180), Some(17));
        assert_eq!(at(240), Some(15));
    }

    #[test]
    fn the_computer_side_shows_its_three_picked() {
        let roster = [10, 11, 12, 13, 14];
        let t = tiles(5, &roster, [12, 10, 14], false, Mode::PickUp, &|_| false);
        assert_eq!(
            t,
            [Tile::Player(10), Tile::Player(12), Tile::Player(14)],
            "a side nobody holds lists only its line-up"
        );
        let p = Picker::computer(t, [12, 10, 14]);
        assert!(p.done);
        assert_eq!(p.picks, [1, 0, 2]);
        assert_eq!(p.cursor, 2, "the focus on the last pick");
        assert_eq!(p.lineup(), Some([12, 10, 14]));
    }

    #[test]
    fn be_a_legend_puts_home_on_the_left() {
        let mut s = Screen::default();
        assert_eq!((s.panel(0), s.panel(1)), (1, 0));
        s.home_left = true;
        assert_eq!((s.panel(0), s.panel(1)), (0, 1));
        let away = tiles(36, &[], [190, 191, 192], false, Mode::Legend, &|_| false);
        assert_eq!(away.len(), 3, "a generic crew outside its team's roster");
    }

    #[test]
    fn the_legend_teams_list_their_own() {
        let held = |team| tiles(team, &[], [0, 1, 2], true, Mode::PickUp, &|r| r == SECRET);
        assert_eq!(held(39).len(), 25);
        assert_eq!(held(39)[0], Tile::Player(100));
        let old = held(38);
        assert_eq!(old.len(), 26);
        assert_eq!(old[0], Tile::Player(0x9e));
        assert_eq!(
            *old.last().unwrap(),
            Tile::Locked(SECRET),
            "the secret one last"
        );
        let computer = tiles(38, &[], [0x9e, 0xa0, 0xa1], false, Mode::PickUp, &|_| false);
        assert_eq!(computer.len(), 3);
        let crew = tiles(50, &[1, 2, 3], [1, 2, 3], true, Mode::Legend, &|_| false);
        assert_eq!(crew.len(), 5, "the crew pads to five");
        assert_eq!(crew[4], Tile::Empty);
    }

    #[test]
    fn the_backs_palettes_follow_the_portrait() {
        assert_eq!(back_palette(5), 0);
        assert_eq!(back_palette(-1), 8);
        assert_eq!(back_palette(0x91), 2);
        assert_eq!(back_palette(0xab), 9);
        assert_eq!(back_palette(0xc0), 10);
        assert_eq!(back_palette(0xb2), 0);
    }
}
