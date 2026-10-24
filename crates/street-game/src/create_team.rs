//! Create Team (`FUN_0027dd80(screen, mode)`, layout blob 24, backdrop 9):
//! class 0x35 is My Team's (mode 0: five players from four categories)
//! and class 0x62 a new career's crew (mode 1: two of the ten local
//! ballers beside the career's baller). The build is `FUN_0027e458`, the
//! categories' lists `FUN_0027f430`, the grid's paging and the slots
//! `FUN_00280040`, the read-out `FUN_0027fc48`, the locals' draw
//! `FUN_001ed1f8`. The trace is in `docs/FRONTEND_RECOVERY.md`, "Create
//! Team and the crew".
//!
//! The left panel shows the category ("League Players", "Street Legends",
//! "Legends", "Created Ballers"; L1 and R1 turn through those not
//! empty; the crew's one "Local Ballers") over two rows of five small
//! portrait tiles, a page of the category's list scrolled a row at a time
//! (League Players a team a row, its name over it); a player already picked
//! dims to 0x7f. The right panel reads out the focused player: the names
//! on two lines, the height and weight on two right-aligned, and the seven
//! meters. Under them the team's five slots take the picks in order, the
//! empty ones open; the crew's first slot holds the career's baller and
//! the last two are shut (0x7f).

use bevy::prelude::*;
use street_assets::font::Layout;

use crate::{
    choose::{self, Tile},
    look::{self, Align, Look, Piece, colours, fonts},
    portraits::Portraits,
    session::Libc,
    setup::Disc,
};

pub const CLASS: u32 = 53;
pub const COLUMNS: usize = 5;
pub const ROWS_SHOWN: usize = 2;
/// The grid's tiles (elements 10..19, small) and the team's slots (20..24).
const GRID: [u32; 10] = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19];
const SLOTS: [u32; 5] = [20, 21, 22, 23, 24];
/// The read-out: the names (9), the size (30) and the meters, Shots ..
/// Rebounds (`FUN_0027fc48`: 0x1d, 2, 0x19, 0, 0x1f, 3, 0x1b).
const NAME: u32 = 9;
const SIZE: u32 = 30;
const METERS: [u32; 7] = [29, 2, 25, 0, 31, 3, 27];
/// The rows' team names, the category's node, the side arrows and the
/// L1 and R1 icons (frames 5 and 6), the scroll arrows.
const ROW_NAMES: [u32; 2] = [32, 33];
const CATEGORY_NODE: u32 = 34;
const SIDE_ARROWS: [u32; 2] = [6, 26];
const ICONS: [(u32, usize); 2] = [(7, 5), (28, 6)];
const UP: u32 = 35;
const DOWN: u32 = 1;
/// A tile already picked, and a shut slot (`FUN_00280040`: 0x7f).
const DIM: f32 = 0x7f as f32 / 255.0;

/// Which Create Team.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// My Team's (class 0x35): five picks.
    #[default]
    MyTeam,
    /// A new career's crew (class 0x62): two picks.
    Crew,
}

impl Mode {
    /// The slots to fill (`+0x1f0`).
    pub fn picks(self) -> usize {
        match self {
            Mode::MyTeam => 5,
            Mode::Crew => 2,
        }
    }
}

/// A category: its title (`createteam.txt`), its players, and for league
/// Players each row's team.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Category {
    pub title: usize,
    pub players: Vec<usize>,
    pub rows: Vec<String>,
}

/// My Team's categories in the order L1 and R1 turn through them
/// (`FUN_00280040`: 0 League Players, 1 Street Legends, 2 Legends, 3
/// Created Ballers): each league team's first five (`FUN_001ecd00`), the
/// streetballers 0x91..=0x9d and the legends 0x9e..=0xb7 unlocked
/// (`FUN_001d9ca0`), the created ballers (`FUN_0027f430`).
pub fn categories(
    disc: &Disc,
    unlocked: &dyn Fn(usize) -> bool,
    created: &[usize],
) -> Vec<Category> {
    let mut league = Category {
        title: 2,
        ..default()
    };
    for team in disc.teams.iter().take(29) {
        league.players.extend(team.roster.iter().take(COLUMNS));
        league.rows.push(team.full_name());
    }
    vec![
        league,
        Category {
            title: 4,
            players: (0x91..=0x9d).filter(|&r| unlocked(r)).collect(),
            rows: Vec::new(),
        },
        Category {
            title: 3,
            players: (0x9e..=0xb7).filter(|&r| unlocked(r)).collect(),
            rows: Vec::new(),
        },
        Category {
            title: 5,
            players: created.to_vec(),
            rows: Vec::new(),
        },
    ]
}

/// The ten local ballers (`FUN_001ed1f8`): the generic ballers 0xb8..0xe9
/// not taken, men and women shuffled apart (`rand() % (k + 1)`), then ten
/// drawn from the ends, one in twelve of the first eight a woman
/// (`rand() % 12 == 0`) and the last two women while any are left, and
/// the ten shuffled.
pub fn locals(disc: &Disc, libc: &mut Libc, taken: &dyn Fn(usize) -> bool) -> Vec<usize> {
    let shuffle = |list: &mut Vec<usize>, libc: &mut Libc| {
        for k in 1..list.len() {
            let j = libc.rand() as usize % (k + 1);
            list.swap(k, j);
        }
    };
    let (mut men, mut women): (Vec<usize>, Vec<usize>) = (0xb8..0xea)
        .filter(|&r| r < disc.players.len() && !taken(r))
        .partition(|&r| !disc.players[r].female);
    shuffle(&mut men, libc);
    shuffle(&mut women, libc);
    let mut out = Vec::new();
    for k in 0..10 {
        let woman = if k < 8 {
            libc.rand().is_multiple_of(12) && !women.is_empty()
        } else {
            !women.is_empty()
        };
        if woman {
            out.extend(women.pop());
        } else if let Some(m) = men.pop() {
            out.push(m);
        } else {
            out.extend(women.pop());
        }
    }
    shuffle(&mut out, libc);
    out
}

/// The grid's place and the picks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picker {
    pub category: usize,
    /// The first row shown (`+0x150`), the focused column and row
    /// (`+0x198`, `+0x19a`).
    pub first: usize,
    pub col: usize,
    pub row: usize,
    pub picks: Vec<usize>,
}

impl Picker {
    /// The focused player's index in the category's list.
    pub fn at(&self) -> usize {
        (self.first + self.row) * COLUMNS + self.col
    }

    /// A step (`FUN_00280040`): across the row, down and up a row, the
    /// window scrolling a row past either end; a step onto the last row's
    /// gap comes to its last player.
    pub fn step(&mut self, dx: i32, dy: i32, len: usize) {
        if len == 0 {
            return;
        }
        let col = (self.col as i32 + dx).clamp(0, COLUMNS as i32 - 1) as usize;
        let mut row = self.row as i32 + dy;
        let mut first = self.first;
        if row < 0 {
            if first == 0 {
                return;
            }
            first -= 1;
            row = 0;
        }
        if row >= ROWS_SHOWN as i32 {
            if (first + ROWS_SHOWN) * COLUMNS >= len {
                return;
            }
            first += 1;
            row = ROWS_SHOWN as i32 - 1;
        }
        let row = row as usize;
        let start = (first + row) * COLUMNS;
        if start >= len {
            return;
        }
        self.first = first;
        self.row = row;
        self.col = col.min(len - 1 - start);
    }

    /// Cross on a player: into the next open slot, unless picked already or
    /// the slots are full.
    pub fn pick(&mut self, record: usize, mode: Mode) -> bool {
        if self.picks.contains(&record) || self.picks.len() >= mode.picks() {
            return false;
        }
        self.picks.push(record);
        true
    }

    /// Triangle: the last pick back; none to take back is false.
    pub fn undo(&mut self) -> bool {
        self.picks.pop().is_some()
    }

    /// L1 (-1) or R1 (+1): the next category with players, the grid back
    /// at its start.
    pub fn turn(&mut self, dir: i32, categories: &[Category]) {
        let n = categories.len() as i32;
        if n == 0 {
            return;
        }
        let mut c = self.category as i32;
        for _ in 0..n {
            c = (c + dir).rem_euclid(n);
            if !categories[c as usize].players.is_empty() {
                break;
            }
        }
        if c as usize != self.category {
            self.category = c as usize;
            self.first = 0;
            self.col = 0;
            self.row = 0;
        }
    }
}

/// What the screen shows.
pub struct View<'a> {
    pub mode: Mode,
    pub category: &'a Category,
    pub picker: &'a Picker,
    /// The crew's baller, in the first slot.
    pub baller: Option<usize>,
}

fn place_lines(
    out: &mut Vec<Piece>,
    labels: &mut look::Labels,
    look: &Look,
    images: &mut Assets<Image>,
    font: usize,
    lines: &[&str],
    colour: [u8; 4],
    at: Vec2,
    align: Align,
) {
    let line = look.fonts.get(font).map_or(20.0, |f| f.line_height as f32);
    for (k, s) in lines.iter().enumerate() {
        if let Some(p) = labels.text(
            look,
            images,
            font,
            s,
            colour,
            None,
            at + Vec2::new(0.0, line * k as f32),
            align,
        ) {
            out.push(p);
        }
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
    let p = view.picker;
    let list = &view.category.players;
    let spot = |id: u32| choose::element(layout, id).and_then(choose::spot);
    // The category's row: My Team's in 0xfbac00 between L1 and R1, the
    // crew's in the row widget's idle 0x88a0be (`FUN_0027e458`).
    if let Some(at) = spot(CATEGORY_NODE) {
        let title = look
            .text("createteam.txt", view.category.title)
            .unwrap_or("");
        let colour = if view.mode == Mode::Crew {
            colours::ROW
        } else {
            colours::ROW_LIT
        };
        let height = look
            .fonts
            .get(fonts::LIST)
            .map_or(20.0, |f| f.text_height as f32);
        // The row widget's text sits with its middle on the node and 13
        // under it (measured against `bal_crew.png`).
        place_lines(
            &mut out,
            labels,
            look,
            images,
            fonts::LIST,
            &[title],
            colour,
            at + Vec2::new(0.0, 13.0 - height / 2.0),
            Align::Centre,
        );
    }
    if view.mode == Mode::MyTeam {
        for (k, &id) in SIDE_ARROWS.iter().enumerate() {
            if let Some(at) = spot(id) {
                let image = if k == 0 {
                    &pm.arrow_left
                } else {
                    &pm.arrow_right
                };
                out.push(Piece::new(
                    image.clone(),
                    Rect::from_center_size(at, pm.arrow_side_size),
                ));
            }
        }
        if let Some((atlas, frames)) = &look.icons {
            for (id, frame) in ICONS {
                if let (Some(at), Some(icon)) = (spot(id), frames.get(frame)) {
                    out.push(Piece::part(
                        atlas.clone(),
                        *icon,
                        Rect::from_center_size(at, icon.size()),
                    ));
                }
            }
        }
    }
    // The rows' team names (League Players only).
    for (k, &id) in ROW_NAMES.iter().enumerate() {
        if let (Some(e), Some(name)) = (
            choose::element(layout, id),
            view.category.rows.get(p.first + k),
        ) && let Some(at) = choose::spot(e)
        {
            let colour = e
                .props
                .get(&10)
                .map_or(colours::CREAM, |&c| street_assets::font::argb(c));
            place_lines(
                &mut out,
                labels,
                look,
                images,
                fonts::LIST,
                &[name],
                colour,
                at,
                Align::Left,
            );
        }
    }
    // The grid: a page of two rows, picked players dimmed.
    let mut cursor = None;
    for (k, &id) in GRID.iter().enumerate() {
        let index = p.first * COLUMNS + k;
        let (Some(at), Some(&record)) = (spot(id), list.get(index)) else {
            continue;
        };
        let bright = if p.picks.contains(&record) { DIM } else { 1.0 };
        choose::tile_pieces(
            &mut out,
            images,
            portraits.as_deref_mut(),
            pm,
            disc,
            at,
            (Tile::Player(record), true),
            bright,
        );
        if index == p.at() {
            cursor = Some(at);
        }
    }
    if let Some(at) = cursor {
        let mut c = Piece::new(
            pm.cursor.clone(),
            Rect::from_center_size(at, pm.tile_small_size),
        );
        c.colour = Color::WHITE.with_alpha(f32::from(look::row_pulse(ticks)) / 255.0);
        out.push(c);
    }
    // The scroll arrows past the page.
    let half = pm.arrow_size / 2.0;
    if p.first > 0
        && let Some(at) = spot(UP)
    {
        out.push(Piece::new(
            pm.arrow_up.clone(),
            Rect::from_corners(at - half, at + half),
        ));
    }
    if (p.first + ROWS_SHOWN) * COLUMNS < list.len()
        && let Some(at) = spot(DOWN)
    {
        out.push(Piece::new(
            pm.arrow_down.clone(),
            Rect::from_corners(at - half, at + half),
        ));
    }
    // The slots: the picks, then the open ones; the crew's baller first and
    // its last two shut.
    let mut slots: Vec<(Tile, f32)> = Vec::new();
    if let Some(baller) = view.baller {
        slots.push((Tile::Player(baller), DIM));
    }
    for k in 0..view.mode.picks() {
        slots.push((
            p.picks.get(k).map_or(Tile::Empty, |&r| Tile::Player(r)),
            1.0,
        ));
    }
    while slots.len() < SLOTS.len() {
        slots.push((Tile::Empty, DIM));
    }
    for (k, &id) in SLOTS.iter().enumerate() {
        if let (Some(at), Some(&(tile, bright))) = (spot(id), slots.get(k)) {
            choose::tile_pieces(
                &mut out,
                images,
                portraits.as_deref_mut(),
                pm,
                disc,
                at,
                (tile, false),
                bright,
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
        list.get(p.at()).copied(),
    );
    out
}

/// The focused player's read-out (`FUN_0027fc48`): "%1s\n%2s" of the names
/// in font 2 on element 9; the height and weight, right-aligned, on 30; the
/// meters (kind 20, anchored on their middle) beside the layout's words.
#[allow(clippy::too_many_arguments)]
fn readout(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    layout: &Layout,
    pm: &look::PrematchArt,
    disc: &Disc,
    record: Option<usize>,
) {
    let player = record.and_then(|r| disc.players.get(r));
    if let (Some(e), Some(p)) = (choose::element(layout, NAME), player)
        && let Some(at) = choose::spot(e)
    {
        let font = e.props.get(&13).map_or(fonts::LIST, |&f| f as usize);
        place_lines(
            out,
            labels,
            look,
            images,
            font,
            &[&p.first_name, &p.last_name],
            colours::HELP,
            at,
            Align::Left,
        );
    }
    if let (Some(e), Some(p)) = (choose::element(layout, SIZE), player)
        && let Some(at) = choose::spot(e)
    {
        let font = e.props.get(&13).map_or(fonts::SMALL, |&f| f as usize);
        let height = format!("{}'{}\"", p.height_in / 12, p.height_in % 12);
        let weight = format!("{} lbs", p.weight_lb);
        place_lines(
            out,
            labels,
            look,
            images,
            font,
            &[&height, &weight],
            colours::HELP,
            at,
            Align::Right,
        );
    }
    for (k, &id) in METERS.iter().enumerate() {
        if let Some(e) = choose::element(layout, id)
            && let Some(at) = choose::spot(e)
        {
            let h = e.size().map_or(0.0, |(_, h)| f32::from(h));
            let skill = crate::choose::SKILLS[k];
            let level = player.map_or(0, |p| {
                crate::progression_trace::skill_bar(skill, p.ratings[skill.rating()])
            });
            out.extend(crate::chooseteam::meter(
                pm,
                at - Vec2::new(0.0, h / 2.0),
                level,
            ));
        }
    }
    // The meters' words: the layout's own texts (font 0, 0xff98a8b8).
    for e in layout
        .elements
        .iter()
        .filter(|e| e.kind == 4 && e.id == 0xffff)
    {
        let (Some(at), Some(word)) = (
            choose::spot(e),
            e.props.get(&14).and_then(|&id| look.text_id(id as u16)),
        ) else {
            continue;
        };
        let colour = e
            .props
            .get(&10)
            .map_or(colours::LEGEND, |&c| street_assets::font::argb(c));
        let font = e.props.get(&13).map_or(fonts::SMALL, |&f| f as usize);
        choose::place(out, labels, look, images, font, word, colour, at, e.align);
    }
}

/// What Create Team shows over its grid (`FUN_002810d0`'s states, each a
/// dialog of its own): the crew's Welcome board before its picks (state 5,
/// `FUN_0024c2a8`); after the last pick the city's keyboard (state 0,
/// `FUN_0024d8a8(.., 0x1d10, .., 0x3ed)`), the name's (state 1, 0x1d11,
/// 0x3ee) and Choose Logo (state 2, `FUN_0024cac0(.., 0x3ef)`); a dialog's
/// answer comes back through `FUN_00280cd0` (the next state) or, cancelled,
/// `FUN_00280f98` (the one before); "Cancel and Return to Choose Baller?"
/// or "...to My Team?" (state 3, 0x1d16, 0x1d17) when Triangle has no pick
/// to take back. After Choose Logo (its answer 0x3ef sets `+0x1d8`) the
/// update (`FUN_002808c0`) hands My Team's team to the memory card widget,
/// whose own question follows ([`CardAsk`], `Save` here); the crew's is
/// made and saved at once. State 4, "Do you want to save your team?"
/// (0x1d12, `FUN_00280cd0`'s 0x3f3), has its answer's code but nothing
/// opens it: the runtime goes from Choose Logo straight to the card's
/// "Create team ...?".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Welcome,
    Pick,
    City,
    Name,
    Logo,
    Cancel,
    Save,
}

/// The cancel question's words and rows (`createteam.txt` 22 or 23; the
/// rows `common.txt` 2 and 3), and the row lit when one opens
/// (`FUN_0025cda0(dialog, 1)`: No).
pub fn question(look: &Look, stage: Stage, mode: Mode) -> Option<(String, Vec<String>)> {
    let text = |file: &str, k: usize, or: &str| look.text(file, k).unwrap_or(or).to_owned();
    let rows = vec![text("common.txt", 2, "Yes"), text("common.txt", 3, "No")];
    let words = match (stage, mode) {
        (Stage::Cancel, Mode::Crew) => text(
            "createteam.txt",
            22,
            "Cancel and Return\\nto Choose Baller?",
        ),
        (Stage::Cancel, Mode::MyTeam) => {
            text("createteam.txt", 23, "Cancel and Return\\nto My Team?")
        }
        _ => return None,
    };
    Some((words, rows))
}

/// The row a question opens on: No.
pub const QUESTION_ROW: usize = 1;

/// The memory card widget's question over My Team's Create Team
/// (`FUN_002272e8` copies the team list and builds dialog 0xcd; the
/// answers are `FUN_00227348`'s cases 5, 6 and 0xc), in the card's own box
/// ([`crate::card::box_pieces`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardAsk {
    /// 0xcd: "Create team\n%1s?" (`delete.txt` 10), YES and NO, YES lit.
    Create,
    /// 0xce: "Overwrite existing\ncreated team\n%1s?" (11), YES and NO,
    /// NO lit: a created team has the name.
    Overwrite(usize),
    /// 0xd4: "Only 10 teams can be created. ..." (3) over Continue
    /// (`common.txt` 20): the ten slots are used (`FUN_001edf70`).
    Full,
}

/// What an answer to [`CardAsk`] does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardNext {
    /// Another of the widget's questions.
    Ask(CardAsk),
    /// The team made (`FUN_001ebbc8`), in a new slot or over the one of
    /// its name, and the card's save (`+0x16c` with 0x3f).
    Make(Option<usize>),
    /// The widget closes unsaved (`+0x154` with 0): the grid again, the
    /// last pick taken back (`FUN_002808c0`'s `+0xfc`).
    Close,
}

impl CardAsk {
    /// Its words, `%1s` the team's whole name ("NEW YORK BALLERS").
    pub fn words(self, look: &Look, team: &str) -> String {
        let (k, or) = match self {
            CardAsk::Create => (10, "Create team\\n%1s?"),
            CardAsk::Overwrite(_) => (11, "Overwrite existing\\ncreated team\\n%1s?"),
            CardAsk::Full => (
                3,
                "Only 10 teams can be created.\\nPlease delete a created team\\nto make room for the new one.",
            ),
        };
        look.text("delete.txt", k)
            .unwrap_or(or)
            .replace("%1s", team)
    }

    /// Its rows: YES and NO (`memcard_ps2.txt` 5, 6), or Continue.
    pub fn rows(self, look: &Look) -> Vec<String> {
        let word = |file: &str, k: usize, or: &str| look.text(file, k).unwrap_or(or).to_owned();
        match self {
            CardAsk::Full => vec![word("common.txt", 20, "Continue")],
            _ => vec![
                word("memcard_ps2.txt", 5, "YES"),
                word("memcard_ps2.txt", 6, "NO"),
            ],
        }
    }

    /// The row lit as it opens (`FUN_0025a880`: 0 for 0xcd, 1 for 0xce).
    pub fn first_row(self) -> usize {
        usize::from(matches!(self, CardAsk::Overwrite(_)))
    }

    /// Cross on `row` (0 YES), the team list as it stands.
    pub fn answer(
        self,
        row: usize,
        teams: &[street_sim::story::CreatedTeam],
        team: &str,
    ) -> CardNext {
        let yes = row == 0;
        match self {
            // `FUN_001edfb8` looks for the name first, then `FUN_001edf70`
            // for a free slot.
            CardAsk::Create if yes => match teams.iter().position(|t| full_name(t) == team) {
                Some(k) => CardNext::Ask(CardAsk::Overwrite(k)),
                None if teams.len() >= street_sim::story::TEAM_SLOTS => {
                    CardNext::Ask(CardAsk::Full)
                }
                None => CardNext::Make(None),
            },
            CardAsk::Overwrite(k) if yes => CardNext::Make(Some(k)),
            CardAsk::Overwrite(_) => CardNext::Ask(CardAsk::Create),
            CardAsk::Create | CardAsk::Full => CardNext::Close,
        }
    }
}

/// A created team's whole name, as its list and the card's questions show
/// it.
pub fn full_name(t: &street_sim::story::CreatedTeam) -> String {
    format!("{} {}", t.city, t.name)
}

/// The ten logos Choose Logo offers (`FUN_0024cac0`: `XATEAMS` images
/// 0x1d..0x26), five a row.
pub const LOGOS: std::ops::Range<u8> = 0x1d..0x27;
const LOGO_COLUMNS: usize = 5;

/// A step on Choose Logo's grid (`FUN_00222608`, `FUN_00222700`: each
/// tile links left and right within its row and up and down to the other
/// row, the edges stopping).
pub fn logo_step(cursor: usize, dx: i32, dy: i32) -> usize {
    let (col, row) = (
        (cursor % LOGO_COLUMNS) as i32,
        (cursor / LOGO_COLUMNS) as i32,
    );
    let (col, row) = ((col + dx).clamp(0, 4), (row + dy).clamp(0, 1));
    row as usize * LOGO_COLUMNS + col as usize
}

/// The dialogs' place (`DAT_003f7170`).
const DIALOG_AT: Vec2 = crate::pause::DIALOG_AT;
/// Where the Welcome board's words and row are centred: measured on
/// `bal_crew_welcome.png`, 10 below the dialogs' place (the board is placed
/// on `DAT_003f7170` with its title, `FUN_0024a530(.., 1)`).
const WELCOME_AT: Vec2 = Vec2::new(320.0, 250.0);
/// The rule under a keyboard's or Choose Logo's heading (`FUN_002332d0`,
/// 0xff424a9c, 4 tall; 16 narrower than the frame).
const RULE: [u8; 4] = [0x42, 0x4a, 0x9c, 0xff];
/// The keyboard widget's size (`FUN_0023c7b8`, measured on
/// `bal_crew_city.png`: the keys from its corner as `users::keyboard`
/// places them).
const KEYBOARD_SIZE: Vec2 = Vec2::new(116.0, 128.0);

/// A dialog's frame from its children's boxes and their margins
/// (`FUN_00212008(margin, frame, child)`).
fn grow(frame: &mut Option<Rect>, child: Rect, margin: f32) {
    let r = Rect::from_corners(child.min - margin, child.max + margin);
    *frame = Some(frame.map_or(r, |f| f.union(r)));
}

fn frame_piece(
    frame: Rect,
    gm: &crate::boards::Gm,
    panels: &mut crate::boards::Panels,
    images: &mut Assets<Image>,
) -> Piece {
    panels.get(
        [frame.min.x, frame.min.y, frame.width(), frame.height()],
        0xff,
        gm,
        images,
    )
}

/// What sits under a dialog: black at 0x40 of 0x80 (`FUN_00233e00(rect,
/// 0x40000000)`), as the GS darkens.
fn dim(gm: &crate::boards::Gm) -> Piece {
    let mut p = Piece::new(gm.white.clone(), Rect::new(0.0, 0.0, 640.0, 480.0));
    p.colour = Color::srgba(0.0, 0.0, 0.0, look::black_alpha(0.5));
    p
}

/// The rule under a heading, `width` wide, its top at `y`.
fn rule(gm: &crate::boards::Gm, width: f32, y: f32) -> Piece {
    let mut line = Piece::new(
        gm.white.clone(),
        Rect::new(
            DIALOG_AT.x - width / 2.0,
            y,
            DIALOG_AT.x + width / 2.0,
            y + 4.0,
        ),
    );
    line.colour = Color::srgba_u8(RULE[0], RULE[1], RULE[2], RULE[3]);
    line
}

/// Breaks `text` into lines no wider than `width` in `font`.
fn wrap(look: &Look, font: usize, text: &str, width: f32) -> Vec<String> {
    let Some(f) = look.fonts.get(font) else {
        return vec![text.to_owned()];
    };
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let next = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if f.measure(&next) as f32 > width && !line.is_empty() {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The crew's Welcome board (`FUN_0024c2a8`): "Welcome" as a title on the
/// frame's corner, 15 above it; the words (`createteam.txt` 24, font 2) in
/// a block 420 wide; "Continue" (`common.txt` 20, font 2) 10 under them,
/// lit with its stars; the frame 40 round both, over the screen dimmed.
pub fn welcome_pieces(
    look: &Look,
    gm: &crate::boards::Gm,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    panels: &mut crate::boards::Panels,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = vec![dim(gm)];
    let font = fonts::LIST;
    let Some(f) = look.fonts.get(font) else {
        return out;
    };
    let (lh, th) = (f.line_height as f32, f.text_height as f32);
    let words = look
        .text("createteam.txt", 24)
        .unwrap_or("Pick a couple of the local ballers to be your teammates and head to the Rec Center for your first game. Your legend starts now.");
    let lines = wrap(look, font, words, 420.0);
    let text_h = th + lh * lines.len().saturating_sub(1) as f32;
    let row_h = th;
    let top = WELCOME_AT.y - (text_h + row_h + 10.0) / 2.0;
    let block = Rect::new(
        WELCOME_AT.x - 210.0,
        top,
        WELCOME_AT.x + 210.0,
        top + text_h,
    );
    let row_top = top + text_h + 10.0;
    let row = Rect::new(
        WELCOME_AT.x - 60.0,
        row_top,
        WELCOME_AT.x + 60.0,
        row_top + row_h,
    );
    let mut frame = None;
    grow(&mut frame, block, 40.0);
    grow(&mut frame, row, 40.0);
    let Some(frame) = frame else { return out };
    out.push(frame_piece(frame, gm, panels, images));
    for (k, line) in lines.iter().enumerate() {
        let at = Vec2::new(block.min.x, top + lh * k as f32);
        if let Some(p) = labels.text(look, images, font, line, [0xff; 4], None, at, Align::Left) {
            out.push(p);
        }
    }
    let mut stars = Vec::new();
    let spot = look::RowSpot {
        at: Vec2::new(WELCOME_AT.x, row_top),
        font,
        align: Align::Centre,
        valign: 0,
    };
    let cont = look.text("common.txt", 20).unwrap_or("Continue");
    look::screen_row(
        &mut out, &mut stars, labels, look, images, spot, cont, true, true, ticks,
    );
    out.extend(stars);
    let title = look.text("createteam.txt", 25).unwrap_or("Welcome");
    look::title(
        &mut out,
        labels,
        look,
        images,
        fonts::TITLE,
        title,
        colours::COOL,
        Vec2::new(frame.min.x, frame.min.y - 15.0),
        // Anchored by its bottom (`FUN_0024a4e8(title, 0x40)`).
        8,
    );
    out
}

/// A keyboard dialog (`FUN_0024d8a8`): the heading (font 2, 0xffdcdcbb), a
/// rule 10 under it, the keyboard (`FUN_0023c7b8`, `users::keyboard`) 15
/// under that, the three stacked about the dialog's place; the frame 18
/// round the heading, 8 round the rule and 40 round the keyboard; the
/// rule 16 narrower than the frame.
#[allow(clippy::too_many_arguments)]
pub fn keyboard_pieces(
    look: &Look,
    gm: &crate::boards::Gm,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    panels: &mut crate::boards::Panels,
    heading: &str,
    typed: &str,
    key: (usize, usize),
) -> Vec<Piece> {
    let mut out = vec![dim(gm)];
    let Some(f) = look.fonts.get(fonts::LIST) else {
        return out;
    };
    let hh = f.text_height as f32;
    let hw = f.measure(heading) as f32;
    let top = DIALOG_AT.y - (4.0 + 25.0 + KEYBOARD_SIZE.y + hh) / 2.0;
    let head = Rect::new(
        DIALOG_AT.x - hw / 2.0,
        top,
        DIALOG_AT.x + hw / 2.0,
        top + hh,
    );
    let rule_y = top + hh + 10.0;
    let kbd_at = Vec2::new(DIALOG_AT.x - KEYBOARD_SIZE.x / 2.0, rule_y + 4.0 + 15.0);
    let kbd = Rect::from_corners(kbd_at, kbd_at + KEYBOARD_SIZE);
    let mut frame = None;
    grow(&mut frame, head, 18.0);
    grow(
        &mut frame,
        Rect::new(
            DIALOG_AT.x - 142.0,
            rule_y,
            DIALOG_AT.x + 142.0,
            rule_y + 4.0,
        ),
        8.0,
    );
    grow(&mut frame, kbd, 40.0);
    let Some(frame) = frame else { return out };
    out.push(frame_piece(frame, gm, panels, images));
    out.push(rule(gm, frame.width() - 16.0, rule_y));
    if let Some(p) = labels.text(
        look,
        images,
        fonts::LIST,
        heading,
        colours::CREAM,
        None,
        Vec2::new(DIALOG_AT.x, top),
        Align::Centre,
    ) {
        out.push(p);
    }
    crate::users::keyboard(&mut out, labels, images, look, kbd_at, typed, key);
    out
}

/// Choose Logo (`FUN_0024cac0`): the focused logo's large picture
/// (`XATEAMS` image 0), a rule 10 under it, two rows of five small tiles
/// 10 apart under that, the group stacked about the dialog's place; the
/// frame 40 round each, then moved 10 down and 20 shorter; the title
/// ("Choose Logo", `chooselogo.txt` 0) on its corner; the cursor tag on
/// the focused tile.
#[allow(clippy::too_many_arguments)]
pub fn logo_pieces(
    look: &Look,
    gm: &crate::boards::Gm,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    panels: &mut crate::boards::Panels,
    art: &crate::art::Art,
    cursor: usize,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = vec![dim(gm)];
    let Some(pm) = look.prematch.as_ref() else {
        return out;
    };
    let tile = pm.cursor_size;
    // The dialog's place: `DAT_003f6998`, `DAT_003f699c` and half the
    // title's height.
    let title_h = look
        .fonts
        .get(fonts::TITLE)
        .map_or(40.0, |f| f.text_height as f32);
    let at = DIALOG_AT + Vec2::new(0.0, title_h / 2.0);
    let logo = usize::from(LOGOS.start) + cursor.min(9);
    let big = art.logos_big.get(logo).and_then(|l| l.as_ref());
    let big_size = big.map_or(Vec2::new(200.0, 80.0), |b| b.1);
    let rule_h = 4.0;
    let mut y = at.y - (big_size.y + rule_h + 30.0 + 2.0 * tile.y) / 2.0;
    let big_rect = Rect::from_center_size(Vec2::new(at.x, y + big_size.y / 2.0), big_size);
    y += big_size.y + 10.0;
    let rule_y = y;
    y += rule_h + 10.0;
    let mut frame = None;
    grow(&mut frame, big_rect, 40.0);
    grow(
        &mut frame,
        Rect::new(at.x - 142.0, rule_y, at.x + 142.0, rule_y + rule_h),
        40.0,
    );
    let mut tiles = Vec::new();
    for _ in 0..2 {
        for col in 0..LOGO_COLUMNS {
            let centre = Vec2::new(
                at.x + (tile.x + 10.0) * (col as f32 - 2.0),
                y + tile.y / 2.0,
            );
            let r = Rect::from_center_size(centre, tile);
            grow(&mut frame, r, 40.0);
            tiles.push(r);
        }
        y += tile.y + 10.0;
    }
    let Some(mut frame) = frame else { return out };
    frame.min.y += 10.0;
    frame.max.y -= 10.0;
    out.push(frame_piece(frame, gm, panels, images));
    if let Some((image, _)) = big {
        out.push(Piece::new(image.clone(), big_rect));
    }
    out.push(rule(gm, frame.width() - 16.0, rule_y));
    // The list's `+0x124` is clear: every tile at full light.
    for (k, r) in tiles.iter().enumerate() {
        if let Some(image) = art.logos.get(usize::from(LOGOS.start) + k) {
            out.push(Piece::new(
                image.clone(),
                Rect::from_center_size(r.center(), crate::art::LOGO_TILE),
            ));
        }
    }
    if let Some(r) = tiles.get(cursor) {
        let mut p = Piece::new(pm.cursor.clone(), *r);
        p.colour = Color::WHITE.with_alpha(f32::from(look::row_pulse(ticks)) / 255.0);
        out.push(p);
    }
    let title = look.text("chooselogo.txt", 0).unwrap_or("Choose Logo");
    look::title(
        &mut out,
        labels,
        look,
        images,
        fonts::TITLE,
        title,
        colours::COOL,
        // By its bottom, 10 under the frame's top as the grab has it.
        Vec2::new(frame.min.x, frame.min.y + 10.0),
        8,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The card widget's answers (`FUN_00227348` cases 5, 6, 0xc): YES
    /// makes the team, asks to overwrite one of its name (NO lit), or says
    /// the ten slots are used; NO closes it, and NO to the overwrite asks
    /// again.
    #[test]
    fn the_cards_question_makes_or_overwrites_the_team() {
        let team = |city: &str| street_sim::story::CreatedTeam {
            city: city.into(),
            name: "BALLERS".into(),
            logo: LOGOS.start,
            roster: [57, 58, 59, 60, 61],
        };
        let mut teams = vec![team("BOSTON")];
        let ask = CardAsk::Create;
        assert_eq!(ask.first_row(), 0, "YES lit");
        assert_eq!(
            ask.answer(0, &teams, "NEW YORK BALLERS"),
            CardNext::Make(None)
        );
        assert_eq!(ask.answer(1, &teams, "NEW YORK BALLERS"), CardNext::Close);
        let again = ask.answer(0, &teams, "BOSTON BALLERS");
        assert_eq!(again, CardNext::Ask(CardAsk::Overwrite(0)));
        assert_eq!(CardAsk::Overwrite(0).first_row(), 1, "NO lit");
        assert_eq!(
            CardAsk::Overwrite(0).answer(0, &teams, "BOSTON BALLERS"),
            CardNext::Make(Some(0))
        );
        assert_eq!(
            CardAsk::Overwrite(0).answer(1, &teams, "BOSTON BALLERS"),
            CardNext::Ask(CardAsk::Create)
        );
        teams.resize(street_sim::story::TEAM_SLOTS, team("BOSTON"));
        assert_eq!(
            ask.answer(0, &teams, "NEW YORK BALLERS"),
            CardNext::Ask(CardAsk::Full)
        );
        assert_eq!(
            ask.answer(0, &teams, "BOSTON BALLERS"),
            CardNext::Ask(CardAsk::Overwrite(0)),
            "the name is looked for before a free slot"
        );
        assert_eq!(CardAsk::Full.answer(0, &teams, ""), CardNext::Close);
    }

    #[test]
    fn choose_logos_grid_stops_at_its_edges() {
        assert_eq!(logo_step(0, -1, 0), 0);
        assert_eq!(logo_step(0, 1, 0), 1);
        assert_eq!(logo_step(4, 1, 0), 4);
        assert_eq!(logo_step(2, 0, 1), 7);
        assert_eq!(logo_step(7, 0, 1), 7);
        assert_eq!(logo_step(7, 0, -1), 2);
        assert_eq!(LOGOS.len(), 10);
    }

    #[test]
    fn the_grid_pages_a_row_at_a_time() {
        let mut p = Picker::default();
        p.step(1, 0, 12);
        assert_eq!(p.at(), 1);
        p.step(0, 1, 12);
        assert_eq!(p.at(), 6);
        p.step(0, 1, 12);
        assert_eq!((p.first, p.row, p.at()), (1, 1, 11), "the last row's gap");
        p.step(0, 1, 12);
        assert_eq!(p.at(), 11, "no row further");
        p.step(0, -1, 12);
        p.step(0, -1, 12);
        assert_eq!((p.first, p.row), (0, 0));
        p.step(-1, 0, 12);
        p.step(-1, 0, 12);
        assert_eq!(p.col, 0, "the row's start stops");
    }

    #[test]
    fn picks_fill_the_slots_once_each() {
        let mut p = Picker::default();
        assert!(p.pick(7, Mode::Crew));
        assert!(!p.pick(7, Mode::Crew), "picked already");
        assert!(p.pick(8, Mode::Crew));
        assert!(!p.pick(9, Mode::Crew), "two for the crew");
        assert!(p.undo());
        assert!(p.undo());
        assert!(!p.undo());
    }

    #[test]
    fn l1_and_r1_skip_the_empty_categories() {
        let cats = vec![
            Category {
                players: vec![1],
                ..default()
            },
            Category::default(),
            Category {
                players: vec![2],
                ..default()
            },
            Category::default(),
        ];
        let mut p = Picker::default();
        p.turn(1, &cats);
        assert_eq!(p.category, 2);
        p.turn(1, &cats);
        assert_eq!(p.category, 0);
        p.turn(-1, &cats);
        assert_eq!(p.category, 2);
    }
}
