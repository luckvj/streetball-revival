//! What a roster must satisfy to play online: the one source of truth for
//! the game (it will not host an illegal roster; joiners re-check what they
//! are sent), `street-server` (it refuses an illegal roster in any match
//! setup, the real anti-cheat since a client can be modified) and
//! `street-export check-roster` / the catalog's `rules` (the launcher's
//! builder clamps to them). Offline, anything goes.
//!
//! # The bounds and their evidence
//!
//! Measured over the disc's 240 records and the Legendary Edition's
//! (`cargo run -p street-assets --example roster_evidence -- <iso> <mod>`):
//!
//! | Rule | Bound | Evidence |
//! |---|---|---|
//! | height | 63..=90 in | Legendary: Mugsy Bogues #5 63, Yao Ming #42 90; disc: Biggie Little #147 64, Yao Ming 89 |
//! | weight | 90..=338 lb | disc: Biggie Little 90, Shaquille O'Neal #55 338 (Legendary 325) |
//! | weight for height | 1.40..=3.97 lb per inch | Biggie Little 90/64, O'Neal 338/85 (disc); Legendary stays inside (Zion Williamson 285/78 = 3.65) |
//! | model height | shown height -2..=+2 | disc 0 everywhere; Legendary Joel Embiid #43 -2, Donovan Mitchell #135 +2 |
//! | model weight | shown weight +0..=+30 | the disc draws its celebrities 30 lb heavier (Murphy Lee #157 +30); Legendary up to +30 |
//! | body scale (fields 43..45) | 0..=18, 0..=15, 0..=22 | disc and Legendary league records: Keith Van Horn / Luka Doncic #101 x 18, O'Neal y 15 and z 22; everyone else 0 |
//! | ratings | 0..=99 each | the game clamps every read to 0..99 (`0x1d7d00`) |
//! | rating total (the first nine: shots..speed) | at most the best real league player's, legend's or streetballer's (records 0..183) in the base roster; a record no better than its own base record (rating by rating) passes too | disc: Bonafide #146 698; Legendary: Wes Unseld #52 794 (its generic record #231, Gianna Bryant, is all 99s: 891, legal as the mod has it, but no edit may reach it) |
//! | build | 0..=5 | league 0..2, created 0..5 |
//!
//! Created ballers (records 234..243, and `created:<n>` characters) are held
//! to Create A Baller instead (`street_sim::cab`): each skill at most the top
//! of its bar (98), long range at most 90 (it follows the shots bar), speed
//! at most 85 (the lightest build's), a total of at most 861 (every bar
//! full), and a height and weight within the build's sizes.
//!
//! Moves: a lay-up slot holds an `XDBMOVE` lay-up row; dunk and dribble-move
//! slots hold a row of their kind and the slot's level (slot 0 level 4,
//! slots 1..4 level 3, slots 5..10 level 2: the trick slot classes at
//! `0x43d408`, the same as Edit Moves' categories); a row at most once per
//! kind. [`MOVE_ROWS`] gives the table's kinds and levels by row (the rows
//! are grouped by kind and level on the disc; a disc-gated test checks it).
//!
//! Fields: [`field_rule`] sorts the 101 fields. Cosmetic ones (names, team,
//! jersey, skin, looks, hair, clothing, accessories, ...) may change freely;
//! bounded ones (sizes, ratings, build, body scale) within the bounds; move
//! fields (68..73, 79..100) as moves; every other field must equal the
//! source record's (fields 31, 33..35, 37, 74..78).
//!
//! A roster holds at most [`MAX_ADDED`] added players (records 244..307),
//! [`MAX_TEAMS`] teams; team lists name existing records, each at most once.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::db::{Player, Team};
use crate::roster_edits::{
    self, CharId, Created, EMPTY, FIRST_ADDED, MOVE_SLOTS, RosterFile, TEMPLATE_RECORD,
};
use crate::roster_share::{Base, DISC_RECORDS, FIELDS, MatchRoster, SharedPlayer, SharedRoster};

pub const HEIGHT_IN: [i16; 2] = [63, 90];
pub const WEIGHT_LB: [i16; 2] = [90, 338];
/// Pounds per inch of height, in hundredths.
pub const LB_PER_INCH_X100: [i32; 2] = [140, 397];
/// Model height minus shown height.
pub const MODEL_HEIGHT_OFFSET: [i16; 2] = [-2, 2];
/// Model weight minus shown weight.
pub const MODEL_WEIGHT_OFFSET: [i16; 2] = [0, 30];
/// Fields 43, 44, 45.
pub const BODY_SCALE: [[i16; 2]; 3] = [[0, 18], [0, 15], [0, 22]];
pub const RATING: [i16; 2] = [0, 99];
pub const BUILD: [i16; 2] = [0, 5];
/// The ratings a total counts: shots, threes, dunks, rebounds, power,
/// blocks, steals, handles, speed.
pub const TOTALLED: usize = 9;
/// The best total on the disc (Bonafide #146).
pub const ORIGINAL_TOTAL_CAP: i32 = 698;
/// The best real player's total in the Legendary Edition (Wes Unseld #52).
pub const LEGENDARY_TOTAL_CAP: i32 = 794;
/// The records whose totals set the cap: league players (0..144),
/// streetballers (145..157) and legends (158..183); not the generic
/// ballers or created-player records, which a mod may repurpose.
pub const REAL_CHARACTERS: usize = crate::db::GENERIC;
/// Create A Baller's caps per rating: every skill bar tops out at 98,
/// long range follows shots (90 at the top), speed follows the build (85
/// for the lightest); ratings 9 and 10 are not set by it.
pub const CAB_RATING_CAPS: [i16; 11] = [98, 90, 98, 98, 98, 98, 98, 98, 85, 99, 99];
/// Every bar full, the quickest build.
pub const CAB_TOTAL_CAP: i32 = 98 * 7 + 90 + 85;
/// Each build's sizes (`street_sim::cab::SIZES`): height range, weight range.
pub const CAB_SIZES: [[i16; 4]; 6] = [
    [69, 81, 200, 260],
    [72, 84, 210, 275],
    [70, 82, 185, 240],
    [71, 83, 175, 235],
    [65, 77, 130, 190],
    [67, 79, 140, 210],
];
/// Added players a roster may hold.
pub const MAX_ADDED: usize = 64;
/// Player records a roster may hold.
pub const MAX_RECORDS: usize = FIRST_ADDED + MAX_ADDED;
/// Teams: the disc's 49 and the "Custom Roster" team.
pub const MAX_TEAMS: usize = 50;
/// Entries in `roster.json`.
pub const MAX_ROSTER_PLAYERS: usize = DISC_RECORDS + MAX_ADDED;
/// Created ballers' records (create-a-player and career stars).
pub const CREATED_SLOTS: usize = FIRST_ADDED - TEMPLATE_RECORD;
/// Players in a match.
pub const LINEUP: usize = 6;
/// `XDBMOVE` rows.
pub const MOVE_COUNT: usize = 93;
/// `XDBMOVE` by row: (kind, level, first row, last row). Kinds: 0 lay-up,
/// 1 dunk, 2 dribble move.
pub const MOVE_ROWS: [(u8, u8, u8, u8); 7] = [
    (0, 2, 0, 9),
    (1, 2, 10, 25),
    (1, 3, 26, 48),
    (1, 4, 49, 53),
    (2, 2, 54, 67),
    (2, 3, 68, 88),
    (2, 4, 89, 92),
];
/// Dunk and dribble-move slots' levels (`0x43d408`).
pub const SLOT_LEVELS: [u8; 11] = [4, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2];
/// The record fields holding equipped rows: lay-ups, dunks, dribble moves.
pub const MOVE_FIELDS: [(usize, usize); 3] = [(68, 6), (79, 11), (90, 11)];
const KIND_NAMES: [&str; 3] = ["layups", "dunks", "tricks"];

/// How a field may change online.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldRule {
    /// Free: nothing the simulation reads.
    Cosmetic,
    /// Within the bounds above.
    Bounded,
    /// An equipped move row.
    Move,
    /// Must equal the source record's.
    Source,
}

/// The rule for field `f`.
pub fn field_rule(f: usize) -> FieldRule {
    match f {
        8..=11 | 16..=26 | 30 | 43..=45 => FieldRule::Bounded,
        68..=73 | 79..=100 => FieldRule::Move,
        0..=7 | 12..=15 | 27..=29 | 32 | 36 | 38..=42 | 46..=67 => FieldRule::Cosmetic,
        _ => FieldRule::Source,
    }
}

/// The fields editable online (cosmetic, bounded and move fields), for the
/// builder.
pub fn editable_fields() -> Vec<usize> {
    (0..FIELDS)
        .filter(|&f| field_rule(f) != FieldRule::Source)
        .collect()
}

/// The catalog's name for field `f`.
pub fn field_name(f: usize) -> String {
    const NAMED: [(usize, &str); 25] = [
        (0, "id"),
        (1, "lastName"),
        (2, "firstName"),
        (3, "team"),
        (4, "position"),
        (5, "jersey"),
        (7, "skinTone"),
        (8, "heightIn"),
        (9, "weightLb"),
        (10, "modelHeightIn"),
        (11, "modelWeightLb"),
        (12, "female"),
        (13, "leftHanded"),
        (14, "portrait"),
        (27, "style"),
        (28, "star"),
        (29, "code"),
        (30, "build"),
        (32, "look"),
        (36, "clothing"),
        (38, "shoes"),
        (39, "hairStyle"),
        (40, "hairColor"),
        (41, "facialHairStyle"),
        (42, "facialHairColor"),
    ];
    if let Some((_, n)) = NAMED.iter().find(|(k, _)| *k == f) {
        return (*n).into();
    }
    match f {
        16..=26 => format!("ratings[{}]", f - 16),
        43..=45 => format!("bodyScale[{}]", f - 43),
        46..=67 => format!("accessories[{}]", f - 46),
        _ => match MOVE_FIELDS
            .iter()
            .position(|&(first, n)| (first..first + n).contains(&f))
        {
            Some(kind) => format!("moves.{}[{}]", KIND_NAMES[kind], f - MOVE_FIELDS[kind].0),
            None => format!("raw.{f}"),
        },
    }
}

/// An `XDBMOVE` row's (kind, level).
pub fn move_row(row: usize) -> Option<(u8, u8)> {
    MOVE_ROWS
        .iter()
        .find(|&&(_, _, a, b)| (usize::from(a)..=usize::from(b)).contains(&row))
        .map(|&(kind, level, _, _)| (kind, level))
}

/// The level slot `slot` of `kind` takes (`None`: any).
pub fn slot_level(kind: usize, slot: usize) -> Option<u8> {
    (kind != 0)
        .then(|| SLOT_LEVELS.get(slot).copied())
        .flatten()
}

/// Something that makes a roster illegal online.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    /// The `roster.json` player (its index in `players`), if any.
    pub player: Option<usize>,
    /// The record in the effective table, if any.
    pub record: Option<usize>,
    pub character: Option<String>,
    pub field: String,
    pub value: Value,
    /// The range allowed, if it is one.
    pub allowed: Option<[i64; 2]>,
    pub message: String,
}

impl Problem {
    fn new(field: impl Into<String>, value: impl Into<Value>, message: impl Into<String>) -> Self {
        Self {
            player: None,
            record: None,
            character: None,
            field: field.into(),
            value: value.into(),
            allowed: None,
            message: message.into(),
        }
    }

    fn range(field: impl Into<String>, value: i64, allowed: [i64; 2], what: &str) -> Self {
        Self {
            allowed: Some(allowed),
            ..Self::new(
                field,
                value,
                format!("{what} {value} is outside {}..{}", allowed[0], allowed[1]),
            )
        }
    }

    fn on(mut self, record: usize) -> Self {
        self.record = Some(record);
        self
    }

    /// As `check-roster` prints it.
    pub fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "record": self.record,
            "character": self.character,
            "field": self.field,
            "value": self.value,
            "allowed": self.allowed,
            "message": self.message,
        })
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.player, self.record) {
            (Some(p), _) => write!(f, "roster.json player {p}: ")?,
            (None, Some(r)) => write!(f, "record {r}: ")?,
            _ => {}
        }
        f.write_str(&self.message)
    }
}

/// The caps that depend on the base roster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub total_cap: i32,
    /// The base roster is a mod nobody here can see, trusted all the same
    /// (`street-server --trust-roster-mods`): a record over the cap is
    /// taken to be the mod's own.
    pub trusted: bool,
}

impl Default for Limits {
    fn default() -> Self {
        Self::for_base(Base::Original)
    }
}

impl Limits {
    /// The cap for a base, without its tables.
    pub fn for_base(base: Base) -> Self {
        Self {
            total_cap: match base {
                Base::Original => ORIGINAL_TOTAL_CAP,
                Base::Mod(_) => LEGENDARY_TOTAL_CAP,
            },
            trusted: false,
        }
    }

    /// The cap measured on the base roster's real characters
    /// ([`REAL_CHARACTERS`]).
    pub fn from_players(base: &[Player]) -> Self {
        Self {
            total_cap: base
                .iter()
                .take(REAL_CHARACTERS)
                .map(total)
                .max()
                .unwrap_or(ORIGINAL_TOTAL_CAP),
            trusted: false,
        }
    }
}

/// What the checks know about the roster's base.
#[derive(Clone, Copy, Debug, Default)]
pub struct Context<'a> {
    /// The disc's own tables.
    pub disc: Option<(&'a [Player], &'a [Team])>,
    /// The base roster's players (the disc's, or the roster mod's).
    pub base: Option<&'a [Player]>,
    pub limits: Limits,
}

/// Whether `p` is no better than `base`, rating by rating (so its total
/// may stand above the cap as the base roster's own does).
pub fn within_base(p: &Player, base: &Player) -> bool {
    p.ratings[..TOTALLED]
        .iter()
        .zip(&base.ratings[..TOTALLED])
        .all(|(a, b)| a <= b)
}

/// A record's rating total ([`TOTALLED`] ratings).
pub fn total(p: &Player) -> i32 {
    p.ratings[..TOTALLED].iter().map(|&r| i32::from(r)).sum()
}

/// Whether a record starting from `source` is a created baller (or has no
/// Vol. 2 data: GameCube characters start from the create-a-player record),
/// held to Create A Baller's rules.
pub fn is_created(source: usize) -> bool {
    (TEMPLATE_RECORD..FIRST_ADDED).contains(&source)
}

/// What a record is held to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    /// A Vol. 2 character: the base roster's cap.
    Real,
    /// A created baller: Create A Baller's ratings and its build's sizes.
    Created,
    /// A GameCube character (`v3gc:`, `v1gc:`): its ratings are the
    /// create-a-player record's, held to Create A Baller's; its size, from
    /// its own disc, to the general bounds.
    GameCube,
}

impl Held {
    /// A record starting from `source`, a GameCube character if `gamecube`.
    pub fn of(source: usize, gamecube: bool) -> Self {
        match (is_created(source), gamecube) {
            (false, _) => Self::Real,
            (true, false) => Self::Created,
            (true, true) => Self::GameCube,
        }
    }
}

/// The value rules on a whole record; `base` is its unedited record in the
/// base roster, if known (a record no better than it may stand above the
/// rating-total cap).
pub fn check_values(
    p: &Player,
    held: Held,
    limits: &Limits,
    base: Option<&Player>,
) -> Vec<Problem> {
    let created = held != Held::Real;
    let mut out = Vec::new();
    let range =
        |out: &mut Vec<Problem>, field: usize, value: i16, allowed: [i16; 2], what: &str| {
            if !(allowed[0]..=allowed[1]).contains(&value) {
                out.push(Problem::range(
                    field_name(field),
                    i64::from(value),
                    [i64::from(allowed[0]), i64::from(allowed[1])],
                    what,
                ));
            }
        };
    let f = crate::roster_share::fields_of(p);
    range(&mut out, 8, p.height_in, HEIGHT_IN, "height");
    range(&mut out, 9, p.weight_lb, WEIGHT_LB, "weight");
    for (k, &bounds) in BODY_SCALE.iter().enumerate() {
        range(&mut out, 43 + k, p.body_scale[k], bounds, "body scale");
    }
    range(&mut out, 30, p.build, BUILD, "build");
    let caps = if created {
        CAB_RATING_CAPS
    } else {
        [RATING[1]; 11]
    };
    for (k, &r) in p.ratings.iter().enumerate() {
        range(&mut out, 16 + k, r, [RATING[0], caps[k]], "rating");
    }
    // The drawn size follows the shown one (no hidden giant model).
    range(
        &mut out,
        10,
        p.model_height_in,
        MODEL_HEIGHT_OFFSET.map(|d| p.height_in.saturating_add(d)),
        "model height (the shown height's, give or take 2)",
    );
    range(
        &mut out,
        11,
        p.model_weight_lb,
        MODEL_WEIGHT_OFFSET.map(|d| p.weight_lb.saturating_add(d)),
        "model weight (the shown weight's, up to 30 more)",
    );
    let h = i32::from(p.height_in.max(1));
    let per_inch = i32::from(p.weight_lb) * 100 / h;
    if (HEIGHT_IN[0]..=HEIGHT_IN[1]).contains(&p.height_in)
        && (WEIGHT_LB[0]..=WEIGHT_LB[1]).contains(&p.weight_lb)
        && !(LB_PER_INCH_X100[0]..=LB_PER_INCH_X100[1]).contains(&per_inch)
    {
        let lo = (LB_PER_INCH_X100[0] * h + 99) / 100;
        let hi = LB_PER_INCH_X100[1] * h / 100;
        out.push(Problem {
            allowed: Some([i64::from(lo), i64::from(hi)]),
            ..Problem::new(
                "weightLb",
                p.weight_lb,
                format!(
                    "{} lb is not a plausible weight at {} in ({lo}..{hi} lb)",
                    p.weight_lb, p.height_in
                ),
            )
        });
    }
    let t = total(p);
    if created {
        if t > CAB_TOTAL_CAP {
            out.push(Problem {
                allowed: Some([0, i64::from(CAB_TOTAL_CAP)]),
                ..Problem::new(
                    "ratings",
                    t,
                    format!(
                        "the ratings total {t} (shots..speed), over Create A Baller's {CAB_TOTAL_CAP}"
                    ),
                )
            });
        }
    } else if t > limits.total_cap && !limits.trusted && !base.is_some_and(|b| within_base(p, b)) {
        let cap = limits.total_cap.max(base.map_or(0, total));
        out.push(Problem {
            allowed: Some([0, i64::from(cap)]),
            ..Problem::new(
                "ratings",
                t,
                format!(
                    "the ratings total {t} (shots..speed) is over the cap of {} (the base \
                     roster's best real player){}",
                    limits.total_cap,
                    match base {
                        Some(b) if total(b) > limits.total_cap => format!(
                            "; past it, no rating may rise above the base record's ({} in all)",
                            total(b)
                        ),
                        _ => String::new(),
                    }
                ),
            )
        });
    }
    if held == Held::Created {
        let build = usize::try_from(p.build).ok().and_then(|b| CAB_SIZES.get(b));
        if let Some(&[h0, h1, w0, w1]) = build {
            range(
                &mut out,
                8,
                p.height_in,
                [h0, h1],
                "a created baller's height for its build:",
            );
            range(
                &mut out,
                9,
                p.weight_lb,
                [w0, w1],
                "a created baller's weight for its build:",
            );
        }
    }
    for (kind, &(first, n)) in MOVE_FIELDS.iter().enumerate() {
        let rows: Vec<i32> = f[first..first + n].iter().map(|&v| i32::from(v)).collect();
        out.extend(check_move_list(kind, &rows, true));
    }
    out
}

/// The fields of `p` that must equal `source`'s and do not.
pub fn check_source(p: &Player, source: &Player) -> Vec<Problem> {
    let (a, b) = (
        crate::roster_share::fields_of(source),
        crate::roster_share::fields_of(p),
    );
    (0..FIELDS)
        .filter(|&k| field_rule(k) == FieldRule::Source && a[k] != b[k])
        .map(|k| Problem {
            allowed: Some([i64::from(a[k]); 2]),
            ..Problem::new(
                field_name(k),
                b[k],
                format!(
                    "field {k} may not change online (the source record has {})",
                    a[k]
                ),
            )
        })
        .collect()
}

/// An equipped list of `kind` (`-1` or [`EMPTY`] for a vacant slot):
/// positionally (slot by slot), or compact (each row placed in the first
/// free slot of its level, as older saves keep them).
pub fn check_move_list(kind: usize, rows: &[i32], positional: bool) -> Vec<Problem> {
    let name = KIND_NAMES[kind];
    let mut out = Vec::new();
    if rows.len() > MOVE_SLOTS[kind] {
        out.push(Problem {
            allowed: Some([0, MOVE_SLOTS[kind] as i64]),
            ..Problem::new(
                format!("moves.{name}"),
                rows.len(),
                format!("{} {name} for {} slots", rows.len(), MOVE_SLOTS[kind]),
            )
        });
    }
    let mut seen = BTreeSet::new();
    let mut free = SLOT_LEVELS;
    for (slot, &row) in rows.iter().enumerate().take(MOVE_SLOTS[kind]) {
        if row == -1 || row == i32::from(EMPTY) {
            continue;
        }
        let field = format!("moves.{name}[{slot}]");
        let Some((k, level)) = usize::try_from(row).ok().and_then(move_row) else {
            out.push(Problem {
                allowed: Some([0, MOVE_COUNT as i64 - 1]),
                ..Problem::new(field, row, format!("{row} is not a move row"))
            });
            continue;
        };
        if usize::from(k) != kind {
            out.push(Problem::new(
                field,
                row,
                format!("move row {row} is not one of the {name}"),
            ));
            continue;
        }
        if !seen.insert(row) {
            out.push(Problem::new(
                field,
                row,
                format!("move row {row} is in two {name} slots"),
            ));
            continue;
        }
        if kind == 0 {
            continue;
        }
        if positional {
            if slot_level(kind, slot) != Some(level) {
                out.push(Problem::new(
                    field,
                    row,
                    format!(
                        "move row {row} is level {level}; {name} slot {slot} takes level {}",
                        slot_level(kind, slot).unwrap_or(0)
                    ),
                ));
            }
        } else if let Some(at) = free.iter().position(|&l| l == level) {
            free[at] = 0;
        } else {
            out.push(Problem::new(
                field,
                row,
                format!("no {name} slot of level {level} left for row {row}"),
            ));
        }
    }
    out
}

/// Team lists: existing records, each once per team; sane names.
pub fn check_teams(teams: &[Team], players: usize) -> Vec<Problem> {
    let mut out = Vec::new();
    if teams.len() > MAX_TEAMS {
        out.push(Problem {
            allowed: Some([0, MAX_TEAMS as i64]),
            ..Problem::new(
                "teams",
                teams.len(),
                format!("{} teams (at most {MAX_TEAMS})", teams.len()),
            )
        });
    }
    for (k, t) in teams.iter().enumerate() {
        let field = format!("teams[{k}]");
        if t.index != k {
            out.push(Problem::new(&field, t.index, "a team out of order"));
        }
        let mut seen = BTreeSet::new();
        for &r in &t.roster {
            if r >= players {
                out.push(Problem {
                    allowed: Some([0, players as i64 - 1]),
                    ..Problem::new(
                        &field,
                        r,
                        format!("{} lists record {r}, past the {players} records", t.name),
                    )
                });
            } else if !seen.insert(r) {
                out.push(Problem::new(
                    &field,
                    r,
                    format!("{} lists record {r} twice", t.name),
                ));
            }
        }
        if t.roster.len() > MAX_ROSTER_PLAYERS {
            out.push(Problem::new(&field, t.roster.len(), "a team list too long"));
        }
        for s in [&t.name, &t.city, &t.nickname] {
            if s.chars().count() > crate::roster_share::MAX_NAME || s.chars().any(char::is_control)
            {
                out.push(Problem::new(&field, s.as_str(), "a team name too long"));
            }
        }
    }
    out
}

fn record_counts(players: usize) -> Vec<Problem> {
    if !(DISC_RECORDS..=MAX_RECORDS).contains(&players) {
        return vec![Problem {
            allowed: Some([DISC_RECORDS as i64, MAX_RECORDS as i64]),
            ..Problem::new(
                "players",
                players,
                format!(
                    "{players} player records; added players go up to {MAX_ADDED} past the career stars"
                ),
            )
        }];
    }
    Vec::new()
}

/// What `check-roster` prints.
#[derive(Clone, Debug)]
pub struct Report {
    pub problems: Vec<Problem>,
    pub fingerprint: String,
    /// The base roster's rating-total cap.
    pub rating_total_cap: i32,
    /// Each `roster.json` player's total and its base record's.
    pub players: Vec<PlayerTotal>,
}

/// One `roster.json` player's rating total.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerTotal {
    pub player: usize,
    pub record: usize,
    pub character: String,
    pub total: i32,
    /// Its starting character's total in the base roster.
    pub base_total: i32,
}

impl Report {
    pub fn legal(&self) -> bool {
        self.problems.is_empty()
    }

    pub fn to_json(&self) -> Value {
        json!({
            "version": 1,
            "legal": self.legal(),
            "fingerprint": self.fingerprint,
            "ratingTotalCap": self.rating_total_cap,
            "players": self.players.iter().map(|p| json!({
                "player": p.player, "record": p.record, "character": p.character,
                "total": p.total, "baseTotal": p.base_total,
            })).collect::<Vec<_>>(),
            "problems": self.problems.iter().map(Problem::to_json).collect::<Vec<_>>(),
        })
    }
}

/// Check `roster.json` applied over `base` (the disc's tables or a mod's),
/// with the save's created ballers; `disc` is the disc's own tables (for
/// the fingerprint, which [`SharedRoster`] defines). Errs when the file
/// cannot be applied at all.
pub fn check_file(
    file: &RosterFile,
    base: (&[Player], &[Team]),
    disc: (&[Player], &[Team]),
    created: &BTreeMap<usize, Created>,
    base_kind: Base,
) -> Result<Report, String> {
    let limits = Limits::from_players(base.0);
    let mut totals = Vec::new();
    let mut problems = Vec::new();
    if file.players.len() > MAX_ROSTER_PLAYERS {
        problems.push(Problem {
            allowed: Some([0, MAX_ROSTER_PLAYERS as i64]),
            ..Problem::new(
                "players",
                file.players.len(),
                format!(
                    "{} players (at most {MAX_ROSTER_PLAYERS})",
                    file.players.len()
                ),
            )
        });
    }
    let added = file.players.iter().filter(|p| p.slot.is_none()).count();
    if added > MAX_ADDED {
        problems.push(Problem {
            allowed: Some([0, MAX_ADDED as i64]),
            ..Problem::new(
                "players",
                added,
                format!("{added} added players (at most {MAX_ADDED})"),
            )
        });
    }
    let mut slots = BTreeSet::new();
    for (k, p) in file.players.iter().enumerate() {
        if let Some(s) = p.slot
            && !slots.insert(s)
        {
            problems.push(Problem {
                player: Some(k),
                character: Some(p.character.clone()),
                ..Problem::new("slot", s, format!("slot {s} is replaced twice"))
            });
        }
    }
    let mut players = base.0.to_vec();
    let mut teams = base.1.to_vec();
    let applied = roster_edits::apply(file, &mut players, created)?;
    roster_edits::apply_teams(file, &mut teams)?;
    problems.extend(record_counts(players.len()));
    let template = base
        .0
        .get(TEMPLATE_RECORD)
        .ok_or("no create-a-player record")?;
    let mut hints = BTreeMap::new();
    for (k, (entry, &record)) in file.players.iter().zip(&file.records()).enumerate() {
        let id = CharId::parse(&entry.character)?;
        let source = match &id {
            CharId::Vol2(r) => base.0.get(*r).unwrap_or(template),
            CharId::Created(n) => created.get(n).map_or(template, |c| &c.player),
            CharId::GameCube(_) => template,
        };
        if let Some(r) = id.record().filter(|&r| r < DISC_RECORDS) {
            hints.insert(record, r);
        }
        let from = id.record().unwrap_or(TEMPLATE_RECORD);
        let p = &players[record];
        totals.push(PlayerTotal {
            player: k,
            record,
            character: entry.character.clone(),
            total: total(p),
            base_total: total(source),
        });
        let held = Held::of(from, matches!(id, CharId::GameCube(_)));
        let mut found = check_values(p, held, &limits, Some(source));
        found.extend(check_source(p, source));
        if let Some(moves) = entry.animations.as_ref().and_then(|a| a.moves.as_ref()) {
            for (kind, list) in [&moves.layups, &moves.dunks, &moves.tricks]
                .into_iter()
                .enumerate()
            {
                if let Some(list) = list {
                    found.extend(check_move_list(kind, list, true));
                }
            }
        }
        for mut problem in found {
            problem.player = Some(k);
            problem.record = Some(record);
            problem.character = Some(entry.character.clone());
            problems.push(problem);
        }
    }
    for (slot, c) in created {
        for (kind, rows) in c.moves.iter().enumerate() {
            let rows: Vec<i32> = rows.iter().map(|&r| i32::from(r)).collect();
            for mut problem in check_move_list(kind, &rows, rows.len() == MOVE_SLOTS[kind]) {
                problem.character = Some(format!("created:{slot}"));
                problems.push(problem);
            }
        }
    }
    problems.extend(check_teams(&teams, players.len()));
    let created_moves = created
        .iter()
        .map(|(&slot, c)| (slot, c.moves.clone()))
        .collect();
    let shared = SharedRoster::diff(
        base_kind,
        disc,
        (&players, &teams),
        &applied,
        &created_moves,
        &hints,
    );
    // The roster as a match shares it (what the game and a server check):
    // the base roster's own records too, the roster.json players' already
    // checked above.
    let named: BTreeSet<usize> = file.records().into_iter().collect();
    problems.extend(
        check_shared(
            &shared,
            &Context {
                disc: Some(disc),
                base: Some(base.0),
                limits,
            },
        )
        .into_iter()
        .filter(|p| p.record.is_none_or(|r| !named.contains(&r)))
        .filter(|p| !p.field.starts_with("teams")),
    );
    Ok(Report {
        problems,
        fingerprint: shared.fingerprint(),
        rating_total_cap: limits.total_cap,
        players: totals,
    })
}

/// The rules on one shared record that need no disc: only fields a roster
/// may change differ from the source, each within its bounds.
pub fn check_shared_player(p: &SharedPlayer) -> Vec<Problem> {
    let mut out = Vec::new();
    if usize::from(p.source) >= DISC_RECORDS {
        out.push(Problem::range(
            "source",
            i64::from(p.source),
            [0, DISC_RECORDS as i64 - 1],
            "source record",
        ));
    }
    for &(k, v) in &p.fields {
        let f = usize::from(k);
        let bounded = |allowed: [i16; 2], what: &str| {
            (!(allowed[0]..=allowed[1]).contains(&v)).then(|| {
                Problem::range(
                    field_name(f),
                    i64::from(v),
                    [i64::from(allowed[0]), i64::from(allowed[1])],
                    what,
                )
            })
        };
        let problem = match field_rule(f) {
            FieldRule::Cosmetic => None,
            FieldRule::Source => Some(Problem::new(
                field_name(f),
                v,
                format!("field {f} may not change online"),
            )),
            FieldRule::Bounded => match f {
                8 => bounded(HEIGHT_IN, "height"),
                9 => bounded(WEIGHT_LB, "weight"),
                // Offsets from the shown size need the whole record.
                10 => bounded(
                    [
                        HEIGHT_IN[0] + MODEL_HEIGHT_OFFSET[0],
                        HEIGHT_IN[1] + MODEL_HEIGHT_OFFSET[1],
                    ],
                    "model height",
                ),
                11 => bounded(
                    [WEIGHT_LB[0], WEIGHT_LB[1] + MODEL_WEIGHT_OFFSET[1]],
                    "model weight",
                ),
                30 => bounded(BUILD, "build"),
                43..=45 => bounded(BODY_SCALE[f - 43], "body scale"),
                _ => bounded(RATING, "rating"),
            },
            FieldRule::Move => {
                let (kind, &(first, _)) = MOVE_FIELDS
                    .iter()
                    .enumerate()
                    .find(|(_, (first, n))| (*first..first + n).contains(&f))
                    .expect("a move field");
                let mut rows = vec![-1; f - first + 1];
                rows[f - first] = i32::from(v);
                check_move_list(kind, &rows, true).into_iter().next()
            }
        };
        out.extend(problem);
    }
    out.into_iter()
        .map(|q| q.on(usize::from(p.record)))
        .collect()
}

/// The rules on a shared roster. Without the disc, what the differences
/// alone show ([`check_shared_player`], move lists, counts, team lists);
/// with it, every changed record rebuilt and checked whole (against its
/// base record, when the base roster is known).
pub fn check_shared(shared: &SharedRoster, ctx: &Context) -> Vec<Problem> {
    let limits = ctx.limits;
    let players = usize::from(shared.players_len);
    let mut out = record_counts(players);
    if usize::from(shared.teams_len) > MAX_TEAMS {
        out.push(Problem::range(
            "teams",
            i64::from(shared.teams_len),
            [0, MAX_TEAMS as i64],
            "teams",
        ));
    }
    for p in &shared.players {
        if usize::from(p.record) >= players {
            out.push(Problem::range(
                "record",
                i64::from(p.record),
                [0, players as i64 - 1],
                "record",
            ));
        }
        out.extend(check_shared_player(p));
    }
    for (r, look) in &shared.looks {
        if let crate::roster_share::SharedLook::Wear(p) = look {
            out.extend(
                check_shared_player(p)
                    .into_iter()
                    .map(|q| q.on(usize::from(*r))),
            );
        }
    }
    for (r, a) in &shared.anims {
        for (what, record) in [
            ("moves source", a.moves_record),
            ("shot source", a.shot_record),
        ] {
            if record >= players.max(FIRST_ADDED) {
                out.push(
                    Problem::range("animations", record as i64, [0, players as i64 - 1], what)
                        .on(usize::from(*r)),
                );
            }
        }
        for (kind, rows) in a.rows.iter().enumerate() {
            if let Some(rows) = rows {
                let rows: Vec<i32> = rows.iter().map(|&r| i32::from(r)).collect();
                out.extend(
                    check_move_list(kind, &rows, true)
                        .into_iter()
                        .map(|q| q.on(usize::from(*r))),
                );
            }
        }
    }
    for (slot, moves) in &shared.created {
        if usize::from(*slot) >= CREATED_SLOTS {
            out.push(Problem::range(
                "created",
                i64::from(*slot),
                [0, CREATED_SLOTS as i64 - 1],
                "created baller slot",
            ));
        }
        for (kind, rows) in moves.iter().enumerate() {
            let rows: Vec<i32> = rows.iter().map(|&r| i32::from(r)).collect();
            out.extend(check_move_list(kind, &rows, rows.len() == MOVE_SLOTS[kind]));
        }
    }
    match ctx.disc {
        None => {
            for t in &shared.teams {
                if t.index >= usize::from(shared.teams_len) {
                    out.push(Problem::new("teams", t.index, "a team past the table"));
                }
                out.extend(
                    check_teams(std::slice::from_ref(t), players)
                        .into_iter()
                        .filter(|p| !p.message.contains("out of order")),
                );
            }
        }
        Some((dp, dt)) => {
            match shared.rebuild_players(dp) {
                Ok((rebuilt, _)) => {
                    for sp in &shared.players {
                        let p = &rebuilt[usize::from(sp.record)];
                        let gamecube = shared.gamecube.iter().any(|(r, _)| *r == sp.record);
                        let held = Held::of(usize::from(sp.source), gamecube);
                        let base = ctx.base.and_then(|b| b.get(usize::from(sp.source)));
                        out.extend(
                            check_values(p, held, &limits, base)
                                .into_iter()
                                .map(|q| q.on(usize::from(sp.record))),
                        );
                    }
                }
                Err(e) => out.push(Problem::new("players", Value::Null, e)),
            }
            match shared.rebuild_teams(dt) {
                Ok(teams) => out.extend(check_teams(&teams, players)),
                Err(e) => out.push(Problem::new("teams", Value::Null, e)),
            }
        }
    }
    out
}

/// The rules on what a match sends: the roster ([`check_shared`]) and the
/// line-up's records, which must be legal players and, with the disc, the
/// rebuilt roster's own.
pub fn check_match(m: &MatchRoster, records: &[usize], ctx: &Context) -> Vec<Problem> {
    let mut out = check_shared(&m.roster, ctx);
    let limits = ctx.limits;
    let disc = ctx.disc;
    if m.lineup.len() != records.len() {
        out.push(Problem::new(
            "lineup",
            m.lineup.len(),
            format!(
                "{} line-up records for {} players",
                m.lineup.len(),
                records.len()
            ),
        ));
        return out;
    }
    let rebuilt = disc.and_then(|(dp, _)| m.roster.rebuild_players(dp).ok());
    for (p, &record) in m.lineup.iter().zip(records) {
        if p.index != record {
            out.push(
                Problem::new(
                    "lineup",
                    p.index,
                    format!("the line-up sends record {} for {record}", p.index),
                )
                .on(record),
            );
            continue;
        }
        let shared = m
            .roster
            .players
            .iter()
            .find(|s| usize::from(s.record) == record);
        // A created record the roster leaves alone is the creator's saved
        // baller (or the disc's template): Create A Baller's rules.
        let gamecube = m
            .roster
            .gamecube
            .iter()
            .any(|(r, _)| usize::from(*r) == record);
        let held = match shared {
            Some(s) => Held::of(usize::from(s.source), gamecube),
            None if (TEMPLATE_RECORD..FIRST_ADDED).contains(&record) => Held::Created,
            None => Held::Real,
        };
        let created = match shared {
            Some(s) => is_created(usize::from(s.source)),
            None => (TEMPLATE_RECORD..FIRST_ADDED).contains(&record),
        };
        let source = shared.map_or(record, |s| usize::from(s.source));
        let base = ctx.base.and_then(|b| b.get(source));
        out.extend(
            check_values(p, held, &limits, base)
                .into_iter()
                .map(|q| q.on(record)),
        );
        if let Some(rebuilt) = &rebuilt
            && !created
        {
            match rebuilt.0.get(record) {
                Some(r)
                    if crate::roster_share::fields_of(r) == crate::roster_share::fields_of(p)
                        && r.name() == p.name() => {}
                _ => out.push(
                    Problem::new(
                        "lineup",
                        record,
                        "the line-up's record is not the roster's (on this disc)",
                    )
                    .on(record),
                ),
            }
        }
    }
    out
}

/// The catalog's `rules` object: what the builder clamps to.
pub fn rules_json(limits: &Limits) -> Value {
    let cosmetic: Vec<usize> = (0..FIELDS)
        .filter(|&f| field_rule(f) == FieldRule::Cosmetic)
        .collect();
    let bounded: Vec<usize> = (0..FIELDS)
        .filter(|&f| field_rule(f) == FieldRule::Bounded)
        .collect();
    let fixed: Vec<usize> = (0..FIELDS)
        .filter(|&f| field_rule(f) == FieldRule::Source)
        .collect();
    json!({
        "version": 1,
        "heightIn": HEIGHT_IN,
        "weightLb": WEIGHT_LB,
        "lbPerInch": [f64::from(LB_PER_INCH_X100[0]) / 100.0, f64::from(LB_PER_INCH_X100[1]) / 100.0],
        "modelHeightOffset": MODEL_HEIGHT_OFFSET,
        "modelWeightOffset": MODEL_WEIGHT_OFFSET,
        "bodyScale": BODY_SCALE,
        "build": BUILD,
        "rating": RATING,
        "ratingTotalCap": limits.total_cap,
        "ratingTotal": { "ratings": TOTALLED, "cap": limits.total_cap,
                         "capOriginal": ORIGINAL_TOTAL_CAP, "capLegendary": LEGENDARY_TOTAL_CAP,
                         "capRecords": [0, REAL_CHARACTERS - 1],
                         "rule": "total <= cap, or no rating above the character's own base record's (characters[].baseTotal)" },
        "created": {
            "records": [TEMPLATE_RECORD, FIRST_ADDED - 1],
            "ratingCaps": CAB_RATING_CAPS,
            "totalCap": CAB_TOTAL_CAP,
            "sizes": CAB_SIZES.iter().map(|s| json!({
                "heightIn": [s[0], s[1]], "weightLb": [s[2], s[3]] })).collect::<Vec<_>>(),
        },
        "moves": {
            "slots": { "layups": MOVE_SLOTS[0], "dunks": MOVE_SLOTS[1], "tricks": MOVE_SLOTS[2] },
            "slotLevels": SLOT_LEVELS,
            "rows": MOVE_ROWS.iter().map(|&(kind, level, first, last)| json!({
                "kind": KIND_NAMES[usize::from(kind)], "level": level, "rows": [first, last] }))
                .collect::<Vec<_>>(),
            "unique": true,
        },
        "editableOnline": editable_fields(),
        "fields": { "cosmetic": cosmetic, "bounded": bounded, "moves": [[68, 73], [79, 100]],
                    "fixed": fixed },
        "fieldNames": (0..FIELDS).map(field_name).collect::<Vec<_>>(),
        "maxAddedPlayers": MAX_ADDED,
        "maxPlayers": MAX_ROSTER_PLAYERS,
        "maxRecords": MAX_RECORDS,
        "maxTeams": MAX_TEAMS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roster_share::{blank, with_fields};

    /// A plain legal record.
    fn player(index: usize) -> Player {
        let mut f = vec![0i16; FIELDS];
        f[3] = (index / 5).min(48) as i16;
        f[8] = 78;
        f[9] = 215;
        f[10] = 78;
        f[11] = 215;
        f[16..27].copy_from_slice(&[60; 11]);
        for (first, n) in MOVE_FIELDS {
            f[first..first + n].fill(-1);
        }
        f[74..79].fill(-1);
        let mut p = with_fields(blank(index), f);
        p.first_name = format!("F{index}");
        p
    }

    fn tables() -> (Vec<Player>, Vec<Team>) {
        let mut players: Vec<Player> = (0..DISC_RECORDS).map(player).collect();
        // The best real character: 8 x 78 + 74 = 698.
        let best = [78, 78, 78, 78, 78, 78, 78, 78, 74, 0, 0];
        players[146].ratings = best;
        players[146].fields[16..27].copy_from_slice(&best);
        // Create-a-player templates: within Create A Baller.
        for p in &mut players[TEMPLATE_RECORD..] {
            p.ratings = [30; 11];
            p.build = 1;
            p.height_in = 76;
            p.model_height_in = 76;
            p.weight_lb = 220;
            p.model_weight_lb = 220;
            let f = crate::roster_share::fields_of(p);
            *p = with_fields(p.clone(), f);
        }
        let teams = (0..49)
            .map(|t| Team {
                index: t,
                name: format!("T{t}"),
                city: format!("C{t}"),
                nickname: format!("N{t}"),
                roster: (5 * t..(5 * t + 5).min(DISC_RECORDS)).collect(),
                logo: t as i16,
            })
            .collect();
        (players, teams)
    }

    fn check(json: &str) -> Report {
        let (p, t) = tables();
        let file = RosterFile::parse(json).unwrap();
        check_file(&file, (&p, &t), (&p, &t), &BTreeMap::new(), Base::Original).unwrap()
    }

    fn fields(r: &Report) -> Vec<String> {
        r.problems.iter().map(|p| p.field.clone()).collect()
    }

    #[test]
    fn plain_edits_and_added_players_are_legal() {
        let r = check(
            r#"{"version": 1, "players": [
                {"slot": 0, "character": "vol2:0",
                 "edits": {"firstName": "Mike", "heightIn": 80, "ratings": [90, 80, 70],
                           "accessories": [1, 2], "raw": {"14": 3, "6": 2}},
                 "animations": {"moves": {"dunks": [49, 26, -1, -1, -1, 10], "layups": [0, 5]}}},
                {"slot": null, "character": "vol2:158", "team": "vol2:3"},
                {"slot": null, "character": "created:0", "team": "vol2:3"}]}"#,
        );
        assert!(r.legal(), "{:?}", r.problems);
        assert_eq!(r.fingerprint.len(), 64);
        // The same roster, the same fingerprint; another, another.
        let again = check(
            r#"{"version": 1, "players": [
                {"slot": 0, "character": "vol2:0",
                 "edits": {"firstName": "Mike", "heightIn": 80, "ratings": [90, 80, 70],
                           "accessories": [1, 2], "raw": {"14": 3, "6": 2}},
                 "animations": {"moves": {"dunks": [49, 26, -1, -1, -1, 10], "layups": [0, 5]}}},
                {"slot": null, "character": "vol2:158", "team": "vol2:3"},
                {"slot": null, "character": "created:0", "team": "vol2:3"}]}"#,
        );
        assert_eq!(again.fingerprint, r.fingerprint);
        let other = check(r#"{"players": [{"slot": 0, "character": "vol2:0"}]}"#);
        assert_ne!(other.fingerprint, r.fingerprint);
    }

    #[test]
    fn giants_heavyweights_and_super_ratings_are_not() {
        let one = |edits: &str| {
            check(&format!(
                r#"{{"players": [{{"slot": 0, "character": "vol2:0", "edits": {edits}}}]}}"#
            ))
        };
        let r = one(r#"{"heightIn": 96}"#);
        let p = &r.problems[0];
        assert_eq!(
            (p.player, p.field.as_str(), p.allowed),
            (Some(0), "heightIn", Some([63, 90]))
        );
        assert_eq!(p.value, json!(96));
        assert_eq!(p.character.as_deref(), Some("vol2:0"));
        assert!(fields(&one(r#"{"weightLb": 400}"#)).contains(&"weightLb".to_string()));
        // A light giant: in range on its own, not for the height.
        let r = one(r#"{"heightIn": 88, "weightLb": 120}"#);
        assert_eq!(fields(&r), vec!["weightLb"]);
        assert!(r.problems[0].message.contains("plausible"));
        let r = one(r#"{"ratings": [120]}"#);
        assert!(
            fields(&r).contains(&"ratings[0]".to_string()),
            "{:?}",
            r.problems
        );
        // Every rating legal, the total over the best real character's.
        let r = one(r#"{"ratings": [99, 99, 99, 99, 99, 99, 99, 99, 99]}"#);
        assert_eq!(fields(&r), vec!["ratings"]);
        assert_eq!(r.problems[0].allowed, Some([0, 698]));
        // A hidden giant model.
        let r = one(r#"{"modelHeightIn": 90}"#);
        assert_eq!(fields(&r), vec!["modelHeightIn"]);
        let r = one(r#"{"heightIn": 70, "modelHeightIn": 70, "modelWeightLb": 300}"#);
        assert_eq!(fields(&r), vec!["modelWeightLb"]);
        // A body scale past the disc's.
        assert_eq!(fields(&one(r#"{"bodyScale": [40]}"#)), vec!["bodyScale[0]"]);
        // A field no roster may change.
        let r = one(r#"{"raw": {"33": 4}}"#);
        assert_eq!(fields(&r), vec!["raw.33"]);
        assert_eq!(r.problems[0].allowed, Some([0, 0]));
    }

    #[test]
    fn moves_must_fit_their_slots() {
        let moves = |m: &str| {
            check(&format!(
                r#"{{"players": [{{"slot": 0, "character": "vol2:0",
                     "animations": {{"moves": {m}}}}}]}}"#
            ))
        };
        // A dribble move in a dunk slot.
        let r = moves(r#"{"dunks": [60]}"#);
        assert_eq!(fields(&r), vec!["moves.dunks[0]"]);
        // A level-2 dunk in the level-4 slot, a row twice, a row past the table.
        assert!(!moves(r#"{"dunks": [10]}"#).legal());
        assert!(!moves(r#"{"tricks": [-1, 68, 68]}"#).legal());
        assert!(!moves(r#"{"layups": [93]}"#).legal());
        assert!(!moves(r#"{"layups": [0, 1, 2, 3, 4, 5, 6]}"#).legal());
        assert!(moves(r#"{"tricks": [89, 68, 69, 70, 71, 54]}"#).legal());
        // Move fields in the record, the same way.
        let r = check(
            r#"{"players": [{"slot": 0, "character": "vol2:0", "edits": {"raw": {"79": 60}}}]}"#,
        );
        assert_eq!(fields(&r), vec!["moves.dunks[0]"]);
    }

    #[test]
    fn created_ballers_keep_to_create_a_baller() {
        let r = check(
            r#"{"players": [{"slot": null, "character": "created:1", "team": "vol2:0",
                "edits": {"ratings": [98, 90, 98, 98, 98, 98, 98, 98, 85]}}]}"#,
        );
        assert!(r.legal(), "{:?}", r.problems);
        let r = check(
            r#"{"players": [{"slot": null, "character": "created:1", "team": "vol2:0",
                "edits": {"ratings": [99], "heightIn": 88, "modelHeightIn": 88}}]}"#,
        );
        assert_eq!(fields(&r), vec!["ratings[0]", "heightIn"]);
    }

    #[test]
    fn gamecube_characters_keep_their_own_size() {
        // A short GameCube character from the create-a-player record: its
        // height is no Create A Baller size, but a legal one.
        let r = check(
            r#"{"players": [{"slot": null, "character": "v3gc:mario", "team": "vol2:0",
                "edits": {"heightIn": 64, "weightLb": 150}}]}"#,
        );
        assert!(r.legal(), "{:?}", r.problems);
        let r = check(
            r#"{"players": [{"slot": null, "character": "v3gc:mario", "team": "vol2:0",
                "edits": {"heightIn": 64, "weightLb": 150, "ratings": [99]}}]}"#,
        );
        assert_eq!(fields(&r), vec!["ratings[0]"]);
        let r = check(
            r#"{"players": [{"slot": null, "character": "created:0", "team": "vol2:0",
                "edits": {"heightIn": 64, "weightLb": 150}}]}"#,
        );
        assert!(!r.legal());
    }

    #[test]
    fn counts_and_teams_are_bounded() {
        let many: Vec<String> = (0..=MAX_ADDED)
            .map(|_| r#"{"slot": null, "character": "vol2:1"}"#.to_string())
            .collect();
        let (p, t) = tables();
        let file = RosterFile::parse(&format!(r#"{{"players": [{}]}}"#, many.join(","))).unwrap();
        let r = check_file(&file, (&p, &t), (&p, &t), &BTreeMap::new(), Base::Original).unwrap();
        assert!(fields(&r).contains(&"players".to_string()));
        let r = check(
            r#"{"players": [{"slot": 0, "character": "vol2:0"}, {"slot": 0, "character": "vol2:1"}]}"#,
        );
        assert_eq!(fields(&r), vec!["slot"]);
        let mut teams = t.clone();
        teams[2].roster.push(10);
        teams[3].roster.push(900);
        let found = check_teams(&teams, p.len());
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn a_shared_roster_is_checked_without_the_disc() {
        let shared = |fields: Vec<(u8, i16)>| SharedRoster {
            players_len: DISC_RECORDS as u16,
            teams_len: 49,
            players: vec![SharedPlayer {
                record: 0,
                source: 0,
                fields,
                ..Default::default()
            }],
            ..Default::default()
        };
        let none = Context::default();
        assert!(check_shared(&shared(vec![(8, 80), (16, 99), (14, 5)]), &none).is_empty());
        for bad in [
            vec![(8, 96)],
            vec![(9, 400)],
            vec![(16, 120)],
            vec![(33, 1)],
            vec![(79, 60)],
            vec![(43, 50)],
        ] {
            assert!(
                !check_shared(&shared(bad.clone()), &none).is_empty(),
                "{bad:?}"
            );
        }
        // With the disc: whole records, so totals and offsets too.
        let (p, t) = tables();
        let ratings: Vec<(u8, i16)> = (16..25).map(|f| (f, 99)).collect();
        assert!(check_shared(&shared(ratings.clone()), &none).is_empty());
        let disc = Context {
            disc: Some((&p, &t)),
            base: Some(&p),
            limits: Limits::default(),
        };
        let found = check_shared(&shared(ratings), &disc);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].field, "ratings");
        let found = check_shared(&shared(vec![(10, 88)]), &disc);
        assert_eq!(found[0].field, "modelHeightIn");
    }

    #[test]
    fn a_match_lineup_must_be_legal_and_the_rosters() {
        let (p, t) = tables();
        let lineup: Vec<Player> = [0, 1, 2, 3, 4, 236].iter().map(|&r| p[r].clone()).collect();
        let records: Vec<usize> = lineup.iter().map(|p| p.index).collect();
        let m = MatchRoster {
            roster: SharedRoster {
                players_len: DISC_RECORDS as u16,
                teams_len: 49,
                ..Default::default()
            },
            lineup,
        };
        let none = Context::default();
        let disc = Context {
            disc: Some((&p, &t)),
            base: Some(&p),
            limits: Limits::default(),
        };
        assert!(check_match(&m, &records, &none).is_empty());
        assert!(check_match(&m, &records, &disc).is_empty());
        // A record that claims better ratings than the disc's: values alone
        // pass, the disc tells.
        let mut lie = m.clone();
        lie.lineup[0].ratings[0] = 90;
        lie.lineup[0].fields[16] = 90;
        assert!(check_match(&lie, &records, &none).is_empty());
        assert!(!check_match(&lie, &records, &disc).is_empty());
        // A godlike one fails either way.
        let mut god = m.clone();
        god.lineup[1].ratings = [99; 11];
        god.lineup[1].fields[16..27].fill(99);
        assert!(!check_match(&god, &records, &none).is_empty());
        // A created baller past Create A Baller.
        let mut cab = m.clone();
        cab.lineup[5].ratings[8] = 99;
        cab.lineup[5].fields[24] = 99;
        assert!(!check_match(&cab, &records, &none).is_empty());
        // The wrong record.
        assert!(!check_match(&m, &[0, 1, 2, 3, 4, 5], &none).is_empty());
    }

    /// A mod whose generic record 231 is better than the cap (Legendary's
    /// Gianna Bryant): legal as the mod has it, never edited up.
    #[test]
    fn a_record_over_the_cap_plays_only_as_the_base_has_it() {
        let (dp, dt) = tables();
        let mut base = dp.clone();
        base[231].ratings = [95; 11];
        base[231].fields[16..27].fill(95);
        let total_95 = 95 * TOTALLED as i32;
        assert_eq!(Limits::from_players(&base).total_cap, 698);
        let check = |json: &str| {
            let file = RosterFile::parse(json).unwrap();
            check_file(
                &file,
                (&base, &dt),
                (&dp, &dt),
                &BTreeMap::new(),
                Base::Mod([1; 32]),
            )
            .unwrap()
        };
        // Unedited (and its own copy, and a cosmetic edit): legal.
        let r = check(r#"{"players": [{"slot": 0, "character": "vol2:0"}]}"#);
        assert!(r.legal(), "{:?}", r.problems);
        assert_eq!(r.rating_total_cap, 698);
        let r = check(
            r#"{"players": [{"slot": 231, "character": "vol2:231", "edits": {"firstName": "G", "heightIn": 72}},
                            {"slot": null, "character": "vol2:231", "team": "vol2:0"}]}"#,
        );
        assert!(r.legal(), "{:?}", r.problems);
        assert_eq!(
            (r.players[0].total, r.players[0].base_total),
            (total_95, total_95)
        );
        // Edited up: over the cap, so no rating may rise past the base's.
        let r = check(
            r#"{"players": [{"slot": 231, "character": "vol2:231", "edits": {"ratings": [99]}}]}"#,
        );
        assert_eq!(fields(&r), vec!["ratings"]);
        assert_eq!(r.problems[0].allowed, Some([0, i64::from(total_95)]));
        // Trading ratings past the cap is no better.
        let r = check(
            r#"{"players": [{"slot": 231, "character": "vol2:231", "edits": {"ratings": [99, 91]}}]}"#,
        );
        assert_eq!(fields(&r), vec!["ratings"]);
        // Down to the cap, anything goes again.
        let r = check(
            r#"{"players": [{"slot": 231, "character": "vol2:231",
                "edits": {"ratings": [99, 99, 99, 99, 99, 99, 99, 1, 1]}}]}"#,
        );
        assert!(r.legal(), "{:?}", r.problems);
        // Nobody else may be edited up to it.
        let r = check(
            r#"{"players": [{"slot": 0, "character": "vol2:0", "edits": {"ratings": [95, 95, 95, 95, 95, 95, 95, 95, 95]}}]}"#,
        );
        assert_eq!(fields(&r), vec!["ratings"]);
        assert_eq!(r.problems[0].allowed, Some([0, 698]));
        // A server that trusts a mod it lacks takes over-cap records as the mod's.
        let lineup = vec![base[231].clone()];
        let m = MatchRoster {
            roster: SharedRoster {
                base: Base::Mod([1; 32]),
                players_len: DISC_RECORDS as u16,
                teams_len: 49,
                ..Default::default()
            },
            lineup,
        };
        let blind = Context {
            limits: Limits::for_base(Base::Mod([1; 32])),
            ..Default::default()
        };
        assert!(!check_match(&m, &[231], &blind).is_empty());
        let trusting = Context {
            limits: Limits {
                trusted: true,
                ..blind.limits
            },
            ..Default::default()
        };
        assert!(check_match(&m, &[231], &trusting).is_empty());
        let known = Context {
            base: Some(&base),
            limits: Limits::from_players(&base),
            ..Default::default()
        };
        assert!(check_match(&m, &[231], &known).is_empty());
    }

    /// Disc-gated (`STREET_ISO`, and `STREET_ROSTER_DB` for the mod): the
    /// constants are the disc's, and the shipped rosters are legal.
    #[test]
    fn the_disc_and_the_legendary_roster_are_legal() {
        use crate::db::{self, Table};
        let Some(iso) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut iso = crate::iso::Iso::open(iso).unwrap();
        let moves = db::moves(&mut iso).unwrap();
        assert_eq!(moves.len(), MOVE_COUNT);
        for m in &moves {
            assert_eq!(
                move_row(m.index),
                Some((m.kind as u8, m.level as u8)),
                "{}",
                m.name
            );
        }
        let original = Table::read(&mut iso, db::PLAYERS).unwrap();
        let original_teams = Table::read(&mut iso, db::TEAMS).unwrap();
        let dp = db::parse_players(&original).unwrap();
        let dt = db::parse_teams(&original_teams).unwrap();
        assert_eq!(Limits::from_players(&dp).total_cap, ORIGINAL_TOTAL_CAP);
        // Every disc record is a legal player (as a match line-up sends it).
        for p in &dp {
            let held = Held::of(p.index, false);
            let found = check_values(p, held, &Limits::for_base(Base::Original), None);
            assert!(found.is_empty(), "{} #{}: {found:?}", p.name(), p.index);
        }
        assert!(check_teams(&dt, dp.len()).is_empty());
        let Some(dir) = std::env::var_os("STREET_ROSTER_DB") else {
            return;
        };
        let dir = db::find_roster_dir(std::path::Path::new(&dir)).unwrap();
        let table = |name: &str| {
            let path = std::fs::read_dir(&dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .find(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(name)
                })
                .unwrap();
            Table::parse(std::fs::read(path).unwrap()).unwrap()
        };
        let mp = db::parse_players_over(&table(db::PLAYERS_FILE), &original).unwrap();
        let mt = db::parse_teams_over(&table(db::TEAMS_FILE), &original_teams).unwrap();
        assert_eq!(
            db::roster_mod_tables(&dir, Some((&original, &original_teams))).unwrap(),
            (mp.clone(), mt.clone())
        );
        assert_eq!(Limits::from_players(&mp).total_cap, LEGENDARY_TOTAL_CAP);
        let legendary = Base::Mod(db::roster_mod_id(&dir).unwrap());
        let shared = SharedRoster::diff(
            legendary,
            (&dp, &dt),
            (&mp, &mt),
            &Default::default(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        println!(
            "Legendary as a shared roster: {} records, {} teams, {} bytes, {} packed",
            shared.players.len(),
            shared.teams.len(),
            shared.encode().len(),
            MatchRoster {
                roster: shared.clone(),
                lineup: Vec::new()
            }
            .pack()
            .unwrap()
            .len()
        );
        let blind = Context {
            limits: Limits::for_base(legendary),
            ..Default::default()
        };
        let found = check_shared(&shared, &blind);
        assert!(found.is_empty(), "{found:?}");
        let known = Context {
            disc: Some((&dp, &dt)),
            base: Some(&mp),
            limits: Limits::from_players(&mp),
        };
        let found = check_shared(&shared, &known);
        assert!(found.is_empty(), "{found:?}");
        // Gianna Bryant (#231, all 99s) plays as the mod has her; the
        // server needs the mod to know it.
        assert_eq!(total(&mp[231]), 891);
        let m = MatchRoster {
            roster: shared.clone(),
            lineup: vec![mp[231].clone()],
        };
        assert!(check_match(&m, &[231], &known).is_empty());
        assert!(!check_match(&m, &[231], &blind).is_empty());
        let (rebuilt, _) = shared.rebuild_players(&dp).unwrap();
        assert_eq!(rebuilt, mp);
        assert_eq!(shared.rebuild_teams(&dt).unwrap(), mt);
        // A roster.json on top: still legal; a 96-inch player is not.
        let file = RosterFile::parse(
            r#"{"base": "legendary", "players": [
                {"slot": 0, "character": "vol2:0", "model": "vol2:42",
                 "edits": {"firstName": "Test", "heightIn": 80, "ratings": [90]},
                 "animations": {"from": "vol2:158", "moves": {"dunks": [49]}}},
                {"slot": null, "character": "vol2:158", "team": "vol2:0"}]}"#,
        )
        .unwrap();
        let r = check_file(&file, (&mp, &mt), (&dp, &dt), &BTreeMap::new(), legendary).unwrap();
        assert!(r.legal(), "{:?}", r.problems);
        let file = RosterFile::parse(
            r#"{"base": "legendary", "players": [
                {"slot": 0, "character": "vol2:0", "edits": {"heightIn": 96}}]}"#,
        )
        .unwrap();
        let r = check_file(&file, (&mp, &mt), (&dp, &dt), &BTreeMap::new(), legendary).unwrap();
        assert_eq!(fields(&r), vec!["heightIn"]);
        let file = RosterFile::parse(
            r#"{"base": "legendary", "players": [
                {"slot": 231, "character": "vol2:231", "edits": {"heightIn": 70}},
                {"slot": 52, "character": "vol2:52", "edits": {"ratings": [99, 99, 99, 99, 99, 99, 99, 99, 99]}}]}"#,
        )
        .unwrap();
        let r = check_file(&file, (&mp, &mt), (&dp, &dt), &BTreeMap::new(), legendary).unwrap();
        assert_eq!(r.rating_total_cap, 794);
        assert_eq!(fields(&r), vec!["ratings"]);
        assert_eq!(r.problems[0].player, Some(1));
    }

    #[test]
    fn the_rules_object_names_every_field() {
        let rules = rules_json(&Limits::for_base(Base::Original));
        assert_eq!(rules["heightIn"], json!([63, 90]));
        assert_eq!(rules["ratingTotal"]["cap"], json!(698));
        assert_eq!(rules["ratingTotalCap"], json!(698));
        assert_eq!(rules["fieldNames"].as_array().unwrap().len(), FIELDS);
        let editable = rules["editableOnline"].as_array().unwrap();
        assert!(editable.contains(&json!(8)) && !editable.contains(&json!(33)));
        assert_eq!(field_name(18), "ratings[2]");
        assert_eq!(field_name(81), "moves.dunks[2]");
        assert_eq!(field_name(33), "raw.33");
        assert_eq!(move_row(49), Some((1, 4)));
        assert_eq!(move_row(93), None);
        assert_eq!(CAB_TOTAL_CAP, 861);
    }
}
