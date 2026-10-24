//! Edit Moves: the editor's screen (`FUN_00290b40` = `FUN_0028b768(screen,
//! 5, 12)`: class 0x4f, layout 79, over backdrop 12) holding one panel,
//! `FUN_00290d80` over layout 85 (class 0x55, on the editor's panel base
//! `FUN_0028c798`). The screen's enter (`FUN_00290bb8`) names it "Edit
//! Moves" (`editplyr_moves.txt` 0, 0x3600), moves its L1 and R1 icons
//! (elements 17 and 18) to (104, 109) and (279, 109) by the category header
//! and deletes its arrows (149, 150).
//!
//! One panel, two steps (the panel's +0x1c0):
//!
//! - *Step 1* (`FUN_002921f8`): the header list 142 (node 143, wrapping:
//!   its +0x11c is set) walks the seven categories with L1 and R1 (actions
//!   0x31, 0x33; `FUN_00292ce0`); list 20 holds the category's slots, two
//!   rows each (`+0xfa` = 2): the move's "%d.  %s" (template 152) and its
//!   button combination (`FUN_00212880`, list items 1, 3, 5), three a page;
//!   the list dims the slots it does not focus (`+0x124`, 0x9f) and the
//!   panel's three boxes (281 by 72 at (51, 149), (51, 227), (51, 303),
//!   0x7f000000) darken the backdrop under them (`FUN_002916c0` hides the
//!   one over the focused slot). Cross picks the slot (`FUN_00292788`), Start
//!   leaves ("Exit": the screen's legend's first pair is set to Start and
//!   raised 3 pixels).
//! - *Step 2* (`FUN_00291c70`): list 440 holds the category's moves that are
//!   in no slot, the ones that can be had first, then the locked ones
//!   (template 260, seven rows over the bars 101..107, `XACOMMON` image 6
//!   frame 9); "Item: %1d DP" (19, the move's price) on the gold box 463
//!   and "Total: %1d DP" (3, the baller's points) on 464. Cross takes the
//!   move: a free or owned one goes in the slot at once; a locked one says
//!   "This move is currently locked." (`editplyr.txt` 22); one the points do
//!   not cover says "You do not have enough development points to unlock
//!   %2S." (6); else "Use %1d development points to unlock %2S?" (4, Yes
//!   and No, No lit; `FUN_00292bb8` buys on Yes). Triangle goes back to
//!   step 1 ("Cancel", the legend's first pair).
//!
//! Both steps: the film room on the right shows the backdrop's still until
//! Circle ("Preview Move", the panel's legend 15; `FUN_00292ce0`'s 0x2a)
//! plays the focused move's clip (`movies/filmroom/`, a movie widget at
//! (350, 110)); element 147 names the move, or "Locked" (`editplyr_moves.txt`
//! 23); the help line (element 1) is "Step 1:  Pick a %1s to replace." or
//! "Step 2:  Pick a new %1s." with the category's word.

use bevy::prelude::*;
use street_assets::font::Layout;
use street_sim::story::{CreatedBaller, EMPTY_MOVE_SLOT, Story};

use crate::{
    cab_view::{Pic, RowWords, Values},
    look::{self, Look, Piece, colours},
    setup::Disc,
};

/// The panel's layout (class 0x55) and the editor screen's backdrop.
pub const LAYOUT: u32 = 85;
pub const BACKDROP: usize = 12;

/// A category (`0x3f9bc8`, seven of eleven words): the slot table (0
/// dunks, 1 tricks, 2 shots), the move kind (`XDBMOVE` field 2), the level
/// (field 3; `None` for every level), the first `XDBMOVE` row and how many,
/// the name's and the steps' word's text ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Category {
    pub table: usize,
    pub kind: usize,
    pub level: Option<i16>,
    pub first: u8,
    pub count: u8,
    pub name: u16,
    pub word: u16,
}

pub const CATEGORIES: [Category; 7] = [
    cat(0, 1, Some(2), 10, 16, 0x3604, 0x360c),
    cat(0, 1, Some(3), 26, 23, 0x3605, 0x360c),
    cat(0, 1, Some(4), 49, 5, 0x3606, 0x360c),
    cat(1, 2, Some(2), 54, 14, 0x3607, 0x360d),
    cat(1, 2, Some(3), 68, 16, 0x3608, 0x360d),
    cat(1, 2, Some(4), 89, 4, 0x3609, 0x360d),
    cat(2, 0, None, 0, 10, 0x360a, 0x360b),
];

const fn cat(
    table: usize,
    kind: usize,
    level: Option<i16>,
    first: u8,
    count: u8,
    name: u16,
    word: u16,
) -> Category {
    Category {
        table,
        kind,
        level,
        first,
        count,
        name,
        word,
    }
}

/// The slot tables' turbo buttons by native slot (`0x43d3b0` dunks and
/// `0x43d408` tricks, eleven; `0x43d460` shots, six; each with the slot's
/// level): bit 14 L1, 15 L2, 16 R1, 17 R2.
const TURBOS: [u32; 11] = [
    0x3c000, 0x1c000, 0x2c000, 0x34000, 0x38000, 0xc000, 0x14000, 0x24000, 0x18000, 0x28000,
    0x30000,
];
const SHOT_TURBOS: [u32; 6] = [0xc000, 0x14000, 0x24000, 0x18000, 0x28000, 0x30000];

/// The button-combination widget's bits and icons (`FUN_00212880`: twelve
/// bits at `0x466878`, their `XACOMMON` sheet 1 image 0 frames at
/// `0x4668a8`), in the order it lays them out.
const COMBO: [(u32, usize); 12] = [
    (0x800, 0xe),
    (0x4000, 5),
    (0x8000, 8),
    (0x10000, 6),
    (0x20000, 7),
    (0x4, 0x10),
    (0x8, 0x11),
    (0x10, 1),
    (0x80, 0),
    (0x100, 3),
    (0x200, 1),
    (0x1000, 2),
];

/// The moves a created player has without buying them (`FUN_001d9e60`:
/// `0x45fde0` shots, `0x45fdf0` dunks, `0x45fe08` tricks): the "Free" ones
/// (`FUN_001e55f0`, the panel's +0x184).
pub const FREE: [&[u8]; 3] = [
    &[0, 1, 2, 3, 4, 5],
    &[49, 26, 27, 48, 28, 10, 11, 12, 13, 14, 15],
    &[89, 68, 69, 70, 71, 54, 55, 56, 57, 58, 59],
];

/// The moves a reward card unlocks (`FUN_001e57a8` keeps them from sale:
/// the Street Milestones table's moves at `0x460790 + 0x18` and rows 0x28,
/// 0x29, 0x2c, 0x2f; `FUN_001d6b50` names each one's card, owned when
/// `FUN_001d26e8` says: the card costs nothing or was bought).
pub const CARD_MOVES: [(u8, u16); 16] = [
    (8, 5),
    (9, 0x44),
    (0x27, 8),
    (0x28, 0x3c),
    (0x29, 0x3a),
    (0x2a, 0x41),
    (0x2b, 0x46),
    (0x2c, 0x22),
    (0x2d, 3),
    (0x2e, 0x43),
    (0x2f, 0x3e),
    (0x4e, 4),
    (0x4f, 6),
    (0x50, 0x45),
    (0x51, 7),
    (0x52, 0x42),
];

impl Category {
    pub fn at(index: usize) -> Category {
        CATEGORIES[index.min(CATEGORIES.len() - 1)]
    }

    /// The category's native slots in its kind's list (the slots whose
    /// level is the category's; every shot slot).
    pub fn slots(self) -> std::ops::Range<usize> {
        match self.level {
            None => 0..6,
            Some(4) => 0..1,
            Some(3) => 1..5,
            _ => 5..11,
        }
    }

    /// The category's `XDBMOVE` rows (its words 3 and 6).
    pub fn rows(self) -> std::ops::Range<u8> {
        self.first..self.first + self.count
    }

    /// The `k`th slot's buttons: its turbos and the move's button (Square
    /// for the tricks' table, `| 0x10`; Circle for the others, `| 0x80`).
    pub fn buttons(self, k: usize) -> u32 {
        let slot = self.slots().nth(k).unwrap_or(0);
        let turbos = if self.table == 2 {
            SHOT_TURBOS.get(slot)
        } else {
            TURBOS.get(slot)
        };
        turbos.copied().unwrap_or(0) | if self.table == 1 { 0x10 } else { 0x80 }
    }
}

/// The icons (`XACOMMON` sheet 1 image 0 frames) a button combination shows,
/// in order.
pub fn combo_frames(buttons: u32) -> Vec<usize> {
    COMBO
        .iter()
        .filter(|(bit, _)| buttons & bit != 0)
        .map(|&(_, frame)| frame)
        .collect()
}

/// What a move is to the baller, as the panel asks (`+0x184`, `+0x18c`,
/// `FUN_00293400`, `+0x17c`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// One of the kind's own moves, or one bought, or its card's.
    Owned,
    /// For sale at its price (`XDBMOVE` field 4).
    Sale(u32),
    /// Its card is not unlocked.
    Locked,
}

fn card_of(row: u8) -> Option<u16> {
    CARD_MOVES
        .iter()
        .find(|&&(r, _)| r == row)
        .map(|&(_, card)| card)
}

/// Whether reward card `id` is the player's (`FUN_001d26e8`: it costs
/// nothing, or it was bought).
fn card_owned(disc: &Disc, story: &Story, id: u16) -> bool {
    story.owned.contains(&id)
        || disc
            .cards
            .iter()
            .find(|c| c.id == id as i16)
            .is_some_and(|c| c.cost < 1)
}

pub fn standing(disc: &Disc, story: &Story, b: &CreatedBaller, row: u8) -> Standing {
    let Some(m) = disc.moves.get(usize::from(row)) else {
        return Standing::Locked;
    };
    if let Some(card) = card_of(row) {
        return if card_owned(disc, story, card) {
            Standing::Owned
        } else {
            Standing::Locked
        };
    }
    let kind = (m.kind.max(0) as usize).min(2);
    if FREE[kind].contains(&row) || b.owns(row) {
        Standing::Owned
    } else {
        Standing::Sale(m.cost.max(0) as u32)
    }
}

/// The price "Item" shows (`+0x17c`: nothing for an owned or locked move).
pub fn price(s: Standing) -> u32 {
    match s {
        Standing::Sale(n) => n,
        _ => 0,
    }
}

/// The moves in the category's slots, by slot (`FUN_001d8778`; an empty
/// slot is [`EMPTY_MOVE_SLOT`]).
pub fn slot_moves(disc: &Disc, b: &CreatedBaller, cat: Category) -> Vec<u8> {
    let slots = crate::clip::native_move_slots(disc, cat.kind, &b.moves[cat.kind]);
    cat.slots()
        .map(|s| slots.get(s).copied().unwrap_or(EMPTY_MOVE_SLOT))
        .collect()
}

/// Step 2's list (`FUN_00291c70`): the category's moves in no slot, the
/// ones to be had (`FUN_00293400`) first, then the locked, each in row
/// order.
pub fn pick_list(disc: &Disc, story: &Story, b: &CreatedBaller, cat: Category) -> Vec<u8> {
    let slots = crate::clip::native_move_slots(disc, cat.kind, &b.moves[cat.kind]);
    let (open, locked): (Vec<u8>, Vec<u8>) = cat
        .rows()
        .filter(|row| !slots.contains(row))
        .partition(|&row| standing(disc, story, b, row) != Standing::Locked);
    open.into_iter().chain(locked).collect()
}

/// A question step 2 asks (`FUN_00292788`'s dialogs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// "Use %1d development points to unlock %2S?" (`editplyr.txt` 4;
    /// Yes, No).
    Buy { row: u8, cost: u32 },
    /// "You do not have enough development points to unlock %2S." (6).
    Poor { row: u8 },
    /// "This move is currently locked." (22).
    Locked,
}

impl Ask {
    /// The dialog's words and the width it wraps them to (`FUN_0025d6d8`
    /// and `FUN_0025d660` 350, `FUN_0025d5c8` 250).
    pub fn words(self, look: &Look, disc: &Disc) -> (String, f32) {
        let name = |row: u8| {
            disc.moves
                .get(usize::from(row))
                .map_or(String::new(), |m| m.name.clone())
        };
        let text = |k: usize, or: &str| look.text("editplyr.txt", k).unwrap_or(or).to_owned();
        match self {
            Ask::Buy { row, cost } => (
                text(4, "Use %1d development points to unlock %2S?")
                    .replace("%1d", &crate::users::count(cost))
                    .replace("%2S", &name(row)),
                350.0,
            ),
            Ask::Poor { row } => (
                text(
                    6,
                    "You do not have enough development points to unlock %2S.",
                )
                .replace("%2S", &name(row)),
                350.0,
            ),
            Ask::Locked => (text(22, "This move is currently locked."), 250.0),
        }
    }

    /// The rows: Yes and No (`common.txt` 2, 3), or Continue (20).
    pub fn rows(self, look: &Look) -> Vec<String> {
        let text = |k: usize, or: &str| look.text("common.txt", k).unwrap_or(or).to_owned();
        match self {
            Ask::Buy { .. } => vec![text(2, "Yes"), text(3, "No")],
            _ => vec![text(20, "Continue")],
        }
    }

    /// The row lit when it opens (`FUN_0025cda0(dialog, 1)`: No, or the
    /// one row).
    pub fn opening_row(self) -> usize {
        match self {
            Ask::Buy { .. } => 1,
            _ => 0,
        }
    }
}

/// Step 2's Cross on `row` for the category's `k`th slot: an owned move
/// goes in at once, else the question to ask.
pub fn select(
    disc: &Disc,
    story: &Story,
    e: &mut crate::ballers::Editing,
    cat: Category,
    k: usize,
    row: u8,
) -> Result<(), Ask> {
    let Some(b) = e.baller.as_ref() else {
        return Err(Ask::Locked);
    };
    match standing(disc, story, b, row) {
        Standing::Owned => {
            assign(disc, e, cat, k, row);
            Ok(())
        }
        Standing::Locked => Err(Ask::Locked),
        Standing::Sale(cost) => {
            if crate::ballers::points_left(e, story) < i64::from(cost) {
                Err(Ask::Poor { row })
            } else {
                Err(Ask::Buy { row, cost })
            }
        }
    }
}

/// Yes to the price (`FUN_00292bb8`: `FUN_002934d8` buys the move, then it
/// goes in the slot). The rewrite takes the points when the baller is
/// saved, as the rest of the editor's purchases.
pub fn buy(
    disc: &Disc,
    story: &Story,
    e: &mut crate::ballers::Editing,
    cat: Category,
    k: usize,
    row: u8,
) -> bool {
    let Some(b) = e.baller.as_ref() else {
        return false;
    };
    let Standing::Sale(cost) = standing(disc, story, b, row) else {
        return false;
    };
    if crate::ballers::points_left(e, story) < i64::from(cost) {
        return false;
    }
    if assign(disc, e, cat, k, row) {
        e.move_spent += i64::from(cost);
        true
    } else {
        false
    }
}

/// Puts `row` in the category's `k`th slot (`FUN_002934b0`).
fn assign(disc: &Disc, e: &mut crate::ballers::Editing, cat: Category, k: usize, row: u8) -> bool {
    let Some(b) = e.baller.as_mut() else {
        return false;
    };
    b.moves[cat.kind] = crate::clip::native_move_slots(disc, cat.kind, &b.moves[cat.kind]);
    let Some(slot) = cat.slots().nth(k) else {
        return false;
    };
    if b.moves[cat.kind].contains(&row) {
        return false;
    }
    if let Some(old) = b.moves[cat.kind].get(slot).copied()
        && old != EMPTY_MOVE_SLOT
        && !b.unlocked.contains(&old)
    {
        // A move taken out of its slot stays the baller's.
        b.unlocked.push(old);
    }
    b.assign_in_slot(cat.kind, slot, row)
}

/// The panel's state besides the page: the lists' first rows shown, the
/// question open, the clip asked for, and whether Edit Moves was opened
/// alone (from a baller's dialog, `FUN_00290b40`'s screen; it saves and
/// goes back there on Exit) or from Edit Baller (Circle).
#[derive(Clone, Debug, Default)]
pub struct Panel {
    pub slots_top: usize,
    pub pick_top: usize,
    pub ask: Option<Ask>,
    pub preview: Option<u8>,
    pub alone: bool,
}

/// The slots a page shows (rows 21, 23, 25) and the moves step 2 shows
/// (rows 441..453).
pub const SLOTS_SHOWN: usize = 3;
pub const PICKS_SHOWN: usize = 7;

/// The step-1 boxes over the slots the list does not focus (`0x4d3518`:
/// 281 by 72, `DAT_003f8160` 0x7f000000).
const DIM_BOXES: [Vec2; 3] = [
    Vec2::new(51.0, 149.0),
    Vec2::new(51.0, 227.0),
    Vec2::new(51.0, 303.0),
];

/// The film room's movie widget (`FUN_00292ce0`: (350, 110); the clips are
/// 224 by 144).
pub const FILM_ROOM: Rect = Rect {
    min: Vec2::new(350.0, 110.0),
    max: Vec2::new(574.0, 254.0),
};

/// What one step shows.
pub struct Shown<'a> {
    pub disc: &'a Disc,
    pub story: &'a Story,
    pub baller: &'a CreatedBaller,
    pub cat: Category,
    /// Step 1's focused slot, or the slot step 2 fills.
    pub slot: usize,
    /// Step 2's focused move (`None` in step 1).
    pub pick: Option<usize>,
    pub panel: &'a Panel,
    /// The baller's points ("Total").
    pub total: i64,
    /// Inside Vol. 2's The Moves (`FUN_00293bc0`): every move of the
    /// category, browsed (the panel's +0x1c0 set from the start).
    pub browse: bool,
}

impl Shown<'_> {
    /// The move the film room and element 147 are on (`FUN_00293558`).
    pub fn focused(&self) -> Option<u8> {
        match self.pick {
            None => slot_moves(self.disc, self.baller, self.cat)
                .get(self.slot)
                .copied()
                .filter(|&r| r != EMPTY_MOVE_SLOT),
            Some(at) => self.list().get(at).copied(),
        }
    }

    /// Step 2's list, or The Moves'.
    pub fn list(&self) -> Vec<u8> {
        if self.browse {
            browse_list(self.cat)
        } else {
            pick_list(self.disc, self.story, self.baller, self.cat)
        }
    }

    /// What the focused move is to the player: The Moves has no baller,
    /// only the cards (`FUN_00295688`).
    fn standing_of(&self, row: u8) -> Standing {
        if self.browse {
            if browse_locked(self.disc, self.story, row) {
                Standing::Locked
            } else {
                Standing::Owned
            }
        } else {
            standing(self.disc, self.story, self.baller, row)
        }
    }
}

/// The Moves' categories' names (`0x3f9de0`'s third words, `editplyr_moves.txt`
/// 16..22: "Dunks - Level 3" .. "Shots"), in the Edit Moves categories'
/// order.
pub const BROWSE_NAMES: [u16; 7] = [0x3610, 0x3611, 0x3612, 0x3613, 0x3614, 0x3615, 0x3616];

/// The Moves' list (`FUN_00294658`): the category's moves in row order, the
/// cards' last (`FUN_00295688`, `FUN_001e57a8`).
pub fn browse_list(cat: Category) -> Vec<u8> {
    let (open, cards): (Vec<u8>, Vec<u8>) = cat.rows().partition(|&row| card_of(row).is_none());
    open.into_iter().chain(cards).collect()
}

/// Whether The Moves keeps `row` dark: its card is not the player's
/// (`FUN_00295688`); a locked move does not preview.
pub fn browse_locked(disc: &Disc, story: &Story, row: u8) -> bool {
    card_of(row).is_some_and(|card| !card_owned(disc, story, card))
}

/// The Moves' cover over Item and Total (`FUN_00294000`: a box of
/// 0xff010955, 279 by 27 at (53, 368)).
pub const BROWSE_COVER: Rect = Rect {
    min: Vec2::new(53.0, 368.0),
    max: Vec2::new(332.0, 395.0),
};

/// The text of a layout's text id (past the first 30 files).
fn text_id(look: &Look, id: u16, or: &str) -> String {
    look.text_full(id).unwrap_or(or).trim().to_owned()
}

/// The panel's values for `panel_pieces` (layout 85).
pub fn values(look: &Look, s: &Shown) -> Values {
    let mut v = Values::default();
    let cat = s.cat;
    // Element 144 has no picture on the editor's base (`FUN_0028c850`);
    // the header's arrows 145 and 146 and the list arrows are the lists'.
    v.pics.push((144, Pic::None));
    v.pics.push((145, Pic::Arrow(2)));
    v.pics.push((146, Pic::Arrow(3)));
    v.pics.push((460, Pic::Arrow(0)));
    v.pics.push((461, Pic::Arrow(1)));
    // The bars 101.. take image 6 frame 9 (`FUN_00290ed0`), the stars
    // 110..112 image 1 frame 1 (`FUN_0028c850`).
    for id in 101..=107 {
        v.pics.push((id, Pic::Common(6, 9)));
    }
    for id in 110..=112 {
        v.pics.push((id, Pic::Common(1, 1)));
    }
    let word = text_id(look, cat.word, "move");
    let step2 = s.pick.is_some();
    let help = if s.browse {
        // 0x360f.
        look.text("editplyr_moves.txt", 15)
            .unwrap_or("Preview the highlighted %1s.")
    } else if step2 {
        look.text("editplyr_moves.txt", 3)
            .unwrap_or("Step 2:  Pick a new %1s.")
    } else {
        look.text("editplyr_moves.txt", 2)
            .unwrap_or("Step 1:  Pick a %1s to replace.")
    };
    v.help = Some(help.replace("%1s", &word));
    if let Some(row) = s.focused() {
        let shown = match s.standing_of(row) {
            Standing::Locked => text_id(look, 0x3617, "Locked"),
            _ => s
                .disc
                .moves
                .get(usize::from(row))
                .map_or(String::new(), |m| m.name.clone()),
        };
        v.words.push((147, shown));
    }
    match s.pick {
        None => {
            // Step 2's pieces stay hidden (`FUN_00291998`).
            v.hidden.extend(101..=107);
            v.hidden.extend([460, 461, 3, 19]);
            v.hidden.extend(462..=466);
            let moves = slot_moves(s.disc, s.baller, cat);
            let top =
                crate::cab_view::scrolled(s.panel.slots_top, s.slot, moves.len(), SLOTS_SHOWN);
            for (k, (&name_row, &combo_row)) in
                [21, 23, 25].iter().zip(&[22u32, 24, 26]).enumerate()
            {
                let at = top + k;
                let Some(&row) = moves.get(at) else {
                    break;
                };
                let name = s
                    .disc
                    .moves
                    .get(usize::from(row))
                    .map_or("", |m| m.name.as_str());
                let lit = at == s.slot;
                v.rows.push(RowWords {
                    row: name_row,
                    template: 152,
                    words: format!("{}.  {name}", at + 1),
                    lit,
                });
                if !lit {
                    v.dim_rows.extend([name_row, combo_row]);
                }
            }
            if top == 0 {
                v.hidden.push(4);
            }
            if top + SLOTS_SHOWN >= moves.len() {
                v.hidden.push(5);
            }
        }
        Some(at) if s.browse => {
            // The header keeps its arrows; Item and Total lie under the
            // cover (`extra`).
            v.hidden.extend([4, 5, 3, 19]);
            let list = s.list();
            let top = crate::cab_view::scrolled(s.panel.pick_top, at, list.len(), PICKS_SHOWN);
            for (k, row_id) in (441..=453).step_by(2).enumerate() {
                let Some(&row) = list.get(top + k) else {
                    continue;
                };
                v.rows.push(RowWords {
                    row: row_id,
                    template: 260,
                    words: s
                        .disc
                        .moves
                        .get(usize::from(row))
                        .map_or(String::new(), |m| m.name.clone()),
                    lit: top + k == at,
                });
            }
            if top == 0 {
                v.hidden.push(460);
            }
            if top + PICKS_SHOWN >= list.len() {
                v.hidden.push(461);
            }
        }
        Some(at) => {
            v.hidden.extend([4, 5, 145, 146]);
            let list = pick_list(s.disc, s.story, s.baller, cat);
            let top = crate::cab_view::scrolled(s.panel.pick_top, at, list.len(), PICKS_SHOWN);
            for (k, row_id) in (441..=453).step_by(2).enumerate() {
                let Some(&row) = list.get(top + k) else {
                    // The bars past the list's end stay (the layout's).
                    continue;
                };
                v.rows.push(RowWords {
                    row: row_id,
                    template: 260,
                    words: s
                        .disc
                        .moves
                        .get(usize::from(row))
                        .map_or(String::new(), |m| m.name.clone()),
                    lit: top + k == at,
                });
            }
            if top == 0 {
                v.hidden.push(460);
            }
            if top + PICKS_SHOWN >= list.len() {
                v.hidden.push(461);
            }
            let item = list
                .get(at)
                .map_or(0, |&row| price(standing(s.disc, s.story, s.baller, row)));
            v.words.push((
                19,
                look.text("editplyr.txt", 20)
                    .unwrap_or("Item: %1d DP")
                    .replace("%1d", &crate::users::count(item)),
            ));
            let total = s.total.max(0).min(i64::from(u32::MAX)) as u32;
            v.words.push((
                3,
                look.text("editplyr.txt", 21)
                    .unwrap_or("Total: %1d DP")
                    .replace("%1d", &crate::users::count(total)),
            ));
        }
    }
    v
}

/// What the panel draws besides its layout's pieces: the dimming boxes
/// (under the rows), and over them the category's name on the header's
/// node 143 (template 360: gold in step 1, cream in step 2), the screen's
/// L1 and R1 icons by it (step 1), and the slots' button combinations.
pub struct Extra {
    pub under: Vec<Piece>,
    pub over: Vec<Piece>,
}

pub fn extra(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    white: &Handle<Image>,
    layout: &Layout,
    s: &Shown,
) -> Extra {
    let mut under = Vec::new();
    let mut over = Vec::new();
    let element = |kind: u8, id: u32| {
        layout
            .elements
            .iter()
            .find(|e| e.kind == kind && e.id == id)
    };
    let step1 = s.pick.is_none();
    if step1 {
        let moves = slot_moves(s.disc, s.baller, s.cat);
        let top = crate::cab_view::scrolled(s.panel.slots_top, s.slot, moves.len(), SLOTS_SHOWN);
        for (k, at) in DIM_BOXES.iter().enumerate() {
            if top + k == s.slot {
                continue;
            }
            let mut p = Piece::new(
                white.clone(),
                Rect::from_corners(*at, *at + Vec2::new(281.0, 72.0)),
            );
            p.colour = Color::srgba(0.0, 0.0, 0.0, look::black_alpha(127.0 / 255.0));
            under.push(p);
        }
        // The combinations: anchored by their right and middle on the
        // rows 22, 24, 26 (as `my_team_edit_moves.png` places them).
        for (k, combo_row) in [22u32, 24, 26].into_iter().enumerate() {
            let at_slot = top + k;
            if at_slot >= moves.len() {
                break;
            }
            let Some(at) = element(10, combo_row).and_then(crate::choose::spot) else {
                continue;
            };
            let bright = if at_slot == s.slot { 1.0 } else { DIM };
            combo(
                &mut over,
                look,
                labels,
                images,
                s.cat.buttons(at_slot),
                at,
                bright,
            );
        }
        // The screen's L1 and R1 icons, moved by the header (`FUN_00290bb8`).
        if let Some((atlas, frames)) = look.icons.as_ref() {
            for (frame, x) in [(5usize, 104.0), (6, 279.0)] {
                if let Some(&f) = frames.get(frame) {
                    over.push(Piece::part(
                        atlas.clone(),
                        f,
                        Rect::from_center_size(Vec2::new(x, 109.0), f.size()),
                    ));
                }
            }
        }
    }
    // The category's name on node 143 in template 360's font and anchor
    // (gold while the header is the list that steps).
    if let (Some(node), Some(template)) = (element(10, 143), element(4, 360))
        && let Some(at) = crate::choose::spot(node)
    {
        let font = template.props.get(&13).map_or(2, |&f| f as usize);
        let colour = if step1 || s.browse {
            colours::HELP
        } else {
            colours::CREAM
        };
        let name = if s.browse {
            let k = CATEGORIES.iter().position(|&c| c == s.cat).unwrap_or(0);
            text_id(look, BROWSE_NAMES[k], "Moves")
        } else {
            text_id(look, s.cat.name, "Moves")
        };
        crate::choose::place(
            &mut over,
            labels,
            look,
            images,
            font,
            &name,
            colour,
            at,
            template.align,
        );
    }
    if s.browse {
        let mut cover = Piece::new(white.clone(), BROWSE_COVER);
        cover.colour = Color::srgb_u8(0x01, 0x09, 0x55);
        over.push(cover);
    }
    Extra { under, over }
}

/// The brightness the dimmed list gives the rows it does not focus (0x9f).
pub const DIM: f32 = 0x9f as f32 / 255.0;

/// A button combination (`FUN_00212880`): each button's icon, a "+" (font
/// 1, 0xffdcdcbb) 8 pixels past the one before and the next icon 5 pixels
/// before the "+" ends; laid out from 0, then anchored by its right and
/// middle at `at`.
pub(crate) fn combo(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    buttons: u32,
    at: Vec2,
    bright: f32,
) {
    let Some((atlas, frames)) = look.icons.as_ref() else {
        return;
    };
    let Some(font) = look.fonts.get(1) else {
        return;
    };
    let plus_w = font.measure("+") as f32;
    let mut x = 0.0;
    let mut parts: Vec<(f32, Option<Rect>)> = Vec::new();
    for (k, frame) in combo_frames(buttons).into_iter().enumerate() {
        let Some(&f) = frames.get(frame) else {
            continue;
        };
        if k > 0 {
            x += PLUS_GAP;
            parts.push((x, None));
            x += plus_w - PLUS_BACK;
        }
        parts.push((x, Some(f)));
        x += f.width();
    }
    let left = at.x - x;
    let dim = Color::srgb(bright, bright, bright);
    for (dx, part) in parts {
        match part {
            Some(f) => {
                let mut p = Piece::part(
                    atlas.clone(),
                    f,
                    Rect::from_corners(
                        Vec2::new(left + dx, at.y - f.height() / 2.0),
                        Vec2::new(left + dx + f.width(), at.y + f.height() / 2.0),
                    ),
                );
                p.colour = dim;
                out.push(p);
            }
            None => {
                let c = colours::CREAM;
                let c = [
                    (f32::from(c[0]) * bright) as u8,
                    (f32::from(c[1]) * bright) as u8,
                    (f32::from(c[2]) * bright) as u8,
                    c[3],
                ];
                let top = at.y - font.text_height as f32 / 2.0;
                if let Some(p) = labels.text(
                    look,
                    images,
                    1,
                    "+",
                    c,
                    None,
                    Vec2::new(left + dx - PLUS_INK, top),
                    look::Align::Left,
                ) {
                    out.push(p);
                }
            }
        }
    }
}

/// A button combination's width as [`combo`] lays it out.
pub(crate) fn combo_width(look: &Look, buttons: u32) -> f32 {
    let (Some((_, frames)), Some(font)) = (look.icons.as_ref(), look.fonts.get(1)) else {
        return 0.0;
    };
    let plus_w = font.measure("+") as f32;
    let mut x = 0.0;
    for (k, frame) in combo_frames(buttons).into_iter().enumerate() {
        let Some(f) = frames.get(frame) else {
            continue;
        };
        if k > 0 {
            x += PLUS_GAP + plus_w - PLUS_BACK;
        }
        x += f.width();
    }
    x
}

/// `FUN_00212880`'s spacing: 8 pixels before a "+", the next icon 5
/// pixels back from its end.
const PLUS_GAP: f32 = 8.0;
const PLUS_BACK: f32 = 5.0;
/// The "+" sits at its box's left in the original's text widget (its ink
/// 9 or 10 pixels past the icon before, `my_team_edit_moves.png`) where the
/// font's glyph centres it in its advance: measured, 6 pixels.
const PLUS_INK: f32 = 6.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_categories_follow_the_table() {
        // Dunk Lvl 3: six slots of two turbos and Circle; Lvl 5 one of four.
        let c = Category::at(0);
        assert_eq!(c.slots().len(), 6);
        assert_eq!(combo_frames(c.buttons(0)), vec![5, 8, 0]); // L1 + L2 + O
        assert_eq!(combo_frames(c.buttons(1)), vec![5, 6, 0]); // L1 + R1 + O
        assert_eq!(combo_frames(c.buttons(2)), vec![5, 7, 0]); // L1 + R2 + O
        assert_eq!(
            combo_frames(Category::at(2).buttons(0)),
            vec![5, 8, 6, 7, 0]
        );
        assert_eq!(Category::at(1).slots().len(), 4);
        // The tricks' table takes Square, the shots' Circle.
        assert_eq!(combo_frames(Category::at(3).buttons(0)), vec![5, 8, 1]);
        assert_eq!(combo_frames(Category::at(6).buttons(5)), vec![6, 7, 0]);
        // Each category's rows follow the one before inside its kind.
        assert_eq!(Category::at(0).rows(), 10..26);
        assert_eq!(Category::at(1).rows(), 26..49);
        assert_eq!(Category::at(4).rows(), 68..84);
        assert_eq!(Category::at(6).rows(), 0..10);
    }

    #[test]
    fn the_moves_lists_every_move_the_cards_last() {
        // Dunk Lvl 3: rows 10..26 in order, the card's 0x27 none of them;
        // Dunk Lvl 4 puts its card moves (0x27..0x2f) after the rest.
        assert_eq!(browse_list(Category::at(0)), (10..26).collect::<Vec<u8>>());
        let lvl4 = browse_list(Category::at(1));
        assert_eq!(lvl4.len(), 23);
        let first_card = lvl4.iter().position(|&r| card_of(r).is_some()).unwrap();
        assert!(lvl4[first_card..].iter().all(|&r| card_of(r).is_some()));
        assert_eq!(BROWSE_NAMES.len(), CATEGORIES.len());
    }

    #[test]
    fn the_questions_open_on_their_traced_rows() {
        assert_eq!(Ask::Buy { row: 16, cost: 50 }.opening_row(), 1);
        assert_eq!(Ask::Locked.opening_row(), 0);
        assert_eq!(Ask::Poor { row: 16 }.opening_row(), 0);
    }

    fn disc() -> Option<Disc> {
        let path = std::env::var("STREET_ISO").ok()?;
        Disc::load(street_assets::iso::Iso::open(path).ok()?).ok()
    }

    #[test]
    fn the_category_rows_are_the_kinds_and_levels_named() {
        let Some(disc) = disc() else {
            return;
        };
        for c in CATEGORIES {
            for row in c.rows() {
                let m: &street_assets::db::Move = &disc.moves[usize::from(row)];
                assert_eq!(m.kind as usize, c.kind, "{} row {row}", m.name);
                if let Some(level) = c.level {
                    assert_eq!(m.level, level, "{} row {row}", m.name);
                }
            }
        }
        // The default moves are the kinds' own.
        for (kind, rows) in FREE.iter().enumerate() {
            for &row in *rows {
                assert_eq!(disc.moves[usize::from(row)].kind as usize, kind);
            }
        }
    }

    #[test]
    fn step_two_lists_the_moves_out_of_slots_locked_last() {
        let Some(disc) = disc() else {
            return;
        };
        let story = Story::default();
        let b = CreatedBaller {
            moves: FREE.map(<[u8]>::to_vec),
            ..Default::default()
        };
        let cat = Category::at(0);
        let list = pick_list(&disc, &story, &b, cat);
        // Sixteen moves less the six in slots.
        assert_eq!(list.len(), 10);
        assert!(list.iter().all(|r| !FREE[1].contains(r)));
        let locked: Vec<bool> = list
            .iter()
            .map(|&r| standing(&disc, &story, &b, r) == Standing::Locked)
            .collect();
        // The locked ones close the list.
        let first_locked = locked.iter().position(|&l| l).unwrap_or(locked.len());
        assert!(locked[first_locked..].iter().all(|&l| l));
        // A card's move is the player's once the card is.
        let mut owner = story.clone();
        owner.owned.push(5);
        assert_eq!(standing(&disc, &story, &b, 8), Standing::Locked);
        assert_eq!(standing(&disc, &owner, &b, 8), Standing::Owned);
        assert_eq!(standing(&disc, &story, &b, 16), Standing::Sale(50));
    }

    #[test]
    fn a_bought_move_takes_the_slot_and_the_old_one_stays_owned() {
        let Some(disc) = disc() else {
            return;
        };
        let mut story = Story::default();
        let mut b = CreatedBaller {
            moves: FREE.map(<[u8]>::to_vec),
            ..Default::default()
        };
        // What it wears and its build are paid for: 100 points to spend.
        b.wardrobe = b.worn_wardrobe();
        let record = b.record() as u16;
        story.credit_baller(record, 100);
        let mut e = crate::ballers::Editing {
            paid: crate::ballers::price(&b),
            baller: Some(b),
            ..Default::default()
        };
        assert_eq!(crate::ballers::points_left(&e, &story), 100);
        let cat = Category::at(0);
        let old = slot_moves(&disc, e.baller.as_ref().unwrap(), cat)[0];
        assert_eq!(
            select(&disc, &story, &mut e, cat, 0, 16),
            Err(Ask::Buy { row: 16, cost: 50 })
        );
        assert!(buy(&disc, &story, &mut e, cat, 0, 16));
        assert_eq!(e.move_spent, 50);
        let b = e.baller.as_ref().unwrap();
        assert_eq!(slot_moves(&disc, b, cat)[0], 16);
        assert_eq!(standing(&disc, &story, b, old), Standing::Owned);
        // The points left no longer cover a second.
        assert_eq!(
            select(&disc, &story, &mut e, cat, 1, 17),
            Err(Ask::Buy { row: 17, cost: 50 })
        );
        assert!(buy(&disc, &story, &mut e, cat, 1, 17));
        assert_eq!(
            select(&disc, &story, &mut e, cat, 2, 18),
            Err(Ask::Poor { row: 18 })
        );
    }
}
