//! Choose Team's "join a team" (class 0x4d, layout 77, backdrop 15:
//! `FUN_002715d0` over the Choose Team family's `FUN_0026e1a8`, its build
//! `FUN_00271730`), which the League Challenge's completion board opens when
//! the challenge's team holds a created baller (`FUN_002a46b8` sets `+0xf0`
//! on a record of kind 0x25; `FUN_002a4898` then goes forward to 0x4d and
//! leaves the card's save to this screen). Each such baller in turn picks an
//! league team to join as its sixth man.
//!
//! - On arrival (`+0x16c`, `FUN_00271c68`) a message, 350 wide
//!   (`FUN_0025d5c8`): "Congratulations! You've beaten every league team ..."
//!   (`wdtm.txt` 20) over Continue.
//! - The left panel (`FUN_00271eb8`): the baller's name in 0xfbac00, font
//!   2, on elements 140, 143 and 142 at (66, 110), (66, 131) and (66, 152):
//!   the first and last names, or the one name on the middle line when the
//!   last is empty; "Joins:" (`common_fe.txt` 22, element 141 at (67,
//!   204)); under it the focused team's five (`FUN_00272280`, list 150 from
//!   (67, 231), 0xfbac00).
//! - The right panel: Choose Team's, over the league's 29.
//! - Cross on a team (`FUN_002719d8`) asks, 350 wide (`FUN_0025d6d8`), "Are
//!   you sure you would like %1S to join %2S?" (`common_fe.txt` 20), or
//!   "%1S already contains a 6th Man. Would you like %2S to make a
//!   replacement?" (21) when the team has one (`FUN_001ed180`): Yes, No,
//!   No lit. Yes (`FUN_00271cb8`) puts the baller on the team
//!   (`FUN_001e0030`) and gives the next one its turn; after the last the
//!   card's save (`FUN_00271dc0`: `FUN_002d8460(.., 0x3f)`) and back to
//!   Main (`FUN_00271c48`). No: sound 9. Triangle does nothing
//!   (`FUN_0026ed50`).

use bevy::prelude::*;
use street_sim::story::Story;

use crate::{
    art::Art,
    chooseteam,
    look::{self, Align, Look, Piece},
    setup::Disc,
};

/// The layout's class.
pub const CLASS: u32 = 77;
/// `FUN_0026e1a8(screen, 0x4d, 0xf)`.
pub const BACKDROP: usize = 15;
/// The names' colour (`FUN_00271eb8`'s `+0x4a4`, the list's prop 40).
const NAME: [u8; 4] = [0xfb, 0xac, 0x00, 0xff];
/// The name's three lines (elements 140, 143, 142), "Joins:" (141) and the
/// list's first row (150), all font 2.
const NAME_AT: [Vec2; 3] = [
    Vec2::new(66.0, 110.0),
    Vec2::new(66.0, 131.0),
    Vec2::new(66.0, 152.0),
];
const JOINS_AT: Vec2 = Vec2::new(67.0, 204.0);
const LIST_AT: Vec2 = Vec2::new(67.0, 231.0);
const FONT: usize = 2;
/// The question's and the message's width (`0x43af0000`).
pub const WIDTH: f32 = 350.0;

/// The screen's state: the ballers still to place and whose turn it is,
/// the grid's cursor and first row, and the message or question over it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Join {
    pub ballers: Vec<usize>,
    pub at: usize,
    pub cursor: usize,
    pub first: usize,
    /// The arrival's message is up.
    pub message: bool,
    /// The question is up, its lit row (0 Yes, 1 No).
    pub ask: Option<usize>,
}

impl Join {
    pub fn new(ballers: Vec<usize>) -> Self {
        Self {
            ballers,
            message: true,
            ..Default::default()
        }
    }

    /// The baller whose turn it is.
    pub fn baller(&self) -> Option<usize> {
        self.ballers.get(self.at).copied()
    }
}

/// The challenge team's created ballers, in its roster's order
/// (`FUN_00271eb8`, `FUN_002a46b8`: its records of kind 0x25).
pub fn joiners(disc: &Disc, story: &Story) -> Vec<usize> {
    let created: Vec<usize> = story.ballers.iter().map(|b| b.record()).collect();
    story
        .league_team
        .and_then(|t| disc.teams.get(usize::from(t)))
        .map(|t| {
            t.roster
                .iter()
                .copied()
                .filter(|r| created.contains(r))
                .collect()
        })
        .unwrap_or_default()
}

/// The baller's name on the panel's three lines.
fn name_lines(disc: &Disc, record: usize) -> [String; 3] {
    let Some(p) = disc.players.get(record) else {
        return Default::default();
    };
    if p.last_name.trim().is_empty() {
        [String::new(), p.first_name.clone(), String::new()]
    } else {
        [p.first_name.clone(), p.last_name.clone(), String::new()]
    }
}

/// The arrival's message.
pub fn message(look: &Look) -> String {
    look.text("wdtm.txt", 20)
        .unwrap_or("Congratulations!")
        .trim_end()
        .to_owned()
}

/// The question for the focused team.
pub fn question(look: &Look, disc: &Disc, story: &Story, join: &Join) -> String {
    let grid = chooseteam::join_grid();
    let Some(team) = grid.get(join.cursor).and_then(|c| c.team) else {
        return String::new();
    };
    let name = join
        .baller()
        .and_then(|r| disc.players.get(r))
        .map(|p| p.name())
        .unwrap_or_default();
    let nickname = disc
        .teams
        .get(team)
        .map(|t| t.nickname.clone())
        .unwrap_or_default();
    let words = if story.sixth_man(team as u16).is_some() {
        look.text("common_fe.txt", 21)
            .unwrap_or("%1S already contains a 6th Man. Would you like %2S to make a replacement?")
            .replace("%1S", &nickname)
            .replace("%2S", &name)
    } else {
        look.text("common_fe.txt", 20)
            .unwrap_or("Are you sure you would like \\n%1S\\n to join %2S? ")
            .replace("%1S", &name)
            .replace("%2S", &nickname)
    };
    words.replace("\\n", "\n").trim_end().to_owned()
}

/// The screen's own pieces: the left panel and Choose Team's right one.
#[allow(clippy::too_many_arguments)]
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: &Art,
    disc: &Disc,
    join: &Join,
    owner: usize,
    ticks: u64,
) -> Vec<Piece> {
    let grid = chooseteam::join_grid();
    let team = grid.get(join.cursor).and_then(|c| c.team).unwrap_or(0);
    let mut out = chooseteam::join_pieces(
        look,
        labels,
        images,
        art,
        disc,
        &grid,
        chooseteam::Side {
            team,
            panel: chooseteam::Panel::Picking {
                cursor: join.cursor,
                first: join.first,
                player: Some(owner),
            },
        },
        ticks,
    );
    let mut text = |out: &mut Vec<Piece>, s: &str, colour: [u8; 4], at: Vec2| {
        if let Some(p) = labels.text(look, images, FONT, s, colour, None, at, Align::Left) {
            out.push(p);
        }
    };
    if let Some(record) = join.baller() {
        for (line, at) in name_lines(disc, record).iter().zip(NAME_AT) {
            text(&mut out, line, NAME, at);
        }
    }
    let joins = look.text("common_fe.txt", 22).unwrap_or("Joins:");
    text(&mut out, joins, look::colours::CREAM, JOINS_AT);
    let line = look.fonts.get(FONT).map_or(27.0, |f| f.line_height as f32);
    if let Some(t) = disc.teams.get(team) {
        for (k, &r) in t.roster.iter().take(5).enumerate() {
            let name = disc.players.get(r).map(|p| p.name()).unwrap_or_default();
            text(
                &mut out,
                &name,
                NAME,
                LIST_AT + Vec2::new(0.0, k as f32 * line),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grid holds the league's 29 and three blanks, four across.
    #[test]
    fn the_grid_is_the_league_teams() {
        let g = chooseteam::join_grid();
        assert_eq!(g.len(), 32);
        assert!((0..29).all(|t| g[t].team == Some(t)));
        assert!(g[29..].iter().all(|c| c.team.is_none()));
    }

    /// A baller who joined a league team is its sixth player (Choose
    /// Players' tiles go up to six), and only while the baller is there.
    #[test]
    fn a_joined_baller_is_the_teams_sixth() {
        let Ok(iso) = std::env::var("STREET_ISO") else {
            return;
        };
        let iso = street_assets::iso::Iso::open(iso).unwrap();
        let mut disc = Disc::load(iso).unwrap();
        let mut story = Story::default();
        let record = usize::from(street_sim::story::BALLER_RECORDS);
        story
            .ballers
            .push(crate::ballers::new(0, &disc.players[record]));
        let five = disc.teams[5].roster.clone();
        assert_eq!(five.len(), 5);
        story.join_team(5, record as u16);
        crate::teams::apply_all(&mut disc, &story);
        assert_eq!(disc.teams[5].roster.len(), 6);
        assert_eq!(disc.teams[5].roster[5], record);
        story.leave_team(record as u16);
        crate::teams::apply_all(&mut disc, &story);
        assert_eq!(disc.teams[5].roster, five);
    }

    /// The board goes on to "join a team" for the challenge team's created
    /// ballers only (`FUN_002a46b8`'s `+0xf0`).
    #[test]
    fn the_board_counts_the_challenge_teams_created_ballers() {
        let Ok(iso) = std::env::var("STREET_ISO") else {
            return;
        };
        let iso = street_assets::iso::Iso::open(iso).unwrap();
        let mut disc = Disc::load(iso).unwrap();
        let mut story = Story::default();
        let record = usize::from(street_sim::story::BALLER_RECORDS);
        story
            .ballers
            .push(crate::ballers::new(0, &disc.players[record]));
        story.league_team = Some(0);
        assert!(joiners(&disc, &story).is_empty(), "a league team");
        story.teams.push(street_sim::story::CreatedTeam {
            city: "NEW YORK".into(),
            name: "BALLERS".into(),
            logo: 41,
            roster: [57, 58, 59, 60, record as u16],
        });
        crate::teams::apply_all(&mut disc, &story);
        story.league_team = Some((disc.teams.len() - 1) as u16);
        assert_eq!(joiners(&disc, &story), [record]);
    }

    /// The name goes on the middle line alone, or the first and the last
    /// on the first two.
    #[test]
    fn one_name_sits_on_the_middle_line() {
        let Ok(iso) = std::env::var("STREET_ISO") else {
            return;
        };
        let iso = street_assets::iso::Iso::open(iso).unwrap();
        let mut disc = Disc::load(iso).unwrap();
        let record = usize::from(street_sim::story::BALLER_RECORDS);
        disc.players[record].first_name = "PLAYER".into();
        disc.players[record].last_name = String::new();
        assert_eq!(name_lines(&disc, record), ["", "PLAYER", ""]);
        disc.players[record].last_name = "ONE".into();
        assert_eq!(name_lines(&disc, record), ["PLAYER", "ONE", ""]);
    }
}
