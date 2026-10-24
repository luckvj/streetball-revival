//! Whole characters from the GameCube discs of Street V3 (`G3VE69`,
//! with the Nintendo All-Stars) and Street V1 (`GNSE69`), assembled the way
//! the games do it and returned as a neutral [`Character`].
//!
//! ```no_run
//! # fn main() -> street_assets::Result<()> {
//! use street_assets::{gc_character, gcdisc::GcDisc};
//! let mut disc = GcDisc::open("Street V3 (USA).rvz")?;
//! for c in gc_character::list(&mut disc)? {
//!     println!("{} {}", c.id, c.name);
//! }
//! let mario = gc_character::assemble(&mut disc, "mario")?;
//! # Ok(()) }
//! ```
//!
//! # V3: appearance recipes
//!
//! A player record (`/database/xdbplyr.ndf`, 109 fields) lists up to 60
//! appearance ids (fields 30..89, pool B): 30..69 the home look, 70..74 what
//! the away look changes (tops, bottoms, socks, shoes), 75..89 a second look
//! some players have. Each id is a record of `/database/xdbagame.ndf`: id,
//! slot (`Tops`), type (`Jersey`), a comma-separated recipe, a flag. Recipe
//! directives, as `main.dol` 0x801fa8bc reads them:
//!
//! - `aref:N` inlines record N's recipe; `arefmale:N`/`areffemale:N` only for
//!   that sex (player field 22 is 1 for women).
//! - `add:N` adds record N as an item of its own (a skin type adds the bare
//!   torso, hands and legs this way).
//! - `mod:Name?skeletontype=..&shadowtype=..&shorts=true..` a model
//!   (`name.o` in `/xf/player.viv`).
//! - `tex:slot_sex_set_layer_variant?tattoo=true..` a texture layer.
//! - `morph:`, `haircolour:N`, `jtext:` (jersey lettering), `cref:`,
//!   `removehat...`: the rest.
//!
//! Adding an item first removes the items with the same slot and type
//! (0x801fda28). Models are grouped by item slot and the newest one in a
//! group is drawn (adding one hides the group's first). Texture layers are
//! grouped by the first four letters of their name (`hairhats_...` binds the
//! model slot `hair`) and drawn in layer order, each at the position stored
//! in its sheet, with alpha. Names ending in `skinfix`/`skinkidfix`/
//! `<x>hairfix` pick the player's skin tone or hair colour (0x801f6920):
//! `skinfix` -> `skinNN`, `skinkidfix` -> `kidNN` for kids, `browthreehairfix`
//! -> `browthree<colour><a|b>` (`b` for skin tones above 6). The skin tone is
//! the one the skin type's hands texture names (`hand_m_master_00_skin06`).
//!
//! Left out: blink lids (slot `Blink`), shadows, jersey lettering (drawn
//! from TrueType fonts at run time) and morph targets (`*_morph.o`: body
//! types and created faces).

use std::collections::{BTreeMap, HashMap};

use crate::{
    Error, Result,
    big::BigEntry,
    character::{Character, Mesh, Texture},
    db::Table,
    eagl::{Bone, Skeleton},
    eagl_gc,
    gcdisc::GcDisc,
    gsh,
};

const V3_PLAYERS: &str = "/database/xdbplyr.ndf";
const V3_TEAMS: &str = "/database/xdbteam.ndf";
const V3_LOOKS: &str = "/database/xdbagame.ndf";
const V3_ARCHIVE: &str = "/xf/player.viv";

/// Which GameCube game a disc holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Game {
    /// Street V3 (`G3VE69`).
    V3,
    /// Street V1, the first game (`GNSE69`).
    V1,
}

impl Game {
    pub fn from_id(game_id: &str) -> Option<Self> {
        match game_id.get(..4)? {
            "G3VE" | "G3VP" | "G3VJ" | "G3VD" | "G3VF" => Some(Self::V3),
            "GNSE" | "GNSP" | "GNSJ" | "GNSD" | "GNSF" => Some(Self::V1),
            _ => None,
        }
    }

    /// The prefix of this game's character ids (`v3gc`, `v1gc`).
    pub fn prefix(self) -> &'static str {
        match self {
            Self::V3 => "v3gc",
            Self::V1 => "v1gc",
        }
    }
}

/// What a character is in the game's own grouping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    League,
    /// legends (Erving, Bird, ...).
    Legend,
    /// Street legends (V3's Bonafide, Stretch, ...; Vol. 1's streetballers).
    Street,
    /// Guests: V3's Nintendo All-Stars and Beastie Boys.
    Guest,
    /// Generic streetballers.
    Generic,
    /// Create-a-player starting points.
    Preset,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::League => "league",
            Self::Legend => "legend",
            Self::Street => "street",
            Self::Guest => "guest",
            Self::Generic => "generic",
            Self::Preset => "preset",
        }
    }
}

/// One playable character and the facts the game's table gives.
#[derive(Clone, Debug)]
pub struct CharacterInfo {
    /// `v3gc:<key>` / `v1gc:<key>`.
    pub id: String,
    pub game: Game,
    /// Lower-case name, unique on the disc (`mario`, `shawn_marion`).
    pub key: String,
    /// Record number in the player table.
    pub record: usize,
    /// The game's own player id (field 0).
    pub db_id: i32,
    pub name: String,
    pub first_name: String,
    pub last_name: String,
    /// Nickname or speech name, when the table has one (`TheMatrix`).
    pub nickname: Option<String>,
    pub kind: Kind,
    /// Team id (the team table's field 0) and name.
    pub team: Option<i32>,
    pub team_name: Option<String>,
    pub jersey: Option<i32>,
    /// 0 C, 1 PF, 2 SF, 3 SG, 4 PG.
    pub position: Option<i32>,
    pub height_in: Option<i32>,
    pub weight_lb: Option<i32>,
    pub female: bool,
    pub kid: bool,
    /// The rating fields as stored (V3: fields 91..101, 0..99).
    pub ratings: Vec<i32>,
    /// Every field of the record: numbers as stored, pool words resolved,
    /// strings as `None` (see `strings`).
    pub raw: Vec<i64>,
    pub strings: BTreeMap<usize, String>,
}

/// A team from the game's team table.
#[derive(Clone, Debug)]
pub struct TeamInfo {
    pub id: i32,
    pub name: String,
    pub city: Option<String>,
    pub nickname: Option<String>,
    /// The roster as character ids, in table order.
    pub roster: Vec<String>,
}

/// Which of a V3 player's looks to assemble.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Look {
    #[default]
    Home,
    /// The home look with the away uniform (fields 70..74).
    Away,
    /// The second look some players have (fields 75..89 over the home look).
    Alternate,
}

/// The roster and appearance data of one GameCube disc, read once.
pub struct Library {
    pub game: Game,
    pub characters: Vec<CharacterInfo>,
    pub teams: Vec<TeamInfo>,
    inner: Inner,
}

enum Inner {
    V3(Box<V3>),
    V1(crate::gc_v1::V1),
}

/// V3's animation banks (`/xf/anims.viv`): `anibig.bin` (in game),
/// `anibfe.bin` (front end, `createplayer_stand`), `anibtrex.bin` (the
/// Nintendo All-Stars), `anibs2.bin`, `anibin.bin`, `anibju.bin`, ...
pub const V3_ANIMATIONS: &str = "/xf/anims.viv";

/// The names of V3's animation banks.
pub fn v3_animation_banks(disc: &mut GcDisc) -> Result<Vec<String>> {
    Ok(disc
        .big_index(V3_ANIMATIONS)?
        .into_iter()
        .map(|e| e.name)
        .filter(|n| n.ends_with(".bin"))
        .collect())
}

/// One of V3's animation banks, e.g. `anibfe.bin`; read by [`crate::anim`]
/// like Vol. 2's (big-endian). Bone numbers index the 44-bone skeleton
/// (`skeleton.o`), or the Nintendo skeletons for `anibtrex.bin`.
pub fn v3_animation_bank(disc: &mut GcDisc, name: &str) -> Result<crate::anim::Bank> {
    let entry = disc
        .big_index(V3_ANIMATIONS)?
        .into_iter()
        .find(|e| e.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::NotFound(format!("{V3_ANIMATIONS} {name}")))?;
    crate::anim::Bank::parse(&disc.big_entry(V3_ANIMATIONS, &entry)?)
        .map_err(|e| Error::format(format!("{name}: {e}")))
}

/// Vol. 2's 22 body bones in its own order: the channels of its animations,
/// and of V3's few animations made for Vol. 2's rig (rig `0x1dc5`,
/// `createplayer_stand`).
pub const VOL2_BODY_BONES: [&str; 22] = [
    "Root",
    "LThigh",
    "LLowLeg",
    "LFoot",
    "LToe",
    "RThigh",
    "RLowLeg",
    "RFoot",
    "RToe",
    "LowerTorso",
    "MidTorso",
    "UpperTorso",
    "LCollarBone",
    "LUpperArm",
    "LLowArm",
    "LHand",
    "RCollarBone",
    "RUpperArm",
    "RLowArm",
    "RHand",
    "Neck",
    "Head",
];

/// The same bones in `skeleton.o`'s order: the channels of V3's own body
/// animations (rigs `0xf79b`, `0xc036` and the rest). Channels 22 and 23,
/// used by a few, drive no bone of the body.
pub const V3_BODY_BONES: [&str; 22] = [
    "Root",
    "LowerTorso",
    "MidTorso",
    "UpperTorso",
    "Neck",
    "Head",
    "RCollarBone",
    "RUpperArm",
    "RLowArm",
    "RHand",
    "LCollarBone",
    "LUpperArm",
    "LLowArm",
    "LHand",
    "RThigh",
    "RLowLeg",
    "RFoot",
    "RToe",
    "LThigh",
    "LLowLeg",
    "LFoot",
    "LToe",
];

/// Rig hash of the Nintendo All-Stars' animations (`anibtrex.bin`).
pub const NINTENDO_RIG: u16 = 0xd423;
/// Rig hash of V3's animations made for Vol. 2's rig.
pub const VOL2_RIG_IN_V3: u16 = 0x1dc5;

/// The skeletons V3's animations are made for, bone `i` driven by channel
/// `i` ([`crate::anim::Bank::rig`] says which an animation uses):
///
/// - `body`: `skeleton.o`'s [`V3_BODY_BONES`] (V3's own animations);
/// - `vol2`: the same bones in [`VOL2_BODY_BONES`] order with the thighs
///   turned half a turn about their z axis. That is Vol. 2's rig exactly:
///   its rest pose matches Vol. 2's skeleton, and a thigh channel at rest
///   reads `(0, 0, 0.999, 0.044)` where `skeleton.o` holds `(0, 0, -0.044,
///   0.999)`. Vol. 2's own animations play through it too.
/// - `nintendo`: `skel_nintendo_mario.o` (Luigi's file is the same rig).
pub struct V3Rigs {
    pub body: Skeleton,
    pub vol2: Skeleton,
    pub nintendo: Skeleton,
}

impl V3Rigs {
    pub fn load(disc: &mut GcDisc) -> Result<Self> {
        let index = disc.big_index(V3_ARCHIVE)?;
        let mut skeleton = |name: &str| -> Result<Skeleton> {
            let entry = index
                .iter()
                .find(|e| e.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| Error::NotFound(name.into()))?;
            eagl_gc::parse_skeleton(&disc.big_entry(V3_ARCHIVE, entry)?)
        };
        let stock = skeleton("skeleton.o")?;
        Ok(Self {
            body: subset(&stock, &V3_BODY_BONES, false)?,
            vol2: subset(&stock, &VOL2_BODY_BONES, true)?,
            nintendo: skeleton("skel_nintendo_mario.o")?,
        })
    }

    /// The rig for an animation's rig hash.
    pub fn for_rig(&self, rig: u16) -> &Skeleton {
        match rig {
            NINTENDO_RIG => &self.nintendo,
            VOL2_RIG_IN_V3 | 0xc016 => &self.vol2,
            _ => &self.body,
        }
    }
}

/// `names` from `stock`, in that order, parents remapped; with `roll`, the
/// thighs turned half a turn about z (their children's offsets with them).
fn subset(stock: &Skeleton, names: &[&str], roll: bool) -> Result<Skeleton> {
    let index = |name: &str| {
        stock
            .bones
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| Error::NotFound(format!("bone {name}")))
    };
    let mut bones = Vec::with_capacity(names.len());
    for name in names {
        let mut b = stock.bones[index(name)?].clone();
        let parent = b.parent.map(|p| stock.bones[p].name.clone());
        b.parent = parent
            .as_ref()
            .and_then(|p| names.iter().position(|n| n.eq_ignore_ascii_case(p)));
        if roll && name.ends_with("Thigh") {
            // q * (0, 0, 1, 0).
            let [x, y, z, w] = b.rotation;
            b.rotation = [y, -x, w, -z];
        }
        if roll && parent.is_some_and(|p| p.ends_with("Thigh")) {
            b.translation = [-b.translation[0], -b.translation[1], b.translation[2]];
        }
        bones.push(b);
    }
    Ok(Skeleton { bones })
}

/// A character's local pose for one frame of an animation made for `rig`
/// (the channels' bones and their rotations), retargeted by bone name.
/// Works across families: V3 or Vol. 2 body animations on the Nintendo
/// All-Stars, Mario's on a league player.
pub fn v3_pose(
    character: &Character,
    rig: &Skeleton,
    bones: &[u8],
    frame: &[[f32; 4]],
) -> crate::retarget::Locals {
    let mut rotations: Vec<[f32; 4]> = rig.bones.iter().map(|b| b.rotation).collect();
    for (&bone, &q) in bones.iter().zip(frame) {
        if let Some(r) = rotations.get_mut(bone as usize) {
            *r = q;
        }
    }
    crate::retarget::Retarget::new(rig, &character.skeleton).pose(&rotations)
}

/// Street V1's animation files (see [`V1Animations`]).
pub use crate::gc_v1::V1Animations;

/// The characters on a disc.
pub fn list(disc: &mut GcDisc) -> Result<Vec<CharacterInfo>> {
    Ok(Library::open(disc)?.characters)
}

/// One character by key (`mario`) or id (`v3gc:mario`), home look.
pub fn assemble(disc: &mut GcDisc, key: &str) -> Result<Character> {
    Library::open(disc)?.assemble(disc, key, Look::Home)
}

impl Library {
    pub fn open(disc: &mut GcDisc) -> Result<Self> {
        let game = Game::from_id(disc.game_id()).ok_or_else(|| {
            Error::format(format!("{} is not Street V1 or Street V3", disc.game_id()))
        })?;
        let (characters, teams, inner) = match game {
            Game::V3 => {
                let v3 = V3::open(disc)?;
                let (c, t) = v3.roster()?;
                (c, t, Inner::V3(Box::new(v3)))
            }
            Game::V1 => {
                let v1 = crate::gc_v1::V1::open(disc)?;
                let (mut c, _) = v1.roster()?;
                let t = v1.teams(disc, &mut c)?;
                (c, t, Inner::V1(v1))
            }
        };
        Ok(Self {
            game,
            characters,
            teams,
            inner,
        })
    }

    /// The character with this key or id.
    pub fn find(&self, key: &str) -> Option<&CharacterInfo> {
        let key = key
            .strip_prefix(self.game.prefix())
            .and_then(|k| k.strip_prefix(':'))
            .unwrap_or(key)
            .to_ascii_lowercase();
        self.characters.iter().find(|c| c.key == key)
    }

    pub fn assemble(&self, disc: &mut GcDisc, key: &str, look: Look) -> Result<Character> {
        let info = self
            .find(key)
            .ok_or_else(|| Error::NotFound(format!("character {key}")))?
            .clone();
        match &self.inner {
            Inner::V3(v3) => v3.assemble(disc, &info, look),
            Inner::V1(v1) => v1.assemble(disc, &info, look),
        }
    }

    /// A stock body's height in the game's units (the first league player's),
    /// for [`model_height_in`]; 0 when none assembles.
    pub fn stock_units(&self, disc: &mut GcDisc) -> f32 {
        self.characters
            .iter()
            .find(|c| c.kind == Kind::League)
            .and_then(|c| self.assemble(disc, &c.key, Look::Home).ok())
            .map_or(0.0, |c| c.height_units())
    }

    /// V3 only: the items a character's look resolves to, for inspection.
    pub fn recipe(&self, key: &str, look: Look) -> Result<Recipe> {
        let info = self
            .find(key)
            .ok_or_else(|| Error::NotFound(format!("character {key}")))?;
        match &self.inner {
            Inner::V3(v3) => v3.recipe(info, look),
            Inner::V1(_) => Err(Error::format("Street V1 has no appearance recipes")),
        }
    }
}

/// The character as the game draws it, soles to crown, in inches. The games
/// scale a model by the player's table height, a stock body standing for
/// that height; V3's Nintendo stars have bodies of their own, shorter or
/// taller than the stock one by their models' proportion (Mario's 75 in
/// draws about 55 in tall). `stock_units` is [`Library::stock_units`].
pub fn model_height_in(info: &CharacterInfo, c: &Character, stock_units: f32) -> f32 {
    let table = info.height_in.unwrap_or(78) as f32;
    let stock = matches!(c.skeleton.bones.len(), 43 | 44);
    let rigid = c.notes.iter().any(|n| n.contains("rigid"));
    if info.game == Game::V3 && (!stock || rigid) && stock_units > 0.0 {
        table * c.height_units() / stock_units
    } else {
        table
    }
}

/// A name as shown: localisation keys lose their prefix (`Kt_Nene` ->
/// `Nene`, `KtCM_BIG_MAN` -> `Big Man`).
pub(crate) fn clean_name(s: &str) -> String {
    let Some(rest) = ["KtCM_", "Kt_", "KT_", "Kt"]
        .iter()
        .find_map(|p| s.strip_prefix(p))
    else {
        return s.to_owned();
    };
    if rest.chars().any(|c| c.is_ascii_lowercase()) {
        return rest.replace('_', " ");
    }
    rest.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| {
                f.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `Shawn Marion` -> `shawn_marion`.
pub(crate) fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_owned()
}

/// Gives each character a unique key: its name's slug, or the slug and the
/// game's id when an earlier (higher priority) character has the name.
pub(crate) fn assign_keys(characters: &mut [CharacterInfo]) {
    let priority = |k: Kind| match k {
        Kind::Guest => 0,
        Kind::League | Kind::Legend | Kind::Street => 1,
        Kind::Generic => 2,
        Kind::Preset => 3,
    };
    let mut order: Vec<usize> = (0..characters.len()).collect();
    order.sort_by_key(|&i| (priority(characters[i].kind), characters[i].record));
    let mut used = std::collections::HashSet::new();
    for i in order {
        let c = &mut characters[i];
        let base = match slug(&c.name) {
            s if s.is_empty() => format!("player_{}", c.db_id),
            s => s,
        };
        let key = if used.contains(&base) {
            format!("{base}_{}", c.db_id)
        } else {
            base
        };
        used.insert(key.clone());
        c.id = format!("{}:{key}", c.game.prefix());
        c.key = key;
    }
}

// ---------------------------------------------------------------------------
// V3

/// One appearance item after its recipe is read.
#[derive(Clone, Debug, Default)]
pub struct Item {
    /// The look record's id.
    pub id: u32,
    pub slot: String,
    pub kind: String,
    pub models: Vec<ModelRef>,
    pub textures: Vec<TextureRef>,
}

#[derive(Clone, Debug)]
pub struct ModelRef {
    /// File name without `.o`, lower case.
    pub name: String,
    pub skeleton: Option<String>,
    pub shorts: bool,
}

#[derive(Clone, Debug)]
pub struct TextureRef {
    /// As the recipe writes it, lower case, without options.
    pub name: String,
    pub tattoo: bool,
}

/// A look, resolved: the items in the order the game keeps them, and the
/// player-wide settings the recipe set.
#[derive(Clone, Debug, Default)]
pub struct Recipe {
    pub items: Vec<Item>,
    /// Skin tone 1..12 (0 when no hands texture names one).
    pub skin: u32,
    /// Hair colour 1..4.
    pub hair_colour: u32,
    pub female: bool,
    pub kid: bool,
    /// Morph weights the recipe set (name, 0..1), not applied.
    pub morphs: Vec<(String, f32)>,
}

impl Recipe {
    /// The models drawn: per item slot, the newest model (a group's first
    /// model is hidden once another joins it). Blink lids are left out.
    pub fn models(&self) -> Vec<(&str, &ModelRef)> {
        let mut groups: Vec<(&str, Vec<&ModelRef>)> = Vec::new();
        for item in &self.items {
            if item.slot.eq_ignore_ascii_case("blink") {
                continue;
            }
            for m in &item.models {
                match groups
                    .iter_mut()
                    .find(|(s, _)| s.eq_ignore_ascii_case(&item.slot))
                {
                    Some((_, g)) => g.push(m),
                    None => groups.push((&item.slot, vec![m])),
                }
            }
        }
        groups
            .into_iter()
            .flat_map(|(slot, g)| {
                let skip = usize::from(g.len() > 1);
                g.into_iter().skip(skip).map(move |m| (slot, m))
            })
            .collect()
    }

    /// Texture layers by four-letter group, each `(layer, file name)` in
    /// drawing order, with the skin and hair names filled in.
    pub fn texture_layers(&self) -> BTreeMap<String, Vec<(i32, String)>> {
        let mut groups: BTreeMap<String, Vec<(i32, String)>> = BTreeMap::new();
        for item in &self.items {
            for t in &item.textures {
                let parts: Vec<&str> = t.name.split('_').collect();
                let group: String = parts[0].chars().take(4).collect();
                let layer = parts
                    .len()
                    .checked_sub(2)
                    .and_then(|i| parts[i].parse().ok())
                    .unwrap_or(0);
                groups
                    .entry(group)
                    .or_default()
                    .push((layer, self.file_name(&parts)));
            }
        }
        for layers in groups.values_mut() {
            layers.sort_by_key(|(l, _)| *l);
        }
        groups
    }

    /// 0x801f6920: a texture name with its `skinfix`/`hairfix` variant
    /// replaced by the player's own.
    fn file_name(&self, parts: &[&str]) -> String {
        let Some((variant, head)) = parts.split_last() else {
            return String::new();
        };
        if head.len() < 4 {
            return parts.join("_");
        }
        let stem = head[..4].join("_");
        if *variant == "skinkidfix" && self.kid {
            format!("{stem}_kid{:02}", self.skin.max(1))
        } else if *variant == "skinfix" || *variant == "skinkidfix" {
            format!("{stem}_skin{:02}", self.skin.max(1))
        } else if let Some(prefix) = variant.strip_suffix("hairfix") {
            let side = if self.skin > 6 { 'b' } else { 'a' };
            format!("{stem}_{prefix}{}{side}", self.hair_colour.max(1))
        } else {
            parts.join("_")
        }
    }
}

struct V3 {
    players: Table,
    teams: Table,
    /// Look record id -> (slot, type, recipe).
    looks: HashMap<u32, (String, String, String)>,
    /// Lower-case entry name -> entry of `/xf/player.viv`.
    entries: HashMap<String, BigEntry>,
}

/// The first appearance field and the end of each look's fields.
const LOOK_FIELDS: usize = 30;
const HOME_END: usize = 70;
const AWAY_END: usize = 75;
const ALT_END: usize = 90;

impl V3 {
    fn open(disc: &mut GcDisc) -> Result<Self> {
        let players = Table::parse(disc.read(V3_PLAYERS)?)?;
        let teams = Table::parse(disc.read(V3_TEAMS)?)?;
        let looks_table = Table::parse(disc.read(V3_LOOKS)?)?;
        let mut looks = HashMap::new();
        for r in 0..looks_table.record_count() {
            let Some(id) = looks_table.pool_word(r, 0)? else {
                continue;
            };
            let s = |f| -> Result<String> { Ok(looks_table.string(r, f)?.unwrap_or_default()) };
            looks.insert(id, (s(1)?, s(2)?, s(3)?));
        }
        let entries = disc
            .big_index(V3_ARCHIVE)?
            .into_iter()
            .map(|e| (e.name.to_ascii_lowercase(), e))
            .collect();
        Ok(Self {
            players,
            teams,
            looks,
            entries,
        })
    }

    fn roster(&self) -> Result<(Vec<CharacterInfo>, Vec<TeamInfo>)> {
        let t = &self.teams;
        let mut team_names = HashMap::new();
        for r in 0..t.record_count() {
            let id = i32::from(t.raw(r, 0)?);
            let name = t.string(r, 1)?.unwrap_or_default();
            let city = t
                .string(r, 2)?
                .filter(|s| s != "N/A" && !s.starts_with("KT"));
            let nick = t
                .string(r, 3)?
                .filter(|s| s != "N/A" && !s.starts_with("KT"));
            team_names.insert(id, team_name(&name, city.as_deref(), nick.as_deref()));
        }
        let p = &self.players;
        let mut characters = Vec::new();
        for r in 0..p.record_count() {
            let field = |f: usize| p.raw(r, f).map(i32::from);
            let kind = match field(5)? {
                0 => Kind::League,
                1 => Kind::Street,
                2 => Kind::Legend,
                3 => Kind::Guest,
                4 => Kind::Generic,
                5 => Kind::Preset,
                _ => continue,
            };
            let has_look = (LOOK_FIELDS..HOME_END)
                .any(|f| p.pool_word(r, f).ok().flatten().is_some_and(|v| v != 0));
            if !has_look {
                continue;
            }
            let last = clean_name(&p.string(r, 1)?.unwrap_or_default());
            let first = clean_name(&p.string(r, 2)?.unwrap_or_default());
            let name = format!("{first} {last}").trim().to_owned();
            let team = Some(field(12)?).filter(|&t| t >= 0);
            let opt = |v: i32| Some(v).filter(|&v| v >= 0);
            let mut raw = Vec::new();
            let mut strings = BTreeMap::new();
            for f in 0..p.field_count() {
                match p.kind(f) {
                    Some(crate::db::FieldKind::String) => {
                        if let Some(s) = p.string(r, f)? {
                            strings.insert(f, s);
                        }
                        raw.push(i64::from(p.raw(r, f)?));
                    }
                    Some(crate::db::FieldKind::PoolA | crate::db::FieldKind::PoolB) => {
                        raw.push(p.pool_word(r, f)?.map_or(-1, i64::from))
                    }
                    _ => raw.push(i64::from(p.raw(r, f)?)),
                }
            }
            characters.push(CharacterInfo {
                id: String::new(),
                game: Game::V3,
                key: String::new(),
                record: r,
                db_id: field(0)?,
                name: if name.is_empty() { last.clone() } else { name },
                first_name: first,
                last_name: last,
                nickname: p.string(r, 14)?.filter(|s| !s.is_empty()),
                kind,
                team,
                team_name: team.and_then(|t| team_names.get(&t).cloned()),
                jersey: opt(field(4)?),
                position: if kind == Kind::Preset {
                    None
                } else {
                    opt(field(13)?)
                },
                height_in: Some(field(18)?).filter(|&h| h > 0),
                weight_lb: Some(field(19)?).filter(|&w| w > 0),
                female: field(22)? == 1,
                kid: field(23)? == 1,
                ratings: (91..102).map(field).collect::<Result<_>>()?,
                raw,
                strings,
            });
        }
        assign_keys(&mut characters);
        let by_db: HashMap<i32, &str> = characters
            .iter()
            .map(|c| (c.db_id, c.id.as_str()))
            .collect();
        let mut teams = Vec::new();
        for r in 0..t.record_count() {
            let id = i32::from(t.raw(r, 0)?);
            let mut roster: Vec<String> = Vec::new();
            for f in 5..11 {
                if let Some(c) = by_db.get(&i32::from(t.raw(r, f)?))
                    && !roster.iter().any(|x| x == c)
                {
                    roster.push((*c).to_owned());
                }
            }
            let city = t
                .string(r, 2)?
                .filter(|s| s != "N/A" && !s.starts_with("KT"));
            let nickname = t
                .string(r, 3)?
                .filter(|s| s != "N/A" && !s.starts_with("KT"));
            teams.push(TeamInfo {
                id,
                name: team_names[&id].clone(),
                city,
                nickname,
                roster,
            });
        }
        Ok((characters, teams))
    }

    fn recipe(&self, info: &CharacterInfo, look: Look) -> Result<Recipe> {
        let p = &self.players;
        let r = info.record;
        let fields: Vec<usize> = match look {
            Look::Home => (LOOK_FIELDS..HOME_END).collect(),
            Look::Away => (LOOK_FIELDS..AWAY_END).collect(),
            Look::Alternate => (LOOK_FIELDS..HOME_END).chain(AWAY_END..ALT_END).collect(),
        };
        let mut state = Recipe {
            female: info.female,
            kid: info.kid,
            hair_colour: 1,
            ..Recipe::default()
        };
        for f in fields {
            if let Some(id) = p.pool_word(r, f)?.filter(|&v| v != 0) {
                self.add(&mut state, id, 0)?;
            }
        }
        Ok(state)
    }

    /// 0x801fd8a0 + 0x801fda28: make an item of look record `id` and add it.
    fn add(&self, state: &mut Recipe, id: u32, depth: usize) -> Result<()> {
        if depth > 16 {
            return Err(Error::format(format!("look {id}: recipes nest too deep")));
        }
        let Some((slot, kind, recipe)) = self.looks.get(&id) else {
            return Ok(());
        };
        let mut item = Item {
            id,
            slot: slot.clone(),
            kind: kind.clone(),
            ..Item::default()
        };
        // Same slot and type replaces; so does any hair for hair.
        let hairy = |k: &str| {
            matches!(
                k.to_ascii_lowercase().as_str(),
                "hairnohat" | "hairsmallhat" | "hairlargehat"
            )
        };
        state.items.retain(|old| {
            !((old.slot.eq_ignore_ascii_case(slot) && old.kind.eq_ignore_ascii_case(kind))
                || (hairy(&old.kind) && hairy(kind)))
        });
        self.parse(state, &mut item, recipe, depth)?;
        state.items.push(item);
        Ok(())
    }

    /// 0x801fa8bc: one recipe string into `item`.
    fn parse(&self, state: &mut Recipe, item: &mut Item, recipe: &str, depth: usize) -> Result<()> {
        for part in recipe.split(',') {
            let part = part.trim();
            let (head, options) = part.split_once('?').unwrap_or((part, ""));
            let option = |key: &str| {
                options.split('&').find_map(|kv| {
                    let (k, v) = kv.split_once('=')?;
                    k.eq_ignore_ascii_case(key).then(|| v.to_owned())
                })
            };
            let truthy = |key: &str| {
                option(key).is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "yes")
            };
            let id_of = |s: &str| s.trim().parse::<u32>().ok();
            if let Some(n) = head.strip_prefix("arefmale:").and_then(id_of) {
                if !state.female {
                    self.inline(state, item, n, depth)?;
                }
            } else if let Some(n) = head.strip_prefix("areffemale:").and_then(id_of) {
                if state.female {
                    self.inline(state, item, n, depth)?;
                }
            } else if let Some(n) = head.strip_prefix("aref:").and_then(id_of) {
                self.inline(state, item, n, depth)?;
            } else if let Some(n) = head.strip_prefix("add:").and_then(id_of) {
                self.add(state, n, depth + 1)?;
            } else if let Some(m) = head.strip_prefix("morph:") {
                if let Some((name, w)) = m.rsplit_once('_')
                    && let Ok(w) = w.parse::<f32>()
                {
                    state.morphs.push((name.to_ascii_lowercase(), w / 1000.0));
                }
            } else if let Some(c) = head.strip_prefix("haircolour:").and_then(id_of) {
                state.hair_colour = c;
            } else if let Some(t) = head.strip_prefix("tex:") {
                let name = t.to_ascii_lowercase();
                // The skin tone comes from the skin type's hands.
                if name.starts_with("hand_")
                    && let Some(n) = name
                        .rsplit_once("_skin")
                        .and_then(|(_, n)| n.parse::<u32>().ok())
                {
                    state.skin = n;
                }
                item.textures.push(TextureRef {
                    name,
                    tattoo: truthy("tattoo"),
                });
            } else if let Some(m) = head.strip_prefix("mod:") {
                item.models.push(ModelRef {
                    name: m.to_ascii_lowercase(),
                    skeleton: option("skeletontype").map(|s| s.to_ascii_lowercase()),
                    shorts: truthy("shorts"),
                });
            }
        }
        Ok(())
    }

    fn inline(&self, state: &mut Recipe, item: &mut Item, id: u32, depth: usize) -> Result<()> {
        if depth > 16 {
            return Err(Error::format(format!("look {id}: recipes nest too deep")));
        }
        if let Some((_, _, recipe)) = self.looks.get(&id) {
            self.parse(state, item, recipe, depth + 1)?;
        }
        Ok(())
    }

    fn entry(&self, disc: &mut GcDisc, name: &str) -> Result<Option<Vec<u8>>> {
        match self.entries.get(&name.to_ascii_lowercase()) {
            Some(e) => disc.big_entry(V3_ARCHIVE, e).map(Some),
            None => Ok(None),
        }
    }

    fn assemble(&self, disc: &mut GcDisc, info: &CharacterInfo, look: Look) -> Result<Character> {
        let recipe = self.recipe(info, look)?;
        let models = recipe.models();
        let main_skeleton = models
            .iter()
            .filter(|(_, m)| !m.shorts)
            .find_map(|(_, m)| m.skeleton.clone())
            .unwrap_or_else(|| "skeleton".into());
        let mut skeleton = self.skeleton(disc, &main_skeleton)?;

        // Finished textures, by four-letter group.
        let mut notes = Vec::new();
        if crate::character::bind_mismatch(&skeleton) > 1e-3 {
            crate::character::rigid_bind_pose(&mut skeleton);
            notes.push(format!(
                "{main_skeleton}: bones made rigid at the bind pose"
            ));
        }
        let mut textures = Vec::new();
        let mut texture_of: HashMap<String, usize> = HashMap::new();
        for (group, layers) in recipe.texture_layers() {
            let mut images = Vec::new();
            for (_, file) in &layers {
                match self.entry(disc, &format!("{file}.gsh"))? {
                    Some(bytes) => {
                        let sheet = gsh::parse(&bytes)?;
                        if let (Some(t), Some(&at)) =
                            (sheet.textures.first(), sheet.positions.first())
                        {
                            images.push((t.clone(), at));
                        }
                    }
                    None => notes.push(format!("no texture {file}.gsh")),
                }
            }
            if let Some(t) = compose(&group, &images) {
                texture_of.insert(group, textures.len());
                textures.push(t);
            }
        }

        let mut skeletons: HashMap<String, Skeleton> = HashMap::new();
        let mut meshes = Vec::new();
        for (slot, m) in models {
            let Some(bytes) = self.entry(disc, &format!("{}.o", m.name))? else {
                notes.push(format!("no model {}.o", m.name));
                continue;
            };
            let model =
                eagl_gc::parse(&bytes).map_err(|e| Error::format(format!("{}: {e}", m.name)))?;
            if model.stats.skipped_batches > 0 {
                notes.push(format!(
                    "no decode for {} of {}'s batches",
                    model.stats.skipped_batches, m.name
                ));
            }
            let own = m.skeleton.clone().unwrap_or_else(|| main_skeleton.clone());
            if !skeletons.contains_key(&own) {
                let s = self.skeleton(disc, &own)?;
                skeletons.insert(own.clone(), s);
            }
            let remap = bone_map(&skeletons[&own], &skeleton);
            for mesh in model.meshes {
                let tex = mesh.texture.clone().unwrap_or_default();
                let joints = mesh
                    .joints
                    .iter()
                    .map(|j| j.map(|b| remap.get(b as usize).copied().unwrap_or(0)))
                    .collect();
                meshes.push(Mesh {
                    name: format!("{slot}/{}/{tex}", m.name),
                    positions: mesh.positions,
                    normals: mesh.normals,
                    uvs: mesh.uvs,
                    joints,
                    weights: mesh.weights,
                    indices: mesh.indices,
                    texture: texture_of.get(&tex).copied(),
                });
            }
        }
        Ok(Character {
            id: info.id.clone(),
            name: info.name.clone(),
            height_in: info.height_in.map(|h| h as f32),
            skeleton,
            meshes,
            textures,
            notes,
        })
    }

    fn skeleton(&self, disc: &mut GcDisc, name: &str) -> Result<Skeleton> {
        let bytes = self
            .entry(disc, &format!("{name}.o"))?
            .ok_or_else(|| Error::NotFound(format!("skeleton {name}")))?;
        eagl_gc::parse_skeleton(&bytes)
    }
}

fn team_name(name: &str, city: Option<&str>, nickname: Option<&str>) -> String {
    match name {
        "KT_NINALL" => return "Nintendo All-Stars".into(),
        "KtTEAM_CREATED_BALLERS" => return "Created Ballers".into(),
        "KtTEAM_GREATEST_DUNKERS" => return "Greatest Dunkers".into(),
        "KtTEAM_STREET_LEGENDS" => return "Street Legends".into(),
        _ => {}
    }
    // The league's own two teams: its stars and its legends.
    if let Some(rest) = name.strip_prefix("KtTEAM_") {
        if rest.len() == 9 && rest.ends_with("_STARS") {
            return "All-Stars".into();
        }
        if rest.len() == 11 && rest.ends_with("_LEGENDS") {
            return "Legends".into();
        }
    }
    if (name.starts_with("Kt") || name.starts_with("KT"))
        && let (Some(c), Some(n)) = (city, nickname)
    {
        return format!("{c} {n}");
    }
    name.replace("\\n", " ")
}

/// For each bone of `from`, the bone of `to` with the same name (or 0).
pub(crate) fn bone_map(from: &Skeleton, to: &Skeleton) -> Vec<u16> {
    from.bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            to.bones
                .iter()
                .position(|t| t.name.eq_ignore_ascii_case(&b.name))
                .or_else(|| (i < to.bones.len() && from.bones.len() == to.bones.len()).then_some(i))
                .unwrap_or(0) as u16
        })
        .collect()
}

/// Draws texture layers bottom to top, each at its position, over a canvas
/// as large as they reach.
pub(crate) fn compose(name: &str, layers: &[(crate::ssh::Texture, [u16; 2])]) -> Option<Texture> {
    let w = layers
        .iter()
        .map(|(t, at)| t.width + u32::from(at[0]))
        .max()?;
    let h = layers
        .iter()
        .map(|(t, at)| t.height + u32::from(at[1]))
        .max()?;
    if layers.len() == 1 && layers[0].1 == [0, 0] {
        return Some(Texture::from_ssh(name, &layers[0].0));
    }
    let mut out = vec![0u8; (w * h * 4) as usize];
    for (t, at) in layers {
        for y in 0..t.height {
            for x in 0..t.width {
                let s = ((y * t.width + x) * 4) as usize;
                let (dx, dy) = (x + u32::from(at[0]), y + u32::from(at[1]));
                let d = ((dy * w + dx) * 4) as usize;
                let a = f32::from(t.rgba[s + 3]) / 255.0;
                let da = f32::from(out[d + 3]) / 255.0;
                for c in 0..3 {
                    let v = f32::from(t.rgba[s + c]) * a + f32::from(out[d + c]) * (1.0 - a);
                    out[d + c] = v.round() as u8;
                }
                out[d + 3] = ((a + da * (1.0 - a)) * 255.0).round() as u8;
            }
        }
    }
    let alpha = out.as_chunks::<4>().0.iter().any(|p| p[3] < 255);
    Some(Texture {
        name: name.to_owned(),
        width: w,
        height: h,
        rgba: out,
        alpha,
    })
}

/// Bones, for callers that want the [`Bone`] type without `eagl`.
pub type GcBone = Bone;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("Shawn Marion"), "shawn_marion");
        assert_eq!(slug(" Mike D "), "mike_d");
        assert_eq!(slug("Abdur-Rahim"), "abdur_rahim");
        assert_eq!(clean_name("Kt_Nene"), "Nene");
        assert_eq!(clean_name("KtCM_BIG_WOMAN_F"), "Big Woman F");
        assert_eq!(clean_name("Marion"), "Marion");
    }

    #[test]
    fn fix_names_follow_skin_and_hair() {
        let r = Recipe {
            skin: 8,
            hair_colour: 3,
            ..Recipe::default()
        };
        let name = |s: &str| r.file_name(&s.split('_').collect::<Vec<_>>());
        assert_eq!(name("head_m_master_00_skinfix"), "head_m_master_00_skin08");
        assert_eq!(
            name("head_m_master_00_skinkidfix"),
            "head_m_master_00_skin08"
        );
        assert_eq!(
            name("head_m_master_10_browthreehairfix"),
            "head_m_master_10_browthree3b"
        );
        assert_eq!(name("tops_m_jerseym_50_sunsh"), "tops_m_jerseym_50_sunsh");
        let kid = Recipe {
            kid: true,
            skin: 2,
            ..Recipe::default()
        };
        assert_eq!(
            kid.file_name(&["head", "m", "master", "00", "skinkidfix"]),
            "head_m_master_00_kid02"
        );
    }

    #[test]
    fn layers_draw_at_their_positions() {
        let base = crate::ssh::Texture {
            name: "a".into(),
            width: 4,
            height: 4,
            rgba: vec![10; 64]
                .chunks(4)
                .flat_map(|_| [10, 10, 10, 255])
                .collect(),
            indices: None,
        };
        let dot = crate::ssh::Texture {
            name: "b".into(),
            width: 1,
            height: 1,
            rgba: vec![200, 0, 0, 255],
            indices: None,
        };
        let t = compose("x", &[(base, [0, 0]), (dot, [2, 1])]).unwrap();
        assert_eq!((t.width, t.height), (4, 4));
        let at = (4 + 2) * 4;
        assert_eq!(&t.rgba[at..at + 4], &[200, 0, 0, 255]);
        assert_eq!(&t.rgba[0..4], &[10, 10, 10, 255]);
        assert!(!t.alpha);
    }
}
