//! Rewards (class 20, layout 20, backdrop 0x15; `FUN_002bfbe8`).
//!
//! The build (`FUN_002bfe10`) puts the categories' names on the strip at
//! the top (list 0x1a, its node 27 at (258, 94); `poster.txt` 10, 9, 11..16
//! in font 2, gold), the L1 and R1 icons (elements 24, 33) and the strip's
//! arrows (28, 35) beside it; two rows of up to six card sprites from the
//! templates 31 and 32 (x 271, y 180 and 292) with a "Locked" text each
//! (templates 36 and 37: font 2, grey 0x80, a black outline two pixels
//! out); the focused card's frame (`FUN_0021b2f8`, kind by category from
//! `0x48f678`); the focused locked card's price ("%1N\nReward\nPoints",
//! font 1, gold, black outline); "Total Reward Points:" (20) and the points
//! (30, nine right of it); "Reward %1d/%2d" (19); the Unlock/View Reward
//! legend (17) and Exit (18); the help line (22); the grid's arrows (58 up,
//! 21 down); and the trophy column's four slots (57, 55, 51, 53 at x 518)
//! with their stars (`XAFESCRN` sheet 0, image 8, frame 0).
//!
//! The tabs (`FUN_002c1df8`) go Street Legends, Legends, Courts,
//! Jerseys, Development Points, Trophies and round: the tips and codes
//! categories (5, 6) are stepped over. A category's cards stand in record
//! order, four a row for the two legends' categories and three for the
//! rest (`0x48f648`), two rows shown, `FUN_002c2d98` spacing their middles
//! a card's width and nine apart about x 271 (the development points'
//! "Locked" twenty lower). A card shows its picture at full light when
//! focused and owned, half when one of the two, a quarter when neither
//! (`FUN_002c2060`: `0xff >> n`). Cross on a locked card asks "Use %1N
//! Reward Points to Unlock %2s?" (`FUN_002c3780`, 265 wide) or says there
//! are not enough (`poster.txt` 8, `FUN_0025d660`); on an owned card it
//! opens the card's close-up. Start leaves for the Main Menu; Square is
//! Help.
//!
//! The Trophies tab (`+0x260` 3): the focused slot's frame (`XAFESCRN`
//! sheet 0, image 10, frame 0, stretched to `0x3fc588`'s sizes, its bottom
//! middle on `0x3fc568`'s points), the trophy's name (`poster.txt` 33..36,
//! element 54, font 3) between two stars fifteen out, three small stars
//! under it (47..49) or "Completed" (50) once earned, "Trophy %1d/%2d"; a
//! trophy not yet earned shows `XAFEBGEX` entries 8..11 side by side at
//! (61, 114) (`FUN_002c3948`), an earned one plays its movie (`XLP0012`,
//! `XLP0014`, `XLP0013`, `XLP0015` for trophies 0..3) and its slot shows
//! `XAFESCRN` sheet 0, image 11's frame for it.

use bevy::prelude::*;
use street_assets::{db::Card, font::Layout};
use street_sim::story::Story;

use crate::{
    choose::place,
    look::{self, Labels, Look, Piece},
    rewards,
};

/// The screen's layout and backdrop (`FUN_002d7720(.., 0x14, 0x15)`).
pub const CLASS: u32 = 20;
pub const BACKDROP: usize = 0x15;
/// The close-up's backdrop: the next one, 0x16 (its frame without the
/// trophy column, the rays and stars), the one its grab matches; the
/// object at `+0x218` that shows it is not traced.
pub const VIEW_BACKDROP: usize = 0x16;
/// The Trophies tab, after the five card categories.
pub const TROPHIES: usize = 5;
pub const TABS: usize = 6;

/// The tabs' names in `poster.txt` (`0x48f628`, the five categories and
/// Trophies).
const TAB_NAME: [usize; TABS] = [10, 9, 11, 12, 13, 16];
/// Cards a row (`0x48f648`).
const COLUMNS: [usize; TROPHIES] = [4, 4, 3, 3, 3];
/// The focused card's frame kind (`0x48f678`: 3 upright, 4 turned).
const WIDE: [bool; TROPHIES] = [false, false, true, true, true];
/// The help line's first string a category (`0x48f658`, `poster.txt`):
/// a locked card's "To unlock: ..." is this plus its place in the category.
const HELP_BASE: [usize; TROPHIES] = [135, 144, 124, 170, 195];
/// The trophies' movies by trophy (`0x4d4eb0`).
pub const TROPHY_MOVIES: [&str; 4] = [
    "/MOVIES/XLP0012.NFX",
    "/MOVIES/XLP0014.NFX",
    "/MOVIES/XLP0013.NFX",
    "/MOVIES/XLP0015.NFX",
];

/// The rows' middles (templates 31, 32) and the grid's middle x.
const ROW_Y: [f32; 2] = [180.0, 292.0];
const GRID_X: f32 = 271.0;
/// The gap between cards (`FUN_002c2d98`) and how much lower the
/// development points' "Locked" stands (`DAT_003fc534`, the text's only:
/// those cards carry their amount in their upper half).
const GAP: f32 = 9.0;
const DEV_DROP: f32 = 20.0;
/// The price's offset from the card's middle (`DAT_003fc528`).
const PRICE_DX: f32 = -1.0;
/// The trophy slots' middles (`DAT_003fc548`), the focused one's frame's
/// bottom middles (`DAT_003fc568`) and sizes (`DAT_003fc588`).
const SLOT: [Vec2; 4] = [
    Vec2::new(518.0, 118.0),
    Vec2::new(518.0, 194.0),
    Vec2::new(518.0, 271.0),
    Vec2::new(518.0, 348.0),
];
const SLOT_FRAME: [Vec2; 4] = [
    Vec2::new(516.8, 160.55),
    Vec2::new(515.7, 237.4),
    Vec2::new(516.0, 314.5),
    Vec2::new(516.5, 390.5),
];
const SLOT_FRAME_SIZE: [Vec2; 4] = [
    Vec2::new(107.0, 85.0),
    Vec2::new(107.0, 79.0),
    Vec2::new(107.0, 80.0),
    Vec2::new(107.0, 77.0),
];
/// The unearned trophy's picture's top left (element 29).
pub const TROPHY_PICTURE: Vec2 = Vec2::new(61.0, 114.0);
/// The close-up's card's middle and the scale its picture shows at
/// (measured on `rewards_card_view.png`: the court card's cream border
/// spans 334x216 there and 293x220 drawn at the frame's size; the group
/// `+0x214` takes its size from somewhere not traced, and the one grab is
/// a wide card's).
const VIEW_CARD: Vec2 = Vec2::new(322.0, 216.0);
const VIEW_SCALE: Vec2 = Vec2::new(334.0 / 293.0, 216.0 / 220.0);
/// A standing card (the legends', their pictures taller than wide) sits
/// lower: `FUN_002c4730` puts the card's group at y 240 for the legends'
/// and 220 for courts, jerseys and development points; on the grabs the
/// rewrite's crops sit 23 apart (`rewards_legend_closeup.png` and
/// `rewards_card_back.png`: Bobbito's spans 191..444 x 88..382).
const VIEW_STANDING: Vec2 = Vec2::new(0.0, 23.0);
/// The column movie's top left (`DAT_003fc540`, `DAT_003fc544` less 16).
const COLUMN_MOVIE: Vec2 = Vec2::new(461.0, 70.0);
/// The trophy's name's stars, this far out of the name (`FUN_002c2060`).
const NAME_STAR_OUT: f32 = 15.0;
/// Colours: the texts' authored ones (props 10).
const GOLD: [u8; 4] = [0xfb, 0xac, 0x00, 0xff];
const CREAM: [u8; 4] = [0xf6, 0xf4, 0xd7, 0xff];
const GREY: [u8; 4] = [0x80, 0x80, 0x80, 0xff];
const STEEL: [u8; 4] = [0x98, 0xa8, 0xb9, 0xff];
const BLACK: [u8; 4] = [0, 0, 0, 0xff];
/// The outline the card texts carry (`+0x4a8` black, `+0x4b8` 2).
const OUTLINE: f32 = 2.0;

/// The close-up's picture, decoded when the close-up asks for it: its
/// index in `XAFERWD`, the image and its size.
#[derive(Resource, Default)]
pub struct BigCard(pub Option<(usize, Handle<Image>, Vec2)>);

/// The close-up's Info turn (`FUN_002c4c38`): Circle on a legend's card
/// turns it to its back (`XARWDBK`, the bio, its words drawn into the
/// picture) and again to its front. Each front-end update moves `+0x2d4`
/// 0.125 toward 0.5, where the picture swaps, then back to 0; the card
/// draws at `1 - 2 * t` of its width about its middle, the back at the
/// front's size.
#[derive(Resource, Default)]
pub struct Turn {
    /// The back's picture: its index in `XARWDBK` and its image.
    pub back: Option<(usize, Handle<Image>)>,
    /// The back shows when still (`+0x2b4` 0).
    pub back_shown: bool,
    /// When the turn began (real seconds), while it runs.
    pub began: Option<f32>,
    /// This frame's face (the back) and `2 * t`, the width given up.
    pub face_back: bool,
    pub squeeze: f32,
}

/// Where the screen stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// 0..4 the card categories, [`TROPHIES`].
    pub tab: usize,
    /// The focused card's row among the two shown, its column and the
    /// first row shown (`+0x264`, `+0x268`, `+0x26c`).
    pub row: usize,
    pub col: usize,
    pub scroll: usize,
    /// The focused trophy (`+0x270`).
    pub trophy: usize,
    /// The close-up (`+0x260` 2): the card's place among the cards one
    /// may look at (`+0x288`).
    pub view: Option<usize>,
    /// Circle asked the close-up's card to turn (`+0x2bc`), until the turn
    /// ends.
    pub turn: bool,
}

impl State {
    /// The focused card's place in its category.
    pub fn index(&self) -> usize {
        (self.row + self.scroll) * columns(self.tab) + self.col
    }
}

fn columns(tab: usize) -> usize {
    COLUMNS.get(tab).copied().unwrap_or(1)
}

/// A category's cards, in record order (`FUN_001d2490`).
pub fn cards(disc: &crate::setup::Disc, tab: usize) -> Vec<&Card> {
    disc.cards
        .iter()
        .filter(|c| c.category as usize == tab && rewards::in_shop(c))
        .collect()
}

/// The cards the close-up steps through (`FUN_001d2c28`, `FUN_001d2a80`):
/// the owned ones of the five categories, in record order.
pub fn viewable<'a>(story: &Story, disc: &'a crate::setup::Disc) -> Vec<&'a Card> {
    disc.cards
        .iter()
        .filter(|c| (0..TROPHIES as i16).contains(&c.category))
        .filter(|c| rewards::in_shop(c) && rewards::card_owned(story, c))
        .collect()
}

/// A card's close-up picture in `XAFERWD` (`FUN_002c2fb8`): its picture,
/// less 0x1c past 0x43, 0x49 for Nelly's 0x67; 0x4a for the development
/// points' (0x4b in French).
pub fn big_picture(card: &Card) -> usize {
    let p = i32::from(card.poster);
    let k = if card.category == rewards::DEVELOPMENT {
        0x4a
    } else if p == 0x67 {
        0x49
    } else if p >= 0x44 {
        p - 0x1c
    } else {
        p
    };
    k.max(0) as usize
}

/// A card's back in `XARWDBK` (`FUN_002c2fb8`'s tail, archive slot 3):
/// only the legends' have one, picture 0x67 (Nelly's) 0x22, those under
/// 0x1f less 1, the rest less 0x42. The original skips it in French.
pub fn back_picture(card: &Card) -> Option<usize> {
    if card.category != rewards::STREET_LEGENDS && card.category != rewards::LEAGUE_LEGENDS {
        return None;
    }
    let p = i32::from(card.poster);
    let k = match p {
        0x67 => 0x22,
        p if p < 0x1f => p - 1,
        p => p - 0x42,
    };
    usize::try_from(k).ok()
}

/// The turn after `n` front-end updates (`FUN_002c4c38`): its `t` and
/// whether the faces have swapped, or None once it has ended (`t` back to
/// 0, `FUN_002c4b90`).
pub fn turn_at(n: u32) -> Option<(f32, bool)> {
    match n {
        0..=4 => Some((n as f32 * 0.125, false)),
        5..=8 => Some(((9 - n) as f32 * 0.125, true)),
        _ => None,
    }
}

/// The close-up's help line (`poster.txt`): the court's, a player's (the
/// St. Lunatics' card the plural), a jersey's, the development points'.
pub fn view_help(card: &Card) -> usize {
    match card.category {
        rewards::COURTS => 27,
        rewards::JERSEYS => 30,
        rewards::DEVELOPMENT => 31,
        _ if rewards::card_records(card).len() > 1 => 29,
        _ => 28,
    }
}

/// L1 (-1) or R1 (+1): the next tab, the grid at its start
/// (`FUN_002c1df8`, `FUN_002c2ce0`).
pub fn step_tab(s: &mut State, dir: i32) {
    s.tab = (s.tab as i32 + dir).rem_euclid(TABS as i32) as usize;
    s.row = 0;
    s.col = 0;
    s.scroll = 0;
    s.trophy = 0;
    s.view = None;
    s.turn = false;
}

/// A step on the grid (`FUN_002c2060`): left and right along the cards,
/// up and down a row; a step past the cards stays put, except down onto a
/// short last row, which takes its last card. Whether it moved.
pub fn step_grid(s: &mut State, count: usize, d_row: i32, d_col: i32) -> bool {
    let cols = columns(s.tab);
    if count == 0 || cols == 0 {
        return false;
    }
    let at = s.index() as i64;
    let mut to = at + i64::from(d_col) + i64::from(d_row) * cols as i64;
    if to < 0 {
        return false;
    }
    if to >= count as i64 {
        let last_row = (count as i64 - 1) / cols as i64;
        if d_row > 0 && to / cols as i64 == last_row {
            to = count as i64 - 1;
        } else {
            return false;
        }
    }
    if to == at {
        return false;
    }
    let to = to as usize;
    let row = to / cols;
    if row > s.scroll + 1 {
        s.scroll = row - 1;
    } else if row < s.scroll {
        s.scroll = row;
    }
    s.row = row - s.scroll;
    s.col = to % cols;
    true
}

/// Up or down among the trophies (clamped).
pub fn step_trophy(s: &mut State, dir: i32) -> bool {
    let to = (s.trophy as i32 + dir).clamp(0, 3) as usize;
    let moved = to != s.trophy;
    s.trophy = to;
    moved
}

/// A price as the strings write `%1N` ("2,000").
fn grouped(n: i32) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// A card's description in the questions (`FUN_002c4dd8`: "Reward #%1d %2s
/// %3s", its place, its category and its name; a card hidden until owned
/// is "Reward #%1d").
pub fn description(look: &Look, disc: &crate::setup::Disc, card: &Card, place: usize) -> String {
    let n = (place + 1).to_string();
    if rewards::HIDDEN_UNTIL_OWNED.contains(&card.id) {
        return look
            .text("poster.txt", 7)
            .unwrap_or("Reward #%1d")
            .replace("%1d", &n);
    }
    let cat = TAB_NAME
        .get(card.category as usize)
        .and_then(|&k| look.text("poster.txt", k))
        .unwrap_or("");
    look.text("poster.txt", 5)
        .unwrap_or("Reward #%1d %2s %3s")
        .replace("%1d", &n)
        .replace("%2s", cat)
        .replace("%3s", &rewards::card_name(&disc.poster, card))
}

/// The help line (`FUN_002c2060`): a locked card's way to unlock, an owned
/// one's "Select small picture ..."; a trophy's way to earn it, or that it
/// was earned.
pub fn help(look: &Look, story: &Story, disc: &crate::setup::Disc, s: &State) -> String {
    if let Some(v) = s.view {
        return viewable(story, disc)
            .get(v)
            .and_then(|c| look.text("poster.txt", view_help(c)))
            .unwrap_or("")
            .to_owned();
    }
    if s.tab == TROPHIES {
        let k = s.trophy.min(3);
        let at = if story.trophies()[k] { 200 + k } else { 37 + k };
        return look.text("poster.txt", at).unwrap_or("").to_owned();
    }
    let all = cards(disc, s.tab);
    let Some(card) = all.get(s.index()) else {
        return String::new();
    };
    if rewards::card_owned(story, card) {
        return look.text("poster.txt", 32).unwrap_or("").to_owned();
    }
    look.text("poster.txt", HELP_BASE[s.tab] + s.index())
        .unwrap_or("")
        .replace("%1N", &grouped(i32::from(card.cost)))
}

/// What Cross on the focused card does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Not enough points (`poster.txt` 8; Continue).
    Cannot(String),
    /// "Use %1N Reward Points to Unlock %2s?" (4; Yes, No) for the card.
    Unlock { card: i16, words: String },
}

impl Ask {
    pub fn words(&self) -> &str {
        match self {
            Ask::Cannot(w) | Ask::Unlock { words: w, .. } => w,
        }
    }

    pub fn rows(&self, look: &Look) -> Vec<String> {
        let common = |k: usize, or: &str| look.text("common.txt", k).unwrap_or(or).to_owned();
        match self {
            Ask::Cannot(_) => vec![common(20, "Continue")],
            Ask::Unlock { .. } => vec![common(2, "Yes"), common(3, "No")],
        }
    }
}

/// The question Cross on a locked card asks (`FUN_002c1370` case 8).
pub fn ask(look: &Look, story: &Story, disc: &crate::setup::Disc, s: &State) -> Option<Ask> {
    let all = cards(disc, s.tab);
    let card = *all.get(s.index())?;
    if rewards::card_owned(story, card) {
        return None;
    }
    let what = description(look, disc, card, s.index());
    if i64::from(card.cost) > i64::from(story.reward_points) {
        let words = look
            .text("poster.txt", 8)
            .unwrap_or("You do not possess enough Reward Points to unlock %1s.")
            .replace("%1s", &what);
        Some(Ask::Cannot(words))
    } else {
        let words = look
            .text("poster.txt", 4)
            .unwrap_or("Use %1N Reward Points to Unlock %2s?")
            .replace("%1N", &grouped(i32::from(card.cost)))
            .replace("%2s", &what);
        Some(Ask::Unlock {
            card: card.id,
            words,
        })
    }
}

/// The width the questions wrap to (`FUN_0025d660`/`FUN_0025d6d8`'s
/// 265).
pub const ASK_WIDTH: f32 = 265.0;

/// The development points' recipient (`FUN_002c1370` on Assign Points:
/// `FUN_0024ed88(.., 0x916 or 0x917, .., 0x3ed)`, the list dialog of
/// `FUN_002585f8`): "Which Created Baller will / receive these development
/// points?" (`poster.txt` 22; "( NO CREATED BALLERS )", 23, with none) in
/// font 2 and 0xffdcdcbb at the panel's top, the created ballers numbered
/// under it (`FUN_00258a48(.., 0x10)`: "%d. NAME", the lit one amber), and
/// the legend's Accept (`common.txt` 0x1003, the Cross) and Cancel (5, the
/// Triangle) at its foot. The screen behind keeps its light; the panel's
/// body (`FUN_00211278(.., 10, 0x1c)`) keeps 0.096 of it (measured over
/// the close-up's card). Its panel's size and the rows' place are set at
/// run time (`DAT_003f69b8` is zero on the disc): measured on
/// `dev_recipient.png`.
pub struct Recipient<'a> {
    pub names: &'a [String],
    pub lit: usize,
    pub ticks: u64,
}

const RECIPIENT_PANEL: [f32; 4] = [121.0, 64.0, 399.0, 349.0];
const RECIPIENT_BODY: u8 = 0xe6;
const RECIPIENT_TITLE_AT: Vec2 = Vec2::new(320.0, 76.0);
const RECIPIENT_TITLE: [u8; 4] = [0xdc, 0xdc, 0xbb, 0xff];
const RECIPIENT_ROWS_AT: Vec2 = Vec2::new(320.0, 170.0);
const RECIPIENT_ROW_STEP: f32 = 30.0;
const RECIPIENT_LEGEND_AT: Vec2 = Vec2::new(320.0, 402.0);

/// The recipient dialog's pieces, back to front.
pub fn recipient_pieces(
    look: &Look,
    gm: &crate::boards::Gm,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    panels: &mut crate::boards::Panels,
    r: &Recipient,
) -> Vec<Piece> {
    let mut out = Vec::new();
    out.push(panels.get(RECIPIENT_PANEL, RECIPIENT_BODY, gm, images));
    let words = |id: usize, or: &str| {
        look.text("poster.txt", id)
            .unwrap_or(or)
            .replace("\\n", "\n")
    };
    let title = words(
        if r.names.is_empty() { 23 } else { 22 },
        "Which Created Baller will
receive these development points?",
    );
    let mut w = crate::hud::Writer {
        labels,
        look,
        images,
    };
    w.text(
        &mut out,
        &title,
        crate::hud::TextStyle::plain(2, RECIPIENT_TITLE),
        RECIPIENT_TITLE_AT,
        1,
        1.0,
    );
    let mut stars = Vec::new();
    for (k, name) in r.names.iter().enumerate() {
        let spot = look::RowSpot {
            at: RECIPIENT_ROWS_AT + Vec2::new(0.0, k as f32 * RECIPIENT_ROW_STEP),
            font: 2,
            align: look::Align::Centre,
            valign: 0,
        };
        look::screen_row(
            &mut out,
            &mut stars,
            labels,
            look,
            images,
            spot,
            &format!("{}. {name}", k + 1),
            k == r.lit,
            false,
            r.ticks,
        );
    }
    out.extend(stars);
    let mut e = street_assets::font::Element {
        kind: 34,
        id: 0x22,
        align: 1 | (1 << 2),
        props: Default::default(),
    };
    e.props.insert(
        3,
        (RECIPIENT_LEGEND_AT.x as u32) | ((RECIPIENT_LEGEND_AT.y as u32) << 16),
    );
    // The legend lays its pairs out from the last: Accept (pair 0) shows
    // on the left.
    e.props.insert(13, 1);
    e.props.insert(56, 5);
    e.props.insert(57, 0x1003);
    e.props.insert(60, 1);
    e.props.insert(61, 0);
    look::legend(&mut out, labels, look, images, &e, &|id| {
        look.text_id(id).map(str::to_owned)
    });
    out
}

/// The screen's pieces over its layout.
#[allow(clippy::too_many_arguments)]
pub fn pieces(
    look: &Look,
    art: &crate::art::Art,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    layout: &Layout,
    story: &Story,
    disc: &crate::setup::Disc,
    s: &State,
    (film, white, big): (
        Option<Handle<Image>>,
        Option<&Handle<Image>>,
        Option<(Handle<Image>, Vec2, f32)>,
    ),
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let spot = |id: u32| {
        layout
            .elements
            .iter()
            .find(|e| e.id == id && e.kind != 0)
            .and_then(|e| e.position())
            .map(|(x, y)| Vec2::new(f32::from(x), f32::from(y)))
    };
    let outlined = |out: &mut Vec<Piece>,
                    labels: &mut look::Labels,
                    images: &mut Assets<Image>,
                    font: usize,
                    words: &str,
                    colour: [u8; 4],
                    at: Vec2| {
        let mut w = crate::hud::Writer {
            labels,
            look,
            images,
        };
        let style = crate::hud::TextStyle {
            outline: Some((BLACK, Vec2::splat(OUTLINE))),
            ..crate::hud::TextStyle::plain(font, colour)
        };
        w.text(out, words, style, at, 1 | 4, 1.0);
    };
    let poster = |k: usize| look.text("poster.txt", k).unwrap_or("").to_owned();
    if let Some(v) = s.view {
        let all = viewable(story, disc);
        // The card on the close-up's backdrop (its rays and stars), the
        // place between the arrows; a turning card gives up its width.
        if let Some((image, size, squeeze)) = big
            && squeeze < 1.0
        {
            let at = if size.y > size.x {
                VIEW_CARD + VIEW_STANDING
            } else {
                VIEW_CARD
            };
            let size = size * VIEW_SCALE * Vec2::new(1.0 - squeeze, 1.0);
            out.push(Piece::new(image.clone(), Rect::from_center_size(at, size)));
        }
        // A jersey's Mitchell & Ness label (`FUN_002c3948` shows element
        // 23 for the jerseys alone).
        if all.get(v).is_some_and(|c| c.category == rewards::JERSEYS)
            && let (Some(ra), Some(at)) = (art.reward_art.as_ref(), spot(23))
        {
            let (image, size) = &ra.jersey_label;
            out.push(Piece::new(image.clone(), Rect::from_center_size(at, *size)));
        }
        // The development points' amount on their card (`FUN_002c4730`:
        // element 14, kind 35, "%d" of `FUN_001d2818`).
        if let Some(card) = all.get(v).filter(|c| c.category == rewards::DEVELOPMENT)
            && let Some(e) = layout.elements.iter().find(|e| e.id == 14 && e.kind == 35)
            && let Some(at) = spot(14)
        {
            let font = e.props.get(&13).map_or(6, |&f| f as usize);
            let shade = |k: u16, or: [u8; 4]| {
                e.props
                    .get(&k)
                    .map_or(or, |&c| street_assets::font::argb(c))
            };
            let shadow = (
                shade(23, look::colours::COOL.0),
                shade(24, look::colours::COOL.1),
            );
            if let Some(p) = labels.text(
                look,
                images,
                font,
                &card.index.to_string(),
                e.colour().unwrap_or(CREAM),
                Some(shadow),
                at,
                look::Align::Centre,
            ) {
                out.push(p);
            }
        }
        if let Some(at) = spot(10) {
            let mut colour = GOLD;
            colour[3] = look::row_pulse(ticks);
            let words = format!("{} / {}", v + 1, all.len());
            place(&mut out, labels, look, images, 2, &words, colour, at, 1 | 4);
        }
        if all.len() > 1
            && let Some(pm) = look.prematch.as_ref()
        {
            for (id, image) in [(9, &pm.arrow_left), (15, &pm.arrow_right)] {
                if let Some(at) = spot(id) {
                    out.push(Piece::new(
                        image.clone(),
                        Rect::from_center_size(at, pm.arrow_side_size),
                    ));
                }
            }
        }
        // The legends (`FUN_002c3948`): a legend's card has Back and Info
        // (8); the others Back (5), with Assign Points for the development
        // points (`FUN_0021aa10` / `FUN_0021a9b8` on its second pair); Help.
        let category = all.get(v).map_or(0, |c| c.category);
        let legend = if category < 2 { 8 } else { 5 };
        for e in layout.elements.iter().filter(|e| e.kind == 34) {
            let mut e = e.clone();
            match e.id {
                id if id == legend => {
                    if legend == 5 && category != rewards::DEVELOPMENT {
                        e.props.insert(57, 0);
                    }
                }
                0xffff => {}
                _ => continue,
            }
            look::legend(&mut out, labels, look, images, &e, &|id| {
                look.text_id(id).map(str::to_owned)
            });
        }
        return out;
    }
    // The tab's name on the strip's node, and its shoulders.
    if let Some(at) = spot(27) {
        let mut colour = GOLD;
        colour[3] = look::row_pulse(ticks);
        place(
            &mut out,
            labels,
            look,
            images,
            2,
            &poster(TAB_NAME[s.tab.min(TROPHIES)]),
            colour,
            at,
            1 | 4,
        );
    }
    look::shoulder_tabs(&mut out, look, layout, (28, 35), (24, 33));
    let reward_art = art.reward_art.as_ref();
    let won = story.trophies();
    // The column's movie (`XLP0009`, at (461, 70)): its shelves' trophies,
    // each slot not yet earned covered by its panel, bar and star.
    let film_size = film
        .as_ref()
        .and_then(|h| images.get(h))
        .map(|i| i.size().as_vec2());
    if s.tab != TROPHIES
        && won.iter().any(|&w| w)
        && let (Some(image), Some(size)) = (film.as_ref(), film_size)
    {
        out.push(Piece::new(
            image.clone(),
            Rect::from_corners(COLUMN_MOVIE, COLUMN_MOVIE + size),
        ));
    }
    let rect_of = |id: u32| {
        let e = layout.elements.iter().find(|e| e.kind == 2 && e.id == id)?;
        let ((x, y), (w, h)) = (e.position()?, e.size()?);
        let colour = e.colour()?;
        Some((
            Rect::new(
                f32::from(x),
                f32::from(y),
                f32::from(x) + f32::from(w),
                f32::from(y) + f32::from(h),
            ),
            colour,
        ))
    };
    // The trophy column: a slot not yet earned shows its panel, bar and
    // star; an earned one, on the Trophies tab, the trophy's picture.
    for (k, &at) in SLOT.iter().enumerate() {
        if !won[k] {
            if let Some(white) = white {
                for id in [39 + k as u32, 43 + k as u32] {
                    if let Some((rect, [r, g, b, a])) = rect_of(id) {
                        let mut p = Piece::new(white.clone(), rect);
                        p.colour = Color::srgba_u8(r, g, b, a);
                        out.push(p);
                    }
                }
            }
            if let Some(ra) = reward_art {
                out.push(Piece::new(
                    ra.star.0.clone(),
                    Rect::from_center_size(at, ra.star.1),
                ));
            }
        } else if s.tab == TROPHIES
            && let Some(picture) = art.trophies.get(k)
        {
            let size = SLOT_FRAME_SIZE[k];
            let bottom = SLOT_FRAME[k];
            out.push(Piece::new(
                picture.clone(),
                Rect::new(
                    bottom.x - size.x / 2.0,
                    bottom.y - size.y,
                    bottom.x + size.x / 2.0,
                    bottom.y,
                ),
            ));
        }
    }
    // The legends: Exit (18) and Help always, Unlock or View Reward (17)
    // on the grid with the focused card's word; the close-up's two (5, 8)
    // not here.
    for e in layout.elements.iter().filter(|e| e.kind == 34) {
        let mut e = e.clone();
        match e.id {
            // Exit on Start (`FUN_0021a7c0(.., 4, ..)`, `FUN_0021aa70(.., 0,
            // 0x13)`).
            18 => {
                e.props.insert(56, 0x13);
                e.props.insert(60, 4);
            }
            0xffff => {}
            17 if s.tab != TROPHIES => {
                e.props
                    .insert(56, 0x900 | legend_word(story, disc, s) as u32);
            }
            _ => continue,
        }
        look::legend(&mut out, labels, look, images, &e, &|id| {
            look.text_id(id).map(str::to_owned)
        });
    }
    if s.tab == TROPHIES {
        let k = s.trophy.min(3);
        // The trophy: its movie once earned, else the picture of a trophy
        // to come.
        if won[k] {
            if let (Some(image), Some(size)) = (film.as_ref(), film_size) {
                out.push(Piece::new(
                    image.clone(),
                    Rect::from_corners(TROPHY_PICTURE, TROPHY_PICTURE + size),
                ));
            }
        } else {
            let mut x = TROPHY_PICTURE.x;
            for (image, size) in &art.trophy_unknown {
                let min = Vec2::new(x, TROPHY_PICTURE.y);
                out.push(Piece::new(
                    image.clone(),
                    Rect::from_corners(min, min + *size),
                ));
                x += size.x;
            }
        }
        if let Some(ra) = reward_art {
            let (bottom, size) = (SLOT_FRAME[k], SLOT_FRAME_SIZE[k]);
            out.push(Piece::new(
                ra.trophy_frame.0.clone(),
                Rect::new(
                    bottom.x - size.x / 2.0,
                    bottom.y - size.y,
                    bottom.x + size.x / 2.0,
                    bottom.y,
                ),
            ));
        }
        // The name between its stars, and under it the three stars or
        // "Completed".
        let name = poster(33 + k);
        if let Some(at) = spot(54) {
            place(&mut out, labels, look, images, 3, &name, CREAM, at, 1 | 4);
            if let (Some(ra), Some(f), Some(star_y)) =
                (reward_art, look.fonts.get(3), spot(56).map(|p| p.y))
            {
                let half = f.measure(&name) as f32 / 2.0;
                for x in [at.x + half + NAME_STAR_OUT, at.x - half - NAME_STAR_OUT] {
                    out.push(Piece::new(
                        ra.name_star.0.clone(),
                        Rect::from_center_size(Vec2::new(x, star_y), ra.name_star.1),
                    ));
                }
            }
        }
        if won[k] {
            if let Some(at) = spot(50) {
                place(
                    &mut out,
                    labels,
                    look,
                    images,
                    2,
                    &poster(26),
                    CREAM,
                    at,
                    1 | 4,
                );
            }
        } else if let Some(ra) = reward_art {
            for id in [47, 48, 49] {
                if let Some(at) = spot(id) {
                    out.push(Piece::new(
                        ra.base_star.0.clone(),
                        Rect::from_center_size(at, ra.base_star.1),
                    ));
                }
            }
        }
        if let Some(at) = spot(19) {
            let words = poster(21)
                .replace("%1d", &(k + 1).to_string())
                .replace("%2d", "4");
            place(&mut out, labels, look, images, 1, &words, STEEL, at, 4);
        }
        return out;
    }
    // The grid.
    let all = cards(disc, s.tab);
    let cols = columns(s.tab);
    let focus = s.index();
    let frames = &art.card_frames;
    let drop = if s.tab == 4 { DEV_DROP } else { 0.0 };
    let mut focused_at = None;
    for (row, &row_y) in ROW_Y.iter().enumerate() {
        for col in 0..cols {
            let at_index = (row + s.scroll) * cols + col;
            let Some(card) = all.get(at_index) else {
                continue;
            };
            let owned = rewards::card_owned(story, card);
            let picture = rewards::card_picture(card, owned, frames.len());
            let Some(Some((image, size))) = frames.get(picture) else {
                continue;
            };
            let step = size.x + GAP;
            let x = GRID_X - step * (cols as f32 - 1.0) / 2.0 + step * col as f32;
            let centre = Vec2::new(x, row_y);
            let focused = at_index == focus;
            let dim = u32::from(!focused) + u32::from(!owned);
            let light = (0xff_u32 >> dim) as f32 / 255.0;
            let mut piece = Piece::new(image.clone(), Rect::from_center_size(centre, *size));
            piece.colour = Color::srgb(light, light, light);
            out.push(piece);
            if focused {
                focused_at = Some(centre);
            }
            if !owned && !focused {
                outlined(
                    &mut out,
                    labels,
                    images,
                    2,
                    &poster(2),
                    GREY,
                    Vec2::new(x, ROW_Y[row] + drop),
                );
            }
        }
    }
    if let (Some(centre), Some(card)) = (focused_at, all.get(focus)) {
        if let Some(ra) = reward_art {
            let (image, size) = if WIDE[s.tab] {
                &ra.frame_wide
            } else {
                &ra.frame
            };
            out.push(Piece::new(
                image.clone(),
                Rect::from_center_size(centre, *size),
            ));
        }
        if !rewards::card_owned(story, card) {
            let words = poster(1).replace("%1N", &grouped(i32::from(card.cost)));
            outlined(
                &mut out,
                labels,
                images,
                1,
                &words.replace("\\n", "\n"),
                GOLD,
                centre + Vec2::new(PRICE_DX, 0.0),
            );
        }
    }
    // The grid's arrows: more rows above, more below.
    if let Some(pm) = look.prematch.as_ref() {
        let rows = all.len().div_ceil(cols);
        if s.scroll > 0
            && let Some(at) = spot(58)
        {
            out.push(Piece::new(
                pm.arrow_up.clone(),
                Rect::from_center_size(at, pm.arrow_size),
            ));
        }
        if s.scroll + 2 < rows
            && let Some(at) = spot(21)
        {
            out.push(Piece::new(
                pm.arrow_down.clone(),
                Rect::from_center_size(at, pm.arrow_size),
            ));
        }
    }
    // The points, the place, the legend's words.
    if let (Some(at), Some(f)) = (spot(20), look.fonts.get(2)) {
        let label = poster(19);
        place(&mut out, labels, look, images, 2, &label, CREAM, at, 4);
        let value_at = Vec2::new(at.x + f.measure(&label) as f32 + 9.0, at.y);
        place(
            &mut out,
            labels,
            look,
            images,
            2,
            &grouped(story.reward_points as i32),
            GOLD,
            value_at,
            4,
        );
    }
    if let Some(at) = spot(19) {
        let words = poster(20)
            .replace("%1d", &(focus + 1).to_string())
            .replace("%2d", &all.len().to_string());
        place(&mut out, labels, look, images, 1, &words, STEEL, at, 4);
    }
    out
}

/// The grid's legend's label: Unlock Reward or View Reward (`poster.txt`
/// 17, 18; `FUN_0021aa70(legend 17, 0, ..)`).
pub fn legend_word(story: &Story, disc: &crate::setup::Disc, s: &State) -> usize {
    let all = cards(disc, s.tab);
    match all.get(s.index()) {
        Some(card) if rewards::card_owned(story, card) => 18,
        _ => 17,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(category: i16, poster: i16) -> Card {
        Card {
            id: 0,
            category,
            index: 0,
            cost: 0,
            poster,
            param: -1,
        }
    }

    #[test]
    fn the_legends_backs_are_the_traced_pictures() {
        // Bobbito (98) is back 32, Nelly (0x67) the last, 34; the first
        // legend's picture 1 is back 0; courts have none.
        assert_eq!(back_picture(&card(0, 98)), Some(32));
        assert_eq!(back_picture(&card(0, 0x67)), Some(0x22));
        assert_eq!(back_picture(&card(0, 1)), Some(0));
        assert_eq!(back_picture(&card(1, 30)), Some(29));
        assert_eq!(back_picture(&card(0, 96)), Some(30));
        assert_eq!(back_picture(&card(rewards::COURTS, 31)), None);
    }

    #[test]
    fn a_turn_squeezes_to_nothing_swaps_and_opens() {
        let ts: Vec<_> = (0..10).map(turn_at).collect();
        assert_eq!(ts[0], Some((0.0, false)));
        assert_eq!(ts[4], Some((0.5, false)));
        // The swap's update keeps t at 0.5: two updates draw nothing.
        assert_eq!(ts[5], Some((0.5, true)));
        assert_eq!(ts[8], Some((0.125, true)));
        assert_eq!(ts[9], None);
    }

    fn at(tab: usize, i: usize) -> State {
        let cols = columns(tab);
        State {
            tab,
            row: i / cols,
            col: i % cols,
            ..Default::default()
        }
    }

    #[test]
    fn the_tabs_go_round_past_tips_and_codes() {
        let mut s = State::default();
        step_tab(&mut s, -1);
        assert_eq!(s.tab, TROPHIES);
        step_tab(&mut s, -1);
        assert_eq!(s.tab, 4);
        step_tab(&mut s, 1);
        step_tab(&mut s, 1);
        assert_eq!(s.tab, 0);
    }

    #[test]
    fn the_grid_steps_along_its_cards_and_scrolls() {
        // Nine Street Legends, four a row: rows 0..2.
        let mut s = at(0, 0);
        assert!(!step_grid(&mut s, 9, 0, -1));
        assert!(step_grid(&mut s, 9, 0, 1));
        assert_eq!(s.index(), 1);
        // Down twice from the second card: the third row scrolls in.
        assert!(step_grid(&mut s, 9, 1, 0));
        assert!(step_grid(&mut s, 9, 1, 0));
        assert_eq!((s.index(), s.scroll, s.row), (8, 1, 1));
        // The ninth is the last: right stays.
        assert!(!step_grid(&mut s, 9, 0, 1));
        // Up from the top shown row scrolls back.
        let mut s = at(0, 4);
        s.scroll = 1;
        s.row = 0;
        assert!(step_grid(&mut s, 9, -1, 0));
        assert_eq!((s.index(), s.scroll, s.row), (0, 0, 0));
        // Down onto a short last row takes its last card.
        let mut s = at(0, 7);
        assert!(step_grid(&mut s, 9, 1, 0));
        assert_eq!(s.index(), 8);
        // Left from a row's first card goes to the row above's last.
        let mut s = at(0, 4);
        assert!(step_grid(&mut s, 9, 0, -1));
        assert_eq!(s.index(), 3);
    }

    #[test]
    fn prices_group_their_thousands() {
        assert_eq!(grouped(500), "500");
        assert_eq!(grouped(2000), "2,000");
        assert_eq!(grouped(10_000), "10,000");
    }
}
