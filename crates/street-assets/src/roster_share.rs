//! An edited roster as data, for online play (`docs/PLAYER_EDITOR.md`,
//! "Online"): the match creator's effective tables as the difference from
//! the disc's own, so every machine plays the same roster from its own disc
//! without the mod's files or `roster.json`.
//!
//! A [`SharedRoster`] holds, for each record that differs from the disc,
//! the disc record it starts from (its *source*) and the fields that differ
//! from that source; the teams that differ, whole; and what is not a table
//! value: each roster player's look and animation sources
//! ([`roster_edits::Applied`]) and the created ballers' equipped moves. A
//! machine rebuilds the tables over its own disc ([`SharedRoster::rebuild`]);
//! [`roster_edits::set_shared`] makes every `db::players` / `db::teams`
//! return them until it is cleared.
//!
//! Because a record is "source + differing fields", fields a roster may not
//! change online (`roster_rules`) show up as differences, so a machine
//! without the disc (`street-server`) can still tell that only allowed
//! fields changed.
//!
//! A match sends a [`MatchRoster`]: the shared roster plus the six players
//! of the line-ups as complete records, packed and deflated
//! ([`MatchRoster::pack`], at most [`MAX_PACKED`] bytes).

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::db::{Player, Team};
use crate::roster_edits::{self, AnimProfile, Applied, Look, MOVE_SLOTS, TEMPLATE_RECORD};

/// Fields of a player record.
pub const FIELDS: usize = 101;
/// The disc's player records (US PS2): sources are among them.
pub const DISC_RECORDS: usize = 240;
/// Longest name kept, in characters (as `roster_edits` keeps them).
pub const MAX_NAME: usize = 64;
/// The most a packed [`MatchRoster`] may take (it rides in a match setup).
pub const MAX_PACKED: usize = 60 * 1024;
/// The most it may unpack to.
pub const MAX_UNPACKED: usize = 1024 * 1024;

const MAGIC: &[u8; 4] = b"RSTR";
const PACKED_MAGIC: &[u8; 4] = b"RSTZ";
const VERSION: u8 = 2;

/// What the creator's roster was built on: the disc's tables or a roster
/// mod's (the Legendary Edition), named by [`crate::db::roster_mod_id`].
/// A server accepts a mod base only if it holds that mod.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Base {
    #[default]
    Original,
    Mod([u8; 32]),
}

impl Base {
    /// The mod's id in hex, if built on one.
    pub fn mod_hex(&self) -> Option<String> {
        match self {
            Base::Original => None,
            Base::Mod(id) => Some(hex(id)),
        }
    }
}

/// Bytes in lower-case hex.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// One record of the effective roster: `source`'s disc record with
/// `fields` changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SharedPlayer {
    pub record: u16,
    pub source: u16,
    /// (field, value), by field, only those differing from the source.
    pub fields: Vec<(u8, i16)>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub code: Option<String>,
}

/// How a roster player is drawn ([`Look`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SharedLook {
    As {
        index: u16,
        team: i16,
    },
    /// Another record's appearance: that record, as data.
    Wear(Box<SharedPlayer>),
}

/// The roster as data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SharedRoster {
    pub base: Base,
    /// How many player records and teams the effective tables hold.
    pub players_len: u16,
    pub teams_len: u16,
    pub players: Vec<SharedPlayer>,
    /// Teams that differ from the disc's, or are added, whole.
    pub teams: Vec<Team>,
    pub looks: Vec<(u16, SharedLook)>,
    pub anims: Vec<(u16, AnimProfile)>,
    pub gamecube: Vec<(u16, String)>,
    /// Created ballers' equipped rows by save slot (the animation sources
    /// `created:<n>` use).
    pub created: Vec<(u8, [Vec<u8>; 3])>,
}

/// The fields of `p`, with its named members written back over them (the
/// game sets created ballers' members directly).
pub fn fields_of(p: &Player) -> Vec<i16> {
    let mut f = p.fields.clone();
    f.resize(f.len().max(FIELDS), -1);
    f[0] = p.id;
    f[3] = p.team;
    f[4] = p.position;
    f[5] = p.jersey;
    f[7] = p.skin_tone;
    f[8] = p.height_in;
    f[9] = p.weight_lb;
    f[10] = p.model_height_in;
    f[11] = p.model_weight_lb;
    let flag = |f: &mut [i16], k: usize, on: bool| {
        if (f[k] > 0) != on {
            f[k] = i16::from(on);
        }
    };
    flag(&mut f, 12, p.female);
    flag(&mut f, 13, p.left_handed);
    f[14] = p.portrait;
    f[16..27].copy_from_slice(&p.ratings);
    f[27] = p.style;
    flag(&mut f, 28, p.star);
    f[30] = p.build;
    f[32] = p.look;
    f[36] = p.clothing;
    f[38] = p.shoes;
    f[39] = p.hair_style;
    f[40] = p.hair_color;
    f[41] = p.facial_hair_style;
    f[42] = p.facial_hair_color;
    f[43..46].copy_from_slice(&p.body_scale);
    f[46..68].copy_from_slice(&p.accessories);
    f
}

/// `p` with `f` as its fields, members and all.
pub fn with_fields(mut p: Player, f: Vec<i16>) -> Player {
    roster_edits::sync(&mut p, f);
    p
}

impl SharedPlayer {
    /// `p` as `source`'s record with the differences.
    pub fn diff(record: usize, source: &Player, p: &Player) -> Self {
        let (a, b) = (fields_of(source), fields_of(p));
        let name = |a: &str, b: &str| (a != b).then(|| b.to_owned());
        Self {
            record: record as u16,
            source: source.index as u16,
            fields: (0..FIELDS)
                .filter(|&k| a[k] != b[k])
                .map(|k| (k as u8, b[k]))
                .collect(),
            first_name: name(&source.first_name, &p.first_name),
            last_name: name(&source.last_name, &p.last_name),
            code: name(&source.code, &p.code),
        }
    }

    /// The record, over the disc's.
    pub fn build(&self, disc: &[Player]) -> Result<Player, String> {
        let source = disc.get(usize::from(self.source)).ok_or_else(|| {
            format!(
                "record {}: source {} is not one of the disc's {} records",
                self.record,
                self.source,
                disc.len()
            )
        })?;
        let mut f = fields_of(source);
        for &(k, v) in &self.fields {
            *f.get_mut(usize::from(k))
                .ok_or_else(|| format!("record {}: no field {k}", self.record))? = v;
        }
        let mut p = with_fields(source.clone(), f);
        if let Some(s) = &self.first_name {
            p.first_name.clone_from(s);
        }
        if let Some(s) = &self.last_name {
            p.last_name.clone_from(s);
        }
        if let Some(s) = &self.code {
            p.code.clone_from(s);
        }
        p.index = usize::from(self.record);
        Ok(p)
    }
}

/// Two records equal as the tables hold them.
fn same(a: &Player, b: &Player) -> bool {
    fields_of(a) == fields_of(b)
        && a.first_name == b.first_name
        && a.last_name == b.last_name
        && a.code == b.code
}

impl SharedRoster {
    /// The effective tables `now` as differences from the disc's. `hints`
    /// gives a record's starting character (`roster.json`'s `character`),
    /// tried as its source; the source with the fewest differences in
    /// fields a roster may not change is taken.
    pub fn diff(
        base: Base,
        disc: (&[Player], &[Team]),
        now: (&[Player], &[Team]),
        applied: &Applied,
        created: &BTreeMap<usize, [Vec<u8>; 3]>,
        hints: &BTreeMap<usize, usize>,
    ) -> Self {
        let (disc_players, disc_teams) = disc;
        let (players, teams) = now;
        let pick = |record: usize, p: &Player| -> SharedPlayer {
            let mut candidates = Vec::new();
            if record < disc_players.len() {
                candidates.push(record);
            }
            candidates.extend(hints.get(&record).copied());
            candidates.push(TEMPLATE_RECORD.min(disc_players.len().saturating_sub(1)));
            candidates
                .into_iter()
                .filter_map(|s| disc_players.get(s))
                .map(|s| SharedPlayer::diff(record, s, p))
                .min_by_key(|d| {
                    let fixed = d
                        .fields
                        .iter()
                        .filter(|(k, _)| {
                            crate::roster_rules::field_rule(usize::from(*k))
                                == crate::roster_rules::FieldRule::Source
                        })
                        .count();
                    (fixed, d.fields.len())
                })
                .unwrap_or_default()
        };
        // Records past the disc's start as the create-a-player template
        // when rebuilt, so one equal to it need not travel.
        let template = disc_players.get(TEMPLATE_RECORD);
        let shared_players = players
            .iter()
            .enumerate()
            .filter(|&(r, p)| disc_players.get(r).or(template).is_none_or(|d| !same(d, p)))
            .map(|(r, p)| pick(r, p))
            .collect();
        let shared_teams = teams
            .iter()
            .enumerate()
            .filter(|&(t, team)| disc_teams.get(t) != Some(team))
            .map(|(_, team)| team.clone())
            .collect();
        let looks = applied
            .looks
            .iter()
            .map(|(&r, look)| {
                let look = match look {
                    Look::As { index, team } => SharedLook::As {
                        index: *index as u16,
                        team: *team,
                    },
                    Look::Wear(wear) => SharedLook::Wear(Box::new(pick(wear.index, wear))),
                };
                (r as u16, look)
            })
            .collect();
        Self {
            base,
            players_len: players.len() as u16,
            teams_len: teams.len() as u16,
            players: shared_players,
            teams: shared_teams,
            looks,
            anims: applied
                .anims
                .iter()
                .map(|(&r, a)| (r as u16, a.clone()))
                .collect(),
            gamecube: applied
                .gamecube
                .iter()
                .map(|(&r, g)| (r as u16, g.clone()))
                .collect(),
            created: created
                .iter()
                .map(|(&slot, moves)| (slot as u8, moves.clone()))
                .collect(),
        }
    }

    /// Nothing differs from the disc.
    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
            && self.teams.is_empty()
            && self.looks.is_empty()
            && self.anims.is_empty()
            && self.gamecube.is_empty()
            && self.created.is_empty()
            && usize::from(self.players_len) == DISC_RECORDS
    }

    /// The players over the disc's, and what `roster_edits` keeps by record.
    pub fn rebuild_players(&self, disc: &[Player]) -> Result<(Vec<Player>, Applied), String> {
        let len = usize::from(self.players_len);
        if len < disc.len() || len > crate::roster_rules::MAX_RECORDS {
            return Err(format!(
                "the roster has {len} player records (the disc {}, at most {})",
                disc.len(),
                crate::roster_rules::MAX_RECORDS
            ));
        }
        let template = disc
            .get(TEMPLATE_RECORD)
            .ok_or("the disc has no create-a-player record")?;
        let mut players = disc.to_vec();
        while players.len() < len {
            let mut p = template.clone();
            p.index = players.len();
            players.push(p);
        }
        for sp in &self.players {
            let record = usize::from(sp.record);
            let slot = players
                .get_mut(record)
                .ok_or_else(|| format!("record {record} is past the {len} records"))?;
            *slot = sp.build(disc)?;
        }
        let mut applied = Applied::default();
        for (r, look) in &self.looks {
            let look = match look {
                SharedLook::As { index, team } => Look::As {
                    index: usize::from(*index),
                    team: *team,
                },
                SharedLook::Wear(wear) => Look::Wear(Box::new(wear.build(disc)?)),
            };
            applied.looks.insert(usize::from(*r), look);
        }
        for (r, a) in &self.anims {
            applied.anims.insert(usize::from(*r), a.clone());
        }
        for (r, g) in &self.gamecube {
            applied.gamecube.insert(usize::from(*r), g.clone());
        }
        Ok((players, applied))
    }

    /// The teams over the disc's.
    pub fn rebuild_teams(&self, disc: &[Team]) -> Result<Vec<Team>, String> {
        let len = usize::from(self.teams_len);
        if len < disc.len() || len > crate::roster_rules::MAX_TEAMS {
            return Err(format!(
                "the roster has {len} teams (the disc {}, at most {})",
                disc.len(),
                crate::roster_rules::MAX_TEAMS
            ));
        }
        let mut teams: Vec<Option<Team>> = disc.iter().cloned().map(Some).collect();
        teams.resize(len, None);
        for t in &self.teams {
            *teams
                .get_mut(t.index)
                .ok_or_else(|| format!("team {} is past the {len} teams", t.index))? =
                Some(t.clone());
        }
        teams
            .into_iter()
            .enumerate()
            .map(|(k, t)| t.ok_or_else(|| format!("team {k} is missing")))
            .collect()
    }

    /// The created ballers' equipped rows by slot.
    pub fn created_moves(&self) -> BTreeMap<usize, [Vec<u8>; 3]> {
        self.created
            .iter()
            .map(|(slot, m)| (usize::from(*slot), m.clone()))
            .collect()
    }

    /// The canonical bytes (what [`Self::fingerprint`] hashes).
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer(MAGIC.to_vec());
        w.u8(VERSION);
        match &self.base {
            Base::Original => w.u8(0),
            Base::Mod(id) => {
                w.u8(1);
                w.0.extend_from_slice(id);
            }
        }
        w.u16(self.players_len);
        w.u16(self.teams_len);
        w.uv(self.players.len() as u64);
        for p in &self.players {
            w.player(p);
        }
        w.uv(self.teams.len() as u64);
        for t in &self.teams {
            w.u16(t.index as u16);
            w.str(&t.name);
            w.str(&t.city);
            w.str(&t.nickname);
            w.uv(t.roster.len() as u64);
            for &r in &t.roster {
                w.u16(r as u16);
            }
            w.iv(i64::from(t.logo));
        }
        w.uv(self.looks.len() as u64);
        for (r, look) in &self.looks {
            w.u16(*r);
            match look {
                SharedLook::As { index, team } => {
                    w.u8(0);
                    w.u16(*index);
                    w.iv(i64::from(*team));
                }
                SharedLook::Wear(p) => {
                    w.u8(1);
                    w.player(p);
                }
            }
        }
        w.uv(self.anims.len() as u64);
        for (r, a) in &self.anims {
            w.u16(*r);
            w.u16(a.moves_record as u16);
            w.u16(a.shot_record as u16);
            for rows in &a.rows {
                match rows {
                    None => w.u8(0),
                    Some(rows) => {
                        w.u8(1);
                        w.bytes(rows);
                    }
                }
            }
            w.iv(i64::from(a.gait.0));
            w.iv(i64::from(a.gait.1));
            w.u8(u8::from(a.gait.2));
        }
        w.uv(self.gamecube.len() as u64);
        for (r, g) in &self.gamecube {
            w.u16(*r);
            w.str(g);
        }
        w.uv(self.created.len() as u64);
        for (slot, moves) in &self.created {
            w.u8(*slot);
            for m in moves {
                w.bytes(m);
            }
        }
        w.0
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Reader(bytes);
        let roster = Self::read(&mut r)?;
        if !r.0.is_empty() {
            return Err("roster: trailing bytes".into());
        }
        Ok(roster)
    }

    fn read(r: &mut Reader) -> Result<Self, String> {
        if r.take(4)? != MAGIC || r.u8()? != VERSION {
            return Err("roster: not a shared roster of this version".into());
        }
        let base = match r.u8()? {
            0 => Base::Original,
            1 => Base::Mod(r.take(32)?.try_into().expect("32 bytes")),
            k => return Err(format!("roster: base {k}")),
        };
        let players_len = r.u16()?;
        let teams_len = r.u16()?;
        let limit = crate::roster_rules::MAX_RECORDS;
        let n = r.len(limit)?;
        let players = (0..n).map(|_| r.player()).collect::<Result<_, _>>()?;
        let n = r.len(crate::roster_rules::MAX_TEAMS)?;
        let mut teams = Vec::with_capacity(n);
        for _ in 0..n {
            let index = usize::from(r.u16()?);
            let (name, city, nickname) = (r.str()?, r.str()?, r.str()?);
            let m = r.len(limit)?;
            let roster = (0..m)
                .map(|_| r.u16().map(usize::from))
                .collect::<Result<_, _>>()?;
            teams.push(Team {
                index,
                name,
                city,
                nickname,
                roster,
                logo: r.i16()?,
            });
        }
        let n = r.len(limit)?;
        let mut looks = Vec::with_capacity(n);
        for _ in 0..n {
            let record = r.u16()?;
            let look = match r.u8()? {
                0 => SharedLook::As {
                    index: r.u16()?,
                    team: r.i16()?,
                },
                1 => SharedLook::Wear(Box::new(r.player()?)),
                k => return Err(format!("roster: look kind {k}")),
            };
            looks.push((record, look));
        }
        let n = r.len(limit)?;
        let mut anims = Vec::with_capacity(n);
        for _ in 0..n {
            let record = r.u16()?;
            let moves_record = usize::from(r.u16()?);
            let shot_record = usize::from(r.u16()?);
            let mut rows: [Option<Vec<u8>>; 3] = Default::default();
            for (kind, slot) in rows.iter_mut().enumerate() {
                *slot = match r.u8()? {
                    0 => None,
                    1 => Some(r.bytes(MOVE_SLOTS[kind])?),
                    k => return Err(format!("roster: move list flag {k}")),
                };
            }
            let gait = i32::try_from(r.iv()?).map_err(|_| "roster: gait record")?;
            let weight = r.i16()?;
            let female = match r.u8()? {
                0 => false,
                1 => true,
                k => return Err(format!("roster: flag {k}")),
            };
            anims.push((
                record,
                AnimProfile {
                    moves_record,
                    shot_record,
                    rows,
                    gait: (gait, weight, female),
                },
            ));
        }
        let n = r.len(limit)?;
        let gamecube = (0..n)
            .map(|_| Ok((r.u16()?, r.str()?)))
            .collect::<Result<_, String>>()?;
        let n = r.len(crate::roster_rules::CREATED_SLOTS)?;
        let mut created = Vec::with_capacity(n);
        for _ in 0..n {
            let slot = r.u8()?;
            let moves = [
                r.bytes(MOVE_SLOTS[0])?,
                r.bytes(MOVE_SLOTS[1])?,
                r.bytes(MOVE_SLOTS[2])?,
            ];
            created.push((slot, moves));
        }
        Ok(Self {
            base,
            players_len,
            teams_len,
            players,
            teams,
            looks,
            anims,
            gamecube,
            created,
        })
    }

    /// SHA-256 of [`Self::encode`], in hex: the same roster gives the same
    /// fingerprint on any machine (`street-export check-roster` prints it).
    pub fn fingerprint(&self) -> String {
        hex(&Sha256::digest(self.encode()))
    }
}

/// What a match sends: the roster and the six players of the line-ups as
/// complete records (in the setup's order), so a server without the disc
/// knows the values the match's world must hold.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MatchRoster {
    pub roster: SharedRoster,
    pub lineup: Vec<Player>,
}

impl MatchRoster {
    /// The bytes for the match setup: a header, then the deflated roster
    /// and line-up. Errs when they would not fit [`MAX_PACKED`].
    pub fn pack(&self) -> Result<Vec<u8>, String> {
        let mut body = Writer(self.roster.encode());
        body.uv(self.lineup.len() as u64);
        for p in &self.lineup {
            body.u16(p.index as u16);
            for v in fields_of(p).iter().take(FIELDS) {
                body.iv(i64::from(*v));
            }
            body.str(&p.first_name);
            body.str(&p.last_name);
            body.str(&p.code);
        }
        let mut out = PACKED_MAGIC.to_vec();
        out.push(VERSION);
        out.extend(miniz_oxide::deflate::compress_to_vec(&body.0, 9));
        if out.len() > MAX_PACKED {
            return Err(format!(
                "the roster takes {} KiB packed; a match carries at most {} KiB",
                out.len() / 1024,
                MAX_PACKED / 1024
            ));
        }
        Ok(out)
    }

    pub fn unpack(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_PACKED
            || bytes.get(..4) != Some(PACKED_MAGIC.as_slice())
            || bytes.get(4) != Some(&VERSION)
        {
            return Err("the match's roster is not one this game reads".into());
        }
        let body = miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes[5..], MAX_UNPACKED)
            .map_err(|_| "the match's roster does not unpack".to_string())?;
        let mut r = Reader(&body);
        let roster = SharedRoster::read(&mut r)?;
        let n = r.len(crate::roster_rules::LINEUP)?;
        let mut lineup = Vec::with_capacity(n);
        for _ in 0..n {
            let index = usize::from(r.u16()?);
            let mut f = Vec::with_capacity(FIELDS);
            for _ in 0..FIELDS {
                f.push(r.i16()?);
            }
            let mut p = with_fields(blank(index), f);
            p.first_name = r.str()?;
            p.last_name = r.str()?;
            p.code = r.str()?;
            lineup.push(p);
        }
        if !r.0.is_empty() {
            return Err("roster: trailing bytes".into());
        }
        Ok(Self { roster, lineup })
    }
}

/// An empty record numbered `index` (its fields come next).
pub fn blank(index: usize) -> Player {
    Player {
        index,
        id: 0,
        first_name: String::new(),
        last_name: String::new(),
        team: 0,
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
        fields: vec![0; FIELDS],
    }
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn uv(&mut self, mut v: u64) {
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.0.push(byte);
                return;
            }
            self.0.push(byte | 0x80);
        }
    }
    fn iv(&mut self, v: i64) {
        self.uv(((v << 1) ^ (v >> 63)) as u64);
    }
    fn str(&mut self, s: &str) {
        let s: String = s.chars().take(MAX_NAME).collect();
        self.uv(s.len() as u64);
        self.0.extend_from_slice(s.as_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.uv(b.len() as u64);
        self.0.extend_from_slice(b);
    }
    fn player(&mut self, p: &SharedPlayer) {
        self.u16(p.record);
        self.u16(p.source);
        self.uv(p.fields.len() as u64);
        for &(k, v) in &p.fields {
            self.u8(k);
            self.iv(i64::from(v));
        }
        let names = [&p.first_name, &p.last_name, &p.code];
        self.u8(names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.is_some())
            .map(|(k, _)| 1 << k)
            .sum());
        for s in names.into_iter().flatten() {
            self.str(s);
        }
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        if self.0.len() < n {
            return Err("roster: cut short".into());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn uv(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.u8()?;
            v |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("roster: number too long".into())
    }
    fn iv(&mut self) -> Result<i64, String> {
        let v = self.uv()?;
        Ok((v >> 1) as i64 ^ -((v & 1) as i64))
    }
    fn i16(&mut self) -> Result<i16, String> {
        i16::try_from(self.iv()?).map_err(|_| "roster: value out of range".to_string())
    }
    fn len(&mut self, most: usize) -> Result<usize, String> {
        let n = self.uv()?;
        usize::try_from(n)
            .ok()
            .filter(|&n| n <= most)
            .ok_or_else(|| format!("roster: {n} entries (at most {most})"))
    }
    fn str(&mut self) -> Result<String, String> {
        let n = self.len(MAX_NAME * 4)?;
        let s = std::str::from_utf8(self.take(n)?)
            .map_err(|_| "roster: a name is not UTF-8".to_string())?;
        if s.chars().count() > MAX_NAME || s.chars().any(char::is_control) {
            return Err("roster: a name is too long or has control characters".into());
        }
        Ok(s.to_owned())
    }
    fn bytes(&mut self, most: usize) -> Result<Vec<u8>, String> {
        let n = self.len(most)?;
        Ok(self.take(n)?.to_vec())
    }
    fn player(&mut self) -> Result<SharedPlayer, String> {
        let record = self.u16()?;
        let source = self.u16()?;
        let n = self.len(FIELDS)?;
        let mut fields = Vec::with_capacity(n);
        for _ in 0..n {
            let k = self.u8()?;
            if usize::from(k) >= FIELDS || fields.last().is_some_and(|&(last, _)| last >= k) {
                return Err(format!("roster: record {record}: field {k} out of order"));
            }
            fields.push((k, self.i16()?));
        }
        let flags = self.u8()?;
        if flags > 7 {
            return Err(format!("roster: record {record}: name flags {flags}"));
        }
        let mut name = |bit: u8| -> Result<Option<String>, String> {
            if flags & (1 << bit) != 0 {
                self.str().map(Some)
            } else {
                Ok(None)
            }
        };
        Ok(SharedPlayer {
            record,
            source,
            fields,
            first_name: name(0)?,
            last_name: name(1)?,
            code: name(2)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disc() -> (Vec<Player>, Vec<Team>) {
        let players = (0..DISC_RECORDS)
            .map(|i| {
                let mut f = vec![0i16; FIELDS];
                f[3] = (i / 5).min(48) as i16;
                f[8] = 78;
                f[9] = 215;
                f[10] = 78;
                f[11] = 215;
                f[16..27].copy_from_slice(&[50; 11]);
                // Records differ in their ratings.
                f[16] = (i % 90) as i16;
                f[17] = (i % 70) as i16;
                f[19] = (i % 80) as i16;
                let mut p = with_fields(blank(i), f);
                p.first_name = format!("F{i}");
                p.last_name = format!("L{i}");
                p
            })
            .collect();
        let teams = (0..49)
            .map(|t| Team {
                index: t,
                name: format!("T{t}"),
                city: format!("C{t}"),
                nickname: format!("N{t}"),
                roster: (5 * t..5 * t + 5).collect(),
                logo: t as i16,
            })
            .collect();
        (players, teams)
    }

    #[test]
    fn a_roster_goes_out_as_differences_and_comes_back_whole() {
        let (dp, dt) = disc();
        let (mut players, mut teams) = (dp.clone(), dt.clone());
        // An edited record, a record replaced by another's data, an added
        // player and an added team.
        players[0].first_name = "Mike".into();
        players[0].ratings[2] = 90;
        players[0].fields[18] = 90;
        assert_eq!(dp[0].ratings[2], 50);
        let mut replaced = dp[158].clone();
        replaced.index = 3;
        replaced.team = 0;
        replaced.fields[3] = 0;
        players[3] = replaced;
        let mut added = dp[7].clone();
        added.index = 244;
        added.last_name = "Rookie".into();
        players.resize(244, dp[TEMPLATE_RECORD].clone());
        for (k, p) in players.iter_mut().enumerate().skip(DISC_RECORDS) {
            p.index = k;
        }
        players.push(added);
        teams[0].roster.push(244);
        teams.push(Team {
            index: 49,
            name: "Custom Roster".into(),
            city: "Custom".into(),
            nickname: "Roster".into(),
            roster: vec![0, 3, 244],
            logo: 41,
        });
        let mut applied = Applied::default();
        applied
            .looks
            .insert(0, Look::Wear(Box::new(dp[146].clone())));
        applied.anims.insert(
            0,
            AnimProfile {
                moves_record: 158,
                shot_record: 158,
                rows: [
                    None,
                    Some(vec![49, 26, 255, 255, 255, 10, 255, 255, 255, 255, 255]),
                    None,
                ],
                gait: (20, 215, false),
            },
        );
        applied.gamecube.insert(244, "v3gc:mario".into());
        let mut created = BTreeMap::new();
        created.insert(2, [vec![1], vec![10], vec![54]]);
        let hints = BTreeMap::from([(3, 158), (244, 7)]);
        let shared = SharedRoster::diff(
            Base::Mod([7; 32]),
            (&dp, &dt),
            (&players, &teams),
            &applied,
            &created,
            &hints,
        );
        // Only what differs travels; record 3 starts from record 158.
        let records: Vec<u16> = shared.players.iter().map(|p| p.record).collect();
        assert_eq!(records, vec![0, 3, 244]);
        assert_eq!(shared.players[1].source, 158);
        assert_eq!(shared.players[0].fields, vec![(18, 90)]);
        assert_eq!(shared.teams.len(), 2);
        let bytes = shared.encode();
        let back = SharedRoster::decode(&bytes).unwrap();
        assert_eq!(back, shared);
        assert_eq!(back.fingerprint(), shared.fingerprint());
        // Rebuilt over the disc, it is the creator's roster.
        let (rp, ra) = back.rebuild_players(&dp).unwrap();
        assert_eq!(rp.len(), players.len());
        for (a, b) in rp.iter().zip(&players) {
            assert!(same(a, b), "record {}", a.index);
        }
        assert_eq!(back.rebuild_teams(&dt).unwrap(), teams);
        assert_eq!(ra.anims, applied.anims);
        assert_eq!(ra.gamecube, applied.gamecube);
        let Look::Wear(w) = &ra.looks[&0] else {
            panic!("a worn model")
        };
        assert_eq!(w.index, 146);
        assert_eq!(back.created_moves(), created);
        // The disc's own roster is empty.
        let none = SharedRoster::diff(
            Base::Original,
            (&dp, &dt),
            (&dp, &dt),
            &Applied::default(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert!(none.is_empty());
        // Packed with a line-up, and back.
        let m = MatchRoster {
            roster: shared,
            lineup: vec![players[0].clone(), players[244].clone()],
        };
        let packed = m.pack().unwrap();
        let un = MatchRoster::unpack(&packed).unwrap();
        assert_eq!(un.roster, m.roster);
        assert!(un.lineup.iter().zip(&m.lineup).all(|(a, b)| same(a, b)));
        assert_eq!(un.lineup[1].index, 244);
    }

    #[test]
    fn damaged_rosters_are_refused() {
        let (dp, dt) = disc();
        let shared = SharedRoster {
            players_len: 241,
            teams_len: 49,
            players: vec![SharedPlayer {
                record: 240,
                source: 7,
                fields: vec![(8, 80)],
                ..Default::default()
            }],
            ..Default::default()
        };
        let bytes = shared.encode();
        for cut in 0..bytes.len() {
            assert!(SharedRoster::decode(&bytes[..cut]).is_err());
        }
        let mut bad = bytes.clone();
        bad.push(0);
        assert!(SharedRoster::decode(&bad).is_err());
        assert!(MatchRoster::unpack(b"RSTZ\x01garbage").is_err());
        assert!(MatchRoster::unpack(&[0; 10]).is_err());
        // A source past the disc, a record past the table.
        let mut wrong = shared.clone();
        wrong.players[0].source = 999;
        assert!(wrong.rebuild_players(&dp).is_err());
        let mut wrong = shared;
        wrong.players[0].record = 400;
        assert!(wrong.rebuild_players(&dp).is_err());
        let teams = SharedRoster {
            teams_len: 80,
            ..Default::default()
        };
        assert!(teams.rebuild_teams(&dt).is_err());
    }
}
