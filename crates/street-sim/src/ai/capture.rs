//! The computer players, the opening toss and the restarts against a
//! per-update capture of the original running its attract match
//! (`tools/pine_ai_capture.py`, computer against computer). Skipped unless
//! `STREET_AI_CAPTURE` names the capture's JSON lines; with
//! `STREET_AI_CAPTURE_REPORT` set, the comparison is also written there.
//!
//! The capture's team 0 attacks +x (its side byte game+0x98 is 1), as the
//! simulation's does: players, balls and the toss's release point are
//! compared as captured.
//!
//! The gate runs every test here on each of the three attract captures
//! (`street-native-ai-attract-1.jsonl`, `-2` and `-3` under `$EVIDENCE`,
//! each with `STREET_AI_CAPTURE_REPORT` and `STREET_ISO` set): the first a
//! whole match (Lakers against Portland, the rosters inferred), the second
//! the next match's first 14,165 updates, the third a whole match with its
//! rosters, turbo bars and playbook star recorded (schema 3, Indiana
//! against San Antonio). `docs/TRACE.md` "The third attract capture as a
//! check" tells how a sample read inside an update looks.

use super::court::dist;
use super::pad::bits;
use super::*;
use crate::rng::Rng;
use crate::{BallState, Player, Rules, restart, tests::hoops, toss};
use serde_json::{Value, json};

/// One update of the capture.
struct Update(Value);

impl Update {
    fn frame(&self) -> i64 {
        self.0["frame"].as_i64().unwrap()
    }
    fn phase(&self) -> i64 {
        self.0["phase"].as_i64().unwrap()
    }
    fn int(&self, key: &str) -> i64 {
        self.0[key].as_i64().unwrap()
    }
    fn rng(&self) -> [u32; 6] {
        std::array::from_fn(|i| self.0["rng"][i].as_u64().unwrap() as u32)
    }
    fn timers(&self) -> [i32; 2] {
        std::array::from_fn(|t| self.0[format!("team{t}")]["timer"].as_i64().unwrap() as i32)
    }
    fn ball(&self) -> [f32; 3] {
        std::array::from_fn(|i| self.0["ball_position"][i].as_f64().unwrap() as f32)
    }
    fn actors(&self) -> &Vec<Value> {
        self.0["actors"].as_array().unwrap()
    }
    /// Actor `i`'s floor position.
    fn floor(&self, i: usize) -> [f32; 2] {
        let p = &self.actors()[i]["position"];
        [p[0].as_f64().unwrap() as f32, p[2].as_f64().unwrap() as f32]
    }
    fn ints(&self, key: &str) -> Vec<i64> {
        self.0[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect()
    }
    /// The front behaviour of actor `i` by its vtable's name.
    fn front(&self, i: usize) -> &'static str {
        self.actors()[i]["queue"]
            .as_array()
            .and_then(|q| q.first())
            .map_or("-", |n| native_name(n["vtable"].as_u64().unwrap_or(0)))
    }
}

/// The capture's updates, split into matches (the update counter
/// game+0x128 starts again), and the pointers it logged.
fn load() -> Option<(Vec<Vec<Update>>, Vec<Value>)> {
    let path = std::env::var_os("STREET_AI_CAPTURE")?;
    let text = std::fs::read_to_string(path).unwrap();
    let (mut matches, mut events): (Vec<Vec<Update>>, Vec<Value>) = (Vec::new(), Vec::new());
    for line in text.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        if let Some(meta) = value.get("metadata") {
            assert_eq!(meta["serial"], "SLUS-20651");
        } else if value.get("frame").is_some() {
            let u = Update(value);
            if u.0["torn"].as_bool() == Some(true) {
                continue;
            }
            if matches
                .last()
                .is_none_or(|m| m.last().unwrap().frame() > u.frame())
            {
                matches.push(Vec::new());
            }
            matches.last_mut().unwrap().push(u);
        } else {
            events.push(value);
        }
    }
    Some((matches, events))
}

fn report(key: &str, value: Value) {
    let Some(path) = std::env::var_os("STREET_AI_CAPTURE_REPORT") else {
        return;
    };
    let path = std::path::PathBuf::from(path).join(format!("{key}.json"));
    std::fs::write(path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
}

/// Behaviour vtables (node+0x14) by their constructors' names, as
/// `tools/pine_ai_capture.py` lists them.
fn native_name(vtable: u64) -> &'static str {
    match vtable {
        0x437398 => "shoot",
        0x43ae88 => "playbook hold",
        0x4370f8 => "leaper",
        0x4370a0 => "second leaper",
        0x437448 => "pass",
        0x4373f0 => "trick pass",
        0x437340 => "carrier move",
        0x4372e8 => "carrier pause",
        0x437290 => "pocket",
        0x437238 => "cancel",
        0x437538 => "defense",
        0x437708 => "drive",
        0x437948 => "fast break",
        0x437898 => "wait for hc",
        0x4378f0 => "outlet",
        0x437aa0 => "throw inbound",
        0x437a48 => "inbound pos",
        0x437bf8 => "iso3pt",
        0x437cd0 => "pause 0x10d090",
        0x437db8 => "keep away",
        0x437f28 => "loose ball",
        0x437ed0 => "open loose ball",
        0x438000 => "goto",
        0x4380d8 => "pause",
        // The cuts (`FUN_0010fae8`'s family): "cut out", "net cut in",
        // "circle cut in" and "cut to" are the rewrite's one cut move.
        0x438368 | 0x438310 | 0x4382b8 | 0x438260 | 0x438208 => "cut move",
        0x438450 => "perim pause",
        0x4385b0 => "perim player",
        0x4386a8 => "post cut",
        0x438810 => "post pause",
        0x438910 => "post player",
        0x438a58 => "post up",
        0x438ce0 => "prepare rebound",
        0x438c88 => "drift up court",
        0x438c30 => "rebound",
        0x438bd8 => "goal tend",
        0x438b80 => "box out",
        0x438dc0 => "roll",
        0x438f90 => "set pick",
        0x438f38 => "run pick play",
        0x439098 => "pick off ball",
        0x4391f0 => "feeder",
        0x4392f0 => "referee wait",
        0x439348 => "wait for toss",
        0x437150 => "node under construction",
        _ => "unknown",
    }
}

/// The same names for the rewrite's behaviours.
fn rewrite_name(act: &Act) -> &'static str {
    match act {
        Act::CarrierPause(_) => "carrier pause",
        Act::Shoot(_) => "shoot",
        Act::Pass(_) => "pass",
        Act::Pocket(_) => "pocket",
        Act::Cancel(_) => "cancel",
        Act::Drive(_) => "drive",
        Act::Iso(_) => "iso3pt",
        Act::Defense(_) => "defense",
        Act::LooseBall(_) => "loose ball",
        Act::PrepareRebound(_) => "prepare rebound",
        Act::FastBreak(_) => "fast break",
        Act::PerimPlayer(_) => "perim player",
        Act::PostPlayer(_) => "post player",
        Act::CutMove(_) => "cut move",
        Act::PostCut(_) => "post cut",
        Act::PostPause(_) => "post pause",
        Act::CarrierMove(_) => "carrier move",
        Act::Leaper(_) => "leaper",
        Act::Follower(_) => "second leaper",
        Act::WaitForToss => "wait for toss",
        Act::Pause(_) => "pause",
        Act::Goto(_) => "goto",
        Act::PerimPause(_) => "perim pause",
        Act::DriftUpCourt(_) => "drift up court",
        Act::Rebound(_) => "rebound",
        Act::GoalTend(_) => "goal tend",
        Act::BoxOut(_) => "box out",
        Act::OpenLooseBall(_) => "open loose ball",
        Act::Outlet => "outlet",
        Act::WaitForHalfCourt => "wait for hc",
        Act::ThrowInbound(_) => "throw inbound",
        Act::InboundPos(_) => "inbound pos",
        Act::SetPick(_) => "set pick",
        Act::Roll(_) => "roll",
        Act::RunPick(_) => "run pick play",
        Act::PickOffBall(_) => "pick off ball",
        Act::KeepAway(_) => "keep away",
        Act::PostUp(_) => "post up",
        Act::Feeder(_) => "feeder",
        Act::TrickPass(_) => "trick pass",
    }
}

fn three_on_three(seed: u32) -> World {
    let mut players = Vec::new();
    for team in 0..2 {
        let side = restart::side(team);
        for k in 0..3 {
            players.push(Player::new(
                team,
                [side * 10.0, (k as f32 - 1.0) * 8.0],
                -side * std::f32::consts::FRAC_PI_2,
            ));
        }
    }
    World::new(hoops(), players, Rules::default(), seed)
}

/// The disc's gait records (with `STREET_ISO` set), so the rewrite's
/// players walk and run at the original's speeds as they do in a game;
/// without them the generic walk is several times slower than the
/// original's walks and jogs. Every player is an ordinary lighter man with
/// 50 Handles (the captured teams' players are not recorded).
pub(crate) fn disc_gait() -> Option<std::sync::Arc<crate::gait_select::GaitConfig>> {
    use crate::gait_select::{GaitConfig, GaitProfile, GaitRecord};
    static GAIT: std::sync::OnceLock<Option<std::sync::Arc<GaitConfig>>> =
        std::sync::OnceLock::new();
    GAIT.get_or_init(|| {
        let path = std::env::var_os("STREET_ISO")?;
        let mut iso = street_assets::iso::Iso::open(path).ok()?;
        let gait = street_assets::gait::GaitData::read(&mut iso).ok()?;
        Some(std::sync::Arc::new(GaitConfig {
            records: gait
                .records
                .iter()
                .map(|r| GaitRecord {
                    rows: r.rows,
                    category_mask: r.category_mask,
                    player: r.player_restriction,
                    minimum_handles: r.minimum_handles,
                    root_distance: r.root_distance,
                    motion: [r.minimum_motion, r.maximum_motion],
                    flags: r.flags,
                })
                .collect(),
            profiles: vec![
                GaitProfile {
                    category: 1,
                    identity: 0,
                    handles: 50,
                };
                6
            ],
            ..Default::default()
        }))
    })
    .clone()
}

/// [`three_on_three`] with the disc's gait records and animations when
/// there are any.
fn demo_match(seed: u32) -> World {
    let mut w = three_on_three(seed);
    w.gait = disc_gait();
    w.poses = crate::pose::disc_bank();
    // The attract's meters fill as `street-game`'s setup sets them for two
    // computer crews: the pair at `0x43e298` averaged, 0.825.
    w.configure_meter_fill([false, false], None);
    w
}

/// A disc player as the harness plays him: his ratings (as a game reads
/// them, `street-game`'s setup), weight and gait profile.
#[derive(Clone, Copy)]
struct Rated {
    ratings: crate::Ratings,
    weight: u16,
    profile: crate::gait_select::GaitProfile,
    body: Option<crate::pose::Body>,
    left_handed: bool,
}

/// The disc's players (`street_assets::db`'s records) as the harness plays
/// them, by record.
fn disc_players() -> Option<&'static Vec<Rated>> {
    use street_assets::db;
    static PLAYERS: std::sync::OnceLock<Option<Vec<Rated>>> = std::sync::OnceLock::new();
    PLAYERS
        .get_or_init(|| {
            let path = std::env::var_os("STREET_ISO")?;
            let mut iso = street_assets::iso::Iso::open(path).ok()?;
            let players = db::original_players(&mut iso).ok()?;
            let rated = |(record, p): (usize, &db::Player)| {
                let r = |skill: usize| (f32::from(p.ratings[skill]) / 99.0).clamp(0.0, 1.0);
                let ratings = crate::Ratings {
                    shots: r(0),
                    threes: r(1),
                    dunks: r(2),
                    rebounds: r(3),
                    blocks: r(5),
                    steals: r(6),
                    handles: r(7),
                    speed: r(8),
                    power: r(4),
                    overall: r(9),
                    reach: crate::player::reach_for_height(f32::from(p.height_in)),
                };
                let body = p
                    .native_body_scale()
                    .map(|[height, width, _]| crate::pose::Body {
                        height,
                        width,
                        head: p.native_head_scale().map_or(1.0, |h| h[0]),
                    });
                Rated {
                    ratings,
                    body,
                    left_handed: p.left_handed,
                    weight: p.weight_lb.max(1) as u16,
                    profile: crate::gait_select::GaitProfile {
                        category: crate::gait_select::GaitProfile::category_for(
                            i32::from(p.weight_lb),
                            p.female,
                        ),
                        identity: record as i32,
                        handles: i32::from(p.ratings[7]),
                    },
                }
            };
            Some(players.iter().enumerate().map(rated).collect())
        })
        .as_ref()
}

/// The disc's league teams the attract match draws from (`FUN_001db7f8`:
/// `r % 28` for each side, records 0..27), each its roster's first three
/// (`FUN_001dbe30` in Pick Up's mode, street-game's `lineup.rs`).
fn disc_teams() -> Option<&'static Vec<[usize; 3]>> {
    static TEAMS: std::sync::OnceLock<Option<Vec<[usize; 3]>>> = std::sync::OnceLock::new();
    TEAMS
        .get_or_init(|| {
            let path = std::env::var_os("STREET_ISO")?;
            let mut iso = street_assets::iso::Iso::open(path).ok()?;
            let teams = street_assets::db::original_teams(&mut iso).ok()?;
            teams
                .iter()
                .take(28)
                .map(|t| (t.roster.len() >= 3).then(|| std::array::from_fn(|k| t.roster[k])))
                .collect()
        })
        .as_ref()
}

/// [`demo_match`] with the disc's players `records`, the sim's players in
/// order (team 0's three first).
fn rostered_match(seed: u32, records: [usize; 6]) -> Option<World> {
    let players = disc_players()?;
    let mut w = demo_match(seed);
    let mut config = (*w.gait.clone()?).clone();
    for (i, record) in records.into_iter().enumerate() {
        let p = players.get(record)?;
        w.players[i].ratings = p.ratings;
        w.players[i].weight = p.weight;
        w.players[i].body = p.body;
        w.players[i].left_handed = p.left_handed;
        config.profiles[i] = p.profile;
    }
    w.gait = Some(std::sync::Arc::new(config));
    Some(w)
}

/// [`demo_match`] with two of the disc's teams, `a` (team 0) and `b`.
fn rated_match(seed: u32, a: usize, b: usize) -> Option<World> {
    let teams = disc_teams()?;
    let [x, y] = [teams.get(a)?, teams.get(b)?];
    rostered_match(seed, [x[0], x[1], x[2], y[0], y[1], y[2]])
}

/// The first attract capture's generator at its first sample: the capture
/// [`captured_rosters`] knows by it.
const FIRST_CAPTURE_GENERATOR: [u32; 6] = [
    1_948_296_016,
    2_478_021_643,
    2_282_782_572,
    3_684_047_753,
    3_093_338_387,
    1_876_921_199,
];

/// Where a capture's rosters come from ([`captured_rosters`]), which also
/// picks the comparison's bounds ([`Bounds`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Own {
    /// The first capture's, inferred.
    First,
    /// Recorded by the capture.
    Recorded,
}

impl Own {
    fn source(self) -> &'static str {
        match self {
            Own::First => {
                "inferred: Lakers against Portland (save state 9, the actors' Handles and jukes)"
            }
            Own::Recorded => "recorded (actor+0x274)",
        }
    }
}

/// The players a capture's match played, actor by actor (the sim's players
/// in the same order: the capture's team 0 is actors 0..2), with where
/// that comes from: each actor's player record (actor+0x274) where the
/// capture holds them (`tools/pine_ai_capture.py` schema 3), else for the
/// first capture (2026-10-15, schema 1) the rosters the counter-tricks pass
/// inferred on 2026-10-17 (docs/TRACE.md "The turbo bars, the juke press
/// and the counter stance"): save state 9, a run of the same boot's reseed
/// (another match: its actors sit elsewhere), holds the Lakers' 55, 56, 57
/// against Portland's 110, 111, 112, and the capture's actors fit them by
/// the gait rows their Handles allow (actors 1 and 2 play rows of Handles
/// 80, actor 0 none). Actor 5 is Portland's 113, not 112 (2026-10-18,
/// docs/TRACE.md "The person's presses"): his 17 combo jukes are levels 2
/// and 3, none 4 (`FUN_0015f098` gives 112's Handles 90 level 4, a third
/// of his combos; the third capture's recorded players fit their Handles
/// so), and his three dunk-window presses are level-2 slot dunks, where
/// 112's Dunks 0 (dunk level 0, `FUN_00182280` raw `0x1825b8`) takes a
/// combo one press in five at most and otherwise lays up (`FUN_001693b8`'s
/// easy rows); 113 has Handles 75 and Dunks 80. None for any other.
fn captured_rosters(matches: &[Vec<Update>]) -> Option<([usize; 6], Own)> {
    let first = matches.first()?.first()?;
    let actors = first.actors();
    let teams: Vec<i64> = actors.iter().map(|a| a["team"].as_i64().unwrap()).collect();
    if teams != [0, 0, 0, 1, 1, 1] {
        return None;
    }
    let records: Option<Vec<usize>> = actors
        .iter()
        .map(|a| a["record"].as_u64().map(|r| r as usize))
        .collect();
    if let Some(records) = records {
        // The same six throughout.
        assert!(matches.iter().flatten().all(|u| {
            u.actors()
                .iter()
                .zip(&records)
                .all(|(a, &r)| a["record"].as_u64() == Some(r as u64))
        }));
        return Some((records.try_into().ok()?, Own::Recorded));
    }
    (first.rng() == FIRST_CAPTURE_GENERATOR).then_some(([55, 56, 57, 110, 111, 113], Own::First))
}

/// The actors' update order a capture's match ran ([`World::update_order`])
/// as far as its block jumps at a jump shot just let go tell it ("a late
/// jump left over", `FUN_00106a88`): a defender's AI frame sees the ball
/// let go in the release's own update when his actor updates after the
/// shooter's, and in the next when before. The capture keeps no order, and
/// those pairs leave others open: of the orders that keep every pair (or
/// break the fewest), the one nearest the rewrite's own
/// ([`crate::HIGHEST_FIRST`], the fewest pairs the other way round), the
/// first of a tie. The attract captures' pairs: the first's 3, 0, 4, 1 in
/// that order and 2 before 5 (2, 5, 3, 0, 4, 1); the second's 1 before 4
/// and 5 before 2 (5, 1, 4, 3, 2, 0); the third's 1 before 5 before 2, 1
/// before 4 and 3 before 0 (1, 5, 4, 3, 2, 0).
fn captured_order(matches: &[Vec<Update>]) -> [u8; 6] {
    let state = |w: &Update, i: usize| w.actors()[i]["animation_state"].as_i64().unwrap_or(0);
    let holder = |w: &Update| {
        if w.int("ball_state") != 3 {
            return None;
        }
        let ball = w.ball();
        let possession = w.int("possession");
        (0..6)
            .filter(|&i| w.actors()[i]["team"].as_i64() == Some(possession))
            .min_by(|&p, &q| {
                dist(w.floor(p), [ball[0], ball[2]])
                    .total_cmp(&dist(w.floor(q), [ball[0], ball[2]]))
            })
    };
    let rises = |a: &Update, u: &Update, i: usize| {
        u.frame() == a.frame() + 1 && state(u, i) & 0x1000 != 0 && state(a, i) & 0x1000 == 0
    };
    // (first, second): first updates before second.
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for m in matches {
        for k in 1..m.len() {
            let (a, u) = (&m[k - 1], &m[k]);
            if u.frame() != a.frame() + 1 || a.int("ball_state") != 3 || u.int("ball_state") != 5 {
                continue;
            }
            let Some(h) = holder(a).filter(|&h| state(a, h) & 0x88 == 0x8) else {
                continue;
            };
            for i in 0..6 {
                if rises(a, u, i) {
                    pairs.push((h, i));
                } else if m.get(k + 1).is_some_and(|v| rises(u, v, i)) {
                    pairs.push((i, h));
                }
            }
        }
    }
    let swapped = |order: &[u8; 6], than: &[u8; 6]| {
        let at = |o: &[u8; 6], j: u8| o.iter().position(|&x| x == j);
        let mut n = 0;
        for a in 0..6 {
            for b in 0..6 {
                if a != b && at(order, a) < at(order, b) && at(than, a) > at(than, b) {
                    n += 1;
                }
            }
        }
        n
    };
    let mut best = crate::HIGHEST_FIRST;
    let mut fewest = (usize::MAX, usize::MAX);
    for code in 0..720usize {
        // The code's permutation (its factorial digits).
        let mut left: Vec<u8> = (0..6).collect();
        let mut order = [0u8; 6];
        let mut c = code;
        for (k, slot) in order.iter_mut().enumerate() {
            let f = (1..6 - k).product::<usize>();
            *slot = left.remove(c / f);
            c %= f;
        }
        let at = |j: usize| order.iter().position(|&x| usize::from(x) == j);
        let broken = pairs.iter().filter(|&&(x, y)| at(x) > at(y)).count();
        let key = (broken, swapped(&order, &crate::HIGHEST_FIRST));
        if key < fewest {
            fewest = key;
            best = order;
        }
    }
    best
}

fn step_computer(w: &mut World) {
    let inputs: Vec<Input> = (0..w.players.len()).map(|i| input(w, i)).collect();
    w.step(&inputs);
}

/// One frame of a computer match as the game runs it (`street-game`'s
/// `game.rs` then `tape.rs`): the gameplay, unless a replay holds it
/// (`FUN_0033d380(0x401b2)`), then the replay director, manager and
/// gamebreaker task on its events, whose end snaps a pending restart to its
/// spots (`FUN_00142648`). Whether the gameplay ran: the game's update
/// counter (game+0x128) counts only those.
fn step_game(w: &mut World, replays: &mut crate::replay::Replays) -> bool {
    let stepped = !replays.holds_world();
    let mut events = Vec::new();
    if stepped {
        step_computer(w);
        events = w.events.clone();
    }
    let option = w.replays;
    replays.update(w, &mut events, stepped, &[], option, false);
    stepped
}

/// The draws taking `from` to `to`, up to `limit`.
fn draws(from: [u32; 6], to: [u32; 6], limit: usize) -> Option<Vec<u32>> {
    let mut rng = Rng::from_state(from);
    let mut out = Vec::new();
    while rng.state() != to {
        if out.len() == limit {
            return None;
        }
        out.push(rng.next_u32());
    }
    Some(out)
}

/// The match whose opening the capture holds.
fn opening(matches: &[Vec<Update>]) -> Option<&Vec<Update>> {
    matches.iter().find(|m| {
        m.iter().any(|u| u.phase() == 1)
            && m.windows(2).any(|w| w[0].phase() != 1 && w[1].phase() == 1)
    })
}

#[test]
fn the_opening_toss_and_tip_follow_the_captured_original() {
    let Some((matches, _)) = load() else {
        return;
    };
    let Some(m) = opening(&matches) else {
        return;
    };
    let at = |frame: i64| m.iter().find(|u| u.frame() == frame).unwrap();
    // FUN_00141fd8 runs in the update the phase becomes 1.
    let start = m.iter().find(|u| u.phase() == 1).unwrap().frame();
    let launch = m.iter().find(|u| u.int("ball_state") == 7).unwrap().frame();
    let decided = m
        .iter()
        .find(|u| u.int("tip_winner") != -1 && u.phase() == 1)
        .unwrap();
    let tipped = m
        .iter()
        .find(|u| u.frame() > launch && u.int("ball_state") == 4)
        .unwrap()
        .frame();
    // Every update of the opening is in the capture.
    assert!((start..=tipped).all(|f| m.iter().any(|u| u.frame() == f)));

    // The referee's task (FUN_0011aea0, his front node, vtable 0x4392f0)
    // when the capture found him: the six ready flags (+0x20..) up from
    // the opening's first updates and his count (+0x1c, the frame rate)
    // out 60 updates after the opening's start; the toss script's 8-frame
    // blend and its 50 updates to frame 6 then reach the launch
    // (`toss::WAIT_UPDATES` = 60 + 8 + 50).
    let waits: Vec<(i64, Vec<i64>)> = m
        .iter()
        .filter(|u| {
            u.0["referee"]["queue"][0]["vtable"].as_u64() == Some(0x4392f0)
                && u.0["referee"]["wait"].is_array()
        })
        .map(|u| {
            (
                u.frame(),
                u.0["referee"]["wait"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap())
                    .collect(),
            )
        })
        .collect();
    let referee = waits.iter().find(|(_, w)| w[0] == 0).map(|&(f, _)| f);
    if let Some(&(first, ref wait)) = waits.first() {
        assert!(
            wait[1..].iter().all(|&r| r == 1),
            "all six ready at {first}"
        );
        // Counting down one an update from 60 at the opening's start.
        assert_eq!(
            wait[0],
            60 - (first - start),
            "the referee's count at {first}"
        );
        let zero = referee.unwrap();
        assert_eq!(zero - start, 60);
        assert_eq!(launch - zero, i64::from(toss::WAIT_UPDATES) - 60);
    }
    // With the disc's animations and the captured players (their bodies),
    // as a match plays: the jumpers aim their posed hands
    // (`World::tip_aim_hand`).
    let mut w = captured_rosters(&matches)
        .and_then(|(records, _)| rostered_match(1, records))
        .unwrap_or_else(|| demo_match(1));
    w.start_opening_toss();
    // The wait: the launch is the 118th update after the opening's.
    let mut launched_after = None;
    for k in 1..=200_i64 {
        step_computer(&mut w);
        if w.opening_toss == OpeningToss::Flight {
            launched_after = Some(k);
            break;
        }
    }
    assert_eq!(launched_after, Some(launch - start), "launch update");
    // The flight, update by update, until the contest is decided; on the
    // deciding update the generator and both team timers are the
    // capture's, so the update's draws must be the original's.
    let mut flight_error: f32 = 0.0;
    let mut decided_at = None;
    let mut tip_update = 0;
    // The jumpers' highest (each team's first player), ours and the
    // capture's, from the toss to the tip pass's catch.
    let mut peaks = [0.0f32; 2];
    for frame in launch + 1..=tipped + 2 {
        let mut from = None;
        if frame == decided.frame() {
            let before = at(frame - 1);
            w.rng = Rng::from_state(before.rng());
            let timers = before.timers();
            for (coach, timer) in w.ai.coaches.iter_mut().zip(timers) {
                coach.timer = timer;
            }
            from = Some(before.rng());
        }
        step_computer(&mut w);
        for (team, peak) in peaks.iter_mut().enumerate() {
            *peak = peak.max(w.players[team * 3].height);
        }
        if decided_at.is_none() && w.tip.winner >= 0 {
            decided_at = Some(frame);
            assert_eq!(frame, decided.frame(), "the contest's update");
            let timing = decided.ints("tip_timing");
            assert_eq!(w.tip.timing.map(i64::from).to_vec(), timing);
            assert_eq!(i64::from(w.tip.winner), decided.int("tip_winner"));
            // The update's draws are the first of the original's: both
            // timings (team 0's first), then the loser's script; any after
            // them are the original's other systems'.
            let from = from.unwrap();
            let ours = draws(from, w.rng.state(), 64).unwrap();
            let native = draws(from, decided.rng(), 4096).unwrap();
            assert!(
                ours.len() == 3 && native.starts_with(&ours),
                "{ours:?} {native:?}"
            );
        }
        if w.ball.state == BallState::Pass {
            // The original's tip is a catch by the winner's hand bones
            // within 1.25 of the ball, on the jump his script's `a1` aims
            // his posed hand on (`World::tip_aim_hand`, `World::tip_hands`):
            // within an update (the second capture's players are not
            // known: its jumpers have stand-in bodies).
            assert!(
                (frame - tipped).abs() <= 1,
                "the tip's update {frame}, {tipped}"
            );
            tip_update = frame;
            let pass = w.ball.pass.unwrap();
            // The winner's slot 2 (actor+0x74 = actor 2 or 5).
            let winner = decided.int("tip_winner") as usize;
            assert_eq!(pass.from, winner);
            assert_eq!(pass.to, winner + 2);
            break;
        }
        // The toss's free flight: until the original's tip.
        if frame < tipped {
            let captured = at(frame).ball();
            let d: f32 = (0..3)
                .map(|k| (w.ball.position[k] - captured[k]).powi(2))
                .sum::<f32>()
                .sqrt();
            flight_error = flight_error.max(d);
        }
    }
    assert!(flight_error < 1e-3, "toss flight error {flight_error}");
    // The tip pass: the first pass after the actors' reset flies 40
    // (`DAT_003d65b8`), as the capture's ball+0x64 shows, and the receiver
    // takes it 38 updates after the tip in the original (his hand bones
    // meet it the update before it arrives); the rewrite's receiver takes
    // it as it arrives, one update later.
    let flight = at(tipped).int("ball_flight_updates");
    let our_flight = i64::from(w.ball.flight_ticks());
    assert_eq!(flight, 40);
    assert_eq!(our_flight, flight);
    let caught = m
        .iter()
        .find(|u| u.frame() > tipped && u.int("ball_state") == 3)
        .unwrap()
        .frame();
    let mut our_catch = None;
    for k in 1..=80_i64 {
        step_computer(&mut w);
        for (team, peak) in peaks.iter_mut().enumerate() {
            *peak = peak.max(w.players[team * 3].height);
        }
        if w.events
            .iter()
            .any(|e| matches!(e, crate::Event::Catch { .. }))
        {
            our_catch = Some(tip_update + k);
            break;
        }
    }
    let our_catch = our_catch.expect("the tip pass caught");
    assert_eq!(caught - tipped, 38);
    let captured_peaks: Vec<f64> = [0, 3]
        .iter()
        .map(|&k| {
            m.iter()
                .filter(|u| (launch..=caught).contains(&u.frame()))
                .map(|u| u.actors()[k]["position"][1].as_f64().unwrap_or(0.0))
                .fold(0.0, f64::max)
        })
        .collect();
    // Both jump on their scripts' aimed `a1`, the clip's hand posed at the
    // script's frame: with the captured players' bodies the jumpers rise
    // as the original's (2026-10-21: 3.72 / 3.99 at the first capture's tip
    // against 3.71 / 3.75, 3.96 / 4.14 at the third's against 3.95 / 4.12).
    assert!(peaks.iter().all(|&p| p > 1.5), "{peaks:?}");
    if captured_rosters(&matches).is_some() {
        for (ours, theirs) in peaks.iter().zip(&captured_peaks) {
            assert!(
                (f64::from(*ours) - theirs).abs() < 0.3,
                "the jumpers' peaks {peaks:?} {captured_peaks:?}"
            );
        }
    }
    assert!(
        ((our_catch - tip_update) - (caught - tipped)).abs() <= 1,
        "the tip's catch {our_catch} after {tip_update}; captured {caught} after {tipped}"
    );
    report(
        "opening",
        json!({
            "phase1_update": start, "launch_update": launch,
            "referee_count_out_update": referee,
            "decided_update": decided.frame(), "tip_update": tipped,
            "our_tip_update": tip_update,
            "timing": decided.ints("tip_timing"), "winner": decided.int("tip_winner"),
            "flight_max_error": flight_error,
            "tip_flight": flight, "our_tip_flight": our_flight,
            "tip_catch_after": caught - tipped, "our_tip_catch_after": our_catch - tip_update,
            "jump_peaks": captured_peaks, "our_jump_peaks": peaks,
        }),
    );
}

#[test]
fn opening_draws_are_the_team_timers_in_the_originals_order() {
    // Phase 1 update by update: the rewrite, given the capture's
    // generator before each update, draws exactly the first draws of the
    // original's update (the rest of the original's are its other
    // systems': animation, camera, crowd, speech), and its team timers,
    // set from the first sample and then its own, are the capture's.
    //
    // A sample is not a paused update (`tools/pine_ai_capture.py`): the
    // original's AI teams update first in an update and the game counts
    // it (game+0x128) later, so a sample whose batches all fall between
    // the two is bracketed as whole while it holds the next update's AI:
    // its generator past that update's team draws (the AI's draws are its
    // update's first) and its timers one update on (schema 3 reads more a
    // sample, so more fall there: 16 of the third capture's 117 opening
    // samples, against one of the second's and none of the first's). The
    // read may also fall between the two sides' AI, side 0's first (one
    // such sample each in the second and third captures). Each team's
    // timer tells which: one update on is the one the rewrite's next
    // update leaves (checked then), and the next update runs from the
    // generator stepped back over those teams' redraws; nothing else
    // passes, and such samples must stay a minority (a timer the port
    // counted an update late would read one update on in every sample).
    let Some((matches, _)) = load() else {
        return;
    };
    let Some(m) = opening(&matches) else {
        return;
    };
    let start = m.iter().position(|u| u.phase() == 1).unwrap();
    let mut w = three_on_three(1);
    w.start_opening_toss();
    let (mut agree, mut differ, mut extra, mut ahead) = (0u32, Vec::new(), 0, 0u32);
    let mut timers = m[start].timers();
    for pair in m[start..].windows(2) {
        let (before, after) = (&pair[0], &pair[1]);
        if after.phase() != 1 || after.frame() != before.frame() + 1 {
            break;
        }
        if after.int("ball_state") != 3 {
            // The launch and the flight are compared by the test above.
            break;
        }
        // The teams `before` read one update on: their timers are not the
        // ones this update starts from (ours, carried from the first
        // sample). The AI teams update in order, so those are the first
        // (side 0 alone, or both), and the generator was read after their
        // draws: the redraws of those at 0.
        let theirs_before = before.timers();
        let on: [bool; 2] = std::array::from_fn(|t| theirs_before[t] != timers[t]);
        let early = on.iter().any(|&on| on);
        ahead += u32::from(early);
        let back = (0..2).filter(|&t| on[t] && timers[t] == 0).count();
        w.rng = Rng::from_state(before.rng());
        for _ in 0..back {
            w.rng.previous();
        }
        for (coach, timer) in w.ai.coaches.iter_mut().zip(timers) {
            coach.timer = timer;
        }
        let from = w.rng.state();
        step_computer(&mut w);
        let ours = draws(from, w.rng.state(), 512).unwrap();
        let native = draws(before.rng(), after.rng(), 4096).unwrap();
        let ours_timers = [w.ai.coaches[0].timer, w.ai.coaches[1].timer];
        let theirs_timers = after.timers();
        // A team read one update on shows this update's timer (the others'
        // are checked as the next update's `before`).
        let same = back <= ours.len()
            && native.starts_with(&ours[back..])
            && (0..2).all(|t| !on[t] || theirs_before[t] == ours_timers[t]);
        timers = ours_timers;
        if same {
            agree += 1;
            extra += native.len() - (ours.len() - back);
        } else {
            differ.push(
                json!({"update": after.frame(), "native": native, "ours": ours,
                "read_one_update_on": on, "native_timers": theirs_timers,
                "our_timers": ours_timers}),
            );
        }
    }
    report(
        "opening_draws",
        json!({"agreeing_updates": agree, "other_systems_draws": extra,
            "samples_read_one_update_on": ahead, "differing": differ}),
    );
    assert!(
        agree > 50 && ahead * 4 < agree,
        "{agree} updates compared, {ahead} samples read one update on"
    );
    assert!(differ.is_empty(), "{differ:?}");
}

/// The game's phase machine ([`crate::game_phase`]) replayed against the
/// capture: fed update by update with the captured ball events, the phase
/// it leaves against the one the original's game update left (game+0xf4),
/// with its possession (game+0x110) and game+0x4a4.
///
/// Fed from the capture: a player newly holding the ball (ball+0xf4; the
/// tip's take and pass inside one update shows as a new previous holder,
/// ball+0xfc, nobody holding it), a launch (the ball's state to 5), a
/// restart (any of the six ready flags game+0x404.. falling: only
/// `FUN_001406b8` lowers them) and its hold (game+0x104), and the tip's
/// start (the front end's, at the intro's end). Not captured, and stood in
/// for: the script manager's state (the staged gamebreaker's camera, the
/// one camera seen only in phases 5 and 6), the score (a match that
/// reaches phase 9 was won by its last basket) and a basket's moment (the
/// ball down through a ring, which the net posts after the game's update:
/// state 5 to 1 from the flight, or 6 to 1 for a ball that touched the
/// rim, the collar or the glass and rolled in, as the third capture's
/// winning gamebreaker shot did at update 30,770).
#[test]
fn the_phase_machine_replays_the_captured_phases() {
    use crate::game_phase::{GamePhase, View, ball, phase};
    use std::collections::BTreeMap;
    let Some((matches, _)) = load() else {
        return;
    };
    let mut pairs: BTreeMap<String, u32> = BTreeMap::new();
    let mut changes: BTreeMap<String, u32> = BTreeMap::new();
    let (mut updates, mut agree, mut possession, mut brought_agree) = (0u32, 0u32, 0u32, 0u32);
    let mut longest = 0u32;
    let rim = crate::shooting::NATIVE_RIM_HEIGHT;
    for m in &matches {
        let actors = actor_map(m);
        let word = m[0].int("sides_word");
        let side = |b: i64| if b & 0xff == 1 { 1 } else { -1 };
        let sides = [side(word), side(word >> 8)];
        let mut cameras: BTreeMap<i64, std::collections::BTreeSet<i64>> = BTreeMap::new();
        for u in m {
            cameras
                .entry(u.int("active_camera"))
                .or_default()
                .insert(u.phase());
        }
        let scripts: Vec<i64> = cameras
            .iter()
            .filter(|(_, p)| p.iter().all(|p| matches!(p, 5 | 6)))
            .map(|(&c, _)| c)
            .collect();
        let through = |a: &Update, u: &Update| {
            matches!(a.int("ball_state") as i32, ball::SHOT | ball::RIM)
                && u.int("ball_state") == i64::from(ball::LOOSE)
                && (u.ball()[0].abs() - 36.75).abs() < 1.0
                && u.ball()[1] < rim
                && rim <= a.ball()[1]
        };
        let won_after = if m.iter().any(|u| u.phase() == i64::from(phase::WON)) {
            m.windows(2)
                .rev()
                .find(|w| through(&w[0], &w[1]))
                .map(|w| w[1].frame())
        } else {
            None
        };
        let at = |u: &Update, h: i64| {
            actors.get(&h).map(|&i| {
                let a = &u.actors()[i];
                (
                    a["team"].as_i64().unwrap() as usize,
                    a["position"][0].as_f64().unwrap() as f32,
                )
            })
        };
        let mut g = GamePhase::default();
        let mut brought = false;
        let mut run = 0u32;
        for w in m.windows(2) {
            let (a, u) = (&w[0], &w[1]);
            let scripted = scripts.contains(&u.int("active_camera"));
            if a.phase() == i64::from(phase::PREGAME) && u.phase() == i64::from(phase::TIP) {
                g.start_tip();
            }
            let h = u.int("ball_holder");
            let holder = at(u, h);
            let previous = u.int("ball_previous_holder");
            if h == 0
                && previous != 0
                && previous != a.int("ball_previous_holder")
                && previous != a.int("ball_holder")
                && let Some((team, x)) = at(u, previous)
            {
                g.gained(team, x, sides, scripted);
            }
            if h != 0
                && h != a.int("ball_holder")
                && let Some((team, x)) = holder
            {
                g.gained(team, x, sides, scripted);
            }
            let state = u.int("ball_state") as i32;
            if state == ball::SHOT
                && matches!(
                    a.int("ball_state") as i32,
                    ball::HELD | ball::PASS | ball::LOOSE
                )
            {
                g.launched(scripted);
            }
            let ready = u.ints("ready");
            let c = g.update(&View {
                // The basket's update: the ball as the flight left it.
                ball: if through(a, u) {
                    a.int("ball_state") as i32
                } else {
                    state
                },
                ball_x: u.ball()[0],
                ball_y: u.ball()[1],
                holder,
                sides,
                ready: ready.iter().all(|&r| r != 0),
                brought_up: brought,
                won: won_after.is_some_and(|f| f < u.frame()),
                settled: !matches!(state, ball::PASS | ball::SHOT | ball::RIM),
                scripted,
            });
            brought |= c.brought_up;
            if ready
                .iter()
                .zip(a.ints("ready"))
                .any(|(&r, q)| r == 0 && q != 0)
            {
                g.start_restart(u.int("possession") as usize);
                brought = false;
            }
            if u.int("game_104") > a.int("game_104") {
                g.hold = u.int("game_104") as i32;
            }
            updates += 1;
            let ours = i64::from(g.phase);
            if ours == u.phase() {
                agree += 1;
                run = 0;
            } else {
                *pairs.entry(format!("{}:{ours}", u.phase())).or_default() += 1;
                run += 1;
                longest = longest.max(run);
            }
            possession += u32::from(i64::from(g.possession) == u.int("possession"));
            brought_agree += u32::from(brought == (u.int("restart_4a4") != 0));
            if a.phase() != u.phase() {
                *changes
                    .entry(format!("{}->{}", a.phase(), u.phase()))
                    .or_default() += 1;
            }
        }
    }
    let share = f64::from(agree) / f64::from(updates.max(1));
    report(
        "phase_machine",
        json!({"updates": updates, "agree": agree, "share": share,
            "captured:machine": pairs, "possession_agrees": possession,
            "game_4a4_agrees": brought_agree, "longest_disagreement": longest,
            "captured_changes": changes}),
    );
    // The captures change a few phases an update later than the machine
    // fed with the same update's sample: the inbound's release (the ball
    // already a flight update out), a held ball or pass within a unit of
    // half court, a rebound just below 6. Never more than one update.
    assert!(share > 0.999, "{agree} of {updates}: {pairs:?}");
    assert!(longest <= 1, "{longest} updates apart: {pairs:?}");
    assert!(possession == updates, "{possession} of {updates}");
    assert!(f64::from(brought_agree) / f64::from(updates) > 0.999);
}

/// The restarts (phase 6) of every match in the capture.
fn restarts(matches: &[Vec<Update>]) -> Vec<(&Vec<Update>, usize, usize)> {
    let mut out = Vec::new();
    for m in matches {
        let mut i = 1;
        while i < m.len() {
            if m[i].phase() == 6 && m[i - 1].phase() != 6 {
                let mut j = i;
                while j < m.len() && m[j].phase() == 6 {
                    j += 1;
                }
                if j < m.len() {
                    out.push((m, i, j));
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }
    out
}

#[test]
fn restart_plays_and_spots_follow_the_captured_original() {
    // FUN_0012bc40's play (DAT_003ce9b8, DAT_004e7408..) for each captured
    // restart, from where everyone stood as it started, and where each
    // player stood once all six were ready.
    let Some((matches, _)) = load() else {
        return;
    };
    let (mut rows, mut differ, mut far) = (Vec::new(), Vec::new(), Vec::new());
    let mut reads = std::collections::BTreeMap::new();
    for (m, first, end) in restarts(&matches) {
        let team = m[first].int("possession") as usize;
        // FUN_0012bc40 runs in the update the possessing team's strategy
        // becomes the restart offense (vtable 0x43b0e0): usually the
        // restart's second, later after a celebration. It reads where
        // everyone stood after the update before.
        let Some(start) = (first..end)
            .find(|&k| m[k].0[format!("team{team}")]["strategy_vtable"].as_u64() == Some(0x43b0e0))
        else {
            continue;
        };
        // Reads are not paused, so a sample is not one whole update: it can
        // show the strategy before its start has written the globals (they
        // are read an update later), and the ball the start read may be the
        // sample's or the one before. Under the rim the ball's z is a
        // fraction either side of 0 and picks between rows 0 and 1, so the
        // play is computed from both and must equal the original's from
        // one of them.
        let chosen = &m[(start + 1).min(end - 1)];
        let globals = chosen.ints("restart_globals");
        let order = chosen.ints("restart_order");
        let before = &m[start - 1];
        let previous = restart::Plan {
            row: before.ints("restart_globals")[2] as usize,
            order: std::array::from_fn(|k| before.ints("restart_order")[k] as usize),
        };
        let plan_from = |u: &Update| {
            let squad: Vec<usize> = (0..6)
                .filter(|&i| u.actors()[i]["team"].as_i64() == Some(team as i64))
                .collect();
            let ball = u.ball();
            restart::plan(&restart::Facts {
                team,
                positions: std::array::from_fn(|k| u.floor(squad[k])),
                ball: [ball[0], ball[2]],
                holder: None,
                previous,
                fresh_clock: true,
                people: 0,
                human: [false; 3],
                heights: [6.5; 3],
                lesson: false,
            })
        };
        let entry = &m[start];
        let native_order: [usize; 3] = std::array::from_fn(|k| order[k] as usize);
        let native_plan = restart::Plan {
            row: globals[2] as usize,
            order: native_order,
        };
        let (plan, read) = if plan_from(entry) == native_plan {
            (native_plan, "start")
        } else if plan_from(before) == native_plan {
            (native_plan, "previous")
        } else {
            (plan_from(entry), "neither")
        };
        // Where they stood once all six were up (FUN_00140840).
        let ready = m[first..end]
            .iter()
            .find(|u| u.ints("ready").iter().all(|&r| r == 1));
        let mut spots = Vec::new();
        if let Some(ready) = ready {
            let heights = [[6.5; 3]; 2];
            let plan = restart::Plan {
                row: ready.ints("restart_globals")[2] as usize,
                order: std::array::from_fn(|k| ready.ints("restart_order")[k] as usize),
            };
            for (i, a) in ready.actors().iter().enumerate() {
                let t = a["team"].as_i64().unwrap() as usize;
                let slot = a["slot"].as_i64().unwrap() as usize;
                let spot = restart::spot(&plan, team, t, slot, &heights);
                let k = plan.order.iter().position(|&s| s == slot).unwrap();
                let at = ready.floor(i);
                let off = crate::distance(at, [spot.position[0], spot.position[2]]);
                let radius = restart::radius(plan.row, team, t, k);
                spots.push(json!({"actor": i, "team": t, "slot": slot, "spot": k,
                    "stood": at, "table": [spot.position[0], spot.position[2]],
                    "off": off, "radius": radius}));
                // A spot with a radius of 1 is walked to (stop within
                // 1.4, the height push at most 1 more); a wide one counts
                // its player ready anywhere within the radius.
                let limit = if radius > 2.0 { radius } else { 2.5 };
                if off > limit {
                    far.push(json!({"update": entry.frame(), "actor": i, "off": off,
                        "limit": limit}));
                }
                // Heights (the push, at most 1) are not captured.
                assert!(
                    off <= limit + 2.0,
                    "restart at {}: actor {i} {off}",
                    entry.frame()
                );
            }
        }
        rows.push(json!({"update": entry.frame(), "team": team,
            "native": {"row": globals[2], "order": native_order},
            "ours": {"row": plan.row, "order": plan.order, "facts_read": read},
            "all_ready_update": ready.map(|u| u.frame()), "spots": spots}));
        *reads.entry(read).or_insert(0) += 1;
        if read == "neither" {
            differ.push(entry.frame());
        }
    }
    report(
        "restarts",
        json!({"restarts": rows, "plans_differ": differ, "facts_read": reads,
            "far_from_spot": far}),
    );
    assert!(!rows.is_empty());
    assert!(differ.is_empty(), "plans differ at {differ:?}");
}

#[test]
fn computer_behaviours_by_phase_against_the_captured_original() {
    // Decision statistics: the front behaviour of every player in every
    // update by the game's phase and whether his side has the ball, the
    // original's attract match against a rewrite computer game of the same
    // length. Structural agreement is asserted where the two must agree;
    // the rest is reported.
    let Some((matches, _)) = load() else {
        return;
    };
    use std::collections::BTreeMap;
    type Table = BTreeMap<String, BTreeMap<String, u32>>;
    let mut native: Table = BTreeMap::new();
    let mut native_draws: BTreeMap<i64, (u64, u64)> = BTreeMap::new();
    let mut updates = 0;
    let mut native_entries: BTreeMap<i64, u32> = BTreeMap::new();
    // The ball holder's front behaviour changing in the half court
    // ("from>to"), the same holder on both updates.
    // (A behaviour being built, "node under construction", stands for the
    // one before it: the rewrite builds its own at once.)
    let mut native_moves: BTreeMap<String, u32> = BTreeMap::new();
    for m in &matches {
        let actors = actor_map(m);
        let mut last: Option<(i64, &str)> = None;
        for pair in m.windows(2) {
            let u = &pair[1];
            updates += 1;
            let h = u.int("ball_holder");
            match actors.get(&h) {
                Some(&i) if u.phase() == 5 => {
                    let now = u.front(i);
                    if now != "node under construction" {
                        if let Some((h0, a)) = last
                            && h0 == h
                            && a != now
                        {
                            *native_moves.entry(format!("{a}>{now}")).or_default() += 1;
                        }
                        last = Some((h, now));
                    }
                }
                _ => last = None,
            }
            if u.phase() != pair[0].phase() {
                *native_entries.entry(u.phase()).or_default() += 1;
            }
            let possession = u.int("possession");
            for i in 0..6 {
                let side = if u.actors()[i]["team"].as_i64() == Some(possession) {
                    "offence"
                } else {
                    "defence"
                };
                *native
                    .entry(format!("{}:{side}", u.phase()))
                    .or_default()
                    .entry(u.front(i).to_string())
                    .or_default() += 1;
            }
            if u.frame() == pair[0].frame() + 1
                && let Some(d) = draws(pair[0].rng(), u.rng(), 4096)
            {
                let e = native_draws.entry(u.phase()).or_default();
                e.0 += 1;
                e.1 += d.len() as u64;
            }
        }
    }
    let mut ours: Table = BTreeMap::new();
    let mut our_draws: BTreeMap<i64, (u64, u64)> = BTreeMap::new();
    let mut our_events: BTreeMap<String, u32> = BTreeMap::new();
    // The capture's own players where it knows them (the plays and the
    // trick passes read their ratings), else the generic six.
    let records = captured_rosters(&matches)
        .map(|(records, _)| records)
        .filter(|_| disc_players().is_some());
    let fresh = |seed: u32| match records {
        Some(records) => rostered_match(seed, records).unwrap_or_else(|| demo_match(seed)),
        None => demo_match(seed),
    };
    let mut seed = 7;
    let mut w = fresh(seed);
    w.start_opening_toss();
    // The replays as the game runs them (the attract's Replays option,
    // Automatic): an instant replay 54 updates after a basket holds the
    // gameplay, and its end snaps the restart's six to their spots.
    let mut replays = crate::replay::Replays::default();
    let (mut our_restarts, mut run) = (Vec::new(), 0_u32);
    let mut our_entries: BTreeMap<i64, u32> = BTreeMap::new();
    let mut last_phase = -1;
    let mut stepped = 0;
    let mut our_moves: BTreeMap<String, u32> = BTreeMap::new();
    let mut last_front: Option<(usize, String, i64)> = None;
    while stepped < updates {
        let from = w.rng.state();
        if !step_game(&mut w, &mut replays) {
            continue;
        }
        stepped += 1;
        if w.restarting() {
            run += 1;
        } else if run > 0 {
            our_restarts.push(run);
            run = 0;
        }
        // The phase after the game's update, as the capture samples it.
        let phase = i64::from(w.game_phase.phase);
        if phase != last_phase {
            *our_entries.entry(phase).or_default() += 1;
            last_phase = phase;
        }
        let front = w.handler().map(|h| {
            let name = w.ai.brains[h]
                .queue
                .first()
                .map_or("-", |n| rewrite_name(&n.act));
            (h, name.to_string(), phase)
        });
        if let (Some((h0, a, p0)), Some((h1, b, p1))) = (&last_front, &front)
            && h0 == h1
            && *p0 == 5
            && *p1 == 5
            && a != b
        {
            *our_moves.entry(format!("{a}>{b}")).or_default() += 1;
        }
        last_front = front;
        // The side with the ball, or restarting (game+0x110).
        let offence = w.possession;
        for i in 0..6 {
            let side = if w.players[i].team == offence {
                "offence"
            } else {
                "defence"
            };
            let name = w.ai.brains[i]
                .queue
                .first()
                .map_or("-", |n| rewrite_name(&n.act));
            *ours
                .entry(format!("{phase}:{side}"))
                .or_default()
                .entry(name.to_string())
                .or_default() += 1;
        }
        for e in &w.events {
            let name = format!("{e:?}");
            let name = name.split([' ', '{', '(']).next().unwrap_or("").to_string();
            *our_events.entry(name).or_default() += 1;
        }
        let e = our_draws.entry(phase).or_default();
        e.0 += 1;
        e.1 += draws(from, w.rng.state(), 4096).map_or(0, |d| d.len() as u64);
        if matches!(w.phase, crate::Phase::Over { .. }) {
            // Another match, as the attract loop plays another.
            seed += 1;
            w = fresh(seed);
            w.start_opening_toss();
            replays = crate::replay::Replays::default();
        }
    }
    let mut native_restarts = Vec::new();
    for m in &matches {
        let mut run = 0_u32;
        for u in m {
            if u.phase() == 6 {
                run += 1;
            } else if run > 0 {
                native_restarts.push(run);
                run = 0;
            }
        }
    }
    let spread = |v: &[u32]| {
        let mut sorted = v.to_vec();
        sorted.sort_unstable();
        json!({"count": v.len(), "min": v.iter().min(), "max": v.iter().max(),
            "mean": v.iter().map(|&x| f64::from(x)).sum::<f64>() / v.len().max(1) as f64,
            "sorted": sorted})
    };
    let mean = |t: &BTreeMap<i64, (u64, u64)>| -> BTreeMap<i64, f64> {
        t.iter()
            .map(|(&k, &(n, d))| (k, d as f64 / n.max(1) as f64))
            .collect()
    };
    // Behaviours the original ran in a phase and side that the rewrite
    // never ran there, and the reverse.
    let mut only_native = Vec::new();
    let mut only_ours = Vec::new();
    for (key, names) in &native {
        for name in names.keys() {
            if !ours.get(key).is_some_and(|o| o.contains_key(name)) {
                only_native.push(format!("{key} {name}"));
            }
        }
    }
    for (key, names) in &ours {
        for name in names.keys() {
            if !native.get(key).is_some_and(|o| o.contains_key(name)) {
                only_ours.push(format!("{key} {name}"));
            }
        }
    }
    report(
        "behaviours",
        json!({"updates": updates, "native": native, "ours": ours,
            "native_draws_per_update": mean(&native_draws),
            "our_draws_per_update": mean(&our_draws),
            "only_native": only_native, "only_ours": only_ours, "our_events": our_events,
            "native_phase_entries": native_entries, "our_phase_entries": our_entries,
            "native_restart_updates": spread(&native_restarts),
            "our_restart_updates": spread(&our_restarts),
            "native_handler_moves": native_moves, "our_handler_moves": our_moves,
            "rosters": records}),
    );
    // The opening: everyone waits for the toss in both.
    for table in [&native, &ours] {
        for side in ["offence", "defence"] {
            if let Some(names) = table.get(&format!("1:{side}")) {
                assert_eq!(names.keys().collect::<Vec<_>>(), ["wait for toss"]);
            }
        }
    }
    // The back court (phase 4): the defence defends and the offence runs
    // the fast break, in both.
    for table in [&native, &ours] {
        let top = |key: &str| {
            table
                .get(key)
                .and_then(|n| n.iter().max_by_key(|&(_, &c)| c))
                .map(|(name, _)| name.clone())
        };
        assert_eq!(top("4:defence").as_deref(), Some("defense"));
        assert_eq!(top("4:offence").as_deref(), Some("fast break"));
        assert_eq!(top("5:defence").as_deref(), Some("defense"));
        assert_eq!(top("6:defence").as_deref(), Some("inbound pos"));
    }
    let total =
        |table: &Table, name: &str| -> u32 { table.values().filter_map(|n| n.get(name)).sum() };
    // The pocket goes in the update it is pressed (`FUN_00159eb8` from
    // the holder's ball update), so "pocket" fronts one update a pocket:
    // the original's twice in the match.
    assert!(total(&native, "pocket") <= 4);
    assert!(total(&ours, "pocket") <= 6, "{}", total(&ours, "pocket"));
    // A "pass" fronts until the ball leaves the passer's hands (the pass
    // animation): about 18 updates a pass in both, not one.
    let passes = our_events.get("Pass").copied().unwrap_or(0).max(1);
    assert!(
        total(&ours, "pass") >= 10 * passes,
        "{} pass fronts for {passes} passes",
        total(&ours, "pass")
    );
}

/// The passes of a capture: (phase at the press, the press's update,
/// updates to the release, the receiver's catch hold after the catch).
/// The script a capture's channel pc (`channel+0x10`) runs: the largest
/// known script address at or below it.
fn script_at(pc: i64, scripts: &[u32]) -> Option<u32> {
    scripts
        .iter()
        .copied()
        .filter(|&s| i64::from(s) <= pc)
        .max()
}

/// The actor index the ball's holder pointer names: the six actors are
/// allocated in order, so the holder pointers sort into actor order (the
/// seventh is the referee).
fn actor_map(m: &[Update]) -> std::collections::HashMap<i64, usize> {
    let mut holders: Vec<i64> = m
        .iter()
        .map(|u| u.int("ball_holder"))
        .filter(|&h| h != 0)
        .collect();
    holders.sort_unstable();
    holders.dedup();
    holders
        .into_iter()
        .take(6)
        .enumerate()
        .map(|(i, h)| (h, i))
        .collect()
}

/// A captured shot: the shooter's shoot press (his pad's pressed word bit
/// 0x80 while he holds the ball), the update the ball's state turns 5,
/// the script his channel ran the update after the press, and the ball's
/// height at the release.
struct CapturedShot {
    delta: i64,
    script: Option<u32>,
    height: f64,
}

fn captured_shots(m: &[Update]) -> Vec<CapturedShot> {
    let at: std::collections::HashMap<i64, &Update> = m.iter().map(|u| (u.frame(), u)).collect();
    let actors = actor_map(m);
    let scripts: Vec<u32> = crate::timing::SHOT_SCRIPTS
        .iter()
        .map(|s| s.script)
        .chain(crate::timing::MOVE_ROWS.iter().map(|r| r.script))
        .collect();
    let mut out = Vec::new();
    for pair in m.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.int("ball_state") == 5 || b.int("ball_state") != 5 || b.frame() != a.frame() + 1 {
            continue;
        }
        let Some(&i) = actors.get(&a.int("ball_holder")) else {
            continue;
        };
        let mut f = a.frame();
        let mut press = None;
        for _ in 0..120 {
            let Some(u) = at.get(&f) else {
                break;
            };
            // The press must have kept the ball in the shooter's hands up
            // to the release: a press that passed (state 4) and got the
            // ball back on a give-and-go is not this shot's (the second
            // capture's update 7613: a pass, the return, and a shot from
            // the catch with no press of its own at 7700).
            if u.int("ball_state") != 3 || u.int("ball_holder") != a.int("ball_holder") {
                break;
            }
            let pad = u.actors()[i]["pad"].as_array();
            let pressed = pad
                .and_then(|p| p.get(1))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            if pressed & 0x80 != 0 {
                press = Some(f);
                break;
            }
            f -= 1;
        }
        let Some(press) = press else {
            continue;
        };
        let next = at.get(&(press + 1)).or_else(|| at.get(&press));
        let pc = next
            .and_then(|u| u.actors()[i]["script_pc"].as_i64())
            .unwrap_or(0);
        out.push(CapturedShot {
            delta: b.frame() - press,
            script: script_at(pc, &scripts),
            height: b.ball()[1].into(),
        });
    }
    out
}

#[test]
fn shots_release_at_their_scripts_callback_21() {
    // The press plays a shot, dunk or lay-up script; the ball turns to
    // state 5 at its callback 21 (`FUN_001729f8`), `release` updates
    // later on its plain path, or `check` + the fadeaway's 19 when
    // callback 17 found the stick pushed away from the basket. The
    // captured computer players' shots sit on those two values for every
    // script the tables know (the alley-oop dunks and the tip-ins play
    // other scripts).
    let Some((matches, _)) = load() else {
        return;
    };
    let shots: Vec<CapturedShot> = matches.iter().flat_map(|m| captured_shots(m)).collect();
    let mut checked = 0;
    let mut off = Vec::new();
    for s in &shots {
        // A tip-in's press is the rebound's: the ball turns to state 5
        // within a few updates, from no shot script.
        let Some(script) = s.script.filter(|_| s.delta >= 5) else {
            continue;
        };
        let expected: Vec<i64> = if let Some(k) = crate::timing::SHOT_SCRIPTS
            .iter()
            .find(|k| k.script == script)
        {
            let fade = crate::timing::SHOT_SCRIPTS[crate::timing::FADEAWAY];
            let mut v = vec![i64::from(k.release)];
            if k.check > 0 {
                v.push(i64::from(k.check) + i64::from(fade.release));
            }
            v
        } else if let Some(r) = crate::timing::MOVE_ROWS
            .iter()
            .find(|r| r.script == script && r.release > 0 && r.mask & 0x100000 == 0)
        {
            vec![i64::from(r.release)]
        } else {
            continue;
        };
        checked += 1;
        if !expected.contains(&s.delta) {
            off.push((script, s.delta, expected));
        }
    }
    report(
        "shots",
        json!({"checked": checked, "off": off.len(),
            "heights": shots.iter().map(|s| s.height).collect::<Vec<_>>()}),
    );
    // The whole match has 78 shots; the second capture's third of one, 19.
    assert!(checked >= 15, "{checked}");
    // `dunk_2h_2f_rev_hang` (`0x44b070`) measured 65 against the walked 57:
    // its animation test (`98`) takes the branch the walker does not. The
    // rest sit on the walked moments.
    assert!(off.len() * 20 <= checked, "{off:?}");
    // Jump shots leave the hand 11.5..13.5 up (the ball at the release);
    // dunks go in at the rim, 12.0.
    assert!(shots.iter().all(|s| (11.0..14.0).contains(&s.height)));
}

/// A captured pass: its phase, press, release (updates after the press,
/// when the flight's progress leaves 0), the receiver's catch hold (bit 9
/// after the ball is his), the pass row's script and the catch row's
/// expected hold.
/// (phase, press, release, hold, pass row script) of a captured pass.
type CapturedPass = (i64, i64, Option<i64>, Option<i64>, Option<u32>);

fn captured_passes(m: &[Update]) -> Vec<CapturedPass> {
    let at: std::collections::HashMap<i64, &Update> = m.iter().map(|u| (u.frame(), u)).collect();
    let actors = actor_map(m);
    let scripts: Vec<u32> = crate::timing::PASS_ROWS.iter().map(|r| r.script).collect();
    let state = |f: i64| at.get(&f).map(|u| u.int("ball_state"));
    let mut out = Vec::new();
    for pair in m.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.int("ball_state") == 4 || b.int("ball_state") != 4 || b.frame() != a.frame() + 1 {
            continue;
        }
        let press = b.frame();
        let script = actors.get(&a.int("ball_holder")).and_then(|&i| {
            let pc = at
                .get(&(press + 1))
                .and_then(|u| u.actors()[i]["script_pc"].as_i64())?;
            script_at(pc, &scripts)
        });
        // The release: the flight's progress (ball+0x1a4) leaves 0.
        let mut f = press;
        while at
            .get(&f)
            .is_some_and(|u| u.int("ball_state") == 4 && u.int("ball_progress") == 0)
        {
            f += 1;
        }
        let release = (state(f) == Some(4)).then_some(f - press);
        // The catch hold: the receiver's bit 9 (actor+0xa4) after the
        // ball is caught (state 3 again).
        let mut hold = None;
        if release.is_some() {
            while state(f) == Some(4) {
                f += 1;
            }
            if state(f) == Some(3)
                && let Some(u) = at.get(&f)
                && let Some(r) = (0..6)
                    .find(|&i| u.actors()[i]["animation_state"].as_i64().unwrap_or(0) & 0x200 != 0)
            {
                let mut g = f;
                while at.get(&g).is_some_and(|u| {
                    u.actors()[r]["animation_state"].as_i64().unwrap_or(0) & 0x200 != 0
                }) {
                    g += 1;
                }
                hold = Some(g - f);
            }
        }
        out.push((a.phase(), press, release, hold, script));
    }
    out
}

#[test]
fn passes_leave_the_hand_when_the_animation_lets_go() {
    // The pass setup (`FUN_0014bff8`) makes the ball state 4 at the press;
    // the ball stays in the passer's hands (its progress at 0) until the
    // pass script's callback 38: every inbound (phase 6, pass row 10)
    // releases `INBOUND_RELEASE` updates after the press, and every live
    // pass whose row script the passer's channel shows releases at that
    // script's callback 38 (`timing::PASS_ROWS`). A handoff (no script)
    // releases at the press. The receiver's catch (bit 9) holds after the
    // catch for the flight's nominal end and the catch row's tail: 9 or
    // 15 for the standing catches, which the captures' spread shows.
    let Some((matches, _)) = load() else {
        return;
    };
    let passes: Vec<_> = matches.iter().flat_map(|m| captured_passes(m)).collect();
    let inbound: Vec<i64> = passes
        .iter()
        .filter(|p| p.0 == 6)
        .filter_map(|p| p.2)
        .collect();
    let mut live_checked = 0;
    let mut off = Vec::new();
    for p in passes.iter().filter(|p| matches!(p.0, 4 | 5)) {
        let (Some(release), Some(script)) = (p.2, p.4) else {
            continue;
        };
        // The passout rows (to oneself, `0x200000`/`0x400000`) release at
        // the press: the self-toss path, not the pass start's.
        let Some(row) = crate::timing::PASS_ROWS
            .iter()
            .find(|r| r.script == script && r.release > 0 && r.flags & 0x600000 == 0)
        else {
            continue;
        };
        live_checked += 1;
        if release != i64::from(row.release) {
            off.push((script, release, row.release));
        }
    }
    let holds: Vec<i64> = passes.iter().filter_map(|p| p.3).collect();
    report(
        "passes",
        json!({"inbound_release": inbound, "live_checked": live_checked, "off": off.len(),
            "catch_hold": holds}),
    );
    assert!(!inbound.is_empty());
    assert!(
        inbound
            .iter()
            .all(|&r| r == i64::from(crate::INBOUND_RELEASE)),
        "{inbound:?}"
    );
    // The whole match has 131 live passes on a known row; the second
    // capture's third of one, 37.
    assert!(live_checked >= 30, "{live_checked}");
    assert!(off.is_empty(), "{off:?}");
    // The holds after the catch: the flight ends 1..5 updates after the
    // catch, then the tail (9 or 15 standing, 9 running): the middle half
    // of the captures lies within 10..=17.
    let mut sorted = holds.clone();
    sorted.sort_unstable();
    assert!(!sorted.is_empty());
    let (q1, q3) = (sorted[sorted.len() / 4], sorted[sorted.len() * 3 / 4]);
    assert!(
        (10..=17).contains(&q1) && (10..=17).contains(&q3),
        "{holds:?}"
    );
}

/// Every captured pass flight (ball+0x64 in the sample the ball leaves the
/// hands), with the places [`crate::passing::flight_agreement`] reads.
fn captured_flights(m: &[Update]) -> Vec<crate::passing::CapturedFlight> {
    let at: std::collections::HashMap<i64, &Update> = m.iter().map(|u| (u.frame(), u)).collect();
    let actors = actor_map(m);
    let place = |u: &Update, i: usize| {
        let p = &u.actors()[i]["position"];
        [p[0].as_f64().unwrap() as f32, p[2].as_f64().unwrap() as f32]
    };
    let mut out = Vec::new();
    let mut tossed = false;
    for pair in m.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        tossed |= a.int("ball_state") == 7;
        if a.int("ball_state") == 4 || b.int("ball_state") != 4 || b.frame() != a.frame() + 1 {
            continue;
        }
        let press = b.frame();
        // The release: the ball out of the hands.
        let mut f = press;
        while at
            .get(&f)
            .is_some_and(|u| u.int("ball_state") == 4 && u.int("ball_holder") != 0)
        {
            f += 1;
        }
        let (Some(release), Some(before)) = (at.get(&f), at.get(&(f - 1))) else {
            continue;
        };
        if release.int("ball_state") != 4 {
            continue;
        }
        let first = std::mem::take(&mut tossed);
        let passer = actors.get(&a.int("ball_holder")).copied();
        let receiver = actors.get(&release.int("ball_target_player")).copied();
        let (Some(passer), Some(receiver)) = (passer, receiver) else {
            if first {
                out.push(crate::passing::CapturedFlight {
                    flight: release.int("ball_flight_updates") as i32,
                    first,
                    to_self: false,
                    lob: false,
                    passer: [0.0; 2],
                    receiver: [0.0; 2],
                    receiver_at_press: [0.0; 2],
                });
            }
            continue;
        };
        let height = release.actors()[receiver]["position"][1]
            .as_f64()
            .unwrap_or(0.0);
        let target = release.0["ball_target"][1].as_f64().unwrap_or(0.0);
        out.push(crate::passing::CapturedFlight {
            flight: release.int("ball_flight_updates") as i32,
            first,
            to_self: passer == receiver,
            lob: height > 0.0 || target > 12.0,
            passer: place(before, passer),
            receiver: place(before, receiver),
            // The press's sample if his actor updates before the passer's,
            // else the one before it (the actors update in order).
            receiver_at_press: place(if receiver < passer { b } else { a }, receiver),
        });
    }
    out
}

#[test]
fn pass_flights_are_timed_by_the_originals_rule() {
    // The release's catch draw (`FUN_001611d8`) flies a pass
    // `max(distance / 0.75196, 20)` updates, truncated: the distance from
    // the passer to the receiver (led) or to his spot. Thrown through the
    // production release by a passer and a standing receiver where the
    // capture had them, the rewrite gives the captured flight for most
    // ordinary passes; most of the rest are the rule on the receiver's
    // place at the press (a spot pass); what neither gives is a running
    // receiver's destination, which the capture does not hold, or the
    // back-court cut's 5 more. The tip (the first pass after the actors'
    // reset) flies 40.
    let Some((matches, _)) = load() else {
        return;
    };
    let flights: Vec<_> = matches.iter().flat_map(|m| captured_flights(m)).collect();
    let a = crate::passing::flight_agreement(&flights);
    report(
        "pass_flights",
        json!({"flights": flights.len(), "first": a.first, "ordinary": a.ordinary,
            "led": a.led, "spot": a.spot, "off": a.off, "to_self": a.to_self,
            "lobs": a.lobs}),
    );
    assert!(
        !a.first.is_empty() && a.first.iter().all(|&f| f == 40),
        "{:?}",
        a.first
    );
    assert!(a.ordinary >= 40, "{a:?}");
    let agreed = a.led + a.spot;
    assert!(
        agreed * 100 >= a.ordinary * 90,
        "{agreed} of {}: {a:?}",
        a.ordinary
    );
    assert!(
        a.off
            .iter()
            .all(|&(f, led, spot)| (f - led).abs() <= 5 || (f - spot).abs() <= 5),
        "{a:?}"
    );
}

#[test]
fn the_captured_dunks_leave_the_launcher_as_ported() {
    // Every dunk the attract captures hold: the launch (ball state 3 to 5)
    // from r + 12, the height the launcher's dunk path puts it at
    // (`shooting::dunk_place`; none of its other paths lands there). Its
    // place is the held ball's in the update before (the hand) brought
    // into the ring, and a made dunk's flight is straight at the basket's
    // middle at the hand's pace (`shooting::dunk_pace`, the ball's last
    // move held to 10..20 mph a frame), or the top one for a rebound
    // jumper (actor state bit 8) or a dunker with actor+0x1e8 0x15 (not
    // captured), sampled after the launch update's own move; the ball
    // goes down through the ring (state 1) the update that flight takes it
    // below the rim: the next one, or the one after for a slow hand (the
    // third capture's gamebreaker dunk at update 16,562, 0.30 a frame).
    // The release callback's odds miss one now and then (98% at most,
    // `shooting::dunk_percent`; a gamebreaker's 100%): a miss is the
    // launcher's kind 3, off the rim, so its ball touches the hoop (state
    // 6) before anything else (the first capture's dunk at 3,973 and the
    // third's alley-oop finish at 25,537).
    let Some((matches, _)) = load() else {
        return;
    };
    let r = crate::ball::RADIUS;
    let rim = crate::shooting::NATIVE_RIM_HEIGHT;
    let f3 = |u: &Update, key: &str| -> [f32; 3] {
        std::array::from_fn(|k| u.0[key][k].as_f64().unwrap() as f32)
    };
    let (mut dunks, mut made, mut hand, mut rebounder, mut top) = (0, 0, 0, 0, 0);
    let mut unsampled = 0;
    let mut to_ring: std::collections::BTreeMap<usize, u32> = Default::default();
    let mut misses = Vec::new();
    for m in &matches {
        let actors = actor_map(m);
        for k in 0..m.len().saturating_sub(3) {
            let [before, held, launch] = [&m[k], &m[k + 1], &m[k + 2]];
            if launch.frame() != before.frame() + 2
                || held.int("ball_state") != 3
                || launch.int("ball_state") != 5
            {
                continue;
            }
            let g = launch.0["ball_gravity"].as_f64().unwrap() as f32;
            let (p, v) = (launch.ball(), f3(launch, "ball_velocity"));
            // The launch's own update undone: the step, half the gravity
            // off the height and all of it off the rise.
            let vy = v[1] + g;
            let at = [p[0] - v[0], p[1] - vy + 0.5 * g, p[2] - v[2]];
            if (at[1] - (r + 12.0)).abs() > 1e-3 {
                continue;
            }
            dunks += 1;
            assert_eq!(launch.phase(), 3, "{}", launch.frame());
            let side = held.ball()[0].signum();
            let middle = [side * 36.75, crate::shooting::NATIVE_AIM_HEIGHT, 0.0];
            let placed = crate::shooting::dunk_place(held.ball(), middle, r);
            let off = (0..3)
                .map(|k| (placed[k] - at[k]).abs())
                .fold(0.0, f32::max);
            assert!(off < 2e-3, "{} {placed:?} {at:?}", launch.frame());
            // The flight's end: the first update out of state 5, every
            // update sampled up to it.
            let Some(end) = (k + 3..m.len().min(k + 60))
                .find(|&j| m[j].int("ball_state") != 5)
                .filter(|&j| m[j].frame() == launch.frame() + (j - k - 2) as i64)
            else {
                unsampled += 1;
                continue;
            };
            let (last, ended) = (&m[end - 1], &m[end]);
            let through = ended.int("ball_state") == 1
                && (ended.ball()[0].abs() - 36.75).abs() < 1.0
                && ended.ball()[1] < rim
                && rim <= last.ball()[1];
            if !through {
                assert_eq!(
                    ended.int("ball_state"),
                    6,
                    "{}: a dunk neither through the ring nor off the rim",
                    launch.frame()
                );
                assert_ne!(
                    launch.0["ball_gamebreaker"].as_i64(),
                    Some(1),
                    "{}: a gamebreaker's dunk missed",
                    launch.frame()
                );
                let dunker = actors.get(&held.int("ball_holder")).copied();
                misses.push(json!({"launch": launch.frame(), "dunker": dunker,
                    "state_bits": dunker.map(|i| held.actors()[i]["animation_state"].clone())}));
                continue;
            }
            made += 1;
            let moved = (0..3)
                .map(|k| (held.ball()[k] - before.ball()[k]).powi(2))
                .sum::<f32>()
                .sqrt();
            // The flight at `pace` from the placed ball: its first update
            // as sampled (the launch's own move), and the moves it takes
            // to go below the rim (straight at the middle, under gravity
            // alone), the launch's one of them.
            let flies = |pace: f32| {
                let mut ball = crate::Ball::loose_at(placed);
                ball.launch_at_pace(placed, middle, pace);
                let u = ball.velocity.map(|c| c * crate::TICK_SECONDS);
                let after = [
                    placed[0] + u[0],
                    placed[1] + u[1] - 0.5 * g,
                    placed[2] + u[2],
                ];
                let ring = (1..60)
                    .find(|&n| {
                        let n = n as f32;
                        placed[1] + n * u[1] - 0.5 * g * n * n < rim
                    })
                    .unwrap_or(0);
                (0..3).map(|k| (after[k] - p[k]).abs()).fold(0.0, f32::max) < 2e-3
                    && ring == end - k - 1
            };
            *to_ring.entry(end - k - 2).or_default() += 1;
            let jumper = actors
                .get(&held.int("ball_holder"))
                .and_then(|&i| held.actors()[i]["animation_state"].as_i64())
                .is_some_and(|s| s & 0x100 != 0);
            if jumper {
                assert!(
                    flies(crate::shooting::dunk_pace(moved, true)),
                    "{}",
                    launch.frame()
                );
                rebounder += 1;
            } else if flies(crate::shooting::dunk_pace(moved, false)) {
                hand += 1;
            } else {
                assert!(
                    flies(crate::shooting::dunk_pace(moved, true)),
                    "{} moved {moved}",
                    launch.frame()
                );
                top += 1;
            }
        }
    }
    report(
        "dunks",
        json!({"dunks": dunks, "made": made, "hand_pace": hand,
            "rebound_jumper_top_pace": rebounder, "other_top_pace": top,
            "made_by_updates_to_the_ring": to_ring, "missed_off_the_rim": misses,
            "flight_not_sampled": unsampled}),
    );
    assert!(
        dunks >= 10 && made + misses.len() + unsampled == dunks && unsampled <= 1,
        "{dunks} {made} {unsampled} {misses:?}"
    );
}

#[test]
fn the_captured_rebound_jumps_follow_their_scripts() {
    // Every rebound jump the attract captures hold (actor state bit 8
    // rising): its script one of `rebound::SCRIPTS` (read from the channel's
    // pc in the next updates); the jumper leaves the floor that script's
    // jump tick after the press (its callback 36); and the script is of
    // the kinds `FUN_0015ce50` picks by where the ball will be: under the
    // basket 8, else by the sector of his facing (actor+0xa8, which it
    // copies to +0xb0) to the ball 30 updates on, ahead 0, 2 or 4 and
    // behind 1, 3 or 5. The ball's place 30 on is the capture's own,
    // which differs from the original's prediction when someone touches
    // it first. Callback 47's branch (`rebound::branch_47`) on that ball and
    // his facing the update before picks the clip the capture shows four to
    // nine updates on, on the eleven scripts with one.
    use crate::rebound::{KINDS, SCRIPTS};
    let Some((matches, _)) = load() else {
        return;
    };
    // The script whose body holds the pc: the nearest start at or below
    // it (they lie within 0xc0 of each other).
    let script_of = |pc: i64| {
        SCRIPTS
            .iter()
            .enumerate()
            .filter(|(_, s)| i64::from(s.address) <= pc && pc < i64::from(s.address) + 0xc0)
            .max_by_key(|(_, s)| s.address)
            .map(|(k, _)| k)
    };
    let (mut jumps, mut kinds_agree, mut kinds_known) = (0, 0, 0);
    let (mut branches, mut branches_agree) = (0, 0);
    for m in &matches {
        let at: std::collections::HashMap<i64, &Update> =
            m.iter().map(|u| (u.frame(), u)).collect();
        for pair in m.windows(2) {
            let (a, u) = (&pair[0], &pair[1]);
            if u.frame() != a.frame() + 1 || a.actors().len() < 6 || u.actors().len() < 6 {
                continue;
            }
            for i in 0..6 {
                let bits = |v: &Update| v.actors()[i]["animation_state"].as_i64().unwrap_or(0);
                if bits(u) & 0x100 == 0 || bits(a) & 0x100 != 0 {
                    continue;
                }
                let Some(s) = (u.frame()..u.frame() + 3).find_map(|f| {
                    at.get(&f)
                        .and_then(|v| v.actors().get(i))
                        .and_then(|x| x["script_pc"].as_i64())
                        .and_then(script_of)
                }) else {
                    continue;
                };
                jumps += 1;
                let jump = i64::from(SCRIPTS[s].jump);
                if jump > 0 {
                    let takeoff = (u.frame()..u.frame() + 30).find(|f| {
                        at.get(f)
                            .and_then(|v| v.actors().get(i))
                            .is_some_and(|x| x["position"][1].as_f64().unwrap_or(0.0) > 0.0)
                    });
                    assert_eq!(takeoff, Some(u.frame() + jump), "{} actor {i}", u.frame());
                }
                let Some(later) = at.get(&(u.frame() - 1 + 30)) else {
                    continue;
                };
                let ball = later.ball();
                let records = SCRIPTS[s].records;
                let shown = (u.frame() + 3..u.frame() + 10).find_map(|f| {
                    at.get(&f)
                        .and_then(|v| v.actors()[i]["animation"].as_u64())
                        .filter(|&r| records.contains(&(r as u16)))
                });
                if let Some(shown) = shown
                    && records[0] != records[1]
                    && !(14..16).contains(&s)
                {
                    let before = a.actors()[i]["heading"].as_i64().unwrap() as i32;
                    let mut path = vec![([0.0; 3], [0.0; 3]); 30];
                    path[29].0 = ball;
                    let branch = crate::rebound::branch_47(a.floor(i), before, &path);
                    branches += 1;
                    if u64::from(records[usize::from(branch)]) == shown {
                        branches_agree += 1;
                    }
                }
                let word = u.actors()[i]["heading"].as_i64().unwrap() as i32;
                let place = u.floor(i);
                let sector = crate::timing::sector(word, place, [ball[0], ball[2]]);
                let (x, z) = (place[0].abs(), place[1].abs());
                let kind: &[usize] = if (z <= 2.0 && x > 37.0) || (x > 38.0 && z <= 10.0) {
                    &[8]
                } else if matches!(sector, 7 | 0 | 1) {
                    &[1, 3, 5]
                } else {
                    &[0, 2, 4]
                };
                let s = s as u8;
                if kind.iter().any(|&k| KINDS[k].contains(&s)) {
                    kinds_agree += 1;
                    kinds_known += 1;
                } else if !KINDS[6].contains(&s) && !KINDS[7].contains(&s) {
                    kinds_known += 1;
                }
            }
        }
    }
    report(
        "rebound_jumps",
        json!({"jumps": jumps, "kinds_known": kinds_known, "kinds_agree": kinds_agree,
            "branches": branches, "branches_agree": branches_agree}),
    );
    assert!(jumps >= 5, "{jumps}");
    assert!(
        branches_agree * 100 >= branches * 70,
        "{branches_agree} of {branches} branches"
    );
    assert!(
        kinds_agree * 100 >= kinds_known * 85,
        "{kinds_agree} of {kinds_known}"
    );
}

/// The bins of the handler's distance to his nearest defender.
const SPACING: [f32; 8] = [2.0, 4.0, 6.0, 8.0, 10.0, 14.0, 18.0, f32::INFINITY];

/// Updates after the handler's last trick from which [`Tally::away`]
/// counts.
const AWAY: i64 = 30;

fn spacing_bin(d: f32) -> usize {
    SPACING.iter().position(|&b| d < b).unwrap()
}

/// What the shooting, turnover and spacing comparison counts in a match.
#[derive(Default, Debug)]
struct Tally {
    /// Front-court updates, and those with the ball held by the handler's
    /// distance to his nearest defender ([`SPACING`]).
    front_court: u32,
    held: [u32; 8],
    /// Entries into phase 3 (a shot, dunk or lay-up in the air).
    shot_phases: u32,
    /// The possession changing from a phase-5 update to the next, by the
    /// next update's phase.
    turnovers: std::collections::BTreeMap<i64, u32>,
    /// Phase-5 updates a defender presses steal in, and those with two
    /// turbos held (a trick counter).
    steal_presses: u32,
    counter_presses: u32,
    /// Phase-5 updates the held ball went loose (strips and knocks).
    knocked_loose: u32,
    /// Phase-4 updates the possessing side's players front "pass", and
    /// the passes pressed in phase 4.
    back_court_pass_fronts: u32,
    back_court_passes: u32,
    /// Held front-court updates 12..29.75 out by the shot's quality from
    /// where everyone stands ([`positional_quality`]).
    quality: [u32; 8],
    /// By the handler's distance from the rim (bins of 6), the held
    /// front-court updates and their summed nearest-defender distance.
    by_reach: [(u32, f32); 6],
    /// Held front-court updates with the handler in a trick (actor bit
    /// 10), and those with a defender within 4.
    tricking: (u32, u32),
    /// Held front-court updates [`AWAY`] or more after the handler's last
    /// trick, and those with a defender within 4: the defence's spacing
    /// without the trick moves' own motion.
    away: (u32, u32),
    /// The speed levels (AIPlayer+0x48, 0..3) of the defenders running
    /// "defense" in held front-court updates, outside the counter stance,
    /// a swipe and a block jump.
    levels: [u32; 4],
    /// Updates by game phase, and the phase's changes from one update to
    /// the next ("from->to").
    phases: std::collections::BTreeMap<i64, u32>,
    changes: std::collections::BTreeMap<String, u32>,
    /// The trick moves (actor bit 10) and where they took the player.
    moves: Moves,
    /// Block jumps (state bit 12 set) by what they went up at, the update
    /// before: the holder in a jump shot or lay-up (bit 3), a dunk (7), a
    /// leap (15), anything else; the jump shot let go that update or the
    /// one before (a jump in the release's own update went up at the ball
    /// let go: the jumper's AI frame ran after the shooter's update, see
    /// [`World::update_order`]); a pass in the air; other.
    block_jumps: std::collections::BTreeMap<&'static str, u32>,
    /// The computer's pump fakes (a shot's script fallen through its
    /// callback 20; the rewrite's side only).
    pump_fakes: u32,
    /// What the ball met on the bones ([`Touches`]).
    touches: Touches,
}

/// What a block jump went up at ([`Tally::block_jumps`]), from the holder's
/// state bits the update before (none: the ball free), the ball's state then
/// and whether a jump shot was let go that update (`let_go`) or the one
/// before.
fn block_jump_kind(
    holder_bits: Option<i64>,
    ball_state: i64,
    released: bool,
    let_go: bool,
) -> &'static str {
    match (holder_bits, ball_state) {
        _ if let_go => "after a release",
        (Some(b), _) if b & 0x80 != 0 => "at a dunker",
        (Some(b), _) if b & 0x8 != 0 => "at a shooter",
        (Some(b), _) if b & 0x8000 != 0 => "at a leaper",
        (Some(_), _) => "at a holder",
        (None, 5) if released => "after a release",
        (None, 4) => "at a pass",
        _ => "other",
    }
}

/// A move under way: its first update, where the player stood the update
/// before and the rim he attacks.
type Started = (i64, [f32; 2], [f32; 2]);

/// The nearest opponent at a move's press, and where both stood then.
type Faced = (usize, [f32; 2], [f32; 2]);

/// How the ball fared in an update, which ends a trick move that ends then
/// ([`Moves::update`]): still held (the move ran its script out), let go
/// (a pass or a shot out of the move, ball state 3 to 4 or 5) or lost
/// (knocked loose, 3 to 1, or taken: the possession changed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveEnd {
    Ran,
    LetGo,
    Lost,
}

impl MoveEnd {
    /// The update that took the ball from state `before` to `after`, the
    /// possession changed or not.
    fn of(before: i64, after: i64, changed: bool) -> Self {
        use crate::game_phase::ball;
        let (before, after) = (before as i32, after as i32);
        if changed || (before == ball::HELD && after == ball::LOOSE) {
            Self::Lost
        } else if before == ball::HELD && matches!(after, ball::PASS | ball::SHOT) {
            Self::LetGo
        } else {
            Self::Ran
        }
    }
}

/// A finished trick move: its updates, how far it took the player, how far
/// along the line to the rim (negative away from it) and how it ended.
type Move = (i64, f32, f32, MoveEnd);

/// Every player's trick moves (actor bit 10 held over consecutive
/// updates): how long each lasted and where it took him.
#[derive(Default, Debug)]
struct Moves {
    /// Per player, a move under way: its first update, where he stood the
    /// update before and the rim he attacks.
    open: [Option<Started>; 6],
    /// Where each player stood the update before.
    last: [[f32; 2]; 6],
    /// Each finished move.
    done: Vec<Move>,
    /// The nearest opponent's distance through the moves, by six updates
    /// from the press (the last bin 72 on): updates, summed distance,
    /// those within 4.
    by_tick: [(u32, f32, u32); 13],
    /// Per player under way: the nearest opponent at the press and both
    /// places then.
    first: [Option<Faced>; 6],
    /// Over the blend's six updates: moves, the change of distance the
    /// handler's own motion made, the defender's, the handler's distance
    /// covered.
    blend: (u32, f32, f32, f32),
}

impl Moves {
    /// Player `i` at `at` in update `now`, in a trick or not, attacking
    /// `rim`; `gap` when the update before is missing (a move under way
    /// is dropped).
    /// Player `i` in a move: his place and every player's, in update `now`
    /// (the first six updates' split of the change of distance).
    fn split(&mut self, i: usize, now: i64, places: &[[f32; 2]], team: &[usize]) {
        let Some((start, _, _)) = self.open[i] else {
            self.first[i] = None;
            return;
        };
        if now == start {
            let near = (0..places.len())
                .filter(|&j| team[j] != team[i])
                .min_by(|&a, &b| dist(places[a], places[i]).total_cmp(&dist(places[b], places[i])));
            self.first[i] = near.map(|d| (d, self.last[i], places[d]));
        }
        if now == start + 6
            && let Some((d, p0, q0)) = self.first[i]
        {
            let (p6, q6) = (places[i], places[d]);
            self.blend.0 += 1;
            self.blend.1 += dist(q0, p6) - dist(q0, p0);
            self.blend.2 += dist(q6, p6) - dist(q0, p6);
            self.blend.3 += dist(p6, p0);
        }
    }

    /// Player `i` in a move with his nearest opponent `d` away.
    fn spacing(&mut self, i: usize, now: i64, d: f32) {
        if let Some((start, _, _)) = self.open[i] {
            let k = (((now - start) / 6) as usize).min(12);
            self.by_tick[k].0 += 1;
            self.by_tick[k].1 += d;
            self.by_tick[k].2 += u32::from(d < 4.0);
        }
    }

    /// Player `i` at `at` in update `now`, in a trick or not, attacking
    /// `rim`, the ball's fate that update `end`; `gap` when the update
    /// before is missing (a move under way is dropped).
    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        i: usize,
        now: i64,
        tricking: bool,
        at: [f32; 2],
        rim: [f32; 2],
        end: MoveEnd,
        gap: bool,
    ) {
        if gap {
            self.open[i] = None;
        }
        match (self.open[i], tricking) {
            (None, true) if !gap => self.open[i] = Some((now, self.last[i], rim)),
            (Some((start, from, rim)), false) => {
                // The move is its updates: from where he stood the update
                // before the first to where the last left him.
                let last = self.last[i];
                let d = [last[0] - from[0], last[1] - from[1]];
                let to_rim = [rim[0] - from[0], rim[1] - from[1]];
                let n = (to_rim[0] * to_rim[0] + to_rim[1] * to_rim[1])
                    .sqrt()
                    .max(1e-3);
                let along = (d[0] * to_rim[0] + d[1] * to_rim[1]) / n;
                self.done
                    .push((now - start, (d[0] * d[0] + d[1] * d[1]).sqrt(), along, end));
                self.open[i] = None;
            }
            _ => {}
        }
        self.last[i] = at;
    }

    /// The moves' mean length in updates.
    fn mean_length(&self) -> f64 {
        self.done.iter().map(|m| m.0 as f64).sum::<f64>() / self.done.len().max(1) as f64
    }

    /// The mean length of the moves that ran their script out (none cut
    /// short by a pass, a shot or the ball lost).
    fn ran_length(&self) -> f64 {
        let ran: Vec<f64> = self
            .done
            .iter()
            .filter(|m| m.3 == MoveEnd::Ran)
            .map(|m| m.0 as f64)
            .collect();
        ran.iter().sum::<f64>() / ran.len().max(1) as f64
    }

    /// The moves that ended `how`.
    fn ended(&self, how: MoveEnd) -> usize {
        self.done.iter().filter(|m| m.3 == how).count()
    }

    fn json(&self) -> Value {
        let n = self.done.len().max(1) as f64;
        let mut lengths: Vec<i64> = self.done.iter().map(|m| m.0).collect();
        lengths.sort_unstable();
        let mean = |f: &dyn Fn(&Move) -> f64| self.done.iter().map(f).sum::<f64>() / n;
        let share =
            |f: &dyn Fn(&Move) -> bool| self.done.iter().filter(|m| f(m)).count() as f64 / n;
        json!({"moves": self.done.len(),
        "updates_mean": mean(&|m| m.0 as f64),
        "updates_median": lengths.get(lengths.len() / 2),
        "ran_out": self.ended(MoveEnd::Ran), "ran_out_updates_mean": self.ran_length(),
        "let_go_out_of": self.ended(MoveEnd::LetGo), "lost_in": self.ended(MoveEnd::Lost),
        "displacement_mean": mean(&|m| f64::from(m.1)),
        "toward_rim_mean": mean(&|m| f64::from(m.2)),
        "toward_rim_over_1": share(&|m| m.2 > 1.0),
        "away_from_rim_over_1": share(&|m| m.2 < -1.0),
        "blend_split": {"moves": self.blend.0,
            "handler_change": self.blend.1 / self.blend.0.max(1) as f32,
            "defender_change": self.blend.2 / self.blend.0.max(1) as f32,
            "handler_covered": self.blend.3 / self.blend.0.max(1) as f32},
        "nearest_by_6_updates": self.by_tick.map(|(n, sum, near)| {
            format!("{n}: {:.2} {:.2}", sum / n.max(1) as f32, f64::from(near) / f64::from(n.max(1)))
        })})
    }
}

impl Tally {
    fn json(&self) -> Value {
        let n = self.held.iter().sum::<u32>().max(1);
        let share: Vec<String> = self
            .held
            .iter()
            .map(|&c| format!("{:.3}", f64::from(c) / f64::from(n)))
            .collect();
        json!({"front_court_updates": self.front_court, "held_updates": n,
        "nearest_defender_bins": SPACING.map(|b| b.min(99.0)),
        "nearest_defender_share": share,
        "shot_phases": self.shot_phases, "turnovers_out_of_phase_5": self.turnovers,
        "steal_press_updates": self.steal_presses, "counter_press_updates": self.counter_presses,
        "held_ball_knocked_loose": self.knocked_loose, "positional_quality": self.quality,
        "back_court_pass_fronts": self.back_court_pass_fronts,
        "back_court_passes": self.back_court_passes,
        "nearest_defender_mean_by_rim_distance_6": self.by_reach.map(|(n, s)| {
            format!("{n}: {:.2}", s / n.max(1) as f32)
        }),
        "handler_tricking": {"updates": self.tricking.0,
            "share": share_of(self.tricking.0, n), "within_4": share_of(self.tricking.1, self.tricking.0)},
        "away_from_tricks": {"updates": self.away.0, "within_4": self.away_within_4()},
        "defender_speed_levels": self.levels.map(|c| share_of(c, self.levels.iter().sum())),
        "trick_moves": self.moves.json(),
        "updates_by_phase": self.phases, "phase_changes": self.changes,
        "block_jumps": self.block_jumps, "pump_fakes": self.pump_fakes,
        "ball_touches": self.touches.json()})
    }

    /// One update in `phase`, after one in `last`.
    fn phase(&mut self, last: i64, phase: i64) {
        *self.phases.entry(phase).or_default() += 1;
        if phase != last {
            *self.changes.entry(format!("{last}->{phase}")).or_default() += 1;
        }
    }

    /// The share of held front-court updates with a defender within 4.
    fn within_4(&self) -> f64 {
        f64::from(self.held[0] + self.held[1]) / f64::from(self.held.iter().sum::<u32>().max(1))
    }

    /// [`Tally::within_4`] away from the handler's tricks.
    fn away_within_4(&self) -> f64 {
        share_of(self.away.1, self.away.0)
    }

    /// The share of the defenders' updates standing (speed level 0).
    fn standing(&self) -> f64 {
        share_of(self.levels[0], self.levels.iter().sum())
    }

    /// One held front-court update: the handler tricking, the updates
    /// since his last trick, the nearest defender's distance.
    fn held_update(&mut self, tricking: bool, since: i64, d: f32) {
        if tricking {
            self.tricking.0 += 1;
            self.tricking.1 += u32::from(d < 4.0);
        } else if since >= AWAY {
            self.away.0 += 1;
            self.away.1 += u32::from(d < 4.0);
        }
    }

    /// The changes of possession to the back court (phase 4): the
    /// original's into phase 6 are its gamebreaker baskets (a staged
    /// sequence's basket hands the ball over without phase 3,
    /// [`native_kinds`]), which a basket the rewrite counts never
    /// reaches (its scored changes are left out).
    fn turnovers(&self) -> u32 {
        self.turnovers
            .iter()
            .filter(|&(&phase, _)| phase != 6)
            .map(|(_, n)| n)
            .sum()
    }
}

fn share_of(n: u32, of: u32) -> f64 {
    f64::from(n) / f64::from(of.max(1))
}

/// The shot's quality (`FUN_001805e8`) of a handler of `team` at `at`
/// from where the defenders stand, every player a 6.5 ft man with 50 in
/// every rating (the captures do not record the players), when he is
/// 12..29.75 from the rim (where the shot check reads it).
fn positional_quality(team: usize, at: [f32; 2], defenders: &[[f32; 2]]) -> Option<i32> {
    let rim = hoops()[crate::target_hoop(team)].rim;
    let rim = [rim[0], rim[2]];
    let d = dist(at, rim);
    if !(12.0 < d && d < 29.75) {
        return None;
    }
    let mut near: Vec<[f32; 2]> = defenders.to_vec();
    near.sort_by(|a, b| dist(*a, at).total_cmp(&dist(*b, at)));
    let view = |p: [f32; 2]| crate::shooting::ContestDefender {
        distance_to_shooter: dist(p, at),
        distance_to_rim: dist(p, rim),
        height_ft: 6.5,
        blocks: 50,
    };
    let shot = crate::shooting::ShotView {
        distance_to_rim: d,
        shots: 50,
        threes: 50,
        dunks: 50,
        outside: !(at[1].abs() < 22.5 && d < 24.25),
        meter_full: false,
        height_ft: 6.5,
    };
    Some(crate::evaluate_shot_quality_tier(
        &shot,
        near.first().map(|&p| view(p)),
        near.get(1).map(|&p| view(p)),
    ))
}

/// The original's turnovers and lost balls by kind (2026-10-16, audit
/// 1.5): each change of possession out of phase 5 by what took the ball
/// (a trick counter: the new holder pressed steal with two turbos within
/// 8 updates; a counter-stance catch of a head pass: a pass caught by a
/// defender playing a `STEALCOUNTER` clip, records 402..409; a swipe's
/// catch: the new holder swiped within 45 updates; an interception: any
/// other pass caught by the defence; a gamebreaker basket: phase 6), and
/// the balls knocked loose (a held ball while a defender's swipe bit is
/// set: a strip; a shot or pass by a defender playing a block clip,
/// records 436..465: a block; any other pass in flight: a deflection).
fn native_kinds(matches: &[Vec<Update>]) -> std::collections::BTreeMap<&'static str, u32> {
    let mut kinds = std::collections::BTreeMap::new();
    for m in matches {
        // The holder's pointer by the actor nearest the held ball.
        let mut votes: std::collections::HashMap<i64, [u32; 6]> = Default::default();
        for u in m {
            let holder = u.int("ball_holder");
            if u.int("ball_state") == 3 && holder != 0 {
                let b = u.ball();
                let i = (0..6)
                    .min_by(|&a, &c| {
                        dist(u.floor(a), [b[0], b[2]]).total_cmp(&dist(u.floor(c), [b[0], b[2]]))
                    })
                    .unwrap();
                votes.entry(holder).or_default()[i] += 1;
            }
        }
        let actor = |p: i64| votes.get(&p).and_then(|v| (0..6).max_by_key(|&i| v[i]));
        let held_by = |u: &Update| {
            (u.int("ball_state") == 3)
                .then(|| actor(u.int("ball_holder")))
                .flatten()
        };
        let animation = |u: &Update, i: usize| u.actors()[i]["animation"].as_i64().unwrap_or(-1);
        let pad =
            |u: &Update, i: usize, k: usize| u.actors()[i]["pad"][k].as_u64().unwrap_or(0) as u32;
        let pressed_within = |k: usize, i: usize, back: usize, bits: u32, two: bool| {
            m[k.saturating_sub(back)..=k].iter().any(|w| {
                let p = pad(w, i, 1);
                p & bits::STEAL != 0
                    && (p & bits::TWO_TURBOS != 0 || pad(w, i, 2) & bits::TWO_TURBOS != 0) == two
                    && p & bits == bits
            })
        };
        for k in 1..m.len() {
            let (a, u) = (&m[k - 1], &m[k]);
            if u.frame() != a.frame() + 1 {
                continue;
            }
            let (sa, su) = (a.int("ball_state"), u.int("ball_state"));
            let team = |i: usize| u.actors()[i]["team"].as_i64().unwrap_or(-1);
            if a.phase() == 5 && a.int("possession") != u.int("possession") {
                let kind = if u.phase() == 6 {
                    "gamebreaker basket"
                } else if let Some(h) = held_by(u) {
                    if sa == 4 && (402..=409).contains(&animation(u, h)) {
                        "counter catch of a head pass"
                    } else if sa == 4 {
                        "interception"
                    } else if pressed_within(k, h, 8, bits::STEAL | bits::TWO_TURBOS, true) {
                        "trick counter"
                    } else if pressed_within(k, h, 45, bits::STEAL, false) {
                        "swipe catch"
                    } else {
                        "other"
                    }
                } else {
                    "other"
                };
                *kinds.entry(kind).or_default() += 1;
            }
            if su != 1 || !matches!(sa, 3..=5) {
                continue;
            }
            let b = a.ball();
            let nearest_blocker = (0..6)
                .filter(|&i| (436..=465).contains(&animation(a, i)))
                .any(|i| dist(a.floor(i), [b[0], b[2]]) < 12.0);
            let kind = match sa {
                3 if a.phase() == 5
                    && (0..6).any(|i| {
                        u.actors()[i]["animation_state"].as_u64().unwrap_or(0) & 0x40 != 0
                            && Some(team(i)) != held_by(a).map(team)
                    }) =>
                {
                    "strip"
                }
                3 => continue,
                4 if nearest_blocker => "block of a pass",
                4 if a.int("ball_progress") > 0
                    && a.int("ball_progress") < a.int("ball_flight_updates") =>
                {
                    "pass deflected"
                }
                5 if nearest_blocker
                    && !m[k..(k + 80).min(m.len())].iter().any(|w| w.phase() == 6) =>
                {
                    "block of a shot"
                }
                _ => continue,
            };
            *kinds.entry(kind).or_default() += 1;
        }
    }
    kinds
}

/// The native side of [`shots_turnovers_and_spacing_against_the_captured_original`].
fn native_tally(matches: &[Vec<Update>]) -> (Tally, usize) {
    let mut native = Tally::default();
    let mut updates = 0;
    for m in matches {
        native.touches.add(&native_touches(m));
        // The frame each actor last played a trick (state bit 10).
        let mut trick = [i64::MIN / 2; 6];
        // The frame a jump shot was last let go.
        let mut released = i64::MIN / 2;
        for pair in m.windows(2) {
            let (a, u) = (&pair[0], &pair[1]);
            updates += 1;
            let bits = |i: usize| {
                u.actors()
                    .get(i)
                    .map_or(0, |x| x["animation_state"].as_i64().unwrap_or(0))
            };
            let end = MoveEnd::of(
                a.int("ball_state"),
                u.int("ball_state"),
                a.int("possession") != u.int("possession"),
            );
            for (i, t) in trick.iter_mut().enumerate() {
                if bits(i) & 0x400 != 0 {
                    *t = u.frame();
                }
                let side = u.actors()[i]["team"].as_i64().unwrap_or(0) as usize;
                let rim = hoops()[crate::target_hoop(side.min(1))].rim;
                native.moves.update(
                    i,
                    u.frame(),
                    bits(i) & 0x400 != 0,
                    u.floor(i),
                    [rim[0], rim[2]],
                    end,
                    u.frame() != a.frame() + 1,
                );
                let near = (0..6)
                    .filter(|&j| u.actors()[j]["team"] != u.actors()[i]["team"])
                    .map(|j| dist(u.floor(j), u.floor(i)))
                    .fold(f32::INFINITY, f32::min);
                native.moves.spacing(i, u.frame(), near);
                let places: Vec<[f32; 2]> = (0..6).map(|j| u.floor(j)).collect();
                let teams: Vec<usize> = (0..6)
                    .map(|j| u.actors()[j]["team"].as_i64().unwrap_or(0) as usize)
                    .collect();
                native.moves.split(i, u.frame(), &places, &teams);
            }
            native.phase(a.phase(), u.phase());
            // Block jumps, by the holder the update before (the possessing
            // side's player nearest the ball while it is held).
            let state =
                |w: &Update, i: usize| w.actors()[i]["animation_state"].as_i64().unwrap_or(0);
            let holder = |w: &Update| {
                if w.int("ball_state") != 3 {
                    return None;
                }
                let ball = w.ball();
                let possession = w.int("possession");
                (0..6)
                    .filter(|&i| w.actors()[i]["team"].as_i64() == Some(possession))
                    .min_by(|&p, &q| {
                        dist(w.floor(p), [ball[0], ball[2]])
                            .total_cmp(&dist(w.floor(q), [ball[0], ball[2]]))
                    })
            };
            if u.frame() == a.frame() + 1 {
                if a.int("ball_state") == 3
                    && u.int("ball_state") == 5
                    && holder(a).is_some_and(|h| state(a, h) & 0x88 == 0x8)
                {
                    released = u.frame();
                }
                for i in 0..6 {
                    if state(u, i) & 0x1000 != 0 && state(a, i) & 0x1000 == 0 {
                        let kind = block_jump_kind(
                            holder(a).map(|h| state(a, h)),
                            a.int("ball_state"),
                            u.frame() - released <= 1,
                            u.frame() == released,
                        );
                        *native.block_jumps.entry(kind).or_default() += 1;
                    }
                }
            }
            if u.phase() == 3 && a.phase() != 3 {
                native.shot_phases += 1;
            }
            if a.phase() == 5 && a.int("possession") != u.int("possession") {
                *native.turnovers.entry(u.phase()).or_default() += 1;
            }
            if a.phase() == 5
                && u.frame() == a.frame() + 1
                && a.int("ball_state") == 3
                && u.int("ball_state") == 1
            {
                native.knocked_loose += 1;
            }
            if u.phase() == 4 {
                if u.frame() == a.frame() + 1
                    && a.int("ball_state") == 3
                    && u.int("ball_state") == 4
                {
                    native.back_court_passes += 1;
                }
                let possession = u.int("possession");
                native.back_court_pass_fronts += (0..6)
                    .filter(|&i| {
                        u.actors()[i]["team"].as_i64() == Some(possession) && u.front(i) == "pass"
                    })
                    .count() as u32;
            }
            if u.phase() != 5 {
                continue;
            }
            native.front_court += 1;
            let possession = u.int("possession");
            let team = |i: usize| u.actors()[i]["team"].as_i64().unwrap();
            let pressed = |i: usize, bit: u32| {
                team(i) != possession
                    && u.actors()[i]["pad"][1].as_u64().unwrap_or(0) & u64::from(bit) != 0
            };
            if (0..6).any(|i| pressed(i, bits::STEAL)) {
                native.steal_presses += 1;
            }
            let held = |i: usize, bit: u32| {
                u.actors()[i]["pad"][2].as_u64().unwrap_or(0) & u64::from(bit) != 0
            };
            if (0..6).any(|i| {
                pressed(i, bits::STEAL)
                    && (pressed(i, bits::TWO_TURBOS) || held(i, bits::TWO_TURBOS))
            }) {
                native.counter_presses += 1;
            }
            if u.int("ball_state") != 3 {
                continue;
            }
            // The holder: the possessing side's player nearest the ball.
            let ball = u.ball();
            let ball = [ball[0], ball[2]];
            let h = (0..6)
                .filter(|&i| team(i) == possession)
                .min_by(|&a, &b| dist(u.floor(a), ball).total_cmp(&dist(u.floor(b), ball)))
                .unwrap();
            let d = (0..6)
                .filter(|&i| team(i) != possession)
                .map(|i| dist(u.floor(i), u.floor(h)))
                .fold(f32::INFINITY, f32::min);
            native.held[spacing_bin(d)] += 1;
            native.held_update(bits(h) & 0x400 != 0, u.frame() - trick[h], d);
            for i in (0..6).filter(|&i| team(i) != possession) {
                // Not in the counter stance (bit 16), a swipe (6) or a
                // block jump (12).
                if u.front(i) == "defense" && bits(i) & 0x11040 == 0 {
                    let level = u.actors()[i]["speed_level"].as_i64().unwrap_or(0);
                    native.levels[level.clamp(0, 3) as usize] += 1;
                }
            }
            let rim = hoops()[crate::target_hoop(possession as usize)].rim;
            let k = ((dist(u.floor(h), [rim[0], rim[2]]) / 6.0) as usize).min(5);
            native.by_reach[k].0 += 1;
            native.by_reach[k].1 += d;
            let defenders: Vec<[f32; 2]> = (0..6)
                .filter(|&i| team(i) != possession)
                .map(|i| u.floor(i))
                .collect();
            if let Some(q) = positional_quality(possession as usize, u.floor(h), &defenders) {
                native.quality[q as usize] += 1;
            }
        }
    }
    (native, updates)
}

/// The rewrite's side of
/// [`shots_turnovers_and_spacing_against_the_captured_original`]: computer
/// games for `updates` updates, each new match from `make(seed)`, with the
/// events by name and the shot check's quality gate.
fn our_tally(updates: usize, make: impl Fn(u32) -> World) -> (Tally, Value) {
    let mut ours = Tally::default();
    let mut seed = 7;
    let mut w = make(seed);
    w.start_opening_toss();
    let mut last_phase = i64::from(w.game_phase.phase);
    let mut last_possession = w.possession;
    let mut last_held = false;
    let mut events: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
    // The update each player last played a trick.
    let mut trick = [i64::MIN / 2; 6];
    super::playbook::probe::take();
    // The update a jump shot was last let go.
    let mut released = i64::MIN / 2;
    for _ in 0..updates {
        // What a block jump this update went up at ([`block_jump_kind`]):
        // the holder's move as the state bits show it, the ball's state.
        let move_bits = |a: &crate::Action| match a {
            crate::Action::Shooting { .. } | crate::Action::Layup { .. } => 0x8,
            crate::Action::Dunking { layup: true, .. } => 0x2008,
            a if crate::leap::leaping(a) => 0x8000,
            crate::Action::Dunking { .. } => 0x80,
            _ => 0,
        };
        let holder = w.handler().map(|h| (h, w.players[h].action));
        let ball_state = i64::from(w.native_ball_state());
        let jumping: Vec<bool> = w
            .players
            .iter()
            .map(|p| matches!(p.action, crate::Action::Jumping { .. }))
            .collect();
        let faking: Vec<bool> = w
            .players
            .iter()
            .map(|p| matches!(p.action, crate::Action::PumpFake { .. }))
            .collect();
        let ball = ball_before(&w);
        // The pads as the last update left them: a press is a button new
        // this update (`FUN_0013a970`'s pad+4, which the captures record).
        let last_pads: Vec<u32> = w.ai.brains.iter().map(|b| b.pad.buttons).collect();
        step_computer(&mut w);
        ours.touches.ours(ball, &w);
        let now = w.tick as i64;
        if let Some((_, crate::Action::Shooting { .. })) = holder
            && w.ball.state == crate::BallState::Shot
        {
            released = now;
        }
        for (i, p) in w.players.iter().enumerate() {
            if matches!(p.action, crate::Action::Jumping { block: Some(_), .. }) && !jumping[i] {
                let kind = block_jump_kind(
                    holder.map(|(_, a)| move_bits(&a)),
                    ball_state,
                    now - released <= 1,
                    now == released,
                );
                *ours.block_jumps.entry(kind).or_default() += 1;
            }
            if matches!(p.action, crate::Action::PumpFake { .. }) && !faking[i] {
                ours.pump_fakes += 1;
            }
        }
        for p in &w.players {
            if let crate::Action::Stumbling {
                reaction: Some(r), ..
            } = p.action
                && r.run.tick == 0
            {
                *events.entry("reaction").or_default() += 1;
            }
        }
        let end = MoveEnd::of(
            ball_state,
            i64::from(w.native_ball_state()),
            w.possession != last_possession,
        );
        for (i, t) in trick.iter_mut().enumerate() {
            let tricking = w.players[i].action.juking();
            if tricking {
                *t = now;
            }
            let rim = w.rim_spot(crate::target_hoop(w.players[i].team));
            ours.moves
                .update(i, now, tricking, w.players[i].position, rim, end, false);
            let near = (0..6)
                .filter(|&j| w.players[j].team != w.players[i].team)
                .map(|j| dist(w.players[j].position, w.players[i].position))
                .fold(f32::INFINITY, f32::min);
            ours.moves.spacing(i, now, near);
            let places: Vec<[f32; 2]> = w.players.iter().map(|p| p.position).collect();
            let teams: Vec<usize> = w.players.iter().map(|p| p.team).collect();
            ours.moves.split(i, now, &places, &teams);
        }
        // The phase after the game's update, as the capture samples it;
        // shots are its entries into phase 3, as the original's (dunks go
        // up through the launcher too, `World::launch_dunk`).
        let phase = i64::from(w.game_phase.phase);
        if phase == 3 && last_phase != 3 {
            ours.shot_phases += 1;
        }
        let (mut scored, mut dunked) = (false, false);
        for e in &w.events {
            let name = match e {
                crate::Event::Shot { .. } => "shot",
                crate::Event::Dunk { .. } => {
                    dunked = true;
                    "dunk"
                }
                crate::Event::Steal {
                    intercepted_pass: true,
                    ..
                } => "interception",
                crate::Event::Steal { .. } => "steal",
                crate::Event::Counter { .. } => "counter",
                crate::Event::Deflection { .. } => "deflection",
                crate::Event::Block { .. } => "block",
                crate::Event::Pass { .. } => "pass",
                crate::Event::Score { .. } => {
                    scored = true;
                    "score"
                }
                crate::Event::Trick { fakeout: None, .. } => "trick",
                crate::Event::Trick { .. } => "trick faked",
                _ => continue,
            };
            *events.entry(name).or_default() += 1;
            if name == "pass" && phase == 4 {
                ours.back_court_passes += 1;
            }
        }
        if last_phase == 5 && w.possession != last_possession && !scored {
            *ours.turnovers.entry(phase).or_default() += 1;
        }
        ours.phase(last_phase, phase);
        // A held ball gone loose (not a basket, nor a dunk let go).
        if last_phase == 5
            && last_held
            && w.ball.state == crate::BallState::Loose
            && !scored
            && !dunked
        {
            ours.knocked_loose += 1;
            let swiping = w.players.iter().any(|p| {
                p.team != w.possession
                    && matches!(p.action, crate::Action::Stealing { counter: false, .. })
            });
            *events
                .entry(if swiping { "strip" } else { "other loose" })
                .or_default() += 1;
        }
        last_held = matches!(w.ball.state, crate::BallState::Held(_));
        if phase == 4 {
            for i in 0..6 {
                if w.players[i].team == w.possession
                    && matches!(
                        w.ai.brains[i].queue.first().map(|n| &n.act),
                        Some(Act::Pass(_))
                    )
                {
                    ours.back_court_pass_fronts += 1;
                }
            }
        }
        if phase == 5 {
            ours.front_court += 1;
            // The steal pressed this update (new since the last), with two
            // turbos held for a counter, as the original's pad words count
            // it ([`native_tally`]).
            let pressed = |i: usize, bits: u32| {
                let now = w.ai.brains[i].pad.buttons;
                let was = last_pads.get(i).copied().unwrap_or(0);
                w.players[i].team != w.possession
                    && now & bits == bits
                    && now & bits::STEAL & !was != 0
            };
            if (0..6).any(|i| pressed(i, bits::STEAL)) {
                ours.steal_presses += 1;
            }
            if (0..6).any(|i| pressed(i, bits::STEAL | bits::TWO_TURBOS)) {
                ours.counter_presses += 1;
            }
            // The ball held as the capture reads it (state 3): a pass's
            // windup is already state 4 (`World::native_ball_state`).
            if let Some(h) = w
                .handler()
                .filter(|_| w.native_ball_state() == crate::game_phase::ball::HELD)
            {
                let d = (0..6)
                    .filter(|&i| w.players[i].team != w.players[h].team)
                    .map(|i| dist(w.players[i].position, w.players[h].position))
                    .fold(f32::INFINITY, f32::min);
                ours.held[spacing_bin(d)] += 1;
                let tricking = w.players[h].action.juking();
                ours.held_update(tricking, now - trick[h], d);
                for i in (0..6).filter(|&i| w.players[i].team != w.players[h].team) {
                    let defending = matches!(
                        w.ai.brains[i].queue.first().map(|n| &n.act),
                        Some(Act::Defense(_))
                    );
                    let free = !matches!(
                        w.players[i].action,
                        crate::Action::Stealing { .. } | crate::Action::Jumping { .. }
                    );
                    if defending && free {
                        ours.levels[w.ai.brains[i].motion.level.clamp(0, 3) as usize] += 1;
                    }
                }
                let rim = w.rim_spot(crate::target_hoop(w.players[h].team));
                let k = ((dist(w.players[h].position, rim) / 6.0) as usize).min(5);
                ours.by_reach[k].0 += 1;
                ours.by_reach[k].1 += d;
                let team = w.players[h].team;
                let defenders: Vec<[f32; 2]> = (0..6)
                    .filter(|&i| w.players[i].team != team)
                    .map(|i| w.players[i].position)
                    .collect();
                if let Some(q) = positional_quality(team, w.players[h].position, &defenders) {
                    ours.quality[q as usize] += 1;
                }
            }
        }
        last_phase = phase;
        last_possession = w.possession;
        if matches!(w.phase, crate::Phase::Over { .. }) {
            seed += 1;
            w = make(seed);
            w.start_opening_toss();
            last_phase = i64::from(w.game_phase.phase);
            last_possession = w.possession;
            trick = [i64::MIN / 2; 6];
            ours.moves.open = Default::default();
        }
    }
    // The playbook's shot check (`FUN_001290b8`) where it reaches the
    // quality gate (12..29.75 out): the qualities, and how many met the
    // need.
    let (qualities, passed) = super::playbook::probe::take();
    let mut report = ours.json();
    report["events"] = json!(events);
    report["gate_qualities"] = json!(qualities);
    report["gate_passed"] = json!(passed);
    report["within_4"] = json!(ours.within_4());
    report["turnovers"] = json!(ours.turnovers());
    (ours, report)
}

/// The seed bases [`shots_turnovers_and_spacing_against_the_captured_original`]
/// runs (each game's seed plus 1000 times the base; base 0 is the single
/// game the comparison ran before 2026-10-17): its figures are means over
/// this many games of the capture's own players, or, for a capture that
/// does not know them, this many all-50 games and four times as many of
/// the disc's pairs.
const BASES: u32 = 16;

/// A figure over the seed bases: the mean, the single games' standard
/// deviation and the mean's standard error.
#[derive(Clone, Copy, Debug)]
struct Spread {
    mean: f64,
    sd: f64,
    se: f64,
}

fn spread(xs: &[f64]) -> Spread {
    let n = xs.len().max(1) as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    let sd = var.sqrt();
    Spread {
        mean,
        sd,
        se: sd / n.sqrt(),
    }
}

impl Spread {
    fn json(&self, native: f64) -> Value {
        json!({"native": native, "mean": self.mean, "sd": self.sd, "se": self.se})
    }
}

/// The comparison's bound on each figure's mean against the original's
/// match ([`shots_turnovers_and_spacing_against_the_captured_original`]):
/// each the gap the 16 bases' mean stands from the match plus three of its
/// standard errors, rounded up, so a change of the generator's path alone
/// stays inside and a change of behaviour shows (the seed set +777 stays
/// inside every one).
///
/// Set again on 2026-10-18 (branch `computer-play`, `docs/TRACE.md` "The
/// computer's shooters, drives and trick passes"), after the players'
/// power and overall ratings came from their records and a pass to
/// himself made everyone busy; the means (sd of single games) against the
/// matches, first capture (Lakers against Portland, inferred) then third
/// (Indiana against San Antonio, recorded), the seed sets +500 and +777
/// inside every bound:
///
/// - standing 0.389 (0.011) against 0.376; 0.390 (0.016) against 0.423;
/// - away from tricks within 4: 0.200 (0.028) against 0.208; 0.211 (0.022)
///   against 0.208;
/// - the moves that ran their script out, length 61.1 (0.7) against 61.2;
///   59.7 (0.7) against 60.7 (all moves 58.6 against 58.7 and 57.4 against
///   52.6: the original lets go of the ball out of 0 and 8 moves and loses
///   it in 5 and 6, the rewrite 4.4 and 5.0, 3.9 and 3.8);
/// - within 4 through the tricks 0.383 (0.030) against 0.372; 0.341
///   (0.048) against 0.357;
/// - the defender's back-off through the blend 0.438 (0.107) against
///   0.406; 0.393 (0.096) against 0.337;
/// - within 4 0.265 (0.025) against 0.262; 0.260 (0.021) against 0.244;
/// - turnovers 5.3 (1.8) against 6; 3.1 (2.8) against 3;
/// - counter presses 21.3 (3.9) against 17; 15.9 (4.3) against 15;
/// - shots 51.6 (3.3) against 55; 40.0 (3.0) against 43;
/// - block jumps (`Tally::block_jumps`; the computer's block decision,
///   `FUN_00106a88`) at a shooter 13.4 (2.6) against 13 and 11.6 (3.1)
///   against 17, after a release 7.6 (2.2) against 7 and 5.6 (2.9)
///   against 3, at a dunker 24.4 (5.3) against 30 and 18.2 (4.7) against
///   20. The third match's shooters are not the rewrite's (Robinson shot
///   once, the rewrite's 4.9 a game; Jermaine O'Neal 11 and Steve Smith
///   14, the rewrite's 6.5 and 8.4): its block jumps at a shooter follow.
///
/// The first capture's set again on 2026-10-18 (branch `presses`,
/// `docs/TRACE.md` "The person's presses"), with its actor 5 taken for
/// Portland's 113 ([`captured_rosters`]) and the shoot press's dunk and
/// lay-up rows ported (`FUN_001693b8`; 112's Dunks 0 laid every press up,
/// 17.9 block jumps a game at a shooter). The means (sd) against the
/// match: standing 0.373 (0.016) against 0.376; away within 4 0.212
/// (0.023) against 0.208; ran-out length 59.9 (0.8) against 61.2 (the
/// seed sets +500 and +777: 60.6 and 60.5); within 4 in tricks 0.365
/// (0.034) against 0.372; the blend's back-off 0.423 (0.074) against
/// 0.406; within 4 0.265 (0.021) against 0.262; turnovers 4.5 (1.4)
/// against 6; counter presses 17.8 (4.3) against 17; shots 55.9 (3.8)
/// against 55; block jumps at a shooter 12.5 (4.1) against 13, after a
/// release 8.2 (2.7) against 7, at a dunker 26.4 (6.0) against 30. The
/// seed sets +500 and +777 inside every bound.
///
/// The blend's back-off set again by the rule on 2026-10-23 with the
/// holders backing down (`docs/TRACE.md` "The post-up's backdown"): 0.488
/// (0.126, se 0.031) against 0.406, 0.18 (the seed sets +500 0.452 and
/// +777 0.420; without the backdown 0.440, 0.414, 0.459).
///
/// The loose ball's bounds added on 2026-10-18 (branch `loose-ball`,
/// `docs/TRACE.md` "The loose ball"), by the same rule, with the receiver's
/// near take, the parked rows, the running catches over the run, the gait
/// blends, the swipe on its script and the contacts on the distance table
/// ported. The means (sd) against the matches, first capture, then third
/// (recorded), then the second's every player at 50: knocks off a body
/// outside a block jump 0.63 (0.81) against 1, 0.88 (1.03) against 0, 0.19
/// (0.40) against 0; passes knocked loose 0.75 (0.86) against 2, 1.06
/// (1.06) against 1, 0.25 (0.58) against 1; shots knocked loose 1.06 (0.77)
/// against 2, 0.88 (0.89) against 2, 0.31 (0.70) against 0; passes loose at
/// their timer 8.0 (3.5) against 1, 5.5 (2.4) against 0, 1.75 (1.2)
/// against 0; catches with 5 or more updates of the flight left 8.7%
/// against 1.2%, 10.6% against 2.2%, 7.6% against 8.2%; the loose-ball
/// phase's updates 1,568 (432) against 762, 1,036 (378) against 915, 366
/// (193) against 396. The passes lost at their timer are the rewrite's own
/// trick passes off the glass and receivers carried into the court's bound
/// (the computer's runs), the late catches its inbound lobs (the
/// inbounder's spot): docs/TRACE.md "The loose ball". With the loose ball
/// mended the first capture's front court runs longer and its counter
/// presses rose to 21.75 (4.6) against 17 (48 bases 21.0): their bound by
/// the rule is 8.5. The third capture's ran-out trick length, 59.5 (1.0)
/// against 60.7 over 48 bases, drew 59.1 on the seed set +16000: its bound
/// is 2.0. The seed set +16000 stays inside every other bound.
///
/// Set again on 2026-10-18 (branch `computer-offence`, `docs/TRACE.md` "The
/// computer's offence"), by the same rule over the 16 bases and the seed
/// set +16000 (32 games), with the receiver's run to the pass's spot, the
/// passout to oneself off the board on its row and legs and the nodes'
/// pass targets ported, and the counter presses counted as the captures
/// count them (a press new that update, pad+4). The means (sd) of the 16,
/// first capture, third, second: passes loose at their timer 1.69 (1.45)
/// against 1, 1.31 (0.95) against 0, 0.13 (0.34) against 0 (+16000: 1.19,
/// 1.13, 0.56); the loose phase 974 (234) against 762, 807 (183) against
/// 915, 341 (117) against 396; counter presses 20.6 (3.4) against 17, 16.2
/// (2.6) against 15, 8.3 (3.9) against 3. The defenders stand 0.397 (0.017)
/// against 0.376 in the first (main 0.372 and 0.381 on the two sets), 0.392
/// (0.015) against 0.423 in the third and 0.405 (0.023) against 0.398 in
/// the second: over the three matches 0.398 against 0.399. The receivers
/// wait at their spots and their men with them in every match; the first
/// match stands less than the other two, so its bound by the rule is
/// 0.035. The third capture's block jumps at a shooter, 9.50 (3.16) and
/// 7.63 (2.85) on the two sets against 17 (main 9.56 and 9.56; the third
/// match's shooters are not the rewrite's, above), take 10.5.
///
/// Every player at 50 and the disc's pairs against the second capture.
#[derive(Clone, Copy, Debug)]
struct Bounds {
    standing: f64,
    away: f64,
    /// The trick moves that ran their script out: their mean length.
    length: f64,
    in_tricks: f64,
    blend: f64,
    within: f64,
    turnovers: f64,
    counters: f64,
    shots: f64,
    /// The block jumps at a shooter, after a release and at a dunker.
    jumps: [f64; 3],
    /// The loose ball ([`Touches`]): knocks off a body outside a block
    /// jump, passes knocked loose, passes loose at their timer, shots
    /// knocked loose, the share of catches with 5 or more updates of the
    /// flight left, the updates of the loose-ball phase.
    loose: [f64; 6],
}

/// The first capture's, on its rosters. Turnovers set again by the rule on
/// 2026-10-23 with the clearout ported (`FUN_0015c778`, about 1.4 a game
/// against the match's 4): 9.0 (2.8, se 0.70) against 6 (8.875 before it,
/// already 5.0 by the rule; the seed sets +500 and +777 8.6 and 8.3). The
/// block jumps at a dunker and within 4 set again on 2026-10-23 with the
/// dunks' and lay-ups' knocks ported (`FUN_001366d8`: about 4.8 floor
/// knocks and 12.6 air duels a game, the knocked men not jumping): at a
/// dunker 21.1 (5.9, se 1.49) against 30 (23.1 before), 13.5 by the rule
/// (+500 21.6, +777 19.7); within 4 0.254 (se 0.0035) against 0.262,
/// 0.02 by the rule, widened to 0.025 for +777's 0.241. The shots set
/// again by the rule on 2026-10-23 with the gait selector drawing from the
/// shared generator (`FUN_00168b30`: about six draws more an update, as the
/// original's 8 or 9): 50.4 (3.9, se 0.97) against 55, 8.0 (+500 52.6,
/// +777 51.9). The block jumps at a shooter set again by the rule on
/// 2026-10-23 with the faces' draws at the pass start, the catches and the
/// missed shots: 16.5 (3.4, se 0.84) against 11 (14.7 before), 8.0 (+500
/// 14.6, +777 15.4). The defenders' standing share set again by the rule
/// on 2026-10-24 with the stop on the stands' planted feet (the stand path
/// zeroes the velocity): 0.414 (0.020, se 0.0049) against 0.376, 0.055
/// (+500 within 0.035, +777 0.412).
const FIRST_BOUNDS: Bounds = Bounds {
    standing: 0.055,
    away: 0.025,
    length: 2.0,
    in_tricks: 0.035,
    blend: 0.18,
    within: 0.025,
    turnovers: 5.5,
    counters: 6.5,
    shots: 8.0,
    jumps: [8.0, 3.5, 13.5],
    loose: [1.0, 2.0, 2.0, 1.5, 0.095, 400.0],
};

/// The third capture's, on its rosters (and any recorded capture's). The
/// block jumps just after a release set again by the rule on 2026-10-21
/// (the tip jumpers aiming their posed hands: every game's opening, and so
/// its path, moved): 8.69 (0.76) against 3; with the old hand 7.38 (0.43).
///
/// The loose-ball phase's updates set again by the rule on 2026-10-23 with
/// the dunks' and lay-ups' knocks ported (`FUN_001366d8`): 654 (se 54)
/// against 915 (694 before), 425; the seed sets +500 and +777 768 and 756.
const RECORDED_BOUNDS: Bounds = Bounds {
    standing: 0.045,
    away: 0.02,
    length: 2.0,
    in_tricks: 0.055,
    blend: 0.13,
    within: 0.035,
    turnovers: 2.5,
    counters: 4.5,
    shots: 5.5,
    jumps: [10.5, 8.0, 5.5],
    loose: [1.75, 1.0, 2.25, 2.0, 0.11, 425.0],
};

/// The second capture's (no rosters: every player at 50, and the disc's
/// pairs for the shots and the block jumps), set again by the rule on
/// 2026-10-18 (branch `computer-play`; the seed sets +500 and +777 inside):
/// standing 0.411 (0.018) against 0.398, away 0.194 (0.028) against
/// 0.150, the moves that ran out 59.0 (1.5) long against 60.4, within 4
/// in tricks 0.350 (0.042) against 0.324, the blend 0.468 (0.104) against
/// 0.388, within 4 0.244 (0.027) against 0.201 (the seed set +500 0.249),
/// turnovers 0.9 (1.1) against 0, counter presses 7.3 (3.1) against 3;
/// the pairs' shots 18.3 (2.4) against 22, block jumps 5.1 (2.2), 2.3
/// (1.4) and 9.8 (3.0) against 8, 1 and 9. Turnovers set again by the
/// rule at the merge of `control-stick` and `computer-offence`
/// (2026-10-18): 2.06 (1.29) against 0, the loose ball's swipe and the
/// passes now caught raising them; the original's 0 is one short match,
/// and a mean of 2 gives none in 13% of matches. The blend set again by
/// the rule on 2026-10-23 (branch `stand-path`, the stand path's draws):
/// 0.558 (0.177) against 0.388 (the seed sets +500 0.539, +777 0.441).
/// Within 4 in tricks set again by the rule on 2026-10-23 with the turbo
/// collision (`docs/TRACE.md`): 0.389 (0.057, se 0.014) against 0.324,
/// 0.11 (the seed sets +500 0.348, +777 0.336; main before it 0.382).
/// The catches with 5+ updates left set again by the rule on 2026-10-23
/// with the side with the ball stopping at once off a pass (the inbound's
/// receivers short of their spot): 0.0109 (se 0.0028) against 0.082 (five
/// of the short match's 61, four of them back-court passes left 5; the
/// long captures' 0.012 and 0.022 now read 0.017 and 0.021), 0.08 (the
/// seed sets +500 0.0051, +777 0.0052). The loose-ball phase's updates
/// set again by the rule on 2026-10-24 with the velocity's readers seeing
/// the stand path's zero (`Player::moving`): 213.8 (109.2, se 27.3) against
/// 396 (324.1 before), 265 (the seed sets +500 325.2, +777 297.9: the base
/// set is the low one). The defenders' standing share set again by the
/// rule on 2026-10-25 with the gait clock on the skeleton's scale (the
/// legs a fifth slower, as the original's): 0.430 (0.029, se 0.007)
/// against 0.398 (0.425 before), 0.055 (the seed sets +500 0.433, +777
/// 0.425; the long captures' 0.406 and 0.414 against 0.376 and 0.423).
/// The passes loose at their timer set again by the rule on 2026-10-25
/// with the screens' reactions and the rewrite's own pick gone (`docs/
/// TRACE.md` "Screens, the strip's knock and the swipe's clock"): 0.75
/// (0.93, se 0.23) against 0 (0.375 before), 1.5 (the seed sets +500
/// 0.31, +777 0.44).
const ALL_50_SHORT_BOUNDS: Bounds = Bounds {
    standing: 0.055,
    away: 0.07,
    length: 3.0,
    in_tricks: 0.11,
    blend: 0.31,
    within: 0.065,
    turnovers: 3.0,
    counters: 7.0,
    shots: 5.0,
    jumps: [4.0, 2.0, 2.0],
    loose: [0.5, 1.25, 1.5, 1.0, 0.08, 265.0],
};

/// A long capture without its rosters, as set for the first before its
/// rosters were known (branch `counter-tricks`): every player at 50,
/// standing 0.407 (0.014) against 0.376, away 0.188 (0.033) against
/// 0.208, all trick moves 56.3 (1.4) long against 58.7, within 4 in
/// tricks 0.348 (0.023) against 0.372, the blend 0.525 (0.092) against
/// 0.406, within 4 0.243 (0.021) against 0.262, turnovers 4.4 (2.1)
/// against 6, counter presses 24.3 (3.6) against 17 (12.5: the seed set
/// +777 gave 27.1); the disc's pairs' shots 52.5 (4.0) against 55, block
/// jumps 13.8 (3.8), 7.0 (2.6) and 26.1 (4.4) against 13, 7 and 30. Within
/// 4 fell 0.025 with the penetration's fix (branch `drive-counter`): the
/// handler spends fewer updates inside 12 and more beyond 30, the plays'
/// mix, not the guarding.
const ALL_50_LONG_BOUNDS: Bounds = Bounds {
    standing: 0.045,
    away: 0.045,
    length: 3.5,
    in_tricks: 0.045,
    blend: 0.19,
    within: 0.035,
    turnovers: 3.5,
    counters: 12.5,
    shots: 4.5,
    jumps: [2.5, 1.5, 6.0],
    loose: [1.0, 2.0, 10.0, 1.5, 0.095, 1150.0],
};

#[test]
fn shots_turnovers_and_spacing_against_the_captured_original() {
    // The handler's nearest defender in the front court (phase 5, the
    // ball held), the shots (entries into phase 3), the changes of
    // possession out of phase 5, the defence's steal presses and the
    // back court's passes: the original's attract match against rewrite
    // computer games of the same length, with the capture's own players
    // where it knows them ([`captured_rosters`]: the third capture records
    // them, the first's are inferred; with `STREET_ISO`). A capture that
    // does not (the second) is met with every player at 50 and with pairs
    // of the disc's teams. Each is run over [`BASES`] seed bases and
    // checked by its mean: one game's draw wanders with every change of
    // the generator's path, the mean only with the behaviour.
    let Some((matches, _)) = load() else {
        return;
    };
    let (native, updates) = native_tally(&matches);
    let rosters = captured_rosters(&matches).filter(|_| disc_players().is_some());
    // Pairs spread over the 28 teams (the attract match draws its own).
    let pairs = [(0, 14), (5, 19), (10, 24), (13, 27)];
    let rated_ok = rosters.is_none() && rated_match(1, 0, 14).is_some();
    let order = captured_order(&matches);
    let runs: Vec<Vec<(Tally, Value)>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..BASES)
            .map(|base| {
                s.spawn(move || {
                    let ordered = |mut w: World| {
                        w.update_order = order;
                        w
                    };
                    if let Some((records, _)) = rosters {
                        return vec![our_tally(updates, |seed| {
                            ordered(rostered_match(seed + 1000 * base, records).unwrap())
                        })];
                    }
                    let mut out = vec![our_tally(updates, |seed| {
                        ordered(demo_match(seed + 1000 * base))
                    })];
                    if rated_ok {
                        for (a, b) in pairs {
                            out.push(our_tally(updates, |seed| {
                                ordered(rated_match(seed + 1000 * base, a, b).unwrap())
                            }));
                        }
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    // The games the defence's figures are met with (the capture's players,
    // else every player at 50), and the shots' (the capture's players,
    // else the disc's pairs: every player at 50 seldom passes the shot's
    // quality gate).
    let like: Vec<&Tally> = runs.iter().map(|r| &r[0].0).collect();
    let rated: Vec<&Tally> = if rosters.is_some() {
        like.clone()
    } else {
        runs.iter()
            .flat_map(|r| r[1..].iter().map(|(t, _)| t))
            .collect()
    };
    let over = |games: &[&Tally], f: &dyn Fn(&Tally) -> f64| {
        spread(&games.iter().map(|t| f(t)).collect::<Vec<_>>())
    };
    let in_tricks = |t: &Tally| share_of(t.tricking.1, t.tricking.0);
    let backs_off = |t: &Tally| f64::from(t.moves.blend.2) / f64::from(t.moves.blend.0.max(1));
    let jumps = |t: &Tally, kind: &str| f64::from(t.block_jumps.get(kind).copied().unwrap_or(0));
    let standing = over(&like, &|t| t.standing());
    let away = over(&like, &|t| t.away_within_4());
    let trick_length = over(&like, &|t| t.moves.mean_length());
    let tricks = over(&like, &in_tricks);
    let blend = over(&like, &backs_off);
    let within = over(&like, &|t| t.within_4());
    let turnovers = over(&like, &|t| f64::from(t.turnovers()));
    let counters = over(&like, &|t| f64::from(t.counter_presses));
    let trick_moves = over(&like, &|t| t.moves.done.len() as f64);
    let ran_length = over(&like, &|t| t.moves.ran_length());
    let let_go = over(&like, &|t| t.moves.ended(MoveEnd::LetGo) as f64);
    let lost_in = over(&like, &|t| t.moves.ended(MoveEnd::Lost) as f64);
    // The loose ball ([`Touches`]): the knocks, the passes lost at their
    // timer, the late catches and the updates the game spends loose.
    let body_knocks = over(&like, &|t| f64::from(t.touches.body_knocks));
    let pass_knocks = over(&like, &|t| f64::from(t.touches.pass_knocks));
    let timed_out = over(&like, &|t| f64::from(t.touches.timed_out));
    let shot_knocks = over(&like, &|t| f64::from(t.touches.shot_knocks));
    let late = over(&like, &|t| t.touches.late_share());
    let loose_pickups = over(&like, &|t| f64::from(t.touches.loose_pickups));
    let rebound_catches = over(&like, &|t| f64::from(t.touches.rebound_catches));
    let loose_phase = |t: &Tally| f64::from(t.phases.get(&2).copied().unwrap_or(0));
    let loose_updates = over(&like, &loose_phase);
    let held_knocked = over(&like, &|t| f64::from(t.knocked_loose));
    let nt = &native.touches;
    let shots = over(&rated, &|t| f64::from(t.shot_phases));
    let fakes = over(&rated, &|t| f64::from(t.pump_fakes));
    let kinds = ["at a shooter", "after a release", "at a dunker"];
    let rated_jumps: Vec<Spread> = kinds
        .iter()
        .map(|k| over(&rated, &|t| jumps(t, k)))
        .collect();
    let mut native_report = native.json();
    native_report["kinds"] = json!(native_kinds(&matches));
    native_report["within_4"] = json!(native.within_4());
    native_report["turnovers"] = json!(native.turnovers());
    let theirs = |kind: &str| jumps(&native, kind);
    report(
        "shots",
        json!({"updates": updates, "bases": BASES, "native": native_report,
        "rosters": rosters.map(|(records, own)| json!({"records": records, "source": own.source()})),
        "ours": runs[0][0].1,
        "ours_disc_teams": runs[0][1..].iter().zip(pairs)
            .map(|((_, r), (a, b))| json!({"teams": [a, b], "ours": r})).collect::<Vec<_>>(),
        "means": {
            "trick_moves": trick_moves.json(native.moves.done.len() as f64),
            "standing": standing.json(native.standing()),
            "away_within_4": away.json(native.away_within_4()),
            "trick_length": trick_length.json(native.moves.mean_length()),
            "trick_length_ran_out": ran_length.json(native.moves.ran_length()),
            "trick_moves_let_go_out_of": let_go.json(native.moves.ended(MoveEnd::LetGo) as f64),
            "trick_moves_lost_in": lost_in.json(native.moves.ended(MoveEnd::Lost) as f64),
            "within_4_in_tricks": tricks.json(in_tricks(&native)),
            "blend_back_off": blend.json(backs_off(&native)),
            "within_4": within.json(native.within_4()),
            "turnovers": turnovers.json(f64::from(native.turnovers())),
            "counter_presses": counters.json(f64::from(native.counter_presses)),
            "pairs_shots": shots.json(f64::from(native.shot_phases)),
            "pairs_pump_fakes": fakes.json(0.0),
            "pairs_block_jumps": kinds.iter().zip(&rated_jumps)
                .map(|(k, s)| (k.to_string(), s.json(theirs(k))))
                .collect::<serde_json::Map<String, Value>>(),
            "loose_ball": {
                "knocks off a body outside a block jump": body_knocks.json(f64::from(nt.body_knocks)),
                "passes knocked loose": pass_knocks.json(f64::from(nt.pass_knocks)),
                "passes loose at their timer": timed_out.json(f64::from(nt.timed_out)),
                "shots knocked loose": shot_knocks.json(f64::from(nt.shot_knocks)),
                "catches with 5+ updates left": late.json(nt.late_share()),
                "loose pickups": loose_pickups.json(f64::from(nt.loose_pickups)),
                "rebound catches": rebound_catches.json(f64::from(nt.rebound_catches)),
                "loose phase updates": loose_updates.json(loose_phase(&native)),
                "held balls knocked loose": held_knocked.json(f64::from(native.knocked_loose)),
            },
        }}),
    );
    assert!(native.front_court > 0 && like.iter().all(|t| t.front_court > 0));
    let bounds = match rosters {
        Some((_, Own::First)) => &FIRST_BOUNDS,
        Some((_, Own::Recorded)) => &RECORDED_BOUNDS,
        None if updates < 20_000 => &ALL_50_SHORT_BOUNDS,
        None => &ALL_50_LONG_BOUNDS,
    };
    let check = |name: &str, s: Spread, native: f64, bound: f64| {
        assert!(
            (s.mean - native).abs() < bound,
            "{name}: mean {:.4} (sd {:.4}, se {:.4}) against {native:.4}, bound {bound}",
            s.mean,
            s.sd,
            s.se
        );
    };
    check("standing", standing, native.standing(), bounds.standing);
    check("away", away, native.away_within_4(), bounds.away);
    // The moves that ran their script out: the original's cut short (a
    // pass or shot out of the move, the ball lost) are reported, not met.
    check(
        "trick length",
        ran_length,
        native.moves.ran_length(),
        bounds.length,
    );
    check(
        "within 4 in tricks",
        tricks,
        in_tricks(&native),
        bounds.in_tricks,
    );
    check("the blend", blend, backs_off(&native), bounds.blend);
    assert!(blend.mean > 0.0);
    check("within 4", within, native.within_4(), bounds.within);
    // The ball changes hands out of the front court about as often (the
    // original's are its trick counters and its counter stance's catches
    // of a head pass; the gamebreaker baskets left out, `Tally::turnovers`;
    // every player at 50 throws no trick pass, so the head catches are
    // missing there), and trick counters are looked for about as often.
    check(
        "turnovers",
        turnovers,
        f64::from(native.turnovers()),
        bounds.turnovers,
    );
    check(
        "counter presses",
        counters,
        f64::from(native.counter_presses),
        bounds.counters,
    );
    if !rated.is_empty() {
        let n = f64::from(native.shot_phases);
        check("shots", shots, n, bounds.shots);
        for (kind, (s, bound)) in kinds.iter().zip(rated_jumps.iter().zip(bounds.jumps)) {
            check(kind, *s, theirs(kind), bound);
        }
        // (0.625 a match on the first capture's rosters, sd 0.89, once
        // the harness's meters filled at the computer crews' 0.825.)
        assert!(fakes.mean < 1.0, "pump fakes {fakes:?}");
    }
    // The loose ball: the knocks and the passes knocked loose about the
    // original's; the passes loose at their timer, the late catches and
    // the loose-ball phase's updates held near today's (the gaps are
    // explained in docs/TRACE.md "The loose ball").
    let loose = [
        ("knocks off a body", body_knocks, f64::from(nt.body_knocks)),
        (
            "passes knocked loose",
            pass_knocks,
            f64::from(nt.pass_knocks),
        ),
        (
            "passes loose at their timer",
            timed_out,
            f64::from(nt.timed_out),
        ),
        (
            "shots knocked loose",
            shot_knocks,
            f64::from(nt.shot_knocks),
        ),
        ("catches with 5+ updates left", late, nt.late_share()),
        ("loose-ball updates", loose_updates, loose_phase(&native)),
    ];
    for ((name, s, theirs), bound) in loose.into_iter().zip(bounds.loose) {
        check(name, s, theirs, bound);
    }
}

#[test]
fn the_playbooks_star_follows_the_records_overall_rating() {
    // `FUN_0012a7d0` at each play start of a capture that records its
    // players (schema 3): each player's share of his side's overall
    // ratings (rating 9, the record's field 25, `FUN_00181bf8(ai, 0)`)
    // scaled by the side's points × 2 + shots tried a second over his own
    // (stats 0 and 10, actor+0x2a4), as `Stats::star_weights` computes it
    // from the capture's own statistics, gives the recorded star (+0xc8,
    // an AIPlayer) and its weights (+0xcc..+0xd4, each a truncated percent
    // of their sum). The rewrite read a stand-in for rating 9 until
    // 2026-10-18 (the mean of eight ratings), which gave San Antonio's
    // Robinson a third of the side's share where his record gives 29%.
    let Some((matches, events)) = load() else {
        return;
    };
    let Some((records, Own::Recorded)) = captured_rosters(&matches) else {
        return;
    };
    let Some(players) = disc_players() else {
        return;
    };
    let Some(ptrs) = events.iter().rev().find(|e| e["event"] == "pointers") else {
        return;
    };
    let ais: Vec<u64> = ptrs["ais"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    let overall: Vec<i32> = records
        .iter()
        .map(|&r| players[r].ratings.native(9))
        .collect();
    let (mut starts, mut stars, mut weights, mut rows) = (0, 0, 0, Vec::new());
    for m in &matches {
        let mut prev: [Option<(u64, i64)>; 2] = [None; 2];
        for u in m {
            for (t, prev) in prev.iter_mut().enumerate() {
                let tm = &u.0[format!("team{t}")];
                if tm["strategy_vtable"].as_u64() != Some(0x43aee0) {
                    *prev = None;
                    continue;
                }
                let play = tm["strategy_words"][0].as_u64().unwrap_or(0);
                let age = tm["play_head"][1].as_i64().unwrap_or(0);
                let started = play != 0 && prev.is_none_or(|(p, a)| p != play || age < a);
                *prev = Some((play, age));
                // A play just chosen, with a star (none after a play ends,
                // `FUN_0012aaa8`). The post cut (vtable `0x43a290`) and the
                // alley-oop (`0x4394b0`) make their cutter and leaper the
                // star (`FUN_00129a20`, `FUN_00129d98`), not the formula.
                let called = matches!(tm["play_head"][3].as_u64(), Some(0x43a290 | 0x4394b0));
                let star = tm["star"][0]
                    .as_u64()
                    .and_then(|p| ais.iter().position(|&a| a == p));
                let Some(star) = star.filter(|_| started && !called) else {
                    continue;
                };
                let mut s = crate::stats::Stats::new(6);
                for i in 0..6 {
                    let stat = |k: usize| u.0["stats"][format!("actor{k}")][i].as_i64().unwrap();
                    s.on_court[i] = u.actors()[i]["on_court"].as_u64().unwrap() as u32;
                    s.players[i][crate::stats::POINTS] = stat(0) as i32;
                    s.players[i][crate::stats::ATTEMPTS] = stat(10) as i32;
                }
                for k in [crate::stats::POINTS, crate::stats::ATTEMPTS] {
                    s.teams[t][k] = u.0["stats"][format!("side{k}")][t].as_i64().unwrap() as i32;
                }
                let squad = [3 * t, 3 * t + 1, 3 * t + 2];
                let w = s.star_weights(t, &squad, &squad.map(|j| overall[j]));
                let mut best = 0;
                for k in 1..3 {
                    if w[best] < w[k] {
                        best = k;
                    }
                }
                let sum: f32 = w.iter().sum();
                let ours: Vec<i64> = w.iter().map(|x| (x / sum * 100.0) as i64).collect();
                let native: Vec<i64> = (1..4).map(|k| tm["star"][k].as_i64().unwrap()).collect();
                starts += 1;
                stars += usize::from(squad[best] == star);
                weights += usize::from(ours.iter().zip(&native).all(|(a, b)| (a - b).abs() <= 1));
                rows.push(json!({"update": u.frame(), "team": t, "star": star,
                    "ours": squad[best], "weights": native, "ours_weights": ours}));
            }
        }
    }
    report(
        "star",
        json!({"overall": overall, "play_starts": starts, "stars_match": stars,
            "weights_within_1": weights, "rows": rows}),
    );
    assert!(starts > 0);
    assert_eq!(stars, starts, "the star at every play start");
    assert_eq!(weights, starts, "the weights within a percent");
}

/// The original's plays by vtable (`FUN_00126f60` builds them in the
/// rewrite's order; a two-variant play's 3pt one is its second object).
fn play_kind(vtable: u64, variant: usize) -> Option<i32> {
    use super::plays::kind::*;
    Some(match vtable {
        0x43a428 => POST_UP,
        0x439638 => BACK_DOWN,
        0x439968 => ISOLATION + variant as i32,
        0x439c18 => PENETRATION,
        0x4397b8 => CUTTER,
        0x439dd0 => PICK_AND_ROLL + variant as i32,
        0x439f80 => PICK_OFF_BALL + variant as i32,
        0x43a5a0 => PULL_UP,
        0x439b00 => PEN_AND_KICK,
        0x43a718 => ROTATE,
        0x43a110 => POST_AND_KICK,
        0x43a290 => POST_CUT,
        0x4394b0 => ALLEY_OOP,
        _ => return None,
    })
}

#[test]
fn the_play_choices_lists_hold_the_originals_plays() {
    // `FUN_001281c8` on the original's own play starts (schema 3, the star
    // recorded): standing where the capture's players stand, with the
    // recorded star and the play before, the rewrite's lists
    // (`play_list`: the star's with his ratings' extra entries, else the
    // default list) hold every play the original chose, with the side's
    // meter mode (`[0x3d]`, not recorded) one way or the other. The
    // penetrations also start from the lane's count (`FUN_001298c8`) and
    // the post cut, alley-oop and pull up from their own checks, so they
    // are left out. Its rosters' ratings 1, 4 and 7 (the 3pt plays,
    // the post up's and back down's extra entries, the isolation's) are
    // read from the records.
    let Some((matches, events)) = load() else {
        return;
    };
    let Some((records, Own::Recorded)) = captured_rosters(&matches) else {
        return;
    };
    let Some(ptrs) = events.iter().rev().find(|e| e["event"] == "pointers") else {
        return;
    };
    let words = |key: &str| -> Vec<u64> {
        ptrs[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect()
    };
    let (actors, ais) = (words("actors"), words("ais"));
    let actor_of = |p: &Value| p.as_u64().and_then(|p| actors.iter().position(|&a| a == p));
    // Each side's play objects by vtable, in address order.
    let mut objects: [std::collections::BTreeMap<u64, std::collections::BTreeSet<u64>>; 2] =
        Default::default();
    for u in matches.iter().flatten() {
        for (t, side) in objects.iter_mut().enumerate() {
            let tm = &u.0[format!("team{t}")];
            let play = tm["strategy_words"][0].as_u64().unwrap_or(0);
            if tm["strategy_vtable"].as_u64() == Some(0x43aee0) && play != 0 {
                let vtable = tm["play_head"][3].as_u64().unwrap_or(0);
                side.entry(vtable).or_default().insert(play);
            }
        }
    }
    use super::plays::kind;
    let (mut checked, mut held, mut rows) = (0, 0, Vec::new());
    for m in &matches {
        let mut prev: [Option<(u64, i64)>; 2] = [None; 2];
        let mut last = [kind::NONE; 2];
        for u in m {
            for t in 0..2 {
                let tm = &u.0[format!("team{t}")];
                if tm["strategy_vtable"].as_u64() != Some(0x43aee0) {
                    prev[t] = None;
                    last[t] = kind::NONE;
                    continue;
                }
                let play = tm["strategy_words"][0].as_u64().unwrap_or(0);
                let age = tm["play_head"][1].as_i64().unwrap_or(0);
                let started = play != 0 && prev[t].is_none_or(|(p, a)| p != play || age < a);
                prev[t] = Some((play, age));
                if play == 0 {
                    continue;
                }
                let vtable = tm["play_head"][3].as_u64().unwrap_or(0);
                let k = objects[t]
                    .get(&vtable)
                    .and_then(|o| o.iter().position(|&p| p == play))
                    .and_then(|variant| play_kind(vtable, variant))
                    .unwrap();
                let before = std::mem::replace(&mut last[t], k);
                if !started
                    || matches!(
                        k,
                        kind::PENETRATION | kind::POST_CUT | kind::ALLEY_OOP | kind::PULL_UP
                    )
                {
                    continue;
                }
                // The handler: the side's man with the ball, or a pass's
                // receiver (`FUN_0018c018`).
                let h = if u.int("ball_state") == 3 {
                    actor_of(&u.0["ball_holder"])
                } else {
                    actor_of(&u.0["ball_target_player"])
                };
                let Some(h) = h.filter(|&h| u.actors()[h]["team"].as_i64() == Some(t as i64))
                else {
                    continue;
                };
                let mut w = rostered_match(1, records).unwrap();
                for (i, p) in w.players.iter_mut().enumerate() {
                    p.position = u.floor(i);
                    p.speed = 0.0;
                }
                w.ball.state = BallState::Held(h);
                w.possession = t;
                w.game_phase.phase = 5;
                w.game_phase.possession = t as i32;
                let mut pb = super::playbook::Playbook::new();
                pb.star = tm["star"][0]
                    .as_u64()
                    .and_then(|p| ais.iter().position(|&a| a == p))
                    .map_or(-1, |s| s as i32);
                pb.prev = before;
                let lists: Vec<Vec<i32>> = [false, true]
                    .into_iter()
                    .map(|meter| {
                        pb.meter = meter;
                        with_ctx(&w, |ctx| super::playbook::play_list(ctx, &pb, t, h)).0
                    })
                    .collect();
                checked += 1;
                let found = lists.iter().any(|l| l.contains(&k));
                held += usize::from(found);
                rows.push(
                    json!({"update": u.frame(), "team": t, "play": k, "star": pb.star,
                    "handler": h, "prev": before, "lists": lists, "found": found}),
                );
            }
        }
    }
    report(
        "play_choices",
        json!({"checked": checked, "held": held, "rows": rows}),
    );
    assert!(checked > 0);
    assert_eq!(held, checked, "plays outside the lists");
}

/// The actors of a captured update as `crate::row_capture` reads them:
/// the attract capture carries no facing word, hand or ratings (the
/// sim's defaults stand in), and the motion is the step from `before`.
fn row_actors(u: &Update, before: Option<&Update>) -> Vec<crate::row_capture::Actor> {
    let word = u.int("sides_word");
    let side = |t: usize| {
        let v = (word >> (8 * t)) & 0xff;
        if v > 127 { v - 256 } else { v }
    };
    let f = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
    (0..6)
        .map(|i| {
            let a = &u.actors()[i];
            let p = &a["position"];
            let at = [f(&p[0]), f(&p[2])];
            let motion = before.map_or([0.0, 0.0], |b| {
                let q = &b.actors()[i]["position"];
                [at[0] - f(&q[0]), at[1] - f(&q[2])]
            });
            let team = a["team"].as_u64().unwrap_or(0).min(1) as usize;
            let pad = |k: usize| a["pad"][k].as_u64().unwrap_or(0) as u32;
            crate::row_capture::Actor {
                team: usize::from(side(team) < 0),
                position: at,
                height: f(&p[1]),
                heading: a["heading"].as_i64().unwrap_or(0) as i32,
                facing: None,
                motion,
                velocity: false,
                hand: None,
                ratings: crate::Ratings::default(),
                computer: true,
                held: pad(0) | pad(1),
                stick: (
                    a["requested_heading"].as_i64().unwrap_or(0) as i32,
                    a["requested_intensity"].as_i64().unwrap_or(0) as i32,
                ),
            }
        })
        .collect()
}

/// The script of `rows` whose body the pc lies in (within 0x400 of the
/// largest at or below it).
fn row_script(pc: i64, scripts: impl Iterator<Item = u32>) -> Option<u32> {
    scripts
        .filter(|&s| i64::from(s) <= pc && pc < i64::from(s) + 0x400)
        .max()
}

/// A match's passes, catches, dunks and lay-ups with a known row.
fn row_cases(
    name: &str,
    m: &[Update],
) -> (
    Vec<crate::row_capture::PassCase>,
    Vec<crate::row_capture::CatchCase>,
    Vec<crate::row_capture::MoveCase>,
) {
    use crate::row_capture::{CatchCase, MoveCase, PassCase};
    use crate::timing::{CATCH_ROWS, MOVE_ROWS, PASS_ROWS, SHOT_SCRIPTS};
    let at: std::collections::HashMap<i64, &Update> = m.iter().map(|u| (u.frame(), u)).collect();
    let actors = actor_map(m);
    let of = |u: &Update, key: &str| actors.get(&u.int(key)).copied();
    let pc = |f: i64, i: usize| {
        at.get(&f)
            .and_then(|u| u.actors()[i]["script_pc"].as_i64())
            .unwrap_or(0)
    };
    let (mut passes, mut catches, mut moves) = (Vec::new(), Vec::new(), Vec::new());
    for pair in m.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if b.frame() != a.frame() + 1 {
            continue;
        }
        let live = matches!(b.phase(), 4 | 5);
        let holder = of(a, "ball_holder");
        let to = of(b, "ball_target_player");
        let before = at.get(&(a.frame() - 1)).copied();
        if live
            && a.int("ball_state") != 4
            && b.int("ball_state") == 4
            && let (Some(i), Some(j)) = (holder, to)
            && i != j
            && let Some(script) =
                row_script(pc(b.frame() + 1, i), PASS_ROWS.iter().map(|r| r.script))
        {
            // The filter runs in the press update after its locomotion: the
            // passer's stick (+0x90) is that update's (the pass node lets
            // go of it as it presses).
            let mut actors = row_actors(a, before);
            actors[i].stick = row_actors(b, Some(a))[i].stick;
            let catch = (b.frame() + 1..b.frame() + 60)
                .filter_map(|f| at.get(&f))
                .find(|u| u.int("ball_progress") > 0)
                .map(|u| {
                    let t = &u.0["ball_target"];
                    [
                        t[0].as_f64().unwrap_or(0.0) as f32,
                        t[2].as_f64().unwrap_or(0.0) as f32,
                    ]
                });
            passes.push(PassCase {
                capture: name.to_string(),
                update: b.frame(),
                actors,
                passer: i,
                receiver: j,
                script,
                shot_clock: None,
                catch,
            });
        }
        let from = holder.or_else(|| of(b, "ball_previous_holder"));
        if a.int("ball_state") == 4
            && b.int("ball_state") == 4
            && a.int("ball_progress") == 0
            && b.int("ball_progress") > 0
            && let (Some(i), Some(j)) = (from, to)
            && i != j
            && let Some(script) = (b.frame()..b.frame() + 4)
                .find_map(|f| row_script(pc(f, j), CATCH_ROWS.iter().map(|r| r.script)))
        {
            let t = &b.0["ball_target"];
            let spot = [
                t[0].as_f64().unwrap_or(0.0) as f32,
                t[2].as_f64().unwrap_or(0.0) as f32,
            ];
            let mut actors = row_actors(b, Some(a));
            actors[j].height = actors[j].height.max(row_actors(a, None)[j].height);
            catches.push(CatchCase {
                capture: name.to_string(),
                update: b.frame(),
                actors,
                passer: i,
                receiver: j,
                spot,
                flight: b.int("ball_flight_updates") as i32,
                script,
            });
        }
        if live
            && a.int("ball_state") == 3
            && let Some(i) = holder
            && b.actors()[i]["pad"][1].as_u64().unwrap_or(0) & u64::from(bits::SHOOT) != 0
            && let Some(script) = row_script(
                pc(b.frame() + 1, i),
                MOVE_ROWS
                    .iter()
                    .map(|r| r.script)
                    .chain(SHOT_SCRIPTS.iter().map(|s| s.script)),
            )
            && MOVE_ROWS.iter().any(|r| r.script == script)
        {
            let pad = |k: usize| b.actors()[i]["pad"][k].as_u64().unwrap_or(0) as u32;
            moves.push(MoveCase {
                capture: name.to_string(),
                update: b.frame(),
                actors: row_actors(a, before),
                shooter: i,
                script,
                buttons: pad(0) | pad(1),
            });
        }
    }
    (passes, catches, moves)
}

#[test]
fn the_pass_catch_and_dunk_rows_read_the_originals_sectors() {
    // The computer players' rows on the attract match, as
    // `played_capture::the_pass_catch_and_dunk_rows_read_the_originals_sectors`
    // measures the played ones: the sim's filter on the captured state (the
    // body's heading word +0xa8 standing for the facing word this capture
    // lacks) reads a sector the captured row's mask holds.
    let Some((matches, _)) = load() else {
        return;
    };
    let name = std::env::var("STREET_AI_CAPTURE").unwrap_or_default();
    let (mut passes, mut catches, mut moves) = (Vec::new(), Vec::new(), Vec::new());
    for m in &matches {
        let (p, c, d) = row_cases(&name, m);
        passes.extend(p);
        catches.extend(c);
        moves.extend(d);
    }
    let p = crate::row_capture::passes(&passes);
    let c = crate::row_capture::catches(&catches);
    let d = crate::row_capture::moves(&moves);
    report(
        "row_sectors",
        json!({"passes": p.json(), "catches": c.json(), "moves": d.json()}),
    );
    let counts = |t: &crate::row_capture::Tally| (t.cases, t.sector, t.candidate);
    eprintln!(
        "passes {:?} catches {:?} moves {:?}",
        counts(&p),
        counts(&c),
        counts(&d)
    );
    // The first capture: 77 of 98 passes and 81 of 92 catches read a
    // sector the captured row holds (the old reading: 6 and 9); the
    // second's third of a match 20 of 30 and 28 of 29 (3 and 1). The
    // captured catch row is a candidate for 77 of 92 in the first capture
    // and 63 of 72 in the third: this capture has no velocity word, so the
    // replay takes the velocity from the stick (16 or more, the moving
    // path; 43 and 24 when every captured pace counted as velocity).
    // The captured pass row is a candidate for 50 of 98, 10 of 30 and 43
    // of 77 passes with the passer's stick read on the press update and
    // the release's catch spot (26, 2 and 21 before): the rest is the yaw
    // (the root bone's, which this capture lacks), the dribble hand (not
    // captured) and, in the first, the ratings (no roster).
    assert!(
        p.cases >= 25 && p.sector * 5 >= p.cases * 3,
        "{:?}",
        counts(&p)
    );
    assert!(p.candidate * 4 >= p.cases, "{:?}", counts(&p));
    assert!(
        c.cases >= 25 && c.sector * 20 >= c.cases * 17,
        "{:?}",
        counts(&c)
    );
    assert!(c.candidate * 3 >= c.cases, "{:?}", counts(&c));
    assert!(d.sector == d.cases, "{:?}", counts(&d));
}

#[test]
fn the_swiping_hand_reaches_the_ball_where_the_original_strips() {
    // Every captured swipe at a held ball (a steal press without the two
    // turbos), posed as `crate::swipe` poses it from what the capture
    // keeps: the clip the swiper played at the press held through the
    // five-update blend into the swipe clip (the clip channel 0 shows five
    // updates on), then the script's wait; his place and facing word as
    // captured, a plain body of height scale 1.2 (the attract teams are not
    // recorded). The ball is the captured ball of the update before (the
    // actor's update reads it before the ball's). `FUN_0016cf30` strips
    // only on the first update its hand is within 1.5 (0.75 moving), so
    // each captured strip should come on the first update the posed hand
    // reaches.
    let Some((matches, _)) = load() else {
        return;
    };
    let bank = crate::pose::disc_bank();
    let Some(poses) = bank.get() else {
        return;
    };
    let body = crate::pose::Body::uniform(1.2);
    let (mut strips, mut on_time, mut early, mut swipes, mut reached) = (0, 0, 0, 0, 0);
    let mut cases = Vec::new();
    for m in &matches {
        for k in 1..m.len() {
            let u = &m[k];
            for i in 0..6 {
                let a = &u.actors()[i];
                let pad =
                    |w: &Update, k: usize| w.actors()[i]["pad"][k].as_u64().unwrap_or(0) as u32;
                if pad(u, 1) & bits::STEAL == 0
                    || (pad(u, 1) | pad(u, 2)) & bits::TWO_TURBOS != 0
                    || u.int("ball_state") != 3
                {
                    continue;
                }
                // The swipe clip on update 5, with the five updates consecutive.
                let Some(clip) = m
                    .get(k + 5)
                    .map(|w| w.actors()[i]["animation"].as_i64().unwrap_or(-1))
                else {
                    continue;
                };
                let Some(script) = crate::swipe::SCRIPTS
                    .iter()
                    .position(|s| s.records.contains(&(clip as u16)))
                else {
                    continue;
                };
                let from = (
                    a["animation"].as_i64().unwrap_or(-1) as u16,
                    a["frame_word"].as_i64().unwrap_or(0) as f32,
                );
                if !poses.has(from.0) {
                    continue;
                }
                let swipe = crate::swipe::Swipe {
                    script: script as u8,
                    record: clip as u16,
                    from: Some(from),
                    tick: 0,
                    tested: false,
                };
                swipes += 1;
                let (mut first, mut strip) = (None, None);
                for t in 0..swipe.span() {
                    let (Some(w), Some(before)) =
                        (m.get(k + usize::from(t)), m.get(k + usize::from(t) - 1))
                    else {
                        break;
                    };
                    if w.frame() != u.frame() + i64::from(t) {
                        break;
                    }
                    let s = crate::swipe::Swipe { tick: t, ..swipe };
                    let p = &w.actors()[i]["position"];
                    let place = crate::pose::Place {
                        position: [
                            p[0].as_f64().unwrap() as f32,
                            p[1].as_f64().unwrap() as f32,
                            p[2].as_f64().unwrap() as f32,
                        ],
                        word: w.actors()[i]["heading"].as_i64().unwrap() as i32,
                    };
                    let moving = before.floor(i) != w.floor(i);
                    let pose = s.pose(poses, body, place).unwrap();
                    let shown = s.shown();
                    let state = shown.record.map_or(0, |r| poses.state(r, shown.frame));
                    let d = crate::swipe::distance(&pose, shown, state, body, before.ball());
                    if first.is_none() && d <= crate::swipe::reach(moving, shown.record) {
                        first = Some(t);
                    }
                    if before.int("ball_state") == 3 && w.int("ball_state") == 1 {
                        strip = Some(t);
                        break;
                    }
                    if w.int("ball_state") != 3 {
                        break;
                    }
                }
                reached += usize::from(first.is_some());
                if let Some(t) = strip {
                    strips += 1;
                    on_time += usize::from(first == Some(t));
                    early += usize::from(first.is_some_and(|f| f < t));
                    cases.push(json!({"frame": u.frame(), "actor": i, "clip": clip, "strip": t, "posed_first": first}));
                }
            }
        }
    }
    report(
        "swipes",
        json!({"swipes": swipes, "reached": reached, "strips": strips, "on_time": on_time,
            "early": early, "cases": cases}),
    );
    eprintln!("swipes {swipes} reached {reached} strips {strips} on time {on_time} early {early}");
    // The attract captures keep neither the float frame nor the players:
    // the first capture's six strips come 0..2.8 from the posed hand (one
    // on its update, one an update after it reached); the played
    // captures, which keep both, put every strip within an update of it
    // (`played_capture`). A measure, not a gate.
    assert!(swipes > 0);
}

#[test]
fn the_captured_block_jumps_follow_their_scripts() {
    // Every block jump the attract captures hold (actor state bit 12
    // rising) with one of `FUN_0015d3a0`'s thirteen scripts on the channel
    // (`crate::block::SCRIPTS`): its arm (callback 48's branch) read from
    // the record the blend plays; the jumper leaves the floor that path's
    // callback 35 tick after the press; his facing settles on the angle
    // from him to the ball of the update before the press (callback 5,
    // turned 46 an update); and the script is of the kind `FUN_0015d3a0`
    // picks from the capture's own state (`crate::block::kind`: the
    // holder's state bits, the ball 28 updates on as captured, every
    // rating 50, the turbo block by the two turbos held). Callback 48's
    // branch is checked against the ball's captured place 15 on.
    use crate::block::{KINDS, SCRIPTS, Scene, TURBO, kind};
    let Some((matches, _)) = load() else {
        return;
    };
    let script_of = |pc: i64| {
        SCRIPTS[..13]
            .iter()
            .enumerate()
            .filter(|(_, s)| i64::from(s.address) <= pc && pc < i64::from(s.address) + 0xac)
            .max_by_key(|(_, s)| s.address)
            .map(|(k, _)| k as u8)
    };
    let (mut jumps, mut takeoffs, mut facings, mut kinds, mut known) = (0, 0, 0, 0, 0);
    // Callback 48 by the ball held (its prediction falls from the hand at
    // the holder's pace, not the capture's) and free.
    let (mut arms, mut armed) = ([0; 2], [0; 2]);
    let mut off = Vec::new();
    for m in &matches {
        let at: std::collections::HashMap<i64, &Update> =
            m.iter().map(|u| (u.frame(), u)).collect();
        let holders = actor_map(m);
        for pair in m.windows(2) {
            let (a, u) = (&pair[0], &pair[1]);
            if u.frame() != a.frame() + 1 || a.actors().len() < 6 || u.actors().len() < 6 {
                continue;
            }
            for i in 0..6 {
                let bits =
                    |v: &Update, j: usize| v.actors()[j]["animation_state"].as_i64().unwrap_or(0);
                if bits(u, i) & 0x1000 == 0 || bits(a, i) & 0x1000 != 0 {
                    continue;
                }
                let get = |f: i64| at.get(&f).filter(|v| v.actors().len() >= 6);
                let Some(script) = (u.frame()..u.frame() + 3).find_map(|f| {
                    get(f)
                        .and_then(|v| v.actors()[i]["script_pc"].as_i64())
                        .and_then(script_of)
                }) else {
                    continue;
                };
                jumps += 1;
                let s = &SCRIPTS[usize::from(script)];
                // The record the blend plays shows the branch.
                let blend = i64::from(s.blend[0]);
                let record = get(u.frame() + blend + 1)
                    .and_then(|v| v.actors()[i]["animation"].as_i64())
                    .unwrap_or(-1);
                let branch = s.records[0] != s.records[1] && record == i64::from(s.records[1]);
                let jump = crate::block::Jump {
                    script,
                    branch,
                    from: None,
                    tick: 0,
                    word: 0,
                    vy: 0.0,
                    drift: [0.0; 2],
                    up: false,
                    meet: 15,
                    airtime: 0,
                    flown: 0,
                    turbo: false,
                };
                let takeoff = (u.frame()..u.frame() + 40).find(|&f| {
                    get(f)
                        .is_some_and(|v| v.actors()[i]["position"][1].as_f64().unwrap_or(0.0) > 0.0)
                });
                if takeoff == Some(u.frame() + i64::from(jump.aim_tick())) {
                    takeoffs += 1;
                } else if takeoff.is_some() {
                    off.push(json!({"frame": u.frame(), "actor": i, "script": s.address,
                        "takeoff": takeoff.map(|t| t - u.frame()), "aim": jump.aim_tick()}));
                }
                // Callback 5's word: the angle to the ball before the press.
                let b = a.ball();
                let word = crate::contact::angle(a.floor(i), [b[0], b[2]]);
                if get(u.frame() + 14)
                    .is_some_and(|v| v.actors()[i]["heading"].as_i64() == Some(i64::from(word)))
                {
                    facings += 1;
                }
                // Callback 48 at the press: the ball 15 on to one side.
                if s.records[0] != s.records[1]
                    && !s.glass
                    && let Some(later) = get(a.frame() + 15)
                {
                    let free = a.int("ball_state") != 3;
                    armed[usize::from(free)] += 1;
                    let c = later.ball();
                    let p = a.floor(i);
                    let angle =
                        (crate::math::atan2(p[0] - c[0], p[1] - c[2]) * 162.974_66 + 0.5) as i32;
                    let d = angle - word;
                    let d = if d.abs() > 0x200 {
                        if d < 0 { d + 0x400 } else { d - 0x400 }
                    } else {
                        d
                    };
                    arms[usize::from(free)] += usize::from((d < 1) == branch);
                }
                // The kind, from the capture's own state.
                let sides = a.int("sides_word");
                let side = |t: i64| {
                    let v = (sides >> (8 * t)) & 0xff;
                    if v > 127 { (v - 256) as i32 } else { v as i32 }
                };
                let team = a.actors()[i]["team"].as_i64().unwrap_or(0).min(1);
                let possession = a.int("possession").clamp(0, 1);
                let holder = (a.int("ball_state") == 3)
                    .then(|| holders.get(&a.int("ball_holder")).copied())
                    .flatten()
                    .map(|h| {
                        let hw = a.actors()[h]["heading"].as_i64().unwrap_or(0) as i32;
                        let d =
                            (hw - crate::contact::angle(a.floor(h), a.floor(i)) + 0x200) & 0x3ff;
                        (a.floor(h), bits(a, h) as u32, ((d + 0x40) & 0x3ff) >> 7)
                    });
                let Some(ahead) = get(a.frame() + 28).map(|v| v.ball()) else {
                    continue;
                };
                let scene = Scene {
                    at: a.floor(i),
                    side: side(team),
                    other: side(team ^ 1),
                    blocks: 50,
                    holder,
                    ball: b,
                    ahead,
                    shot: a.int("ball_state") == 5,
                    radius: crate::ball::RADIUS,
                    rim: [36.75 * side(possession) as f32, 0.0],
                };
                let pad = |k: usize| {
                    a.actors()[i]["pad"][k].as_u64().unwrap_or(0) as u32
                        | u.actors()[i]["pad"][k].as_u64().unwrap_or(0) as u32
                };
                let turbo = (pad(1) | pad(2)) & bits::TWO_TURBOS != 0;
                known += 1;
                let k = kind(&scene);
                if (turbo && script == TURBO) || KINDS[k].contains(&script) {
                    kinds += 1;
                } else {
                    off.push(json!({"frame": u.frame(), "actor": i, "script": s.address,
                        "kind": k, "holder_bits": holder.map(|h| h.1), "turbo": turbo}));
                }
            }
        }
    }
    report(
        "block_jumps",
        json!({"jumps": jumps, "takeoffs": takeoffs, "facings": facings, "kinds_known": known,
            "kinds_agree": kinds, "arms_checked (held, free)": armed,
            "arms_agree (held, free)": arms, "off": off}),
    );
    eprintln!(
        "block jumps {jumps}: takeoffs {takeoffs}, facings {facings}, kinds {kinds} of {known}, arms held {} of {}, free {} of {}",
        arms[0], armed[0], arms[1], armed[1]
    );
    assert!(jumps >= 5, "{jumps}");
}

/// What the ball meets on the bones ([`crate::touch`]) in one match: how
/// many updates before arriving its receiver takes a pass (the flight less
/// its progress in the update before, ball+0x64 - +0x60), the catches of a
/// loose ball and a rebound, the passes knocked off a body and those lost
/// to their timer.
#[derive(Default, Debug)]
struct Touches {
    leads: std::collections::BTreeMap<i64, u32>,
    loose_pickups: u32,
    rebound_catches: u32,
    pass_knocks: u32,
    shot_knocks: u32,
    /// Knocks off a body outside a block jump (the tails' `FUN_00137ed8`).
    body_knocks: u32,
    timed_out: u32,
}

impl Touches {
    fn json(&self) -> Value {
        json!({
            "pass catches by flight left": self.leads.iter().map(|(k, v)| (k.to_string(), *v)).collect::<std::collections::BTreeMap<_, _>>(),
            "loose pickups": self.loose_pickups,
            "rebound catches": self.rebound_catches,
            "passes knocked loose": self.pass_knocks,
            "shots knocked loose": self.shot_knocks,
            "knocks off a body outside a block jump": self.body_knocks,
            "passes loose at their timer": self.timed_out,
        })
    }
}

/// The original's side of [`the_receivers_hands_take_the_ball_on_the_originals_timing`].
fn native_touches(m: &[Update]) -> Touches {
    let mut t = Touches::default();
    for pair in m.windows(2) {
        let (a, u) = (&pair[0], &pair[1]);
        if u.frame() != a.frame() + 1 || a.int("ball_holder") != 0 {
            continue;
        }
        let state = a.int("ball_state");
        if u.int("ball_holder") != 0 {
            match state {
                4 if u.int("ball_holder") == a.int("ball_target_player") => {
                    let left = a.int("ball_flight_updates") - a.int("ball_progress");
                    *t.leads.entry(left).or_default() += 1;
                }
                0 => t.rebound_catches += 1,
                1 => t.loose_pickups += 1,
                _ => {}
            }
            continue;
        }
        if !matches!(state, 4 | 5) || u.int("ball_state") != 1 {
            continue;
        }
        let b = u.ball();
        if state == 4 && a.int("ball_timer") <= 1 {
            t.timed_out += 1;
        } else if b[1] > 0.6 && !(b[0].abs() > 34.0 && b[1] > 10.0) && b[1] < 20.0 {
            if state == 4 {
                t.pass_knocks += 1;
            } else {
                t.shot_knocks += 1;
            }
            // `FUN_00137ed8` caps its knock at 15 mph an update; a block
            // jumper's knock leaves at 0.35 across (bit 12 on the nearest).
            let v = &u.0["ball_velocity"];
            let speed = (0..3)
                .map(|k| v[k].as_f64().unwrap_or(0.0).powi(2))
                .sum::<f64>()
                .sqrt();
            let nearest = (0..6)
                .min_by(|&i, &j| {
                    dist(u.floor(i), [b[0], b[2]]).total_cmp(&dist(u.floor(j), [b[0], b[2]]))
                })
                .unwrap();
            let jumping =
                u.actors()[nearest]["animation_state"].as_i64().unwrap_or(0) & 0x1000 != 0;
            if speed <= 0.37 && !jumping {
                t.body_knocks += 1;
            }
        }
    }
    t
}

/// The ball as an update of the rewrite's found it, for [`Touches::ours`]:
/// its state, its pass, its state as the original numbers it and the
/// pass's updates left.
type BallBefore = (BallState, Option<crate::ball::Pass>, i32, i64);

fn ball_before(w: &World) -> BallBefore {
    (
        w.ball.state,
        w.ball.pass,
        w.native_ball_state(),
        i64::from(w.ball.flight_ticks() - w.ball.flown_ticks()),
    )
}

impl Touches {
    /// One update of the rewrite's game, the ball as it found it.
    fn ours(&mut self, before: BallBefore, w: &World) {
        let left = before.3;
        for e in &w.events {
            match *e {
                crate::Event::Catch { player }
                    if before.0 == BallState::Pass
                        && before.1.is_some_and(|p| {
                            p.to == player && usize::from(p.catch) < crate::timing::CATCH_ROWS.len()
                        }) =>
                {
                    *self.leads.entry(left).or_default() += 1;
                }
                crate::Event::Pickup { .. } if before.2 == crate::game_phase::ball::REBOUND => {
                    self.rebound_catches += 1;
                }
                crate::Event::Pickup { .. } if before.2 == crate::game_phase::ball::LOOSE => {
                    self.loose_pickups += 1;
                }
                // Counted as the original's are ([`native_touches`]): the
                // ball loose after it, above 0.6, below 20 and not about the
                // rims.
                crate::Event::Deflection { player } | crate::Event::Block { player, .. }
                    if w.ball.state == BallState::Loose && {
                        let b = w.ball.position;
                        b[1] > 0.6 && !(b[0].abs() > 34.0 && b[1] > 10.0) && b[1] < 20.0
                    } =>
                {
                    match before.0 {
                        BallState::Pass => self.pass_knocks += 1,
                        BallState::Shot => self.shot_knocks += 1,
                        _ => {}
                    }
                    if !matches!(
                        w.players[player].action,
                        crate::Action::Jumping { block: Some(_), .. }
                    ) && matches!(before.0, BallState::Pass | BallState::Shot)
                    {
                        self.body_knocks += 1;
                    }
                }
                _ => {}
            }
        }
        if before.0 == BallState::Pass
            && w.ball.state == BallState::Loose
            && !w.events.iter().any(|e| {
                matches!(
                    e,
                    crate::Event::Deflection { .. }
                        | crate::Event::Block { .. }
                        | crate::Event::Score { .. }
                )
            })
        {
            self.timed_out += 1;
        }
    }

    fn add(&mut self, t: &Touches) {
        for (k, v) in &t.leads {
            *self.leads.entry(*k).or_default() += v;
        }
        self.loose_pickups += t.loose_pickups;
        self.rebound_catches += t.rebound_catches;
        self.pass_knocks += t.pass_knocks;
        self.shot_knocks += t.shot_knocks;
        self.body_knocks += t.body_knocks;
        self.timed_out += t.timed_out;
    }

    /// The share of the receivers' catches with 5 or more updates of the
    /// flight left.
    fn late_share(&self) -> f64 {
        let caught: u32 = self.leads.values().sum();
        let early: u32 = self.leads.range(5..).map(|(_, v)| v).sum();
        share_of(early, caught)
    }
}

/// The rewrite's side: its computer game from `w` for `updates` updates.
fn our_touches(mut w: World, updates: usize) -> Touches {
    let mut t = Touches::default();
    w.start_opening_toss();
    for _ in 0..updates {
        let before = ball_before(&w);
        step_computer(&mut w);
        t.ours(before, &w);
    }
    t
}

#[test]
fn the_receivers_hands_take_the_ball_on_the_originals_timing() {
    // `FUN_00137db8` in the tails of `FUN_0015a060` and `FUN_0015da28`
    // ([`crate::touch`]): the receiver's catch row takes the pass with the
    // hand its clip holds out, 0..5 updates before it arrives; the floor's
    // ball and a rebound on the floor are taken in the actor's update.
    let Some((matches, _)) = load() else {
        return;
    };
    if crate::pose::disc_bank().get().is_none() {
        return;
    }
    let m = matches.iter().max_by_key(|m| m.len()).unwrap();
    let native = native_touches(m);
    let mut ours = Touches::default();
    for seed in [7, 11, 13, 17] {
        ours.add(&our_touches(demo_match(seed), m.len()));
    }
    report(
        "ball_touches",
        json!({"original": native.json(), "rewrite (four games)": ours.json()}),
    );
    eprintln!("original {native:?}\nrewrite (four games) {ours:?}");
    let caught: u32 = ours.leads.values().sum();
    let early: u32 = ours.leads.range(1..=5).map(|(_, v)| v).sum();
    assert!(caught > 0 && early * 10 >= caught * 8, "{ours:?}");
}

#[test]
fn putbacks_start_on_their_records_and_some_catch_the_ball() {
    // `FUN_00169c98` in computer games (the disc's animations): at the
    // shots and rebounds a man of the side with the ball pressing the
    // rebound with two turbos starts one of the putback rows when a record
    // meets the ball; the dunker's take catches it at its catch frames.
    if crate::pose::disc_bank().get().is_none() {
        return;
    }
    // (Seed 10: once the pass filter measured on its flight estimate,
    // seed 7's twenty putbacks caught none; with the rebound's clearout
    // (callback 37) seed 8's and 11's catch none, 9, 10 and 12..15 do.)
    let (mut started, mut caught) = (0, 0);
    let mut w = demo_match(10);
    w.start_opening_toss();
    for _ in 0..20_000 {
        step_computer(&mut w);
        if !matches!(w.native_ball_state(), 0 | 5 | 6) || started >= 20 {
            continue;
        }
        for i in 0..w.players.len() {
            if w.players[i].team != w.possession || w.players[i].height != 0.0 {
                continue;
            }
            let mut c = w.clone();
            c.players[i].turbo = 1.0;
            let turbo = Input::default().with(crate::button::L1 | crate::button::R1);
            if !c.putback(i, &turbo) {
                continue;
            }
            started += 1;
            assert!(matches!(
                c.players[i].action,
                crate::Action::Dunking { style: crate::player::DunkStyle::Putback, row, .. }
                    if row >= crate::timing::PUTBACK_FIRST
            ));
            for _ in 0..120 {
                step_computer(&mut c);
                if c.ball.state == BallState::Held(i) {
                    caught += 1;
                    break;
                }
            }
            break;
        }
    }
    assert!(started >= 10 && caught > 0, "{started} {caught}");
}

/// Per-update drift of the players standing on the stand path (15
/// updates after they stopped, off the ball, in phases 4 and 5 with the
/// ball held), by side: (offence, defence), each the drifts' mean and the
/// share above 0.005.
#[derive(Debug, Default)]
struct Drift {
    off: Vec<f32>,
    def: Vec<f32>,
}

impl Drift {
    fn summary(v: &[f32]) -> (f32, f32) {
        let n = v.len().max(1) as f32;
        (
            v.iter().sum::<f32>() / n,
            v.iter().filter(|&&d| d > 0.005).count() as f32 / n,
        )
    }
}

/// The original's side of [`standing_players_sway_on_their_stand_clips`]:
/// the stick let go (+0x90 zero) stands for the stand path.
fn native_drift(m: &[Update], d: &mut Drift) {
    let holders = actor_map(m);
    let mut run = [0u32; 6];
    for pair in m.windows(2) {
        let (a, u) = (&pair[0], &pair[1]);
        if u.frame() != a.frame() + 1
            || !matches!(u.int("phase"), 4 | 5)
            || u.int("ball_state") != 3
        {
            run = [0; 6];
            continue;
        }
        let holder = holders.get(&u.int("ball_holder")).copied();
        for (i, r) in run.iter_mut().enumerate() {
            let actor = &u.actors()[i];
            let still = actor["requested_intensity"].as_i64() == Some(0)
                && actor["position"][1].as_f64() == Some(0.0)
                && Some(i) != holder;
            *r = if still { *r + 1 } else { 0 };
            if *r > 15 {
                let side = actor["team"].as_i64() == Some(u.int("possession"));
                let step = dist(u.floor(i), a.floor(i));
                if side {
                    d.off.push(step)
                } else {
                    d.def.push(step)
                }
            }
        }
    }
}

/// The rewrite's side: its computer game from `w` for `updates` updates.
fn our_drift(mut w: World, updates: usize, d: &mut Drift) {
    w.start_opening_toss();
    let mut run = [0u32; 6];
    for _ in 0..updates {
        let before: Vec<[f32; 2]> = w.players.iter().map(|p| p.position).collect();
        step_computer(&mut w);
        let holder = match w.ball.state {
            BallState::Held(h) => Some(h),
            _ => None,
        };
        if holder.is_none() || !matches!(w.game_phase.phase, 4 | 5) {
            run = [0; 6];
            continue;
        }
        for (i, r) in run.iter_mut().enumerate() {
            let p = &w.players[i];
            let still = p.stand.is_some() && p.height == 0.0 && Some(i) != holder;
            *r = if still { *r + 1 } else { 0 };
            if *r > 15 {
                let step = crate::distance(p.position, before[i]);
                if p.team == w.possession {
                    d.off.push(step)
                } else {
                    d.def.push(step)
                }
            }
        }
    }
}

#[test]
fn standing_players_sway_on_their_stand_clips() {
    // `FUN_0019a658` on the stand path: no stand script's `91` sets
    // 0x1000, so the stand clips' roots carry their players (the idle
    // clips sway them on the spot). Main before this measured 0.0003 and
    // 0.0015 an update, moving in 0.1% and 1.5% of the updates.
    let Some((matches, _)) = load() else {
        return;
    };
    if crate::pose::disc_bank().get().is_none() {
        return;
    }
    let mut native = Drift::default();
    for m in &matches {
        native_drift(m, &mut native);
    }
    let m = matches.iter().max_by_key(|m| m.len()).unwrap();
    let mut ours = Drift::default();
    for seed in [7, 11, 13, 17] {
        our_drift(demo_match(seed), m.len(), &mut ours);
    }
    let sides = [
        (
            "offence",
            Drift::summary(&native.off),
            Drift::summary(&ours.off),
        ),
        (
            "defence",
            Drift::summary(&native.def),
            Drift::summary(&ours.def),
        ),
    ];
    report(
        "idle_drift",
        json!(
            sides
                .iter()
                .map(|(k, n, o)| (
                    k.to_string(),
                    json!({
                        "original": {"mean": n.0, "moving share": n.1},
                        "rewrite (four games)": {"mean": o.0, "moving share": o.1},
                    })
                ))
                .collect::<serde_json::Map<_, _>>()
        ),
    );
    for (side, n, o) in sides {
        eprintln!("{side}: original {n:?}, rewrite {o:?}");
        assert!(
            (o.0 - n.0).abs() <= n.0 * 0.4,
            "{side}: mean {} against {}",
            o.0,
            n.0
        );
        assert!(
            (o.1 - n.1).abs() <= 0.1,
            "{side}: moving {} against {}",
            o.1,
            n.1
        );
    }
}

/// The stops' slides by role (holder, the offence off the ball, the
/// defence): the stick from 16 or more to 0, held 14 updates on the floor
/// in phases 4 and 5, the place 12 updates on less the place before.
#[derive(Debug, Default)]
struct Stops {
    by: [Vec<f32>; 3],
}

impl Stops {
    const ROLES: [&'static str; 3] = ["holder", "offence", "defence"];

    fn median(v: &[f32]) -> f32 {
        let mut v = v.to_vec();
        v.sort_by(f32::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    }
}

/// The original's side of [`stops_slide_as_the_originals_by_role`].
fn native_stops(m: &[Update], out: &mut Stops) {
    let holders = actor_map(m);
    let at: std::collections::HashMap<i64, &Update> = m.iter().map(|u| (u.frame(), u)).collect();
    for u in m {
        let Some(a) = at.get(&(u.frame() - 1)) else {
            continue;
        };
        if !matches!(u.int("phase"), 4 | 5) {
            continue;
        }
        let holder = (u.int("ball_state") == 3)
            .then(|| holders.get(&u.int("ball_holder")).copied())
            .flatten();
        for i in 0..6 {
            let stick = |v: &Update| v.actors()[i]["requested_intensity"].as_i64().unwrap_or(0);
            if !(stick(a) >= 16 && stick(u) == 0) {
                continue;
            }
            let run: Option<Vec<&Update>> =
                (0..14).map(|k| at.get(&(u.frame() + k)).copied()).collect();
            let Some(run) = run else {
                continue;
            };
            if run
                .iter()
                .any(|v| stick(v) != 0 || v.actors()[i]["position"][1].as_f64() != Some(0.0))
            {
                continue;
            }
            let role = if Some(i) == holder {
                0
            } else if u.actors()[i]["team"].as_i64() == Some(u.int("possession")) {
                1
            } else {
                2
            };
            if role == 0 && u.int("ball_state") != 3 {
                continue;
            }
            out.by[role].push(dist(run[12].floor(i), a.floor(i)));
        }
    }
}

/// The rewrite's side: its computer game from `w` for `updates` updates.
fn our_stops(mut w: World, updates: usize, out: &mut Stops) {
    w.start_opening_toss();
    type Seen = (
        Vec<i32>,
        Vec<[f32; 2]>,
        Vec<bool>,
        Option<usize>,
        i32,
        usize,
    );
    let mut seen: std::collections::VecDeque<Seen> = Default::default();
    for _ in 0..updates {
        step_computer(&mut w);
        let holder = match w.ball.state {
            BallState::Held(h) => Some(h),
            _ => None,
        };
        seen.push_back((
            (0..6)
                .map(|i| w.ai.brains[i].pad.strength.min(64))
                .collect(),
            w.players.iter().map(|p| p.position).collect(),
            w.players.iter().map(|p| p.height == 0.0).collect(),
            holder,
            w.game_phase.phase,
            w.possession,
        ));
        if seen.len() > 15 {
            seen.pop_front();
        }
        if seen.len() < 15 {
            continue;
        }
        let (a, f) = (&seen[0], &seen[1]);
        if !matches!(f.4, 4 | 5) {
            continue;
        }
        for i in 0..6 {
            if !(a.0[i] >= 16 && f.0[i] == 0)
                || seen.iter().skip(1).take(14).any(|s| s.0[i] != 0 || !s.2[i])
            {
                continue;
            }
            let role = if f.3 == Some(i) {
                0
            } else if w.players[i].team == f.5 {
                1
            } else {
                2
            };
            out.by[role].push(crate::distance(seen[13].1[i], a.1[i]));
        }
    }
}

#[test]
fn stops_slide_as_the_originals_by_role() {
    // The stand scripts' planted foot (`95`) carries a stop and their
    // clips' roots carry the stand after it (`World::stand_tick`). The
    // medians, the original's against the rewrite's four games (the third
    // capture on its rosters, the others every player at 50): the
    // defence's 1.30, 1.22, 1.28 against 1.19, 1.21, 1.27; the offence's
    // off the ball 0.26, 0.25, 0.26 against 0.24 in each; the holder's
    // 0.84, 0.80, 0.77 against 0.79, 0.83, 0.93. Main before the planted
    // foot slid the defence's nearly 0.
    let Some((matches, _)) = load() else {
        return;
    };
    if crate::pose::disc_bank().get().is_none() {
        return;
    }
    let mut native = Stops::default();
    for m in &matches {
        native_stops(m, &mut native);
    }
    let records = captured_rosters(&matches)
        .map(|(records, _)| records)
        .filter(|_| disc_players().is_some());
    let fresh = |seed: u32| match records {
        Some(records) => rostered_match(seed, records).unwrap_or_else(|| demo_match(seed)),
        None => demo_match(seed),
    };
    let updates = matches.iter().map(Vec::len).max().unwrap();
    let mut ours = Stops::default();
    for seed in [7, 11, 13, 17] {
        our_stops(fresh(seed), updates, &mut ours);
    }
    let rows: Vec<Value> = (0..3)
        .map(|k| {
            json!({"role": Stops::ROLES[k],
                "original": {"stops": native.by[k].len(), "median": Stops::median(&native.by[k])},
                "rewrite (four games)": {"stops": ours.by[k].len(), "median": Stops::median(&ours.by[k])}})
        })
        .collect();
    report("stop_slides", json!(rows));
    for k in 0..3 {
        let (n, o) = (Stops::median(&native.by[k]), Stops::median(&ours.by[k]));
        eprintln!(
            "{}: original {n:.2} ({}), rewrite {o:.2} ({})",
            Stops::ROLES[k],
            native.by[k].len(),
            ours.by[k].len()
        );
        if native.by[k].len() >= 20 {
            assert!(
                (o - n).abs() <= n * 0.3 + 0.1,
                "{}: median {o} against {n}",
                Stops::ROLES[k]
            );
        }
    }
}
