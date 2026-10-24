//! The game database: `/DATABASE/*.ADF` (players, teams, courts, moves,
//! nicknames and reward cards), and where each player's model and textures live.
//!
//! All six files share one layout, read by a single generic table class in
//! `SLUS_206.51` (field kind at 0x1da3c8, field address at 0x1da4e0):
//!
//! | Offset | Contents |
//! |---|---|
//! | 0x00 | u32 field count (every field is an `i16`) |
//! | 0x04 | u32 record count |
//! | 0x08 | u32 offset of pool A (u32 values) |
//! | 0x0c | u32 offset of pool B (u32 values) |
//! | 0x10 | u32 offset of the string pool (NUL-terminated Latin-1) |
//! | 0x14 | u8: string offsets are in units of `1 << n` bytes (0 on the PS2 and in the original game; 1..3 in V3's larger tables) |
//! | 0x18 | 2 bits per field, most significant pair first: the field's [`FieldKind`] |
//! | | records, from the next 4-byte boundary |
//!
//! The records of every file end exactly where pool A (or, for the teams, two
//! bytes of padding) begins, which pins the layout down independently of the
//! code. The files are loaded whole and accessed in place (loader 0x1d3a38,
//! called for each file from 0x1da250).
//!
//! The GameCube release stores the same tables big-endian. [`Table::parse`]
//! reads either, so a GameCube roster mod (the "Legendary Edition" updates
//! `xdbplyr.adf` and `xdbteam.adf`) can stand in for the disc's own tables:
//! see [`set_roster_override`].
//!
//! Field meanings come from the code that reads them (cited per field); where
//! no reader was traced, the name is a guess from the data and says so.

use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::{Error, Result, bytes, iso::Iso};

pub const PLAYERS: &str = "/DATABASE/XDBPLYR.ADF";
pub const TEAMS: &str = "/DATABASE/XDBTEAM.ADF";
pub const COURTS: &str = "/DATABASE/XDBCOURT.ADF";
pub const MOVES: &str = "/DATABASE/XDBMOVE.ADF";
pub const NICKNAMES: &str = "/DATABASE/XDBALIAS.ADF";
pub const CARDS: &str = "/DATABASE/XDBCARD.ADF";

/// How a field's `i16` is interpreted (0x1da3c8 returns it, 0x1da4e0 acts on it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// The value itself.
    Value,
    /// Byte offset into the string pool; negative for none.
    String,
    /// Index of a u32 in pool A (the court table keeps `f32`s there).
    PoolA,
    /// Index of a u32 in pool B (unused on the USA disc).
    PoolB,
}

/// One ADF file, kept whole and read in place like the game does.
pub struct Table {
    data: Vec<u8>,
    /// GameCube tables are big-endian; the PS2's are little-endian.
    big_endian: bool,
    kinds: Vec<FieldKind>,
    records: usize,
    /// Start of record 0.
    base: usize,
    pool_a: usize,
    pool_b: usize,
    strings: usize,
    /// String offsets count `1 << string_shift` bytes (V3's `xdbagame.ndf`
    /// has 232 KB of strings, past an `i16` offset).
    string_shift: u32,
}

/// More fields than any table on the disc (101); guards against corrupt headers.
const MAX_FIELDS: usize = 4096;

impl Table {
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        // A PS2 header's field count read the wrong way round is at least
        // 0x0100_0000, so the plausible reading decides the byte order.
        let le = bytes::u32_le(&data, 0)? as usize;
        let big_endian = le == 0 || le > MAX_FIELDS;
        let word = |i: usize| {
            if big_endian {
                bytes::u32_be(&data, 4 * i).map(|w| w as usize)
            } else {
                bytes::u32_le(&data, 4 * i).map(|w| w as usize)
            }
        };
        let (fields, records) = (word(0)?, word(1)?);
        let (pool_a, pool_b, strings) = (word(2)?, word(3)?, word(4)?);
        if fields == 0 || fields > MAX_FIELDS {
            return Err(Error::format(format!("ADF field count {fields}")));
        }
        let kind_bytes = fields.div_ceil(4);
        let kinds = (0..fields)
            .map(|f| {
                let b = *data
                    .get(0x18 + f / 4)
                    .ok_or_else(|| Error::format("ADF header truncated"))?;
                Ok(match (b >> (6 - 2 * (f % 4))) & 3 {
                    0 => FieldKind::Value,
                    1 => FieldKind::String,
                    2 => FieldKind::PoolA,
                    _ => FieldKind::PoolB,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let base = (0x18 + kind_bytes).next_multiple_of(4);
        let end = records
            .checked_mul(fields * 2)
            .and_then(|n| n.checked_add(base))
            .ok_or_else(|| Error::format("ADF record count overflows"))?;
        // The game does not check this, but on the disc the records always
        // end before the pools and the strings, which all lie inside the file.
        let starts = [pool_a, pool_b, strings];
        if starts.iter().any(|&s| s < end || s > data.len()) {
            return Err(Error::format(format!(
                "ADF records end at {end:#x}, pools at {pool_a:#x}/{pool_b:#x}, \
                 strings at {strings:#x}, file {:#x}",
                data.len()
            )));
        }
        let string_shift = u32::from(data[0x14] & 7);
        Ok(Self {
            data,
            big_endian,
            kinds,
            records,
            base,
            pool_a,
            pool_b,
            strings,
            string_shift,
        })
    }

    pub fn read<R: Read + Seek>(iso: &mut Iso<R>, path: &str) -> Result<Self> {
        Self::parse(iso.read(path)?).map_err(|e| Error::format(format!("{path}: {e}")))
    }

    pub fn field_count(&self) -> usize {
        self.kinds.len()
    }

    /// Whether the file is a GameCube (big-endian) table.
    pub fn is_big_endian(&self) -> bool {
        self.big_endian
    }

    pub fn record_count(&self) -> usize {
        self.records
    }

    pub fn kind(&self, field: usize) -> Option<FieldKind> {
        self.kinds.get(field).copied()
    }

    /// The stored `i16` of `field` in `record`.
    pub fn raw(&self, record: usize, field: usize) -> Result<i16> {
        if record >= self.records || field >= self.kinds.len() {
            return Err(Error::format(format!(
                "ADF field {field} of record {record} out of range ({} x {})",
                self.records,
                self.kinds.len()
            )));
        }
        let off = self.base + 2 * (record * self.kinds.len() + field);
        if self.big_endian {
            bytes::i16_be(&self.data, off)
        } else {
            bytes::u16_le(&self.data, off).map(|v| v as i16)
        }
    }

    /// Every field of `record`, raw.
    pub fn record(&self, record: usize) -> Result<Vec<i16>> {
        (0..self.kinds.len()).map(|f| self.raw(record, f)).collect()
    }

    /// A string field; `None` when the offset is negative (`-1` only, in a
    /// table whose offsets are scaled, where they are unsigned).
    pub fn string(&self, record: usize, field: usize) -> Result<Option<String>> {
        self.expect(field, FieldKind::String)?;
        let v = self.raw(record, field)?;
        if v == -1 || (v < 0 && self.string_shift == 0) {
            return Ok(None);
        }
        let off = (v as u16 as usize) << self.string_shift;
        bytes::cstr(&self.data, self.strings + off).map(Some)
    }

    /// A pool field's u32; `None` when the index is negative.
    pub fn pool_word(&self, record: usize, field: usize) -> Result<Option<u32>> {
        let pool = match self.kind(field) {
            Some(FieldKind::PoolA) => self.pool_a,
            Some(FieldKind::PoolB) => self.pool_b,
            _ => {
                return Err(Error::format(format!(
                    "ADF field {field} is not a pool index"
                )));
            }
        };
        let v = self.raw(record, field)?;
        if v < 0 {
            return Ok(None);
        }
        let at = pool + 4 * v as usize;
        if self.big_endian {
            bytes::u32_be(&self.data, at).map(Some)
        } else {
            bytes::u32_le(&self.data, at).map(Some)
        }
    }

    fn expect(&self, field: usize, kind: FieldKind) -> Result<()> {
        match self.kind(field) {
            Some(k) if k == kind => Ok(()),
            k => Err(Error::format(format!(
                "ADF field {field} is {k:?}, not {kind:?}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Players (XDBPLYR.ADF): 240 records of 101 fields.

/// Records `0..LEAGUE_PLAYERS` are the league players, team by team, five each. A
/// record's index is also its player number in `/XF/XF000002` (entries `5i..5i+4`).
pub const LEAGUE_PLAYERS: usize = 145;
/// First streetball legend (Stretch); they run to `LEGENDS`. Thresholds from
/// the model loader 0x315790 (`< 0x91`, `< 0x9e`, `< 0xb8`).
pub const STREETBALLERS: usize = 145;
/// First legend (Julius Erving); they run to `GENERIC`.
pub const LEGENDS: usize = 158;
/// First generic baller; everything from here that is not a created player.
pub const GENERIC: usize = 184;
/// The "CP Team" (create player). The model loader and the clothing getters
/// test `team == 0x25` to take the create-a-player path.
pub const CREATED_PLAYER_TEAM: i16 = 37;

/// Names of the 11 rating fields (player fields 16..26), by rating index.
/// Indices 0, 2..7 are the UI skills (see [`Skill`]); `threes` and `speed` are
/// read from the data (3-pointers never exceed `shots`; the fastest are small
/// guards); the last two are unknown (for created players 0x1d7d00 derives
/// rating 9 from the shots and dunks levels).
pub const RATING_NAMES: [&str; 11] = [
    "shots", "threes", "dunks", "rebounds", "power", "blocks", "steals", "handles", "speed",
    "rating9", "rating10",
];

/// The seven skills the front end shows, in its order (locale text "Shots,
/// Dunks, Power, Blocks, Steals, Handles, Rebounds"). The nickname table uses
/// the same numbering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skill {
    Shots,
    Dunks,
    Power,
    Blocks,
    Steals,
    Handles,
    Rebounds,
}

impl Skill {
    pub const ALL: [Skill; 7] = [
        Skill::Shots,
        Skill::Dunks,
        Skill::Power,
        Skill::Blocks,
        Skill::Steals,
        Skill::Handles,
        Skill::Rebounds,
    ];

    pub fn from_index(i: i16) -> Option<Self> {
        usize::try_from(i)
            .ok()
            .and_then(|i| Self::ALL.get(i))
            .copied()
    }

    /// The rating index the skill reads (jump table at 0x45fbb0 in 0x1d7f48:
    /// skills 0..6 read ratings 0, 2, 4, 5, 6, 7, 3).
    pub fn rating(self) -> usize {
        [0, 2, 4, 5, 6, 7, 3][self as usize]
    }
}

/// Position of a league player (field 4). Other players use the field as an
/// order within their team or leave it 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Center,
    PowerForward,
    SmallForward,
    ShootingGuard,
    PointGuard,
}

impl Position {
    pub fn abbrev(self) -> &'static str {
        ["C", "PF", "SF", "SG", "PG"][self as usize]
    }
}

/// Which loader path builds a player (0x315790), and so where its model is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerKind {
    /// Shared body by build, per-player morph and texture sheets (`/XF/XF000002`).
    League,
    /// One unique model and one sheet each (Stretch, Bonafide, ..., Murphy Lee).
    Streetball,
    /// One unique model and a road and a home sheet each.
    Legend,
    /// One unique model and one sheet each (`male01`..`male38`, `female01`..`female12`).
    Generic,
    /// Assembled from the create-a-player kit in `/XF/XF000001`.
    Created,
}

/// Accessory slots (player fields 46..67). Each value is an image index in
/// the slot's sheet, -1 for none. The table at 0x49a620 in the exe gives each
/// slot an `/XF/XF000002` model (the large left/right model; +2 is medium, +4
/// small) and a sheet; 0x316260 loads them. Pairs are left then right.
pub const ACCESSORY_SLOTS: [AccessorySlot; 22] = {
    const fn s(name: &'static str, model: Option<usize>, sheet: Option<usize>) -> AccessorySlot {
        AccessorySlot { name, model, sheet }
    }
    [
        s("unused", None, None),
        s("headband", None, Some(949)),
        s("elpdL", Some(965), Some(947)),
        s("elpdR", Some(966), Some(947)),
        s("elbrL", Some(959), Some(946)),
        s("elbrR", Some(960), Some(946)),
        s("knpdL", Some(989), Some(951)),
        s("knpdR", Some(990), Some(951)),
        s("knbrL", Some(983), Some(950)),
        s("knbrR", Some(984), Some(950)),
        s("bcepL", Some(953), Some(945)),
        s("bcepR", Some(954), Some(945)),
        s("farmL", Some(971), Some(948)),
        s("farmR", Some(972), Some(948)),
        s("fawdL", Some(977), Some(948)),
        s("fawdR", Some(978), Some(948)),
        s("wrapL", Some(995), Some(952)),
        s("wrapR", Some(996), Some(952)),
        s("wrstL", Some(1001), Some(952)),
        s("wrstR", Some(1002), Some(952)),
        s("wthnL", Some(1007), Some(952)),
        s("wthnR", Some(1008), Some(952)),
    ]
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessorySlot {
    /// The accessory models' name prefix and side (`bcep` + `lrg` + `L` = `bceplrgL`).
    pub name: &'static str,
    /// `/XF/XF000002` entry of the large model; `None` for the headband, which
    /// is a draw group of the body (`HEADBAND`, texture slot `hdbd`).
    pub model: Option<usize>,
    pub sheet: Option<usize>,
}

/// A model height's uniform scale, `height * 1.2 / 78` (0x3153d0: constants
/// 1.2 and 78.0): [`Player::height_scale`], and a created player's from his
/// record's field 10.
pub fn model_height_scale(model_height_in: i16) -> f32 {
    // 003153e8 chops the bounded integer-height multiplication before
    // 00315400's nearest-rounded division. Ordinary f32 multiplication
    // can raise the product by one ULP and changes motion-matched rates.
    let product = f64::from(model_height_in) * f64::from(f32::from_bits(0x3f99_999a));
    let nearest = product as f32;
    let scaled = if f64::from(nearest.abs()) > product.abs() {
        f32::from_bits(nearest.to_bits() - 1)
    } else {
        nearest
    };
    scaled / 78.0
}

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    /// Record number; for league players also the player number in `/XF/XF000002`.
    pub index: usize,
    /// Field 0: persistent player id (Abdur-Rahim 0, rookies 501+; 0x1a7804
    /// turns ids 145..151 into unlock bits). Not an index into anything here.
    pub id: i16,
    /// Field 2 (string): the debug dump at 0x2cec70 prints it as "First Name".
    pub first_name: String,
    /// Field 1 (string): "Last Name"; empty for one-name players.
    pub last_name: String,
    /// Field 3: index into [`teams`]; 37 is the create-player team.
    pub team: i16,
    /// Field 4: see [`Position`].
    pub position: i16,
    /// Field 5: jersey number; -1 for everyone else but Stretch (41) and Bonafide (17).
    pub jersey: i16,
    /// Field 7 (guess): skin tone, 1..4 for men from light to dark and 5..6 for
    /// women; the streetballers carry their id instead and the celebrities -1.
    /// 0x1acb68 picks from per-tone lists with it.
    pub skin_tone: i16,
    /// Field 8: height in inches as shown ("Height: %d' %d\"" at 0x2cec70).
    pub height_in: i16,
    /// Field 9: weight in pounds as shown ("Weight: %d lbs.").
    pub weight_lb: i16,
    /// Field 10: height the model is scaled to (0x3153d0, see [`Player::height_scale`]).
    pub model_height_in: i16,
    /// Field 11: weight the model's width is derived from (0x3153d0). Equal to
    /// field 9 except for the celebrities, who are drawn 30 lb heavier.
    pub model_weight_lb: i16,
    /// Field 12: "Gender: Female" when non-zero (0x2cec70).
    pub female: bool,
    /// Field 13: handedness trait, copied to actor+0xc0 as 2/1 (0x156648).
    /// The live animation hand at actor+0x25c starts at 2 independently
    /// (0x156638), then changes with the animation flags.
    pub left_handed: bool,
    /// Field 14: portrait index in `/FE/XAPORTS.BIN` (245 images). Traded
    /// players keep their old index (Payton 125, Ray Allen 70); created
    /// players use 0xb0 + n (0x223dd0).
    pub portrait: i16,
    /// Fields 16..26, 0..99; names in [`RATING_NAMES`]. Read through 0x1d7d00,
    /// which applies the rating cheats and clamps to 0..99.
    pub ratings: [i16; 11],
    /// Field 27 (guess): play style, 0 shooter, 1 slasher, 2 playmaker,
    /// 3 shot blocker, 4 power player; no reader traced.
    pub style: i16,
    /// Field 28: star; non-zero adds the player to the star lists 0x1dbe30
    /// builds. Created players reuse it as a team index (0x1d84d0).
    pub star: bool,
    /// Field 29 (string): four-letter code, first initial plus surname (`sabd`).
    pub code: String,
    /// Field 30: build. league players 0..2 (small, medium, large body); created
    /// players 0..5 (4 and 5 are female). Printed as "Build: %d".
    pub build: i16,
    /// Field 32: face ("Look: %d"); created players only.
    pub look: i16,
    /// Field 36: created players' clothing in the low 14 bits and its colour
    /// variant in the top two ("Clothing" via 0x1d81a8 and 0x1d8340).
    pub clothing: i16,
    /// Field 38: shoes ("Shoes: %d"); the league players' shoes are in their own sheet.
    pub shoes: i16,
    /// Field 39: hair style ("Hair Style: %d"). For league players non-zero turns
    /// on the body's `zdreds` group (0x316260): Brian Grant, Latrell Sprewell.
    pub hair_style: i16,
    /// Field 40: hair colour ("Hair Color: %d").
    pub hair_color: i16,
    /// Field 41: facial hair style, -1 for none ("Facial Hair Style: %d").
    pub facial_hair_style: i16,
    /// Field 42: facial hair colour ("Facial Hair Color: %d").
    pub facial_hair_color: i16,
    /// Fields 43..45: head-axis percentages (`1 + v / 100`, 0x3153d0),
    /// stored at skeleton +4/+8/+c; see [`Self::native_head_scale`].
    pub body_scale: [i16; 3],
    /// Fields 46..67, see [`ACCESSORY_SLOTS`].
    pub accessories: [i16; 22],
    /// All 101 fields. Unnamed ones: 6 is always -1; 15 is a packed value for
    /// created players (0x1d83c8 and 0x1d8440 split it as `v / 11`, `v % 11`)
    /// and -1 for everyone else; 31 is 0 except for the created players
    /// (0..3); 33..35 and 37 are small unknown values; 68..100 are -1 on the
    /// whole disc (create-a-player data: six at 68..73 and two lists of 11 at
    /// 79..100, read through 0x1d8778).
    pub fields: Vec<i16>,
}

// Original 003153d0 model-scale operations on the retained emulator target.
// MUL chops the exact binary32 product; ADD/SUB mask the smaller operand
// to one guard bit before chopping. DIV/SQRT retain nearest rounding.
// These helpers are scoped to this producer, not general renderer math.
fn model_scale_chop(value: f64) -> f32 {
    let rounded = value as f32;
    if f64::from(rounded.abs()) > value.abs() {
        f32::from_bits(rounded.to_bits() - 1)
    } else {
        rounded
    }
}

fn model_scale_mul(a: f32, b: f32) -> f32 {
    model_scale_chop(f64::from(a) * f64::from(b))
}

fn model_scale_add_sub(a: f32, b: f32, subtract: bool) -> f32 {
    let (mut ab, mut bb) = (a.to_bits(), b.to_bits());
    let difference = ((ab >> 23) & 255) as i32 - ((bb >> 23) & 255) as i32;
    if difference > 0 {
        bb &= if difference >= 25 {
            0x8000_0000
        } else {
            u32::MAX << (difference - 1)
        };
    } else if difference < 0 {
        ab &= if difference <= -25 {
            0x8000_0000
        } else {
            u32::MAX << (-difference - 1)
        };
    }
    let (a, b) = (f64::from(f32::from_bits(ab)), f64::from(f32::from_bits(bb)));
    model_scale_chop(if subtract { a - b } else { a + b })
}

impl Player {
    fn parse(t: &Table, index: usize) -> Result<Self> {
        let f = t.record(index)?;
        if f.len() < 68 {
            return Err(Error::format(format!(
                "player table has {} fields, expected 101",
                f.len()
            )));
        }
        let text = |field| t.string(index, field).map(Option::unwrap_or_default);
        Ok(Self {
            index,
            id: f[0],
            first_name: text(2)?,
            last_name: text(1)?,
            team: f[3],
            position: f[4],
            jersey: f[5],
            skin_tone: f[7],
            height_in: f[8],
            weight_lb: f[9],
            model_height_in: f[10],
            model_weight_lb: f[11],
            female: f[12] > 0,
            left_handed: f[13] > 0,
            portrait: f[14],
            ratings: f[16..27].try_into().expect("11 ratings"),
            style: f[27],
            star: f[28] > 0,
            code: text(29)?,
            build: f[30],
            look: f[32],
            clothing: f[36],
            shoes: f[38],
            hair_style: f[39],
            hair_color: f[40],
            facial_hair_style: f[41],
            facial_hair_color: f[42],
            body_scale: [f[43], f[44], f[45]],
            accessories: f[46..68].try_into().expect("22 accessory slots"),
            fields: f,
        })
    }

    /// "First Last", or the one name a streetballer has.
    pub fn name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
            .trim()
            .to_owned()
    }

    pub fn kind(&self) -> PlayerKind {
        match self.index {
            _ if self.team == CREATED_PLAYER_TEAM => PlayerKind::Created,
            i if i < LEAGUE_PLAYERS => PlayerKind::League,
            i if i < LEGENDS => PlayerKind::Streetball,
            i if i < GENERIC => PlayerKind::Legend,
            _ => PlayerKind::Generic,
        }
    }

    /// The league position; `None` for everyone else.
    pub fn position(&self) -> Option<Position> {
        if self.kind() != PlayerKind::League {
            return None;
        }
        use Position::*;
        [
            Center,
            PowerForward,
            SmallForward,
            ShootingGuard,
            PointGuard,
        ]
        .get(usize::try_from(self.position).ok()?)
        .copied()
    }

    pub fn jersey(&self) -> Option<u16> {
        u16::try_from(self.jersey).ok()
    }

    pub fn skill(&self, skill: Skill) -> i16 {
        self.ratings[skill.rating()]
    }

    /// Uniform model scale from the model height: `height * 1.2 / 78`
    /// (0x3153d0: constants 1.2 and 78.0). The width also depends on the model
    /// weight and the build; see [`Self::native_body_scale`].
    pub fn height_scale(&self) -> f32 {
        model_height_scale(self.model_height_in)
    }

    /// Skeleton scale written by `3153d0`, before the tiny-player option.
    /// Only the normal morphed league model uses the weight-dependent branch;
    /// `315790` supplies the uniform branch for prebuilt and created models.
    pub fn native_body_scale(&self) -> Option<[f32; 3]> {
        let height = self.height_scale();
        if height <= 0.0 {
            return None;
        }
        if !matches!(self.model()?, PlayerModel::League(_)) {
            return Some([height; 3]);
        }
        let build = usize::try_from(self.build).ok()?;
        let base_weight = *[215.0f32, 218.0, 254.0, 150.0].get(build)?;
        let base_radius = *[1.215444f32, 1.215444, 1.246152, 1.2].get(build)?;
        let expansion = *[1.0f32, 0.875, 0.75, 0.875].get(build)?;
        let weight = f32::from(self.model_weight_lb);
        if weight <= 0.0 {
            return None;
        }
        let pi = std::f32::consts::PI;
        // Keep the original scalar multiply/divide order, including the
        // wider-body correction when the first radius exceeds height scale.
        let density = base_weight
            / model_scale_mul(
                model_scale_mul(model_scale_mul(base_radius, pi), base_radius),
                base_radius,
            );
        let height_pi = model_scale_mul(height, pi);
        let factor = model_scale_mul(height_pi, density);
        let mut width = (weight / factor).sqrt();
        if height < width {
            // 00315518..00315540 keeps this product chain before the
            // subtraction, rather than regrouping the radius expression.
            let displaced = model_scale_mul(
                model_scale_mul(model_scale_mul(height_pi, height), height),
                density,
            );
            let radius_squared = model_scale_add_sub(
                model_scale_add_sub(weight, displaced, true) / model_scale_mul(factor, expansion),
                model_scale_mul(height, height),
                false,
            );
            width = radius_squared.sqrt();
        }
        Some([height, width, width])
    }

    /// Head-joint XYZ scale before head-size options. `3153d0` stores
    /// DB43/44/45 at skeleton+4/+8/+c; `19d59c` applies them as Y/Z/X.
    pub fn native_head_scale(&self) -> Option<[f32; 3]> {
        let body = self.native_body_scale()?;
        let factor = if matches!(self.model()?, PlayerModel::League(_)) {
            1.2 / body[0]
        } else {
            1.0
        };
        let [x, y, z] = self.body_scale;
        // 0031564c..003156ac: nearest percentage division, chopped add
        // and multiply, then skeleton +4/+8/+c; the axis map stays Y/Z/X.
        Some([y, z, x].map(|value| {
            model_scale_mul(
                factor,
                model_scale_add_sub(f32::from(value) / 100.0, 1.0, false),
            )
        }))
    }

    /// Where the player's model and textures are, as the loader 0x315790
    /// picks them. `None` when the record's appearance values are out of range.
    pub fn model(&self) -> Option<PlayerModel> {
        let unique = |model: usize, home: usize| {
            Some(PlayerModel::Unique(UniqueModel {
                model,
                road_sheet: model + 1,
                home_sheet: home,
            }))
        };
        match self.kind() {
            PlayerKind::League => self.league_model().map(PlayerModel::League),
            PlayerKind::Streetball => {
                let m = STREET_MODELS + 2 * (self.index - STREETBALLERS);
                unique(m, m + 1)
            }
            PlayerKind::Legend => {
                let m = LEGEND_MODELS + 3 * (self.index - LEGENDS);
                unique(m, m + 2)
            }
            PlayerKind::Generic => {
                let m = GENERIC_MODELS + 2 * (self.index - GENERIC);
                unique(m, m + 1)
            }
            PlayerKind::Created => self.created_model(None).map(PlayerModel::Created),
        }
    }

    fn league_model(&self) -> Option<LeagueModel> {
        let build = usize::try_from(self.build).ok()?;
        let body = *LEAGUE_BODIES.get(build)?;
        let team = usize::try_from(self.team).ok()?;
        let first = 5 * self.index;
        let accessories = ACCESSORY_SLOTS
            .iter()
            .zip(self.accessories)
            .enumerate()
            .filter_map(|(slot, (s, v))| {
                let image = usize::try_from(v).ok()?;
                Some(AccessoryPart {
                    slot,
                    name: s.name,
                    model: s.model.map(|m| m + ACCESSORY_SIZE[build]),
                    sheet: s.sheet?,
                    image,
                })
            })
            .collect();
        Some(LeagueModel {
            bodies: [body, body + 1, body + 2],
            morphs: [first, first + 1, first + 2],
            road_sheet: first + 3,
            home_sheet: first + 4,
            shorts_sheet: TEAM_SHORTS + team,
            tshirt_sheets: [TEAM_TSHIRTS + 2 * team, TEAM_TSHIRTS + 2 * team + 1],
            pants_sheet: PANTS,
            dreads: self.hair_style != 0,
            accessories,
        })
    }

    /// The create-a-player kit parts (`/XF/XF000001`) for this record's
    /// appearance fields, in its own clothes or, with `Some((team, home))`, in
    /// that league team's uniform (the loader forces clothing 80 when a created
    /// player plays for teams 0..28). Meaningful for created players only.
    pub fn created_model(&self, uniform: Option<(usize, bool)>) -> Option<CreatedModel> {
        let build = usize::try_from(self.build).ok()?.min(5);
        let kit = &KIT[build];
        // Female builds index their skin sheets from build 4 (0x31e568).
        let tone_build = if self.female {
            build.checked_sub(4)?
        } else {
            build
        };
        let raw = self.clothing as u16;
        let (clothing, variant) = match uniform {
            Some(_) => (TEAM_UNIFORM, 0),
            None => {
                let c = raw & 0x3fff;
                // 0x1d8340: top two bits, ignored above 2; not applied to team
                // uniforms (54..80) or clothing 81 (0x31594c).
                let v = (raw >> 14) as usize;
                let v = if (54..=81).contains(&c) || v > 2 {
                    0
                } else {
                    v
                };
                (c, v)
            }
        };
        // 0x31f788: clothing -> one of 11 groups -> outfit 1..6 (0x49ad58).
        let group = match clothing {
            54..=80 => 10,
            81 => 9,
            c if c < 11 => c as usize,
            _ => 10,
        };
        let outfit = OUTFITS[build][group] - 1;
        let set = kit.body + kit.stride * outfit;
        let look = clamp(self.look, kit.looks)?;
        // Hair style -1 means no head model and no hair sheet; women have no
        // facial hair slots (0x31ee10 returns -1 for both).
        let hair = match self.hair_style {
            h if h >= 0 => Some(clamp(h, kit.hair_styles)?),
            _ => None,
        };
        let beard = match self.facial_hair_style {
            b if b >= 0 && !self.female => Some(clamp(b, kit.beards)?),
            _ => None,
        };
        let shoes = usize::try_from(self.shoes).ok()?;
        let shoe_class = SHOE_CLASSES.get(shoes).copied().unwrap_or(2);
        let face_morphs = kit.face_morphs + kit.stride * outfit;
        let c = clothing as usize;
        // 0x31e600 (tops) and 0x31e5c0 (bottoms).
        let top_sheet = match clothing {
            54..=79 => Some(c + 4),
            80 => uniform.map(|(team, home)| team + if home { 29 } else { 0 }),
            81 => Some(TOPS + 28 * build + 27),
            _ => Some(TOPS + 28 * build + 3 * if c < 11 { c } else { 0 } + variant),
        };
        let bottom = match clothing {
            0..=8 => 3 * c + variant,
            81 => 28,
            _ => 27,
        };
        let hair_base = if self.female { 426 } else { 514 };
        let hair_color = usize::try_from(self.hair_color).ok();
        let beard_color = usize::try_from(self.facial_hair_color).ok();
        let skin = if self.female {
            854 + 2 * look
        } else {
            782 + 4 * look
        };
        Some(CreatedModel {
            body: set,
            head: hair.map(|h| kit.heads + h * kit.looks + look),
            face_morph: face_morphs + look,
            beard_morph: beard.map(|b| kit.morphs + kit.stride * outfit + b),
            shoe_morph: face_morphs + kit.looks + shoe_class,
            skin_sheet: skin + tone_build,
            hair_sheet: hair.zip(hair_color).map(|(h, c)| hair_base + 8 * h + c),
            beard_sheet: beard.zip(beard_color).map(|(b, c)| 650 + 6 * b + c),
            shoe_sheet: 866 + shoes,
            top_sheet,
            bottom_sheet: BOTTOMS + 29 * build + bottom,
        })
    }
}

/// `v` limited to `0..count` like the exe's `slt`/`movz` pairs in 0x31ee10;
/// `None` for negative values or an empty range.
fn clamp(v: i16, count: usize) -> Option<usize> {
    let v = usize::try_from(v).ok()?;
    (count > 0).then(|| v.min(count - 1))
}

/// `/XF/XF000002` layout after the league players (model loader 0x315790).
const STREET_MODELS: usize = 725;
const LEGEND_MODELS: usize = 751;
const GENERIC_MODELS: usize = 829;
/// `Small1Body`, `Medium1Body`, `Large1Body` by build (table at 0x49a318;
/// build 3 maps back to small); each is followed by variants 2 and 3.
const LEAGUE_BODIES: [usize; 4] = [931, 934, 937, 931];
/// Accessory model offset by build: small +4, medium +2, large +0 (0x49a6d0).
const ACCESSORY_SIZE: [usize; 4] = [4, 2, 0, 0];
const PANTS: usize = 1013;
const TEAM_SHORTS: usize = 1014;
const TEAM_TSHIRTS: usize = 1043;

/// Clothing value the loader substitutes for a team uniform.
const TEAM_UNIFORM: u16 = 80;
/// `/XF/XF000001`: per build 28 top sheets from 84, 29 bottom sheets from 252.
const TOPS: usize = 84;
const BOTTOMS: usize = 252;

/// Per-build create-a-player kit in `/XF/XF000001`, from the table at 0x40f530.
struct Kit {
    /// Entries per outfit: the body model and its morph targets.
    stride: usize,
    /// `body<b>_outfit1`.
    body: usize,
    /// First facial hair morph (after `body`).
    morphs: usize,
    beards: usize,
    /// `malehair<h>_head<n>_body<b>`: `hair_styles` x `looks` head models.
    heads: usize,
    hair_styles: usize,
    /// First face morph; `looks` faces, then three shoe morphs.
    face_morphs: usize,
    looks: usize,
}

const KIT: [Kit; 6] = {
    const fn k(
        stride: usize,
        body: usize,
        beards: usize,
        heads: usize,
        hairs: usize,
        looks: usize,
    ) -> Kit {
        Kit {
            stride,
            body,
            morphs: body + 1,
            beards,
            heads,
            hair_styles: hairs,
            face_morphs: body + 1 + beards,
            looks,
        }
    }
    [
        k(44, 886, 22, 1150, 17, 18),
        k(44, 1456, 22, 1720, 17, 18),
        k(44, 2026, 22, 2290, 17, 18),
        k(44, 2596, 22, 2860, 17, 18),
        k(10, 3166, 0, 3226, 11, 6),
        k(10, 3292, 0, 3352, 11, 6),
    ]
};

/// Outfit number (1..6) by build and clothing group (0x49ad58).
const OUTFITS: [[usize; 11]; 6] = [
    [1, 1, 2, 2, 2, 3, 4, 5, 5, 2, 6],
    [1, 1, 2, 2, 2, 3, 4, 5, 5, 1, 6],
    [1, 1, 1, 2, 2, 3, 4, 5, 5, 1, 6],
    [1, 2, 2, 2, 3, 3, 4, 5, 5, 2, 6],
    [1, 1, 2, 2, 3, 4, 5, 5, 5, 1, 6],
    [1, 1, 2, 2, 3, 4, 5, 5, 5, 1, 6],
];

/// What the create-a-player kit offers a build: (faces, hair styles, facial
/// hair styles); women have no facial hair. Hair comes in 8 colours and
/// facial hair in 6 (the sheets' strides), shoes in [`SHOE_STYLES`].
pub fn kit_limits(build: usize) -> (usize, usize, usize) {
    let k = &KIT[build.min(5)];
    (k.looks, k.hair_styles, k.beards)
}

/// Shoe styles the create-a-player kit has sheets for (866..885).
pub const SHOE_STYLES: usize = 20;
pub const HAIR_COLORS: usize = 8;
pub const FACIAL_HAIR_COLORS: usize = 6;

/// Shoe morph by shoe (0x317750, table 0x49a3b8); shoes 20+ use 2.
const SHOE_CLASSES: [usize; 20] = [0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2];

/// Where a player's model comes from. Entry numbers index `/XF/XF000002`
/// unless stated otherwise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerModel {
    League(LeagueModel),
    Unique(UniqueModel),
    /// Entries of `/XF/XF000001`.
    Created(CreatedModel),
}

/// A league player: a shared body for their build, shaped by their own morph
/// target and dressed with their own sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeagueModel {
    /// Body model per clothing variant: 0 shorts (the normal look), 1 shorts
    /// and T-shirt, 2 long pants and T-shirt. The variant only changes in
    /// story mode (0x1d81a8) and is forced to 0 with a bicep band.
    pub bodies: [usize; 3],
    /// The player's morph target for each body variant.
    pub morphs: [usize; 3],
    /// Coloured road jersey sheet (`jers`, `head`, `hand`, `body`, arms, legs,
    /// `shoe`, `eyes`, `mout`); the loader takes it when the player's side flag
    /// at +0x284 is set.
    pub road_sheet: usize,
    /// White home jersey sheet, otherwise identical.
    pub home_sheet: usize,
    /// Team shorts for the `shor` slot: one of six images, picked at random.
    pub shorts_sheet: usize,
    /// Team T-shirts (road, home) for the `tsht` slot of variants 1 and 2.
    pub tshirt_sheets: [usize; 2],
    /// Pants for the `pant` slot of variant 2: one of five images at random.
    pub pants_sheet: usize,
    /// Draw the body's `zdreds` group (hair style non-zero).
    pub dreads: bool,
    /// Accessories and the headband (which shows the body's `HEADBAND` group
    /// with the sheet's image bound to `hdbd`).
    pub accessories: Vec<AccessoryPart>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessoryPart {
    /// Index into [`ACCESSORY_SLOTS`].
    pub slot: usize,
    pub name: &'static str,
    /// Model sized for the player's build; `None` for the headband.
    pub model: Option<usize>,
    pub sheet: usize,
    /// Image in `sheet` (the exe uses image 0 if this is past the end).
    pub image: usize,
}

/// A player with a model of their own: streetballers, legends, generic ballers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UniqueModel {
    pub model: usize,
    /// The sheet used on the road side; the only sheet for non-legends.
    pub road_sheet: usize,
    /// Legends have a white home sheet; for the others this equals `road_sheet`.
    pub home_sheet: usize,
}

/// Create-a-player kit entries in `/XF/XF000001`, from the appearance slot
/// resolver 0x31ee10 and the helpers it dispatches to (0x31e4b8..0x31e600).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreatedModel {
    /// `body<b>_outfit<o>`.
    pub body: usize,
    /// `malehair<h>_head<n>_body<b>` / `femalehair...`; `None` with hair style -1.
    pub head: Option<usize>,
    /// Morph targets of `body`: the face, the facial hair (men) and the shoe shape.
    pub face_morph: usize,
    pub beard_morph: Option<usize>,
    pub shoe_morph: usize,
    /// Skin sheet (`head`, `arms`, `hand`, `legs`, `eyes`, `mout`).
    pub skin_sheet: usize,
    pub hair_sheet: Option<usize>,
    pub beard_sheet: Option<usize>,
    pub shoe_sheet: usize,
    /// Shirt or jersey; `None` for clothing 80 without a team.
    pub top_sheet: Option<usize>,
    /// Shorts, sleeves and socks.
    pub bottom_sheet: usize,
}

/// The players: the disc's own, or the roster set by [`set_roster_override`].
pub fn players<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Vec<Player>> {
    let original = Table::read(iso, PLAYERS)?;
    // An online match's shared roster stands in for everything else.
    if let Some(players) = crate::roster_edits::shared_players(&parse_players(&original)?)? {
        return Ok(players);
    }
    let mut players = match roster_override_table(PLAYERS_FILE)? {
        Some(table) => parse_players_over(&table, &original)?,
        None => parse_players(&original)?,
    };
    // The player editor's roster.json, if one is set.
    crate::roster_edits::apply_players(&mut players)?;
    Ok(players)
}

/// The disc's own players, ignoring any roster override.
pub fn original_players<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Vec<Player>> {
    parse_players(&Table::read(iso, PLAYERS)?)
}

pub fn parse_players(t: &Table) -> Result<Vec<Player>> {
    (0..t.record_count()).map(|i| Player::parse(t, i)).collect()
}

/// Players from an override table (a roster mod), checked against the disc's.
///
/// Hex-editor roster edits can leave records the mod never meant to change
/// pointing at leftover text: in the Legendary Edition, Stretch, Bonafide,
/// Biggie Little, Dime, Whitewater and the generic Tyson read "ipo eal",
/// "radley aMarcus" and the like. A record whose every non-string field is
/// unchanged from the disc keeps the disc's names.
pub fn parse_players_over(t: &Table, original: &Table) -> Result<Vec<Player>> {
    if t.field_count() != original.field_count() || t.record_count() != original.record_count() {
        return Err(Error::format(format!(
            "roster players are {} x {}, the disc's {} x {}",
            t.record_count(),
            t.field_count(),
            original.record_count(),
            original.field_count()
        )));
    }
    let mut players = parse_players(t)?;
    for p in &mut players {
        if stale_strings(t, original, p.index)? {
            let old = Player::parse(original, p.index)?;
            p.first_name = old.first_name;
            p.last_name = old.last_name;
            p.code = old.code;
        }
    }
    Ok(players)
}

/// True when `record` of the override keeps every non-string value of the
/// disc's record but its strings read differently.
fn stale_strings(t: &Table, original: &Table, record: usize) -> Result<bool> {
    if record >= original.record_count() {
        return Ok(false);
    }
    for field in 0..t.field_count() {
        if t.kind(field) != Some(FieldKind::String)
            && t.raw(record, field)? != original.raw(record, field)?
        {
            return Ok(false);
        }
    }
    for field in 0..t.field_count() {
        if t.kind(field) == Some(FieldKind::String)
            && t.string(record, field)? != original.string(record, field)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The override roster's file names, as the GameCube disc spells them.
pub const PLAYERS_FILE: &str = "xdbplyr.adf";
pub const TEAMS_FILE: &str = "xdbteam.adf";

static ROSTER_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Use the player and team tables in `dir` (`xdbplyr.adf`, `xdbteam.adf`,
/// either byte order, any letter case) instead of the disc's, for every later
/// [`players`] and [`teams`] call; `None` goes back to the disc. Either file
/// may be missing. Both are parsed once here, so a bad roster fails now.
pub fn set_roster_override(dir: Option<&Path>) -> Result<()> {
    if let Some(dir) = dir {
        if !dir.is_dir() {
            return Err(Error::format(format!(
                "roster folder {} not found",
                dir.display()
            )));
        }
        let mut found = false;
        for name in [PLAYERS_FILE, TEAMS_FILE] {
            if let Some(path) = find_file(dir, name) {
                Table::parse(std::fs::read(&path)?)
                    .map_err(|e| Error::format(format!("{}: {e}", path.display())))?;
                found = true;
            }
        }
        if !found {
            return Err(Error::format(format!(
                "{} has neither {PLAYERS_FILE} nor {TEAMS_FILE}",
                dir.display()
            )));
        }
    }
    *ROSTER_OVERRIDE.write().unwrap_or_else(|e| e.into_inner()) = dir.map(Path::to_path_buf);
    Ok(())
}

/// The folder set by [`set_roster_override`], if any.
pub fn roster_override() -> Option<PathBuf> {
    ROSTER_OVERRIDE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Where a roster mod keeps its tables: the folder itself, or the
/// `ISO Files/database` (or `database`) folder below it, as the Legendary
/// Edition is laid out.
pub fn find_roster_dir(dir: &Path) -> Option<PathBuf> {
    [
        dir.to_path_buf(),
        dir.join("database"),
        dir.join("ISO Files").join("database"),
    ]
    .into_iter()
    .find(|d| find_file(d, PLAYERS_FILE).is_some())
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
}

/// A roster mod's identity: SHA-256 over its player and team tables as
/// parsed (record and field counts, every value and string), so either
/// byte order gives the same. Online, a roster built on a mod names it by
/// this, and a server accepts it only if it holds the same mod.
pub fn roster_mod_id(dir: &Path) -> Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    for (tag, name) in [(b'P', PLAYERS_FILE), (b'T', TEAMS_FILE)] {
        hash.update([tag]);
        let Some(path) = find_file(dir, name) else {
            hash.update([0]);
            continue;
        };
        let t = Table::parse(std::fs::read(&path)?)
            .map_err(|e| Error::format(format!("{}: {e}", path.display())))?;
        hash.update((t.record_count() as u32).to_le_bytes());
        hash.update((t.field_count() as u32).to_le_bytes());
        for record in 0..t.record_count() {
            for field in 0..t.field_count() {
                if t.kind(field) == Some(FieldKind::String) {
                    let s = t.string(record, field)?.unwrap_or_default();
                    hash.update((s.len() as u32).to_le_bytes());
                    hash.update(s.as_bytes());
                } else {
                    hash.update(t.raw(record, field)?.to_le_bytes());
                }
            }
        }
    }
    Ok(hash.finalize().into())
}

/// A roster mod's players and teams (the folder holding `xdbplyr.adf`),
/// with stale names repaired from the disc's tables when given. A mod
/// without a team table takes the disc's.
pub fn roster_mod_tables(
    dir: &Path,
    original: Option<(&Table, &Table)>,
) -> Result<(Vec<Player>, Vec<Team>)> {
    let read = |name: &str| -> Result<Option<Table>> {
        match find_file(dir, name) {
            Some(path) => Table::parse(std::fs::read(&path)?)
                .map(Some)
                .map_err(|e| Error::format(format!("{}: {e}", path.display()))),
            None => Ok(None),
        }
    };
    let players = read(PLAYERS_FILE)?
        .ok_or_else(|| Error::NotFound(format!("{PLAYERS_FILE} in {}", dir.display())))?;
    let players = match original {
        Some((op, _)) => parse_players_over(&players, op)?,
        None => parse_players(&players)?,
    };
    let teams = match (read(TEAMS_FILE)?, original) {
        (Some(t), Some((_, ot))) => parse_teams_over(&t, ot)?,
        (Some(t), None) => parse_teams(&t)?,
        (None, Some((_, ot))) => parse_teams(ot)?,
        (None, None) => {
            return Err(Error::NotFound(format!(
                "{TEAMS_FILE} in {}",
                dir.display()
            )));
        }
    };
    Ok((players, teams))
}

fn roster_override_table(name: &str) -> Result<Option<Table>> {
    let Some(dir) = roster_override() else {
        return Ok(None);
    };
    let Some(path) = find_file(&dir, name) else {
        return Ok(None);
    };
    Table::parse(std::fs::read(&path)?)
        .map(Some)
        .map_err(|e| Error::format(format!("{}: {e}", path.display())))
}

// ---------------------------------------------------------------------------
// Teams (XDBTEAM.ADF): 49 records of 11 fields.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Team {
    /// Field 0, equal to the record number.
    pub index: usize,
    /// Field 1 (string): "LA Clippers", "Street Legends", "CP Team".
    pub name: String,
    /// Field 2 (string): "Los Angeles".
    pub city: String,
    /// Field 3 (string): "Clippers".
    pub nickname: String,
    /// Fields 4..9: player records, -1 for empty slots. Each league team lists
    /// its five players, whose team field points back at it.
    pub roster: Vec<usize>,
    /// Field 10: logo in `/FE/XATEAMS.BIN` (50 images; the league teams use
    /// their own index, "Random League Team" 49).
    pub logo: i16,
}

impl Team {
    /// City and nickname: "Atlanta Hawks", "Street Legends".
    pub fn full_name(&self) -> String {
        format!("{} {}", self.city, self.nickname)
    }
}

/// The teams: the disc's own, or the roster set by [`set_roster_override`].
pub fn teams<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Vec<Team>> {
    let original = Table::read(iso, TEAMS)?;
    if let Some(teams) = crate::roster_edits::shared_teams(&parse_teams(&original)?)? {
        return Ok(teams);
    }
    let mut teams = match roster_override_table(TEAMS_FILE)? {
        Some(table) => parse_teams_over(&table, &original)?,
        None => parse_teams(&original)?,
    };
    // The player editor's roster.json, if one is set.
    crate::roster_edits::apply_teams_active(&mut teams)?;
    Ok(teams)
}

/// The disc's own teams, ignoring any roster override.
pub fn original_teams<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Vec<Team>> {
    parse_teams(&Table::read(iso, TEAMS)?)
}

/// Teams from an override table, with stale names repaired from the disc's
/// (see [`parse_players_over`]).
pub fn parse_teams_over(t: &Table, original: &Table) -> Result<Vec<Team>> {
    let mut teams = parse_teams(t)?;
    let base = parse_teams(original)?;
    for (team, old) in teams.iter_mut().zip(&base) {
        if stale_strings(t, original, team.index)? {
            team.name.clone_from(&old.name);
            team.city.clone_from(&old.city);
            team.nickname.clone_from(&old.nickname);
        }
    }
    Ok(teams)
}

pub fn parse_teams(t: &Table) -> Result<Vec<Team>> {
    (0..t.record_count())
        .map(|i| {
            let f = t.record(i)?;
            if f.len() < 11 {
                return Err(Error::format("team table has fewer than 11 fields"));
            }
            let text = |field| t.string(i, field).map(Option::unwrap_or_default);
            Ok(Team {
                index: i,
                name: text(1)?,
                city: text(2)?,
                nickname: text(3)?,
                roster: f[4..10]
                    .iter()
                    .filter_map(|&p| usize::try_from(p).ok())
                    .collect(),
                logo: f[10],
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Courts (XDBCOURT.ADF): 11 records of 10 fields.

#[derive(Clone, Debug, PartialEq)]
pub struct Court {
    pub index: usize,
    /// Field 1 (string, getter 0x1d3670): "Rucker Park".
    pub name: String,
    /// Field 2 (string, getter 0x1d36e8): "Harlem, NY".
    pub location: String,
    /// Field 3 (string): the court archive, `/XF/<ID>.VIV`, upper-cased here.
    pub id: String,
    /// Field 4: 0, 1 or 2; meaning unknown.
    pub unknown: i16,
    /// Fields 5..9: indices into pool A, which holds `f32`s (getter 0x1d3880).
    /// They sum to 1 per court; 0x1dbe30 multiplies pool sizes by them. Guess:
    /// the share of opponents from five difficulty tiers (the first court is
    /// all tier 0, Rucker Park 90% tier 4).
    pub tier_mix: [f32; 5],
}

pub fn courts<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Vec<Court>> {
    let t = Table::read(iso, COURTS)?;
    (0..t.record_count())
        .map(|i| {
            let text = |field| t.string(i, field).map(Option::unwrap_or_default);
            let mut tier_mix = [0.0; 5];
            for (k, mix) in tier_mix.iter_mut().enumerate() {
                *mix = f32::from_bits(t.pool_word(i, 5 + k)?.unwrap_or(0));
            }
            Ok(Court {
                index: i,
                name: text(1)?,
                location: text(2)?,
                id: text(3)?.to_ascii_uppercase(),
                unknown: t.raw(i, 4)?,
                tier_mix,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Moves (XDBMOVE.ADF): 93 records of 6 fields. Meanings read from the data.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub index: usize,
    /// Field 1 (string): "Rope-a-Dope", "Spin da LP", "Breaking You Off".
    pub name: String,
    /// Field 2: 0 layup, 1 dunk, 2 dribble move (by the names).
    pub kind: i16,
    /// Field 3: level 2..4.
    pub level: i16,
    /// Field 4: 50, 125 or 250, rising with the level (points or price).
    pub cost: i16,
    /// Field 5: move id, unique except for dribble moves listed at two levels.
    pub move_id: i16,
}

pub fn moves<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Vec<Move>> {
    let t = Table::read(iso, MOVES)?;
    (0..t.record_count())
        .map(|i| {
            Ok(Move {
                index: i,
                name: t.string(i, 1)?.unwrap_or_default(),
                kind: t.raw(i, 2)?,
                level: t.raw(i, 3)?,
                cost: t.raw(i, 4)?,
                move_id: t.raw(i, 5)?,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Reward cards (XDBCARD.ADF): 113 records of 6 fields. The shop: Street
// Legends, Legends, courts, jerseys, development points, tips and
// codes; the names come from the locale's
// `poster.txt` (#9..16 name the categories, #41..51 the courts).

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// Field 0: the card's own id (equal to the record number).
    pub id: i16,
    /// Field 1: 0 Street Legends, 1 Legends, 2 courts, 3 jerseys,
    /// 4 development points, 5 tips, 6 cheat codes.
    pub category: i16,
    /// Field 2: the entry's index within the category (development points:
    /// the amount).
    pub index: i16,
    /// Field 3: the price in Reward Points.
    pub cost: i16,
    /// Field 4: an icon/poster reference (`poster.txt` or a sheet image).
    pub poster: i16,
    /// Field 5: automatic Pick Up win threshold (positive), otherwise -1.
    /// The first three Street Legends use 10/15/20, tips 15..53 and
    /// pictured cheat cards 40..59. This is not a button sequence.
    pub param: i16,
}

pub fn cards<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Vec<Card>> {
    let t = Table::read(iso, CARDS)?;
    (0..t.record_count())
        .map(|i| {
            let f = t.record(i)?;
            if f.len() < 6 {
                return Err(Error::format("card table has fewer than 6 fields"));
            }
            Ok(Card {
                id: f[0],
                category: f[1],
                index: f[2],
                cost: f[3],
                poster: f[4],
                param: f[5],
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Nicknames (XDBALIAS.ADF): 48 records of 3 fields.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nickname {
    /// Field 0: id (1..57 with gaps).
    pub id: i16,
    /// Field 1 (string): "Back At'cha", "Splash".
    pub name: String,
    /// Field 2: the skill the nickname goes with, seven per skill ("Denial"
    /// and "Vengeance" are blocks, "Splash" and "Rain" shots).
    pub skill: Option<Skill>,
}

pub fn nicknames<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Vec<Nickname>> {
    let t = Table::read(iso, NICKNAMES)?;
    (0..t.record_count())
        .map(|i| {
            Ok(Nickname {
                id: t.raw(i, 0)?,
                name: t.string(i, 1)?.unwrap_or_default(),
                skill: Skill::from_index(t.raw(i, 2)?),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-field table: a value and a string, three records.
    fn sample() -> Vec<u8> {
        let mut d = Vec::new();
        // Records start at 0x1c and end at 0x28, where the pools and strings begin.
        for w in [2u32, 3, 0x28, 0x28, 0x28, 0] {
            d.extend(w.to_le_bytes());
        }
        d.extend([0b0001_0000, 0, 0, 0]); // kinds: Value, String
        for (v, s) in [(7i16, 0i16), (-1, 2), (9, -1)] {
            d.extend(v.to_le_bytes());
            d.extend(s.to_le_bytes());
        }
        d.extend(b"a\0bc\0");
        d
    }

    #[test]
    fn model_scale_operations_keep_chopped_products_and_signed_guard_bits() {
        assert_eq!(
            model_scale_mul(78.0, f32::from_bits(0x3f99_999a)).to_bits(),
            0x42bb_3333
        );
        for (a, b, subtract, expected) in [
            (0x3f80_0000, 0x33ff_ffff, true, 0x3f7f_ffff),
            (0x33ff_ffff, 0x3f80_0000, true, 0xbf7f_ffff),
            (0x3f80_0000, 0x337f_ffff, true, 0x3f80_0000),
            (0xbf80_0000, 0x3f7f_ffff, false, 0xb380_0000),
            (0x8000_0000, 0x8000_0000, false, 0x8000_0000),
        ] {
            assert_eq!(
                model_scale_add_sub(f32::from_bits(a), f32::from_bits(b), subtract).to_bits(),
                expected
            );
        }
    }

    #[test]
    fn reads_values_and_strings() {
        let t = Table::parse(sample()).unwrap();
        assert_eq!((t.field_count(), t.record_count()), (2, 3));
        assert_eq!(t.kind(0), Some(FieldKind::Value));
        assert_eq!(t.kind(1), Some(FieldKind::String));
        assert_eq!(t.raw(1, 0).unwrap(), -1);
        assert_eq!(t.string(0, 1).unwrap().as_deref(), Some("a"));
        assert_eq!(t.string(1, 1).unwrap().as_deref(), Some("bc"));
        assert_eq!(t.string(2, 1).unwrap(), None);
        assert!(t.string(0, 0).is_err());
        assert!(t.raw(3, 0).is_err());
    }

    #[test]
    fn rejects_records_overlapping_the_pools() {
        let mut d = sample();
        d[4] = 4; // four records no longer fit before the pools
        assert!(Table::parse(d).is_err());
        assert!(Table::parse(vec![0; 8]).is_err());
    }

    #[test]
    fn skills_read_their_ratings() {
        let ratings: Vec<usize> = Skill::ALL.iter().map(|s| s.rating()).collect();
        assert_eq!(ratings, [0, 2, 4, 5, 6, 7, 3]);
        assert_eq!(Skill::from_index(6), Some(Skill::Rebounds));
        assert_eq!(Skill::from_index(7), None);
    }
}
