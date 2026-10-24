//! Courts: each one is a BIG archive at `/XF/<ID>.VIV` holding EAGL models
//! (`*.o`) and the texture sheets (`*.ssh`) they draw from. The crowd for a
//! court lives in a separate archive (`/XF/<ID>C.VIV`) and is not loaded here.
//!
//! Model files in a court archive, by the letter after the court's prefix
//! (`rucker`): `o` the surroundings, `a` more scenery, `f` the floor, `l` the
//! floor lines, `h` light flares, `b` and `d` the two hoops, `bs` and `ds` their
//! skeletons, and `c` and `c2` the hoops' projected shadows.

use std::{
    collections::BTreeMap,
    io::{Read, Seek},
};

use crate::{Error, Result, big::Big, eagl, iso::Iso, ssh};

/// Archive with the textures and models every court shares.
const SHARED: &str = "/XF/XF000003";
/// Sheets the game loads from [`SHARED`] alongside a court's own.
const SHARED_SHEETS: &[&str] = &["flare.ssh"];

/// Where the game puts a model, relative to the court's origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub translation: [f32; 3],
    /// Unit quaternion (x, y, z, w).
    pub rotation: [f32; 4],
}

/// The two baskets, as the game's scene setup creates them (`SLUS_206.51`,
/// around 0x13c740). Each basket's position is the origin of its hoop model.
/// The hoop constructor (0x316ee0) loads model `d` for a basket at negative x
/// and `b` otherwise; the `b` basket is turned half a turn about y to face the
/// other end. The values are the executable's own floats.
fn hoop_placements(prefix: &str) -> [(String, Placement); 2] {
    let (y, z) = (f32::from_bits(0x4115_e7d5), f32::from_bits(0xbb2a_64c3));
    [
        (
            format!("{prefix}d"),
            Placement {
                translation: [f32::from_bits(0xc224_0c5d), y, z],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
        ),
        (
            format!("{prefix}b"),
            Placement {
                translation: [f32::from_bits(0x4224_17ae), y, z],
                rotation: [0.0, 1.0, 0.0, 0.0],
            },
        ),
    ]
}

/// True for draw groups the game keeps hidden in normal play. Every outdoor
/// court's scenery model has `opaque_riml` and `opaque_rimr`: a spare copy of
/// the hoop's rim lying on the court near each basket, presumably shown after
/// a rim comes off.
pub fn hidden_group(group: &str) -> bool {
    group.ends_with("_riml") || group.ends_with("_rimr")
}

pub struct Court {
    pub id: String,
    pub models: Vec<eagl::Model>,
    /// Placement of models that do not sit at the court's origin, by model name.
    pub placements: BTreeMap<String, Placement>,
    /// Models not decoded: the hoops' projected shadows, which a separate
    /// microprogram (`ArbitraryShadow_Skin`) draws.
    pub skipped_models: Vec<String>,
    /// Every decoded image in the court's sheets, by name.
    pub textures: BTreeMap<String, ssh::Texture>,
    /// Images in formats the decoder skips: (sheet image name, pixel block type).
    pub skipped_textures: Vec<(String, u8)>,
}

impl Court {
    /// Texture names that models use but no sheet in the archive provides.
    pub fn missing_textures(&self) -> Vec<&str> {
        let mut missing: Vec<&str> = self
            .models
            .iter()
            .flat_map(|m| m.meshes.iter().filter_map(|mesh| mesh.texture.as_deref()))
            .filter(|t| !self.textures.contains_key(*t))
            .collect();
        missing.sort_unstable();
        missing.dedup();
        missing
    }
}

/// IDs of the archives in `/XF` that contain at least one model.
pub fn court_ids<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Vec<String>> {
    let vivs: Vec<String> = iso
        .files()
        .filter_map(|(p, _)| {
            p.strip_prefix("/XF/")?
                .strip_suffix(".VIV")
                .map(str::to_owned)
        })
        .collect();
    let mut ids = Vec::new();
    for id in vivs {
        let bytes = iso.read(&format!("/XF/{id}.VIV"))?;
        if Big::parse(&bytes)?
            .entries()
            .iter()
            .any(|e| e.name.to_ascii_lowercase().ends_with(".o"))
        {
            ids.push(id);
        }
    }
    Ok(ids)
}

pub fn load<R: Read + Seek>(iso: &mut Iso<R>, id: &str) -> Result<Court> {
    let id = id.to_ascii_uppercase();
    let path = format!("/XF/{id}.VIV");
    let bytes = iso.read(&path).map_err(|e| match e {
        Error::NotFound(_) => Error::NotFound(format!("court {id} ({path})")),
        e => e,
    })?;
    let big = Big::parse(&bytes)?;

    let mut court = Court {
        placements: hoop_placements(&id.to_ascii_lowercase())
            .into_iter()
            .collect(),
        id,
        models: Vec::new(),
        skipped_models: Vec::new(),
        textures: BTreeMap::new(),
        skipped_textures: Vec::new(),
    };
    for entry in big.entries() {
        let name = entry.name.to_ascii_lowercase();
        let data = big.bytes(entry);
        if name.ends_with(".ssh") {
            add_sheet(&mut court, &entry.name, data, true)?;
        } else if name.ends_with(".o") {
            let model =
                eagl::parse(data).map_err(|e| Error::format(format!("{}: {e}", entry.name)))?;
            if model.name.is_empty() {
                continue; // a skeleton (`__Skeleton`, `__Bone` symbols), not a model
            }
            if model
                .microcode
                .iter()
                .all(|m| m.starts_with("ArbitraryShadow"))
            {
                court.skipped_models.push(model.name);
                continue;
            }
            court.models.push(model);
        }
    }

    let shared = iso.read(SHARED)?;
    let shared = Big::parse(&shared)?;
    for sheet in SHARED_SHEETS {
        add_sheet(&mut court, sheet, shared.get(sheet)?, false)?;
    }
    court
        .placements
        .retain(|name, _| court.models.iter().any(|m| &m.name == name));
    Ok(court)
}

/// A ball from the shared archive: `bal<letter>.o` and its `bal<letter>.ssh`
/// (letters a to m; the executable builds the name as `bal%c.o`).
pub fn ball<R: Read + Seek>(
    iso: &mut Iso<R>,
    letter: char,
) -> Result<(eagl::Model, BTreeMap<String, ssh::Texture>)> {
    let shared = iso.read(SHARED)?;
    let shared = Big::parse(&shared)?;
    let name = format!("bal{letter}");
    let model = eagl::parse(shared.get(&format!("{name}.o"))?)
        .map_err(|e| Error::format(format!("{name}.o: {e}")))?;
    let sheet = ssh::parse(shared.get(&format!("{name}.ssh"))?)
        .map_err(|e| Error::format(format!("{name}.ssh: {e}")))?;
    let textures = sheet
        .textures
        .into_iter()
        .map(|t| (t.name.clone(), t))
        .collect();
    Ok((model, textures))
}

/// Add a sheet's images. An image whose name is already loaded is replaced
/// only when `replace` is set.
fn add_sheet(court: &mut Court, name: &str, data: &[u8], replace: bool) -> Result<()> {
    let sheet = ssh::parse(data).map_err(|e| Error::format(format!("{name}: {e}")))?;
    court.skipped_textures.extend(sheet.skipped);
    for t in sheet.textures {
        if replace || !court.textures.contains_key(&t.name) {
            court.textures.insert(t.name.clone(), t);
        }
    }
    Ok(())
}
