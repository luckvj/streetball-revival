//! V3's and Street V1's characters (GameCube discs) for the export folder,
//! all on Vol. 2's skeleton so that every clip of every game plays on every
//! model by bone name.
//!
//! - Models: `street_assets::gc_character` assembles each character, and
//!   `street_assets::canonical::rebind` puts it on the canonical skeleton
//!   (Vol. 2's 43 bones, names and rest frames; the character's own bone
//!   positions; a Nintendo star simply has some joints without weight). The
//!   glTF is then written like a Vol. 2 player's (`vol2::joints`): root
//!   lifted onto the floor and half turned, the armature half turned and
//!   scaled so the crown stands at the model height (`modelHeightIn`).
//! - Animation libraries: each clip baked onto Vol. 2's skeleton through
//!   `canonical::Baker` (`retarget` by bone name, root motion scaled by hip
//!   height), named as the game names it. V3: `anims/v3gc/base.glb` (the
//!   front-end, in-game and second in-game banks; a name in two banks keeps
//!   the first), `anims/v3gc/nintendo.glb` (`anibtrex.bin`, the Nintendo
//!   All-Stars' versions of the in-game set); `--all-anims` adds the intro
//!   and judges banks. Street V1: `anims/v1gc/base.glb` (`anibot.ani`) and
//!   `anims/v1gc/front.glb` (`anifront.ani`, numbered `front_<n>`).
//! - Portraits: neither disc's portrait art is decoded, so each character
//!   gets a head shot rendered from its bind pose ([`crate::headshot`]).

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Map, Value, json};
use street_assets::{
    anim,
    canonical::{self, Baker, Rebound, VOL2_HIP},
    character,
    eagl::Skeleton,
    gc_character::{
        self as gc, CharacterInfo, Game, Kind, Library, Look, NINTENDO_RIG, V1Animations, V3Rigs,
        VOL2_RIG_IN_V3,
    },
    gcdisc::GcDisc,
};

use crate::anims::FPS;
use crate::export::{CELEBRATIONS, Error, OUTROS, ROLES};
use crate::model::{self, Clip, Image, Library as ClipLibrary, Material, Primitive};
use crate::out::Out;
use crate::vol2;

/// What one disc added to the export.
pub struct Exported {
    /// The catalog's `games` entry.
    pub game: Value,
    pub characters: Vec<Value>,
    pub teams: Vec<Value>,
    pub sets: Vec<Value>,
    /// Clip index entries, keyed `<game>:<name>` (`<game>:nintendo:<name>`
    /// for the Nintendo library).
    pub clips: Map<String, Value>,
    pub models: usize,
    pub clip_count: usize,
}

/// A role's clip: the first of these names a library has (any case).
/// Vol. 2's own role names come first (V3 and Street V1 share most).
const ROLE_FALLBACKS: &[(&str, &[&str])] = &[
    ("stand", &["createplayer_stand", "STANDTALL", "BREATHE_1"]),
    ("idle", &["breathe_1", "bm_stand_no_ball", "STNDRLX_NOBALL"]),
    (
        "walk",
        &["walk_cycle_noball_01_3", "bm_wlk_no_ball", "WALK_1"],
    ),
    ("jog", &["bm_jog_no_ball", "RUN_3"]),
    ("run", &["run_cycle_noball_01_2", "bm_run_no_ball", "RUN_1"]),
    ("turbo", &["run_cycle_noball_06_1", "RUN_2"]),
    (
        "standBall",
        &["bm_stand_drib_r", "DRIBBLE_STAND_1_R", "dribble_stand_1_l"],
    ),
    ("walkBall", &["bm_wlk_w_ball_r", "DRIBBLE_WALK_POWER_1_R"]),
    ("runBall", &["bm_run_w_ball_r", "DRIBBLE_RUN_POWER_1_R"]),
    ("turboBall", &["DRIBBLE_RUN_FINESSE_1_R"]),
    ("rebound", &["rebound_standing_3_r", "REBOUND_R_1"]),
    (
        "dunk",
        &[
            "dunk_2f1h_windmill_notrans_r",
            "DUNK_2F1H_WINDMILL_NOTRANS_R",
        ],
    ),
];

/// Celebrations beyond Vol. 2's own names: V3's short and post-gamebreaker
/// ones, Street V1's dances.
const CELEBRATE_PREFIXES: &[&str] = &[
    "celeb_short_",
    "celeb_post_gb_",
    "STAND_DANCE",
    "STNDRLX_DANCE",
];

/// A baked clip library being filled.
struct Lib {
    name: &'static str,
    file: String,
    /// Index key prefix (`v3gc`, `v3gc:nintendo`).
    key: String,
    clips: Vec<Clip>,
    names: BTreeSet<String>,
}

impl Lib {
    fn new(game: &str, name: &'static str, nintendo: bool) -> Self {
        Self {
            name,
            file: format!("anims/{game}/{name}.glb"),
            key: if nintendo {
                format!("{game}:nintendo")
            } else {
                game.to_owned()
            },
            clips: Vec::new(),
            names: BTreeSet::new(),
        }
    }

    /// The library's name for `want` (any case), if it has it.
    fn find(&self, want: &str) -> Option<&str> {
        self.clips
            .iter()
            .map(|c| c.name.as_str())
            .find(|n| n.eq_ignore_ascii_case(want))
    }

    fn push(&mut self, clip: Clip) -> bool {
        if self.names.insert(clip.name.to_ascii_lowercase()) {
            self.clips.push(clip);
            true
        } else {
            false
        }
    }

    /// The role clips of an animation set drawn from this library.
    fn roles(&self) -> Map<String, Value> {
        let mut clips = Map::new();
        for (role, name) in ROLES {
            let found = self.find(name).or_else(|| {
                ROLE_FALLBACKS
                    .iter()
                    .filter(|(r, _)| *r == role)
                    .flat_map(|(_, names)| names.iter())
                    .find_map(|n| self.find(n))
            });
            if let Some(n) = found {
                clips.insert(role.into(), n.into());
            }
        }
        // Roles only the fallbacks name (`jog`).
        for (role, names) in ROLE_FALLBACKS {
            if !clips.contains_key(*role)
                && let Some(n) = names.iter().find_map(|n| self.find(n))
            {
                clips.insert((*role).into(), n.into());
            }
        }
        let mut celebrate: Vec<Value> = CELEBRATIONS
            .iter()
            .filter_map(|c| self.find(c))
            .map(Value::from)
            .collect();
        for c in &self.clips {
            if CELEBRATE_PREFIXES.iter().any(|p| {
                c.name
                    .to_ascii_lowercase()
                    .starts_with(&p.to_ascii_lowercase())
            }) && !celebrate.iter().any(|v| v == c.name.as_str())
            {
                celebrate.push(c.name.clone().into());
            }
        }
        clips.insert("celebrate".into(), celebrate.into());
        let outro: Vec<Value> = OUTROS
            .iter()
            .filter_map(|c| self.find(c))
            .map(Value::from)
            .collect();
        clips.insert("outro".into(), outro.into());
        clips
    }

    fn write(
        &self,
        out: &Out,
        joints: &[model::Joint],
        index: &mut Map<String, Value>,
    ) -> Result<usize, Error> {
        for c in &self.clips {
            index.insert(
                format!("{}:{}", self.key, c.name),
                json!({
                    "file": self.file,
                    "bank": self.name,
                    "frames": c.frames,
                    "seconds": c.seconds(),
                }),
            );
        }
        let lib = ClipLibrary {
            name: self.name.into(),
            skeleton: "vol2".into(),
            joints: joints.to_vec(),
            scale: 1.2 * vol2::METRES_PER_UNIT,
            turn: vol2::TURN,
            clips: self.clips.clone(),
        };
        out.bytes(&self.file, &crate::glb::library(&lib))?;
        Ok(self.clips.len())
    }
}

/// A baked clip as the glTF writer takes it.
fn clip(name: String, baked: canonical::Baked, frames: usize) -> Clip {
    Clip {
        name,
        fps: FPS,
        frames,
        rotations: baked.rotations,
        translations: if baked.root.is_empty() {
            Vec::new()
        } else {
            vec![(0, baked.root)]
        },
    }
}

/// Which family a character's own skeleton is.
fn source_skeleton(game: Game, c: &character::Character) -> &'static str {
    match (game, c.skeleton.bones.len()) {
        (Game::V1, _) => "v1gc",
        (Game::V3, n) if n < 22 => "nintendo",
        (Game::V3, _) => "v3gc",
    }
}

/// The catalog's kind: the Nintendo All-Stars apart from the other guests.
fn kind_name(info: &CharacterInfo, team_name: Option<&str>) -> &'static str {
    match info.kind {
        Kind::Guest if team_name == Some("Nintendo All-Stars") => "nintendo",
        k => k.as_str(),
    }
}

/// The rebound character as a glTF-ready [`model::Character`].
fn to_model(r: &Rebound, info: &CharacterInfo, height_in: f32, source: &str) -> model::Character {
    let c = &r.character;
    let joints = vol2::joints(&c.skeleton, None, None, r.hip);
    let images: Vec<Image> = c
        .textures
        .iter()
        .map(|t| Image {
            name: t.name.clone(),
            width: t.width,
            height: t.height,
            rgba: t.rgba.clone(),
        })
        .collect();
    let mut materials: Vec<Material> = c
        .textures
        .iter()
        .enumerate()
        .map(|(i, t)| Material {
            name: t.name.clone(),
            image: Some(i),
            color: [1.0; 4],
            alpha_mask: t.alpha,
        })
        .collect();
    let untextured = materials.len();
    materials.push(Material {
        name: "-".into(),
        image: None,
        color: [0.35, 0.35, 0.38, 1.0],
        alpha_mask: false,
    });
    let primitives = c
        .meshes
        .iter()
        .filter(|m| !m.indices.is_empty())
        .map(|m| Primitive {
            name: m.name.clone(),
            positions: m.positions.clone(),
            normals: m.normals.clone(),
            uvs: m.uvs.clone(),
            colors: Vec::new(),
            joints: m.joints.clone(),
            weights: m.weights.clone(),
            indices: m.indices.clone(),
            material: m.texture.filter(|&t| t < untextured).unwrap_or(untextured),
        })
        .collect();
    let mut extras = Map::new();
    extras.insert("heightIn".into(), info.height_in.into());
    extras.insert("modelHeightIn".into(), height_in.into());
    extras.insert("record".into(), info.record.into());
    extras.insert("name".into(), info.name.clone().into());
    extras.insert("sourceSkeleton".into(), source.into());
    model::Character {
        id: info.id.clone(),
        name: info.name.clone(),
        skeleton: "vol2".into(),
        joints,
        primitives,
        materials,
        images,
        scale: height_in * 0.0254 / r.height.max(1e-3),
        turn: vol2::TURN,
        extras,
    }
}

/// A rendered head shot of a rebound character.
fn portrait(r: &Rebound) -> Option<Vec<u8>> {
    let s = &r.character.skeleton;
    let world = character::pose_from_locals(&s.bones, &[]);
    let at = |name: &str| {
        s.bones
            .iter()
            .position(|b| b.name == name)
            .map(|i| [world[i][12], world[i][13], world[i][14]])
    };
    let size = 128;
    let rgba = crate::headshot::render(&r.character, at("Neck")?, at("Head")?, size)?;
    Some(crate::picture::encode(size, size, &rgba))
}

/// One exported character: id, model height (in), source skeleton, and
/// whether a portrait was written.
type Done = (String, f32, &'static str, bool);

/// Characters per thread: assemble, rebind, write the model and portrait.
#[allow(clippy::too_many_arguments)]
fn models(
    path: &Path,
    lib: &Library,
    wanted: &[&CharacterInfo],
    vol2: &Skeleton,
    stock_units: f32,
    out: &Out,
    threads: usize,
) -> (Vec<Done>, Vec<String>) {
    let next = AtomicUsize::new(0);
    let done = Mutex::new(Vec::new());
    let failures = Mutex::new(Vec::new());
    let prefix = lib.game.prefix();
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1).min(wanted.len().max(1)) {
            scope.spawn(|| {
                let mut disc = match GcDisc::open(path) {
                    Ok(d) => d,
                    Err(e) => {
                        failures
                            .lock()
                            .unwrap()
                            .push(format!("{}: {e}", path.display()));
                        return;
                    }
                };
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(info) = wanted.get(k) else { break };
                    let result = (|| -> Result<(f32, &'static str, bool), Error> {
                        let c = lib.assemble(&mut disc, &info.key, Look::Home)?;
                        let r = canonical::rebind(&c, vol2)?;
                        let source = source_skeleton(lib.game, &c);
                        let height = gc::model_height_in(info, &c, stock_units);
                        let m = to_model(&r, info, height, source);
                        out.bytes(
                            &format!("models/{prefix}/{}.glb", info.key),
                            &crate::glb::character(&m),
                        )?;
                        let portrait = match portrait(&r) {
                            Some(png) => {
                                out.bytes(&format!("portraits/{prefix}/{}.png", info.key), &png)?;
                                true
                            }
                            None => false,
                        };
                        Ok((height, source, portrait))
                    })();
                    match result {
                        Ok((h, s, p)) => done.lock().unwrap().push((info.id.clone(), h, s, p)),
                        Err(e) => failures.lock().unwrap().push(format!("{}: {e}", info.id)),
                    }
                }
            });
        }
    });
    (done.into_inner().unwrap(), failures.into_inner().unwrap())
}

/// Bake V3's banks into the base and Nintendo libraries.
fn v3_libraries(
    disc: &mut GcDisc,
    lib: &Library,
    vol2: &Skeleton,
    all: bool,
    warnings: &mut Vec<String>,
) -> Result<Vec<Lib>, Error> {
    let rigs = V3Rigs::load(disc)?;
    // Hip heights of the rigs, from a stock character and Mario.
    let hip_of = |disc: &mut GcDisc, c: &CharacterInfo| -> Result<f32, Error> {
        let ch = lib.assemble(disc, &c.key, Look::Home)?;
        Ok(VOL2_HIP / canonical::rebind(&ch, vol2)?.scale)
    };
    let stock = lib
        .characters
        .iter()
        .find(|c| c.kind == Kind::League)
        .ok_or("V3: no league player to measure the rig by")?;
    let body_hip = hip_of(disc, stock)?;
    let nintendo_hip = match lib.find("mario") {
        Some(m) => hip_of(disc, m)?,
        None => body_hip,
    };
    let body = Baker::new(&rigs.body, vol2, body_hip);
    let vol2_rig = Baker::new(&rigs.vol2, vol2, body_hip);
    let nintendo = Baker::new(&rigs.nintendo, vol2, nintendo_hip);
    let mut base = Lib::new("v3gc", "base", false);
    let mut trex = Lib::new("v3gc", "nintendo", true);
    let mut extra: Vec<Lib> = Vec::new();
    let mut groups: Vec<(&str, usize)> = vec![
        ("anibfe.bin", 0),
        ("anibig.bin", 0),
        ("anibs2.bin", 0),
        ("anibtrex.bin", 1),
    ];
    if all {
        groups.push(("anibin.bin", 2));
        groups.push(("anibju.bin", 3));
        extra.push(Lib::new("v3gc", "intro", false));
        extra.push(Lib::new("v3gc", "judges", false));
    }
    let mut skipped = 0;
    for (bank_name, target) in groups {
        let bank = match gc::v3_animation_bank(disc, bank_name) {
            Ok(b) => b,
            Err(e) => {
                warnings.push(format!("V3 {bank_name}: {e}"));
                continue;
            }
        };
        let names: Vec<String> = bank.animations().map(str::to_owned).collect();
        for name in names {
            let Ok(Some(track)) = bank.rotations(&name) else {
                skipped += 1;
                continue;
            };
            let baker = match bank.rig(&name).unwrap_or(0) {
                NINTENDO_RIG => &nintendo,
                VOL2_RIG_IN_V3 | 0xc016 => &vol2_rig,
                0xf79b | 0xc036 => &body,
                _ => {
                    skipped += 1;
                    continue;
                }
            };
            let root: Option<Vec<[f32; 3]>> = bank.root_motion(&name).ok().flatten().map(|r| {
                r.frames
                    .iter()
                    .map(|t| t.map(|v| v * anim::TRANSLATION_SCALE))
                    .collect()
            });
            let frames = track.frames.len();
            let baked = baker.bake(&track.bones, &track.frames, root.as_deref());
            let lib = match target {
                0 => &mut base,
                1 => &mut trex,
                n => &mut extra[n - 2],
            };
            lib.push(clip(name, baked, frames));
        }
    }
    if skipped > 0 {
        warnings.push(format!(
            "V3: {skipped} animations left out (face, cloth and prop rigs, or undecoded)"
        ));
    }
    let mut libs = vec![base, trex];
    libs.extend(extra);
    Ok(libs)
}

/// Bake Street V1's in-game and front-end files.
fn v1_libraries(
    disc: &mut GcDisc,
    lib: &Library,
    vol2: &Skeleton,
    warnings: &mut Vec<String>,
) -> Result<Vec<Lib>, Error> {
    let stock = lib
        .characters
        .iter()
        .find(|c| c.kind == Kind::League)
        .ok_or("Street V1: no league player to measure the rig by")?;
    let ch = lib.assemble(disc, &stock.key, Look::Home)?;
    let r = canonical::rebind(&ch, vol2)?;
    let baker = Baker::new(&ch.skeleton, vol2, VOL2_HIP / r.scale);
    let mut libs = Vec::new();
    for (file, name) in [("anibot.ani", "base"), ("anifront.ani", "front")] {
        let anims = match V1Animations::load(disc, file) {
            Ok(a) => a,
            Err(e) => {
                warnings.push(format!("Street V1 {file}: {e}"));
                continue;
            }
        };
        let mut out = Lib::new("v1gc", name, false);
        let names: Vec<String> = anims.names().map(str::to_owned).collect();
        let mut skipped = 0;
        for (i, clip_name) in names.into_iter().enumerate() {
            let Ok(track) = anims.rotations(i) else {
                skipped += 1;
                continue;
            };
            let root = anims.root(i).ok();
            let frames = track.frames.len();
            let baked = baker.bake(&track.bones, &track.frames, root.as_deref());
            let clip_name = if name == "front" {
                format!("front_{clip_name}")
            } else {
                clip_name
            };
            out.push(clip(clip_name, baked, frames));
        }
        if skipped > 0 {
            warnings.push(format!("Street V1 {file}: {skipped} animations left out"));
        }
        libs.push(out);
    }
    Ok(libs)
}

/// Export one GameCube disc's characters, teams and animation libraries.
#[allow(clippy::too_many_arguments)]
pub fn export(
    path: &Path,
    vol2: &Skeleton,
    out: &Out,
    only: &[String],
    all_anims: bool,
    threads: usize,
    warnings: &mut Vec<String>,
) -> Result<Exported, Error> {
    let mut disc = GcDisc::open(path)?;
    let lib = Library::open(&mut disc)?;
    let prefix = lib.game.prefix();
    let title = match lib.game {
        Game::V3 => "Street V3",
        Game::V1 => "Street V1",
    };
    let game = json!({
        "id": prefix,
        "title": title,
        "platform": "gamecube",
        "disc": disc.game_id(),
    });
    let wanted: Vec<&CharacterInfo> = lib
        .characters
        .iter()
        .filter(|c| only.is_empty() || only.iter().any(|o| o.eq_ignore_ascii_case(&c.id)))
        .collect();

    // A stock body's height in the game's units, for the guests' heights.
    let stock_units = lib.stock_units(&mut disc);

    let (done, failures) = models(path, &lib, &wanted, vol2, stock_units, out, threads);
    warnings.extend(failures);

    let libs = match lib.game {
        Game::V3 => v3_libraries(&mut disc, &lib, vol2, all_anims, warnings)?,
        Game::V1 => v1_libraries(&mut disc, &lib, vol2, warnings)?,
    };
    let lib_joints = vol2::joints(vol2, None, None, VOL2_HIP);
    let mut clips = Map::new();
    let mut clip_count = 0;
    for l in &libs {
        clip_count += l.write(out, &lib_joints, &mut clips)?;
    }
    let base = libs.iter().find(|l| l.name == "base");
    let nintendo = libs.iter().find(|l| l.name == "nintendo");

    let team_of = |c: &CharacterInfo| {
        c.team
            .filter(|t| lib.teams.iter().any(|x| x.id == *t))
            .map(|t| format!("{prefix}:{t}"))
    };
    let mut characters = Vec::new();
    let mut sets = Vec::new();
    for info in &wanted {
        let Some((_, height, source, has_portrait)) = done.iter().find(|d| d.0 == info.id) else {
            continue;
        };
        let strings: Map<String, Value> = info
            .strings
            .iter()
            .map(|(f, s)| (f.to_string(), s.clone().into()))
            .collect();
        characters.push(json!({
            "id": info.id,
            "game": prefix,
            "record": info.record,
            "kind": kind_name(info, info.team_name.as_deref()),
            "name": info.name,
            "firstName": info.first_name,
            "lastName": info.last_name,
            "nickname": info.nickname,
            "team": team_of(info),
            "model": format!("models/{prefix}/{}.glb", info.key),
            "portrait": has_portrait.then(|| format!("portraits/{prefix}/{}.png", info.key)),
            "skeleton": "vol2",
            "sourceSkeleton": source,
            "fields": {
                "playerId": info.db_id,
                "position": info.position,
                "jersey": info.jersey,
                "heightIn": info.height_in,
                "weightLb": info.weight_lb,
                "modelHeightIn": (height * 10.0).round() / 10.0,
                "female": info.female,
                "kid": info.kid,
                "ratings": info.ratings,
            },
            "raw": info.raw,
            "strings": strings,
            "animationSet": info.id,
        }));
        // The Nintendo stars' own rig plays the Nintendo library.
        let set_lib = if *source == "nintendo" {
            nintendo.or(base)
        } else {
            base
        };
        if let Some(l) = set_lib {
            sets.push(json!({
                "id": info.id,
                "game": prefix,
                "label": info.name,
                "skeleton": "vol2",
                "sourceSkeleton": source,
                "file": l.file,
                "clips": l.roles(),
            }));
        }
    }
    let teams = lib
        .teams
        .iter()
        .enumerate()
        .map(|(i, t)| {
            json!({
                "id": format!("{prefix}:{}", t.id),
                "game": prefix,
                "index": i,
                "name": t.name,
                "city": t.city,
                "nickname": t.nickname,
                "roster": t.roster,
            })
        })
        .collect();
    Ok(Exported {
        game,
        models: done.len(),
        characters,
        teams,
        sets,
        clips,
        clip_count,
    })
}
