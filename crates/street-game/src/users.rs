//! User Setup on the game's own layout (`entername.txt`): screen classes
//! 12..19, one constructor `FUN_00297848(screen, 1, from, next)` over
//! layout class 11 (blob 5) and backdrop 13, whose picture is the four
//! columns' panels. Pick Up games reach class 13 from Difficulty (class
//! 105) and go on to Choose Team (class 27, or 28 with people on both
//! sides). The trace is in `docs/FRONTEND_RECOVERY.md`, "User Setup".
//!
//! The layout holds the title (`entername.txt` 0 at (56, 58)), "vs" (font
//! 5 at (329, 154)), the help line (58, 424) and four column widgets (kind
//! 13, ids 1..4 at x 57, 189, 321, 453, y 113; prop 49 is the controller).
//! A column (`FUN_002401b8` builds its children in code, `FUN_00243730`
//! colours it per controller, `FUN_00246880` switches its states) shows,
//! by state:
//!
//! - 0, no controller: "Insert DUALSHOCK®2 analog controller into
//!   controller port N." (`entername_ps2.txt` 0, or 1 for ports 3 and 4),
//!   the frame at half alpha; a plugged controller that has not started
//!   reads "Press START To Enter Game" (`entername_ps2.txt` 2);
//! - 10, choosing a side (Pick Up games; `FUN_002445c0` moves the pieces):
//!   "Player N", AWAY and HOME rows (`entername.txt` 40, 42; the middle
//!   "Choose" row is hidden in the US), the controller icon between them
//!   with an arrow each side; a chosen side moves the icon past its row;
//! - 2, "Enter Name" (10, 11) with Yes and No (`common.txt` 2, 3);
//! - 3, "Select Name" (27, 28) over the saved names, "Enter New" (26)
//!   first, five rows 20 apart;
//! - 4, entering a name: "Enter" / "Name" and the on-screen keyboard
//!   (`FUN_0023c7b8`: A..Z six to a row, SPC, DEL, DONE; the typed name,
//!   "PLAYER%1d" until a key is pressed, over it; the cursor starts on
//!   DONE);
//! - 7, the career panel of the name chosen: the name, "%1dW - %2dL",
//!   WIN% as a baseball average, then Points, Trick Points, Dunks, Steals
//!   and Blocks (the user record's fields 0..6, `FUN_001ee328`);
//! - 9, a guest: "No Name Entered" / "User stats cannot be saved.";
//! - 11, ready: "Player N", the side's word (40 or 42) and "Ready" (25);
//!   once every column is ready (12), "Continue" (22) at the foot and the
//!   help line "Enter cheat codes now." (44).
//!
//! The rewrite shows the side chooser on Choose Sides (every controller's
//! column at once, each moved by its own pad) and the name states on the
//! page after it, where one cursor goes from column to column (the
//! original's columns each answer only their own controller).

use bevy::prelude::*;
use street_sim::records::User;

use crate::look::{self, Align, Look, Piece, RowSpot, colours, fonts};

/// The screen's backdrop (`FUN_002d7720(screen, 0xb, 0xd)`).
pub const BACKDROP: usize = 13;

/// The columns' top-left corners: the layout's kind-13 elements 1..4.
pub const COLUMNS: [Vec2; 4] = [
    Vec2::new(57.0, 113.0),
    Vec2::new(189.0, 113.0),
    Vec2::new(321.0, 113.0),
    Vec2::new(453.0, 113.0),
];
/// A column's frame (`FUN_002401b8`: 131 by 284) and its middle, where
/// the centred lines sit.
pub const WIDTH: f32 = 131.0;
pub const HEIGHT: f32 = 284.0;
pub const MIDDLE: f32 = 65.5;

/// 1P..4P's frame colours (`FUN_00243730`: 0xbd2908, 0xefce19, 0x083ae6,
/// 0x639c29), the off-screen markers' red, yellow, blue and green; then
/// 5P's and 6P's online, the rewrite's purple and orange
/// (`controls::SEAT_COLOURS`).
pub const PLAYER: [[u8; 4]; 6] = [
    [0xbd, 0x29, 0x08, 0xff],
    [0xef, 0xce, 0x19, 0xff],
    [0x08, 0x3a, 0xe6, 0xff],
    [0x63, 0x9c, 0x29, 0xff],
    [0x94, 0x33, 0xe0, 0xff],
    [0xf0, 0x80, 0x19, 0xff],
];
/// The labels' colour (`0xff88a0be`); values are cream (`0xffdcdcbb`),
/// names orange (`0xfffbac00`).
const LABEL: [u8; 4] = [0x88, 0xa0, 0xbe, 0xff];

/// Where the column's pieces sit, from its corner: the constructor
/// `FUN_002401b8` places each child at the column's point plus these
/// (the x of a centred line is [`MIDDLE`]).
pub mod at {
    /// Elements 0xf "Player" and 0xe, the number (font 5; the number 7
    /// left for 1P, `FUN_00243730`).
    pub const PLAYER: f32 = 18.0;
    pub const NUMBER: f32 = 39.0;
    /// Element 0xb, the message of an empty or idle column (font 0, one
    /// pixel left of the middle): 18 + 21 + 37.
    pub const MESSAGE: f32 = 76.0;
    /// The side chooser (list 0x34): the AWAY row's node 0x35 at 90, the
    /// HOME row's 0x37 at 230; the up arrow 0x3b at 128, the down arrow
    /// 0x3c at 198; the controller icon 0x40 at 151, 25 left of the middle.
    /// With AWAY chosen (`FUN_002445c0`): the icon at 68, AWAY at 108; with
    /// HOME: the icon at 208, HOME at 248; the arrows hidden either way.
    pub const AWAY: f32 = 90.0;
    pub const AWAY_PICKED: f32 = 108.0;
    pub const HOME: f32 = 230.0;
    pub const HOME_PICKED: f32 = 248.0;
    pub const ARROW_UP: f32 = 128.0;
    pub const ARROW_DOWN: f32 = 198.0;
    pub const PAD: f32 = 151.0;
    pub const PAD_AWAY: f32 = 68.0;
    pub const PAD_HOME: f32 = 208.0;
    pub const PAD_LEFT: f32 = 25.0;
    /// Elements 0x10 and 0x11 (font 3): "Enter" / "Name", "Select" /
    /// "Name".
    pub const ENTER: f32 = 75.0;
    pub const NAME: f32 = 110.0;
    /// The Yes / No rows' nodes 0x19 and 0x1a (18 + 21 + 37 + 75, then 34
    /// more).
    pub const YES: f32 = 151.0;
    pub const NO: f32 = 185.0;
    /// The saved names' list 0x12: five nodes 20 apart from 155.
    pub const LIST: f32 = 155.0;
    pub const LIST_STEP: f32 = 20.0;
    pub const LIST_ROWS: usize = 5;
    /// Element 0x3f, the side's word when ready (font 2), and 0x32,
    /// "Ready" (font 5; its height read from the original's screen).
    pub const SIDE_WORD: f32 = 76.0;
    pub const READY: f32 = 112.0;
    /// The foot (elements 0x44 and 0x43): the button icon's corner.
    pub const FOOT: (f32, f32) = (16.0, 285.0);
}

/// The career panel's rows (`FUN_002401b8`, elements 0x26..0x2f): the
/// label's top, `entername.txt` line, and the value's top; values are the
/// record's fields 2..6, `CAREER` 0..4 here.
const CAREER_ROWS: [(f32, usize, f32); 5] = [
    (69.0, 15, 90.0),
    (111.0, 16, 132.0),
    (153.0, 17, 174.0),
    (195.0, 18, 216.0),
    (237.0, 19, 258.0),
];

/// The on-screen keyboard's place in the column (element 0x1d at (6, 140))
/// and its keys' middles from there (`FUN_0023c7b8`): letter `k` at
/// (19 * (k % 6) + 10, 20 * (k / 6) + 25), then SPC, DEL and DONE.
const KEYBOARD_AT: Vec2 = Vec2::new(6.0, 140.0);

/// Where key `(row, column)` of [`crate::typing::NAME_KEYBOARD`] sits,
/// from the keyboard's corner.
fn key_middle(row: usize, column: usize) -> Vec2 {
    match (row, column) {
        // Y and Z close the fifth row; SPC and DEL follow them.
        (4, 2) => Vec2::new(19.0 * 2.0 + 20.0, 104.0),
        (4, 3) => Vec2::new(19.0 * 4.0 + 17.0, 104.0),
        (5, _) => Vec2::new(19.0 * 2.5 + 10.0, 124.0),
        _ => Vec2::new(19.0 * column as f32 + 10.0, 20.0 * row as f32 + 25.0),
    }
}

/// The cross button's icon (`XACOMMON` sheet 1, image 0; the foot's
/// element 0x44 at (16, 285), 0.8 of its size).
const CROSS: Rect = Rect {
    min: Vec2::new(36.0, 52.0),
    max: Vec2::new(60.0, 76.0),
};

/// A name being typed for a column: the text so far (empty while the
/// default "PLAYER%1d" shows) and the key under the on-screen keyboard's
/// cursor.
pub struct Typed<'a> {
    pub text: &'a str,
    pub key: (usize, usize),
}

/// What one column shows (the states above).
pub enum Column<'a> {
    /// State 0 with no controller in port `port` (1-based).
    Empty { port: usize },
    /// State 0 with a controller that has not started.
    Idle,
    /// State 10: the side chooser, with the side picked (0 home, 1 away)
    /// or none yet.
    Sides { side: Option<usize> },
    /// State 2: "Enter Name", Yes or No.
    Ask { yes: bool },
    /// State 3: the saved names, "Enter New" first; `at` the highlighted
    /// row, `first` the first row shown.
    Pick {
        names: Vec<String>,
        at: usize,
        first: usize,
    },
    /// State 4: entering a name.
    Typing(&'a Typed<'a>),
    /// State 9: playing as a guest.
    Guest,
    /// State 7: a saved name's career.
    Named(&'a User),
    /// State 11, and 12 (`go`) once every column is ready.
    Ready { side: usize, go: bool },
}

/// A count as `%1N` prints it: thousands apart.
pub(crate) fn count(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// WIN% as the column prints it (`FUN_00246880` state 7, the US formats
/// at `0x46e200`/`0x46e218`/`0x46e230`): a baseball average, ".000" with
/// no games and "1.000" unbeaten.
fn average(wins: u32, losses: u32) -> String {
    let games = u64::from(wins) + u64::from(losses);
    if games == 0 {
        ".000".into()
    } else if u64::from(wins) == games {
        "1.000".into()
    } else {
        let thousandths = (f64::from(wins) / games as f64 * 1000.0 + 0.5).floor() as u32;
        format!(".{thousandths:03}")
    }
}

/// The side chooser's places by the side picked (`FUN_002445c0`): the
/// AWAY row, the HOME row and the controller icon; the arrows show only
/// while no side is picked.
pub fn chooser_layout(side: Option<usize>) -> (f32, f32, f32) {
    match side {
        Some(1) => (at::AWAY_PICKED, at::HOME, at::PAD_AWAY),
        Some(_) => (at::AWAY, at::HOME_PICKED, at::PAD_HOME),
        None => (at::AWAY, at::HOME, at::PAD),
    }
}

/// The saved names list's row `k`, from its first shown.
pub fn list_row(k: usize) -> f32 {
    at::LIST + at::LIST_STEP * k as f32
}

/// The locale line, or `plain` without the disc's strings; the game's
/// `%%%%` (doubled twice for its two formatting passes) as one `%`.
fn line<'a>(look: &'a Look, file: &str, index: usize, plain: &'a str) -> String {
    look.text(file, index)
        .unwrap_or(plain)
        .replace("%%%%", "%")
        .replace("%%", "%")
}

/// The default name a column offers (`entername.txt` 45, "PLAYER%1d").
pub fn default_name(look: Option<&Look>, player: usize) -> String {
    let plain = "PLAYER%1d";
    let s = look.map_or(plain.to_owned(), |l| line(l, "entername.txt", 45, plain));
    s.replace("%1d", &(player + 1).to_string())
}

/// A solid rectangle (the frame's pieces are untextured quads).
fn solid(rect: Rect, colour: [u8; 4], alpha: f32) -> Piece {
    let [r, g, b, a] = colour;
    let mut p = Piece::new(Handle::default(), rect);
    p.colour = Color::srgba_u8(r, g, b, a).with_alpha(alpha * f32::from(a) / 255.0);
    p
}

/// The column's frame (`FUN_002401b8` children 0..9, coloured by
/// `FUN_00243730`): a 4-wide border in the player's colour with its top
/// left corner cut at 45 degrees, a 2-wide black line inside it, and the
/// player's tab at (18, 0).
fn frame(out: &mut Vec<Piece>, look: &Look, at: Vec2, width: f32, player: usize, alpha: f32) {
    let colour = PLAYER[player.min(PLAYER.len() - 1)];
    let black = [0, 0, 0, 0xff];
    let r =
        |x0: f32, y0: f32, x1: f32, y1: f32| Rect::new(at.x + x0, at.y + y0, at.x + x1, at.y + y1);
    // Children 0, 2, 3 and 1: top, left, right, bottom.
    out.push(solid(r(20.0, 0.0, width, 4.0), colour, alpha));
    out.push(solid(r(0.0, 20.0, 4.0, HEIGHT), colour, alpha));
    out.push(solid(r(width - 4.0, 0.0, width, HEIGHT), colour, alpha));
    out.push(solid(r(0.0, HEIGHT - 4.0, width, HEIGHT), colour, alpha));
    // Child 4: a 4 by 28 bar at (8, -3) turned 45 degrees, from (0, 20)
    // to (20, 0); drawn as a run of squares along it.
    for k in 0..=9 {
        let t = 2.0 * k as f32;
        out.push(solid(r(t, 17.0 - t, t + 4.0, 21.0 - t), colour, alpha));
    }
    // Children 5, 7, 8 and 6: the black lines, then child 9 (2 by 24 at
    // (11, 1), turned) inside the cut.
    out.push(solid(r(20.0, 4.0, width - 4.0, 6.0), black, alpha));
    out.push(solid(r(4.0, 20.0, 6.0, HEIGHT - 4.0), black, alpha));
    out.push(solid(
        r(width - 4.0, 4.0, width - 2.0, HEIGHT - 4.0),
        black,
        alpha,
    ));
    out.push(solid(
        r(4.0, HEIGHT - 4.0, width - 4.0, HEIGHT - 2.0),
        black,
        alpha,
    ));
    for k in 0..=8 {
        let t = 2.0 * k as f32;
        out.push(solid(r(4.0 + t, 19.0 - t, 6.0 + t, 21.0 - t), black, alpha));
    }
    // Child 10: the tab (`XACOMMON` sheet 0, image 2, 60 by 14 at (18,
    // 0)) in the controller's palette, `XPAL_FE` sheet 0's
    // (`FUN_00243730`, `FUN_00210368(tab, 0, k)`); without the palettes
    // the grey shape takes the player's colour.
    // 5P's and 6P's (online's three on three, the rewrite's colours) take
    // the grey shape in their colour too.
    if let Some(pm) = look.prematch.as_ref().filter(|_| player < 4) {
        let mut p = Piece::new(pm.tabs[player.min(3)].clone(), r(18.0, 0.0, 78.0, 14.0));
        p.colour = Color::WHITE.with_alpha(alpha);
        out.push(p);
    } else if let Some(tab) = &look.tab {
        let mut p = Piece::new(tab.clone(), r(18.0, 0.0, 78.0, 14.0));
        let [cr, cg, cb, _] = colour;
        p.colour = Color::srgba_u8(cr, cg, cb, 0xff).with_alpha(alpha);
        out.push(p);
    }
}

/// A line of text (lines break at the text's `\n`); `at` is the line's
/// top, centred lines centre on its x.
#[allow(clippy::too_many_arguments)]
fn text(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    font: usize,
    s: &str,
    colour: [u8; 4],
    at: Vec2,
    align: Align,
) {
    let step = look.fonts.get(font).map_or(18.0, |f| f.line_height as f32);
    for (k, part) in s.split("\\n").enumerate() {
        let at = at + Vec2::new(0.0, step * k as f32);
        if let Some(p) = labels.text(look, images, font, part.trim(), colour, None, at, align) {
            out.push(p);
        }
    }
}

/// "Player" and the number (elements 0xf and 0xe, font 5; the number 7
/// left for 1P, `FUN_00243730`).
fn header(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    (k, font): (usize, usize),
    origin: Vec2,
) {
    let player = line(look, "entername.txt", 2, "Player");
    text(
        out,
        look,
        labels,
        images,
        font,
        &player,
        colours::CREAM,
        origin + Vec2::new(MIDDLE, at::PLAYER),
        Align::Centre,
    );
    let nudge = if k == 0 { -7.0 } else { 0.0 };
    let number = line(look, "entername.txt", 3, "%1d").replace("%1d", &(k + 1).to_string());
    text(
        out,
        look,
        labels,
        images,
        font,
        &number,
        colours::CREAM,
        origin + Vec2::new(MIDDLE + nudge, at::NUMBER),
        Align::Centre,
    );
}

/// A row widget (kind 19, `FUN_0021c4c0`) centred on the column's middle
/// at `y`: the row colours, the pulse and the stars when lit.
#[allow(clippy::too_many_arguments)]
fn row(
    out: &mut Vec<Piece>,
    stars: &mut Vec<Piece>,
    labels: &mut look::Labels,
    look: &Look,
    images: &mut Assets<Image>,
    origin: Vec2,
    font: usize,
    s: &str,
    y: f32,
    lit: bool,
    ticks: u64,
) {
    look::screen_row(
        out,
        stars,
        labels,
        look,
        images,
        RowSpot {
            at: origin + Vec2::new(MIDDLE, y),
            font,
            align: Align::Centre,
            valign: 0,
        },
        s,
        lit,
        true,
        ticks,
    );
}

/// The screen's pieces: one column per controller port, `lit` the column
/// the rewrite's cursor is on, `ticks` the front-end updates since the
/// page opened (the highlighted row's pulse).
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    columns: &[Column],
    lit: Option<usize>,
    ticks: u64,
) -> Vec<Piece> {
    let places: Vec<Place> = COLUMNS
        .iter()
        .enumerate()
        .map(|(k, &corner)| Place {
            corner,
            width: WIDTH,
            player: k,
        })
        .collect();
    pieces_in(look, labels, images, columns, &places, lit, ticks)
}

/// A column's place: its corner, its width, and whose it is (the number
/// and colour, 0 for 1P).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub corner: Vec2,
    pub width: f32,
    pub player: usize,
}

/// `n` columns across the four's room (57 to 584): the four's own places
/// for four or fewer, narrower ones a pixel apart past four (the rewrite's
/// six for three on three online), numbered by `players`.
pub fn places(players: &[usize]) -> Vec<Place> {
    let n = players.len();
    if n <= COLUMNS.len() {
        return players
            .iter()
            .zip(COLUMNS)
            .map(|(&player, corner)| Place {
                corner,
                width: WIDTH,
                player,
            })
            .collect();
    }
    let room = COLUMNS[3].x + WIDTH - COLUMNS[0].x;
    let width = ((room - (n - 1) as f32) / n as f32).floor();
    players
        .iter()
        .enumerate()
        .map(|(k, &player)| Place {
            corner: Vec2::new(COLUMNS[0].x + k as f32 * (width + 1.0), COLUMNS[0].y),
            width,
            player,
        })
        .collect()
}

/// [`pieces`] in `places`: a column's centred lines on its own middle.
pub fn pieces_in(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    columns: &[Column],
    places: &[Place],
    lit: Option<usize>,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut stars = Vec::new();
    for (k, (column, place)) in columns.iter().zip(places).enumerate() {
        let Place {
            corner,
            width,
            player,
        } = *place;
        // The lines centred on the middle sit [`MIDDLE`] from this.
        let origin = corner + Vec2::new(width / 2.0 - MIDDLE, 0.0);
        // "Player N" and "Ready" (font 5) would spill out of a column
        // narrower than the four's: the rows' font there.
        let big = if width < WIDTH {
            fonts::LIST
        } else {
            fonts::TITLE_SMALL
        };
        // An empty controller's column is dimmed (`FUN_00244258` sets its
        // frame's alpha to 0x7f where no controller is in).
        let alpha = if matches!(column, Column::Empty { .. }) {
            0.5
        } else {
            1.0
        };
        frame(&mut out, look, corner, width, player, alpha);
        let at = |x: f32, y: f32| origin + Vec2::new(x, y);
        let lit = lit == Some(k);
        let art = look.prematch.as_ref();
        let picture = |out: &mut Vec<Piece>, image: &Handle<Image>, size: Vec2, min: Vec2| {
            out.push(Piece::new(
                image.clone(),
                Rect::from_corners(min, min + size),
            ));
        };
        match column {
            Column::Empty { port } => {
                // `FUN_00243730`: `entername_ps2.txt` 0 with the port's
                // name (`control_chk_ps2.txt` 3, 4) for ports 1 and 2, 1
                // (the multitap) past them.
                let s = if *port <= 2 {
                    let number = line(look, "control_chk_ps2.txt", 2 + port, "%1s");
                    line(
                        look,
                        "entername_ps2.txt",
                        0,
                        "Insert\\nDUALSHOCK®2\\nanalog\\ncontroller\\ninto\\ncontroller\\nport %1s.",
                    )
                    .replace("%1s", &number)
                } else {
                    line(
                        look,
                        "entername_ps2.txt",
                        1,
                        "Insert a\\nmultitap\\n(for\\nPlayStation®2)\\ninto\\ncontroller\\nport 1.",
                    )
                };
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    fonts::SMALL,
                    &s,
                    LABEL,
                    at(MIDDLE - 1.0, at::MESSAGE),
                    Align::Centre,
                );
                continue;
            }
            Column::Idle => {
                let s = line(
                    look,
                    "entername_ps2.txt",
                    2,
                    "Press\\nSTART\\nTo\\nEnter\\nGame",
                );
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    fonts::SMALL,
                    &s,
                    LABEL,
                    at(MIDDLE - 1.0, at::MESSAGE),
                    Align::Centre,
                );
                continue;
            }
            Column::Sides { side } => {
                header(&mut out, look, labels, images, (player, big), origin);
                let away = line(look, "entername.txt", 40, "AWAY");
                let home = line(look, "entername.txt", 42, "HOME");
                let (away_y, home_y, pad_y) = chooser_layout(*side);
                row(
                    &mut out,
                    &mut stars,
                    labels,
                    look,
                    images,
                    origin,
                    fonts::LIST,
                    &away,
                    away_y,
                    *side == Some(1),
                    ticks,
                );
                row(
                    &mut out,
                    &mut stars,
                    labels,
                    look,
                    images,
                    origin,
                    fonts::LIST,
                    &home,
                    home_y,
                    *side == Some(0),
                    ticks,
                );
                if let Some(art) = art {
                    picture(
                        &mut out,
                        &art.pad_icon,
                        art.pad_icon_size,
                        at(MIDDLE - at::PAD_LEFT, pad_y),
                    );
                    if side.is_none() {
                        let half = art.arrow_size / 2.0;
                        picture(
                            &mut out,
                            &art.arrow_up,
                            art.arrow_size,
                            at(MIDDLE - half.x, at::ARROW_UP - half.y),
                        );
                        picture(
                            &mut out,
                            &art.arrow_down,
                            art.arrow_size,
                            at(MIDDLE - half.x, at::ARROW_DOWN - half.y),
                        );
                    }
                }
                continue;
            }
            Column::Ask { yes } => {
                header(&mut out, look, labels, images, (player, big), origin);
                for (y, index, plain) in [(at::ENTER, 10, "Enter"), (at::NAME, 11, "Name")] {
                    let s = line(look, "entername.txt", index, plain);
                    text(
                        &mut out,
                        look,
                        labels,
                        images,
                        3,
                        &s,
                        LABEL,
                        at(MIDDLE, y),
                        Align::Centre,
                    );
                }
                let yes_word = line(look, "common.txt", 2, "Yes");
                let no_word = line(look, "common.txt", 3, "No");
                row(
                    &mut out,
                    &mut stars,
                    labels,
                    look,
                    images,
                    origin,
                    3,
                    &yes_word,
                    at::YES,
                    lit && *yes,
                    ticks,
                );
                row(
                    &mut out,
                    &mut stars,
                    labels,
                    look,
                    images,
                    origin,
                    3,
                    &no_word,
                    at::NO,
                    lit && !*yes,
                    ticks,
                );
                continue;
            }
            Column::Pick {
                names,
                at: chosen,
                first,
            } => {
                header(&mut out, look, labels, images, (player, big), origin);
                for (y, index, plain) in [(at::ENTER, 27, "Select"), (at::NAME, 28, "Name")] {
                    let s = line(look, "entername.txt", index, plain);
                    text(
                        &mut out,
                        look,
                        labels,
                        images,
                        3,
                        &s,
                        LABEL,
                        at(MIDDLE, y),
                        Align::Centre,
                    );
                }
                let new = line(look, "entername.txt", 26, "Enter New");
                let rows = std::iter::once(new.as_str()).chain(names.iter().map(String::as_str));
                for (r, name) in rows.enumerate().skip(*first).take(at::LIST_ROWS) {
                    let y = list_row(r - first);
                    let colour = if lit && r == *chosen {
                        colours::ROW_LIT
                    } else {
                        colours::ROW
                    };
                    text(
                        &mut out,
                        look,
                        labels,
                        images,
                        fonts::SMALL,
                        name,
                        colour,
                        at(MIDDLE, y),
                        Align::Centre,
                    );
                }
                continue;
            }
            Column::Typing(t) => {
                header(&mut out, look, labels, images, (player, big), origin);
                for (y, index, plain) in [(at::ENTER, 10, "Enter"), (at::NAME, 11, "Name")] {
                    let s = line(look, "entername.txt", index, plain);
                    text(
                        &mut out,
                        look,
                        labels,
                        images,
                        3,
                        &s,
                        LABEL,
                        at(MIDDLE, y),
                        Align::Centre,
                    );
                }
                let shown = if t.text.is_empty() {
                    default_name(Some(look), player)
                } else {
                    t.text.to_owned()
                };
                keyboard(
                    &mut out,
                    labels,
                    images,
                    look,
                    at(KEYBOARD_AT.x, KEYBOARD_AT.y),
                    &shown,
                    t.key,
                );
                continue;
            }
            Column::Guest => {
                // Elements 0x1e (cream since `FUN_00243730`) and 0x1f.
                let none = line(look, "entername.txt", 23, "No Name\\nEntered");
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    fonts::LIST,
                    &none,
                    colours::CREAM,
                    at(MIDDLE, 45.0),
                    Align::Centre,
                );
                let lost = line(look, "entername.txt", 24, "User stats\\ncannot\\nbe saved.");
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    fonts::SMALL,
                    &lost,
                    LABEL,
                    at(MIDDLE, 146.0),
                    Align::Centre,
                );
            }
            Column::Named(u) => {
                // Elements 0x20, 0x22, 0x23/0x24, then the career rows.
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    fonts::SMALL,
                    &u.name,
                    colours::HELP,
                    at(MIDDLE, 12.0),
                    Align::Centre,
                );
                let record = line(look, "entername.txt", 29, "%1dW - %2dL")
                    .replace("%1d", &u.wins.to_string())
                    .replace("%2d", &u.losses.to_string());
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    fonts::LIST_SMALL,
                    &record,
                    colours::CREAM,
                    at(MIDDLE, 28.0),
                    Align::Centre,
                );
                let win = line(look, "entername.txt", 13, "WIN%");
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    fonts::LIST_SMALL,
                    &win,
                    colours::CREAM,
                    at(10.0, 48.0),
                    Align::Left,
                );
                let avg = average(u.wins, u.losses);
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    fonts::LIST_SMALL,
                    &avg,
                    colours::CREAM,
                    at(118.0, 48.0),
                    Align::Right,
                );
                for (r, &(label_y, index, value_y)) in CAREER_ROWS.iter().enumerate() {
                    let label = line(look, "entername.txt", index, street_sim::records::CAREER[r]);
                    text(
                        &mut out,
                        look,
                        labels,
                        images,
                        fonts::LIST_SMALL,
                        &label,
                        LABEL,
                        at(MIDDLE, label_y),
                        Align::Centre,
                    );
                    let value = count(u.career[r]);
                    text(
                        &mut out,
                        look,
                        labels,
                        images,
                        fonts::LIST_SMALL,
                        &value,
                        colours::CREAM,
                        at(MIDDLE, value_y),
                        Align::Centre,
                    );
                }
            }
            Column::Ready { side, go } => {
                header(&mut out, look, labels, images, (player, big), origin);
                let word = line(
                    look,
                    "entername.txt",
                    if *side == 1 { 40 } else { 42 },
                    if *side == 1 { "AWAY" } else { "HOME" },
                );
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    fonts::LIST,
                    &word,
                    colours::ROW_LIT,
                    at(MIDDLE, at::SIDE_WORD),
                    Align::Centre,
                );
                let ready = line(look, "entername.txt", 25, "Ready");
                text(
                    &mut out,
                    look,
                    labels,
                    images,
                    big,
                    &ready,
                    colours::CREAM,
                    at(MIDDLE, at::READY),
                    Align::Centre,
                );
                if !*go {
                    continue;
                }
            }
        }
        // The foot (elements 0x44 and 0x43): the button, 0.8 of its size,
        // and "Continue" level with its middle.
        let size = CROSS.size() * 0.8;
        let icon = at(at::FOOT.0, at::FOOT.1);
        if let Some([buttons, _]) = &look.pad {
            out.push(Piece::part(
                buttons.clone(),
                CROSS,
                Rect::from_corners(icon, icon + size),
            ));
        }
        let go = line(look, "entername.txt", 22, "Continue");
        // A text button: 0x88a0be, lit in the colour `FUN_0021ce90` gives
        // it, cream (`FUN_0021c590` sets the pair, `FUN_0021c9e0` lights).
        let colour = if lit { colours::CREAM } else { LABEL };
        let line_h = look
            .fonts
            .get(fonts::SMALL)
            .map_or(18.0, |f| f.line_height as f32);
        text(
            &mut out,
            look,
            labels,
            images,
            fonts::SMALL,
            &go,
            colour,
            icon + Vec2::new(size.x + 4.0, (size.y - line_h) / 2.0),
            Align::Left,
        );
    }
    out.extend(stars);
    out
}

/// The on-screen keyboard (`FUN_0023c7b8`, the column's element 0x1d at
/// `at`): the name so far at (59, 4) in orange, the letters in font 7,
/// SPC, DEL and DONE in font 0, each key a text button centred on its
/// place. Keys are 0x88a0be; the one under the cursor is lit orange and
/// grown (`FUN_0021c590` sets the pair, `FUN_0021c9e0` lights it, at the
/// size `FUN_0021ce48` gives: 1.5 for letters, 1.25 for the rest).
pub(crate) fn keyboard(
    out: &mut Vec<Piece>,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    look: &Look,
    at: Vec2,
    typed: &str,
    key: (usize, usize),
) {
    // Centred on its place too (the line's 0x20 flag, `FUN_0024a4e8`).
    let line_h = look
        .fonts
        .get(fonts::SMALL)
        .map_or(18.0, |f| f.line_height as f32);
    text(
        out,
        look,
        labels,
        images,
        fonts::SMALL,
        typed,
        colours::HELP,
        at + Vec2::new(59.0, 4.0 - line_h / 2.0),
        Align::Centre,
    );
    for (r, row) in crate::typing::NAME_KEYBOARD.iter().enumerate() {
        for (c, k) in row.iter().enumerate() {
            let (font, grown, label) = match *k {
                "SPC" => (fonts::SMALL, 1.25, line(look, "entername.txt", 6, "SPC")),
                "DEL" => (fonts::SMALL, 1.25, line(look, "entername.txt", 7, "DEL")),
                "DONE" => (fonts::SMALL, 1.25, line(look, "entername.txt", 9, "DONE")),
                k => (7, 1.5, k.to_owned()),
            };
            let lit = key == (r, c);
            let (colour, scale) = if lit {
                (colours::HELP, grown)
            } else {
                (LABEL, 1.0)
            };
            let middle = at + key_middle(r, c);
            let line_h = look.fonts.get(font).map_or(18.0, |f| f.line_height as f32);
            let from = out.len();
            text(
                out,
                look,
                labels,
                images,
                font,
                &label,
                colour,
                middle - Vec2::new(0.0, line_h / 2.0),
                Align::Centre,
            );
            // Grown about the key's middle.
            for p in &mut out[from..] {
                let half = p.rect.half_size() * scale;
                let centre = middle + (p.rect.center() - middle) * scale;
                p.rect = Rect::from_center_half_size(centre, half);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win_percentage_reads_as_the_game_prints_it() {
        assert_eq!(average(0, 0), ".000");
        assert_eq!(average(3, 0), "1.000");
        assert_eq!(average(2, 1), ".667");
        assert_eq!(average(1, 2), ".333");
        assert_eq!(average(0, 4), ".000");
        assert_eq!(average(u32::MAX, u32::MAX), ".500");
    }

    #[test]
    fn counts_are_grouped_by_thousands() {
        assert_eq!(count(0), "0");
        assert_eq!(count(999), "999");
        assert_eq!(count(1000), "1,000");
        assert_eq!(count(1_234_567), "1,234,567");
    }

    #[test]
    fn the_keyboard_fits_its_column() {
        for (r, row) in crate::typing::NAME_KEYBOARD.iter().enumerate() {
            for c in 0..row.len() {
                let m = KEYBOARD_AT + key_middle(r, c);
                assert!(m.x > 4.0 && m.x < WIDTH - 4.0, "{r},{c}");
                assert!(m.y > 0.0 && m.y < HEIGHT - 4.0, "{r},{c}");
            }
        }
    }

    /// The side chooser's pieces as `FUN_002401b8` places them and
    /// `FUN_002445c0` moves them: rows, arrows and icon keep their order
    /// top to bottom, and a chosen side pushes the icon past its row while
    /// the arrows go.
    #[test]
    fn the_side_chooser_keeps_its_order_and_moves_the_icon_past_the_chosen_row() {
        let (away, home, pad) = chooser_layout(None);
        assert!(away < at::ARROW_UP && at::ARROW_UP < pad);
        assert!(pad < at::ARROW_DOWN && at::ARROW_DOWN < home);
        let (away, home, pad) = chooser_layout(Some(1));
        assert!(pad < away && away < home);
        let (away, home, pad) = chooser_layout(Some(0));
        assert!(away < pad && pad < home);
        assert!(home + 20.0 < HEIGHT);
        // The two end rows keep 140 between them whichever is picked.
        assert_eq!(home - chooser_layout(Some(1)).0, 140.0);
        assert_eq!(at::PAD_LEFT * 2.0, 50.0);
    }

    #[test]
    fn the_prompt_and_list_rows_sit_where_the_constructor_puts_them() {
        // Yes at 18 + 21 + 37 + 75, No 34 below.
        assert_eq!(at::YES, 18.0 + 21.0 + 37.0 + 75.0);
        assert_eq!(at::NO, at::YES + 34.0);
        assert_eq!(at::MESSAGE, 18.0 + 21.0 + 37.0);
        // Five names fit under "Select Name" and above the foot.
        assert_eq!(list_row(0), 155.0);
        assert_eq!(list_row(4), 235.0);
        assert!(list_row(at::LIST_ROWS - 1) + at::LIST_STEP <= at::FOOT.1);
        assert!(list_row(0) > at::NAME);
    }

    #[test]
    fn the_default_name_numbers_the_player() {
        assert_eq!(default_name(None, 0), "PLAYER1");
        assert_eq!(default_name(None, 3), "PLAYER4");
    }
}
