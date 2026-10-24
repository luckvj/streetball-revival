//! The player editor's `roster.json` (`docs/PLAYER_EDITOR.md`), applied at
//! the database layer: once [`set`], every [`db::players`] and [`db::teams`]
//! returns the edited roster (field edits, replaced and added records, team
//! lists and a "Custom Roster" team), on top of the disc's tables or a
//! roster mod's ([`db::set_roster_override`]).
//!
//! What is not a table value is kept here for the game to ask by record:
//! the model a roster player wears ([`look_for`], used by
//! [`crate::player::look`]) and where its animation choices come from
//! ([`route_moves`], [`gait_source`]). For every record `roster.json` does
//! not name these are the identity, so nothing changes without it.

use std::{collections::BTreeMap, sync::RwLock};

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::db::{Player, PlayerKind, Team};
use crate::roster_share::SharedRoster;

/// The create-a-player record (the template for new records).
pub const TEMPLATE_RECORD: usize = 234;
/// The first record free for added players: after the create-a-player
/// records and the career stars' (234 + 10, `street_sim::story`'s
/// `BALLER_RECORDS + STAR_SLOTS`), which created ballers may use.
pub const FIRST_ADDED: usize = 244;
/// Equipped slots per kind (lay-ups, dunks, dribble moves).
pub const MOVE_SLOTS: [usize; 3] = [6, 11, 11];
/// A vacant equipped slot.
pub const EMPTY: u8 = u8::MAX;
/// league teams 0..29 have uniform sheets.
const LEAGUE_TEAMS: i16 = 29;
/// A new team's logo: the generic ballers' (as Create Team's).
const CUSTOM_LOGO: i16 = 41;
const CREATED_SLOTS: usize = FIRST_ADDED - TEMPLATE_RECORD;

/// `roster.json`, as the builder writes it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterFile {
    #[serde(default = "one")]
    pub version: u32,
    /// "original" or "legendary": the roster the edits were made on.
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub players: Vec<RosterPlayer>,
}

fn one() -> u32 {
    1
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterPlayer {
    /// The Vol. 2 record replaced; `None` adds a record.
    #[serde(default)]
    pub slot: Option<usize>,
    /// Whose data the edits start from.
    pub character: String,
    /// The team an added player joins.
    #[serde(default)]
    pub team: Option<String>,
    /// Wear another character's model.
    #[serde(default)]
    pub model: Option<String>,
    /// Changed fields by name, and `raw` by field number.
    #[serde(default)]
    pub edits: Map<String, Value>,
    #[serde(default)]
    pub animations: Option<Animations>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Animations {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub idle: Option<String>,
    #[serde(default)]
    pub gait: Option<String>,
    #[serde(default)]
    pub shot: Option<String>,
    #[serde(default)]
    pub celebrate: Option<String>,
    #[serde(default)]
    pub moves: Option<MoveLists>,
}

/// Equipped `XDBMOVE` rows by slot (6, 11, 11), `-1` for an empty slot; a
/// missing list keeps the animation source's.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MoveLists {
    #[serde(default)]
    pub layups: Option<Vec<i32>>,
    #[serde(default)]
    pub dunks: Option<Vec<i32>>,
    #[serde(default)]
    pub tricks: Option<Vec<i32>>,
}

impl RosterFile {
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: Self = serde_json::from_str(text).map_err(|e| format!("roster.json: {e}"))?;
        if file.version != 1 {
            return Err(format!(
                "roster.json version {} (this game reads version 1)",
                file.version
            ));
        }
        Ok(file)
    }

    /// `Some(true)` when the edits were made on a roster mod (Legendary),
    /// `Some(false)` on the disc's own.
    pub fn wants_mod(&self) -> Option<bool> {
        match self.base.as_deref()?.to_ascii_lowercase().as_str() {
            "original" => Some(false),
            _ => Some(true),
        }
    }

    /// The record each player ends up in, in the file's order: its slot,
    /// or [`FIRST_ADDED`] on for added players.
    pub fn records(&self) -> Vec<usize> {
        let mut added = FIRST_ADDED;
        self.players
            .iter()
            .map(|p| {
                p.slot.unwrap_or_else(|| {
                    added += 1;
                    added - 1
                })
            })
            .collect()
    }

    /// Whether any player starts from, or takes animations from, a created
    /// baller (so the save must be known: [`set_created`]).
    pub fn uses_created(&self) -> bool {
        self.players.iter().any(|p| {
            let a = p.animations.clone().unwrap_or_default();
            [
                Some(&p.character),
                p.model.as_ref(),
                a.from.as_ref(),
                a.shot.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|id| id.to_ascii_lowercase().starts_with("created:"))
        })
    }
}

/// A character id (`docs/PLAYER_EDITOR.md`, "Character ids").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CharId {
    /// A record of Vol. 2's player table, in the active roster.
    Vol2(usize),
    /// A created baller of the save (`created:<n>`).
    Created(usize),
    /// A GameCube character (`v3gc:mario`, `v1gc:...`): "game:name".
    GameCube(String),
}

impl CharId {
    pub fn parse(id: &str) -> Result<Self, String> {
        let (game, key) = id
            .split_once(':')
            .ok_or_else(|| format!("{id:?} is not a character id (game:key)"))?;
        let number = || {
            key.parse::<usize>()
                .map_err(|_| format!("{id:?}: {key:?} is not a number"))
        };
        match game.to_ascii_lowercase().as_str() {
            "vol2" => Ok(Self::Vol2(number()?)),
            "created" => Ok(Self::Created(number()?)),
            g @ ("v3gc" | "v1gc") if !key.is_empty() => {
                Ok(Self::GameCube(format!("{g}:{}", key.to_ascii_lowercase())))
            }
            _ => Err(format!("{id:?}: unknown game {game:?}")),
        }
    }

    /// The Vol. 2 record this character is in this game, if it has one.
    pub fn record(&self) -> Option<usize> {
        match *self {
            Self::Vol2(r) => Some(r),
            Self::Created(n) => Some(TEMPLATE_RECORD + n),
            Self::GameCube(_) => None,
        }
    }
}

/// How a roster player is drawn.
#[derive(Clone, Debug, PartialEq)]
pub enum Look {
    /// Its own appearance, as the model of record `index` (the starting
    /// character's), in team `team`'s uniform.
    As { index: usize, team: i16 },
    /// Another record's appearance (`model`), ready to load.
    Wear(Box<Player>),
}

/// Where a roster player's animation choices come from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimProfile {
    /// The record whose dunk and dribble-move choices the player makes
    /// (the native legends' own slot-1 moves are keyed on it).
    pub moves_record: usize,
    /// The record whose lay-up / shot slots the player uses.
    pub shot_record: usize,
    /// Explicit equipped rows by kind, [`EMPTY`] holes; `None` keeps the
    /// source's.
    pub rows: [Option<Vec<u8>>; 3],
    /// The gait profile: the player restriction (a record) and the weight
    /// and sex that pick the category.
    pub gait: (i32, i16, bool),
}

/// What applying `roster.json` to a player table worked out.
#[derive(Clone, Debug, Default)]
pub struct Applied {
    /// By final record.
    pub looks: BTreeMap<usize, Look>,
    pub anims: BTreeMap<usize, AnimProfile>,
    /// GameCube models asked for, by final record.
    pub gamecube: BTreeMap<usize, String>,
    /// Things worth a line on stderr (fallbacks, ignored values).
    pub notes: Vec<String>,
}

/// A created baller of the save, for `created:<n>`: its record and its
/// equipped rows.
#[derive(Clone, Debug)]
pub struct Created {
    pub player: Player,
    pub moves: [Vec<u8>; 3],
}

/// Apply `file` to `players` (the active roster's), with the save's created
/// ballers by slot.
pub fn apply(
    file: &RosterFile,
    players: &mut Vec<Player>,
    created: &BTreeMap<usize, Created>,
) -> Result<Applied, String> {
    if players.len() > FIRST_ADDED {
        return Err(format!(
            "the player table has {} records; roster.json adds from {FIRST_ADDED}",
            players.len()
        ));
    }
    let base: Vec<Player> = players.clone();
    let template = base
        .get(TEMPLATE_RECORD)
        .or(base.last())
        .cloned()
        .ok_or("the player table is empty")?;
    let mut out = Applied::default();
    let character = |id: &CharId, notes: &mut Vec<String>| -> Result<Player, String> {
        match id {
            CharId::Vol2(r) => base
                .get(*r)
                .cloned()
                .ok_or_else(|| format!("vol2:{r} is past the {} records", base.len())),
            CharId::Created(n) => Ok(created.get(n).map_or_else(
                || {
                    notes.push(format!(
                        "created:{n} is not in this save; starting from the create-a-player record"
                    ));
                    template.clone()
                },
                |c| c.player.clone(),
            )),
            CharId::GameCube(id) => Ok(gamecube_start(&template, id, notes)),
        }
    };
    let data = |r: usize| {
        r.checked_sub(TEMPLATE_RECORD)
            .filter(|&n| n < CREATED_SLOTS)
            .and_then(|n| created.get(&n))
            .map(|c| &c.player)
            .or_else(|| base.get(r))
            .map(|g| (g.weight_lb, g.female))
    };
    let records = file.records();
    for (k, (entry, &record)) in file.players.iter().zip(&records).enumerate() {
        let what = |e: String| format!("roster.json player {k}: {e}");
        if entry.slot.is_some_and(|s| s >= base.len()) {
            return Err(what(format!(
                "slot {record} is past the {} records",
                base.len()
            )));
        }
        let id = CharId::parse(&entry.character).map_err(what)?;
        let mut p = character(&id, &mut out.notes).map_err(what)?;
        let source = p.clone();
        for note in apply_edits(&mut p, &entry.edits).map_err(what)? {
            out.notes.push(what(note));
        }
        let team_edited = entry.edits.contains_key("team")
            || entry
                .edits
                .get("raw")
                .and_then(Value::as_object)
                .is_some_and(|r| r.contains_key("3"));
        let team = match (entry.slot, &entry.team) {
            // An added player joins its team; a replacing one stays on the
            // slot's (whose roster lists it) unless the edits move it.
            (None, Some(team)) => Some(team_number(team).map_err(what)? as i16),
            (Some(slot), _) if !team_edited => Some(base[slot].team),
            _ => None,
        };
        if let Some(t) = team {
            p.team = t;
            p.fields[3] = t;
        }
        let own_index = id.record().unwrap_or(TEMPLATE_RECORD);
        let own = Look::As {
            index: own_index,
            team: uniform_team(&source, own_index, p.team),
        };
        let look = match &entry.model {
            None => own,
            Some(m) => match CharId::parse(m).map_err(what)? {
                CharId::GameCube(gc) => {
                    if !crate::gc_discs::has(&gc) {
                        let stand_in = match &id {
                            CharId::GameCube(_) => "the create-a-player model",
                            _ => entry.character.as_str(),
                        };
                        out.notes.push(what(format!(
                            "{gc}: no disc for it (--v3 / --v1); drawing {stand_in}"
                        )));
                    }
                    out.gamecube.insert(record, gc);
                    own
                }
                other => {
                    let m = character(&other, &mut out.notes).map_err(what)?;
                    let mut wear = m.clone();
                    wear.index = other.record().unwrap_or(m.index);
                    wear.team = uniform_team(&m, wear.index, p.team);
                    wear.first_name.clone_from(&p.first_name);
                    wear.last_name.clone_from(&p.last_name);
                    // Drawn at the player's own size: a worn model is the
                    // appearance, never a hidden giant (`roster_rules`).
                    wear.height_in = p.height_in;
                    wear.weight_lb = p.weight_lb;
                    wear.model_height_in = p.model_height_in;
                    wear.model_weight_lb = p.model_weight_lb;
                    Look::Wear(Box::new(wear))
                }
            },
        };
        // A GameCube character without another model wears its own.
        if let (None, CharId::GameCube(gc)) = (&entry.model, &id) {
            if !crate::gc_discs::has(gc) {
                out.notes.push(what(format!(
                    "{gc}: no disc for it (--v3 / --v1); drawing the create-a-player model"
                )));
            }
            out.gamecube.insert(record, gc.clone());
        }
        p.index = record;
        if entry.slot.is_none() {
            players.resize(record, template.clone());
            players.push(p.clone());
        } else {
            players[record] = p.clone();
        }
        out.looks.insert(record, look);
        let profile = anim_profile(entry, own_index, &p, &data, base.len(), &mut out.notes);
        out.anims.insert(record, profile.map_err(what)?);
    }
    Ok(out)
}

/// Add each added player to its team and every named player to a "Custom
/// Roster" team (when there are three or more).
pub fn apply_teams(file: &RosterFile, teams: &mut Vec<Team>) -> Result<(), String> {
    let records = file.records();
    for (k, (entry, &record)) in file.players.iter().zip(&records).enumerate() {
        if entry.slot.is_none()
            && let Some(team) = &entry.team
        {
            let t = team_number(team).map_err(|e| format!("roster.json player {k}: {e}"))?;
            let team = teams.get_mut(t).ok_or_else(|| {
                format!("roster.json player {k}: team {t} is not one of the teams")
            })?;
            team.roster.push(record);
        }
    }
    if records.len() >= 3 {
        teams.push(Team {
            index: teams.len(),
            name: "Custom Roster".into(),
            city: "Custom".into(),
            nickname: "Roster".into(),
            roster: records,
            logo: CUSTOM_LOGO,
        });
    }
    Ok(())
}

/// `vol2:<team>` or a bare team number.
fn team_number(id: &str) -> Result<usize, String> {
    id.strip_prefix("vol2:")
        .unwrap_or(id)
        .parse::<usize>()
        .map_err(|_| format!("team {id:?} is not a team id (vol2:<n>)"))
}

/// The team whose uniform a model wears: a league model takes the player's
/// league team; anyone else keeps the model's own team (a created player's must
/// stay the create-a-player team for its kit).
fn uniform_team(model: &Player, model_record: usize, player_team: i16) -> i16 {
    let mut probe = model.clone();
    probe.index = model_record;
    if probe.kind() == PlayerKind::League && (0..LEAGUE_TEAMS).contains(&player_team) {
        player_team
    } else {
        model.team
    }
}

fn anim_profile(
    entry: &RosterPlayer,
    own: usize,
    p: &Player,
    data: &dyn Fn(usize) -> Option<(i16, bool)>,
    records: usize,
    notes: &mut Vec<String>,
) -> Result<AnimProfile, String> {
    let anims = entry.animations.clone().unwrap_or_default();
    // A source: a Vol. 2 record or created baller; GameCube sets are
    // preview-only and fall back.
    let source = |key: &str,
                  value: &Option<String>,
                  fallback: usize,
                  notes: &mut Vec<String>|
     -> Result<usize, String> {
        let Some(value) = value else {
            return Ok(fallback);
        };
        let id = CharId::parse(value)?;
        match id.record() {
            Some(r) if r < records || matches!(id, CharId::Created(_)) => Ok(r),
            Some(r) => Err(format!("{key}: vol2:{r} is past the {records} records")),
            None => {
                notes.push(format!(
                    "animations.{key}: {value} is preview-only; playing {}'s",
                    entry.character
                ));
                Ok(fallback)
            }
        }
    };
    let from = source("from", &anims.from, own, notes)?;
    let gait = source("gait", &anims.gait, from, notes)?;
    let shot = source("shot", &anims.shot, from, notes)?;
    for (key, value) in [("idle", &anims.idle), ("celebrate", &anims.celebrate)] {
        if value.is_some() {
            source(key, value, from, notes)?;
            notes.push(format!(
                "animations.{key}: every Vol. 2 player shares the {key} animations; kept"
            ));
        }
    }
    let lists = anims.moves.unwrap_or_default();
    let mut rows: [Option<Vec<u8>>; 3] = Default::default();
    for (kind, (name, list)) in [
        ("layups", &lists.layups),
        ("dunks", &lists.dunks),
        ("tricks", &lists.tricks),
    ]
    .into_iter()
    .enumerate()
    {
        let Some(list) = list else { continue };
        if list.len() > MOVE_SLOTS[kind] {
            notes.push(format!(
                "animations.moves.{name}: {} rows for {} slots; the rest dropped",
                list.len(),
                MOVE_SLOTS[kind]
            ));
        }
        let mut slots = vec![EMPTY; MOVE_SLOTS[kind]];
        for (slot, &row) in slots.iter_mut().zip(list) {
            // The game checks each row's kind against XDBMOVE when it plays
            // (a wrong one plays the slot's default).
            *slot = u8::try_from(row)
                .ok()
                .filter(|&r| r != EMPTY)
                .unwrap_or(EMPTY);
        }
        rows[kind] = Some(slots);
    }
    // The gait category follows the source's build; the player's own
    // (edited) build when the source is its own character.
    let (weight, female) = if gait == own {
        (p.weight_lb, p.female)
    } else {
        data(gait).unwrap_or((p.weight_lb, p.female))
    };
    Ok(AnimProfile {
        moves_record: from,
        shot_record: shot,
        rows,
        gait: (gait as i32, weight, female),
    })
}

/// Named fields (the catalog's `fields`) by player field number.
const NAMED: [(&str, usize); 22] = [
    ("id", 0),
    ("team", 3),
    ("position", 4),
    ("jersey", 5),
    ("skinTone", 7),
    ("heightIn", 8),
    ("weightLb", 9),
    ("modelHeightIn", 10),
    ("modelWeightLb", 11),
    ("female", 12),
    ("leftHanded", 13),
    ("portrait", 14),
    ("style", 27),
    ("star", 28),
    ("build", 30),
    ("look", 32),
    ("clothing", 36),
    ("shoes", 38),
    ("hairStyle", 39),
    ("hairColor", 40),
    ("facialHairStyle", 41),
    ("facialHairColor", 42),
];
/// Named lists: name, first field, length.
const LISTS: [(&str, usize, usize); 3] = [
    ("ratings", 16, 11),
    ("bodyScale", 43, 3),
    ("accessories", 46, 22),
];

fn small(v: &Value) -> Option<i16> {
    match v {
        Value::Bool(b) => Some(i16::from(*b)),
        Value::Number(n) => n.as_i64().and_then(|n| i16::try_from(n).ok()),
        _ => None,
    }
}

/// Apply `edits` (`raw` by field number, then the named fields over it) to
/// a record; returns notes for keys it does not know.
pub fn apply_edits(p: &mut Player, edits: &Map<String, Value>) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    let mut f = p.fields.clone();
    f.resize(f.len().max(101), -1);
    let (old_height, old_weight) = (f[8], f[9]);
    let mut set = |field: usize, v: &Value, key: &str| -> Result<(), String> {
        let v = small(v).ok_or_else(|| format!("{key}: {v} is not a small number"))?;
        *f.get_mut(field)
            .ok_or_else(|| format!("{key}: the record has no field {field}"))? = v;
        Ok(())
    };
    let raw = match edits.get("raw") {
        Some(raw) => Some(
            raw.as_object()
                .ok_or("raw: not an object of field numbers")?,
        ),
        None => None,
    };
    for (key, v) in raw.into_iter().flatten() {
        let field: usize = key
            .parse()
            .map_err(|_| format!("raw: {key:?} is not a field number"))?;
        set(field, v, &format!("raw.{key}"))?;
    }
    for (key, v) in edits {
        if key == "raw" {
            continue;
        }
        if let Some(&(_, field)) = NAMED.iter().find(|(n, _)| n == key) {
            set(field, v, key)?;
        } else if let Some(&(_, first, len)) = LISTS.iter().find(|(n, _, _)| n == key) {
            let list = v.as_array().ok_or_else(|| format!("{key}: not a list"))?;
            if list.len() > len {
                return Err(format!("{key}: {} values for {len} fields", list.len()));
            }
            for (k, v) in list.iter().enumerate() {
                set(first + k, v, key)?;
            }
        } else if matches!(key.as_str(), "firstName" | "lastName" | "code") {
            let s: String = v
                .as_str()
                .ok_or_else(|| format!("{key}: not a string"))?
                .chars()
                .filter(|c| !c.is_control())
                .take(64)
                .collect();
            match key.as_str() {
                "firstName" => p.first_name = s,
                "lastName" => p.last_name = s,
                _ => p.code = s,
            }
        } else {
            notes.push(format!(
                "edits.{key} is not a field this game knows; ignored"
            ));
        }
    }
    let given = |name: &str, field: usize| {
        edits.contains_key(name) || raw.is_some_and(|r| r.contains_key(&field.to_string()))
    };
    // A new height or weight redraws the model to it (keeping the
    // celebrities' extra drawn weight) unless the model's own are given.
    if f[8] != old_height && !given("modelHeightIn", 10) {
        f[10] = f[8];
    }
    if f[9] != old_weight && !given("modelWeightLb", 11) {
        f[11] = f[9].saturating_add(f[11].saturating_sub(old_weight));
    }
    sync(p, f);
    Ok(notes)
}

/// Every numeric named field of `p` from `f` (as [`db::players`] reads them).
pub(crate) fn sync(p: &mut Player, f: Vec<i16>) {
    p.id = f[0];
    p.team = f[3];
    p.position = f[4];
    p.jersey = f[5];
    p.skin_tone = f[7];
    p.height_in = f[8];
    p.weight_lb = f[9];
    p.model_height_in = f[10];
    p.model_weight_lb = f[11];
    p.female = f[12] > 0;
    p.left_handed = f[13] > 0;
    p.portrait = f[14];
    p.ratings.copy_from_slice(&f[16..27]);
    p.style = f[27];
    p.star = f[28] > 0;
    p.build = f[30];
    p.look = f[32];
    p.clothing = f[36];
    p.shoes = f[38];
    p.hair_style = f[39];
    p.hair_color = f[40];
    p.facial_hair_style = f[41];
    p.facial_hair_color = f[42];
    p.body_scale.copy_from_slice(&f[43..46]);
    p.accessories.copy_from_slice(&f[46..68]);
    p.fields = f;
}

// ---------------------------------------------------------------------------
// The active roster.json, read by `db::players`, `db::teams` and the game.

#[derive(Default)]
struct State {
    file: Option<RosterFile>,
    created: BTreeMap<usize, Created>,
    /// From the last [`db::players`].
    applied: Applied,
    /// An online match's roster ([`set_shared`]), over everything else.
    shared: Option<SharedRoster>,
}

static STATE: RwLock<Option<State>> = RwLock::new(None);

fn read<T>(f: impl FnOnce(&State) -> Option<T>) -> Option<T> {
    STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(f)
}

fn write<T>(f: impl FnOnce(&mut State) -> T) -> T {
    f(STATE
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(Default::default))
}

/// Use `file` for every later [`db::players`] / [`db::teams`] (`None`
/// stops). Call before the disc's tables are read.
pub fn set(file: Option<RosterFile>) {
    write(|s| {
        s.file = file;
        s.applied = Applied::default();
    });
}

/// The save's created ballers by slot, for `created:<n>` (re-read the
/// tables afterwards).
pub fn set_created(created: BTreeMap<usize, Created>) {
    write(|s| s.created = created);
}

/// The active file, if any.
pub fn active() -> Option<RosterFile> {
    read(|s| s.file.clone())
}

/// Play `roster` (an online match's, `roster_share`) for every later
/// [`db::players`] / [`db::teams`], over the disc's own tables, in place of
/// any roster mod and `roster.json`; `None` goes back to them. Re-read the
/// tables afterwards.
pub fn set_shared(roster: Option<SharedRoster>) {
    write(|s| {
        s.shared = roster;
        s.applied = Applied::default();
    });
}

/// The shared roster in play, if any.
pub fn shared() -> Option<SharedRoster> {
    read(|s| s.shared.clone())
}

/// What the last [`db::players`] worked out (looks, animation sources).
pub fn applied() -> Applied {
    read(|s| Some(s.applied.clone())).unwrap_or_default()
}

/// The save's created ballers' equipped rows by slot, as [`set_created`]
/// gave them.
pub fn created_moves() -> BTreeMap<usize, [Vec<u8>; 3]> {
    read(|s| {
        Some(
            s.created
                .iter()
                .map(|(&n, c)| (n, c.moves.clone()))
                .collect(),
        )
    })
    .unwrap_or_default()
}

/// [`db::players`]' hook for a shared roster: the players over the disc's
/// (`disc`), or `None` without one.
pub(crate) fn shared_players(disc: &[Player]) -> crate::Result<Option<Vec<Player>>> {
    let Some(shared) = shared() else {
        return Ok(None);
    };
    let (players, applied) = shared.rebuild_players(disc).map_err(crate::Error::format)?;
    write(|s| s.applied = applied);
    Ok(Some(players))
}

/// [`db::teams`]' hook for a shared roster.
pub(crate) fn shared_teams(disc: &[Team]) -> crate::Result<Option<Vec<Team>>> {
    match shared() {
        Some(shared) => shared
            .rebuild_teams(disc)
            .map(Some)
            .map_err(crate::Error::format),
        None => Ok(None),
    }
}

/// The notes of the last application (fallbacks and ignored values).
pub fn notes() -> Vec<String> {
    read(|s| Some(s.applied.notes.clone())).unwrap_or_default()
}

/// [`db::players`]' hook.
pub(crate) fn apply_players(players: &mut Vec<Player>) -> crate::Result<()> {
    let Some((file, created)) = read(|s| Some((s.file.clone()?, s.created.clone()))) else {
        return Ok(());
    };
    let applied = apply(&file, players, &created).map_err(crate::Error::format)?;
    write(|s| s.applied = applied);
    Ok(())
}

/// [`db::teams`]' hook.
pub(crate) fn apply_teams_active(teams: &mut Vec<Team>) -> crate::Result<()> {
    match read(|s| s.file.clone()) {
        Some(file) => apply_teams(&file, teams).map_err(crate::Error::format),
        None => Ok(()),
    }
}

/// The record to draw `p` with: a roster player's chosen model (another
/// record's, or its own appearance as its starting character's model);
/// `None` for everyone else.
pub fn look_for(p: &Player) -> Option<Player> {
    match read(|s| s.applied.looks.get(&p.index).cloned())? {
        Look::As { index, team } if index == p.index && team == p.team => None,
        Look::As { index, team } => {
            let mut look = p.clone();
            look.index = index;
            look.team = team;
            Some(look)
        }
        Look::Wear(wear) => Some(*wear),
    }
}

/// A GameCube character's starting record: the create-a-player record with
/// the character's name, position, jersey, height, weight and sex from its
/// disc's table ([`crate::gc_discs`]), or the bare record without the disc.
/// (Ratings stay the record's: the games' rating scales differ.)
fn gamecube_start(template: &Player, id: &str, notes: &mut Vec<String>) -> Player {
    let mut p = template.clone();
    let Some(info) = crate::gc_discs::info(id) else {
        notes.push(format!(
            "{id}: its disc was not given; starting from the create-a-player record"
        ));
        return p;
    };
    let mut edits = Map::new();
    let (first, last) = if info.last_name.is_empty() {
        (String::new(), info.first_name.clone())
    } else {
        (info.first_name.clone(), info.last_name.clone())
    };
    edits.insert("firstName".into(), first.into());
    edits.insert("lastName".into(), last.into());
    for (key, v) in [
        ("position", info.position),
        ("jersey", info.jersey),
        ("heightIn", info.height_in),
        ("weightLb", info.weight_lb),
    ] {
        if let Some(v) = v.filter(|v| (0..=i32::from(i16::MAX)).contains(v)) {
            edits.insert(key.into(), v.into());
        }
    }
    edits.insert("female".into(), info.female.into());
    if let Err(e) = apply_edits(&mut p, &edits) {
        notes.push(format!("{id}: {e}"));
    }
    p
}

/// The GameCube model `record` wears (`v3gc:mario`): a roster.json
/// player's `model`, or its `character` when that is a GameCube one and no
/// model is given. Drawn by `street-render` from the disc
/// ([`crate::gc_discs`]); [`look_for`] gives the Vol. 2 stand-in without it.
pub fn gamecube_model(record: usize) -> Option<String> {
    read(|s| s.applied.gamecube.get(&record).cloned())
}

/// A roster player's animation sources; `None` for everyone else.
pub fn animation(record: usize) -> Option<AnimProfile> {
    read(|s| s.applied.anims.get(&record).cloned())
}

/// A created baller's equipped rows, by its record.
pub fn equipped_moves(record: usize) -> Option<[Vec<u8>; 3]> {
    let n = record.checked_sub(TEMPLATE_RECORD)?;
    read(|s| match &s.shared {
        Some(shared) => shared.created_moves().remove(&n),
        None => Some(s.created.get(&n)?.moves.clone()),
    })
}

/// The record and equipped rows the move lookup (`0x13a1f8`) uses for the
/// player in `record` with `equipped`: a roster player's animation
/// source's, under the rows `roster.json` sets; anyone else's own.
pub fn route_moves(record: usize, equipped: &[Vec<u8>; 3]) -> (usize, [Vec<u8>; 3]) {
    let Some(profile) = animation(record) else {
        return (record, equipped.clone());
    };
    let rows_of = |source: usize| equipped_moves(source).unwrap_or_default();
    let (from, shot) = (rows_of(profile.moves_record), rows_of(profile.shot_record));
    let source = [shot[0].clone(), from[1].clone(), from[2].clone()];
    let rows = std::array::from_fn(|kind| {
        profile.rows[kind]
            .clone()
            .unwrap_or_else(|| source[kind].clone())
    });
    (profile.moves_record, rows)
}

/// The gait profile (player restriction, weight, sex) for a roster player
/// whose walk is another character's; `None` for everyone else.
pub fn gait_source(record: usize) -> Option<(i32, i16, bool)> {
    Some(animation(record)?.gait)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in record with all 101 fields.
    fn player(index: usize, first: &str, team: i16) -> Player {
        let mut f = vec![0i16; 101];
        f[3] = team;
        f[8] = 78;
        f[9] = 215;
        f[10] = 78;
        f[11] = 215;
        f[30] = 1;
        let mut p = Player {
            index,
            id: 0,
            first_name: first.into(),
            last_name: format!("Player{index}"),
            team,
            position: 0,
            jersey: 0,
            skin_tone: 0,
            height_in: 0,
            weight_lb: 0,
            model_height_in: 0,
            model_weight_lb: 0,
            female: false,
            left_handed: false,
            portrait: 0,
            ratings: [0; 11],
            style: 0,
            star: false,
            code: String::new(),
            build: 0,
            look: 0,
            clothing: 0,
            shoes: 0,
            hair_style: 0,
            hair_color: 0,
            facial_hair_style: 0,
            facial_hair_color: 0,
            body_scale: [0; 3],
            accessories: [0; 22],
            fields: Vec::new(),
        };
        sync(&mut p, f);
        p
    }

    fn table() -> (Vec<Player>, Vec<Team>) {
        let players = (0..240)
            .map(|i| {
                let team = match i {
                    0..145 => (i / 5) as i16,
                    234.. => 37,
                    _ => 30,
                };
                player(i, &format!("P{i}"), team)
            })
            .collect();
        let teams = (0..49)
            .map(|t| Team {
                index: t,
                name: format!("Team {t}"),
                city: format!("City{t}"),
                nickname: format!("Nick{t}"),
                roster: if t < 29 {
                    (5 * t..5 * t + 5).collect()
                } else {
                    vec![]
                },
                logo: t as i16,
            })
            .collect();
        (players, teams)
    }

    const SAMPLE: &str = r#"{
      "version": 1,
      "base": "legendary",
      "players": [
        { "slot": 0, "character": "vol2:0",
          "edits": { "firstName": "Mike", "ratings": [99, 98], "heightIn": 80,
                     "leftHanded": true, "raw": { "33": 2, "8": 70 } },
          "animations": { "from": "vol2:158", "gait": "vol2:20",
                          "moves": { "dunks": [1, -1, 4, 300], "tricks": [] } } },
        { "slot": 1, "character": "vol2:158", "model": "vol2:146" },
        { "slot": null, "character": "vol2:7", "team": "vol2:3",
          "edits": { "lastName": "Rookie" } },
        { "slot": null, "character": "v3gc:mario", "model": "v3gc:mario",
          "animations": { "from": "v3gc:mario", "celebrate": "vol2:140" } }
      ]
    }"#;

    #[test]
    fn roster_json_applies_edits_slots_added_players_and_teams() {
        let file = RosterFile::parse(SAMPLE).unwrap();
        assert_eq!(file.wants_mod(), Some(true));
        assert!(!file.uses_created());
        let (mut players, mut teams) = table();
        let applied = apply(&file, &mut players, &BTreeMap::new()).unwrap();
        // Field edits: named ones over raw ones, the model height following.
        let p = &players[0];
        assert_eq!(
            (p.first_name.as_str(), p.last_name.as_str()),
            ("Mike", "Player0")
        );
        assert_eq!(&p.ratings[..3], &[99, 98, 0]);
        assert_eq!((p.height_in, p.model_height_in), (80, 80));
        assert_eq!(p.fields[33], 2);
        assert!(p.left_handed && p.fields[13] == 1);
        // A replaced slot: Dr. J's data in record 1, wearing record 146's
        // model under his own name.
        assert_eq!(
            (players[1].first_name.as_str(), players[1].index),
            ("P158", 1)
        );
        let Look::Wear(wear) = &applied.looks[&1] else {
            panic!("a model swap")
        };
        assert_eq!((wear.index, wear.first_name.as_str()), (146, "P158"));
        // Added players go after the career stars' records.
        assert_eq!(players.len(), FIRST_ADDED + 2);
        assert_eq!(players[FIRST_ADDED].last_name, "Rookie");
        assert_eq!(players[FIRST_ADDED].index, FIRST_ADDED);
        assert_eq!(players[FIRST_ADDED].team, 3);
        assert_eq!(applied.looks[&FIRST_ADDED], Look::As { index: 7, team: 3 });
        // A GameCube model is kept for the GameCube loader; the Vol. 2
        // stand-in is the template's.
        assert_eq!(applied.gamecube[&(FIRST_ADDED + 1)].as_str(), "v3gc:mario");
        assert_eq!(
            applied.looks[&(FIRST_ADDED + 1)],
            Look::As {
                index: TEMPLATE_RECORD,
                team: 37
            }
        );
        // Animation sources.
        let a = &applied.anims[&0];
        assert_eq!((a.moves_record, a.shot_record), (158, 158));
        assert_eq!(a.gait, (20, 215, false));
        assert_eq!(a.rows[0], None);
        let mut dunks = vec![EMPTY; 11];
        dunks[0] = 1;
        dunks[2] = 4;
        assert_eq!(a.rows[1].as_ref(), Some(&dunks));
        assert_eq!(a.rows[2].as_ref(), Some(&vec![EMPTY; 11]));
        // A replaced slot's animations default to its character's.
        assert_eq!(applied.anims[&1].moves_record, 158);
        assert_eq!(applied.anims[&1].gait.0, 158);
        assert!(applied.notes.iter().any(|n| n.contains("preview-only")));
        assert!(applied.notes.iter().any(|n| n.contains("celebrate")));
        // Teams: the added player on team 3, everyone on the custom team.
        apply_teams(&file, &mut teams).unwrap();
        assert!(teams[3].roster.contains(&FIRST_ADDED));
        let custom = teams.last().unwrap();
        assert_eq!(custom.roster, vec![0, 1, FIRST_ADDED, FIRST_ADDED + 1]);
        assert_eq!(
            (custom.index, custom.full_name().as_str()),
            (49, "Custom Roster")
        );
    }

    #[test]
    fn bad_roster_files_say_why() {
        assert!(RosterFile::parse(r#"{"version": 2, "players": []}"#).is_err());
        assert!(RosterFile::parse("3").is_err());
        for bad in [
            r#"{"players": [{"slot": 0, "character": "league:1"}]}"#,
            r#"{"players": [{"slot": 999, "character": "vol2:1"}]}"#,
            r#"{"players": [{"slot": 0, "character": "vol2:1", "edits": {"ratings": [1,2,3,4,5,6,7,8,9,10,11,12]}}]}"#,
            r#"{"players": [{"slot": 0, "character": "vol2:1", "edits": {"jersey": 99999}}]}"#,
            r#"{"players": [{"slot": 0, "character": "vol2:1", "animations": {"gait": "vol2:900"}}]}"#,
        ] {
            let (mut players, _) = table();
            let file = RosterFile::parse(bad).unwrap();
            assert!(
                apply(&file, &mut players, &BTreeMap::new()).is_err(),
                "{bad}"
            );
        }
        let (_, mut teams) = table();
        let file =
            RosterFile::parse(r#"{"players": [{"character": "vol2:1", "team": "vol2:99"}]}"#)
                .unwrap();
        assert!(apply_teams(&file, &mut teams).is_err());
    }

    #[test]
    fn a_created_character_brings_its_save_data() {
        let (mut players, _) = table();
        let mut created = BTreeMap::new();
        created.insert(
            2,
            Created {
                player: player(236, "Made", 37),
                moves: Default::default(),
            },
        );
        let file =
            RosterFile::parse(r#"{"players": [{"slot": 10, "character": "created:2"}]}"#).unwrap();
        assert!(file.uses_created());
        let applied = apply(&file, &mut players, &created).unwrap();
        assert_eq!(players[10].first_name, "Made");
        // Drawn as the created player it is (record 236, the CP team's kit).
        assert_eq!(
            applied.looks[&10],
            Look::As {
                index: 236,
                team: 37
            }
        );
        assert_eq!(applied.anims[&10].moves_record, 236);
    }
}
