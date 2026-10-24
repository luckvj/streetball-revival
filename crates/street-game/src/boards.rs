//! The post-game boards, drawn on the game's own layouts (`/FE/XLAY_GM.BIN`)
//! with its words (`/LOCALE/XLOC_GM.EUS`, text files 0x57.. by the table at
//! 0x3ec2f0: 0x57 `common_gm.txt`, 0x5b `pickuprewards.txt`, 0x5c
//! `bealegendrewards.txt`, 0x5d `bealegendrewards2.txt`, 0x5e
//! `topballer.txt`, 0x5f `ballerbreakdown.txt`, 0x60 `rundown.txt`).
//!
//! After the buzzer the Pause task (`FUN_002e3690`) opens The Rundown
//! (class 52, `FUN_002c7a98`; fill `FUN_002c7b48`). Its Continue
//! (`FUN_002c8620`) goes to the Baller Breakdown (class 38) in Be A Legend
//! (game record +0x78 = 5) and to Top Ballers (class 37) otherwise. Top
//! Ballers' Continue (`FUN_002d4ed0`) shows the hookups (class 34) when the
//! results queue at +0x1c0c holds anything in Pick Up and the league
//! Challenge; the Breakdown's (`FUN_00261758`) goes to the development
//! points (class 35, `FUN_002625a8`), whose Continue (`FUN_00262710`) goes
//! to Be A Legend's hookups (class 36) when more is queued. A hookups page
//! that runs out of room shows the rest on another (`DAT_003fc674`).
//!
//! - **The Rundown** (`FUN_002c7b48`): the two teams' logos, city and name,
//!   the score in font 6 (not in a Trick Point Challenge or Gamebreaker
//!   Wins, game record +0xe8 = 1 or 6), then each side's totals
//!   (`FUN_001871e0(stats, side, 1, stat)`): trick points (stat 0xb),
//!   dunks (1), blocks (6), steals (5), rebounds (4), Level 1 (0xc) and
//!   Level 2 gamebreakers (0xd). Side 0, the home side, is on the right.
//! - **Top Ballers** (`FUN_002d42a0`): each side's best player
//!   (`FUN_002d52e0`: points + dunks / 4 + blocks * 3/4 + steals / 2 +
//!   rebounds / 2, or trick points alone in a Trick Point Challenge; the
//!   first of equals), the name over two lines (field 2, then field 1),
//!   the portrait (field 14 into `XAPORTS`), and the six stats.
//! - **Baller Breakdown** (`FUN_002610d0`): the Be A Legend star's name in
//!   font 3, the crew's name, the six stats beside the bars of `XACOMMON`
//!   sheet 0 image 6, and the star standing in 3D on the left.
//! - **Development points** (`FUN_002625a8`): the story block's five lines
//!   at +0xce4 and their total at +0xcd0 (see `street_sim::story`), each
//!   "%1d Dev Points"; the court line reads "Tournament Victory:" for a
//!   tournament, else "Completed Court:".
//! - **Hookups** (`FUN_002c5cf8`, `FUN_00262ba8`): "Congratulations!", the
//!   hookups line between two stars (`XACOMMON` image 1, frame 0), and a
//!   centred stack: each item a label (font 3, cream) over its value
//!   (font 2, amber), 4 apart and 8 to the next item.
//!
//! Every board sits on the layouts' panel (kind 28, `FUN_00211278`; its
//! pieces placed by `FUN_00211760`): a translucent dark body with the top
//! right corner cut at 45 degrees (`XACOMMON` image 3, frames 0 and 1), a
//! 12-wide navy border on its edge, a thin blue-grey line 7.5 outside, a
//! thin blue line 7.5 inside with a 20-tall band along the top and three
//! dark stars (image 1, frame 3) on it. A layout lists its elements front
//! to back, so they are drawn in reverse.
//!
//! The milestone, nickname and event-unlock notices have their own screens
//! in the game (classes 87 and 95, the map); here they follow on one more
//! board of the same look.

use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use street_assets::{
    fe,
    font::{self, Element, Layout},
    locale, ssh,
};
use street_sim::{
    Goal,
    story::{DevPoints, EVENTS, EventKind, Notice},
};

use crate::{
    Screen,
    art::Art,
    game::{Sim, Stats},
    look::{Align, Labels, Look, Piece, colours},
    setup::{Disc, GameClips, MatchSetup, Scene, TeamNames},
};

/// `XLOC_GM`'s text files from id 0x57 (the table at 0x3ec2f0); id 0 is
/// the archive's own `common.txt`.
const GM_FILES: [&str; 14] = [
    "common_gm.txt",
    "pause.txt",
    "overlays.txt",
    "school.txt",
    "pickuprewards.txt",
    "bealegendrewards.txt",
    "bealegendrewards2.txt",
    "topballer.txt",
    "ballerbreakdown.txt",
    "rundown.txt",
    "milestone.txt",
    "givenickname.txt",
    "nicknames.txt",
    "challengercomes.txt",
];
const GM_FIRST: u16 = 0x57;

/// The screen classes.
mod class {
    pub const PICKUP_REWARDS: u32 = 34;
    pub const DEV_POINTS: u32 = 35;
    pub const LEGEND_REWARDS: u32 = 36;
    pub const TOP_BALLERS: u32 = 37;
    pub const BREAKDOWN: u32 = 38;
    pub const RUNDOWN: u32 = 52;
    pub const MILESTONE: u32 = 87;
    pub const NICKNAME: u32 = 95;
}

/// Colours the boards' code sets (`+0x4a4` of its text widgets).
const LABEL: [u8; 4] = [0xdc, 0xdc, 0xbb, 0xff];
const VALUE: [u8; 4] = [0xfb, 0xac, 0x00, 0xff];

/// The layouts, words and art the boards need.
#[derive(Resource)]
pub struct Gm {
    layouts: Vec<Layout>,
    texts: HashMap<String, Vec<String>>,
    /// A white pixel, tinted for the layouts' filled boxes (kind 2).
    pub(crate) white: Handle<Image>,
    /// `XACOMMON` sheet 0: image 1 (stars), 3 (the panel), 6 (bars).
    stars: Option<ssh::Texture>,
    stars_image: Option<Handle<Image>>,
    panel: Option<ssh::Texture>,
    bars: Option<Handle<Image>>,
    /// `XAFESCRN` sheet 0: the portrait's tile (image 4) and frame (5).
    tile: Option<Handle<Image>>,
    frame: Option<Handle<Image>>,
}

impl Gm {
    pub(crate) fn layout(&self, class: u32) -> Option<&Layout> {
        self.layouts.iter().find(|l| l.class == class)
    }

    /// A text id of the game's layouts or code (`file << 8 | index`).
    pub fn text(&self, id: u16) -> &str {
        let file = id >> 8;
        let name = if file == 0 {
            "common.txt"
        } else {
            match file
                .checked_sub(GM_FIRST)
                .and_then(|k| GM_FILES.get(usize::from(k)))
            {
                Some(name) => name,
                None => return "",
            }
        };
        self.texts
            .get(name)
            .and_then(|t| t.get(usize::from(id & 0xff)))
            .map_or("", String::as_str)
    }
}

fn image(width: u32, height: u32, rgba: Vec<u8>) -> Image {
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Startup: the layouts, words and art. Without them the results stay the
/// plain text pages.
pub fn start(mut commands: Commands, mut disc: ResMut<Disc>, mut images: ResMut<Assets<Image>>) {
    let iso = &mut disc.iso;
    let layouts = match font::layouts(iso, "/FE/XLAY_GM.BIN") {
        Ok(l) => l,
        Err(e) => return warn!("in-game layouts: {e}; the results stay text"),
    };
    let texts = locale::read(iso, "/LOCALE/XLOC_GM.EUS")
        .map(|files| files.into_iter().map(|f| (f.name, f.texts)).collect())
        .unwrap_or_default();
    let common = fe::read(iso, "/FE/XACOMMON.BIN")
        .ok()
        .and_then(|s| s.into_iter().next());
    let take = |name: &str| {
        common
            .as_ref()
            .and_then(|s| s.textures.iter().find(|t| t.name == name).cloned())
    };
    let (stars, panel, bars) = (take("0001"), take("0003"), take("0006"));
    let screen = fe::read(iso, "/FE/XAFESCRN.BIN")
        .ok()
        .and_then(|s| s.into_iter().next());
    let mut add =
        |t: Option<&ssh::Texture>| t.map(|t| images.add(image(t.width, t.height, t.rgba.clone())));
    let find = |name: &str| {
        screen
            .as_ref()
            .and_then(|s| s.textures.iter().find(|t| t.name == name))
    };
    let (tile, frame) = (add(find("0004")), add(find("0005")));
    let (stars_image, bars) = (add(stars.as_ref()), add(bars.as_ref()));
    let white = images.add(image(1, 1, vec![255; 4]));
    commands.insert_resource(Gm {
        layouts,
        texts,
        white,
        stars,
        stars_image,
        panel,
        bars,
        tile,
        frame,
    });
}

/// One line of a hookup.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub font: usize,
    pub colour: [u8; 4],
}

impl Line {
    fn new(text: impl Into<String>, font: usize, colour: [u8; 4]) -> Self {
        Self {
            text: text.into(),
            font,
            colour,
        }
    }
}

/// A hookup: a label over its value, or a value alone.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub label: Option<Line>,
    /// Space between the label and the value.
    pub gap: f32,
    pub value: Line,
    /// Whether two stars flank the value (`FUN_00262ba8`'s and
    /// `FUN_002c5cf8`'s single amber lines: the reputation's, the Reward
    /// Points', the completed game's, the Street Champ jersey's).
    pub starred: bool,
}

impl Item {
    fn pair(label: &str, value: impl Into<String>) -> Self {
        Self {
            label: Some(Line::new(label, 3, LABEL)),
            gap: 4.0,
            value: Line::new(value, 2, VALUE),
            starred: false,
        }
    }

    fn alone(text: impl Into<String>) -> Self {
        Self {
            label: None,
            gap: 0.0,
            value: Line::new(text, 3, VALUE),
            starred: true,
        }
    }
}

/// One post-game board.
#[derive(Clone, Debug, PartialEq)]
pub enum Board {
    /// Class 52.
    Rundown,
    /// Class 37.
    TopBallers,
    /// Class 38: the star (a player record), found among the match's six.
    Breakdown {
        star: Option<usize>,
    },
    /// Class 35.
    DevPoints {
        dev: DevPoints,
        tournament: bool,
    },
    /// Class 34 (`legend` false) or 36: one page of hookups.
    Hookups {
        legend: bool,
        items: Vec<Item>,
    },
    Milestone(u8),
    Nickname(usize),
    /// The notices with screens of their own in the game.
    Notes(Vec<String>),
}

/// Which mode the game was, for the boards it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Pick Up (and online, demo and quick games).
    Match,
    /// An League Challenge game.
    League,
    /// A Be A Legend event game.
    Legend(usize),
    /// A Street School lesson's practice.
    Lesson,
}

/// What a finished game earned, for [`plan`].
#[derive(Clone, Debug)]
pub struct Report {
    pub kind: Kind,
    /// The notices the boards show themselves (see [`on_board`]).
    pub shown: Vec<Notice>,
    /// Lines for the notices drawn elsewhere in the game.
    pub notes: Vec<String>,
    /// How the reputation moved, when the level did not change.
    pub reputation: std::cmp::Ordering,
    /// Be A Legend's star (a player record).
    pub star: Option<usize>,
    pub nickname: Option<usize>,
}

/// Whether the boards show this notice themselves.
pub fn on_board(n: &Notice) -> bool {
    matches!(
        n,
        Notice::DevBreakdown(_)
            | Notice::Development(_)
            | Notice::Court(_)
            | Notice::Team(_)
            | Notice::Player(_)
            | Notice::Progress(_)
            | Notice::Reputation(_)
            | Notice::RewardPoints(_)
            | Notice::Milestone(_)
            | Notice::Nickname
            | Notice::Jersey(_)
            | Notice::BossMove(_)
            | Notice::Card(_)
            | Notice::StreetChamp
    )
}

/// A card's player (`FUN_001d2818`): by its category's table, Street
/// Legends (`0x45e150`), Legends (`0x45e168`) or jerseys (`0x45e1b8`).
pub(crate) fn card_player(disc: &Disc, id: u16) -> Option<usize> {
    let card = disc.cards.iter().find(|c| c.id == id as i16)?;
    let k = usize::try_from(card.index).ok()?;
    let record = match card.category {
        0 => *street_sim::story::STREET_CARD_RECORDS.get(k)?,
        1 => *crate::rewards::LEAGUE_RECORDS.get(k)?,
        3 => *JERSEY_RECORDS.get(k)?,
        _ => return None,
    };
    Some(usize::from(record))
}

/// The jersey cards' players (`FUN_001d2818`'s category-3 table at
/// `0x45e1b8`), by the card's index.
pub const JERSEY_RECORDS: [u16; 25] = [
    165, 164, 162, 163, 177, 179, 161, 183, 176, 160, 182, 180, 159, 174, 173, 171, 175, 170, 168,
    181, 169, 178, 172, 166, 158,
];

/// The Rucker Classic's track (the literal at `FUN_00262ba8`'s 0x5d0a item).
const RUCKER_CLASSIC_TRACK: &str = "Just Blaze: \"Young Guru\"";

/// "First Last" of a player record.
fn full_name(disc: &Disc, record: usize) -> String {
    disc.players.get(record).map_or_else(String::new, |p| {
        if p.last_name.trim().is_empty() {
            p.first_name.clone()
        } else {
            format!("{} {}", p.first_name, p.last_name)
        }
    })
}

/// Names as the hookups join them in one value: "A, B,\nC" (`0x48fdf8`
/// after the first, `0x48fe08` after the second).
fn joined(names: impl IntoIterator<Item = String>) -> String {
    let mut out = String::new();
    for (k, name) in names.into_iter().enumerate() {
        if k > 0 {
            out.push_str(if k < 2 { ", " } else { ",\n" });
        }
        out.push_str(&name);
    }
    out
}

/// A team's players as the hookups list them: "A, B,\nC" (`0x48fdc8`..).
fn roster_line(disc: &Disc, team: usize) -> String {
    let roster = disc.teams.get(team).map(|t| t.roster.clone());
    joined(
        roster
            .unwrap_or_default()
            .iter()
            .map(|&record| full_name(disc, record)),
    )
}

/// The notice kinds (`FUN_001de528`'s slots, game data +0x1c0c + 4 *
/// kind; each holds the first id a game gives it) in the order each results
/// page reads them: Be A Legend's (`FUN_00262ba8`) and Pick Up's
/// (`FUN_002c5cf8`). 0 Reward Points, 1 a team, 2 a court, 3 a level boss,
/// 4..6 jerseys, 7..9 boss moves, 10 a completed game. [`hookups`] lists
/// them in this order; the test reads it from the executable.
#[cfg(test)]
pub const LEGEND_KINDS: [usize; 10] = [2, 1, 4, 5, 6, 3, 7, 8, 9, 10];
#[cfg(test)]
pub const PICKUP_KINDS: [usize; 10] = [2, 1, 10, 4, 5, 6, 7, 8, 9, 0];

/// The hookups in the order the game lists them. Be A Legend's page
/// (`FUN_00262ba8`): the court (+0x1c14; the Rucker Classic, court 10,
/// adds its Just Blaze track, label 0x5d0a), the Legends (+0x1c10),
/// the legend jerseys (+0x1c1c..24), the level boss (+0x1c18, 0x5d01),
/// the boss moves (+0x1c28..30), the completed game (+0x1c34), the Street
/// Champ jersey (block +0xcdc), the progress (+0xccc) and the reputation
/// (+0xcd4, else +0xcd8). Pick Up's (`FUN_002c5cf8`): the court, the
/// legends, the completed game, the jerseys, the boss moves and the
/// Reward Points (+0x1c0c). A jersey line (`0x5d07`, Pick Up's `0x5b03`)
/// names its cards' players, a boss move's (`0x5d02`, `0x5b04`) the moves,
/// each up to its three slots in one value.
fn hookups(report: &Report, disc: &Disc, gm: &Gm) -> Vec<Item> {
    let legend = matches!(report.kind, Kind::Legend(_));
    // Label ids: pickuprewards.txt (0x5b) or bealegendrewards2.txt (0x5d).
    let (court, legends) = if legend {
        (0x5d03, 0x5d06)
    } else {
        (0x5b01, 0x5b02)
    };
    let mut out = Vec::new();
    // One id a kind: `FUN_001de528` fills a slot only while it is empty.
    if let Some(id) = report.shown.iter().find_map(|n| match *n {
        Notice::Court(id) => Some(id),
        _ => None,
    }) {
        let name = disc
            .courts
            .iter()
            .find(|c| c.index == usize::from(id))
            .map_or_else(String::new, |c| c.name.clone());
        out.push(Item::pair(gm.text(court), name));
        if legend && id == 10 {
            out.push(Item::pair(gm.text(0x5d0a), RUCKER_CLASSIC_TRACK));
        }
    }
    if let Some(team) = report.shown.iter().find_map(|n| match *n {
        Notice::Team(team) => Some(team),
        _ => None,
    }) {
        out.push(Item::pair(
            gm.text(legends),
            roster_line(disc, usize::from(team)),
        ));
    }
    let completed = report.shown.iter().find_map(|n| match *n {
        Notice::Card(card) => card_player(disc, card),
        _ => None,
    });
    // The completed game (+0x1c34): its label (`0x5b05`, `0x5d08`) only
    // over the fourth trophy's Jordan (record 167, 0x2c6474 and 0x264250),
    // anyone else's name alone in font 3; either between two stars.
    let completion = |label: u16, record: usize| {
        let name = full_name(disc, record);
        if record == 167 {
            Item {
                starred: true,
                ..Item::pair(gm.text(label), name)
            }
        } else {
            Item::alone(name)
        }
    };
    // Pick Up's follows the legends.
    if let (false, Some(record)) = (legend, completed) {
        out.push(completion(0x5b05, record));
    }
    // The jerseys (+0x1c1c..24, three slots in one value): each card's
    // player (`FUN_001d2818`), on Pick Up's page after its completion.
    let jerseys: Vec<String> = report
        .shown
        .iter()
        .filter_map(|n| match *n {
            Notice::Jersey(card) => card_player(disc, card),
            _ => None,
        })
        .take(3)
        .map(|record| full_name(disc, record))
        .collect();
    if !jerseys.is_empty() {
        let label = if legend { 0x5d07 } else { 0x5b03 };
        out.push(Item::pair(gm.text(label), joined(jerseys)));
    }
    let boss = report.shown.iter().find_map(|n| match *n {
        Notice::Player(r) => Some(usize::from(r)),
        _ => None,
    });
    if let (true, Some(boss)) = (legend, boss) {
        // A level boss beaten (the +0x1c18 branch): his name, then the
        // rest of the page as without him (0x263cb0 goes on to 0x263cbc).
        out.push(Item::pair(gm.text(0x5d01), full_name(disc, boss)));
    }
    // The boss moves (+0x1c28..30, three slots in one value): each move's
    // name (`FUN_001d6888`).
    let moves: Vec<String> = report
        .shown
        .iter()
        .filter_map(|n| match *n {
            Notice::BossMove(row) => disc.moves.get(usize::from(row)),
            _ => None,
        })
        .take(3)
        .map(|m| m.name.clone())
        .collect();
    if !moves.is_empty() {
        let label = if legend { 0x5d02 } else { 0x5b04 };
        out.push(Item::pair(gm.text(label), joined(moves)));
    }
    // Be A Legend's completion (+0x1c34, `0x5d08`): the card's player.
    if let (true, Some(record)) = (legend, completed) {
        out.push(completion(0x5d08, record));
    }
    // The Street Champ jersey (block +0xcdc, `0x5d09` between stars).
    if legend && report.shown.contains(&Notice::StreetChamp) {
        out.push(Item::alone(gm.text(0x5d09)));
    }
    if let Kind::Legend(ev) = report.kind {
        if let Some(pp) = report.shown.iter().find_map(|n| match *n {
            Notice::Progress(pp) => Some(pp),
            _ => None,
        }) {
            let why = match EVENTS[ev].kind {
                EventKind::Pickup => gm.text(0x5d0d),
                EventKind::Tournament => gm.text(0x5d0e),
                EventKind::StreetChallenge if EVENTS[ev].court == 10 => gm.text(0x5d10),
                EventKind::StreetChallenge => "",
            };
            out.push(Item {
                label: Some(Line::new(
                    gm.text(0x5d04).replace("%1d", &pp.to_string()),
                    3,
                    LABEL,
                )),
                // The placement's gap after a label (`FUN_00262ba8`'s last
                // loop: 4 after a line whose `+0x18` is not 0xff).
                gap: 4.0,
                value: Line::new(gm.text(0x5d05).replace("%1s", why), 2, LABEL),
                starred: false,
            });
        }
        let level_up = report
            .shown
            .iter()
            .any(|n| matches!(n, Notice::Reputation(_)))
            && report.reputation == std::cmp::Ordering::Equal;
        if level_up {
            // A new level: its ten progress points (`DAT_00460a78`).
            out.push(Item {
                label: Some(Line::new(gm.text(0x5d04).replace("%1d", "10"), 3, LABEL)),
                gap: 4.0,
                value: Line::new(gm.text(0x5d05).replace("%1s", gm.text(0x5d0f)), 2, LABEL),
                starred: false,
            });
        } else if report
            .shown
            .iter()
            .any(|n| matches!(n, Notice::Reputation(_)))
        {
            let id = if report.reputation == std::cmp::Ordering::Less {
                0x5d0c
            } else {
                0x5d0b
            };
            out.push(Item::alone(gm.text(id)));
        }
    }
    if !legend
        && let Some(rp) = report.shown.iter().find_map(|n| match *n {
            Notice::RewardPoints(rp) => Some(rp),
            _ => None,
        })
    {
        out.push(Item::alone(
            gm.text(0x5b00).replace("%1d", &number(i64::from(rp))),
        ));
    }
    out
}

fn line_height(look: &Look, font: usize) -> f32 {
    look.fonts.get(font).map_or(20.0, |f| f.line_height as f32)
}

fn lines_of(text: &str) -> Vec<&str> {
    text.split("\\n").flat_map(|l| l.split('\n')).collect()
}

/// A text's box as the text widget measures it (`FUN_00239e70`, the
/// widget's `+0x54`): one line is the font's text height, each further
/// line its spacing.
fn height(look: &Look, line: &Line) -> f32 {
    let n = lines_of(&line.text).len();
    look.fonts.get(line.font).map_or(n as f32 * 20.0, |f| {
        f.text_height as f32 + f.line_height as f32 * n.saturating_sub(1) as f32
    })
}

/// The hookups page's room: from the hookups line (element 100) to the
/// Continue button (101), less 16 (`FUN_002c5cf8`).
fn hookups_room(look: &Look, gm: &Gm, class: u32) -> (f32, f32) {
    let layout = gm.layout(class);
    let at = |id: u32| {
        layout
            .and_then(|l| l.elements.iter().find(|e| e.id == id))
            .and_then(|e| e.position())
            .map_or(0.0, |(_, y)| f32::from(y))
    };
    let line = lines_of(gm.text(if class == class::PICKUP_REWARDS {
        0x5b06
    } else {
        0x5d00
    }))
    .len() as f32
        * line_height(look, 2);
    let top = at(100) + line - 4.0;
    let gap = at(101) - top;
    (top + gap / 2.0, gap - 16.0)
}

fn item_height(look: &Look, item: &Item) -> f32 {
    item.label
        .as_ref()
        .map_or(0.0, |l| height(look, l) + item.gap)
        + height(look, &item.value)
        + 8.0
}

/// Splits hookups into pages that fit, as the game does.
fn paginate(look: &Look, gm: &Gm, legend: bool, items: Vec<Item>) -> Vec<Board> {
    let class = if legend {
        class::LEGEND_REWARDS
    } else {
        class::PICKUP_REWARDS
    };
    let (_, room) = hookups_room(look, gm, class);
    let mut pages: Vec<Vec<Item>> = Vec::new();
    let mut used = room;
    for item in items {
        let h = item_height(look, &item);
        if used + h > room && pages.last().is_none_or(|p| !p.is_empty()) {
            pages.push(Vec::new());
            used = 0.0;
        }
        used += h;
        if let Some(page) = pages.last_mut() {
            page.push(item);
        }
    }
    pages
        .into_iter()
        .map(|items| Board::Hookups { legend, items })
        .collect()
}

/// The boards a finished game shows, in order. Empty without the
/// layouts (the plain text pages stay).
pub fn plan(report: &Report, disc: &Disc, look: Option<&Look>, gm: Option<&Gm>) -> Vec<Board> {
    let (Some(look), Some(gm)) = (look, gm) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match report.kind {
        Kind::Lesson => {}
        Kind::Legend(ev) => {
            out.push(Board::Rundown);
            out.push(Board::Breakdown { star: report.star });
            if let Some(dev) = report.shown.iter().find_map(|n| match *n {
                Notice::DevBreakdown(d) => Some(d),
                _ => None,
            }) {
                out.push(Board::DevPoints {
                    dev,
                    tournament: EVENTS[ev].kind == EventKind::Tournament,
                });
            }
            out.extend(paginate(look, gm, true, hookups(report, disc, gm)));
        }
        Kind::Match | Kind::League => {
            out.push(Board::Rundown);
            out.push(Board::TopBallers);
            out.extend(paginate(look, gm, false, hookups(report, disc, gm)));
        }
    }
    let mut milestones = Vec::new();
    for notice in &report.shown {
        if let Notice::Milestone(number) = *notice
            && !milestones.contains(&number)
        {
            milestones.push(number);
            out.push(Board::Milestone(number));
        }
    }
    if report.shown.contains(&Notice::Nickname)
        && let Some(row) = report.nickname
    {
        out.push(Board::Nickname(row));
    }
    // Eight lines a board.
    for chunk in report.notes.chunks(8) {
        out.push(Board::Notes(chunk.to_vec()));
    }
    out
}

/// What the results screen shows this frame; [`crate::game`] keeps it.
#[derive(Resource, Default)]
pub struct PostGame {
    pub boards: Vec<Board>,
    /// The board shown; past the last, the choices over the last board.
    pub page: usize,
    pub options: Vec<String>,
    pub choice: usize,
    /// Online: the rematch vote is in, waiting for the others.
    pub waiting: bool,
}

impl PostGame {
    pub fn active(&self) -> bool {
        !self.boards.is_empty()
    }
}

/// A number as the game prints `%1N`: with thousands separators.
fn number(n: i64) -> String {
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

/// One player's line on the boards (`FUN_002d52e0`'s record).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Line6 {
    points: u32,
    dunks: u32,
    blocks: u32,
    steals: u32,
    rebounds: u32,
    tricks: u32,
}

impl Line6 {
    fn of(stats: &Stats, i: usize) -> Self {
        let at = |v: &Vec<u32>| v.get(i).copied().unwrap_or(0);
        Self {
            points: at(&stats.points),
            dunks: at(&stats.dunks),
            blocks: at(&stats.blocks),
            steals: at(&stats.steals),
            rebounds: at(&stats.rebounds),
            tricks: at(&stats.tricks),
        }
    }

    /// Top Ballers' measure.
    fn score(&self, tricks_only: bool) -> f32 {
        if tricks_only {
            self.tricks as f32
        } else {
            self.points as f32
                + self.dunks as f32 * 0.25
                + self.blocks as f32 * 0.75
                + self.steals as f32 * 0.5
                + self.rebounds as f32 * 0.5
        }
    }
}

/// The best of a side's players (slots), the first of equals.
fn top_baller(lines: &[(usize, Line6)], tricks_only: bool) -> Option<(usize, Line6)> {
    let mut best: Option<(usize, Line6)> = None;
    for &(slot, line) in lines {
        if best.is_none_or(|(_, b)| line.score(tricks_only) > b.score(tricks_only)) {
            best = Some((slot, line));
        }
    }
    best
}

/// Where a layout element's text goes: its point and alignment.
fn place(e: &Element, look: &Look, font: usize, lines: usize) -> (Vec2, Align) {
    let (x, y) = e.position().unwrap_or((0, 0));
    let mut at = Vec2::new(f32::from(x), f32::from(y));
    // The text widget's box (`FUN_00239e70`): one line is the font's text
    // height (its header's byte 0x12), each further line its spacing.
    let h = look
        .fonts
        .get(font)
        .map_or(line_height(look, font) * lines as f32, |f| {
            f.text_height as f32 + line_height(look, font) * lines.saturating_sub(1) as f32
        });
    match (e.align >> 2) & 3 {
        1 => at.y -= h / 2.0,
        2 => at.y -= h,
        _ => {}
    }
    let align = match e.align & 3 {
        1 => Align::Centre,
        2 => Align::Right,
        _ => Align::Left,
    };
    (at, align)
}

/// What fills a layout element with an id.
enum Fill {
    Text(String),
    /// A framed picture (kinds 24, 25): a team logo or a portrait.
    Picture(Option<Handle<Image>>),
    /// A sprite's part at its own size (kind 3).
    Sprite(Handle<Image>, Rect),
    Hidden,
}

/// A panel's line colours (`0xRRGGBB`): the blue 3-wide lines 7.5
/// inside, the navy 12-wide edge, the blue-grey 3-wide lines outside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PanelInk {
    pub blue: u32,
    pub navy: u32,
    pub grey: u32,
}

/// The boards', the pause screen's and the dialogs' panels.
pub(crate) const BOARD_INK: PanelInk = PanelInk {
    blue: 0x424a9c,
    navy: 0x080852,
    grey: 0x98a8b8,
};

/// The panel (kind 28) drawn once into a picture: `rect` is the element's
/// place and size, `body` the alpha its body pieces take (the element's
/// colour, 0xa0 on the boards; `FUN_00211428` sets 0xc0 on the pause
/// screen's, the dialogs keep 0xff); returns the picture and where it
/// goes on the screen; `ink` its line colours ([`BOARD_INK`] for the
/// boards').
pub(crate) fn panel_inked(rect: [f32; 4], gm: &Gm, body: u8, ink: PanelInk) -> (Image, Rect) {
    const S: f32 = 2.0;
    const MARGIN: f32 = 16.0;
    let [x, y, w, h] = rect;
    // `FUN_00211760`: the body reaches 17 above the element; the corner
    // is 40 square.
    let (left, top, right, bottom) = (x, y - 17.0, x + w, y + h);
    let (cx, cy) = (right - 40.0, top + 40.0);
    let origin = Vec2::new(left - MARGIN, top - MARGIN);
    let size = Vec2::new(right - left, bottom - top) + 2.0 * MARGIN;
    let (pw, ph) = ((size.x * S) as usize, (size.y * S) as usize);
    let mut px = vec![[0.0f32; 4]; pw * ph];
    // Paints `colour` (straight RGBA, 0..1) where `inside` holds, sampled
    // 2x2 per pixel, over what is there.
    let mut paint = |lo: Vec2, hi: Vec2, colour: [f32; 4], inside: &dyn Fn(Vec2) -> bool| {
        let to = |v: Vec2| (v - origin) * S;
        let (a, b) = (to(lo).max(Vec2::ZERO), to(hi));
        let (x0, y0) = (a.x.floor() as usize, a.y.floor() as usize);
        let (x1, y1) = ((b.x.ceil() as usize).min(pw), (b.y.ceil() as usize).min(ph));
        for py in y0..y1 {
            for qx in x0..x1 {
                let mut cover = 0.0;
                for (sx, sy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let p = origin + Vec2::new(qx as f32 + sx, py as f32 + sy) / S;
                    if inside(p) {
                        cover += 0.25;
                    }
                }
                if cover == 0.0 {
                    continue;
                }
                let a = colour[3] * cover;
                let d = &mut px[py * pw + qx];
                let out = a + d[3] * (1.0 - a);
                for k in 0..3 {
                    d[k] = (colour[k] * a + d[k] * d[3] * (1.0 - a)) / out.max(1e-6);
                }
                d[3] = out;
            }
        }
    };
    let rgb = |v: u32, a: f32| {
        [
            ((v >> 16) & 0xff) as f32 / 255.0,
            ((v >> 8) & 0xff) as f32 / 255.0,
            (v & 0xff) as f32 / 255.0,
            a,
        ]
    };
    // The body: image 3's frames are black at alpha 0x6e (doubled), under
    // the element's colour 0xa0ffffff.
    let body_alpha = gm
        .panel
        .as_ref()
        .and_then(|t| t.rgba.get((10 * t.width as usize + 10) * 4 + 3))
        .map_or(0.86, |&a| f32::from(a) / 255.0)
        * (f32::from(body) / 255.0);
    // Composited in linear light, so the PS2's darkening takes more alpha.
    let body = [0.0, 0.0, 0.0, crate::look::black_alpha(body_alpha)];
    paint(Vec2::new(left, top), Vec2::new(cx, bottom), body, &|p| {
        p.x >= left && p.x < cx && p.y >= top && p.y < bottom
    });
    paint(Vec2::new(cx, cy), Vec2::new(right, bottom), body, &|p| {
        p.x >= cx && p.x < right && p.y >= cy && p.y < bottom
    });
    // The corner: frame 1, its lower left half.
    paint(Vec2::new(cx, top), Vec2::new(right, cy), body, &|p| {
        p.x >= cx && p.y < cy && p.y - top >= p.x - cx
    });
    // A line `width` wide with square ends, as the line widgets draw.
    let line = |a: Vec2, b: Vec2, width: f32| {
        let d = b - a;
        let len = d.length();
        let n = d / len.max(1e-6);
        move |p: Vec2| {
            let q = p - a;
            let t = q.dot(n);
            let off = (q.x * n.y - q.y * n.x).abs();
            (0.0..=len).contains(&t) && off <= width / 2.0
        }
    };
    let mut stroke = |a: Vec2, b: Vec2, width: f32, colour: [f32; 4]| {
        let pad = Vec2::splat(width);
        let inside = line(a, b, width);
        paint(a.min(b) - pad, a.max(b) + pad, colour, &inside);
    };
    let m = 6.0 * (std::f32::consts::PI / 8.0).tan(); // 2.4852815
    let m3 = 1.5 * (std::f32::consts::PI / 8.0).tan(); // 0.62132037
    let v = Vec2::new;
    // Items 3..7: blue, 3 wide (the top band 20), 7.5 inside.
    let blue = rgb(ink.blue, 1.0);
    let (l, t, r, b) = (left + 7.5, top + 7.5, right - 7.5, bottom - 7.5);
    let (c, d) = (cx - 3.106_601_7, cy + 3.106_601_7);
    stroke(v(l - 1.5, t + 8.5), v(c + m3 + 20.0, t + 8.5), 20.0, blue);
    stroke(v(l, t - 1.5), v(l, b + 1.5), 3.0, blue);
    stroke(v(l - 1.5, b), v(r + 1.5, b), 3.0, blue);
    stroke(v(r, d - m3), v(r, b + 1.5), 3.0, blue);
    stroke(v(c - m3, t - m3), v(r + m3, d + m3), 3.0, blue);
    // Items 8..12: navy, 12 wide, on the edge.
    let navy = rgb(ink.navy, 1.0);
    stroke(v(left - 6.0, top), v(cx + m, top), 12.0, navy);
    stroke(v(left, top - 6.0), v(left, bottom + 6.0), 12.0, navy);
    stroke(v(right, cy - m), v(right, bottom + 6.0), 12.0, navy);
    stroke(v(left - 6.0, bottom), v(right + 6.0, bottom), 12.0, navy);
    stroke(v(cx - m, top - m), v(right + m, cy + m), 12.0, navy);
    // Items 13..17: blue-grey, 3 wide, 7.5 outside.
    let grey = rgb(ink.grey, 1.0);
    let (l, t, r, b) = (left - 7.5, top - 7.5, right + 7.5, bottom + 7.5);
    let (c, d) = (cx + m3, cy - m3);
    stroke(v(l - m3, t), v(c + m3, t), 3.0, grey);
    stroke(v(l, b + m3), v(l, t - m3), 3.0, grey);
    stroke(v(r + m3, b), v(l - m3, b), 3.0, grey);
    stroke(v(r, d - m3), v(r, b + m3), 3.0, grey);
    stroke(v(c + m3, t - m3), v(r + m3, d - m3), 3.0, grey);
    // Items 18..20: image 1's frame 3 (a dark star) at 0.2, 0.5 and 0.8
    // along the band.
    let mut rgba: Vec<u8> = px
        .iter()
        .flat_map(|p| {
            [
                (p[0] * 255.0).round() as u8,
                (p[1] * 255.0).round() as u8,
                (p[2] * 255.0).round() as u8,
                (p[3] * 255.0).round() as u8,
            ]
        })
        .collect();
    if let Some(star) = gm.stars.as_ref() {
        let (sl, st, sw, sh) = (40u32, 1u32, 21u32, 19u32);
        let (l, c) = (left + 7.5, cx - 3.106_601_7);
        for k in [0.2f32, 0.5, 0.8] {
            let centre = v(l + (c - l) * k, top + 7.5 + 8.0);
            let at = (centre - v(sw as f32, sh as f32) / 2.0 - origin) * S;
            for yy in 0..(sh as f32 * S) as u32 {
                for xx in 0..(sw as f32 * S) as u32 {
                    let (tx, ty) = (sl + (xx as f32 / S) as u32, st + (yy as f32 / S) as u32);
                    let s = ((ty * star.width + tx) * 4) as usize;
                    let Some(src) = star.rgba.get(s..s + 4) else {
                        continue;
                    };
                    let (qx, qy) = (at.x as i64 + i64::from(xx), at.y as i64 + i64::from(yy));
                    if qx < 0 || qy < 0 || qx as usize >= pw || qy as usize >= ph {
                        continue;
                    }
                    let o = (qy as usize * pw + qx as usize) * 4;
                    let a = f32::from(src[3]) / 255.0;
                    let da = f32::from(rgba[o + 3]) / 255.0;
                    let out = a + da * (1.0 - a);
                    for ch in 0..3 {
                        let cv = (f32::from(src[ch]) * a
                            + f32::from(rgba[o + ch]) * da * (1.0 - a))
                            / out.max(1e-6);
                        rgba[o + ch] = cv.round() as u8;
                    }
                    rgba[o + 3] = (out * 255.0).round() as u8;
                }
            }
        }
    }
    (
        image(pw as u32, ph as u32, rgba),
        Rect::from_corners(origin, origin + size),
    )
}

/// The panels drawn so far, by their element's place, size and body alpha.
#[derive(Default)]
pub struct Panels(HashMap<[u16; 6], (Handle<Image>, Rect)>);

impl Panels {
    /// The panel at `rect` ([`panel_inked`]), drawn the first time it is asked
    /// for.
    pub(crate) fn get(
        &mut self,
        rect: [f32; 4],
        body: u8,
        gm: &Gm,
        images: &mut Assets<Image>,
    ) -> Piece {
        self.get_inked(rect, body, BOARD_INK, gm, images)
    }

    /// [`Panels::get`] in other line colours.
    pub(crate) fn get_inked(
        &mut self,
        rect: [f32; 4],
        body: u8,
        ink: PanelInk,
        gm: &Gm,
        images: &mut Assets<Image>,
    ) -> Piece {
        let key = [
            rect[0].round() as u16,
            rect[1].round() as u16,
            rect[2].round() as u16,
            rect[3].round() as u16,
            u16::from(body),
            (ink.navy ^ ink.blue ^ (ink.grey << 1)) as u16,
        ];
        let (handle, at) = self
            .0
            .entry(key)
            .or_insert_with(|| {
                let (img, at) = panel_inked(rect, gm, body, ink);
                (images.add(img), at)
            })
            .clone();
        Piece::new(handle, at)
    }
}

struct Painter<'a> {
    look: &'a Look,
    gm: &'a Gm,
    labels: &'a mut Labels,
    images: &'a mut Assets<Image>,
    out: Vec<Piece>,
}

impl Painter<'_> {
    /// `s` (lines split at "\n") in `font`, the first line's top at `at`.
    fn text(
        &mut self,
        font: usize,
        s: &str,
        colour: [u8; 4],
        shadow: Option<([u8; 4], [u8; 4])>,
        at: Vec2,
        align: Align,
    ) {
        let lh = line_height(self.look, font);
        for (k, line) in lines_of(s).into_iter().enumerate() {
            let at = at + Vec2::new(0.0, lh * k as f32);
            if let Some(p) = self.labels.text(
                self.look,
                self.images,
                font,
                line,
                colour,
                shadow,
                at,
                align,
            ) {
                self.out.push(p);
            }
        }
    }

    fn width(&self, font: usize, s: &str) -> f32 {
        self.look.fonts.get(font).map_or(0.0, |f| {
            lines_of(s)
                .into_iter()
                .map(|l| f.measure(l) as f32)
                .fold(0.0, f32::max)
        })
    }

    fn rect(&mut self, r: Rect, colour: [u8; 4]) {
        let mut p = Piece::new(self.gm.white.clone(), r);
        p.colour = Color::srgba_u8(colour[0], colour[1], colour[2], colour[3]);
        self.out.push(p);
    }

    /// A framed picture centred on `centre` (kinds 24 and 25,
    /// `FUN_00223698`): the tile (`XAFESCRN` 0 image 4, frame 1, 66x65),
    /// the picture at the tile's size, the frame (image 5, 74x73).
    fn picture(&mut self, centre: Vec2, picture: Option<&Handle<Image>>) {
        let tile = Rect::from_center_size(centre, Vec2::new(66.0, 65.0));
        if let Some(t) = &self.gm.tile {
            self.out.push(Piece::part(
                t.clone(),
                Rect::new(54.0, 1.0, 120.0, 66.0),
                tile,
            ));
        }
        if let Some(p) = picture {
            self.out.push(Piece::new(p.clone(), tile));
        }
        if let Some(f) = &self.gm.frame {
            self.out.push(Piece::part(
                f.clone(),
                Rect::new(3.0, 2.0, 77.0, 75.0),
                Rect::from_center_size(centre, Vec2::new(74.0, 73.0)),
            ));
        }
    }

    /// A layout's elements back to front, filled from `fills` by id.
    /// `bare` leaves out the layout's own words but the title.
    fn layout(&mut self, class: u32, fills: &HashMap<u32, Fill>, panels: &mut Panels, bare: bool) {
        let gm = self.gm;
        let Some(layout) = gm.layout(class) else {
            return;
        };
        for e in layout.elements.iter().rev() {
            let fill = (e.id < 0xffff).then(|| fills.get(&e.id)).flatten();
            if matches!(fill, Some(Fill::Hidden)) {
                continue;
            }
            let at = e
                .position()
                .map_or(Vec2::ZERO, |(x, y)| Vec2::new(f32::from(x), f32::from(y)));
            match e.kind {
                28 => {
                    let (w, h) = e.size().unwrap_or((496, 346));
                    let body = e.colour().map_or(0xa0, |c| c[3]);
                    let piece = panels.get(
                        [at.x, at.y, f32::from(w), f32::from(h)],
                        body,
                        self.gm,
                        self.images,
                    );
                    self.out.push(piece);
                }
                2 => {
                    let (w, h) = e.size().unwrap_or((0, 0));
                    let colour = e.colour().unwrap_or([0, 0, 0, 0xff]);
                    self.rect(
                        Rect::new(at.x, at.y, at.x + f32::from(w), at.y + f32::from(h)),
                        colour,
                    );
                }
                4 | 11 | 35 => {
                    let text = match fill {
                        Some(Fill::Text(s)) => s.clone(),
                        _ if bare && e.kind == 4 => continue,
                        _ => e
                            .texts()
                            .first()
                            .map_or(String::new(), |&id| self.gm.text(id).to_owned()),
                    };
                    let font = e.props.get(&13).map_or(1, |&f| f as usize);
                    let colour = e.colour().unwrap_or(colours::PALE);
                    let shadow = (e.kind == 35).then(|| {
                        (
                            e.props.get(&23).map_or(colours::COOL.0, |&v| font::argb(v)),
                            e.props.get(&24).map_or(colours::COOL.1, |&v| font::argb(v)),
                        )
                    });
                    let (at, align) = place(e, self.look, font, lines_of(&text).len());
                    self.text(font, &text, colour, shadow, at, align);
                }
                // The legend (`FUN_0021a600`): Continue on its Cross icon,
                // drawn as the front end's legends are.
                34 => crate::look::legend(
                    &mut self.out,
                    self.labels,
                    self.look,
                    self.images,
                    e,
                    &|id| Some(gm.text(id).to_owned()),
                ),
                24 | 25 => {
                    let picture = match fill {
                        Some(Fill::Picture(p)) => p.clone(),
                        _ => None,
                    };
                    self.picture(at, picture.as_ref());
                }
                3 => {
                    if let Some(Fill::Sprite(img, src)) = fill {
                        self.out.push(Piece::part(
                            img.clone(),
                            *src,
                            Rect::from_corners(at, at + src.size()),
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    /// The hookups (classes 34 and 36): the items stacked and centred in
    /// the room left, a starred item's value between two stars
    /// (`XACOMMON` image 1 frame 0, middle-anchored: the left one 4 left
    /// of the words, the right one 8 right).
    fn hookups(&mut self, class: u32, items: &[Item]) {
        let (mid, _) = hookups_room(self.look, self.gm, class);
        let total: f32 = items.iter().map(|i| item_height(self.look, i)).sum();
        let mut y = mid - total / 2.0;
        for item in items {
            if let Some(label) = &item.label {
                self.text(
                    label.font,
                    &label.text,
                    label.colour,
                    None,
                    Vec2::new(320.0, y),
                    Align::Centre,
                );
                y += height(self.look, label) + item.gap;
            }
            let v = &item.value;
            self.text(
                v.font,
                &v.text,
                v.colour,
                None,
                Vec2::new(320.0, y),
                Align::Centre,
            );
            if item.starred
                && let Some(stars) = &self.gm.stars_image
            {
                let size = Vec2::new(19.0, 18.0);
                let src = Rect::new(2.0, 1.0, 21.0, 19.0);
                let w = self.width(v.font, &v.text);
                let middle = y + height(self.look, v) / 2.0;
                for x in [320.0 - w / 2.0 - size.x - 4.0, 320.0 + w / 2.0 + 8.0] {
                    let at = Vec2::new(x, middle - size.y / 2.0);
                    self.out.push(Piece::part(
                        stars.clone(),
                        src,
                        Rect::from_corners(at, at + size),
                    ));
                }
            }
            y += height(self.look, v) + 8.0;
        }
    }
}

/// The pieces of one board.
#[allow(clippy::too_many_arguments)]
fn board_pieces(
    painter: &mut Painter,
    board: &Board,
    world: &street_sim::World,
    stats: &Stats,
    setup: &MatchSetup,
    teams: &TeamNames,
    disc: &mut Disc,
    art: Option<&Art>,
    mut portraits: Option<&mut crate::portraits::Portraits>,
    panels: &mut Panels,
) {
    let mut fills: HashMap<u32, Fill> = HashMap::new();
    let n = world.players.len();
    let lines: Vec<(usize, Line6)> = (0..n).map(|i| (i, Line6::of(stats, i))).collect();
    let side = |t: usize| -> Vec<(usize, Line6)> {
        lines
            .iter()
            .copied()
            .filter(|&(i, _)| world.team_of(i) == t)
            .collect()
    };
    let record = |slot: usize| {
        setup
            .lineups
            .get(slot / 3)
            .and_then(|l| l.get(slot % 3))
            .copied()
    };
    let no_score = matches!(
        world.rules.goal,
        Goal::TrickPoints(_) | Goal::Gamebreaker(_)
    );
    let tricks_only = matches!(world.rules.goal, Goal::TrickPoints(_));
    let num = |v: u32| Fill::Text(number(i64::from(v)));
    // A team's logo, and its city and name over two lines.
    let team_parts = |t: usize| {
        let team = setup.teams.get(t).and_then(|&k| disc.teams.get(k));
        let logo = team.and_then(|tm| {
            art.and_then(|a| a.logos.get(usize::try_from(tm.logo).unwrap_or(0)).cloned())
        });
        (logo, teams.1[t].clone(), teams.0[t].clone())
    };
    match board {
        Board::Rundown => {
            // Side 1 (away) on the left, side 0 (home) on the right.
            for (t, score, logo, city, name, ids) in [
                (1usize, 5u32, 4u32, 2u32, 10u32, [11u32, 3, 1, 7, 6, 8, 9]),
                (0, 16, 15, 13, 21, [22, 14, 12, 18, 17, 19, 20]),
            ] {
                let (pic, c, nm) = team_parts(t);
                fills.insert(logo, Fill::Picture(pic));
                fills.insert(city, Fill::Text(c));
                fills.insert(name, Fill::Text(nm));
                fills.insert(
                    score,
                    if no_score {
                        Fill::Hidden
                    } else {
                        num(world.score[t])
                    },
                );
                let sum = |f: fn(&Line6) -> u32| side(t).iter().map(|(_, l)| f(l)).sum::<u32>();
                let gb = |v: &Vec<u32>| {
                    (0..n)
                        .filter(|&i| world.team_of(i) == t)
                        .map(|i| v.get(i).copied().unwrap_or(0))
                        .sum::<u32>()
                };
                let values = [
                    world.teams[t].trick_points,
                    sum(|l| l.dunks),
                    sum(|l| l.blocks),
                    sum(|l| l.steals),
                    sum(|l| l.rebounds),
                    gb(&stats.gb1),
                    gb(&stats.gb2),
                ];
                for (id, v) in ids.into_iter().zip(values) {
                    fills.insert(id, num(v));
                }
            }
            painter.layout(class::RUNDOWN, &fills, panels, false);
        }
        Board::TopBallers => {
            for (t, name, portrait, ids) in [
                (1usize, 11u32, 13u32, [12u32, 10, 9, 15, 14, 16]),
                (0, 3, 5, [4, 2, 1, 7, 6, 8]),
            ] {
                let Some((slot, line)) = top_baller(&side(t), tricks_only) else {
                    continue;
                };
                let rec = record(slot).and_then(|r| disc.players.get(r).cloned());
                if let Some(p) = &rec {
                    let two = if p.last_name.trim().is_empty() {
                        p.first_name.clone()
                    } else {
                        format!("{}\n{}", p.first_name, p.last_name)
                    };
                    fills.insert(name, Fill::Text(two));
                }
                // Created ballers' portraits are built from parts in the
                // game; they keep the empty tile here.
                let pic = rec.filter(|p| p.team != 37).and_then(|p| {
                    let k = usize::try_from(p.portrait).ok()?;
                    portraits.as_deref_mut()?.get(painter.images, k)
                });
                fills.insert(portrait, Fill::Picture(pic));
                let values = [
                    line.points,
                    line.dunks,
                    line.blocks,
                    line.steals,
                    line.rebounds,
                    line.tricks,
                ];
                for (k, (id, v)) in ids.into_iter().zip(values).enumerate() {
                    fills.insert(
                        id,
                        if k == 0 && no_score {
                            Fill::Hidden
                        } else {
                            num(v)
                        },
                    );
                }
            }
            if no_score {
                fills.insert(17, Fill::Hidden);
            }
            painter.layout(class::TOP_BALLERS, &fills, panels, false);
        }
        Board::Breakdown { star } => {
            let slot = star
                .and_then(|r| (0..n).find(|&s| record(s) == Some(r)))
                .unwrap_or(0);
            let line = Line6::of(stats, slot);
            if let Some(p) = record(slot).and_then(|r| disc.players.get(r)) {
                let two = if p.last_name.trim().is_empty() {
                    p.first_name.clone()
                } else {
                    format!("{}\n{}", p.first_name, p.last_name)
                };
                fills.insert(3, Fill::Text(two));
            }
            let t = world.team_of(slot);
            let (_, city, name) = team_parts(t);
            fills.insert(13, Fill::Text(format!("{city} {name}").trim().to_owned()));
            for (id, v) in [4u32, 2, 0, 6, 5, 14].into_iter().zip([
                line.points,
                line.dunks,
                line.blocks,
                line.steals,
                line.rebounds,
                line.tricks,
            ]) {
                fills.insert(id, num(v));
            }
            if let Some(bars) = &painter.gm.bars {
                for id in 7..=12 {
                    fills.insert(
                        id,
                        Fill::Sprite(bars.clone(), Rect::new(2.0, 2.0, 307.0, 32.0)),
                    );
                }
            }
            painter.layout(class::BREAKDOWN, &fills, panels, false);
        }
        Board::DevPoints { dev, tournament } => {
            let label = painter.gm.text(if *tournament { 0x5c05 } else { 0x5c04 });
            fills.insert(6, Fill::Text(label.to_owned()));
            let dp = |v: u32| {
                Fill::Text(
                    painter
                        .gm
                        .text(0x5c00)
                        .replace("%1d", &number(i64::from(v))),
                )
            };
            for (id, v) in [
                (4u32, dev.tricks),
                (1, dev.level1),
                (2, dev.level2),
                (5, dev.court),
                (0, dev.win_loss),
                (3, dev.total()),
            ] {
                fills.insert(id, dp(v));
            }
            painter.layout(class::DEV_POINTS, &fills, panels, false);
        }
        Board::Hookups { legend, items } => {
            let class = if *legend {
                class::LEGEND_REWARDS
            } else {
                class::PICKUP_REWARDS
            };
            painter.layout(class, &fills, panels, false);
            painter.hookups(class, items);
        }
        Board::Milestone(number) => {
            fills.insert(
                7,
                Fill::Text(
                    painter
                        .gm
                        .text(0x6100)
                        .replace("%1d", &(u32::from(*number) + 1).to_string()),
                ),
            );
            fills.insert(
                2,
                Fill::Text(painter.gm.text(0x6103 + u16::from(*number)).to_owned()),
            );
            let card_id = street_sim::story::MILESTONE_CARD + u16::from(*number);
            let name = disc
                .cards
                .iter()
                .find(|card| card.id as u16 == card_id)
                .map(|card| crate::rewards::card_name(&disc.poster, card))
                .unwrap_or_default();
            fills.insert(5, Fill::Text(name.clone()));
            fills.insert(3, Fill::Hidden);
            fills.insert(6, Fill::Hidden);
            painter.layout(class::MILESTONE, &fills, panels, false);
            if let Some(stars) = &painter.gm.stars_image {
                let half = painter.width(4, &name) * 0.5;
                for x in [320.0 - half - 20.0, 320.0 + half + 20.0] {
                    painter.out.push(Piece::part(
                        stars.clone(),
                        Rect::new(2.0, 1.0, 21.0, 19.0),
                        Rect::new(x - 9.5, 239.0, x + 9.5, 257.0),
                    ));
                }
            }
        }
        Board::Nickname(row) => {
            let name = disc
                .nicknames
                .get(*row)
                .map(|nick| nick.name.as_str())
                .unwrap_or("");
            fills.insert(2, Fill::Text(name.to_owned()));
            fills.insert(
                1,
                Fill::Text(disc.nickname_flavor.get(*row).cloned().unwrap_or_default()),
            );
            painter.layout(class::NICKNAME, &fills, panels, false);
        }
        Board::Notes(notes) => {
            painter.layout(class::LEGEND_REWARDS, &fills, panels, true);
            let lh = line_height(painter.look, 2) + 8.0;
            let rows: usize = notes.iter().map(|n| lines_of(n).len()).sum();
            let mut y = 245.0 - lh * rows as f32 / 2.0;
            for note in notes {
                painter.text(2, note, LABEL, None, Vec2::new(320.0, y), Align::Centre);
                y += lh * lines_of(note).len() as f32;
            }
        }
    }
}

/// One UI node showing a board's piece.
#[derive(Component)]
pub struct BoardSlot(usize);

/// Draws the board [`PostGame`] names, above the match and its HUD.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    post: Res<PostGame>,
    (look, gm, art): (Option<Res<Look>>, Option<Res<Gm>>, Option<Res<Art>>),
    mut labels: ResMut<Labels>,
    mut images: ResMut<Assets<Image>>,
    (sim, stats, teams, setup): (Res<Sim>, Res<Stats>, Res<TeamNames>, Res<MatchSetup>),
    mut disc: ResMut<Disc>,
    (mut panels, mut portraits, mut ticks): (
        Local<Panels>,
        Option<ResMut<crate::portraits::Portraits>>,
        Local<u64>,
    ),
    mut commands: Commands,
    mut slots: Query<(&BoardSlot, &mut ImageNode, &mut Node, &mut Visibility)>,
) {
    let mut pieces = Vec::new();
    if let (true, Some(look), Some(gm)) = (post.active(), look.as_deref(), gm.as_deref()) {
        labels.tidy();
        if let Some(board) = post.boards.get(post.page) {
            *ticks = 0;
            let mut painter = Painter {
                look,
                gm,
                labels: &mut labels,
                images: &mut images,
                out: Vec::new(),
            };
            // The screen's own full-screen rectangle (`FUN_002d7720`'s
            // `+0xc4`), which every board's constructor turns black at
            // alpha 0xa0 (`FUN_002d8350(screen, 1)`; the pause passes 0):
            // the court keeps 0.37 of its light behind the boards.
            let mut dim = Piece::new(gm.white.clone(), Rect::new(0.0, 0.0, 640.0, 480.0));
            dim.colour = Color::srgba(0.0, 0.0, 0.0, crate::look::black_alpha(BOARD_DIM));
            painter.out.push(dim);
            board_pieces(
                &mut painter,
                board,
                &sim.world,
                &stats,
                &setup,
                &teams,
                &mut disc,
                art.as_deref(),
                portraits.as_deref_mut(),
                &mut panels,
            );
            pieces = painter.out;
        } else if !post.options.is_empty() {
            // After the boards, the choices in a dialog over the court
            // (`FUN_002d4ed0` in Pick Up: "Do you want to quit?",
            // `common_gm.txt` 11, over REMATCH, lit, and the three
            // quits). Online the rewrite's own vote line takes the words.
            *ticks += 1;
            let text = if post.waiting {
                "Rematch: waiting for the other players..."
            } else if post.options.len() == 4 {
                gm.text(0x570b)
            } else {
                ""
            };
            pieces = crate::pause::dialog(
                look,
                gm,
                &mut labels,
                &mut images,
                &mut panels,
                &crate::pause::Dialog {
                    text,
                    rows: &post.options,
                    row_font: crate::pause::FONT,
                    lit: post.choice,
                    ticks: *ticks,
                    dim: 0.0,
                    width: 0.0,
                },
            );
        }
    }
    show(&mut commands, &mut slots, &pieces);
}

/// The boards' dim (`FUN_002d8350`: 0xa0 of 0xff).
const BOARD_DIM: f32 = 160.0 / 255.0;

/// Points the board's slots at this frame's pieces, spawning more when
/// short (as [`crate::look::show`], with slots of its own over the HUD's).
fn show(
    commands: &mut Commands,
    slots: &mut Query<(&BoardSlot, &mut ImageNode, &mut Node, &mut Visibility)>,
    pieces: &[Piece],
) {
    let pct = |v: f32, of: f32| Val::Percent(v / of * 100.0);
    let place = |node: &mut Node, r: &Rect| {
        node.left = pct(r.min.x, 640.0);
        node.top = pct(r.min.y, 480.0);
        node.width = pct(r.width(), 640.0);
        node.height = pct(r.height(), 480.0);
    };
    let mut have = 0;
    for (slot, mut image, mut node, mut vis) in slots.iter_mut() {
        have = have.max(slot.0 + 1);
        match pieces.get(slot.0) {
            Some(p) => {
                if image.image != p.image {
                    image.image = p.image.clone();
                }
                image.rect = p.source;
                image.flip_x = p.flip;
                image.color = p.colour;
                place(&mut node, &p.rect);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    for (k, p) in pieces.iter().enumerate().skip(have) {
        let mut node = Node {
            position_type: PositionType::Absolute,
            ..default()
        };
        place(&mut node, &p.rect);
        let mut image = ImageNode::new(p.image.clone()).with_mode(NodeImageMode::Stretch);
        image.rect = p.source;
        image.flip_x = p.flip;
        image.color = p.colour;
        commands.spawn((
            image,
            node,
            Visibility::Inherited,
            GlobalZIndex(200 + k as i32),
            BoardSlot(k),
            DespawnOnExit(Screen::Playing),
        ));
    }
}

/// The Baller Breakdown's star, standing in 3D on the board's left
/// (`FUN_002610d0` builds an FEPlayerObject from the baller's record).
#[derive(Component)]
pub struct StarPart;

/// Where the star stands, away from the court.
const STAR_STAGE: Vec3 = Vec3::new(3000.0, 0.0, 0.0);
/// The board's left half: from the panel's edge to the divider at x 257.
const STAR_RECT: Rect = Rect {
    min: Vec2::new(78.0, 92.0),
    max: Vec2::new(254.0, 424.0),
};

/// Keeps the star standing while the Breakdown shows.
#[allow(clippy::too_many_arguments)]
pub fn star(
    post: Res<PostGame>,
    setup: Res<MatchSetup>,
    scene: Option<Res<Scene>>,
    clips: Res<GameClips>,
    mut shown: Local<Option<usize>>,
    parts: Query<Entity, With<StarPart>>,
    mut commands: Commands,
    (mut images, mut meshes, mut materials, mut bindposes): (
        ResMut<Assets<Image>>,
        ResMut<Assets<Mesh>>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    ),
) {
    let wanted = match post.boards.get(post.page) {
        Some(Board::Breakdown { star }) => {
            let slots = setup.lineups.iter().flatten().copied().collect::<Vec<_>>();
            Some(
                star.and_then(|r| slots.iter().position(|&s| s == r))
                    .unwrap_or(0),
            )
        }
        _ => None,
    };
    if *shown == wanted {
        return;
    }
    for e in &parts {
        commands.entity(e).despawn();
    }
    *shown = wanted;
    let (Some(slot), Some(scene)) = (wanted, scene) else {
        return;
    };
    let Some(assets) = scene.players.get(slot) else {
        return;
    };
    let size = STAR_RECT.size();
    let target = images.add(Image::new_target_texture(
        (size.x * 2.0) as u32,
        (size.y * 2.0) as u32,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let player = street_render::player::spawn(
        &mut commands,
        assets,
        Transform::from_translation(STAR_STAGE)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI - 0.3)),
        &mut meshes,
        &mut materials,
        &mut images,
        &mut bindposes,
    );
    commands.entity(player).insert((
        street_render::animation::Animator::new(clips.get(crate::clip::CREATE_STAND)),
        StarPart,
        DespawnOnExit(Screen::Playing),
    ));
    let height = assets.hip_height();
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        bevy::camera::RenderTarget::from(target.clone()),
        Transform::from_translation(STAR_STAGE + Vec3::new(0.0, height * 1.05, height * 4.4))
            .looking_at(STAR_STAGE + Vec3::new(0.0, height * 0.98, 0.0), Vec3::Y),
        StarPart,
        DespawnOnExit(Screen::Playing),
    ));
    let pct = |v: f32, of: f32| Val::Percent(v / of * 100.0);
    commands.spawn((
        ImageNode::new(target),
        Node {
            position_type: PositionType::Absolute,
            left: pct(STAR_RECT.min.x, 640.0),
            top: pct(STAR_RECT.min.y, 480.0),
            width: pct(size.x, 640.0),
            height: pct(size.y, 480.0),
            ..default()
        },
        GlobalZIndex(199),
        StarPart,
        DespawnOnExit(Screen::Playing),
    ));
}

/// A new match: no boards yet.
fn clear(mut post: ResMut<PostGame>) {
    *post = PostGame::default();
}

pub struct BoardsPlugin;

impl Plugin for BoardsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PostGame>()
            .add_systems(Startup, start)
            .add_systems(OnEnter(Screen::Playing), clear)
            .add_systems(Update, (draw, star).run_if(in_state(Screen::Playing)));
    }
}

#[cfg(test)]
mod tests {
    /// The results pages read the notice slots in [`LEGEND_KINDS`] and
    /// [`PICKUP_KINDS`] order: the first `lw`/`lh` of each slot offset
    /// (+0x1c0c..+0x1c34) in `FUN_00262ba8` (to 0x2655bc) and
    /// `FUN_002c5cf8` (to 0x2c77f4), read from the disc's executable.
    #[test]
    fn the_results_pages_read_the_notice_kinds_in_the_executables_order() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let elf = street_assets::iso::Iso::open(&path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let kinds = |start: u32, end: u32| {
            let words =
                street_assets::scripts::executable_words(&elf, start, ((end - start) / 4) as usize)
                    .unwrap();
            let mut out = Vec::new();
            for w in words {
                // lh 0x21, lw 0x23, lhu 0x25.
                let offset = w & 0xffff;
                if matches!(w >> 26, 0x21 | 0x23 | 0x25) && (0x1c0c..=0x1c34).contains(&offset) {
                    let kind = ((offset - 0x1c0c) / 4) as usize;
                    if !out.contains(&kind) {
                        out.push(kind);
                    }
                }
            }
            out
        };
        assert_eq!(kinds(0x262ba8, 0x2655bc), super::LEGEND_KINDS);
        assert_eq!(kinds(0x2c5cf8, 0x2c77f4), super::PICKUP_KINDS);
    }

    use super::*;

    #[test]
    fn three_slots_join_in_one_value() {
        let names = |n: usize| (0..n).map(|k| format!("N{k}")).collect::<Vec<_>>();
        assert_eq!(joined(names(1)), "N0");
        assert_eq!(joined(names(2)), "N0, N1");
        assert_eq!(joined(names(3)), "N0, N1,\nN2");
    }

    #[test]
    fn the_hookups_star_their_single_amber_lines() {
        // `FUN_00262ba8` and `FUN_002c5cf8` put the stars beside the
        // reputation's, the Reward Points', the completed game's and the
        // Street Champ jersey's lines, not beside the page's header.
        let alone = Item::alone("Your Reputation Went Up!");
        assert!(alone.starred);
        assert_eq!(alone.value.colour, VALUE);
        let pair = Item::pair("Court:", "Rucker Park");
        assert!(!pair.starred);
    }

    #[test]
    fn numbers_take_thousands_separators() {
        assert_eq!(number(0), "0");
        assert_eq!(number(999), "999");
        assert_eq!(number(1000), "1,000");
        assert_eq!(number(1_234_567), "1,234,567");
        assert_eq!(number(-25_000), "-25,000");
    }

    #[test]
    fn top_ballers_weigh_the_box_score_and_keep_the_first_of_equals() {
        let a = Line6 {
            points: 6,
            ..Default::default()
        };
        // 4 points + 4 blocks * 3/4 = 7 beats 6 points.
        let b = Line6 {
            points: 4,
            blocks: 4,
            ..Default::default()
        };
        assert_eq!(top_baller(&[(0, a), (1, b)], false).map(|t| t.0), Some(1));
        assert_eq!(top_baller(&[(0, a), (1, a)], false).map(|t| t.0), Some(0));
        // A Trick Point Challenge goes by trick points alone.
        let c = Line6 {
            tricks: 50_000,
            ..Default::default()
        };
        assert_eq!(top_baller(&[(0, b), (1, c)], true).map(|t| t.0), Some(1));
    }

    #[test]
    fn the_text_files_follow_the_games_table() {
        // 0x3ec2f0: 0x5b pickuprewards ... 0x60 rundown, 0x64 the last.
        assert_eq!(
            GM_FILES[usize::from(0x5bu16 - GM_FIRST)],
            "pickuprewards.txt"
        );
        assert_eq!(GM_FILES[usize::from(0x60u16 - GM_FIRST)], "rundown.txt");
        assert_eq!(
            GM_FILES[usize::from(0x64u16 - GM_FIRST)],
            "challengercomes.txt"
        );
    }

    #[test]
    fn the_panel_is_dark_inside_and_cut_at_the_corner() {
        let gm = Gm {
            layouts: Vec::new(),
            texts: HashMap::new(),
            white: Handle::default(),
            stars: None,
            stars_image: None,
            panel: None,
            bars: None,
            tile: None,
            frame: None,
        };
        let (img, rect) = panel_inked([72.0, 82.0, 496.0, 346.0], &gm, 0xa0, BOARD_INK);
        let data = img.data.as_ref().unwrap();
        let w = img.width() as f32;
        let at = |x: f32, y: f32| {
            let (px, py) = ((x - rect.min.x) * 2.0, (y - rect.min.y) * 2.0);
            let o = ((py as usize) * (w as usize) + px as usize) * 4;
            [data[o], data[o + 1], data[o + 2], data[o + 3]]
        };
        // The middle: translucent black, at the alpha that darkens as the
        // PS2's 0.86 * 0xa0 does once composited in linear light.
        let mid = at(320.0, 250.0);
        assert_eq!(&mid[..3], &[0, 0, 0]);
        let want = crate::look::black_alpha(0.86 * 160.0 / 255.0) * 255.0;
        assert!((f32::from(mid[3]) - want).abs() <= 1.5, "{mid:?} {want}");
        assert!(mid[3] > 200, "{mid:?}");
        // The cut corner's outside is empty; the navy border is opaque.
        assert_eq!(at(566.0, 67.0)[3], 0);
        assert_eq!(at(72.0, 250.0), [0x08, 0x08, 0x52, 0xff]);
    }
}
