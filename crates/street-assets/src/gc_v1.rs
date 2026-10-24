//! the original game (`GNSE69`, GameCube) characters, assembled as the game's model
//! loader (0x80190784) and texture loader (0x80191364) do it.
//!
//! The roster is `/database/xdbplyr.adf` (175 records of 69 fields, the
//! big-endian ADF of [`crate::db`]); the fields used here: 0 id, 1 last
//! name, 2 first name, 3 team (row of `xdbteam.adf`; 29 street legends, 30
//! the street challenge crews), 5 position, 6 jersey, 15 height, 16 weight,
//! 22..32 the eleven ratings, 34 build (0 small, 1 medium, 2 large, 3
//! female), 35 model key, 36 skin tone (0..5), 37 shoe style, 38 body
//! texture variant, 39 own body texture, 40/41 crew body/head kit numbers.
//!
//! Models and sheets are entries of `/xbigf002.viv`..`/xbigf006.viv`
//! (unpacked BIG archives), found by name:
//!
//! - league players: `head<key>.o` is the whole player (the `small`, `medium`
//!   or `large` body morphed to the player, which is how the game draws it;
//!   that body file holds the skeleton). Sheets: `head<key>.gsh`, the body
//!   sheet (`body<tone><variant>.gsh` from the table at 0x80285628, or
//!   `body<key>.gsh`), the jersey `jers<team>h.gsh` (`a` away), the shoes
//!   `<style + 1>.gsh` (`%04d`), and `mout.gsh`.
//! - Street legends: `<key>.o` with its skeleton, sheet `<key>ho.gsh`
//!   (`aw` away).
//! - Crews: `bodycp<nn>.o`, `headcp<nn><s|m|l|w>.o` and shoes
//!   `<s|m|l><style>.o` from the kit table at 0x8025caa8 (32 bytes per kit:
//!   name, build, body sheet, tattoo), with sheets `headcp<nn>.gsh`,
//!   `clthcp<nn>.gsh`, `body<sheet>.gsh` and `<style>.gsh`.
//! - Hands: `<build>_lhand_relax.o` and `_rhand_relax.o` (`small`,
//!   `medium`, `large`, `female`; Magma, the alien and the yeti have their
//!   own).
//!
//! A texture slot binds the sheet image of the same name; `larm`/`rarm`
//! take a body sheet's `arm`, `lleg`/`rleg` its `leg`, and `shoe` the shoe
//! sheet's image. Accessories (headbands, pads, braces) and shadows are left
//! out.

use std::collections::HashMap;

use crate::{
    Error, Result,
    big::BigEntry,
    character::{Character, Mesh, Texture},
    db::Table,
    eagl_gc,
    gc_character::{CharacterInfo, Game, Kind, TeamInfo, assign_keys, bone_map},
    gcdisc::GcDisc,
    gsh, ssh,
};

const PLAYERS: &str = "/database/xdbplyr.adf";
const TEAMS: &str = "/database/xdbteam.adf";
const ARCHIVES: [&str; 5] = [
    "/xbigf002.viv",
    "/xbigf003.viv",
    "/xbigf004.viv",
    "/xbigf005.viv",
    "/xbigf006.viv",
];
/// Body sheets by skin tone (0x80285628).
const BODY_SHEETS: [&str; 6] = [
    "body0lw", "body1mw", "body2dw", "body3lb", "body4mb", "body5db",
];
const BUILDS: [&str; 4] = ["small", "medium", "large", "female"];
/// The crews' kit table in `main.dol` and its length.
const KITS: u32 = 0x8025_caa8;
const KIT_COUNT: usize = 40;

pub(crate) struct V1 {
    players: Table,
    /// Lower-case entry name -> (archive, entry).
    entries: HashMap<String, (&'static str, BigEntry)>,
    /// Team row -> three-letter jersey code (`atl`).
    team_codes: Vec<String>,
    /// Crew kits: (name `cp04`, build 0..3, body sheet `1mw1`).
    kits: Vec<(String, i32, String)>,
}

struct Kit<'a> {
    name: &'a str,
    build: i32,
    sheet: &'a str,
}

impl V1 {
    pub(crate) fn open(disc: &mut GcDisc) -> Result<Self> {
        let players = Table::parse(disc.read(PLAYERS)?)?;
        let teams = Table::parse(disc.read(TEAMS)?)?;
        let mut entries = HashMap::new();
        for archive in ARCHIVES {
            for e in disc.big_index(archive)? {
                entries
                    .entry(e.name.to_ascii_lowercase())
                    .or_insert((archive, e));
            }
        }
        let team_codes = (0..teams.record_count())
            .map(|r| Ok(teams.string(r, 9)?.unwrap_or_default()))
            .collect::<Result<_>>()?;
        let kits = read_kits(&disc.main_dol()?).unwrap_or_default();
        Ok(Self {
            players,
            entries,
            team_codes,
            kits,
        })
    }

    pub(crate) fn roster(&self) -> Result<(Vec<CharacterInfo>, Vec<TeamInfo>)> {
        let p = &self.players;
        let mut characters = Vec::new();
        for r in 0..p.record_count() {
            let field = |f: usize| p.raw(r, f).map(i32::from);
            let team = field(3)?;
            let kind = match team {
                0..=28 => Kind::League,
                29 => Kind::Street,
                30 => Kind::Generic,
                _ => continue,
            };
            let last = title(&p.string(r, 1)?.unwrap_or_default());
            let first = title(&p.string(r, 2)?.unwrap_or_default());
            let name = format!("{first} {last}").trim().to_owned();
            let mut raw = Vec::new();
            let mut strings = std::collections::BTreeMap::new();
            for f in 0..p.field_count() {
                if p.kind(f) == Some(crate::db::FieldKind::String)
                    && let Some(s) = p.string(r, f)?
                {
                    strings.insert(f, s);
                }
                raw.push(i64::from(p.raw(r, f)?));
            }
            let opt = |v: i32| Some(v).filter(|&v| v >= 0);
            characters.push(CharacterInfo {
                id: String::new(),
                game: Game::V1,
                key: String::new(),
                record: r,
                db_id: field(0)?,
                name,
                first_name: first,
                last_name: last,
                nickname: None,
                kind,
                team: Some(team),
                team_name: None,
                jersey: opt(field(6)?),
                position: opt(field(5)?),
                height_in: Some(field(15)?).filter(|&h| h > 0),
                weight_lb: Some(field(16)?).filter(|&w| w > 0),
                female: field(34)? == 3 || (team == 29 && (155..=159).contains(&r)),
                kid: false,
                ratings: (22..33).map(field).collect::<Result<_>>()?,
                raw,
                strings,
            });
        }
        assign_keys(&mut characters);
        Ok((characters, Vec::new()))
    }

    pub(crate) fn teams(
        &self,
        disc: &mut GcDisc,
        characters: &mut [CharacterInfo],
    ) -> Result<Vec<TeamInfo>> {
        let t = Table::parse(disc.read(TEAMS)?)?;
        let mut teams = Vec::new();
        for r in 0..t.record_count() {
            let city = t.string(r, 1)?.map(|s| title(&s));
            let nickname = t.string(r, 2)?.map(|s| title(&s));
            let name = match (&city, &nickname) {
                (Some(c), Some(n)) if r < 29 => format!("{c} {n}"),
                _ => title(&t.string(r, 0)?.unwrap_or_default()),
            };
            // Rows 29 and 30 mark the street legends and the crews rather
            // than a team; the street teams' line-ups live elsewhere.
            let on_team = |c: &CharacterInfo| r < 29 && c.team == Some(r as i32);
            let roster: Vec<String> = characters
                .iter()
                .filter(|c| on_team(c))
                .map(|c| c.id.clone())
                .collect();
            for c in characters.iter_mut().filter(|c| on_team(c)) {
                c.team_name = Some(name.clone());
            }
            teams.push(TeamInfo {
                id: r as i32,
                name,
                city,
                nickname,
                roster,
            });
        }
        Ok(teams)
    }

    fn entry(&self, disc: &mut GcDisc, name: &str) -> Result<Option<Vec<u8>>> {
        match self.entries.get(&name.to_ascii_lowercase()) {
            Some((archive, e)) => disc.big_entry(archive, e).map(Some),
            None => Ok(None),
        }
    }

    fn kit(&self, n: i32) -> Option<Kit<'_>> {
        let (name, build, sheet) = self.kits.get(usize::try_from(n).ok()?)?;
        Some(Kit {
            name,
            build: *build,
            sheet,
        })
    }

    pub(crate) fn assemble(
        &self,
        disc: &mut GcDisc,
        info: &CharacterInfo,
        look: crate::gc_character::Look,
    ) -> Result<Character> {
        let r = info.record;
        let p = &self.players;
        let field = |f: usize| p.raw(r, f).map(i32::from);
        let key = p.string(r, 35)?.unwrap_or_default().to_ascii_lowercase();
        let team = field(3)?;
        let build = field(34)?.clamp(0, 3) as usize;
        let shoe = field(37)?;
        let mut notes = Vec::new();
        let away = look == crate::gc_character::Look::Away;
        // Models, sheets in binding order, the file holding the skeleton and
        // the hands' build.
        let mut models: Vec<String> = Vec::new();
        let mut sheets: Vec<(String, Option<&str>)> = Vec::new();
        let (skeleton_from, hands): (String, String) = match team {
            29 => {
                models.push(format!("{key}.o"));
                let side = if away { "aw" } else { "ho" };
                sheets.push((format!("{key}{side}.gsh"), None));
                let hands = match r {
                    151..=153 => key.clone(),
                    155..=159 => "female".into(),
                    _ => "small".into(),
                };
                (format!("{key}.o"), hands)
            }
            30 => {
                let body = self
                    .kit(field(40)?)
                    .ok_or_else(|| Error::format("crew body kit not in main.dol's table"))?;
                let head = self
                    .kit(field(41)?)
                    .ok_or_else(|| Error::format("crew head kit not in main.dol's table"))?;
                let size = ["s", "m", "l", "w"][body.build.clamp(0, 3) as usize];
                let shoe_size = ["s", "m", "l", "s"][body.build.clamp(0, 3) as usize];
                models.push(format!("body{}.o", body.name));
                models.push(format!("head{}{size}.o", head.name));
                models.push(format!("{shoe_size}{shoe:04}.o"));
                sheets.push((format!("head{}.gsh", head.name), None));
                sheets.push((format!("clth{}.gsh", body.name), None));
                sheets.push((format!("body{}.gsh", head.sheet), None));
                sheets.push((format!("{shoe:04}.gsh"), Some("shoe")));
                (
                    format!("body{}.o", body.name),
                    BUILDS[body.build.clamp(0, 3) as usize].into(),
                )
            }
            _ => {
                let base = BUILDS[build.min(2)];
                models.push(format!("head{key}.o"));
                sheets.push((format!("head{key}.gsh"), None));
                if field(39)? == 0 {
                    let tone = BODY_SHEETS[field(36)?.clamp(0, 5) as usize];
                    let variant = field(38)?.clamp(0, 1);
                    sheets.push((format!("{tone}{variant}.gsh"), None));
                } else {
                    sheets.push((format!("body{key}.gsh"), None));
                }
                let code = self
                    .team_codes
                    .get(team as usize)
                    .cloned()
                    .unwrap_or_default();
                let side = if away { 'a' } else { 'h' };
                sheets.push((format!("jers{code}{side}.gsh"), None));
                sheets.push((format!("{:04}.gsh", shoe + 1), Some("shoe")));
                (format!("{base}.o"), BUILDS[build].into())
            }
        };
        // The mouth sheet is loaded once for everyone.
        sheets.push(("mout.gsh".into(), None));
        models.push(format!("{hands}_lhand_relax.o"));
        models.push(format!("{hands}_rhand_relax.o"));

        // Sheet images by name; the shoe sheet's first image is `shoe`.
        let mut images: HashMap<String, ssh::Texture> = HashMap::new();
        for (sheet, alias) in &sheets {
            let Some(bytes) = self.entry(disc, sheet)? else {
                notes.push(format!("no texture {sheet}"));
                continue;
            };
            let parsed = gsh::parse(&bytes)?;
            for (i, t) in parsed.textures.into_iter().enumerate() {
                if let Some(a) = alias
                    && i == 0
                {
                    images.entry((*a).to_owned()).or_insert_with(|| t.clone());
                }
                images
                    .entry(t.name.trim().to_ascii_lowercase())
                    .or_insert(t);
            }
        }

        let skeleton_bytes = self
            .entry(disc, &skeleton_from)?
            .ok_or_else(|| Error::NotFound(skeleton_from.clone()))?;
        let skeleton = eagl_gc::parse_skeleton(&skeleton_bytes)?;
        let mut textures: Vec<Texture> = Vec::new();
        let mut texture_of: HashMap<String, usize> = HashMap::new();
        let mut meshes = Vec::new();
        for name in &models {
            let Some(bytes) = self.entry(disc, name)? else {
                notes.push(format!("no model {name}"));
                continue;
            };
            let model =
                eagl_gc::parse(&bytes).map_err(|e| Error::format(format!("{name}: {e}")))?;
            if model.stats.skipped_batches > 0 {
                notes.push(format!(
                    "no decode for {} of {name}'s batches",
                    model.stats.skipped_batches
                ));
            }
            // Parts with a skeleton of their own (crew heads, hands) use it.
            let own = eagl_gc::parse_skeleton(&bytes).ok();
            let remap = own.as_ref().map(|s| bone_map(s, &skeleton));
            for mesh in model.meshes {
                let slot = mesh.texture.clone().unwrap_or_default();
                let texture = match texture_of.get(&slot) {
                    Some(&i) => Some(i),
                    None => {
                        let image = images.get(&slot).or_else(|| {
                            let generic = match slot.as_str() {
                                "larm" | "rarm" => "arm",
                                "lleg" | "rleg" => "leg",
                                "tors" => "body",
                                other => other,
                            };
                            images.get(generic)
                        });
                        image.map(|t| {
                            texture_of.insert(slot.clone(), textures.len());
                            textures.push(Texture::from_ssh(&slot, t));
                            textures.len() - 1
                        })
                    }
                };
                if texture.is_none() && !slot.is_empty() {
                    notes.push(format!("no image for {name}/{slot}"));
                }
                let joints = match &remap {
                    Some(m) => mesh
                        .joints
                        .iter()
                        .map(|j| j.map(|b| m.get(b as usize).copied().unwrap_or(0)))
                        .collect(),
                    None => mesh.joints,
                };
                meshes.push(Mesh {
                    name: format!("{}/{slot}", name.trim_end_matches(".o")),
                    positions: mesh.positions,
                    normals: mesh.normals,
                    uvs: mesh.uvs,
                    joints,
                    weights: mesh.weights,
                    indices: mesh.indices,
                    texture,
                });
            }
        }
        notes.dedup();
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
}

/// `main.dol`'s 256 delta steps for compressed animations (0x8025b668).
const DELTA_TABLE: u32 = 0x8025_b668;
/// Per body bone (12 bytes): unused, the mirror partner, the mirror kind
/// (0x8025bb6c, read by 0x800ccee8).
const MIRROR_TABLE: u32 = 0x8025_bb6c;
/// The names of `anibot.ani`'s animations, one pointer each (0x80252980).
const ANIBOT_NAMES: u32 = 0x8025_2980;

/// One of the original game's animation files (`/anim/anibot.ani` in game,
/// `anifront.ani` front end, `aniface.ani` faces), decoded as 0x800cd... does:
///
/// - a table of 16-byte entries: frames, channels, data offset, flags;
/// - at the offset, one `(x, y, z, w)` root translation per frame, then the
///   rotations: with flag 2 clear, `frames × channels` raw quaternions
///   (frame by frame); with flag 2 set, the first frame's quaternions and
///   then per later frame `channels × 4` bytes, each byte indexing a
///   256-step table in `main.dol` whose value is added to the running
///   component;
/// - flag 1: the animation plays mirrored (bone pairs swapped and
///   components flipped by `main.dol`'s mirror table, the root's x negated);
/// - flag 4 (`aniface.ani`, 21 face channels): another layout, not decoded.
///
/// Channel `i` drives skeleton bone `i` (Vol. 2's order, which the original game's
/// 43-bone skeleton shares). Animations can share data (a mirrored copy).
pub struct V1Animations {
    names: Vec<String>,
    entries: Vec<[u32; 4]>,
    data: Vec<u8>,
    deltas: Vec<f32>,
    mirror: Vec<(usize, u32)>,
}

impl V1Animations {
    /// `file` is `anibot.ani`, `anifront.ani` or `aniface.ani`.
    pub fn load(disc: &mut GcDisc, file: &str) -> Result<Self> {
        let data = disc.read(&format!("/anim/{file}"))?;
        let dol = disc.main_dol()?;
        let be = |b: &[u8], at: usize| crate::bytes::u32_be(b, at);
        let first = be(&data, 8)? as usize;
        let count = first / 16;
        if count == 0 || count > 4096 {
            return Err(Error::format(format!("{file}: {count} animations")));
        }
        let entries = (0..count)
            .map(|i| {
                Ok([
                    be(&data, 16 * i)?,
                    be(&data, 16 * i + 4)?,
                    be(&data, 16 * i + 8)?,
                    be(&data, 16 * i + 12)?,
                ])
            })
            .collect::<Result<Vec<_>>>()?;
        let at = |va: u32| dol_offset(&dol, va).ok_or_else(|| Error::format("main.dol table"));
        let table = at(DELTA_TABLE)?;
        let deltas = (0..256)
            .map(|k| crate::bytes::f32_be(&dol, table + 4 * k))
            .collect::<Result<Vec<_>>>()?;
        let mt = at(MIRROR_TABLE)?;
        let mirror = (0..22)
            .map(|k| {
                Ok((
                    be(&dol, mt + 12 * k + 4)? as usize,
                    be(&dol, mt + 12 * k + 8)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let names = if file.eq_ignore_ascii_case("anibot.ani") {
            let base = at(ANIBOT_NAMES)?;
            (0..count)
                .map(|k| {
                    let ptr = be(&dol, base + 4 * k)?;
                    let o = dol_offset(&dol, ptr).ok_or_else(|| Error::format("name pointer"))?;
                    crate::bytes::cstr(&dol, o)
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            (0..count).map(|k| format!("{k}")).collect()
        };
        Ok(Self {
            names,
            entries,
            data,
            deltas,
            mirror,
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Animation names (`anibot.ani`'s from `main.dol`; others numbered).
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n.eq_ignore_ascii_case(name))
    }

    /// One animation's rotations: channel `c` drives bone `c`.
    pub fn rotations(&self, index: usize) -> Result<crate::anim::Rotations> {
        let [frames, channels, offset, flags] = *self
            .entries
            .get(index)
            .ok_or_else(|| Error::NotFound(format!("animation {index}")))?;
        let (frames, channels) = (frames as usize, channels as usize);
        if channels > 64 || frames > 10_000 {
            return Err(Error::format("implausible animation entry"));
        }
        if flags & !3 != 0 {
            // `aniface.ani`'s face animations (flag 4) use another layout.
            return Err(Error::format(format!(
                "animation flags {flags:#x} not decoded"
            )));
        }
        let rot = offset as usize + 16 * frames;
        let f = |at: usize| crate::bytes::f32_be(&self.data, at);
        let mut out = Vec::with_capacity(frames);
        let mut pose: Vec<[f32; 4]> = (0..channels)
            .map(|c| {
                Ok([
                    f(rot + 16 * c)?,
                    f(rot + 16 * c + 4)?,
                    f(rot + 16 * c + 8)?,
                    f(rot + 16 * c + 12)?,
                ])
            })
            .collect::<Result<_>>()?;
        for frame in 0..frames {
            if flags & 2 == 0 {
                let at = rot + 16 * channels * frame;
                for (c, q) in pose.iter_mut().enumerate() {
                    for (k, v) in q.iter_mut().enumerate() {
                        *v = f(at + 16 * c + 4 * k)?;
                    }
                }
            } else if frame > 0 {
                let row = crate::bytes::range(
                    &self.data,
                    rot + 16 * channels + 4 * channels * (frame - 1),
                    4 * channels,
                )?;
                for (q, d) in pose.iter_mut().zip(row.as_chunks::<4>().0) {
                    for k in 0..4 {
                        q[k] += self.deltas[d[k] as usize];
                    }
                }
            }
            let mut frame_pose = pose.clone();
            if flags & 1 != 0 {
                self.mirror_pose(&mut frame_pose);
            }
            out.push(
                frame_pose
                    .into_iter()
                    .map(|q| {
                        let n = q.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
                        q.map(|v| v / n)
                    })
                    .collect(),
            );
        }
        Ok(crate::anim::Rotations {
            bones: (0..channels as u8).collect(),
            frames: out,
        })
    }

    /// One animation's root translation per frame, in the original game's units.
    pub fn root(&self, index: usize) -> Result<Vec<[f32; 3]>> {
        let [frames, _, offset, flags] = *self
            .entries
            .get(index)
            .ok_or_else(|| Error::NotFound(format!("animation {index}")))?;
        (0..frames as usize)
            .map(|k| {
                let at = offset as usize + 16 * k;
                let f = |o: usize| crate::bytes::f32_be(&self.data, at + o);
                let x = f(0)?;
                Ok([if flags & 1 != 0 { -x } else { x }, f(4)?, f(8)?])
            })
            .collect()
    }

    /// 0x800ccee8: swap each body bone with its partner, flipping
    /// components by the pair's kind.
    fn mirror_pose(&self, pose: &mut [[f32; 4]]) {
        for (i, &(j, kind)) in self.mirror.iter().enumerate() {
            if i >= pose.len() || j >= pose.len() {
                continue;
            }
            let (a, b) = (pose[i], pose[j]);
            let flip = |q: [f32; 4]| -> Option<[f32; 4]> {
                let [x, y, z, w] = q;
                Some(match kind {
                    0 => [x, -y, -z, w],
                    1 => [-z, -w, -x, -y],
                    2 => [w, z, y, x],
                    3 => [-x, y, -z, w],
                    4 => [-x, -y, z, w],
                    _ => return None,
                })
            };
            if let (Some(fa), Some(fb)) = (flip(a), flip(b)) {
                pose[j] = fa;
                pose[i] = fb;
            }
        }
    }
}

/// The file offset of a `main.dol` address.
fn dol_offset(dol: &[u8], va: u32) -> Option<usize> {
    let be = |at: usize| crate::bytes::u32_be(dol, at).ok();
    (0..18).find_map(|i| {
        let (off, addr, size) = (be(4 * i)?, be(0x48 + 4 * i)?, be(0x90 + 4 * i)?);
        (size > 0 && (addr..addr + size).contains(&va)).then(|| (off + va - addr) as usize)
    })
}

/// `abdur-rahim` -> `Abdur-Rahim`.
fn title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        out.push(if start { c.to_ascii_uppercase() } else { c });
        start = !c.is_ascii_alphanumeric() && c != '\'';
    }
    out
}

/// The crews' kit table (0x8025caa8): per kit a name pointer, the build, a
/// body-sheet name pointer and a tattoo index.
fn read_kits(dol: &[u8]) -> Result<Vec<(String, i32, String)>> {
    let be = |at: usize| crate::bytes::u32_be(dol, at);
    let file_offset = |va: u32| dol_offset(dol, va);
    let cstr = |va: u32| -> Result<String> {
        let at = file_offset(va).ok_or_else(|| Error::format("kit name outside main.dol"))?;
        crate::bytes::cstr(dol, at)
    };
    let base = file_offset(KITS).ok_or_else(|| Error::format("no kit table in main.dol"))?;
    (0..KIT_COUNT)
        .map(|k| {
            let at = base + 32 * k;
            Ok((cstr(be(at)?)?, be(at + 4)? as i32, cstr(be(at + 8)?)?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_title_cased() {
        assert_eq!(title("abdur-rahim"), "Abdur-Rahim");
        assert_eq!(title("o'neal"), "O'neal");
        assert_eq!(title("skip to"), "Skip To");
    }

    #[test]
    fn kit_table_entries_follow_the_dol_sections() {
        // A one-section DOL whose data holds a two-entry table and names.
        let mut dol = vec![0u8; 0x100];
        let data_off = 0x100u32;
        let va = KITS;
        dol[0x1c..0x20].copy_from_slice(&data_off.to_be_bytes());
        dol[0x48 + 0x1c..0x48 + 0x20].copy_from_slice(&va.to_be_bytes());
        let mut section = vec![0u8; 32 * KIT_COUNT + 32];
        let names = va + 32 * KIT_COUNT as u32;
        for k in 0..KIT_COUNT {
            section[32 * k..32 * k + 4].copy_from_slice(&names.to_be_bytes());
            section[32 * k + 4..32 * k + 8].copy_from_slice(&(k as u32 % 4).to_be_bytes());
            section[32 * k + 8..32 * k + 12].copy_from_slice(&(names + 5).to_be_bytes());
        }
        let at = 32 * KIT_COUNT;
        section[at..at + 10].copy_from_slice(b"cp00\x001mw1\x00");
        dol[0x90 + 0x1c..0x90 + 0x20].copy_from_slice(&(section.len() as u32).to_be_bytes());
        dol.extend(section);
        let kits = read_kits(&dol).unwrap();
        assert_eq!(kits[2], ("cp00".into(), 2, "1mw1".into()));
    }
}
