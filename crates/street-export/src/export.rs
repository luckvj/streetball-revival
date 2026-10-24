//! `street-export characters`: the whole export folder for Vol. 2.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use serde_json::{Map, Value, json};
use street_assets::{
    db, fe, gait, iso::Iso, moves::Kind, player, scripts::MoveScripts, ssh, timeline,
};
use street_sim::gait_select;

use crate::anims::{Banks, MIRROR};
use crate::hires::Hires;
use crate::model::Library;
use crate::out::Out;
use crate::{glb, ui, vol2};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// What to export.
pub struct Options {
    pub out: PathBuf,
    pub vol2: PathBuf,
    pub roster_db: Option<PathBuf>,
    pub hires: Option<PathBuf>,
    pub v3: Option<PathBuf>,
    pub v1: Option<PathBuf>,
    /// Only these character ids (`vol2:12`); every one when empty.
    pub only: Vec<String>,
    /// Also every bank whole (`anims/vol2/<bank>.glb`).
    pub all_anims: bool,
    /// Skip `ui/` (for quick character-only runs).
    pub no_ui: bool,
    pub threads: usize,
}

/// What a run wrote.
pub struct Report {
    pub characters: usize,
    pub clips: usize,
    pub bytes: u64,
    pub files: u64,
    pub seconds: f32,
    pub warnings: Vec<String>,
}

/// The roles a character's animation set names, as `street-game`'s
/// `clip.rs` plays them in a match (every name is a clip on the disc).
pub const ROLES: [(&str, &str); 17] = [
    ("stand", "createplayer_stand"),
    ("idle", "STANDRELAX_ANTICIPATION_01_2"),
    ("defend", "defensivestand02_faceup"),
    ("walk", "WALK_CYCLE_NOBALL_01_3"),
    ("run", "RUN_CYCLE_NOBALL_01_2"),
    ("turbo", "RUN_CYCLE_NOBALL_06_1"),
    ("standBall", "dribble_stand_flair_5_r"),
    ("walkBall", "WALK_CYCLE_WBALL_01_3_R"),
    ("runBall", "RUN_CYCLE_WBALL_01_4_R"),
    ("turboBall", "RUN_CYCLE_WBALL_06_1_R"),
    ("shot", "jumpshot_r_3"),
    ("fadeaway", "jumpshot_fade_r_2"),
    ("layup", "1footjoglayup01_notrans_l"),
    ("dunk", "dunk_1hnd_r_1"),
    ("pass", "jogpass_2handchest"),
    ("block", "STANDBLOCK_FREESTYLE01_2_R"),
    ("rebound", "rebound_l_1"),
];

/// The celebration set (the game's table at 0x3e4e70) and the winners'
/// outros, as `street-game`'s `clip.rs` lists them.
pub const CELEBRATIONS: [&str; 15] = [
    "celeb01_FINAL",
    "celeb03_FINAL",
    "celeb05_FINAL",
    "CELEBRATE_FREESTYLE_08_1",
    "CELEBRATE_FREESTYLE_14_2",
    "CELEBRATE_FREESTYLE_16_2",
    "CELEBRATE_FREESTYLE_16_3",
    "CELEBRATE_FREESTYLE07_1",
    "CELEBRATE_FREESTYLE11_1",
    "CELEBRATE_FREESTYLE02_4",
    "CELEBRATE_FREESTYLE03_1",
    "CELEBRATE_FREESTYLE03_3",
    "CELEBRATE_FREESTYLE05_1",
    "jogceleb_bringdownhouse_FINAL",
    "jogceleb_sidehistep_FINAL",
];
pub const OUTROS: [&str; 5] = [
    "OUTRO_GENERIC_1_1_L",
    "OUTRO_GENERIC_2_1_L",
    "OUTRO_GENERIC_3_1_L",
    "OUTRO_GENERIC_4_2_L",
    "OUTRO_GENERIC_5_1_L",
];

/// Locomotion families the gait records carry (`street_sim::input::GaitFamily`)
/// and the ball flags (1 without, 2 with).
const GAITS: [(&str, u32); 6] = [
    ("walk", 0x400 | 1),
    ("jog", 0x800 | 1),
    ("run", 0x1000 | 1),
    ("walkBall", 0x400 | 2),
    ("jogBall", 0x800 | 2),
    ("runBall", 0x1000 | 2),
];

const BASE: &str = "anims/vol2/base.glb";
const MOVES: &str = "anims/vol2/moves.glb";

/// The kinds' names in the catalog (XDBMOVE field 2) and their script tables.
const KINDS: [(&str, &str, Kind); 3] = [
    ("layup", "layups", Kind::Shot),
    ("dunk", "dunks", Kind::Dunk),
    ("trick", "tricks", Kind::Trick),
];

/// A record's equipped rows per kind (fields 68..73, 79..89, 90..100, the
/// slots 0x1d8778 reads); -1 or an invalid row is an empty slot.
fn equipped(p: &db::Player, moves: &[db::Move], kind: usize, slot: usize) -> Option<usize> {
    let field = [68, 79, 90][kind] + slot;
    let row = usize::try_from(*p.fields.get(field)?).ok()?;
    (moves.get(row)?.kind == kind as i16).then_some(row)
}

fn time_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    rfc3339(secs)
}

/// Seconds since 1970 as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn rfc3339(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// Run the export.
pub fn characters(opts: &Options, hires: &dyn Hires) -> Result<Report, Error> {
    let start = Instant::now();
    let mut warnings = Vec::new();
    let roster = match &opts.roster_db {
        Some(dir) => {
            let found = db::find_roster_dir(dir)
                .ok_or_else(|| format!("no {} under {}", db::PLAYERS_FILE, dir.display()))?;
            db::set_roster_override(Some(&found))?;
            "legendary"
        }
        None => "original",
    };
    let out = Out::new(&opts.out)?;
    let mut iso = Iso::open(&opts.vol2)?;
    let players = db::players(&mut iso)?;
    let teams = db::teams(&mut iso)?;
    let moves = db::moves(&mut iso)?;
    let skeleton = player::skeleton(&mut iso)?;
    let scripts = MoveScripts::read(&mut iso)
        .map_err(|e| warnings.push(format!("move scripts: {e}")))
        .ok();
    let rows = timeline::ScriptBank::read(&mut iso)
        .map(|b| b.animations)
        .map_err(|e| warnings.push(format!("animation rows: {e}")))
        .ok();
    let gait = gait::GaitData::read(&mut iso)
        .map_err(|e| warnings.push(format!("gait records: {e}")))
        .ok();
    let banks = Banks::load(&mut iso)?;

    let wanted: Vec<usize> = if opts.only.is_empty() {
        (0..players.len()).collect()
    } else {
        opts.only
            .iter()
            .filter_map(|id| id.strip_prefix("vol2:")?.parse().ok())
            .filter(|&r: &usize| r < players.len())
            .collect()
    };

    // Portraits (one sheet per XAPORTS entry).
    let portraits: Vec<Option<ssh::Texture>> = fe::read(&mut iso, "/FE/XAPORTS.BIN")
        .map(|sheets| {
            sheets
                .into_iter()
                .map(|s| s.textures.into_iter().next())
                .collect()
        })
        .unwrap_or_else(|e| {
            warnings.push(format!("portraits: {e}"));
            Vec::new()
        });

    // Characters, on every core: each thread opens the disc itself.
    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let threads = opts.threads.max(1).min(wanted.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let mut iso = match Iso::open(&opts.vol2) {
                    Ok(i) => i,
                    Err(e) => {
                        failures.lock().unwrap().push(format!("disc: {e}"));
                        return;
                    }
                };
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&r) = wanted.get(k) else { break };
                    if let Err(e) =
                        one_character(&mut iso, &out, &players[r], &skeleton, hires, &portraits)
                    {
                        failures
                            .lock()
                            .unwrap()
                            .push(format!("vol2:{r} {}: {e}", players[r].name()));
                    }
                }
            });
        }
    });
    let failed = failures.into_inner().unwrap();
    let exported: BTreeSet<usize> = wanted
        .iter()
        .copied()
        .filter(|r| !failed.iter().any(|f| f.starts_with(&format!("vol2:{r} "))))
        .collect();
    warnings.extend(failed);

    // Animation libraries.
    let names = |row: u16| -> Option<String> {
        let a = rows.as_ref()?.get(usize::from(row))?;
        let name = a.name.clone()?;
        Some(if a.mirrored {
            format!("{name}{MIRROR}")
        } else {
            name
        })
    };
    let gait_records: Vec<gait_select::GaitRecord> = gait
        .as_ref()
        .map(|g| {
            g.records
                .iter()
                .map(|r| gait_select::GaitRecord {
                    rows: r.rows,
                    category_mask: r.category_mask,
                    player: r.player_restriction,
                    minimum_handles: r.minimum_handles,
                    root_distance: r.root_distance,
                    motion: [r.minimum_motion, r.maximum_motion],
                    flags: r.flags,
                })
                .collect()
        })
        .unwrap_or_default();

    let mut base: BTreeSet<String> = ROLES.iter().map(|(_, c)| (*c).to_owned()).collect();
    base.extend(CELEBRATIONS.iter().map(|c| (*c).to_owned()));
    base.extend(OUTROS.iter().map(|c| (*c).to_owned()));
    for r in &gait_records {
        base.extend(r.rows.iter().filter_map(|&row| names(row)));
    }
    base.retain(|n| banks.contains(n));

    // Every script a move row or a default slot can play, and its clips.
    let mut script_json: Map<String, Value> = Map::new();
    let mut move_clips: BTreeSet<String> = BTreeSet::new();
    let mut clips_of = |kind: Kind, id: u16| -> Vec<String> {
        let Some(s) = scripts
            .as_ref()
            .and_then(|s| s.playback_script(kind, usize::from(id)))
        else {
            return Vec::new();
        };
        let clips: Vec<String> = s
            .clip_targets
            .iter()
            .filter(|c| banks.contains(c))
            .cloned()
            .collect();
        move_clips.extend(clips.iter().cloned());
        clips
    };
    for (_, plural, kind) in KINDS {
        let count = match kind {
            Kind::Shot => scripts.as_ref().map_or(0, |s| s.shots.len()),
            Kind::Dunk => scripts.as_ref().map_or(0, |s| s.dunks.len()),
            Kind::Trick => scripts.as_ref().map_or(0, |s| s.tricks.len()),
        };
        let list: Vec<Value> = (0..count)
            .map(|id| {
                let name = scripts
                    .as_ref()
                    .and_then(|s| s.playback_script(kind, id))
                    .map(|s| s.name.clone());
                json!({ "id": id, "name": name, "clips": clips_of(kind, id as u16), "anims": MOVES })
            })
            .collect();
        script_json.insert(plural.into(), list.into());
    }
    let move_json: Vec<Value> = moves
        .iter()
        .map(|m| {
            let (kind_name, kind) = usize::try_from(m.kind)
                .ok()
                .and_then(|k| KINDS.get(k))
                .map_or(("unknown", None), |(n, _, k)| (*n, Some(*k)));
            let script = scripts.as_ref().and_then(|s| s.row_script(m.index));
            let clips = match (kind, script) {
                (Some(k), Some(id)) => clips_of(k, id),
                _ => Vec::new(),
            };
            json!({
                "game": "vol2",
                "row": m.index,
                "name": m.name,
                "kind": kind_name,
                "level": m.level,
                "cost": m.cost,
                "moveId": m.move_id,
                "script": script,
                "clips": clips,
                "anims": MOVES,
            })
        })
        .collect();

    // Write the libraries.
    let lib_joints = vol2::joints(&skeleton, None, None, 4.05);
    let mut clip_index: Map<String, Value> = Map::new();
    let mut write_lib = |name: &str, file: &str, set: &BTreeSet<String>| -> Result<usize, Error> {
        let (clips, missing) = banks.clips(set);
        for m in missing {
            warnings.push(format!("{file}: clip {m} not decoded"));
        }
        for c in &clips {
            clip_index.entry(c.name.clone()).or_insert_with(|| {
                json!({
                    "file": file,
                    "bank": banks.bank_of(&c.name),
                    "frames": c.frames,
                    "seconds": c.seconds(),
                })
            });
        }
        let lib = Library {
            name: name.into(),
            skeleton: "vol2".into(),
            joints: lib_joints.clone(),
            scale: 1.2 * vol2::METRES_PER_UNIT,
            turn: vol2::TURN,
            clips,
        };
        out.bytes(file, &glb::library(&lib))?;
        Ok(lib.clips.len())
    };
    let mut clip_count = write_lib("base", BASE, &base)?;
    clip_count += write_lib("moves", MOVES, &move_clips)?;
    if opts.all_anims {
        let bank_names: Vec<String> = banks.bank_names().map(str::to_owned).collect();
        for bank in bank_names {
            let set: BTreeSet<String> = banks
                .bank_clips(&bank)
                .into_iter()
                .map(str::to_owned)
                .collect();
            let file = format!("anims/vol2/{}.glb", bank.to_ascii_lowercase());
            clip_count += write_lib(&bank.to_ascii_lowercase(), &file, &set)?;
        }
    }

    // The catalog.
    let mut characters = Vec::new();
    let mut sets = Vec::new();
    for p in &players {
        if !opts.only.is_empty() && !wanted.contains(&p.index) {
            continue;
        }
        let id = format!("vol2:{}", p.index);
        let mut slots = Map::new();
        let mut slot_scripts = Map::new();
        for (k, (_, plural, kind)) in KINDS.iter().enumerate() {
            let mut rows = Vec::new();
            let mut ids = Vec::new();
            for slot in 0..street_assets::MOVE_SLOTS[k] {
                let row = equipped(p, &moves, k, slot);
                let script = scripts
                    .as_ref()
                    .and_then(|s| s.slot_script(*kind, p.index, Some(p.team), slot, row));
                // A default script shows as the row that buys it, if any.
                let row = row.or_else(|| {
                    let s = scripts.as_ref()?;
                    let id = script?;
                    moves
                        .iter()
                        .find(|m| m.kind == k as i16 && s.row_script(m.index) == Some(id))
                        .map(|m| m.index)
                });
                rows.push(row.map_or(-1, |r| r as i64));
                ids.push(script.map_or(Value::Null, Value::from));
            }
            slots.insert((*plural).into(), rows.into());
            slot_scripts.insert((*plural).into(), ids.into());
        }
        let team = usize::try_from(p.team)
            .ok()
            .filter(|&t| t < teams.len())
            .map(|t| format!("vol2:{t}"));
        let portrait = usize::try_from(p.portrait)
            .ok()
            .filter(|&i| portraits.get(i).is_some_and(Option::is_some))
            .map(|_| format!("portraits/vol2/{}.png", p.index));
        let model = exported
            .contains(&p.index)
            .then(|| format!("models/vol2/{}.glb", p.index));
        characters.push(json!({
            "id": id,
            "game": "vol2",
            "record": p.index,
            "kind": vol2::kind_name(p),
            "name": p.name(),
            "firstName": p.first_name,
            "lastName": p.last_name,
            "team": team,
            "model": model,
            "portrait": portrait,
            "skeleton": "vol2",
            "fields": fields(p),
            "raw": p.fields,
            // Its rating total (`rules.ratingTotal`): unedited, it may
            // stand above the cap; edited, no rating may rise past these.
            "baseTotal": street_assets::roster_rules::total(p),
            "moves": slots,
            "moveScripts": slot_scripts,
            "animationSet": id,
        }));
        sets.push(animation_set(p, &id, &gait_records, &names, &banks));
    }
    let team_json: Vec<Value> = teams
        .iter()
        .map(|t| {
            let logo = usize::try_from(t.logo)
                .ok()
                .map(|l| format!("ui/logos/{l}.png"));
            json!({
                "id": format!("vol2:{}", t.index),
                "game": "vol2",
                "index": t.index,
                "name": t.name,
                "city": t.city,
                "nickname": t.nickname,
                "roster": t.roster.iter().map(|r| format!("vol2:{r}")).collect::<Vec<_>>(),
                "logo": logo,
            })
        })
        .collect();

    // The GameCube games, on Vol. 2's skeleton (`gamecube`).
    let mut games = vec![json!({ "id": "vol2", "title": "the original game", "platform": "ps2" })];
    let mut team_json = team_json;
    let mut exported_count = exported.len();
    for (flag, path) in [("--v3", &opts.v3), ("--v1", &opts.v1)] {
        let Some(path) = path else { continue };
        let started = Instant::now();
        match crate::gamecube::export(
            path,
            &skeleton,
            &out,
            &opts.only,
            opts.all_anims,
            opts.threads,
            &mut warnings,
        ) {
            Ok(gc) => {
                eprintln!(
                    "street-export: {flag}: {} characters, {} clips in {:.1} s",
                    gc.models,
                    gc.clip_count,
                    started.elapsed().as_secs_f32()
                );
                games.push(gc.game);
                characters.extend(gc.characters);
                sets.extend(gc.sets);
                team_json.extend(gc.teams);
                clip_index.extend(gc.clips);
                clip_count += gc.clip_count;
                exported_count += gc.models;
            }
            Err(e) => warnings.push(format!("{flag} {}: {e}", path.display())),
        }
    }

    let manifest = if opts.no_ui {
        Value::Null
    } else {
        let mut iso = Iso::open(&opts.vol2)?;
        ui::export(&mut iso, &out, &players).map_err(|e| e.to_string())?
    };

    let catalog = json!({
        "version": 1,
        "generated": time_now(),
        "roster": roster,
        "hires": hires.active(),
        "games": games,
        "skeletons": {
            "vol2": {
                "joints": skeleton.bones.iter().map(|b| b.name.clone()).collect::<Vec<_>>(),
                "parents": skeleton.bones.iter().map(|b| b.parent.map_or(-1, |p| p as i64)).collect::<Vec<_>>(),
                "metresPerUnit": vol2::METRES_PER_UNIT,
                "fps": crate::anims::FPS,
            }
        },
        "teams": team_json,
        "moves": move_json,
        "scripts": script_json,
        "animationSets": sets,
        "clips": clip_index,
        "characters": characters,
        "options": options(&manifest),
        // What a roster must satisfy online, for the builder to clamp to.
        "rules": street_assets::roster_rules::rules_json(
            &street_assets::roster_rules::Limits::from_players(&players),
        ),
        "ui": if opts.no_ui { Value::Null } else { "ui/manifest.json".into() },
        "warnings": warnings,
    });
    out.json("catalog.json", &catalog)?;
    let (bytes, files) = out.written();
    Ok(Report {
        characters: exported_count,
        clips: clip_count,
        bytes,
        files,
        seconds: start.elapsed().as_secs_f32(),
        warnings,
    })
}

fn one_character(
    iso: &mut Iso<BufReader<File>>,
    out: &Out,
    p: &db::Player,
    skeleton: &street_assets::eagl::Skeleton,
    hires: &dyn Hires,
    portraits: &[Option<ssh::Texture>],
) -> Result<(), Error> {
    let c = vol2::character(iso, p, skeleton, hires)?;
    out.bytes(&format!("models/vol2/{}.glb", p.index), &glb::character(&c))?;
    if let Some(Some(t)) = usize::try_from(p.portrait)
        .ok()
        .and_then(|i| portraits.get(i))
    {
        out.png(&format!("portraits/vol2/{}.png", p.index), t)?;
    }
    Ok(())
}

/// The named fields of the contract.
fn fields(p: &db::Player) -> Value {
    json!({
        "playerId": p.id,
        "position": p.position,
        "jersey": p.jersey,
        "heightIn": p.height_in,
        "weightLb": p.weight_lb,
        "modelHeightIn": p.model_height_in,
        "modelWeightLb": p.model_weight_lb,
        "female": p.female,
        "ratings": p.ratings,
        "skinTone": p.skin_tone,
        "leftHanded": p.left_handed,
        "portraitIndex": p.portrait,
        "style": p.style,
        "star": p.star,
        "code": p.code,
        "build": p.build,
        "look": p.look,
        "clothing": p.clothing,
        "shoes": p.shoes,
        "hairStyle": p.hair_style,
        "hairColor": p.hair_color,
        "facialHairStyle": p.facial_hair_style,
        "facialHairColor": p.facial_hair_color,
        "bodyScale": p.body_scale,
        "accessories": p.accessories,
    })
}

/// A character's animations: the match roles, and the walks and runs its
/// gait records offer it (`FUN_00168b30`'s candidates, its own first).
fn animation_set(
    p: &db::Player,
    id: &str,
    records: &[gait_select::GaitRecord],
    names: &dyn Fn(u16) -> Option<String>,
    banks: &Banks,
) -> Value {
    let mut clips = Map::new();
    for (role, clip) in ROLES {
        if banks.contains(clip) {
            clips.insert(role.into(), clip.into());
        }
    }
    clips.insert(
        "celebrate".into(),
        CELEBRATIONS
            .iter()
            .filter(|c| banks.contains(c))
            .map(|c| Value::from(*c))
            .collect::<Vec<_>>()
            .into(),
    );
    clips.insert(
        "outro".into(),
        OUTROS
            .iter()
            .filter(|c| banks.contains(c))
            .map(|c| Value::from(*c))
            .collect::<Vec<_>>()
            .into(),
    );
    let profile = gait_select::GaitProfile {
        category: gait_select::GaitProfile::category_for(i32::from(p.weight_lb), p.female),
        identity: p.index as i32,
        handles: i32::from(p.ratings[7]),
    };
    let mut gaits = Map::new();
    let mut own = Vec::new();
    for (family, flags) in GAITS {
        let mut list: Vec<String> = Vec::new();
        if !records.is_empty() {
            for k in 0..records.len() as u32 {
                let i = gait_select::select(records, None, flags, profile, [0, k]);
                let r = &records[i];
                let Some(name) = names(r.rows[0]) else {
                    continue;
                };
                if !list.contains(&name) && banks.contains(&name) {
                    if r.player == profile.identity {
                        own.push(name.clone());
                    }
                    list.push(name);
                }
            }
        }
        // The player's own clips stand for the family when they have any.
        if let Some(first) = list
            .iter()
            .find(|n| own.contains(n))
            .or_else(|| list.first())
        {
            clips.insert(family.into(), first.clone().into());
        }
        gaits.insert(family.into(), list.into());
    }
    let mut set = json!({
        "id": id,
        "game": "vol2",
        "label": p.name(),
        "skeleton": "vol2",
        "file": BASE,
        "clips": clips,
        "gaits": gaits,
    });
    if !own.is_empty() {
        own.dedup();
        set["own"] = own.into();
    }
    set
}

/// What the builder offers, as Vol. 2's Create A Baller does.
fn options(manifest: &Value) -> Value {
    use street_sim::cab;
    let builds: Vec<Value> = (0..6)
        .map(|b| {
            let (faces, hair, beards) = db::kit_limits(b);
            json!({
                "build": b,
                "name": cab::BUILDS[b],
                "female": b >= 4,
                "dp": cab::BUILD_DP[b],
                "sizes": (0..4).map(|s| {
                    let (dp, h, w) = cab::SIZES[b][s];
                    json!({ "name": cab::SIZE_NAMES[s], "dp": dp, "heightIn": h, "weightLb": w })
                }).collect::<Vec<_>>(),
                "outfits": cab::OUTFITS[b].iter().zip(cab::OUTFIT_PRICES)
                    .map(|(&c, dp)| json!({ "clothing": c, "dp": dp }))
                    .collect::<Vec<_>>(),
                "faces": faces,
                "hairStyles": hair,
                "facialHairStyles": beards,
            })
        })
        .collect();
    let slots: Vec<Value> = db::ACCESSORY_SLOTS
        .iter()
        .enumerate()
        .filter(|(_, s)| s.sheet.is_some())
        .map(|(i, s)| {
            let base = ui::accessory_base(s.name);
            let side = match s.name.len() - base.len() {
                0 => Value::Null,
                _ => s.name[base.len()..].into(),
            };
            json!({
                "slot": i,
                "name": s.name,
                "base": base,
                "side": side,
                "choices": ui::accessory_choices(manifest, base),
                "thumbs": format!("ui/acc/{base}/"),
            })
        })
        .collect();
    json!({
        "ratings": db::RATING_NAMES,
        "skills": db::Skill::ALL.iter().map(|s| json!({
            "name": format!("{s:?}").to_ascii_lowercase(),
            "rating": s.rating(),
        })).collect::<Vec<_>>(),
        "positions": ["C", "PF", "SF", "SG", "PG"],
        "builds": builds,
        "hairColors": db::HAIR_COLORS,
        "facialHairColors": db::FACIAL_HAIR_COLORS,
        "shoeStyles": db::SHOE_STYLES,
        "accessorySlots": slots,
        "moveSlots": { "layups": street_assets::MOVE_SLOTS[0], "dunks": street_assets::MOVE_SLOTS[1], "tricks": street_assets::MOVE_SLOTS[2] },
        "skillLevels": street_sim::cab::SKILL_LEVELS,
        "skillStepPrices": street_sim::cab::SKILL_STEP_PRICES,
    })
}

/// Ids the `--only` list names, for the progress line.
pub fn describe(only: &[String]) -> String {
    if only.is_empty() {
        "every character".into()
    } else {
        only.join(",")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dates() {
        assert_eq!(super::rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(super::rfc3339(1_791_936_000), "2026-10-14T00:00:00Z");
    }
}
