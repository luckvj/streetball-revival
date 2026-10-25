//! Players: models and textures in two archives whose entries have no names,
//! addressed by index (see [`crate::db::Player::model`] for which entries a
//! player uses).
//!
//! `/XF/XF000002` holds everyone in the database except created players:
//! - league players: five entries each (three morph targets, a road and a home
//!   sheet) for a shared `Small`/`Medium`/`Large` body by build. The body
//!   draws `body`, `HEAD` and `MOUTH`, one hand pose per hand, and optionally
//!   `HEADBAND` and `zdreds`.
//! - streetballers, legends and generic ballers: a model of their own (one
//!   group named after them plus the hand poses) and its sheets;
//! - the referee, the skeletons and accessories (pads, braces, bands, team
//!   shorts and T-shirts).
//!
//! `/XF/XF000001` holds the create-a-player kit ([`Parts`]):
//! - body models `body<b>_outfit<o>`, six builds by six outfits. Outfit 6 is
//!   the team uniform (it has a `jers` texture slot); builds 4 and 5 have no
//!   beard or glasses. Each body is followed by its morph targets.
//! - head and hair models `malehair<h>_head<n>_body<b>` and
//!   `femalehair<h>_head<n>_body<b>`, each made to fit one build;
//! - single-image texture sheets (team jerseys, shirts, hair).
//!
//! Kit bodies draw `Head2`, `MadCoach<o>`, `shoes`, the hand poses and the
//! optional `zzBeard` and `zzGlasses`; see [`default_groups`].

use std::{
    collections::BTreeMap,
    io::{Read, Seek},
};

use crate::{
    Error, Result,
    big::{self, BigEntry},
    db::{self, PlayerModel},
    eagl,
    iso::Iso,
    morph, ssh,
};

const PARTS: &str = "/XF/XF000001";
const PLAYERS: &str = "/XF/XF000002";
/// Entries per player in [`PLAYERS`]: three morph targets, then two sheets.
const PLAYER_STRIDE: usize = 5;
/// Entry of the 43-bone body skeleton in [`PLAYERS`]. Entries 942-944 are
/// small eight-bone rigs for the shorts.
const SKELETON_ENTRY: usize = 941;
const ELF_MAGIC: &[u8] = b"\x7fELF";

/// The shared player parts, indexed by model name.
pub struct Parts {
    entries: Vec<BigEntry>,
    /// `__Model` name -> entry index.
    models: BTreeMap<String, usize>,
}

impl Parts {
    /// Read the parts archive's index and the name of every model in it.
    pub fn load<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self> {
        let entries = big::read_index(iso, PARTS)?;
        let mut models = BTreeMap::new();
        for (i, e) in entries.iter().enumerate() {
            // The model name is in the ELF string table; a short read of the
            // whole entry is the simplest way to reach it.
            let bytes = big::read_entry(iso, PARTS, e)?;
            if bytes.starts_with(ELF_MAGIC)
                && let Some(name) = model_name(&bytes)
            {
                models.insert(name, i);
            }
        }
        Ok(Self { entries, models })
    }

    pub fn model_names(&self) -> impl Iterator<Item = &str> {
        self.models.keys().map(String::as_str)
    }

    pub fn model<R: Read + Seek>(&self, iso: &mut Iso<R>, name: &str) -> Result<eagl::Model> {
        let i = *self
            .models
            .get(name)
            .ok_or_else(|| Error::NotFound(format!("player model {name}")))?;
        let bytes = big::read_entry(iso, PARTS, &self.entries[i])?;
        eagl::parse(&bytes).map_err(|e| Error::format(format!("{name}: {e}")))
    }
}

/// The `__Model:::<name>` symbol's name, found by searching the raw object.
fn model_name(bytes: &[u8]) -> Option<String> {
    const TAG: &[u8] = b"__Model:::";
    let at = bytes.windows(TAG.len()).position(|w| w == TAG)? + TAG.len();
    let len = bytes[at..].iter().position(|&b| b == 0)?;
    Some(bytes[at..at + len].iter().map(|&b| b as char).collect())
}

/// Number of players stored as three morph targets and two sheets at the
/// start of [`PLAYERS`] (145 on the USA disc).
pub fn player_count<R: Read + Seek>(iso: &mut Iso<R>) -> Result<usize> {
    let entries = big::read_index(iso, PLAYERS)?;
    let expected: [&[u8]; PLAYER_STRIDE] = [ELF_MAGIC, ELF_MAGIC, ELF_MAGIC, b"SHPS", b"SHPS"];
    let mut count = 0;
    for player in entries.as_chunks::<PLAYER_STRIDE>().0 {
        for (e, magic) in player.iter().zip(expected) {
            if iso.read_range(PLAYERS, e.offset as u64, magic.len().min(e.size as usize))? != magic
            {
                return Ok(count);
            }
        }
        count += 1;
    }
    Ok(count)
}

/// The body skeleton every player model is skinned to.
pub fn skeleton<R: Read + Seek>(iso: &mut Iso<R>) -> Result<eagl::Skeleton> {
    let entries = big::read_index(iso, PLAYERS)?;
    let entry = entries
        .get(SKELETON_ENTRY)
        .ok_or_else(|| Error::NotFound("player skeleton".into()))?;
    eagl::parse_skeleton(&big::read_entry(iso, PLAYERS, entry)?)
}

/// A player's textures from the first of their two sheets, by name. `player`
/// counts from 0 up to [`player_count`].
pub fn player_textures<R: Read + Seek>(
    iso: &mut Iso<R>,
    player: usize,
) -> Result<BTreeMap<String, ssh::Texture>> {
    if player >= player_count(iso)? {
        return Err(Error::NotFound(format!("player {player}")));
    }
    let entries = big::read_index(iso, PLAYERS)?;
    let sheet = ssh::parse(&big::read_entry(
        iso,
        PLAYERS,
        &entries[player * PLAYER_STRIDE + 3],
    )?)?;
    Ok(sheet
        .textures
        .into_iter()
        .map(|t| (t.name.clone(), t))
        .collect())
}

/// Texture slots the game fills with the player's jersey image rather than by
/// name (`SLUS_206.51` around 0x31f6ac: `jers`, `shir` and `tank` all get it).
pub const JERSEY_SLOTS: &[&str] = &["jers", "shir", "tank"];

/// The groups a body model draws by default: relaxed hands, and none of the
/// optional parts (beard, glasses, headband, dreadlocks).
pub fn default_groups(group: &str) -> bool {
    let optional = group.starts_with("AL")
        || group.starts_with("AR")
        || group.starts_with("zz")
        || group == "HEADBAND"
        || group == "zdreds";
    !optional || group == "ALrelax" || group == "ARrelax"
}

/// The referee's model and sheet in [`PLAYERS`]: `FUN_003168b8` (his
/// render object, built by the `AIPhysReferee` constructor `FUN_0017c600`)
/// loads entries `0x3a1` (`reff`) and `0x3a2`, the shadow `0x3ac` and the
/// body skeleton `0x3ad` ([`skeleton`]), with no morph target.
const REFEREE_MODEL: usize = 0x3a1;
const REFEREE_SHEET: usize = 0x3a2;

/// The referee who opens a match with the toss, drawn at full size.
pub fn referee<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Look> {
    let entries = big::read_index(iso, PLAYERS)?;
    let entry = |i: usize| {
        entries
            .get(i)
            .ok_or_else(|| Error::NotFound(format!("{PLAYERS} entry {i}")))
    };
    let model = eagl::parse(&big::read_entry(iso, PLAYERS, entry(REFEREE_MODEL)?)?)
        .map_err(|e| Error::format(format!("referee model: {e}")))?;
    let sheet = ssh::parse(&big::read_entry(iso, PLAYERS, entry(REFEREE_SHEET)?)?)?;
    Ok(Look {
        models: vec![model],
        textures: sheet
            .textures
            .into_iter()
            .map(|t| (t.name.clone(), t))
            .collect(),
        extra_groups: Vec::new(),
        scale: 1.0,
        native_body_scale: None,
        native_head_scale: None,
    })
}

/// Which of a team's two uniforms a player wears.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    /// The coloured jersey.
    #[default]
    Road,
    /// The white jersey.
    Home,
}

/// A database player ready to draw: rest-pose models with the player's morph
/// target applied, the image for each texture slot, and the optional draw
/// groups the player shows.
pub struct Look {
    pub models: Vec<eagl::Model>,
    /// Images by texture slot name.
    pub textures: BTreeMap<String, ssh::Texture>,
    /// Groups shown on top of [`default_groups`] (`HEADBAND`, `zdreds`).
    pub extra_groups: Vec<String>,
    /// Uniform scale the game gives the model ([`db::Player::height_scale`]).
    pub scale: f32,
    /// Recovered native skeleton body scale, before uniform size options.
    pub native_body_scale: Option<[f32; 3]>,
    /// Native per-axis head scale, before head-size options.
    pub native_head_scale: Option<[f32; 3]>,
}

impl Look {
    pub fn shows(&self, group: &str) -> bool {
        default_groups(group) || self.extra_groups.iter().any(|g| g == group)
    }
}

/// Assemble `player` the way the game's model loader (0x315790) does, in the
/// given uniform, with its accessories (headband, pads, braces, bands).
/// Created players (team 37) wear their own clothes; see [`created`].
pub fn look<R: Read + Seek>(iso: &mut Iso<R>, player: &db::Player, side: Side) -> Result<Look> {
    look_for_team(iso, player, side, None)
}

/// [`look`] for a player on league team `team` (0..28): a created player
/// there wears the team's uniform in `side`'s colours, clothing 80 (the
/// loader forces it, `0x31594c`).
pub fn look_for_team<R: Read + Seek>(
    iso: &mut Iso<R>,
    player: &db::Player,
    side: Side,
    team: Option<usize>,
) -> Result<Look> {
    look_dressed(iso, player, side, team, 0)
}

/// [`look_for_team`] with the loader's draw for the shorts (`rand()`,
/// `0x315cc8`): a league player wears image `shorts % count` of his
/// team's sheet.
pub fn look_dressed<R: Read + Seek>(
    iso: &mut Iso<R>,
    player: &db::Player,
    side: Side,
    team: Option<usize>,
    shorts: u32,
) -> Result<Look> {
    // The player editor: a roster.json player's chosen model. (GameCube
    // models, `roster_edits::gamecube_model`, draw this fallback for now.)
    let swapped = crate::roster_edits::look_for(player);
    let player = swapped.as_ref().unwrap_or(player);
    let entries = big::read_index(iso, PLAYERS)?;
    let mut entry = |i: usize| -> Result<Vec<u8>> {
        let e = entries
            .get(i)
            .ok_or_else(|| Error::NotFound(format!("{PLAYERS} entry {i}")))?;
        big::read_entry(iso, PLAYERS, e)
    };
    let model = |bytes: &[u8], i: usize| {
        eagl::parse(bytes).map_err(|e| Error::format(format!("{PLAYERS} entry {i}: {e}")))
    };
    let sheet = |bytes: &[u8], i: usize| {
        ssh::parse(bytes).map_err(|e| Error::format(format!("{PLAYERS} entry {i}: {e}")))
    };
    let name = player.name();
    let mut look = Look {
        models: Vec::new(),
        textures: BTreeMap::new(),
        extra_groups: Vec::new(),
        scale: player.height_scale(),
        native_body_scale: player.native_body_scale(),
        native_head_scale: player.native_head_scale(),
    };
    let (own, road, home) = match player.model() {
        Some(PlayerModel::League(m)) => {
            let mut body = model(&entry(m.bodies[0])?, m.bodies[0])?;
            let target = morph::parse(&entry(m.morphs[0])?)
                .map_err(|e| Error::format(format!("morph target of {name}: {e}")))?;
            target.apply(&mut body, 1.0)?;
            look.models.push(body);
            // The game picks one of the team's six pairs of shorts at
            // random: `rand() % count` over the sheet (`0x315cb8..0x315cf0`).
            let images = sheet(&entry(m.shorts_sheet)?, m.shorts_sheet)?.textures;
            let pick = (shorts as usize).checked_rem(images.len()).unwrap_or(0);
            if let Some(t) = images.into_iter().nth(pick) {
                look.textures.insert("shor".into(), t);
            }
            for part in m.accessories.iter().filter(|p| p.model.is_none()) {
                // The headband: the body's own group, the image bound to `hdbd`.
                let images = sheet(&entry(part.sheet)?, part.sheet)?.textures;
                let index = if part.image < images.len() {
                    part.image
                } else {
                    0
                };
                if let Some(t) = images.into_iter().nth(index) {
                    look.textures.insert("hdbd".into(), t);
                }
                look.extra_groups.push("HEADBAND".into());
            }
            // Pads, braces and bands (0x316260): a model of their own, sized
            // for the build and skinned to the body's bones, its texture slot
            // bound to the chosen image of the slot's sheet.
            for part in &m.accessories {
                let Some(i) = part.model else { continue };
                let piece = model(&entry(i)?, i)?;
                let images = sheet(&entry(part.sheet)?, part.sheet)?.textures;
                let index = if part.image < images.len() {
                    part.image
                } else {
                    0
                };
                if let Some(t) = images.into_iter().nth(index) {
                    for slot in piece.meshes.iter().filter_map(|m| m.texture.as_ref()) {
                        look.textures.insert(slot.clone(), t.clone());
                    }
                }
                look.models.push(piece);
            }
            if m.dreads {
                look.extra_groups.push("zdreds".into());
            }
            (None, m.road_sheet, m.home_sheet)
        }
        Some(PlayerModel::Unique(m)) => (Some(m.model), m.road_sheet, m.home_sheet),
        Some(PlayerModel::Created(m)) => {
            let uniform = team
                .filter(|&t| t < 29)
                .and_then(|t| player.created_model(Some((t, side == Side::Home))));
            return created(iso, &name, &uniform.unwrap_or(m), look);
        }
        None => return Err(Error::format(format!("{name} has no model"))),
    };
    if let Some(i) = own {
        look.models.push(model(&entry(i)?, i)?);
    }
    let i = match side {
        Side::Road => road,
        Side::Home => home,
    };
    for t in sheet(&entry(i)?, i)?.textures {
        look.textures.insert(t.name.clone(), t);
    }
    Ok(look)
}

/// A created player from the kit in `/XF/XF000001` (the appearance object
/// 0x31e938): the outfit body with the face, beard and shoe morph targets
/// applied, the hair-and-head model, and the sheets bound by image name.
/// The skin sheet names its images after the slots (`head`, `arms` or
/// `larm`/`rarm`, `hand`, `legs`, `eyes`, `mout`, `bely`, `glas`), as do the
/// top (`shir`) and bottom sheets (`shor`, `sock`, `slev`, `unds`, `armb`,
/// `pant`); the hair, beard and shoe sheets hold one image each, for the
/// head's `hair` slot and the body's `berd` and `shoe` slots. The body's
/// optional groups show with their parts: `zzBeard` with facial hair,
/// `zzGlasses` when the skin sheet carries glasses.
fn created<R: Read + Seek>(
    iso: &mut Iso<R>,
    name: &str,
    m: &db::CreatedModel,
    mut look: Look,
) -> Result<Look> {
    let entries = big::read_index(iso, PARTS)?;
    let mut entry = |i: usize| -> Result<Vec<u8>> {
        let e = entries
            .get(i)
            .ok_or_else(|| Error::NotFound(format!("{PARTS} entry {i}")))?;
        big::read_entry(iso, PARTS, e)
    };
    let model = |bytes: &[u8], i: usize| {
        eagl::parse(bytes).map_err(|e| Error::format(format!("{PARTS} entry {i}: {e}")))
    };
    let sheet = |bytes: &[u8], i: usize| {
        ssh::parse(bytes).map_err(|e| Error::format(format!("{PARTS} entry {i}: {e}")))
    };
    let mut body = model(&entry(m.body)?, m.body)?;
    for target in [Some(m.face_morph), m.beard_morph, Some(m.shoe_morph)]
        .into_iter()
        .flatten()
    {
        morph::parse(&entry(target)?)
            .map_err(|e| Error::format(format!("morph target {target} of {name}: {e}")))?
            .apply(&mut body, 1.0)?;
    }
    look.models.push(body);
    if let Some(head) = m.head {
        look.models.push(model(&entry(head)?, head)?);
    }
    // Sheets whose images are named after their slots.
    for i in [Some(m.skin_sheet), m.top_sheet, Some(m.bottom_sheet)]
        .into_iter()
        .flatten()
    {
        for t in sheet(&entry(i)?, i)?.textures {
            if t.name == "glas" {
                look.extra_groups.push("zzGlasses".into());
            }
            look.textures.insert(t.name.clone(), t);
        }
    }
    // The top also dresses the other jersey slots some bodies have.
    if let Some(shirt) = look.textures.get("shir").cloned() {
        for slot in JERSEY_SLOTS {
            look.textures
                .entry((*slot).into())
                .or_insert_with(|| shirt.clone());
        }
    }
    // Single-image sheets, bound to one slot each.
    for (i, slot) in [
        (m.hair_sheet, "hair"),
        (m.beard_sheet, "berd"),
        (Some(m.shoe_sheet), "shoe"),
    ] {
        let Some(i) = i else { continue };
        if let Some(t) = sheet(&entry(i)?, i)?.textures.into_iter().next() {
            look.textures.insert(slot.into(), t);
        }
    }
    if m.beard_sheet.is_some() {
        look.extra_groups.push("zzBeard".into());
    }
    Ok(look)
}
