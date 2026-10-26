//! The person's shots, passes and catches against per-update captures of
//! the original running a match a person plays from the user's save state
//! 1 (`tools/pine_played_capture.py`; the keyboard driven by a play script:
//! jump shots inside and beyond the arc with the button let go before and
//! after the release gate, pump fakes, a fadeaway, back-court heaves,
//! passes, kick passes, dunks; then side fades run across the rim's line
//! with the button held past the gates' self-opening), the openings of
//! fresh points games started from the front end after a boot, the
//! person's passes in every direction, at rest, with turbos and on drives,
//! and the person defending and pressing Cross.
//! Skipped unless `STREET_PLAYED_CAPTURE` names the captures' JSON lines
//! (joined by `;`: the gate's fifteen, `street-played-pads-1`, `-2`,
//! `street-played-fades-1`, `street-played-fresh-1` .. `-9`,
//! `street-played-passes-1`, `-2` and `street-played-defence-1`; the
//! asserted counts are those of the whole set in docs/TRACE.md, the last in
//! "Who the person controls, and the stick"); with `STREET_PLAYED_CAPTURE_REPORT` set to a directory, each
//! test also writes its measurements there as JSON.
//!
//! The capture's team 0 attacks +x, as the simulation's does: positions
//! are compared as captured, and the original's heading units (0 toward
//! -z, 256 toward -x, 1024 a turn) become the simulation's heading
//! `u + 512` (the heading points (sin, cos), the word (-sin, -cos)).

use crate::{
    Action, BallState, Difficulty, Event, Goal, Hoop, Input, Phase, Player, Ratings, Rules, World,
    button, row_capture, shooting, timing,
};
use serde_json::{Value, json};
use std::collections::HashMap;

/// One update of a capture.
struct Update(Value);

impl Update {
    fn frame(&self) -> i64 {
        self.0["frame"].as_i64().unwrap()
    }
    fn phase(&self) -> i64 {
        self.0["phase"].as_i64().unwrap()
    }
    fn controllers(&self) -> Vec<i64> {
        self.0["controllers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect()
    }
    fn actor(&self, k: usize) -> &Value {
        &self.0["actors"][k]
    }
    fn ball(&self) -> &Value {
        &self.0["ball"]
    }
    fn holder(&self) -> u64 {
        self.ball()["holder"].as_u64().unwrap_or(0)
    }
    fn ball_state(&self) -> i64 {
        self.ball()["state"].as_i64().unwrap_or(-1)
    }
    fn word(&self, k: usize, key: &str) -> i64 {
        self.actor(k)[key].as_i64().unwrap_or(0)
    }
    fn pc(&self, k: usize) -> i64 {
        self.actor(k)["channels"][0]["pc"].as_i64().unwrap_or(0)
    }
    fn floor(&self, k: usize) -> [f32; 2] {
        let p = &self.actor(k)["position"];
        [p[0].as_f64().unwrap() as f32, p[2].as_f64().unwrap() as f32]
    }
    fn motion(&self, k: usize) -> f32 {
        let m = &self.actor(k)["motion"];
        let (x, z) = (m[0].as_f64().unwrap() as f32, m[2].as_f64().unwrap() as f32);
        libm::sqrtf(x * x + z * z)
    }
    fn distance(&self, k: usize) -> f32 {
        self.0["distances"][k].as_f64().unwrap() as f32
    }
    fn points(&self) -> [i64; 2] {
        let p = &self.0["stats"]["points"];
        [p[0].as_i64().unwrap_or(0), p[1].as_i64().unwrap_or(0)]
    }
}

/// What the School tests read of each sample of a School capture
/// (schema 3), the `repeat` samples included: the lesson manager's words
/// (game+0x4c0 + 4k), the user's lesson words (+0x54..), the script
/// manager's state (+0x50).
struct SchoolSample {
    frame: i64,
    repeat: bool,
    words: [i64; 17],
    user: Vec<u64>,
    script: i64,
}

impl SchoolSample {
    fn of(v: &Value) -> Option<Self> {
        let ints = |key: &str, n: usize| -> Option<Vec<i64>> {
            let a = v.get(key)?.as_array()?;
            Some(
                (0..n)
                    .map(|k| a.get(k).and_then(Value::as_i64).unwrap_or(0))
                    .collect(),
            )
        };
        let words = ints("lesson_words", 17)?;
        Some(Self {
            frame: v["frame"].as_i64()?,
            repeat: v["repeat"].as_bool() == Some(true),
            words: std::array::from_fn(|k| words[k]),
            user: v
                .get("lesson_user")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default(),
            script: ints("script_words", 12).map_or(0, |w| w[8]),
        })
    }
}

/// An actor's roster record: his team, the eleven ratings and the
/// persistent player id (field 0; none for the created and unique players
/// the capture does not resolve).
struct Record {
    team: usize,
    ratings: Vec<i64>,
    player: Option<usize>,
}

impl Record {
    /// The ratings as `street-game`'s setup reads them (`ai::capture`'s
    /// `disc_teams`).
    fn ratings(&self) -> Ratings {
        let r =
            |k: usize| (self.ratings.get(k).copied().unwrap_or(50) as f32 / 99.0).clamp(0.0, 1.0);
        Ratings {
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
            ..Ratings::default()
        }
    }
}

struct Capture {
    name: String,
    updates: Vec<Update>,
    /// A School capture's samples in the file's order, the `repeat` ones
    /// (the menu holding the game's counter) among them.
    school: Vec<SchoolSample>,
    at: HashMap<i64, usize>,
    actors: Vec<u64>,
    records: Vec<Record>,
    /// Each team's side byte (game+0x98): +1 attacks +x.
    sides: [i64; 2],
}

impl Capture {
    fn get(&self, frame: i64) -> Option<&Update> {
        self.at.get(&frame).map(|&i| &self.updates[i])
    }
    /// The actor index the holder pointer names.
    fn actor_of(&self, pointer: u64) -> Option<usize> {
        self.actors.iter().position(|&a| a == pointer)
    }
    /// The attacked rim of actor `k` in the capture's frame.
    fn rim(&self, k: usize) -> [f32; 2] {
        [36.75 * self.sides[self.records[k].team.min(1)] as f32, 0.0]
    }
}

/// The captures `STREET_PLAYED_CAPTURE` names, read once and shared by
/// every test (one parsed copy: the pass captures alone are 250 MB of
/// JSON, and a copy for each test running at once overran the memory).
fn load() -> Option<&'static Vec<Capture>> {
    static CAPTURES: std::sync::OnceLock<Option<Vec<Capture>>> = std::sync::OnceLock::new();
    CAPTURES.get_or_init(read_captures).as_ref()
}

fn read_captures() -> Option<Vec<Capture>> {
    let paths = std::env::var("STREET_PLAYED_CAPTURE").ok()?;
    let mut out = Vec::new();
    for path in paths.split(';').filter(|p| !p.trim().is_empty()) {
        let text = std::fs::read_to_string(path.trim()).unwrap();
        let mut updates = Vec::new();
        let mut school = Vec::new();
        let mut actors = Vec::new();
        let mut records = Vec::new();
        let mut sides = [1, -1];
        for line in text.lines() {
            let value: Value = serde_json::from_str(line).unwrap();
            if let Some(meta) = value.get("metadata") {
                assert_eq!(meta["serial"], "SLUS-20651");
            } else if value.get("event") == Some(&Value::from("pointers")) {
                actors = value["actors"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap())
                    .collect();
                records = value["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| Record {
                        team: r["team"].as_i64().unwrap_or(0).max(0) as usize,
                        ratings: r["ratings"]
                            .as_array()
                            .map(|a| a.iter().map(|v| v.as_i64().unwrap_or(50)).collect())
                            .unwrap_or_default(),
                        player: r["player_id"].as_u64().map(|v| v as usize),
                    })
                    .collect();
            } else if value.get("frame").is_some() {
                if value["torn"].as_bool() == Some(true) {
                    continue;
                }
                if let Some(sample) = SchoolSample::of(&value) {
                    school.push(sample);
                }
                if value["repeat"].as_bool() == Some(true) {
                    continue;
                }
                if updates.is_empty() {
                    let word = value["sides_word"].as_i64().unwrap_or(0x01ff01);
                    let byte = |b: i64| {
                        let v = (word >> (8 * b)) & 0xff;
                        if v > 127 { v - 256 } else { v }
                    };
                    sides = [byte(0), byte(1)];
                }
                updates.push(Update(value));
            }
        }
        assert_eq!(actors.len(), 6, "{path}: six actors");
        resolve_records(&mut records);
        let at = updates
            .iter()
            .enumerate()
            .map(|(i, u)| (u.frame(), i))
            .collect();
        out.push(Capture {
            name: std::path::Path::new(path.trim())
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            updates,
            school,
            at,
            actors,
            records,
            sides,
        });
    }
    (!out.is_empty()).then_some(out)
}

/// The match of save state 1, which every played capture so far plays (the
/// pads, the fades, the fresh games and the passes): the disc's players
/// 55, 56 and 100 against 82, 95 and 116. `tools/pine_played_capture.py`
/// read a record's fields only at a word's address until 2026-10-18, and
/// the fields of actors 0 and 4 lie 2 past one: the state's RAM names them
/// 55 and 95 (their records' +0x1c and field 0), with the disc's ratings.
const SAVE_STATE_1: [usize; 6] = [55, 56, 100, 82, 95, 116];

/// The records a capture left without their player (above), from the
/// disc's: only for save state 1's match, known by the four it named.
fn resolve_records(records: &mut [Record]) {
    if records.len() != 6
        || records.iter().all(|r| r.player.is_some())
        || !records
            .iter()
            .zip(SAVE_STATE_1)
            .all(|(r, id)| r.player.is_none_or(|p| p == id))
    {
        return;
    }
    let Some(players) = std::env::var_os("STREET_ISO").and_then(|path| {
        let mut iso = street_assets::iso::Iso::open(path).ok()?;
        street_assets::db::original_players(&mut iso).ok()
    }) else {
        return;
    };
    for (r, id) in records.iter_mut().zip(SAVE_STATE_1) {
        if r.player.is_some() {
            continue;
        }
        if let Some(p) = players.get(id).filter(|p| p.id as usize == id) {
            r.player = Some(id);
            r.ratings = p.ratings.iter().map(|&v| i64::from(v)).collect();
        }
    }
}

fn report(key: &str, value: Value) {
    let Some(path) = std::env::var_os("STREET_PLAYED_CAPTURE_REPORT") else {
        return;
    };
    let path = std::path::PathBuf::from(path).join(format!("{key}.json"));
    std::fs::write(path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
}

/// The shot script whose body the channel's pc lies in.
fn shot_script_at(pc: i64) -> Option<usize> {
    timing::SHOT_SCRIPTS
        .iter()
        .enumerate()
        .filter(|(_, s)| i64::from(s.script) <= pc && pc < i64::from(s.script) + 0x400)
        .max_by_key(|(_, s)| s.script)
        .map(|(i, _)| i)
}

fn pass_row_at(pc: i64) -> Option<usize> {
    timing::PASS_ROWS
        .iter()
        .enumerate()
        .filter(|(_, r)| i64::from(r.script) <= pc && pc < i64::from(r.script) + 0x400)
        .max_by_key(|(_, r)| r.script)
        .map(|(i, _)| i)
}

fn catch_row_at(pc: i64) -> Option<usize> {
    timing::CATCH_ROWS
        .iter()
        .enumerate()
        .filter(|(_, r)| i64::from(r.script) <= pc && pc < i64::from(r.script) + 0x400)
        .max_by_key(|(_, r)| r.script)
        .map(|(i, _)| i)
}

/// A person's shoot press in a capture.
struct Shot {
    capture: usize,
    press: i64,
    actor: usize,
    /// Updates from the press the held word kept 0x80: the button is up
    /// at this update of the script.
    held: i64,
    /// The script the channel ran and the update after the press it began
    /// (0 unless the actor had to finish something first).
    script: Option<usize>,
    started: i64,
    /// Updates from the press to the ball's state 5 with this shooter,
    /// before his next press.
    release: Option<i64>,
    /// Updates from the press to actor+0x1d0 rising (callback 20).
    fake_at: Option<i64>,
    /// Updates from the press to actor+0x1e4 turning 8 (callback 17).
    fade_at: Option<i64>,
    /// The stick (heading units, intensity) per update from the press.
    stick: Vec<(i32, i32)>,
    position: [f32; 2],
    heading: i32,
    /// The shooter on the update before the press, in the captures that
    /// carry the facing word actor+0xb0.
    running: Option<Running>,
    motion: f32,
    hand: i64,
    /// game+0x374 and the floor position at the release.
    distance: f32,
    z_abs: f32,
    made: Option<bool>,
    near: u32,
    contesting: u32,
    points: [i64; 2],
    target: i64,
    bounds: [i64; 2],
}

/// What `FUN_0016b4b8` reads of a shooter on the press: the facing word
/// (`FUN_0018d290`'s), whether he moves, the live hand. The capture's
/// update before the press holds them; on the press update he already
/// stands in the shot.
#[derive(Clone, Copy)]
struct Running {
    facing: i32,
    motion: f32,
    hand: i64,
}

fn human_shots(captures: &[Capture]) -> Vec<Shot> {
    let mut out = Vec::new();
    for (c, cap) in captures.iter().enumerate() {
        for u in &cap.updates {
            if !matches!(u.phase(), 4 | 5) {
                continue;
            }
            for (k, &controller) in u.controllers().iter().enumerate() {
                if controller == 4
                    || u.holder() != cap.actors[k]
                    || u.word(k, "pressed") & 0x80 == 0
                {
                    continue;
                }
                let press = u.frame();
                let mut held = 0;
                while cap
                    .get(press + held)
                    .is_some_and(|x| x.word(k, "held") & 0x80 != 0)
                {
                    held += 1;
                }
                let (mut script, mut started) = (None, 0);
                for d in 0..=3 {
                    if let Some(s) = cap.get(press + d).and_then(|x| shot_script_at(x.pc(k))) {
                        script = Some(s);
                        started = d;
                        break;
                    }
                }
                let mut release = None;
                let mut f = press;
                while let Some(x) = cap.get(f) {
                    if f > press && x.word(k, "pressed") & 0x80 != 0 {
                        break;
                    }
                    if x.ball_state() == 5
                        && x.ball()["shooter"].as_u64().unwrap_or(0) == cap.actors[k]
                    {
                        release = Some(f - press);
                        break;
                    }
                    if x.holder() != cap.actors[k] && x.ball_state() != 5 {
                        break;
                    }
                    if f - press > 150 {
                        break;
                    }
                    f += 1;
                }
                let (mut fake_at, mut fade_at) = (None, None);
                let (mut fake, mut fade) = (u.word(k, "pump_fake"), u.word(k, "fadeaway"));
                for d in 0..80 {
                    let Some(x) = cap.get(press + d) else {
                        break;
                    };
                    if d > 0 && x.word(k, "pressed") & 0x80 != 0 {
                        break;
                    }
                    let (pf, fd) = (x.word(k, "pump_fake"), x.word(k, "fadeaway"));
                    if fake_at.is_none() && pf == 1 && fake != 1 {
                        fake_at = Some(d);
                    }
                    if fade_at.is_none() && fd == 8 && fade != 8 {
                        fade_at = Some(d);
                    }
                    fake = pf;
                    fade = fd;
                }
                let stick = (0..held + 70)
                    .filter_map(|d| cap.get(press + d))
                    .map(|x| {
                        (
                            x.word(k, "requested_heading") as i32,
                            x.word(k, "requested_intensity") as i32,
                        )
                    })
                    .collect();
                let at_release = release.and_then(|r| cap.get(press + r));
                let (mut near, mut contesting) = (0, 0);
                if let Some(x) = at_release {
                    let me = x.floor(k);
                    for j in 0..6 {
                        if cap.records[j].team == cap.records[k].team {
                            continue;
                        }
                        let q = x.floor(j);
                        if crate::distance(me, q) <= 7.0 {
                            near += 1;
                            contesting += u32::from(x.word(j, "bits") & 0x1000 != 0);
                        }
                    }
                }
                let bounds = &u.0["stage_bounds"];
                out.push(Shot {
                    capture: c,
                    press,
                    actor: k,
                    held,
                    script,
                    started,
                    release,
                    fake_at,
                    fade_at,
                    stick,
                    position: u.floor(k),
                    heading: u.word(k, "heading") as i32,
                    running: cap.get(press - 1).and_then(|x| {
                        Some(Running {
                            facing: x.actor(k)["facing"].as_i64()? as i32,
                            motion: x.motion(k),
                            hand: x.word(k, "hand"),
                        })
                    }),
                    motion: u.motion(k),
                    hand: u.word(k, "hand"),
                    distance: at_release.map_or(u.distance(k), |x| x.distance(k)),
                    z_abs: at_release.map_or(u.floor(k)[1], |x| x.floor(k)[1]).abs(),
                    made: at_release.map(|x| x.0["shot_roll"][0].as_i64() == Some(1)),
                    near,
                    contesting,
                    points: u.points(),
                    target: u.0["target_points"].as_i64().unwrap_or(21),
                    bounds: [
                        bounds[0].as_i64().unwrap_or(2),
                        bounds[1].as_i64().unwrap_or(16),
                    ],
                });
            }
        }
    }
    out
}

/// The update the ball leaves on a script whose button comes up at `up`
/// (updates from the script's start): the gate (`86 19`) waits while the
/// button is held, until it opens by itself at `latest`; the release
/// follows `release - gate` updates later.
fn gated_release(s: &timing::ShotScript, up: i64) -> i64 {
    let (gate, release, latest) = (i64::from(s.gate), i64::from(s.release), i64::from(s.latest));
    if gate == 0 || up <= gate {
        release
    } else {
        let open = if latest > 0 { up.min(latest) } else { up };
        open + (release - gate)
    }
}

/// What the tables say a captured shot does: `(release, fake, fade)` in
/// updates from the press, or None for a press that started no shot
/// script the tables know.
fn expected(shot: &Shot) -> Option<(Option<i64>, Option<i64>, Option<i64>)> {
    let s = &timing::SHOT_SCRIPTS[shot.script?];
    let up = shot.held - shot.started;
    let check = i64::from(s.check);
    if check > 0 && up <= check && !s.name.starts_with("jumpshot_sidefade") {
        // The button up by the check: a pump fake, no release.
        return Some((None, Some(shot.started + check), None));
    }
    if shot.fade_at.is_some() {
        let fade = &timing::SHOT_SCRIPTS[timing::FADEAWAY];
        let entry = shot.started + check;
        return Some((
            Some(entry + gated_release(fade, up - check)),
            None,
            Some(entry),
        ));
    }
    Some((Some(shot.started + gated_release(s, up)), None, None))
}

#[test]
fn the_persons_shots_leave_at_the_scripts_moment_or_the_buttons_release() {
    // The shot script runs from the press update (its tick 0; the
    // channel's pc is inside it by the end of that update) and the ball
    // leaves at callback 21 (`release` from the start) when the button is
    // up before the gate `86 19`; held through the gate, the script waits
    // there until the button is up or the jump is 15 updates from
    // landing (`FUN_00174c10`), and the ball leaves `release - gate`
    // later. A tap shorter than the check is a pump fake at `check`
    // (actor+0x1d0 set by callback 20); the stick away from the basket
    // at the check turns the shot into `jumpshot_1_fadeaway` (actor+0x1e4
    // 8, callback 17), released 19 after. Every one of the person's
    // presses that started a shot script sits on those numbers.
    let Some(captures) = load() else {
        return;
    };
    let shots = human_shots(captures);
    let mut checked = 0;
    let mut off = Vec::new();
    let mut rows = Vec::new();
    for shot in &shots {
        let Some(script) = shot.script else {
            continue;
        };
        let Some((release, fake, fade)) = expected(shot) else {
            continue;
        };
        let s = &timing::SHOT_SCRIPTS[script];
        checked += 1;
        let got = (shot.release, shot.fake_at, shot.fade_at);
        rows.push(json!({
            "capture": captures[shot.capture].name, "press": shot.press, "actor": shot.actor,
            "script": s.name, "started": shot.started, "held": shot.held,
            "release": shot.release, "fake": shot.fake_at, "fade": shot.fade_at,
            "expected": [release, fake, fade], "distance": shot.distance, "made": shot.made,
        }));
        if got != (release, fake, fade) {
            off.push((s.name, shot.press, got, (release, fake, fade)));
        }
    }
    report(
        "shots",
        json!({"checked": checked, "off": off.len(), "shots": rows}),
    );
    // The two captures: 24 and 22 presses that started a known script.
    assert!(checked >= 20, "{checked}");
    assert!(off.is_empty(), "{off:?}");
    // Both regimes are in the evidence: the button up before the gate
    // (the table's release), and held past it (the gate's stall, capped
    // by the landing).
    let before = shots
        .iter()
        .filter(|s| {
            s.release.is_some() && s.script.is_some_and(|i| timing::SHOT_SCRIPTS[i].gate > 0)
        })
        .filter(|s| s.held - s.started <= i64::from(timing::SHOT_SCRIPTS[s.script.unwrap()].gate))
        .count();
    let after = shots
        .iter()
        .filter(|s| {
            s.release.is_some() && s.script.is_some_and(|i| timing::SHOT_SCRIPTS[i].gate > 0)
        })
        .filter(|s| s.held - s.started > i64::from(timing::SHOT_SCRIPTS[s.script.unwrap()].gate))
        .count();
    assert!(
        before >= 5 && after >= 5,
        "{before} before the gate, {after} past it"
    );
    assert!(shots.iter().filter(|s| s.fake_at.is_some()).count() >= 3);
}

#[test]
fn the_held_side_fades_leave_when_callback_18s_landing_opens_their_gate() {
    // Held past the gate, a side fade waits there until its jump (game
    // callback 18, `FUN_00172788`, at 15 or 10) is 15 (clips 320, 321)
    // or 5 (322, 323) updates from landing, 45 after it: the gate opens
    // by itself 45 updates after the press for `right_r` and `left_l`
    // (gate 33) and 50 for `right_l` and `left_r` (gate 28), and the ball
    // leaves `release - gate` later, at 51 and 54, however much longer
    // the button stays down.
    let Some(captures) = load() else {
        return;
    };
    let shots = human_shots(captures);
    let mut held = Vec::new();
    let mut off = Vec::new();
    for shot in &shots {
        let Some(i) = shot.script.filter(|_| shot.started == 0) else {
            continue;
        };
        let s = &timing::SHOT_SCRIPTS[i];
        if !s.name.starts_with("jumpshot_sidefade") || shot.held <= i64::from(s.latest) {
            continue;
        }
        let want = i64::from(s.latest) + i64::from(s.release) - i64::from(s.gate);
        held.push(json!({
            "capture": captures[shot.capture].name, "press": shot.press, "script": s.name,
            "held": shot.held, "release": shot.release, "expected": want,
        }));
        if shot.release != Some(want) {
            off.push((s.name, shot.press, shot.held, shot.release, want));
        }
    }
    report(
        "held_side_fades",
        json!({"held": held.len(), "off": off.len(), "shots": held}),
    );
    assert!(off.is_empty(), "{off:?}");
    // Both landings are in the evidence: 51 (gate 33 opening at 45) and
    // 54 (gate 28 opening at 50).
    for (latest, release) in [(45, 51), (50, 54)] {
        let n = held
            .iter()
            .filter(|h| h["expected"] == release)
            .filter(|h| {
                timing::SHOT_SCRIPTS
                    .iter()
                    .any(|s| s.name == h["script"] && s.latest == latest)
            })
            .count();
        assert!(n >= 2, "{n} side fades held past {latest}");
    }
}

/// Whether a script is one of the side hooks (`0x3d9038`); the table lists
/// their scripts twice (rows 8..10 and 24..26) and a channel's pc names
/// the later rows.
fn side_hook(script: usize) -> bool {
    timing::SIDE_HOOKS
        .iter()
        .any(|&h| timing::SHOT_SCRIPTS[h].script == timing::SHOT_SCRIPTS[script].script)
}

/// The fade of the 51% branch a captured side shot came from (`left_l`
/// with hand 2, `right_r` with hand 1, a side hook from either), or None
/// where the facing band and the hand decide the script (`left_r`,
/// `right_l`) or the script is no side shot.
fn drawn_fade(script: usize, hand: i64) -> Option<usize> {
    match script {
        timing::SIDEFADE_LEFT_L | timing::SIDEFADE_RIGHT_R => Some(script),
        _ if side_hook(script) && hand == 1 => Some(timing::SIDEFADE_RIGHT_R),
        _ if side_hook(script) => Some(timing::SIDEFADE_LEFT_L),
        _ => None,
    }
}

/// The simulation's stick for the original's heading units and intensity
/// (court axes).
fn stick(heading: i32, intensity: i32) -> [f32; 2] {
    let units = (heading + 512).rem_euclid(1024) as f32;
    let radians = units * (2.0 * std::f32::consts::PI / 1024.0);
    let len = intensity as f32 / 64.0;
    [libm::sinf(radians) * len, libm::cosf(radians) * len]
}

fn heading(units: i32) -> f32 {
    (units + 512).rem_euclid(1024) as f32 * (2.0 * std::f32::consts::PI / 1024.0)
}

/// The captured match at the press, the shooter and five standing
/// players at their captured spots, the ball in the shooter's hands.
/// Where the capture carries the facing word the shooter faces, moves and
/// holds his hand as on the update before the press.
fn replay_world(cap: &Capture, shot: &Shot, update: &Update) -> World {
    let (facing, motion, hand) = match shot.running {
        Some(r) => (r.facing, r.motion, r.hand),
        None => (shot.heading, shot.motion, shot.hand),
    };
    replay_world_for(cap, update, shot.actor, facing, motion, hand)
}

/// [`replay_world`] for actor `k` holding the ball, facing word `facing`,
/// moving `motion` an update with live hand `hand`.
fn replay_world_for(
    cap: &Capture,
    update: &Update,
    k: usize,
    facing: i32,
    motion: f32,
    hand: i64,
) -> World {
    let mut players = Vec::new();
    for j in 0..6 {
        let p = update.floor(j);
        let (facing, motion, hand) = if j == k {
            (facing, motion, hand)
        } else {
            (update.word(j, "heading") as i32, 0.0, 0)
        };
        let mut player = Player::new(cap.records[j].team.min(1), p, heading(facing))
            .with_ratings(cap.records[j].ratings());
        if j == k {
            player.speed = motion * crate::TICK_HZ as f32;
            // The captured velocity (actor+0x40): not zero, he is on the
            // locomotion's moving path (`Player::moving`).
            if motion != 0.0 {
                player.gait = crate::player::Gait::Walk;
            }
            if matches!(hand, 1 | 2) {
                player.gait_channel.hand = hand as u8;
            }
        }
        players.push(player);
    }
    let rules = Rules {
        difficulty: Difficulty::Easy,
        ..Rules::default()
    };
    let mut w = World::new(Hoop::native(), players, rules, 1);
    // The match's own update order where the state's RAM gives it.
    if order_known(cap) {
        w.update_order = SAVE_STATE_1_ORDER.map(|i| i as u8);
    }
    w.phase = Phase::Live;
    w.ball.hold(k);
    // The holder's side has the ball (the School's person plays team 1).
    w.possession = w.players[k].team;
    w.game_phase.possession = w.possession as i32;
    let mut controlled = [false; 2];
    for (j, &c) in update.controllers().iter().enumerate() {
        if c != 4 {
            controlled[w.players[j].team] = true;
        }
    }
    w.configure_meter_fill(controlled, None);
    // The meters as the update shows them (meter+0xc), and their pockets
    // (+0x20): a full meter turns the two turbos' shot into the
    // gamebreaker.
    for side in 0..2 {
        if let Some(m) = update.0[format!("meter{side}")].as_f64() {
            w.set_meter(side, m as f32);
        }
        w.teams[side].pocketed = update.0[format!("meter{side}_words")][8]
            .as_u64()
            .is_some_and(|p| p != 0);
    }
    w
}

#[test]
fn the_persons_shots_replayed_in_the_sim_release_fake_and_fade_alike() {
    // Each press replayed through World::step with the captured pad: the
    // button held as long as the person held it, the stick as the
    // original read it each update, the turbos and shoulder buttons as
    // he held them (two turbos on a full meter: the gamebreaker's shot,
    // the School's lessons). The sim's script draw is its own
    // random choice among the default three (or the side hooks), so a
    // different draw is moved onto the captured script's clock (and a
    // side fade the sim did not pick onto the side fade's); then the
    // release update, the pump fake and the fadeaway must agree with the
    // capture.
    let Some(captures) = load() else {
        return;
    };
    let shots = human_shots(captures);
    let mut checked = 0;
    let mut side_fades = 0;
    let mut held_side_fades = [0; 2];
    let mut choice_off = Vec::new();
    let mut decided = 0;
    let mut off = Vec::new();
    let mut rows = Vec::new();
    for shot in &shots {
        let Some(script) = shot.script.filter(|_| shot.started == 0) else {
            continue;
        };
        let cap = &captures[shot.capture];
        let Some(update) = cap.get(shot.press) else {
            continue;
        };
        let k = shot.actor;
        let mut w = replay_world(cap, shot, update);
        let input_at = |n: usize, w: &World| {
            let mut all = vec![Input::default(); w.players.len()];
            let (h, i) = shot.stick.get(n).copied().unwrap_or((0, 0));
            let held = cap
                .get(shot.press + n as i64)
                .map_or(0, |x| x.word(k, "held"));
            let mut mine = Input::moving(stick(h, i)).with(sim_buttons(held));
            if (n as i64) < shot.held {
                mine = mine.with(button::SHOOT);
            }
            all[k] = mine;
            all
        };
        w.step(&input_at(0, &w));
        let chosen = match w.players[k].action {
            Action::Shooting { script, .. } => Some(usize::from(script)),
            _ => None,
        };
        let captured = &timing::SHOT_SCRIPTS[script];
        let same_kind = chosen.is_some_and(|c| {
            c == script
                || (timing::DEFAULT_SHOTS.contains(&c) && timing::DEFAULT_SHOTS.contains(&script))
                || (side_hook(c) && side_hook(script))
        });
        let side_fade = captured.name.starts_with("jumpshot_sidefade");
        // The original's side branch where the capture carries the facing:
        // drawn (the fade or a hook) or decided by the band and the hand.
        let drawn = shot
            .running
            .and_then(|r| drawn_fade(script, r.hand))
            .filter(|&f| chosen.is_some_and(|c| c == f || side_hook(c)));
        decided += usize::from(
            same_kind
                && shot.running.is_some()
                && matches!(script, timing::SIDEFADE_LEFT_R | timing::SIDEFADE_RIGHT_L),
        );
        if !same_kind {
            choice_off.push((
                captured.name,
                shot.press,
                chosen.map(|c| timing::SHOT_SCRIPTS[c].name),
                shot.running.is_some(),
                drawn.is_some(),
                side_fade || side_hook(script),
            ));
            // Where the sim picked otherwise (another draw, or a capture
            // without the facing word), its shot is moved onto the
            // captured side fade's clock all the same, so their gate (and
            // callback 18's landing, `latest`) runs through World::step.
            if chosen.is_none() || !side_fade {
                continue;
            }
        }
        if let Action::Shooting {
            script: s,
            release,
            check,
            end,
            gate,
            latest,
            ..
        } = &mut w.players[k].action
        {
            *s = script as u8;
            *release = captured.release;
            *check = captured.check;
            *end = captured.end;
            *gate = captured.gate;
            *latest = captured.latest;
        }
        let (mut release, mut fake, mut fade) = (None, None, None);
        for n in 1..150usize {
            w.step(&input_at(n, &w));
            if release.is_none()
                && w.events
                    .iter()
                    .any(|e| matches!(e, Event::Shot { player, .. } if *player == k))
            {
                release = Some(n as i64);
            }
            if fake.is_none()
                && w.events
                    .iter()
                    .any(|e| matches!(e, Event::PumpFake { player } if *player == k))
            {
                fake = Some(n as i64);
            }
            if fade.is_none()
                && let Action::Shooting { fade: true, .. } = w.players[k].action
            {
                fade = Some(n as i64);
            }
            if release.is_some() || (fake.is_some() && n as i64 > shot.held + 40) {
                break;
            }
        }
        checked += 1;
        side_fades += usize::from(side_fade);
        if side_fade && shot.held > i64::from(captured.latest) {
            held_side_fades[usize::from(captured.latest == 50)] += 1;
        }
        let got = (release, fake, fade);
        let want = (shot.release, shot.fake_at, shot.fade_at);
        rows.push(json!({
            "capture": cap.name, "press": shot.press, "script": captured.name,
            "held": shot.held, "sim": [release, fake, fade], "original": [shot.release, shot.fake_at, shot.fade_at],
        }));
        if got != want {
            off.push((captured.name, shot.press, got, want));
        }
    }
    report(
        "replay",
        json!({"checked": checked, "side_fades": side_fades,
            "held_side_fades": held_side_fades, "off": off.len(),
            "choice_off": choice_off, "shots": rows}),
    );
    assert!(checked >= 20, "{checked}");
    assert!(side_fades >= 3, "{side_fades} side fades replayed");
    // Side fades held past both self-openings (45 and 50) replayed through
    // World::step leave on the captured update (51 and 54).
    assert!(
        held_side_fades.iter().all(|&n| n >= 2),
        "{held_side_fades:?} held side fades replayed (latest 45, 50)"
    );
    assert!(off.is_empty(), "{off:?}");
    // With the facing word, the running and the hand of the update before
    // the press, the sim picks the side fade the band and the hand decide
    // (`left_r` 2, `right_l` 8 in the side-fade capture) and differs only
    // on the original's 51% draws, within the drawn branch. Without them
    // (the pad captures carry no facing word and stop the shooter on the
    // press) only on the side shots.
    assert!(decided >= 10, "{decided} decided side fades picked");
    assert!(
        choice_off
            .iter()
            .all(|&(_, _, _, carried, drawn, side)| if carried { drawn } else { side }),
        "{choice_off:?}"
    );
}

#[test]
fn the_side_shots_follow_the_originals_facing_band_and_the_live_hand() {
    // `FUN_0016b4b8` on a running shooter: facing 160..300 units from the
    // rim (`FUN_0018d290`) with the live hand 1 (+0x25c) plays `left_r`,
    // with hand 2 `left_l` on 51 draws of 100 and a side hook on the
    // rest; 672..812 with hand 2 `right_l`, with hand 1 `right_r` or a
    // hook. Each press on the match as it stood the update before it (the
    // facing word as the shooter's heading, his hand, his speed): the
    // sim's facing angle is the original's, and its chooser picks the
    // captured script where the branch decides it, and the captured
    // branch's fade or a hook where it draws (both across 64 draws).
    let Some(captures) = load() else {
        return;
    };
    let shots = human_shots(captures);
    let hooks = timing::SIDE_HOOKS;
    // `left_r` and `right_l` decided; fades and hooks drawn.
    let mut decided = [0; 2];
    let mut drawn = [0; 2];
    let mut rows = Vec::new();
    for shot in &shots {
        let (Some(script), Some(running)) =
            (shot.script.filter(|_| shot.started == 0), shot.running)
        else {
            continue;
        };
        let cap = &captures[shot.capture];
        let Some(before) = cap.get(shot.press - 1) else {
            continue;
        };
        let k = shot.actor;
        let (facing, hand) = (running.facing, running.hand);
        let hook = side_hook(script);
        // The branch the captured script came from: a hook only from the
        // drawn ones, the left band's with hand 2, the right's with hand 1.
        let (fade, decides) = match drawn_fade(script, hand) {
            Some(fade) => (fade, false),
            None if matches!(script, timing::SIDEFADE_LEFT_R | timing::SIDEFADE_RIGHT_L) => {
                (script, true)
            }
            None => continue,
        };
        let world = || replay_world(cap, shot, before);
        // The original's angle, measured on its own court.
        let at = before.floor(k);
        let rim = cap.rim(k);
        let original = (facing
            - timing::angle_units(crate::math::atan2(at[0] - rim[0], at[1] - rim[1])))
            & 0x3ff;
        let w = world();
        let p = &w.players[k];
        let hoop = crate::target_hoop(p.team);
        let (angle, _) = timing::facing_angle(p.heading, p.position, w.rim_spot(hoop));
        let long = w.is_long_range(p.position, hoop);
        let moving = p.speed > 0.0;
        let picks: Vec<usize> = (0..64)
            .map(|n| {
                let mut w = world();
                for _ in 0..n {
                    w.rng.next_u32();
                }
                timing::shot_script(&mut w, k, 0, long, 0, false)
            })
            .collect();
        let name = timing::SHOT_SCRIPTS[script].name;
        rows.push(json!({
            "capture": cap.name, "press": shot.press, "script": name, "hand": hand,
            "angle": angle, "original": original,
            "picks": picks.iter().map(|&i| timing::SHOT_SCRIPTS[i].name).collect::<Vec<_>>(),
        }));
        assert_eq!(
            angle, original,
            "{name}: press {} of {}",
            shot.press, cap.name
        );
        assert!(moving, "{name}: press {} runs", shot.press);
        if decides {
            assert!(
                picks.iter().all(|&i| i == fade),
                "{name} (angle {angle}, hand {hand}): {:?}",
                rows.last()
            );
            decided[usize::from(fade == timing::SIDEFADE_RIGHT_L)] += 1;
        } else {
            assert!(
                picks.iter().all(|i| *i == fade || hooks.contains(i))
                    && picks.contains(&fade)
                    && picks.iter().any(|i| hooks.contains(i)),
                "{name} (angle {angle}, hand {hand}): {:?}",
                rows.last()
            );
            drawn[usize::from(hook)] += 1;
        }
    }
    report(
        "side_shots",
        json!({"decided": decided, "drawn": drawn, "shots": rows}),
    );
    // The side-fade capture: `left_r` 2 and `right_l` 8 decided, 7 side
    // fades and 6 side hooks drawn.
    assert!(decided[0] >= 2 && decided[1] >= 8, "{decided:?}");
    assert!(drawn[0] >= 7 && drawn[1] >= 6, "{drawn:?}");
}

#[test]
fn the_persons_passes_release_at_their_rows_callback_38() {
    // A person's pass press plays its row's script and the ball leaves
    // at the row's callback 38: `pass_fwd_5` 21, `pass_fwd_1` 18,
    // `pass_fwd_6` 10, `pass_left_or_right_4` 31; a kick pass (L3) draws
    // a rated row, `pass_sidekick` 26. The trick rows (flags 0x200000,
    // the passout specials, and 0x400000, the alley-oop passes) leave on
    // the passout legs and the lob to the leaper (`FUN_00162aa8`,
    // `the_lobs_to_leapers_are_timed_and_aimed_as_fun_00162aa8`), not
    // here; a press pressed again before the ball leaves (the pass
    // captures' driver doubled one key, passes-1's 22712 and 22713)
    // leaves on the later press.
    let Some(captures) = load() else {
        return;
    };
    let mut checked = 0;
    let mut kicks = 0;
    let mut off = Vec::new();
    for cap in captures {
        for u in &cap.updates {
            if !matches!(u.phase(), 4 | 5) {
                continue;
            }
            for (k, &controller) in u.controllers().iter().enumerate() {
                if controller == 4
                    || u.holder() != cap.actors[k]
                    || u.word(k, "pressed") & 0x100 == 0
                {
                    continue;
                }
                let press = u.frame();
                let Some(row) = cap.get(press + 1).and_then(|x| pass_row_at(x.pc(k))) else {
                    continue;
                };
                if timing::PASS_ROWS[row].flags & 0x60_0000 != 0 {
                    continue;
                }
                let mut f = press;
                while cap.get(f).is_some_and(|x| {
                    matches!(x.ball_state(), 3 | 4) && x.ball()["progress"].as_i64() == Some(0)
                }) && f - press < 80
                {
                    f += 1;
                }
                let Some(release) = cap
                    .get(f)
                    .filter(|x| x.ball_state() == 4)
                    .map(|_| f - press)
                else {
                    continue;
                };
                if (press + 1..f).any(|g| {
                    cap.get(g)
                        .is_some_and(|x| x.word(k, "pressed") & 0x100 != 0)
                }) {
                    continue;
                }
                checked += 1;
                let r = &timing::PASS_ROWS[row];
                kicks += usize::from(r.min_rating >= 0);
                if release != i64::from(r.release) {
                    off.push((row, release, r.release));
                }
            }
        }
    }
    report(
        "passes",
        json!({"checked": checked, "kicks": kicks, "off": off.len()}),
    );
    assert!(checked >= 5, "{checked}");
    assert!(kicks >= 1);
    assert!(off.is_empty(), "{off:?}");
}

/// What the person's control did in one update, by the original's rule
/// ([`crate::control`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Moved {
    /// Callback 38's swap: to the receiver as the ball leaves.
    Release,
    /// The take (slot 24), or a tip's take and shot in one update.
    Take,
    /// The game's update: to a computer holder once a hold is over.
    GameUpdate,
    /// Cross's switch (the action tap): on defence or in a loose ball or a
    /// shot, outside phases 0 and 1, the side not held (its pick is
    /// `the_persons_switches_go_to_fun_0012fd60s_pick`'s).
    Switch,
}

#[test]
fn the_persons_control_moves_where_the_original_moves_it() {
    // The person's actor (the controller word not 4) through every update of
    // the played captures, by the original's rule on the captured ball,
    // possession and pads: the game's update hands a computer holder on the
    // side with the ball the pad (`FUN_0013d868`); callback 38 hands it to
    // the receiver as the ball leaves (`FUN_00173570`: the update the
    // flight is set); a take hands it to the new holder (`FUN_001415c0`);
    // all three wait while a give 'n go holds (team+0xc8: the holder's
    // right-stick pass sets it; the side's shot, a computer's pass, the
    // ball lost or the right stick again off the ball clear it). The
    // rule's person must be the captured one in every update, and every
    // change one of its moves. While a staged gamebreaker plays (the
    // script manager's +0x50 state 3 to 7, schema 3) its pass is no row's
    // callback 38 and its catch no take: only the game's update moves the
    // pad, the update after the catch (the School's Level 2 lesson).
    let Some(captures) = load() else {
        return;
    };
    let staged = |u: &Update| {
        u.0["script_words"][8]
            .as_i64()
            .is_some_and(|s| (3..=7).contains(&s))
    };
    let mut updates = 0;
    let mut moved: HashMap<(Moved, i64), usize> = HashMap::new();
    let mut held = 0;
    let mut mid_pass = Vec::new();
    let mut off = Vec::new();
    for cap in captures {
        let side_of = |k: usize| k / 3;
        let single = |u: &Update, side: usize| {
            let c = u.controllers();
            let mut on = (side * 3..side * 3 + 3).filter(|&k| c.get(k).is_some_and(|&x| x != 4));
            match (on.next(), on.next()) {
                (Some(k), None) => Some(k),
                _ => None,
            }
        };
        let mut person: [Option<usize>; 2] = [None; 2];
        let mut hold = [false; 2];
        for pair in cap.updates.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if b.frame() != a.frame() + 1 {
                person = [single(b, 0), single(b, 1)];
                continue;
            }
            let (ba, bb) = (a.ball(), b.ball());
            let flight = |x: &Value| x["flight_updates"].as_i64().unwrap_or(0);
            let holder_a = cap.actor_of(a.holder());
            let holder_b = cap.actor_of(b.holder());
            let possession = a.0["possession"].as_i64().unwrap_or(-1);
            for side in 0..2 {
                let Some(mut p) = person[side].or(single(a, side)) else {
                    continue;
                };
                let was = p;
                let mut how = None;
                // The game's update, on the last update's ball.
                if a.phase() != 0
                    && possession == side as i64
                    && !hold[side]
                    && let Some(h) = holder_a.filter(|&h| side_of(h) == side && h != p)
                {
                    p = h;
                    how = Some(Moved::GameUpdate);
                }
                // The team's update: off the ball the hold ends.
                if possession != side as i64 {
                    hold[side] = false;
                }
                let pressed = b.word(p, "pressed") | a.word(p, "pressed");
                // The holder's right-stick pass starts a give 'n go.
                if holder_a == Some(p)
                    && a.ball_state() == 3
                    && b.ball_state() == 4
                    && pressed & 0x400 != 0
                {
                    hold[side] = true;
                }
                // The right stick again off the ball ends it.
                if hold[side]
                    && holder_a.is_some_and(|h| h != p)
                    && b.word(p, "pressed") & 0x400 != 0
                    && b.word(p, "pressed") & 0x100 == 0
                {
                    hold[side] = false;
                }
                let staged = staged(a) || staged(b);
                // Callback 38: the flight is set in its update.
                if !staged && b.ball_state() == 4 && flight(ba) == 0 && flight(bb) > 0 {
                    let receiver = cap.actor_of(bb["target_player"].as_u64().unwrap_or(0));
                    if let Some(passer) = holder_a.filter(|&h| side_of(h) == side) {
                        if passer == p && !hold[side] {
                            if let Some(r) = receiver.filter(|&r| r != p) {
                                p = r;
                                how = Some(Moved::Release);
                                // How far into the ball's state 4 (from the
                                // press) and how much flight is left.
                                let mut f = b.frame();
                                while cap.get(f - 1).is_some_and(|x| x.ball_state() == 4) {
                                    f -= 1;
                                }
                                mid_pass.push((b.frame() - f, flight(bb), b.phase()));
                            }
                        } else if passer != p {
                            hold[side] = false;
                        }
                    }
                }
                // Callback 21: the side's shot ends it; a tip's take and
                // shot in one update hands the shooter the pad.
                if b.ball_state() == 5 && a.ball_state() != 5 {
                    let shooter = cap.actor_of(bb["shooter"].as_u64().unwrap_or(0));
                    if let Some(s) = shooter.filter(|&s| side_of(s) == side) {
                        hold[side] = false;
                        if matches!(a.ball_state(), 0 | 1 | 6) && s != p {
                            p = s;
                            how = Some(Moved::Take);
                        }
                    }
                }
                // The take.
                if b.ball_state() == 3
                    && !staged
                    && let Some(h) = holder_b.filter(|&h| {
                        Some(h) != holder_a && side_of(h) == side && h != p && !hold[side]
                    })
                {
                    p = h;
                    how = Some(Moved::Take);
                }
                if hold[side] && p == was && holder_b.is_some_and(|h| h != p) {
                    held += 1;
                }
                let theirs = single(b, side);
                updates += 1;
                // The action tap ran before the game's update: on the last
                // update's possession and phase.
                let may_switch = how.is_none()
                    && p == was
                    && !hold[side]
                    && !matches!(a.phase(), 0 | 1)
                    && (possession != side as i64 || matches!(a.phase(), 2 | 3));
                if theirs.is_some_and(|t| t != p) && may_switch {
                    p = theirs.unwrap_or(p);
                    *moved.entry((Moved::Switch, b.phase())).or_default() += 1;
                } else if theirs.is_some_and(|t| t != p) {
                    off.push((cap.name.clone(), b.frame(), side, p, theirs));
                    p = theirs.unwrap_or(p);
                } else if p != was
                    && let Some(how) = how
                {
                    *moved.entry((how, b.phase())).or_default() += 1;
                }
                person[side] = Some(p);
            }
        }
    }
    let count = |m: Moved| {
        moved
            .iter()
            .filter(|((k, _), _)| *k == m)
            .map(|(_, &n)| n)
            .sum::<usize>()
    };
    let mut rows: Vec<_> = moved.iter().map(|(&(m, ph), &n)| (m, ph, n)).collect();
    rows.sort();
    report(
        "control_hand_off",
        json!({"updates": updates, "moves": format!("{rows:?}"),
            "release": count(Moved::Release), "take": count(Moved::Take),
            "game_update": count(Moved::GameUpdate), "switch": count(Moved::Switch),
            "held_off_the_holder": held,
            "release_into_state_4": format!("{:?}", {
                let mut c: HashMap<i64, usize> = HashMap::new();
                for &(into, _, _) in &mid_pass { *c.entry(into).or_default() += 1; }
                let mut v: Vec<_> = c.into_iter().collect(); v.sort(); v
            }),
            "off": off.len(), "first_off": format!("{:?}", off.first())}),
    );
    assert!(
        off.is_empty(),
        "{} of {updates}: {:?}",
        off.len(),
        &off[..off.len().min(8)]
    );
    assert!(count(Moved::Release) >= 40, "{rows:?}");
    assert!(count(Moved::Take) >= 60, "{rows:?}");
}

#[test]
fn the_persons_switches_go_to_fun_0012fd60s_pick() {
    // A switch is a change of the person's actor with no take, release or
    // shot in its update (the action tap's Cross, `FUN_0012f2c8` case 9).
    // `FUN_0012fd60`'s pick reads the tables the last update built
    // (game+0x374, the capture's distances sampled after it; the places and
    // the ball as it began, the sample before): among the person's computer
    // teammates the least of his distance to a flying pass's receiver,
    // else to the rim the offence attacks when a shot is up that is the
    // side's, falling or above 8, or the holder dunks or lays up, or the
    // person's player is his side's nearest the ball, else to the ball; 3
    // more when fallen (channel 0's flag 0x40). The 20 off the screen
    // (`FUN_00171920`, the camera's projection) cannot be read from a
    // capture: a pick that differs counts as screened when 20 more on the
    // computed winner gives the captured one. That 20 can only reorder two
    // teammates less than 20 apart, so a switch with one teammate to pick
    // or two 20 or more apart is decided whatever the camera shows: those
    // must all be exact. How often the camera's term decides the rest is
    // the person's play (a story game's person, switching all through his
    // defence, meets it in a sixth of his switches), not the rule's.
    let Some(captures) = load() else {
        return;
    };
    let mut switches = 0;
    let mut screened = 0;
    let (mut decided, mut decided_exact) = (0, 0);
    let mut by: HashMap<&str, usize> = HashMap::new();
    let mut off = Vec::new();
    for cap in captures {
        for u in &cap.updates {
            let (Some(a), Some(t)) = (cap.get(u.frame() - 1), cap.get(u.frame() - 2)) else {
                continue;
            };
            let person = |x: &Update| {
                let c = x.controllers();
                let on: Vec<usize> = (0..6).filter(|&k| c[k] != 4).collect();
                (on.len() == 1).then(|| on[0])
            };
            let (Some(pa), Some(pb)) = (person(a), person(u)) else {
                continue;
            };
            if pa == pb {
                continue;
            }
            let (ba, bb) = (a.ball(), u.ball());
            let flight = |x: &Value| x["flight_updates"].as_i64().unwrap_or(0);
            let took = u.ball_state() == 3 && cap.actor_of(u.holder()) != cap.actor_of(a.holder());
            let released = u.ball_state() == 4 && flight(ba) == 0 && flight(bb) > 0;
            let shot = u.ball_state() == 5 && a.ball_state() != 5;
            if took || released || shot {
                continue;
            }
            switches += 1;
            let side = pa / 3;
            let squad: Vec<usize> = (side * 3..side * 3 + 3).collect();
            let ball = {
                let p = &t.ball()["position"];
                [p[0].as_f64().unwrap() as f32, p[2].as_f64().unwrap() as f32]
            };
            let dist = |p: [f32; 2], q: [f32; 2]| crate::distance(p, q);
            let state = a.ball_state();
            let possession = a.0["possession"].as_i64().unwrap_or(-1);
            let vy = ba["velocity"][1].as_f64().unwrap_or(0.0);
            let y = ba["position"][1].as_f64().unwrap_or(0.0);
            let mut by_rim = state == 5 && (possession == side as i64 || vy < 0.0 || y > 8.0);
            let mut receiver = None;
            match cap.actor_of(a.holder()) {
                None if state == 4 => {
                    receiver = cap.actor_of(ba["target_player"].as_u64().unwrap_or(0));
                }
                None => {}
                Some(h) => by_rim |= a.word(h, "bits") & (1 << 7 | 1 << 13) != 0,
            }
            let nearest = squad
                .iter()
                .copied()
                .min_by(|&p, &q| dist(t.floor(p), ball).total_cmp(&dist(t.floor(q), ball)));
            let rule = if receiver.is_some() {
                "receiver"
            } else if by_rim || nearest == Some(pa) {
                "rim"
            } else {
                "ball"
            };
            let score = |k: usize| {
                let mut s = match receiver {
                    Some(r) => dist(t.floor(k), t.floor(r)),
                    None if rule == "rim" => a.distance(k),
                    None => dist(t.floor(k), ball),
                };
                if a.actor(k)["channels"][0]["flags"].as_i64().unwrap_or(0) & 0x40 != 0 {
                    s += 3.0;
                }
                s
            };
            let c = a.controllers();
            let mut best: Option<(usize, f32)> = None;
            let mut scores = Vec::new();
            for &k in &squad {
                if c[k] != 4 {
                    continue;
                }
                let s = score(k);
                scores.push(s);
                if s < best.map_or(200.0, |(_, b)| b) {
                    best = Some((k, s));
                }
            }
            scores.sort_by(f32::total_cmp);
            if scores.len() < 2 || scores[1] - scores[0] >= 20.0 {
                decided += 1;
                decided_exact += usize::from(best.is_some_and(|(k, _)| k == pb));
            }
            *by.entry(rule).or_default() += 1;
            let Some((won, _)) = best else {
                off.push((cap.name.clone(), u.frame(), rule, pa, pb, None));
                continue;
            };
            if won != pb {
                // Off the screen the winner would score 20 more: does the
                // captured pick win then?
                let flips = squad
                    .iter()
                    .copied()
                    .filter(|&k| c[k] == 4)
                    .min_by(|&p, &q| {
                        let s = |k: usize| score(k) + if k == won { 20.0 } else { 0.0 };
                        s(p).total_cmp(&s(q))
                    })
                    == Some(pb);
                if flips {
                    screened += 1;
                } else {
                    off.push((cap.name.clone(), u.frame(), rule, pa, pb, Some(won)));
                }
            }
        }
    }
    let exact = switches - screened - off.len();
    report(
        "switches",
        json!({"switches": switches, "exact": exact, "off_screen": screened,
            "decided_by_the_rule_alone": decided, "decided_exact": decided_exact,
            "by": format!("{:?}", {
            let mut v: Vec<_> = by.iter().map(|(k, n)| (*k, *n)).collect(); v.sort(); v
        }), "off": off.len(), "first_off": format!("{:?}", &off[..off.len().min(6)])}),
    );
    eprintln!(
        "switches {switches}: exact {exact}, off the screen {screened}, unexplained {}; decided by the rule alone {decided}, exact {decided_exact}",
        off.len()
    );
    if switches > 0 {
        // (2026-10-21, the 23 captures: 78 of 372 decided, 76 exact; of
        // the 294 the camera may decide, 249 exact and 45 the other.)
        assert!(decided * 5 >= switches, "{decided} of {switches} decided");
        assert!(decided_exact + 3 >= decided, "{decided_exact} of {decided}");
        // The camera follows the play, so the winner, nearer it, is mostly
        // on the screen.
        let undecided = switches - decided;
        assert!(
            (exact - decided_exact) * 4 >= undecided * 3,
            "{} of {undecided} undecided exact",
            exact - decided_exact
        );
        assert!(
            off.len() <= 3,
            "{} of {switches}: {:?}",
            off.len(),
            &off[..off.len().min(8)]
        );
    }
}

#[test]
fn the_persons_d_pad_words_are_the_pad_builders() {
    // The person's stick words (actor+0x88, +0x90) on his own pad, less the
    // camera's yaw word of the update before (the camera updates after the
    // actors; the captures from schema 2 keep its yaw): one of the D-pad's
    // eight as `input::dpad_words` makes them, at its strength
    // (`FUN_001587d8`; the keys are the D-pad). Not while his AI's frame
    // drives him (a pass's receiver, a restart, outside ±40.5 by ±25).
    let Some(captures) = load() else {
        return;
    };
    use crate::input::{dpad, dpad_words};
    let combos = [
        dpad::DOWN,
        dpad::DOWN | dpad::RIGHT,
        dpad::RIGHT,
        dpad::UP | dpad::RIGHT,
        dpad::UP,
        dpad::UP | dpad::LEFT,
        dpad::LEFT,
        dpad::DOWN | dpad::LEFT,
    ]
    .map(dpad_words);
    let (mut checked, mut exact, mut near) = (0, 0, 0);
    for cap in captures {
        for pair in cap.updates.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let Some(yaw) = a.0["camera_yaw"].as_f64() else {
                continue;
            };
            if b.frame() != a.frame() + 1 || b.phase() == 6 {
                continue;
            }
            let yaw = ((yaw as f32 * 162.974_66 + 0.5) as i32) & 0x3ff;
            for (k, &c) in b.controllers().iter().enumerate() {
                let receiver = |x: &Update| {
                    x.ball_state() == 4
                        && cap.actor_of(x.ball()["target_player"].as_u64().unwrap_or(0)) == Some(k)
                };
                let [x, z] = b.floor(k);
                if c == 4
                    || receiver(a)
                    || receiver(b)
                    || x.abs() > 40.5
                    || z.abs() > 25.0
                    || b.word(k, "ai_forced") != 0
                {
                    continue;
                }
                let strength = b.word(k, "requested_intensity") as i32;
                if strength != 64 && strength != 90 {
                    continue;
                }
                let word = (b.word(k, "requested_heading") as i32 - yaw) & 0x3ff;
                checked += 1;
                if combos.contains(&(word, strength)) {
                    exact += 1;
                } else if combos
                    .iter()
                    .any(|&(w, s)| s == strength && crate::timing::angle_diff(w, word).abs() <= 2)
                {
                    near += 1;
                }
            }
        }
    }
    report(
        "d_pad_words",
        json!({"checked": checked, "exact": exact, "within_2": near}),
    );
    if checked > 0 {
        assert!(
            exact * 100 >= checked * 98,
            "{exact} of {checked}, {near} near"
        );
    }
}

#[test]
fn the_persons_and_the_computers_pass_flights_are_timed_by_the_originals_rule() {
    // As `ai::capture`'s pass_flights_are_timed_by_the_originals_rule, on
    // the played captures: every pass flight (ball+0x64 when the ball
    // leaves the hands), the person's and his computer teammates' and
    // opponents', against the production release for a standing
    // receiver and the rule on the receiver's place at the press. The
    // first pass after the actors' reset flies 40 (`passing::FIRST_PASS_FLIGHT`):
    // each capture from save state 1 opens with the toss, its tip that
    // pass; the School's game has no toss, and a capture of it from update
    // 0 opens with that pass, its first lesson's inbound (the lessons'
    // later launches are no reset). A staged gamebreaker's passes (the
    // script manager's +0x50 state 3 to 7, schema 3) fly their cue's
    // updates, counted apart.
    let Some(captures) = load() else {
        return;
    };
    let mut flights = Vec::new();
    let mut staged = 0usize;
    for cap in captures {
        let mut tossed = !cap.school.is_empty() && cap.updates[0].frame() == 0;
        for pair in cap.updates.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            tossed |= a.ball_state() == 7;
            if a.ball_state() == 4 || b.ball_state() != 4 || b.frame() != a.frame() + 1 {
                continue;
            }
            if b.0["script_words"][8]
                .as_i64()
                .is_some_and(|s| (3..=7).contains(&s))
            {
                staged += 1;
                continue;
            }
            let mut f = b.frame();
            while cap
                .get(f)
                .is_some_and(|x| x.ball_state() == 4 && x.holder() != 0)
            {
                f += 1;
            }
            let (Some(release), Some(before)) = (cap.get(f), cap.get(f - 1)) else {
                continue;
            };
            if release.ball_state() != 4 {
                continue;
            }
            let first = std::mem::take(&mut tossed);
            let flight = release.ball()["flight_updates"].as_i64().unwrap_or(0) as i32;
            let passer = cap.actor_of(a.holder());
            let receiver = cap.actor_of(release.ball()["target_player"].as_u64().unwrap_or(0));
            let (Some(passer), Some(receiver)) = (passer, receiver) else {
                if first {
                    flights.push(crate::passing::CapturedFlight {
                        flight,
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
            let height = release.actor(receiver)["position"][1]
                .as_f64()
                .unwrap_or(0.0);
            let target = release.ball()["target"][1].as_f64().unwrap_or(0.0);
            flights.push(crate::passing::CapturedFlight {
                flight,
                first,
                to_self: passer == receiver,
                lob: height > 0.0 || target > 12.0,
                passer: before.floor(passer),
                receiver: before.floor(receiver),
                receiver_at_press: if receiver < passer { b } else { a }.floor(receiver),
            });
        }
    }
    let a = crate::passing::flight_agreement(&flights);
    report(
        "pass_flights",
        json!({"flights": flights.len(), "first": a.first, "ordinary": a.ordinary,
            "led": a.led, "spot": a.spot, "off": a.off, "to_self": a.to_self,
            "lobs": a.lobs, "staged": staged}),
    );
    assert!(a.first.iter().all(|&f| f == 40), "{:?}", a.first);
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
fn the_persons_catches_hold_for_the_flight_and_the_catch_rows_tail() {
    // The receiver's bit 9 holds after the ball is his for the flight's
    // nominal end still to come plus the catch row's tail: 9 for
    // `catch_stand_1`, 15 for `catch_stand_3r`, as it does for the
    // computer's receivers; a person's pad changes nothing. That holds for
    // catches whose script began at least the tail's 9 updates before the
    // ball arrived (every one but one, 15 to 57 ahead). A late one (a
    // computer's running receiver in the side-fade capture switched to
    // `catch_stand_1` 3 updates before a 25-update pass landed) held 15:
    // the script's blend from the run clip (6 updates) and the catch
    // clip played out (12) outlast the flight's end plus 9. Late catches
    // are reported and only bound from below (the rewrite's own: not
    // modelled). A script replacing the row inside its tail ends the hold
    // there (the pass captures' two: the shot clock's violation 9 updates
    // into `catch_stand_3r`, a computer's shot 4 in), reported as cut.
    let Some(captures) = load() else {
        return;
    };
    let mut checked = 0;
    let mut people = 0;
    let mut late = Vec::new();
    let mut off = Vec::new();
    let mut cut = Vec::new();
    for cap in captures {
        for pair in cap.updates.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if a.ball_state() != 4 || b.ball_state() != 3 || b.frame() != a.frame() + 1 {
                continue;
            }
            let Some(r) = cap.actor_of(b.holder()) else {
                continue;
            };
            let Some(row) = catch_row_at(b.pc(r)) else {
                continue;
            };
            // The post catches play their clip out (`8c`), a length the
            // executable does not hold (`timing_tables`' CLIP_TAIL is the
            // standing catch's): not measured here.
            if timing::CATCH_ROWS[row].name.starts_with("catch_post") {
                continue;
            }
            let left = a.ball()["flight_updates"].as_i64().unwrap_or(0)
                - a.ball()["progress"].as_i64().unwrap_or(0);
            let mut hold = 0;
            while cap
                .get(b.frame() + hold)
                .is_some_and(|x| x.word(r, "bits") & 0x200 != 0)
            {
                hold += 1;
            }
            // A script replacing the row ends it: its `9b` replaces the
            // state word (the pass captures' computer shot 4 updates in,
            // the shot clock's violation 9 in). Bit 9 then goes with the
            // row, in the update the channel leaves it.
            let mut kept = 0;
            while cap
                .get(b.frame() + kept)
                .is_some_and(|x| catch_row_at(x.pc(r)) == Some(row))
            {
                kept += 1;
            }
            let full = i64::from(timing::catch_hold(row, left as i32));
            let want = full.min(kept);
            if kept < full {
                cut.push((timing::CATCH_ROWS[row].name, b.frame(), kept, hold, full));
            }
            // Updates the catch script ran before the ball was his.
            let mut began = 0;
            while cap
                .get(b.frame() - began - 1)
                .is_some_and(|x| catch_row_at(x.pc(r)) == Some(row))
            {
                began += 1;
            }
            if began < 9 {
                late.push((timing::CATCH_ROWS[row].name, b.frame(), began, hold, want));
                if hold < want {
                    off.push((timing::CATCH_ROWS[row].name, b.frame(), hold, want));
                }
                continue;
            }
            checked += 1;
            people += usize::from(b.controllers()[r] != 4);
            if hold != want {
                off.push((timing::CATCH_ROWS[row].name, b.frame(), hold, want));
            }
        }
    }
    report(
        "catches",
        json!({"checked": checked, "people": people, "off": off.len(), "late": late, "cut": cut}),
    );
    assert!(checked >= 20 && people >= 10, "{checked} / {people}");
    assert!(off.is_empty(), "{off:?}");
    // Every cut row is one where bit 9 went with it, not before.
    assert!(
        cut.iter().all(|&(_, _, kept, hold, _)| hold == kept),
        "{cut:?}"
    );
}

#[test]
fn the_persons_shot_odds_on_got_game_carry_the_persons_bonus() {
    // The release callback's inputs for the person's jump shots, read
    // from the capture (game+0x374's distance, the long-range rule, the
    // shooter's record ratings, the opponents within 7 and those with
    // state bit 12, the points game's lead and stage bounds, a person on
    // the side, Got Game), through the sim's `shot_percent`: the makes
    // the capture shows are what those odds predict with the person's
    // doubled catch-up and +40 within 33.75 on Easy, and not what the
    // same odds predict without them. Statistical, over every jump shot
    // the person released.
    let Some(captures) = load() else {
        return;
    };
    let shots = human_shots(captures);
    let (mut rows, mut makes) = (Vec::new(), 0u32);
    let (mut with, mut without, mut with_or) = (Vec::new(), Vec::new(), Vec::new());
    let mut or_differs = 0;
    for shot in &shots {
        let Some(script) = shot.script else {
            continue;
        };
        if shot.release.is_none() || shot.made.is_none() || shot.fake_at.is_some() {
            continue;
        }
        let name = timing::SHOT_SCRIPTS[script].name;
        if !name.starts_with("jumpshot") {
            continue;
        }
        let cap = &captures[shot.capture];
        let record = &cap.records[shot.actor];
        let long = shot.distance >= 23.75 || shot.z_abs >= 22.5;
        let rating = |r: f32| (r * 99.0).round().clamp(0.0, 99.0) as i32;
        let ratings = record.ratings();
        let rim = cap.rim(shot.actor);
        let team = record.team.min(1);
        // `FUN_0012eac0` through the sim's own reading of it: the
        // captured points game (its target, both sides' points and the
        // stage bounds game+0x4b0/+0x4b4 it drew) on the replayed match.
        let Some(update) = cap.get(shot.press) else {
            continue;
        };
        let mut w = replay_world(cap, shot, update);
        w.rules.goal = Goal::Points(shot.target as u32);
        w.score = shot.points.map(|p| p as u32);
        w.stage_bounds = shot.bounds.map(|b| b as u32);
        let (lead, stage) = shooting::catch_up_cell(&w, team).unwrap();
        // The decompile's OR of the scaled scores (the Ghidra copy's
        // stand-in for `pmaxw`), kept for the report.
        let scaled = |t: usize| shot.points[t] * 21 / shot.target.max(1);
        let both = scaled(team) | scaled(1 - team);
        let or_stage = if both < shot.bounds[1] {
            usize::from(both >= shot.bounds[0])
        } else {
            2
        };
        or_differs += usize::from(or_stage != stage);
        let odds = shooting::ShotOdds {
            distance: shot.distance,
            long,
            rating: rating(if long { ratings.threes } else { ratings.shots }),
            frontcourt: (shot.position[0] >= 0.0) == (rim[0] >= 0.0),
            catch_up: shooting::catch_up_row(lead, stage),
            human: Some(true),
            difficulty: Difficulty::Easy,
            near: shot.near,
            contesting: shot.contesting,
            gamebreaker: 0,
        };
        let p = shooting::shot_percent(&odds);
        let q = shooting::shot_percent(&shooting::ShotOdds {
            human: None,
            ..odds
        });
        let o = shooting::shot_percent(&shooting::ShotOdds {
            catch_up: shooting::catch_up_row(lead, or_stage),
            ..odds
        });
        let made = shot.made == Some(true);
        makes += u32::from(made);
        with.push(p as f64 / 100.0);
        without.push(q as f64 / 100.0);
        with_or.push(o as f64 / 100.0);
        rows.push(
            json!({"press": shot.press, "script": name, "distance": shot.distance,
            "long": long, "near": shot.near, "contesting": shot.contesting,
            "points": shot.points, "target": shot.target, "bounds": shot.bounds,
            "lead": lead, "stage": stage, "or_stage": or_stage,
            "percent": p, "with_the_or": o, "without_the_person": q, "made": made}),
        );
    }
    let n = with.len();
    let mean = |v: &[f64]| v.iter().sum::<f64>();
    let sigma = |v: &[f64]| v.iter().map(|p| p * (1.0 - p)).sum::<f64>().sqrt();
    let (mu_p, sd_p, mu_q, sd_q) = (mean(&with), sigma(&with), mean(&without), sigma(&without));
    report(
        "odds",
        json!({"shots": n, "makes": makes, "expected_with": mu_p, "sigma_with": sd_p,
            "expected_without": mu_q, "sigma_without": sd_q,
            "expected_with_the_or": mean(&with_or), "or_differs": or_differs, "rows": rows}),
    );
    assert!(n >= 15, "{n}");
    let m = f64::from(makes);
    assert!(
        (m - mu_p).abs() <= 3.0 * sd_p + 1.0,
        "{makes} makes of {n}; the person's odds expect {mu_p:.1} ± {sd_p:.1}"
    );
    // The two models are told apart when they differ by more than their
    // spreads; then the makes must side with the person's bonus.
    if mu_p - mu_q > 3.0 * (sd_p + sd_q) {
        assert!(
            m > mu_q + 2.0 * sd_q,
            "{makes} makes of {n}; without the person's bonus {mu_q:.1} ± {sd_q:.1}"
        );
    }
}

/// One update's generator words (`0x4309f0`, [`crate::rng::Rng`]).
fn generator_of(update: &Update) -> crate::rng::Rng {
    crate::rng::Rng::from_state(std::array::from_fn(|k| {
        update.0["rng"][k].as_u64().unwrap() as u32
    }))
}

/// `rng` with `draws` of its draws undone.
fn undone(mut rng: crate::rng::Rng, draws: u32) -> crate::rng::Rng {
    for _ in 0..draws {
        rng.previous();
    }
    rng
}

/// Six players lined up as `street-game` lines a match up, for a world
/// built only to see what its construction draws.
fn lined_up() -> Vec<Player> {
    (0..6)
        .map(|k| {
            let team = k / 3;
            let x = if team == 0 { 12.0 } else { -12.0 };
            Player::new(team, [x, (k % 3) as f32 * 9.0 - 9.0], 0.0)
        })
        .collect()
}

/// The draws between the shared generator's reseed (`FUN_003a1790` from
/// the 100 Hz count) and the School game's update 0, in the one School
/// session captured from a fresh boot (`%TEMP%/street-played-school-0`):
/// 41, against the Pick Up game's 46 and its pair
/// ([`crate::CONSTRUCTION_DRAWS`]). Its pair (0, 17, from `FUN_0012e798`
/// under `FUN_0018e9f0`) matches the draws 12 and 20 back, as two of 40
/// places do by chance at 1 in 18, so where the School's loading and
/// construction draw it is not read.
const SCHOOL_DRAWS_AFTER_RESEED: u32 = 41;

/// The draws between the reseed and update 0 of a Be A Legend pick-up
/// played on from Update Team (`%TEMP%/street-played-legend-1`, save state
/// 3, the second NYC Rec Center game): its opponents' draw
/// (`FUN_001dbe30`) reseeds both generators from the 100 Hz count at the
/// map's press (113,415, `lineup.rs`'s disc test), so a story game is a
/// boot of its own; 51, its pair (2, 15) the last two.
const STORY_DRAWS_AFTER_RESEED: u32 = 51;

/// A story game's capture: the profile's front-end mode (`FUN_001e0bc0()`
/// +0x78) is Be A Legend's 5.
fn is_story(cap: &Capture) -> bool {
    cap.updates
        .first()
        .is_some_and(|u| u.0["front_end_mode"].as_i64() == Some(5))
}

#[test]
fn the_points_games_draw_their_stage_bounds_from_the_shared_generator_once_a_game() {
    // `FUN_0012e798` draws game+0x4b0 = `rand() % 6` and game+0x4b4 =
    // `15 + rand() % 3` when the points game is built. The captured games
    // (the save state's match, rematches from the pause menu's Quit Game
    // and the post-game dialog, a Pick Up game after a fresh boot) each
    // keep one pair for the match, and the pairs differ from game to game.
    // Where the capture starts at the game's update 0, the shared
    // generator (`0x4309f0`) holds the state right after those two draws:
    // the sim's construction on the generator as it stood before the
    // loading ([`World::from_generator`]) draws exactly the captured pair
    // and lands on the captured state. The School's game is a points game
    // too (`FUN_0018e9f0` builds it on `FUN_0012e750`), one for every
    // lesson of a session: it keeps one pair; its construction's count is
    // not the Pick Up game's ([`SCHOOL_DRAWS_AFTER_RESEED`]), so its pair
    // is not drawn here.
    let Some(captures) = load() else {
        return;
    };
    let (mut pairs_seen, mut at_construction, mut rows) = (Vec::new(), 0, Vec::new());
    for cap in captures {
        let school = !cap.school.is_empty();
        let pairs: Vec<[i64; 2]> = cap
            .updates
            .iter()
            .map(|u| {
                let b = &u.0["stage_bounds"];
                [b[0].as_i64().unwrap(), b[1].as_i64().unwrap()]
            })
            .collect();
        let Some(&pair) = pairs.first() else {
            continue;
        };
        assert!(
            pairs.iter().all(|&p| p == pair),
            "{}: one pair a match",
            cap.name
        );
        assert!(
            (0..6).contains(&pair[0]) && (15..18).contains(&pair[1]),
            "{}: {pair:?}",
            cap.name
        );
        let first = &cap.updates[0];
        let state = generator_of(first);
        // The inverse is the sim's step undone.
        let mut forward = undone(state.clone(), 1);
        forward.next_u32();
        assert_eq!(forward, state);
        let mut drawn = None;
        if !school && first.frame() == 0 {
            let before = undone(state.clone(), crate::CONSTRUCTION_DRAWS + 2);
            let w = World::from_generator(Hoop::native(), lined_up(), Rules::default(), before);
            assert_eq!(w.generator(), &state, "{}", cap.name);
            let built = w.stage_bounds.map(i64::from);
            assert_eq!(built, pair, "{}", cap.name);
            at_construction += 1;
            drawn = Some(built);
        }
        rows.push(json!({"capture": cap.name, "first_update": first.frame(),
            "bounds": pair, "drawn_from_the_state": drawn, "school": school}));
        if !school {
            pairs_seen.push(pair);
        }
    }
    let mut distinct = pairs_seen;
    distinct.sort_unstable();
    distinct.dedup();
    report(
        "stage_bounds",
        json!({"captures": rows, "at_construction": at_construction, "pairs": distinct}),
    );
    // Ten games in the evidence: nine pairs, every lower bound 0..5 and
    // every upper bound 15..17; seven captured from their update 0.
    assert!(at_construction >= 5, "{at_construction}");
    assert!(distinct.len() >= 6, "{distinct:?}");
    let mut lows: Vec<i64> = distinct.iter().map(|p| p[0]).collect();
    lows.dedup();
    let highs: std::collections::BTreeSet<i64> = distinct.iter().map(|p| p[1]).collect();
    assert!(lows.len() >= 4 && highs.len() == 3, "{distinct:?}");
}

#[test]
fn the_games_of_a_boot_run_on_from_one_reseed_of_the_shared_generator() {
    // After the boot's clock seed, only `FUN_001dbe30` reseeds the shared
    // generator: from the 100 Hz timer count (`0x4b32e4`) when a team is
    // set for a side no person holds. Walked back draw by draw, every
    // captured state reaches a state `FUN_003a1790` set (every word the
    // seed plus its running increment): the save state's boot (its
    // match, both pad captures and the side-fade capture, then the eight
    // games after it, rematches all) one reseed, the draws since growing
    // from game to game; the fresh boot's Pick Up game its own, the
    // construction's draws and the pair before update 0. (The pass
    // captures are played from save state 1 too: its reseed.) The School's
    // captures, a boot of their own, are one game's (every lesson of a
    // session plays in the one game) from one reseed, its update 0
    // [`SCHOOL_DRAWS_AFTER_RESEED`] draws on and the draws since growing
    // from capture to capture. A story game reseeds at its opponents' draw
    // on the map ([`STORY_DRAWS_AFTER_RESEED`]): each a boot of its own.
    let Some(captures) = load() else {
        return;
    };
    let story: std::collections::BTreeSet<&str> = captures
        .iter()
        .filter(|c| is_story(c))
        .map(|c| c.name.as_str())
        .collect();
    let school: std::collections::BTreeSet<&str> = captures
        .iter()
        .filter(|c| !c.school.is_empty())
        .map(|c| c.name.as_str())
        .collect();
    let mut boots: std::collections::BTreeMap<u32, Vec<(String, i64, u32)>> = Default::default();
    for cap in captures {
        let first = &cap.updates[0];
        let mut rng = generator_of(first);
        let mut since = 0u32;
        let seed = loop {
            if let Some(seed) = rng.seeded_by() {
                break seed;
            }
            rng.previous();
            since += 1;
            assert!(since < 2_000_000, "{}: no reseed in reach", cap.name);
        };
        boots
            .entry(seed)
            .or_default()
            .push((cap.name.clone(), first.frame(), since));
    }
    report(
        "reseeds",
        json!(
            boots
                .iter()
                .map(|(seed, games)| json!({"seed": seed, "captures": games
                    .iter()
                    .map(|(name, update, since)| json!({"capture": name,
                        "first_update": update, "draws_since": since}))
                    .collect::<Vec<_>>()}))
                .collect::<Vec<_>>()
        ),
    );
    let of_school =
        |games: &Vec<(String, i64, u32)>| games.iter().any(|g| school.contains(g.0.as_str()));
    let of_story =
        |games: &Vec<(String, i64, u32)>| games.iter().any(|g| story.contains(g.0.as_str()));
    for games in boots.values().filter(|g| of_story(g)) {
        assert_eq!(
            games.len(),
            1,
            "a story game's reseed is its own: {boots:?}"
        );
        if games[0].1 == 0 {
            assert_eq!(games[0].2, STORY_DRAWS_AFTER_RESEED, "{games:?}");
        }
    }
    let played = captures.len() - school.len() - story.len();
    let mut sizes: Vec<usize> = boots
        .values()
        .filter(|g| !of_school(g) && !of_story(g))
        .map(Vec::len)
        .collect();
    sizes.sort_unstable();
    assert_eq!(sizes, [1, played - 1], "{boots:?}");
    if !school.is_empty() {
        let games: Vec<&Vec<(String, i64, u32)>> =
            boots.values().filter(|g| of_school(g)).collect();
        assert_eq!(games.len(), 1, "{boots:?}");
        let mut games = games[0].clone();
        assert_eq!(games.len(), school.len(), "{games:?}");
        games.sort_by_key(|g| g.1);
        if games[0].1 == 0 {
            assert_eq!(games[0].2, SCHOOL_DRAWS_AFTER_RESEED, "{games:?}");
        }
        assert!(games.windows(2).all(|w| w[0].2 < w[1].2), "{games:?}");
    }
    // A count of hundredths since the boot: both a few minutes in (a story
    // game's, played on from a save state, under the hour).
    assert!(
        boots.iter().all(|(s, games)| if of_story(games) {
            (6_000..360_000).contains(s)
        } else {
            (6_000..60_000).contains(s)
        }),
        "{boots:?}"
    );
    let fresh = boots
        .values()
        .find(|games| games.len() == 1 && !of_story(games))
        .unwrap();
    assert_eq!(fresh[0].1, 0);
    assert_eq!(fresh[0].2, crate::CONSTRUCTION_DRAWS + 2);
    // The save state's boot: the three captures that load it start at the
    // same state, and each later game (and each pass capture, from later
    // updates of that match) further on.
    let saved = boots
        .values()
        .find(|games| games.len() == played - 1 && !of_school(games) && !of_story(games))
        .unwrap();
    let mut since: Vec<u32> = saved.iter().map(|g| g.2).collect();
    since.sort_unstable();
    since.dedup();
    assert_eq!(since.len(), saved.len() - 2, "{saved:?}");
}

#[test]
fn the_default_scripts_of_the_persons_shots_are_the_sims_candidates() {
    // Standing still in the front court the person's presses played the
    // default three (`0x3d7b10`), from the back court `heaveshot_1`, and
    // moving sideways the side fades or, on the 49% draws, the side hooks
    // (`0x3d9038`); running from out of the dunking area the runner, and
    // with two turbos on a full meter the gamebreaker's shots (the
    // School's lessons): every captured script is one the sim's chooser
    // (`timing::shot_script`) can name.
    let Some(captures) = load() else {
        return;
    };
    let shots = human_shots(captures);
    let mut names: Vec<&str> = shots
        .iter()
        .filter_map(|s| s.script.map(|i| timing::SHOT_SCRIPTS[i].name))
        .collect();
    names.sort_unstable();
    names.dedup();
    let known: Vec<&str> = timing::DEFAULT_SHOTS
        .iter()
        .chain(timing::SIDE_HOOKS.iter())
        .chain([timing::HEAVE, timing::FADEAWAY, timing::RUNNER].iter())
        .chain([timing::GAMEBREAKER_LONG, timing::GAMEBREAKER_SHORT].iter())
        .chain(
            [
                timing::SIDEFADE_RIGHT_R,
                timing::SIDEFADE_LEFT_L,
                timing::SIDEFADE_RIGHT_L,
                timing::SIDEFADE_LEFT_R,
            ]
            .iter(),
        )
        .map(|&i| timing::SHOT_SCRIPTS[i].name)
        .collect();
    assert!(names.iter().all(|n| known.contains(n)), "{names:?}");
    // The heave from the back court, and the default three from the
    // front court: the sim's chooser on the captured spots.
    for shot in shots
        .iter()
        .filter(|s| s.script.is_some() && s.started == 0)
    {
        let cap = &captures[shot.capture];
        let Some(update) = cap.get(shot.press) else {
            continue;
        };
        let name = timing::SHOT_SCRIPTS[shot.script.unwrap()].name;
        let mut w = replay_world(cap, shot, update);
        w.players[shot.actor].speed = 0.0;
        let picked = timing::shot_script(&mut w, shot.actor, 0, false, 0, false);
        let expected_heave = name == "heaveshot_1";
        assert_eq!(
            picked == timing::HEAVE,
            expected_heave,
            "{name} at {:?} (press {})",
            shot.position,
            shot.press
        );
        assert_eq!(
            BallState::Held(shot.actor),
            w.ball.state,
            "the replay keeps the ball in the shooter's hands"
        );
    }
}

/// The dunk or lay-up script whose body the channel's pc lies in (the
/// largest shot, dunk or lay-up script at or below it is a move row's).
fn move_script_at(pc: i64) -> Option<u32> {
    let script = timing::MOVE_ROWS
        .iter()
        .map(|r| r.script)
        .chain(timing::SHOT_SCRIPTS.iter().map(|s| s.script))
        .filter(|&s| i64::from(s) <= pc && pc < i64::from(s) + 0x400)
        .max()?;
    timing::MOVE_ROWS
        .iter()
        .any(|r| r.script == script)
        .then_some(script)
}

/// The actors of a captured update as `row_capture` reads them.
fn row_actors(cap: &Capture, u: &Update) -> Vec<row_capture::Actor> {
    let controllers = u.controllers();
    (0..6)
        .map(|k| {
            let a = u.actor(k);
            let team = cap.records[k].team.min(1);
            let motion = &a["motion"];
            let f = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
            row_capture::Actor {
                team: usize::from(cap.sides[team] < 0),
                position: u.floor(k),
                height: f(&a["position"][1]),
                heading: u.word(k, "heading") as i32,
                facing: a["facing"].as_i64().map(|v| v as i32),
                motion: [f(&motion[0]), f(&motion[2])],
                velocity: true,
                hand: a["hand"].as_i64().map(|h| h as u8),
                ratings: row_capture::ratings(&cap.records[k].ratings),
                computer: controllers.get(k) == Some(&4),
                held: u.word(k, "held") as u32,
                stick: (
                    u.word(k, "requested_heading") as i32,
                    u.word(k, "requested_intensity") as i32,
                ),
            }
        })
        .collect()
}

/// The captured passes, catches, dunks and lay-ups with a known row.
fn row_cases(
    cap: &Capture,
) -> (
    Vec<row_capture::PassCase>,
    Vec<row_capture::CatchCase>,
    Vec<row_capture::MoveCase>,
) {
    let (mut passes, mut catches, mut moves) = (Vec::new(), Vec::new(), Vec::new());
    let target = |u: &Update| -> Option<[f32; 2]> {
        let t = &u.ball()["target"];
        Some([t[0].as_f64()? as f32, t[2].as_f64()? as f32])
    };
    for pair in cap.updates.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if b.frame() != a.frame() + 1 {
            continue;
        }
        let live = matches!(b.phase(), 4 | 5);
        let holder = cap.actor_of(a.holder());
        let to = cap.actor_of(b.ball()["target_player"].as_u64().unwrap_or(0));
        let next = cap.get(b.frame() + 1);
        // The pass press: the ball turns to state 4.
        if live
            && a.ball_state() != 4
            && b.ball_state() == 4
            && let (Some(i), Some(j), Some(next)) = (holder, to, next)
            && i != j
            && let Some(row) = pass_row_at(next.pc(i))
        {
            // The filter runs in the press update after its locomotion:
            // the passer's stick (+0x90) and velocity are that update's (a
            // computer's pass node lets go of the stick as it presses, and
            // the stand path has zeroed the velocity).
            let mut actors = row_actors(cap, a);
            let now = row_actors(cap, b);
            actors[i].stick = now[i].stick;
            actors[i].motion = now[i].motion;
            let catch = (b.frame() + 1..b.frame() + 60)
                .filter_map(|f| cap.get(f))
                .find(|u| u.ball()["progress"].as_i64().unwrap_or(0) > 0)
                .and_then(&target);
            passes.push(row_capture::PassCase {
                capture: cap.name.clone(),
                update: b.frame(),
                actors,
                passer: i,
                receiver: j,
                script: timing::PASS_ROWS[row].script,
                shot_clock: a.0["shot_clock_words"][1].as_i64().map(|s| s as f32),
                catch,
            });
        }
        // The release: the flight's progress leaves 0; the receiver's
        // channel plays the catch row from it.
        let from =
            holder.or_else(|| cap.actor_of(b.ball()["previous_holder"].as_u64().unwrap_or(0)));
        if a.ball_state() == 4
            && b.ball_state() == 4
            && a.ball()["progress"].as_i64() == Some(0)
            && b.ball()["progress"].as_i64().unwrap_or(0) > 0
            && let (Some(i), Some(j), Some(spot)) = (from, to, target(b))
            && i != j
            && let Some(row) = (b.frame()..b.frame() + 4)
                .find_map(|f| cap.get(f).and_then(|x| catch_row_at(x.pc(j))))
        {
            let mut actors = row_actors(cap, b);
            actors[j].height = actors[j].height.max(row_actors(cap, a)[j].height);
            catches.push(row_capture::CatchCase {
                capture: cap.name.clone(),
                update: b.frame(),
                actors,
                passer: i,
                receiver: j,
                spot,
                flight: b.ball()["flight_updates"].as_i64().unwrap_or(0) as i32,
                script: timing::CATCH_ROWS[row].script,
            });
        }
        // The shoot press of the holder that played a dunk or lay-up row.
        if live
            && a.ball_state() == 3
            && let (Some(i), Some(next)) = (holder, next)
            && b.word(i, "pressed") & 0x80 != 0
            && let Some(script) = move_script_at(next.pc(i))
        {
            moves.push(row_capture::MoveCase {
                capture: cap.name.clone(),
                update: b.frame(),
                actors: row_actors(cap, a),
                shooter: i,
                script,
                buttons: (b.word(i, "held") | b.word(i, "pressed")) as u32,
            });
        }
    }
    (passes, catches, moves)
}

#[test]
fn the_pass_catch_and_dunk_rows_read_the_originals_sectors() {
    // `FUN_0018d768` measures in the original's frame: the pass filter the
    // receiver from the passer's yaw, the catch filter the ball's way in
    // (from the passer to the catch spot) against the receiver's facing
    // word, the dunk and lay-up filter the shooter from the rim's fixed
    // word. On every captured pass, catch, dunk and lay-up whose row reads
    // a sector, the sim's filter on the same state (the words as
    // headings) reads a sector the captured row's mask holds, and the
    // captured row is among its candidates (docs/TRACE.md "The sector in
    // the original's frame").
    let Some(captures) = load() else {
        return;
    };
    let (mut passes, mut catches, mut moves) = (Vec::new(), Vec::new(), Vec::new());
    for cap in captures {
        let (p, c, m) = row_cases(cap);
        passes.extend(p);
        catches.extend(c);
        moves.extend(m);
    }
    let p = row_capture::passes(&passes);
    let c = row_capture::catches(&catches);
    let m = row_capture::moves(&moves);
    report(
        "row_sectors",
        json!({"passes": p.json(), "catches": c.json(), "moves": m.json()}),
    );
    let counts = |t: &row_capture::Tally| (t.cases, t.sector, t.candidate);
    eprintln!(
        "passes {:?} catches {:?} moves {:?}",
        counts(&p),
        counts(&c),
        counts(&m)
    );
    // All twelve captures: 72 of 98 passes and 110 of 112 catches read a
    // sector the captured row holds (the sim's old reading, its heading
    // against a sim-frame direction: 7 and 3), and the captured row is a
    // candidate for 32 passes and 108 catches (3 and 1). On all 23
    // captures, the captured pass row is a candidate for 333 of 451 passes
    // (254 before the passer's stick and velocity were read on the press
    // update, where the pass node has let go of the stick and the stand
    // path has zeroed the velocity, and the release's catch spot stood in
    // for the receiver's destination). The rest: the sector (95, the yaw:
    // +0xa8 for the root bone's) and the distance band (21, the catch
    // spot read at the release rather than the press). The 16 dunks and lay-ups
    // the filter chose (30 more were two-turbo slot dunks) all play rows
    // that take any approach, so they agree either way.
    assert!(
        p.cases >= 90 && p.sector * 10 >= p.cases * 7,
        "{:?}",
        counts(&p)
    );
    assert!(p.candidate >= 320, "{:?}", counts(&p));
    assert!(
        c.cases >= 100 && c.sector * 50 >= c.cases * 48,
        "{:?}",
        counts(&c)
    );
    assert!(c.candidate * 50 >= c.cases * 47, "{:?}", counts(&c));
    assert!(m.cases >= 10 && m.sector == m.cases, "{:?}", counts(&m));
}

/// Each actor's model scales from his roster record's player (the disc's
/// `native_body_scale` and head scale), a plain 1.2 body for the rest.
fn bodies(cap: &Capture) -> Vec<crate::pose::Body> {
    cap.records
        .iter()
        .map(|r| {
            if let Some(height) = created_star(r) {
                return crate::pose::Body::uniform(street_assets::db::model_height_scale(height));
            }
            r.player
                .and_then(disc_player)
                .and_then(|p| {
                    let [height, width, _] = p.native_body_scale()?;
                    Some(crate::pose::Body {
                        height,
                        width,
                        head: p.native_head_scale().map_or(1.0, |h| h[0]),
                    })
                })
                .unwrap_or(crate::pose::Body::uniform(1.2))
        })
        .collect()
}

#[test]
fn the_swiping_hand_reaches_the_ball_where_the_original_strips_in_play() {
    // Every swipe at a held ball in the played captures (a steal press
    // without the two turbos), posed from the channel's own state as the
    // capture keeps it (`crate::swipe`'s clock checked against it: the
    // blend's target at +0x54 from the press, the swipe clip at 0.85,
    // 1.70, .. from update 5): through the blend the record and float frame
    // held at the press mixed into the target at its start by (t + 1) / 5,
    // then the clip at its captured float frame; the actor's place, his
    // heading word (+0xa8) and his roster body, all as the update before
    // left them, and that update's ball (the test runs in his side's
    // update, before his channels step; the ball updates after every
    // actor). Each strip should come on the first update the hand is in
    // reach.
    let Some(captures) = load() else {
        return;
    };
    let bank = crate::pose::disc_bank();
    let Some(poses) = bank.get() else {
        return;
    };
    let mut cases = Vec::new();
    let (mut swipes, mut strips, mut on_time, mut near) = (0, 0, 0, 0);
    for cap in captures {
        let bodies = bodies(cap);
        for (k, u) in cap.updates.iter().enumerate() {
            for (i, &body) in bodies.iter().enumerate() {
                let pressed = u.word(i, "pressed") as u32;
                let held = u.word(i, "held") as u32;
                if pressed & 0x200 == 0 || (pressed | held) & 0x40000 != 0 || u.ball_state() != 3 {
                    continue;
                }
                let Some(holder) = cap.actor_of(u.holder()) else {
                    continue;
                };
                let channel = |w: &Update| w.actor(i)["channels"][0].clone();
                let c0 = channel(u);
                let target = c0["target"].as_i64().unwrap_or(-1);
                let from_record = c0["record"].as_i64().unwrap_or(-1);
                if !(0..1144).contains(&target) || !(0..1144).contains(&from_record) || k == 0 {
                    continue;
                }
                let from = crate::pose::Frame {
                    record: from_record as u16,
                    time: c0["float_frame"].as_f64().unwrap_or(0.0) as f32,
                    looping: false,
                };
                swipes += 1;
                let (mut first, mut strip, mut trace) = (None, None, Vec::new());
                // The test of update t (`FUN_0015da28`, the side's update)
                // runs before his channels step and he moves: it poses the
                // clip, frame and place update t - 1 left (`s`), against
                // the ball the last ball update left.
                for t in 1..40usize {
                    let (Some(w), Some(before)) =
                        (cap.updates.get(k + t), cap.updates.get(k + t - 1))
                    else {
                        break;
                    };
                    if w.frame() != u.frame() + t as i64 || before.word(i, "bits") & 0x40 == 0 {
                        break;
                    }
                    let s = t - 1;
                    let c = channel(before);
                    let p = &before.actor(i)["position"];
                    let position = [0, 1, 2].map(|q| p[q].as_f64().unwrap() as f32);
                    let words = [
                        before.word(i, "heading") as i32,
                        before.word(i, "facing") as i32,
                    ];
                    let blending = s < 5;
                    let shown_record = if blending {
                        from.record
                    } else {
                        c["record"].as_i64().unwrap_or(-1) as u16
                    };
                    let shown_frame = if blending {
                        from.time as i32
                    } else {
                        c["frame"].as_i64().unwrap_or(0) as i32
                    };
                    let to = crate::pose::Frame {
                        record: target as u16,
                        time: if blending {
                            0.0
                        } else {
                            c["float_frame"].as_f64().unwrap_or(0.0) as f32
                        },
                        looping: false,
                    };
                    let b = &before.ball()["position"];
                    let ball = [0, 1, 2].map(|q| b[q].as_f64().unwrap() as f32);
                    let moving = before.motion(i) != 0.0;
                    let state = poses.state(shown_record, shown_frame);
                    let hand = crate::swipe::hand(state);
                    let d = words.map(|word| {
                        let place = crate::pose::Place { position, word };
                        let pose = if blending {
                            poses.blend(from, to, (s + 1) as f32 / 5.0, body, place)
                        } else {
                            poses.pose(to, body, place)
                        }
                        .unwrap();
                        let at = crate::pose::hand_point(
                            &pose,
                            i32::from(shown_record),
                            shown_frame,
                            hand,
                            body,
                            None,
                        );
                        crate::pose::length([at[0] - ball[0], at[1] - ball[1], at[2] - ball[2]])
                    });
                    let reach = crate::swipe::reach(moving, Some(shown_record));
                    trace.push(json!([t, d[0], d[1], reach]));
                    if first.is_none() && d[0] <= reach {
                        first = Some(t);
                    }
                    // The shot clock's violation drops the ball too (phase
                    // 6, the holder on its script): not a strip.
                    if before.ball_state() == 3 && w.ball_state() == 1 && w.phase() != 6 {
                        strip = Some((t, d[0] - reach));
                        break;
                    }
                    if w.ball_state() != 3 || cap.actor_of(w.holder()) != Some(holder) {
                        break;
                    }
                }
                if let Some((t, short)) = strip {
                    strips += 1;
                    on_time += usize::from(first == Some(t));
                    // Never in reach before it, and at most 0.2 short of it
                    // on the strip's own update.
                    near += usize::from(first == Some(t) || (first.is_none() && short <= 0.2));
                    cases.push(json!({"capture": cap.name, "frame": u.frame(), "actor": i,
                        "target": target, "body": [body.height, body.width],
                        "strip": t, "posed_first": first, "short": short,
                        "trace (t, by +0xa8, by +0xb0, reach)": trace}));
                }
            }
        }
    }
    report(
        "swipes",
        json!({"swipes": swipes, "strips": strips, "on_time": on_time, "cases": cases}),
    );
    eprintln!("swipes {swipes} strips {strips} on time {on_time} near {near}");
    // fades-1 and pads-2 (2026-10-16): 43 swipes, 4 strips, 2 on the
    // update the posed hand first reaches, 2 an update after it, posed
    // from the update's own clip. The test runs in the side's update,
    // before the swiper's channels step, and the ball (AIPhysBall) updates
    // after every actor (the process registry's order): posed from the
    // update before against its ball, the fourteen's 86 swipes and 7
    // strips (a dropped ball at the shot clock's violation is none): 5 on
    // the first update in reach, 2 never quite reached (0.02 and 0.16
    // short on the strip's update, the second in the swipe clip's blend).
    assert!(swipes > 0);
    assert_eq!(near, strips, "{on_time} of {strips} on time");
    assert!(on_time * 10 >= strips * 7, "{on_time} of {strips} on time");
}

#[test]
fn the_held_ball_follows_the_originals_dribble_in_play() {
    // Every update of the played captures with the ball held (state 3) by
    // an actor whose ball channel (`FUN_0014e060`: channel 1 when its flags
    // have 4, else 0) plays a dribbling record (a frame with 0x10) and is
    // not blending (+0x54 none, flag 0x200000 clear): the ball the ball
    // update placed (this update's sample) against `crate::dribble`'s
    // point for that record and float frame, the holder's place, his
    // facing word (+0xa8) and his roster body.
    let Some(captures) = load() else {
        return;
    };
    let bank = crate::pose::disc_bank();
    let Some(poses) = bank.get() else {
        return;
    };
    let mut errors: [Vec<f32>; 2] = [Vec::new(), Vec::new()];
    let mut old: [Vec<f32>; 2] = [Vec::new(), Vec::new()];
    let mut by_record: std::collections::BTreeMap<i64, (u32, f32)> = Default::default();
    let mut worst = Vec::new();
    let mut split_by: std::collections::BTreeMap<i64, [[f32; 3]; 2]> = Default::default();
    for cap in captures {
        let bodies = bodies(cap);
        for u in &cap.updates {
            if u.ball_state() != 3 {
                continue;
            }
            let Some(h) = cap.actor_of(u.holder()) else {
                continue;
            };
            let channels = &u.actor(h)["channels"];
            let c1 = &channels[1];
            let c = if c1["flags"].as_u64().unwrap_or(0) & 4 != 0
                && (0..1144).contains(&c1["record"].as_i64().unwrap_or(-1))
            {
                c1
            } else {
                &channels[0]
            };
            let record = c["record"].as_i64().unwrap_or(-1);
            let blending = c["target"].as_i64().unwrap_or(-1) != 0xffff_ffff
                || c["flags"].as_u64().unwrap_or(0) & 0x20_0000 != 0;
            if !(0..1144).contains(&record) || blending {
                continue;
            }
            let record = record as u16;
            if !crate::dribble::dribbles(poses, record) {
                continue;
            }
            let p = &u.actor(h)["position"];
            let position = [0, 1, 2].map(|q| p[q].as_f64().unwrap() as f32);
            let place = crate::pose::Place {
                position,
                word: u.word(h, "heading") as i32,
            };
            let time = c["float_frame"].as_f64().unwrap_or(0.0) as f32;
            let Some(at) = crate::dribble::ball_point(
                poses,
                record,
                time,
                bodies[h],
                place,
                crate::ball::RADIUS,
            ) else {
                continue;
            };
            let b = &u.ball()["position"];
            let ball = [0, 1, 2].map(|q| b[q].as_f64().unwrap() as f32);
            let e = crate::pose::length([at[0] - ball[0], at[1] - ball[1], at[2] - ball[2]]);
            // Before (2026-10-16): the ball always in the posed hand.
            let held = poses
                .pose(
                    crate::pose::Frame {
                        record,
                        time,
                        looping: true,
                    },
                    bodies[h],
                    place,
                )
                .map(|pose| {
                    let n = poses.frames(record).unwrap_or(1) as i32;
                    let int = (time.rem_euclid(n as f32) as i32).min(n - 1);
                    let s = poses.state(record, int);
                    let at = |hand| {
                        crate::pose::hand_point(
                            &pose,
                            i32::from(record),
                            int,
                            hand,
                            bodies[h],
                            Some(crate::ball::RADIUS),
                        )
                    };
                    if s & 0xc == 0xc {
                        let (a, c) = (at(1), at(2));
                        std::array::from_fn::<f32, 3, _>(|k| (a[k] + c[k]) * 0.5)
                    } else {
                        at(crate::swipe::hand(s))
                    }
                })
                .unwrap_or(at);
            let before =
                crate::pose::length([held[0] - ball[0], held[1] - ball[1], held[2] - ball[2]]);
            let n = poses.frames(record).unwrap_or(1) as i32;
            let int = (time.rem_euclid(n as f32) as i32).min(n - 1);
            let s = poses.state(record, int);
            let flying = s & 0xc == 0 || (s & 0x10 != 0 && time.fract() != 0.0);
            errors[usize::from(flying)].push(e);
            old[usize::from(flying)].push(before);
            let entry = by_record.entry(i64::from(record)).or_default();
            entry.0 += 1;
            entry.1 += e;
            let split = split_by.entry(i64::from(record)).or_default();
            split[usize::from(flying)][0] += (at[1] - ball[1]).abs();
            split[usize::from(flying)][1] += crate::math::len(at[0] - ball[0], at[2] - ball[2]);
            split[usize::from(flying)][2] += 1.0;
            if e > 1.0 && worst.len() < 40 {
                worst.push(
                    json!({"capture": cap.name, "frame": u.frame(), "record": record,
                    "time": time, "state": s, "ours": at, "ball": ball}),
                );
            }
        }
    }
    let median = |v: &mut Vec<f32>| {
        v.sort_by(f32::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    let within = |v: &[f32], d: f32| v.iter().filter(|&&e| e <= d).count();
    let [mut hand, mut flight] = errors;
    let [_, mut old_flight] = old;
    report(
        "dribble",
        json!({"in_hand": hand.len(), "in_flight": flight.len(),
            "before: flight_within_0.5": within(&old_flight, 0.5),
            "before: flight_median": median(&mut old_flight),
            "hand_within_0.1": within(&hand, 0.1), "flight_within_0.1": within(&flight, 0.1),
            "hand_within_0.5": within(&hand, 0.5), "flight_within_0.5": within(&flight, 0.5),
            "hand_median": median(&mut hand), "flight_median": median(&mut flight),
            "by_record (n, mean)": by_record.iter().map(|(r, (n, s))| json!([r, n, s / *n as f32])).collect::<Vec<_>>(),
            "by_record (hand dy, dxz, n; flight dy, dxz, n)": split_by.iter().map(|(r, v)| json!([r, v[0][0] / v[0][2].max(1.0), v[0][1] / v[0][2].max(1.0), v[0][2], v[1][0] / v[1][2].max(1.0), v[1][1] / v[1][2].max(1.0), v[1][2]])).collect::<Vec<_>>(),
            "worst": worst}),
    );
    eprintln!(
        "dribble: in hand {} (median {}), in flight {} (median {})",
        hand.len(),
        median(&mut hand),
        flight.len(),
        median(&mut flight)
    );
    // 2026-10-16, the twelve played captures: 10,409 updates in the hand
    // (median 0.015), 8,389 in flight (median 0.012, 8,372 within 0.5;
    // before, the ball in the posed hand: median 1.75, 1,531 within 0.5).
    // The rest are clips whose hand is off in the hand too (86, 87, 47,
    // 154: channels the pose does not blend).
    assert!(!flight.is_empty());
    assert!(median(&mut flight) < 0.05);
    assert!(within(&flight, 0.5) * 100 >= flight.len() * 97);
}

#[test]
fn the_blocking_hands_reach_the_ball_where_the_original_knocks_it_in_play() {
    // Every block jump of the played captures (state bit 12), posed each
    // update after its blend from the captured channel (record, float
    // frame), the actor's place, his facing word (+0xa8) and roster body:
    // the hands `FUN_0016d888` tries (both on state 0xc, else the right on
    // 8, the left on 4; the clip's state with 0x240) within `max(r, 1.0)`
    // of the ball of the update before (1.0 more for a person's jumper,
    // 0.5 less for the computer's against a person's side: Got Game is
    // the story record's difficulty 0), and the forearms' segments in the
    // limb sweep. A knock (the ball held, passed or shot gone loose at
    // 0.35 across and 0.125 up) should come on the first update a hand
    // reaches.
    let Some(captures) = load() else {
        return;
    };
    let bank = crate::pose::disc_bank();
    let Some(poses) = bank.get() else {
        return;
    };
    let r = crate::ball::RADIUS;
    let (mut jumps, mut knocks, mut on_time, mut reached) = (0, 0, 0, 0);
    let mut cases = Vec::new();
    for cap in captures {
        let bodies = bodies(cap);
        for (k, u) in cap.updates.iter().enumerate() {
            for (i, &body) in bodies.iter().enumerate() {
                let Some(before) = k.checked_sub(1).and_then(|j| cap.updates.get(j)) else {
                    continue;
                };
                if u.word(i, "bits") & 0x1000 == 0 || before.word(i, "bits") & 0x1000 != 0 {
                    continue;
                }
                jumps += 1;
                let people: Vec<bool> = u.controllers().iter().map(|&c| c != 4).collect();
                let team = cap.records[i].team.min(1);
                let person = people.get(i).copied().unwrap_or(false);
                let other_person = (0..6).any(|j| cap.records[j].team.min(1) != team && people[j]);
                let reach = r.max(
                    1.0 + if person {
                        1.0
                    } else if other_person {
                        -0.5
                    } else {
                        0.0
                    },
                );
                let (mut first, mut knock, mut trace) = (None, None, Vec::new());
                for t in 0..120usize {
                    let (Some(w), Some(prev)) =
                        (cap.updates.get(k + t), cap.updates.get(k + t - 1))
                    else {
                        break;
                    };
                    if w.frame() != u.frame() + t as i64 || w.word(i, "bits") & 0x1000 == 0 {
                        break;
                    }
                    // The bones the test reads are the last update's pose
                    // (`FUN_00156f48` poses the model after the actor's
                    // tests): the previous sample's channel and place.
                    let c = &prev.actor(i)["channels"][0];
                    let record = c["record"].as_i64().unwrap_or(-1);
                    let target = c["target"].as_i64().unwrap_or(-1);
                    let b = &prev.ball()["position"];
                    let ball = [0, 1, 2].map(|q| b[q].as_f64().unwrap() as f32);
                    let knocked =
                        matches!(prev.ball_state(), 3..=6) && matches!(w.ball_state(), 0 | 1) && {
                            let v = &w.ball()["velocity"];
                            let v = [0, 1, 2].map(|q| v[q].as_f64().unwrap_or(0.0) as f32);
                            (crate::math::len(v[0], v[2]) - 0.35).abs() < 0.02
                                && 0.09 < v[1]
                                && v[1] < 0.13
                        };
                    if (0..1144).contains(&record) && target == 0xffff_ffff {
                        let record = record as u16;
                        let p = &prev.actor(i)["position"];
                        let position = [0, 1, 2].map(|q| p[q].as_f64().unwrap() as f32);
                        let place = crate::pose::Place {
                            position,
                            word: prev.word(i, "heading") as i32,
                        };
                        let time = c["float_frame"].as_f64().unwrap_or(0.0) as f32;
                        let f = c["frame"].as_i64().unwrap_or(0) as i32;
                        let state = poses.state(record, f);
                        if let Some(pose) = poses.pose(
                            crate::pose::Frame {
                                record,
                                time,
                                looping: false,
                            },
                            body,
                            place,
                        ) && state & 0xc != 0
                            && state & 0x240 != 0
                        {
                            let at = |hand| {
                                let h = crate::pose::hand_point(
                                    &pose,
                                    i32::from(record),
                                    f,
                                    hand,
                                    body,
                                    None,
                                );
                                crate::pose::length([
                                    h[0] - ball[0],
                                    h[1] - ball[1],
                                    h[2] - ball[2],
                                ])
                            };
                            let d = if state & 0xc == 0xc {
                                at(1).min(at(2))
                            } else {
                                at(if state & 8 != 0 { 1 } else { 2 })
                            };
                            // The forearms in the limb sweep (the ball's
                            // last move, `FUN_00137db8`), by the clip's hands.
                            let older = cap
                                .updates
                                .get(k + t - 2)
                                .filter(|o| o.frame() + 2 == w.frame());
                            let arm = older
                                .and_then(|o| {
                                    let q = &o.ball()["position"];
                                    let from = [0, 1, 2].map(|j| q[j].as_f64().unwrap() as f32);
                                    crate::pose::sweep(&pose, from, ball, r, poses.reach(body))
                                })
                                .is_some_and(|hit| {
                                    let (a, c) = hit.bones;
                                    let left = (14..16).contains(&a) && (14..16).contains(&c);
                                    let right = (18..20).contains(&a) && (18..20).contains(&c);
                                    (state & 8 != 0 && right) || (state & 4 != 0 && left)
                                });
                            trace.push(json!([t, record, f, d, arm]));
                            if first.is_none() && (d <= reach || arm) {
                                first = Some(t);
                            }
                        }
                    }
                    if knocked {
                        knock = Some(t);
                        break;
                    }
                    if !matches!(w.ball_state(), 3..=6) {
                        break;
                    }
                }
                reached += usize::from(first.is_some());
                if let Some(t) = knock {
                    knocks += 1;
                    on_time += usize::from(first == Some(t));
                    cases.push(json!({"capture": cap.name, "frame": u.frame(), "actor": i,
                        "person": person, "reach": reach, "knock": t, "posed_first": first,
                        "trace (t, record, frame, hand to ball)": trace}));
                }
            }
        }
    }
    report(
        "blocks",
        json!({"jumps": jumps, "reached": reached, "knocks": knocks, "on_time": on_time,
            "cases": cases}),
    );
    eprintln!("block jumps {jumps} reached {reached} knocks {knocks} on time {on_time}");
    // 2026-10-16, pads-1, pads-2 and fades-1: 103 block jumps, 14 knocks
    // while one is up. Eight are the jumper's: 4 on the first update the
    // posed hand or forearm reaches, one an update and one three after it,
    // two at 0.94 and 1.08 (the hands posed without the channel blends);
    // six come with his hands 3.5 or more away or before his clip shows
    // (the contacts' knocks, not ported).
    assert!(jumps > 0);
    assert!(on_time >= 4, "{on_time} of {knocks}");
}

#[test]
fn the_block_jumps_rise_as_callback_35_aims_them_in_play() {
    // Every block jump of the played captures at a ball in the air (a pass
    // or a shot at the aim): callback 35 rebuilt from the capture (the
    // script by the channel's pc, its arm by the record the blend plays;
    // the hand posed at the script's frame where the jumper stood before
    // the takeoff, facing his +0xb0; the ball's captured places ahead; his
    // Blocks; a person's jumper on Got Game 1 higher) against the rise and
    // drift he took off with (the rise from his second airborne step, the
    // first carrying the clip's planted-foot correction).
    use crate::block::{Bounds, SCRIPTS, aim, rises};
    let Some(captures) = load() else {
        return;
    };
    let bank = crate::pose::disc_bank();
    let Some(poses) = bank.get() else {
        return;
    };
    let g = crate::toss::JUMP_GRAVITY;
    let script_of = |pc: i64| {
        SCRIPTS
            .iter()
            .enumerate()
            .filter(|(_, s)| i64::from(s.address) <= pc && pc < i64::from(s.address) + 0xac)
            .max_by_key(|(_, s)| s.address)
            .map(|(k, _)| k as u8)
    };
    let (mut checked, mut rise_close, mut drift_close) = (0, 0, 0);
    let mut cases = Vec::new();
    for cap in captures {
        let bodies = bodies(cap);
        for (k, u) in cap.updates.iter().enumerate() {
            for (i, &body) in bodies.iter().enumerate() {
                let Some(before) = k.checked_sub(1).and_then(|j| cap.updates.get(j)) else {
                    continue;
                };
                if u.word(i, "bits") & 0x1000 == 0 || before.word(i, "bits") & 0x1000 != 0 {
                    continue;
                }
                let Some(script) = script_of(u.actor(i)["channels"][0]["pc"].as_i64().unwrap_or(0))
                else {
                    continue;
                };
                let s = &SCRIPTS[usize::from(script)];
                let at = |t: i64| cap.get(u.frame() + t);
                let record = at(i64::from(s.blend[0]) + 1)
                    .and_then(|w| w.actor(i)["channels"][0]["record"].as_i64())
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
                let aim_at = i64::from(jump.aim_tick());
                let (Some(pre), Some(t0), Some(t1)) = (at(aim_at - 1), at(aim_at), at(aim_at + 1))
                else {
                    continue;
                };
                let y = |w: &Update| w.actor(i)["position"][1].as_f64().unwrap_or(0.0) as f32;
                if y(pre) != 0.0 || y(t0) <= 0.0 || y(t1) <= y(t0) - 1.0 {
                    continue;
                }
                let state = pre.ball_state();
                if !matches!(state, 4 | 5) {
                    continue;
                }
                let p = &pre.actor(i)["position"];
                let position = [0, 1, 2].map(|q| p[q].as_f64().unwrap() as f32);
                let place = crate::pose::Place {
                    position,
                    word: pre.word(i, "facing") as i32,
                };
                let record = jump.record();
                let frame = f32::from(s.frame);
                let Some(pose) = poses.pose(
                    crate::pose::Frame {
                        record,
                        time: frame,
                        looping: false,
                    },
                    body,
                    place,
                ) else {
                    continue;
                };
                let f = i32::from(s.frame);
                let st = poses.state(record, f);
                let hp = |hand| {
                    crate::pose::hand_point(
                        &pose,
                        i32::from(record),
                        f,
                        hand,
                        body,
                        Some(crate::ball::RADIUS),
                    )
                };
                let hand = if st & 0xc == 0xc {
                    let (a, c) = (hp(1), hp(2));
                    std::array::from_fn(|q| (a[q] + c[q]) * 0.5)
                } else {
                    hp(if st & 8 != 0 { 1 } else { 2 })
                };
                let blocks = cap.records[i].ratings().native(5);
                let person = u.controllers().get(i).is_some_and(|&c| c != 4);
                let (v_top, _, v_min) = rises(blocks, person);
                let mut n = (v_top / g) as i32 + 1;
                let left = pre.ball()["flight_updates"].as_i64().unwrap_or(0)
                    - pre.ball()["progress"].as_i64().unwrap_or(0)
                    - 3;
                n = n.min(left as i32).max(1);
                let path: Option<Vec<[f32; 3]>> = (1..=i64::from(n))
                    .map(|q| {
                        cap.get(pre.frame() + q)
                            .filter(|w| w.ball_state() == state)
                            .map(|w| {
                                let b = &w.ball()["position"];
                                [0, 1, 2].map(|j| b[j].as_f64().unwrap() as f32)
                            })
                    })
                    .collect();
                let Some(path) = path else {
                    continue;
                };
                let out = aim(
                    Bounds {
                        hand,
                        v_top,
                        v_min,
                        across: v_top * 0.57,
                        n,
                        floor: 0.0,
                        latest: state == 5,
                    },
                    |q| path[(q - 1) as usize],
                );
                // The rise: the median of each later step plus its share of
                // the gravity (the first steps carry the clip's planted-foot
                // correction, `FUN_00156f48` with channel flag 0x40000).
                let mut rises: Vec<f32> = (3..12)
                    .filter_map(|q: i64| {
                        let (a, c) = (at(aim_at + q)?, at(aim_at + q + 1)?);
                        (y(c) > 0.0).then(|| y(c) - y(a) + (q as f32 + 1.5) * g)
                    })
                    .collect();
                rises.sort_by(f32::total_cmp);
                let Some(&rise) = rises.get(rises.len() / 2) else {
                    continue;
                };
                let drift = [0, 2].map(|q| {
                    (t1.actor(i)["position"][q].as_f64().unwrap()
                        - t0.actor(i)["position"][q].as_f64().unwrap()) as f32
                });
                checked += 1;
                let dv = (rise - out.velocity[1]).abs();
                let dd = crate::math::len(drift[0] - out.velocity[0], drift[1] - out.velocity[2]);
                rise_close += usize::from(dv <= 0.02);
                drift_close += usize::from(dd <= 0.02);
                cases.push(json!({"capture": cap.name, "frame": u.frame(), "actor": i,
                    "script": s.address, "blocks": blocks, "person": person, "n": n,
                    "ours": out.velocity, "meet": out.meet, "rise": rise, "drift": drift}));
            }
        }
    }
    report(
        "block_aims",
        json!({"checked": checked, "rise_within_0.02": rise_close,
            "drift_within_0.02": drift_close, "cases": cases}),
    );
    eprintln!("block aims {checked}: rise within 0.02 {rise_close}, drift {drift_close}");
    // 2026-10-16, pads-1, pads-2 and fades-1: 50 aims at a ball in the air;
    // the rise within 0.02 in 46 (exact at the top for Blocks 21, 45 and
    // 80; the two the captures left unrated, read as 50, rose as Blocks
    // 60), the drift in 40. 2026-10-18, the fourteen, those two being the
    // disc's 55 and 95 ([`SAVE_STATE_1`], Blocks 81 and 60): 56 aims, the
    // rise within 0.02 in 53, the drift in 45.
    assert!(checked > 0);
    assert!(
        rise_close * 100 >= checked * 85,
        "{rise_close} of {checked}"
    );
    assert!(
        drift_close * 100 >= checked * 70,
        "{drift_close} of {checked}"
    );
}

#[test]
fn the_block_jumps_at_a_holder_rise_as_callback_35_aims_them_in_play() {
    // Every block jump of the played captures at a held ball (state 3 at
    // the aim): callback 35's point by the holder rebuilt from the capture
    // ([`crate::block::holder_target`]: his state bits, his head posed from
    // his channel, his velocity +0x40, his place and facing; the jumper's
    // hand posed at the script's frame where he stood before the takeoff,
    // facing his +0xb0; his Blocks), against the rise and drift he took
    // off with.
    use crate::block::{Bounds, Holder, SCRIPTS, aim, holder_target, rises};
    let Some(captures) = load() else {
        return;
    };
    let bank = crate::pose::disc_bank();
    let Some(poses) = bank.get() else {
        return;
    };
    let g = crate::toss::JUMP_GRAVITY;
    let script_of = |pc: i64| {
        SCRIPTS
            .iter()
            .enumerate()
            .filter(|(_, s)| i64::from(s.address) <= pc && pc < i64::from(s.address) + 0xac)
            .max_by_key(|(_, s)| s.address)
            .map(|(k, _)| k as u8)
    };
    let mut by_kind: std::collections::BTreeMap<&str, (usize, usize, usize)> = Default::default();
    let mut cases = Vec::new();
    for cap in captures {
        let bodies = bodies(cap);
        for (k, u) in cap.updates.iter().enumerate() {
            for (i, &body) in bodies.iter().enumerate() {
                let Some(before) = k.checked_sub(1).and_then(|j| cap.updates.get(j)) else {
                    continue;
                };
                if u.word(i, "bits") & 0x1000 == 0 || before.word(i, "bits") & 0x1000 != 0 {
                    continue;
                }
                let Some(script) = script_of(u.actor(i)["channels"][0]["pc"].as_i64().unwrap_or(0))
                else {
                    continue;
                };
                let s = &SCRIPTS[usize::from(script)];
                let at = |t: i64| cap.get(u.frame() + t);
                let record = at(i64::from(s.blend[0]) + 1)
                    .and_then(|w| w.actor(i)["channels"][0]["record"].as_i64())
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
                let aim_at = i64::from(jump.aim_tick());
                let (Some(pre), Some(t0), Some(t1)) = (at(aim_at - 1), at(aim_at), at(aim_at + 1))
                else {
                    continue;
                };
                let y =
                    |w: &Update, j: usize| w.actor(j)["position"][1].as_f64().unwrap_or(0.0) as f32;
                if y(pre, i) != 0.0 || y(t0, i) <= 0.0 || y(t1, i) <= y(t0, i) - 1.0 {
                    continue;
                }
                if pre.ball_state() != 3 {
                    continue;
                }
                let Some(h) = cap.actor_of(pre.holder()) else {
                    continue;
                };
                if cap.records[h].team == cap.records[i].team {
                    continue;
                }
                // The jumper's hand at the script's frame.
                let p = &pre.actor(i)["position"];
                let position = [0, 1, 2].map(|q| p[q].as_f64().unwrap() as f32);
                let place = crate::pose::Place {
                    position,
                    word: pre.word(i, "facing") as i32,
                };
                let record = jump.record();
                let Some(pose) = poses.pose(
                    crate::pose::Frame {
                        record,
                        time: f32::from(s.frame),
                        looping: false,
                    },
                    body,
                    place,
                ) else {
                    continue;
                };
                let f = i32::from(s.frame);
                let st = poses.state(record, f);
                let hp = |hand| {
                    crate::pose::hand_point(
                        &pose,
                        i32::from(record),
                        f,
                        hand,
                        body,
                        Some(crate::ball::RADIUS),
                    )
                };
                let hand = if st & 0xc == 0xc {
                    let (a, c) = (hp(1), hp(2));
                    std::array::from_fn(|q| (a[q] + c[q]) * 0.5)
                } else {
                    hp(if st & 8 != 0 { 1 } else { 2 })
                };
                // The holder as callback 35 reads him.
                let c = &pre.actor(h)["channels"][0];
                let (hrec, htime) = (
                    c["record"].as_i64().unwrap_or(-1),
                    c["float_frame"].as_f64().unwrap_or(0.0) as f32,
                );
                let hp_ = &pre.actor(h)["position"];
                let hpos = [0, 1, 2].map(|q| hp_[q].as_f64().unwrap() as f32);
                let hplace = crate::pose::Place {
                    position: hpos,
                    word: pre.word(h, "heading") as i32,
                };
                let hbody = bodies[h];
                let head = (0..1144)
                    .contains(&hrec)
                    .then(|| {
                        poses.pose(
                            crate::pose::Frame {
                                record: hrec as u16,
                                time: htime.max(0.0),
                                looping: false,
                            },
                            hbody,
                            hplace,
                        )
                    })
                    .flatten()
                    .map(|pose| pose.joints[21].position);
                let Some(head) = head else {
                    continue;
                };
                let m = &pre.actor(h)["motion"];
                let motion = [0, 1, 2].map(|q| m[q].as_f64().unwrap_or(0.0) as f32);
                let bits = (pre.word(h, "bits") as u32) & 0xa088;
                let bits = if bits & 0x2000 != 0 { 0x2008 } else { bits };
                let kind = match bits {
                    b if b & 0x8000 != 0 => "leap",
                    b if b & 0x80 != 0 => "dunk",
                    b if b & 0x2008 != 0 && hpos[1] > 0.0 => "shot in the air",
                    b if b & 0x2008 != 0 => "shot on the floor",
                    _ => "holder",
                };
                // A dunker's or leaper's meet is not captured.
                if bits & 0x8080 != 0 && hpos[1] > 0.0 {
                    continue;
                }
                let view = Holder {
                    bits,
                    head,
                    velocity: motion,
                    height: hpos[1],
                    height_ft: hbody.height / 1.2 * 6.5,
                    position: [hpos[0], hpos[2]],
                    facing: pre.word(h, "heading") as i32,
                    passing: pre.word(h, "bits") & 0x10 != 0,
                    left: None,
                };
                let blocks = cap.records[i].ratings().native(5);
                let person = u.controllers().get(i).is_some_and(|&c| c != 4);
                let (v_low, v_high, v_min) = rises(blocks, person);
                let side = cap.sides[cap.records[h].team.min(1)] as f32;
                let rim = [side * 36.75, 0.0];
                let (target, v_top, across, n) =
                    holder_target(&view, [position[0], position[2]], rim, v_low, v_high);
                let out = aim(
                    Bounds {
                        hand,
                        v_top,
                        v_min,
                        across,
                        n: n.max(1),
                        floor: target.floor,
                        latest: false,
                    },
                    |q| target.at(q as f32, g),
                );
                let mut rises: Vec<f32> = (3..12)
                    .filter_map(|q: i64| {
                        let (a, c) = (at(aim_at + q)?, at(aim_at + q + 1)?);
                        (y(c, i) > 0.0).then(|| y(c, i) - y(a, i) + (q as f32 + 1.5) * g)
                    })
                    .collect();
                rises.sort_by(f32::total_cmp);
                let Some(&rise) = rises.get(rises.len() / 2) else {
                    continue;
                };
                let drift = [0, 2].map(|q| {
                    (t1.actor(i)["position"][q].as_f64().unwrap()
                        - t0.actor(i)["position"][q].as_f64().unwrap()) as f32
                });
                let dv = (rise - out.velocity[1]).abs();
                let dd = crate::math::len(drift[0] - out.velocity[0], drift[1] - out.velocity[2]);
                let e = by_kind.entry(kind).or_default();
                e.0 += 1;
                e.1 += usize::from(dv <= 0.02);
                e.2 += usize::from(dd <= 0.02);
                cases.push(json!({"capture": cap.name, "frame": u.frame(), "actor": i,
                    "holder": h, "kind": kind, "script": s.address, "blocks": blocks,
                    "person": person, "n": n, "ours": out.velocity, "meet": out.meet,
                    "rise": rise, "drift": drift}));
            }
        }
    }
    report(
        "block_aims_at_holders",
        json!({"by_kind": by_kind, "cases": cases}),
    );
    eprintln!("block aims at holders (checked, rise within 0.02, drift within 0.02): {by_kind:?}");
    assert!(!by_kind.is_empty());
    // 2026-10-17, the twelve captures (pads-1, pads-2 and fades-1 hold such
    // aims): 12 of 13 rise within 0.02, the miss read at Blocks 50, the
    // default for a player whose ratings the capture lacks. 2026-10-18, the
    // fourteen with save state 1's players ([`SAVE_STATE_1`]): 23 of 24, the
    // one left (passes-2's 33308) rising 0.19, under the lowest rise the
    // aim allows.
    let (checked, risen) = by_kind
        .values()
        .fold((0, 0), |(n, r), &(a, b, _)| (n + a, r + b));
    assert!(
        risen * 5 >= checked * 4,
        "{risen} of {checked} rise within 0.02"
    );
}

#[test]
fn the_trick_moves_play_their_scripts_and_clips_in_play() {
    // Every trick move of the played captures (actor bit 10 set), replayed
    // on its script ([`crate::trick`]): the script by the channel's pc, its
    // clip by the record the blend plays; the channel's clock against the
    // capture's (the blend's target, then the clip's float frame, update
    // by update, and the update bit 10 drops); the coast through the blend
    // (the velocity the press left, held); the clip's root after it, with
    // the player's own height scale, turned by his captured facing (the
    // updates nobody stood within the contact radii); the facing callback
    // 7 takes from the pelvis at the end.
    use crate::trick::{self, Run, SCRIPTS};
    let Some(captures) = load() else {
        return;
    };
    let bank = crate::pose::disc_bank();
    let Some(poses) = bank.get() else {
        return;
    };
    let script_of = |pc: i64| {
        SCRIPTS
            .iter()
            .enumerate()
            .filter(|(_, s)| i64::from(s.address) <= pc && pc < i64::from(s.address) + 0x50)
            .max_by_key(|(_, s)| s.address)
            .map(|(k, _)| k as u16)
    };
    let f2 = |v: &Value| {
        [
            v[0].as_f64().unwrap_or(0.0) as f32,
            v[2].as_f64().unwrap_or(0.0) as f32,
        ]
    };
    let (mut tricks, mut clocks, mut ends, mut coasts, mut faces) = (0, 0, 0, 0, 0);
    let (mut cuts, mut lates) = (0, 0);
    // The clip's steps by the nearest other player's distance (within the
    // contact radii, within 5.5, beyond): steps, within 0.01, the motion
    // along the facing (ours, theirs).
    let mut by_room = [(0usize, 0usize, 0.0f32, 0.0f32); 3];
    let mut cases = Vec::new();
    for cap in captures {
        let bodies = bodies(cap);
        for (k, u) in cap.updates.iter().enumerate() {
            for (i, &body) in bodies.iter().enumerate() {
                let Some(before) = k.checked_sub(1).and_then(|j| cap.updates.get(j)) else {
                    continue;
                };
                if u.word(i, "bits") & 0x400 == 0 || before.word(i, "bits") & 0x400 != 0 {
                    continue;
                }
                let Some(script) = script_of(u.pc(i)) else {
                    continue;
                };
                let s = &SCRIPTS[usize::from(script)];
                let Some(trick::Step::Blend { records, .. }) = s
                    .steps
                    .iter()
                    .find(|x| matches!(x, trick::Step::Blend { .. }))
                    .copied()
                else {
                    continue;
                };
                let target = u.actor(i)["channels"][0]["target"].as_i64().unwrap_or(-1);
                let left = target == i64::from(records[1]) && records[0] != records[1];
                if target != i64::from(records[0]) && !left {
                    continue;
                }
                tricks += 1;
                let mut run = Run::new(left, trick::flight(script));
                let (mut clock_ok, mut coast_ok) = (true, true);
                let mut end = None;
                let mut cut = None;
                let mut bad = None;
                let mut previous = run.shown();
                let v0 = f2(&u.actor(i)["motion"]);
                for t in 0..200i64 {
                    let (Some(now), Some(last)) =
                        (cap.get(u.frame() + t), cap.get(u.frame() + t - 1))
                    else {
                        clock_ok = false;
                        break;
                    };
                    run.step(s.steps, 0.0);
                    let shown = run.shown();
                    if run.done {
                        end = Some(t);
                        break;
                    }
                    // A script replacing the trick's cuts it (a pass out
                    // of it, a strip and the reaction it plays, a counter's
                    // steal): the clock is checked up to there.
                    if script_of(now.pc(i)) != Some(script) {
                        cut = Some(t);
                        break;
                    }
                    let c = &now.actor(i)["channels"][0];
                    let record = c["record"].as_i64().unwrap_or(-1);
                    let frame = c["float_frame"].as_f64().unwrap_or(-1.0) as f32;
                    let blending = c["target"].as_i64().unwrap_or(-1) != 0xffff_ffff;
                    if shown.blend.is_some() {
                        clock_ok &= blending && c["target"].as_i64() == shown.record.map(i64::from);
                        // The coast: the press's velocity, held.
                        let v = f2(&now.actor(i)["motion"]);
                        coast_ok &= (v[0] - v0[0]).abs() < 1e-4 && (v[1] - v0[1]).abs() < 1e-4;
                        previous = shown;
                        continue;
                    }
                    let at = |shown: trick::Shown, record: i64, frame: f32| {
                        !blending
                            && Some(record) == shown.record.map(i64::from)
                            && (frame - shown.frame).abs() < 2e-3
                    };
                    // A sample read late shows the next update's step and
                    // the next sample the same frame again (the reads are
                    // not paused: 20 ms after the last, then 13): the
                    // clock is the next update's on both.
                    let late = || {
                        let mut ahead = run;
                        ahead.step(s.steps, 0.0);
                        let next = cap.get(u.frame() + t + 1).map(|x| {
                            let c = &x.actor(i)["channels"][0];
                            (
                                c["record"].as_i64().unwrap_or(-1),
                                c["float_frame"].as_f64().unwrap_or(-1.0) as f32,
                            )
                        });
                        let shown = ahead.shown();
                        !ahead.done
                            && shown.blend.is_none()
                            && at(shown, record, frame)
                            && next.is_some_and(|(r, f)| at(shown, r, f))
                    };
                    let ok = at(shown, record, frame) || late();
                    lates += usize::from(!at(shown, record, frame) && ok);
                    if !ok && bad.is_none() {
                        bad = Some((t, record, frame, shown.frame));
                    }
                    clock_ok &= ok;
                    if script >= 74 || crate::self_toss::is_self_toss_script(script) {
                        previous = shown;
                        continue;
                    }
                    // The clip's root, turned by the facing he has.
                    let word = now.word(i, "heading") as i32;
                    let ours =
                        trick::root_motion(trick::root_step(poses, previous, shown), body, word);
                    let theirs = [
                        now.floor(i)[0] - last.floor(i)[0],
                        now.floor(i)[1] - last.floor(i)[1],
                    ];
                    let room = (0..6)
                        .filter(|&j| j != i)
                        .map(|j| {
                            crate::math::len(
                                now.floor(j)[0] - now.floor(i)[0],
                                now.floor(j)[1] - now.floor(i)[1],
                            )
                        })
                        .fold(f32::INFINITY, f32::min);
                    let bin = &mut by_room[if room <= 2.6 {
                        0
                    } else if room <= 5.5 {
                        1
                    } else {
                        2
                    }];
                    let e = crate::math::len(ours[0] - theirs[0], ours[1] - theirs[1]);
                    let w = word as f32 * std::f32::consts::TAU / 1024.0;
                    let f = [-crate::math::sin(w), -crate::math::cos(w)];
                    bin.0 += 1;
                    bin.1 += usize::from(e <= 0.01);
                    bin.2 += ours[0] * f[0] + ours[1] * f[1];
                    bin.3 += theirs[0] * f[0] + theirs[1] * f[1];
                    previous = shown;
                }
                let ended = end.is_some_and(|t| {
                    cap.get(u.frame() + t)
                        .is_some_and(|e| e.word(i, "bits") & 0x400 == 0)
                        && cap
                            .get(u.frame() + t - 1)
                            .is_some_and(|e| e.word(i, "bits") & 0x400 != 0)
                });
                clocks += usize::from(clock_ok);
                cuts += usize::from(cut.is_some());
                ends += usize::from(ended);
                coasts += usize::from(coast_ok);
                // Callback 7's facing at the end, from the last frame's pelvis.
                let mut face = None;
                if let Some(t) = end
                    && let (Some(last), Some(at_end)) =
                        (cap.get(u.frame() + t - 1), cap.get(u.frame() + t))
                {
                    let p = &last.actor(i)["position"];
                    let place = crate::pose::Place {
                        position: [
                            p[0].as_f64().unwrap_or(0.0) as f32,
                            p[1].as_f64().unwrap_or(0.0) as f32,
                            p[2].as_f64().unwrap_or(0.0) as f32,
                        ],
                        word: last.word(i, "heading") as i32,
                    };
                    if let Some(pose) = trick::pose(poses, run.shown(), None, body, place) {
                        let ours = trick::pelvis_word(&pose);
                        let theirs = at_end.word(i, "heading") as i32;
                        let stick = at_end.word(i, "requested_heading") as i32;
                        // The pelvis's word, then the gait's turn toward the
                        // stick in the same update: the captured facing lies
                        // on that turn.
                        let wrap = |d: i32| (d + 0x200).rem_euclid(0x400) - 0x200;
                        let (turned, toward) = (wrap(theirs - ours), wrap(stick - ours));
                        let on_turn = turned.abs() <= 8
                            || (turned.signum() == toward.signum()
                                && turned.abs() <= toward.abs() + 8);
                        faces += usize::from(on_turn);
                        face = Some((ours, theirs, stick));
                    }
                }
                cases.push(json!({"capture": cap.name, "frame": u.frame(), "actor": i,
                    "script": script, "left": left, "clock": clock_ok, "bad": bad, "end": end,
                    "cut": cut, "ended": ended, "coast": coast_ok, "face": face}));
            }
        }
    }
    report(
        "tricks",
        json!({"tricks": tricks, "clock": clocks, "ends": ends, "coasts": coasts,
            "cut": cuts, "late_reads": lates,
            "faces": faces, "root_by_room": by_room.map(|(n, close, a, b)| {
                json!({"steps": n, "within_0.01": close, "along_ours": a, "along_theirs": b})
            }), "cases": cases}),
    );
    eprintln!(
        "tricks {tricks}: clock {clocks}, ends {ends}, coasts {coasts}, faces {faces}, cut {cuts}, late reads {lates}; root by room (steps, within 0.01, along ours, theirs) {by_room:?}"
    );
    // 2026-10-16, pads-1, pads-2 and fades-1: 27 tricks. The clock as the
    // original's in 25 (one trick countered at its sixth update; in one
    // the original's channel stepped twice in an update at its 24th, and
    // still ended on its script's update), the end in 26, the coast in 27,
    // the facing in 26. The clip's root within 0.01 in 695 of the 740
    // steps with nobody within 4.5, along the facing 41.96 against 39.57;
    // nearer, the original's contacts (whose radii add the clip's limbs,
    // `FUN_001337e0`) hold the handler back: 55.62 against 26.31 between
    // 2.6 and 4.5.
    // 2026-10-18, the fourteen: 50 tricks, 4 cut by a move replacing the
    // script and checked to there (the twelve's "countered" one was a
    // swipe's strip at its sixth update; in passes-2 a pass out of the
    // trick at the 17th and two counters' steals at the 10th and 13th);
    // 3 samples read late (the channel a step ahead, the next sample the
    // same frame: the twelve's at the 24th update, two in passes-2). The
    // clock as the original's in all 50, the end in all 46 played out, the
    // facing in 45. The contacts' radii reach with the bodies' limbs past
    // 4.5 (the old open bin's edge): with the roster's bodies the steps
    // with nobody within 5.5 hold the clip's root within 0.01 in 783 of
    // 785, along the facing 32.49 against 32.49.
    let whole = tricks - cuts;
    assert!(tricks > 0);
    assert!(clocks * 100 >= tricks * 95, "clock {clocks} of {tricks}");
    assert!(ends * 100 >= whole * 95, "ends {ends} of {whole}");
    assert!(coasts * 100 >= tricks * 95, "coasts {coasts} of {tricks}");
    assert!(faces * 100 >= whole * 90, "faces {faces} of {whole}");
    let (n, close, ours, theirs) = by_room[2];
    assert!(close * 100 >= n * 98, "root {close} of {n}");
    assert!(
        (ours - theirs).abs() <= 0.02 * theirs.abs(),
        "along {ours} {theirs}"
    );
}

#[test]
fn the_guards_move_along_their_sticks_and_face_the_holder_in_play() {
    // The computer's defenders guarding the person's holder (the person's
    // own stick is idle whenever his side defends in these captures): every
    // update of theirs with the stick out (actor+0x90 at least 16), on the
    // floor and in no move, in the front court with the ball held. The
    // velocity (actor+0x40) points along the stick (+0x88), whatever the
    // facing (`0x167d88`); the guarding mode as ported ([`crate::guard`]:
    // the gates' starts and ends kept per guard, the record by the stick
    // about the holder) against the rows the channel plays or blends to
    // (the guarding records' 40..45 and 18..20); and while it is on, the
    // wanted facing (+0xb0, where the capture has it) at the holder. Not
    // while a staged gamebreaker plays (the script manager's +0x50 state 3
    // to 7, schema 3): its script poses the six.
    use crate::guard;
    let Some(captures) = load() else {
        return;
    };
    const GUARDING: [i64; 9] = [40, 41, 42, 43, 44, 45, 18, 19, 20];
    let (mut moving, mut along) = (0u32, 0u32);
    let mut agree = [[0u32; 2]; 2];
    let (mut facing, mut at_holder) = (0u32, 0u32);
    let mut misses = Vec::new();
    for cap in captures {
        let mut state = [guard::Guard::default(); 6];
        for pair in cap.updates.windows(2) {
            let (before, u) = (&pair[0], &pair[1]);
            let staged = u.0["script_words"][8]
                .as_i64()
                .is_some_and(|s| (3..=7).contains(&s));
            if u.frame() != before.frame() + 1 || staged {
                state = [guard::Guard::default(); 6];
                continue;
            }
            // The gate runs on every computer player with his stick out
            // (`0x167220..0x1673f0`): outside a restart (game+0xf4 not 6),
            // with a holder of the other side, in the back court too;
            // otherwise not eligible, so the mode's end is stamped and
            // holds it off 36 updates (through a pass's flight, into the
            // front court: the School's lessons start in the back court).
            // Counted in the front court with the ball held.
            let held = matches!(u.phase(), 4 | 5) && u.ball_state() == 3;
            let h = cap.actor_of(u.holder()).filter(|_| held);
            let counted = u.phase() == 5;
            let side = |k: usize| cap.sides[cap.records[k].team.min(1)] as f32;
            for (k, g) in state.iter_mut().enumerate() {
                let a = u.actor(k);
                let team = cap.records[k].team;
                if u.controllers()[k] < 4 {
                    continue;
                }
                let lifted = a["position"][1].as_f64().unwrap_or(0.0) != 0.0;
                if lifted || u.word(k, "requested_intensity") < 16 || u.word(k, "bits") & !4 != 0 {
                    *g = guard::Guard::default();
                    continue;
                }
                let Some(h) = h.filter(|&h| team != cap.records[h].team) else {
                    guard::gate(g, u.frame(), false, false);
                    continue;
                };
                let stick = u.word(k, "requested_heading") as i32;
                let m = &a["motion"];
                let v = [m[0].as_f64().unwrap() as f32, m[2].as_f64().unwrap() as f32];
                if counted && libm::sqrtf(v[0] * v[0] + v[1] * v[1]) > 1e-4 {
                    moving += 1;
                    let word = guard::toward([0.0, 0.0], v);
                    along += u32::from(guard::turn(word, stick).abs() <= 2);
                }
                // The mode, as `World::guard_stance` runs it.
                let now = u.frame();
                let turbo = u.word(k, "held") & 0x2000 != 0;
                let (me, him) = (u.floor(k), u.floor(h));
                let word = |j: usize| {
                    let w = u.actor(j);
                    w.get("facing").unwrap_or(&w["heading"]).as_i64().unwrap() as i32
                };
                let close = guard::placed(me, him, word(h), [side(h), side(k), side(h)]);
                let mut on = guard::gate(g, now, !turbo, close);
                let toward = guard::toward(me, him);
                let hm = &u.actor(h)["motion"];
                let still = hm[0].as_f64() == Some(0.0) && hm[2].as_f64() == Some(0.0);
                if on && guard::record(g, now, stick, toward, still).is_none() {
                    on = false;
                    g.entered = -1;
                    g.left = now;
                }
                let channel = &a["channels"][0];
                let row = |key: &str| channel[key].as_i64().unwrap_or(-1);
                let guarding =
                    GUARDING.contains(&row("record")) || GUARDING.contains(&row("target"));
                if !counted {
                    continue;
                }
                agree[usize::from(on)][usize::from(guarding)] += 1;
                if on && let Some(f) = a.get("facing").and_then(Value::as_i64) {
                    facing += 1;
                    let at = guard::turn(f as i32, toward).abs() <= 32;
                    at_holder += u32::from(at);
                    if !at && misses.len() < 400 {
                        misses.push((
                            cap.name.clone(),
                            u.frame(),
                            k,
                            guard::turn(f as i32, toward),
                        ));
                    }
                }
            }
        }
    }
    let total = agree.iter().flatten().sum::<u32>();
    let share = |n: u32, of: u32| f64::from(n) / f64::from(of.max(1));
    let agreed = share(agree[0][0] + agree[1][1], total);
    report(
        "guards",
        json!({"moving": moving, "along_the_stick": share(along, moving),
            "mode_against_rows": agree, "agreed": agreed,
            "facing_updates": facing, "facing_at_holder": share(at_holder, facing),
            "facing_off": misses}),
    );
    assert!(moving > 1000, "{moving}");
    // The velocity along the stick within 2 units: 66,812 updates in the
    // fifteen played captures and the School's seven, 99.97%.
    assert!(share(along, moving) > 0.99, "{along} of {moving}");
    // The ported mode against the rows: 96.7% (96.6% in the fifteen; the
    // capture's row is the blend's source for a blend's first updates).
    // Before the gate ran through the flights and the back court as the
    // original's does, 94%.
    assert!(agreed > 0.95, "{agree:?}");
    // On, +0xb0 at the holder within 32 units: 91.5% of the 9,429 updates
    // that carry it (91.2% of the fifteen's 7,389; 85% before).
    if facing > 0 {
        assert!(share(at_holder, facing) > 0.9, "{at_holder} of {facing}");
    }
}

#[test]
fn the_receivers_hands_meet_the_ball_where_the_original_catches_in_play() {
    // The offence's tail ([`crate::touch`]): every pass caught by its
    // receiver in the played captures, the flight left in the update before
    // the catch (ball+0x64 less +0x60); and for those caught standing (the
    // catch clip on channel 0, its blend over: the running rows' clip
    // plays on a channel the capture does not keep), the near take
    // (`0x15bad0..0x15bd68`, bit 9 set by the row's script): the ball where
    // the last ball update left it within 1.0 of the pelvis or either hand
    // of the model as the last update posed it (the pose step runs after
    // the tail in `FUN_00156f48`): the clip at the float frame the channel
    // showed at the end of the update before, at that update's place,
    // heading word and roster body. The catch should come on the first
    // update the near take finds the ball. For comparison, the forearm-
    // and-hand rule of the sweep (`FUN_00137db8`, `0x15c2fc..`), which the
    // tail reaches only after the flight's end, on the update's own frame.
    let Some(captures) = load() else {
        return;
    };
    let bank = crate::pose::disc_bank();
    let Some(poses) = bank.get() else {
        return;
    };
    let mut leads: std::collections::BTreeMap<i64, u32> = std::collections::BTreeMap::new();
    let (mut checked, mut on_time, mut within, mut early, mut in_blend) = (0, 0, 0, 0, 0);
    let (mut hand_on_time, mut hand_within) = (0, 0);
    let mut cases = Vec::new();
    for cap in captures {
        let bodies = bodies(cap);
        for k in 1..cap.updates.len() {
            let (a, u) = (&cap.updates[k - 1], &cap.updates[k]);
            if u.frame() != a.frame() + 1
                || a.holder() != 0
                || u.holder() == 0
                || a.ball_state() != 4
            {
                continue;
            }
            let Some(i) = cap.actor_of(u.holder()) else {
                continue;
            };
            if cap.actor_of(a.ball()["target_player"].as_u64().unwrap_or(0)) != Some(i) {
                continue;
            }
            let flight = a.ball()["flight_updates"].as_i64().unwrap_or(0);
            let progress = a.ball()["progress"].as_i64().unwrap_or(0);
            *leads.entry(flight - progress).or_default() += 1;
            // The updates since the release, each tested on the samples
            // before it.
            let (mut near_first, mut hand_first) = (None, None);
            let mut posed = true;
            for w in (k.saturating_sub(progress as usize))..=k {
                if w < 2 {
                    posed = false;
                    break;
                }
                let (last, prev) = (&cap.updates[w - 1], &cap.updates[w - 2]);
                if last.frame() + 1 != cap.updates[w].frame() || prev.frame() + 1 != last.frame() {
                    posed = false;
                    break;
                }
                let c = &last.actor(i)["channels"][0];
                let record = c["record"].as_i64().unwrap_or(-1);
                let target = c["target"].as_i64().unwrap_or(-1);
                let catch_clip = timing::CATCH_ROWS
                    .iter()
                    .any(|r| i64::from(r.record) == record);
                if !catch_clip || (0..1144).contains(&target) {
                    if w == k {
                        posed = false;
                    }
                    continue;
                }
                let p = &last.actor(i)["position"];
                let place = crate::pose::Place {
                    position: [0, 1, 2].map(|q| p[q].as_f64().unwrap() as f32),
                    word: last.word(i, "heading") as i32,
                };
                let at = |s: &Update| {
                    let b = &s.ball()["position"];
                    [0, 1, 2].map(|q| b[q].as_f64().unwrap() as f32)
                };
                // The model as the update before posed it.
                let modelled = crate::pose::Frame {
                    record: record as u16,
                    time: c["float_frame"].as_f64().unwrap_or(0.0) as f32,
                    looping: false,
                };
                if near_first.is_none()
                    && let Some(pose) = poses.pose(modelled, bodies[i], place)
                {
                    let b = at(last);
                    let near = [
                        crate::pose::PELVIS,
                        crate::pose::RIGHT_HAND,
                        crate::pose::LEFT_HAND,
                    ]
                    .iter()
                    .any(|&j| {
                        crate::pose::length(std::array::from_fn(|q| {
                            pose.joints[j].position[q] - b[q]
                        })) <= crate::touch::NEAR
                    });
                    if near {
                        near_first = Some(w);
                    }
                }
                // The clip as the channel shows it at the end of this
                // update, for the sweep's hand rule.
                let shown = &cap.updates[w].actor(i)["channels"][0];
                let time = if shown["record"].as_i64() == Some(record) {
                    shown["float_frame"].as_f64().unwrap_or(0.0) as f32
                } else {
                    c["float_frame"].as_f64().unwrap_or(0.0) as f32
                };
                let frame = crate::pose::Frame {
                    record: record as u16,
                    time,
                    looping: false,
                };
                if hand_first.is_none()
                    && let Some(pose) = poses.pose(frame, bodies[i], place)
                    && let Some(hit) = crate::pose::sweep(
                        &pose,
                        at(prev),
                        at(last),
                        crate::ball::RADIUS,
                        poses.reach(bodies[i]),
                    )
                {
                    let hands = poses.state(record as u16, frame.time as i32);
                    let hand = hit.bones.1;
                    if (hands & 8 != 0 && hand == crate::pose::RIGHT_HAND)
                        || (hands & 4 != 0 && hand == crate::pose::LEFT_HAND)
                    {
                        hand_first = Some(w);
                    }
                }
            }
            if !posed {
                in_blend += 1;
                continue;
            }
            checked += 1;
            on_time += usize::from(near_first == Some(k));
            within += usize::from(near_first.is_some_and(|f| f.abs_diff(k) <= 1));
            early += usize::from(near_first.is_some_and(|f| f < k));
            hand_on_time += usize::from(hand_first == Some(k));
            hand_within += usize::from(hand_first.is_some_and(|f| f.abs_diff(k) <= 1));
            cases.push(json!({"capture": cap.name, "frame": u.frame(), "actor": i,
                "left": flight - progress,
                "near_first": near_first.map(|f| f as i64 - k as i64),
                "hand_first": hand_first.map(|f| f as i64 - k as i64)}));
        }
    }
    let leads_json: std::collections::BTreeMap<String, u32> =
        leads.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    report(
        "receivers_hands",
        json!({"catches by flight left": leads_json, "posed standing catches": checked,
            "near take first on the catch": on_time, "within one update": within,
            "near take before the catch": early,
            "sweep's hand rule first on the catch (this update's frame)": hand_on_time,
            "sweep's hand rule within one": hand_within,
            "caught in the blend or running (not posed)": in_blend, "cases": cases}),
    );
    eprintln!(
        "catches by flight left {leads:?}; posed standing catches {checked}: the near take first finds the ball on the catch {on_time}, within one {within}, before it {early} (the sweep's hand rule {hand_on_time}, within one {hand_within}); not posed {in_blend}"
    );
    if checked > 0 {
        assert!(on_time * 10 >= checked * 8, "{on_time} of {checked}");
        assert!(within * 10 >= checked * 8, "{within} of {checked}");
        assert!(early * 50 <= checked, "{early} of {checked}");
    }
}

/// The executable's bytes (the blend opcodes a channel's pc follows), with
/// `STREET_ISO`.
fn executable() -> Option<&'static [u8]> {
    static ELF: std::sync::OnceLock<Option<Vec<u8>>> = std::sync::OnceLock::new();
    ELF.get_or_init(|| {
        let path = std::env::var_os("STREET_ISO")?;
        let mut iso = street_assets::iso::Iso::open(path).ok()?;
        iso.read("/SLUS_206.51").ok()
    })
    .as_deref()
}

fn elf_byte(elf: &[u8], address: u32) -> Option<u8> {
    let w = street_assets::scripts::executable_words(elf, address & !3, 1).ok()?[0];
    Some((w >> ((address & 3) * 8)) as u8)
}

fn elf_word(elf: &[u8], address: u32) -> Option<u32> {
    Some(u32::from_le_bytes([
        elf_byte(elf, address)?,
        elf_byte(elf, address + 1)?,
        elf_byte(elf, address + 2)?,
        elf_byte(elf, address + 3)?,
    ]))
}

/// The blend opcode (`8e`, `FUN_001978b0`) whose operands end at `pc`
/// (the channel's next opcode while its span runs): its length in updates,
/// its start frame and its records (the primary, then one per selector).
fn blend_before(elf: &[u8], pc: u32) -> Option<(u8, i32, Vec<u32>)> {
    for k in 0..=2u32 {
        let op = pc.checked_sub(10 + 5 * k)?;
        if elf_byte(elf, op)? != 0x8e {
            continue;
        }
        let selectors = (0..k).all(|s| {
            elf_byte(elf, op + 1 + s)
                .is_some_and(|b| matches!(b, 0xf2 | 0xf8 | 0xec | 0xf5 | 0xf4 | 0xf3))
        });
        if !selectors {
            continue;
        }
        let frames = elf_byte(elf, op + 1 + k)?;
        let start = elf_word(elf, op + 2 + k)? as i32;
        let records = (0..=k)
            .map(|s| elf_word(elf, op + 6 + k + 4 * s))
            .collect::<Option<Vec<_>>>()?;
        return Some((frames, start, records));
    }
    None
}

/// The move whose clip `record` is: a blend target of the shot, dunk and
/// lay-up, pass, catch-row or trick scripts, else the gait.
fn clip_kind(record: i64) -> &'static str {
    use crate::trick::Step;
    static KINDS: std::sync::OnceLock<std::collections::HashMap<u16, &'static str>> =
        std::sync::OnceLock::new();
    let kinds = KINDS.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        let tables: [(&'static str, Vec<&'static [Step]>); 5] = [
            (
                "trick",
                crate::trick::SCRIPTS.iter().map(|s| s.steps).collect(),
            ),
            ("catch", crate::touch::CATCH_SCRIPTS.to_vec()),
            ("pass", crate::held::PASS_STEPS.to_vec()),
            ("dunk or lay-up", crate::held::MOVE_STEPS.to_vec()),
            ("shot", crate::held::SHOT_STEPS.to_vec()),
        ];
        for (kind, scripts) in tables {
            for steps in scripts {
                for step in steps {
                    if let Step::Blend { records, .. } | Step::Pick { records, .. } = *step {
                        for r in records {
                            m.insert(r, kind);
                        }
                    }
                }
            }
        }
        m
    });
    u16::try_from(record)
        .ok()
        .and_then(|r| kinds.get(&r).copied())
        .unwrap_or("gait or other")
}

/// The yaw quaternion of facing word `word` (the pose's).
fn yaw_of(word: i32) -> [f32; 4] {
    let theta = word as f32 * std::f32::consts::TAU / 1024.0;
    [
        0.0,
        crate::math::sin(theta * 0.5),
        0.0,
        crate::math::cos(theta * 0.5),
    ]
}

fn vec3(v: &Value) -> [f32; 3] {
    [0, 1, 2].map(|q| v[q].as_f64().unwrap() as f32)
}

/// `FUN_0014e060`'s ball for holder `h` of update `k`, from the channel
/// the capture keeps (see the test below); Err names why it is not posed.
fn held_point_from_capture(
    cap: &Capture,
    k: usize,
    h: usize,
    body: crate::pose::Body,
    poses: &crate::pose::Poses,
    elf: &[u8],
) -> Result<[f32; 3], &'static str> {
    let radius = crate::ball::RADIUS;
    let u = &cap.updates[k];
    let c = &u.actor(h)["channels"][0];
    let pc = c["pc"].as_i64().unwrap_or(0);
    let record = c["record"].as_i64().unwrap_or(-1);
    let flags = c["flags"].as_u64().unwrap_or(0);
    let blending = flags & 0x20_0000 != 0;
    if !(0..1144).contains(&record) || (!blending && c["target"].as_u64() != Some(0xffff_ffff)) {
        return Err("no record, or a target without the flag");
    }
    let place = crate::pose::Place {
        position: vec3(&u.actor(h)["position"]),
        word: u.word(h, "heading") as i32,
    };
    let time = c["float_frame"].as_f64().unwrap_or(0.0) as f32;
    if !blending {
        return crate::dribble::ball_point(poses, record as u16, time, body, place, radius)
            .ok_or("not posed");
    }
    let target = c["target"].as_u64().unwrap_or(0) as u32;
    let (frames, start, records) =
        blend_before(elf, pc as u32).ok_or("blend opcode not found at the pc")?;
    if !records.contains(&target) && records[0] != 0xffff_fc18 {
        return Err("blend target not the opcode's");
    }
    if start < 0 || target >= 1144 {
        return Err("special blend start");
    }
    // The update the blend began (this pc and target since).
    let same = |w: &Update| {
        let x = &w.actor(h)["channels"][0];
        x["pc"].as_i64() == Some(pc)
            && x["target"].as_u64() == Some(u64::from(target))
            && x["flags"].as_u64().unwrap_or(0) & 0x20_0000 != 0
    };
    let mut s = k;
    while s > 0
        && cap.updates[s - 1].frame() + 1 == cap.updates[s].frame()
        && same(&cap.updates[s - 1])
    {
        s -= 1;
    }
    let counter = (k - s + 1) as u16;
    if counter > u16::from(frames) || s == 0 {
        return Err("blend past its length, or at the capture's start");
    }
    let weight = f32::from(counter) / f32::from(frames.max(1));
    let from = crate::pose::Frame {
        record: record as u16,
        time,
        looping: true,
    };
    let to = crate::pose::Frame {
        record: target as u16,
        time: start as f32,
        looping: false,
    };
    let state = poses.state(target as u16, start);
    let point = if crate::dribble::dribbles(poses, target as u16) && state & 0xc == 0 {
        crate::dribble::ball_point(poses, target as u16, start as f32, body, place, radius)
    } else {
        poses
            .blend(from, to, weight, body, place)
            .map(|pose| crate::dribble::in_hand(&pose, target as u16, start, state, body, radius))
    }
    .ok_or("not posed")?;
    // The offset the blend's start stored (`FUN_00193300`): the ball and
    // the holder as the update before left them, unless the hands going
    // out (+0x14, that update's frame) and coming in meet; a blend begun
    // on a blend stores again.
    let b = &cap.updates[s - 1];
    if cap.actor_of(b.holder()) != Some(h) {
        return Ok(point);
    }
    let x = &b.actor(h)["channels"][0];
    let r = x["record"].as_i64().unwrap_or(-1);
    let out = if (0..1144).contains(&r) {
        poses.state(r as u16, x["frame"].as_i64().unwrap_or(0) as i32) & 0xc
    } else {
        0
    };
    let tgt = state & 0xc;
    let meet = out & tgt != 0 && (out == tgt || tgt == 4 || tgt == 8);
    let again = x["flags"].as_u64().unwrap_or(0) & 0x20_0000 != 0;
    if meet && !again {
        return Ok(point);
    }
    let p0 = vec3(&b.actor(h)["position"]);
    let b0 = vec3(&b.ball()["position"]);
    let q0 = yaw_of(b.word(h, "heading") as i32);
    let local = crate::pose::rotate(
        [-q0[0], -q0[1], -q0[2], q0[3]],
        std::array::from_fn(|q| b0[q] - p0[q]),
    );
    let o = crate::pose::rotate(yaw_of(place.word), local);
    Ok(std::array::from_fn(|q| {
        (place.position[q] + o[q]) * (1.0 - weight) + point[q] * weight
    }))
}

#[test]
fn the_held_ball_lies_on_the_holders_clip_in_play() {
    // `FUN_0014e060` ([`crate::held`]) against every update of the played
    // captures with a holder (ball+0xf4: state 3, and a pass's windup in
    // state 4), from the holder's channel 0 as the capture keeps it, his
    // place, heading word (+0xa8) and roster body. Outside a blend: the
    // clip at its float frame ([`crate::dribble::ball_point`]: in the hand
    // its frame holds out, or a dribble's flight). Through a blend (flag
    // 0x200000): its target at its start frame, the `8e` opcode ending at
    // the channel's pc (read from the executable), on the pose blended by
    // the counter (the updates since the blend began, plus one) over its
    // length, eased from the ball's offset the update before it began
    // unless the hands going out and coming in meet. By the move the pc
    // lies in; before: the rewrite's earlier rule (the dribble in the gait,
    // the carried point at the waist in every other move).
    let Some(captures) = load() else {
        return;
    };
    let bank = crate::pose::disc_bank();
    let (Some(poses), Some(elf)) = (bank.get(), executable()) else {
        return;
    };
    // By (move, blending): the errors, and the earlier rule's.
    type Errors = (Vec<f32>, Vec<f32>);
    let mut by_kind: std::collections::BTreeMap<(&str, bool), Errors> = Default::default();
    let mut skipped: std::collections::BTreeMap<&str, u32> = Default::default();
    let mut worst = Vec::new();
    for cap in captures {
        let bodies = bodies(cap);
        for k in 1..cap.updates.len() {
            let u = &cap.updates[k];
            if !matches!(u.ball_state(), 3 | 4) || u.holder() == 0 {
                continue;
            }
            let Some(h) = cap.actor_of(u.holder()) else {
                continue;
            };
            let c = &u.actor(h)["channels"][0];
            let blending = c["flags"].as_u64().unwrap_or(0) & 0x20_0000 != 0;
            let kind = clip_kind(if blending {
                c["target"].as_i64().unwrap_or(-1)
            } else {
                c["record"].as_i64().unwrap_or(-1)
            });
            let point = match held_point_from_capture(cap, k, h, bodies[h], poses, elf) {
                Ok(p) => p,
                Err(why) => {
                    *skipped.entry(why).or_default() += 1;
                    continue;
                }
            };
            let ball = vec3(&u.ball()["position"]);
            let e = crate::pose::length(std::array::from_fn(|q| point[q] - ball[q]));
            // Before: the dribble in the gait, else the carried point.
            let position = vec3(&u.actor(h)["position"]);
            let old = if kind == "gait or other" && !blending {
                point
            } else {
                let a = heading(u.word(h, "heading") as i32);
                [
                    position[0] + libm::sinf(a) * 0.9,
                    3.0 + position[1],
                    position[2] + libm::cosf(a) * 0.9,
                ]
            };
            let o = crate::pose::length(std::array::from_fn(|q| old[q] - ball[q]));
            let entry = by_kind.entry((kind, blending)).or_default();
            entry.0.push(e);
            entry.1.push(o);
            if e > 1.0 && worst.len() < 60 {
                worst.push(
                    json!({"capture": cap.name, "frame": u.frame(), "kind": kind,
                    "blending": blending, "record": c["record"], "target": c["target"],
                    "time": c["float_frame"], "error": e, "ours": point, "ball": ball}),
                );
            }
        }
    }
    let median = |v: &[f32]| {
        let mut v = v.to_vec();
        v.sort_by(f32::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    let within = |v: &[f32], d: f32| v.iter().filter(|&&e| e <= d).count();
    let rows: Vec<Value> = by_kind
        .iter()
        .map(|((kind, blending), (new, old))| {
            json!({"move": kind, "blending": blending, "updates": new.len(),
                "median": median(new), "within 0.25": within(new, 0.25),
                "within 0.5": within(new, 0.5), "before: median": median(old),
                "before: within 0.5": within(old, 0.5)})
        })
        .collect();
    for r in &rows {
        eprintln!("{r}");
    }
    eprintln!("skipped {skipped:?}");
    report(
        "held_ball",
        json!({"by move": rows, "skipped": skipped, "worst": worst}),
    );
    let all: Vec<f32> = by_kind
        .values()
        .flat_map(|(n, _)| n.iter().copied())
        .collect();
    assert!(!all.is_empty());
    assert!(median(&all) < 0.1, "median {}", median(&all));
}

/// The captured actors' roster handedness (field 13, actor+0xc0 2).
fn lefties(cap: &Capture) -> Vec<bool> {
    cap.records
        .iter()
        .map(|r| {
            r.player
                .and_then(disc_player)
                .is_some_and(|p| p.left_handed)
        })
        .collect()
}

/// The created players the captures meet, whose records are on no disc:
/// their id (`0x4000 +` the slot) and ratings (fields 16..26), and the
/// model height (field 10) of the record the game copied in. Save state
/// 3's career star (the story capture's actor 5), from the career block's
/// copy (`0x4bce20 + 0x6ec`, `FUN_001e7db8`'s +0x6ec + slot * 0x124: field
/// k at +0x42 + 2(k - 3)): 69 in., right-handed (field 13), no head scale
/// (fields 43..45), so the uniform branch of `0x315790` at 69.
const CREATED_STARS: [(usize, [i64; 11], i16); 1] =
    [(0x4000, [50, 20, 0, 20, 20, 20, 40, 60, 60, 20, 0], 69)];

/// A record's created star's model height, by [`CREATED_STARS`].
fn created_star(r: &Record) -> Option<i16> {
    let id = r.player?;
    CREATED_STARS
        .iter()
        .find(|(i, ratings, _)| *i == id && r.ratings == ratings)
        .map(|&(_, _, height)| height)
}

/// The disc's record whose persistent id (field 0) is `id`. The league's
/// players sit at their ids; the streetballers do not, and the generic
/// ballers (records 184..233) carry their record less 11 (a story game's
/// crews: the capture's 193 is record 204). A created player's id
/// (`0x4000 +` its slot) names no disc record.
fn disc_player(id: usize) -> Option<&'static street_assets::db::Player> {
    static PLAYERS: std::sync::OnceLock<Option<Vec<street_assets::db::Player>>> =
        std::sync::OnceLock::new();
    let players = PLAYERS
        .get_or_init(|| {
            let path = std::env::var_os("STREET_ISO")?;
            let mut iso = street_assets::iso::Iso::open(path).ok()?;
            street_assets::db::original_players(&mut iso).ok()
        })
        .as_ref()?;
    players
        .get(id)
        .filter(|p| p.id as usize == id)
        .or_else(|| players.iter().find(|p| p.id as usize == id))
}

/// The ball against its holder: across (his right), up and ahead of his
/// feet, `heading` his facing in the simulation's radians.
fn holder_frame(ball: [f32; 3], feet: [f32; 3], heading: f32) -> [f32; 3] {
    let (s, c) = (libm::sinf(heading), libm::cosf(heading));
    let (dx, dz) = (ball[0] - feet[0], ball[2] - feet[2]);
    [dx * c - dz * s, ball[1] - feet[1], dx * s + dz * c]
}

fn distance3(a: [f32; 3], b: [f32; 3]) -> f32 {
    crate::pose::length(std::array::from_fn(|q| a[q] - b[q]))
}

/// The held ball of the sim against the capture's through one replayed
/// move: per update against the holder's feet and facing, and in the court.
#[derive(Default)]
struct HeldTrace {
    relative: Vec<f32>,
    relative_before: Vec<f32>,
    court: Vec<f32>,
    release: Vec<f32>,
    release_before: Vec<f32>,
}

impl HeldTrace {
    /// Update `n` of a move whose press was captured update `press`: the
    /// sim's ball with holder `k` against the capture's, while both hold.
    /// `before` the rewrite's earlier point against the holder.
    fn update(
        &mut self,
        w: &World,
        cap: &Capture,
        press: i64,
        n: i64,
        k: usize,
        before: [f32; 3],
    ) -> bool {
        let Some(u) = cap.get(press + n) else {
            return false;
        };
        if cap.actor_of(u.holder()) != Some(k) || w.ball.state != BallState::Held(k) {
            return false;
        }
        let p = &w.players[k];
        let ours = holder_frame(
            w.ball.position,
            [p.position[0], p.height, p.position[1]],
            p.heading,
        );
        let theirs = holder_frame(
            vec3(&u.ball()["position"]),
            vec3(&u.actor(k)["position"]),
            heading(u.word(k, "heading") as i32),
        );
        self.relative.push(distance3(ours, theirs));
        self.relative_before.push(distance3(before, theirs));
        self.court
            .push(distance3(w.ball.position, vec3(&u.ball()["position"])));
        true
    }

    /// The release points: the ball against the holder on the last update
    /// each held it.
    fn released(&mut self, ours: Option<[f32; 3]>, theirs: Option<[f32; 3]>, before: [f32; 3]) {
        if let (Some(a), Some(b)) = (ours, theirs) {
            self.release.push(distance3(a, b));
            self.release_before.push(distance3(before, b));
        }
    }

    fn json(&self) -> Value {
        let median = |v: &[f32]| {
            let mut v = v.to_vec();
            v.sort_by(f32::total_cmp);
            v.get(v.len() / 2).copied().unwrap_or(0.0)
        };
        let within = |v: &[f32], d: f32| v.iter().filter(|&&e| e <= d).count();
        json!({"held updates": self.relative.len(),
            "against the holder, within 0.25": within(&self.relative, 0.25),
            "release points within 0.1": within(&self.release, 0.1),
            "before: release points within 0.1": within(&self.release_before, 0.1),
            "against the holder, median": median(&self.relative),
            "before: median": median(&self.relative_before),
            "in the court, median": median(&self.court),
            "releases": self.release.len(),
            "release point against the holder, median": median(&self.release),
            "before: release median": median(&self.release_before)})
    }
}

/// The capture's ball against holder `k` on the last update he held it,
/// from `press` on.
fn captured_release(cap: &Capture, press: i64, k: usize) -> Option<[f32; 3]> {
    let mut f = press;
    let mut last = None;
    while let Some(u) = cap.get(f) {
        if cap.actor_of(u.holder()) != Some(k) {
            break;
        }
        last = Some(holder_frame(
            vec3(&u.ball()["position"]),
            vec3(&u.actor(k)["position"]),
            heading(u.word(k, "heading") as i32),
        ));
        f += 1;
        if f - press > 150 {
            return None;
        }
    }
    last
}

/// A captured lob to a leaper: its leap's run, the update its ball was
/// launched (callback 38's `FUN_00162aa8`), the passer, the flight and
/// point the original stored (ball+0x64, +0x90).
struct Lob<'a> {
    run: &'a JumpRun,
    release: i64,
    passer: usize,
    flight: i64,
    point: [f32; 3],
}

/// Every lob to a leaper the played captures show: the first update of a
/// pass in the air (state 4, no holder, progress 1) to an actor in an
/// alley-oop row's run.
fn captured_lobs<'a>(captures: &[Capture], runs: &'a [JumpRun]) -> Vec<Lob<'a>> {
    let mut out = Vec::new();
    for run in runs.iter().filter(|r| jump_kind(r) == "alley-oop dunk") {
        let cap = &captures[run.capture];
        for n in 0..run.len as i64 {
            let Some(u) = cap.get(run.press + n) else {
                continue;
            };
            let ball = u.ball();
            let progress = ball["progress"].as_i64().unwrap_or(0);
            let to = ball["target_player"].as_u64().unwrap_or(0);
            if u.ball_state() != 4
                || u.holder() != 0
                || progress != 1
                || cap.actor_of(to) != Some(run.actor)
            {
                continue;
            }
            // A passout special to the leaper (game+0x160 set, -2 when
            // none: off the board or the head, `World::passout`) is not
            // `FUN_00162aa8`'s lob.
            if u.0["passout"][8].as_u64().is_some_and(|k| k != 0xffff_fffe) {
                break;
            }
            let Some(before) = cap.get(run.press + n - 1) else {
                break;
            };
            let Some(passer) = cap.actor_of(before.holder()) else {
                break;
            };
            let t = &ball["target"];
            out.push(Lob {
                run,
                release: run.press + n,
                passer,
                flight: ball["flight_updates"].as_i64().unwrap_or(0),
                point: std::array::from_fn(|k| t[k].as_f64().unwrap_or(0.0) as f32),
            });
            break;
        }
    }
    out
}

/// The order the actors of save state 1's match update in: the process
/// registry's (`FUN_0033ce00` runs it front to back; `FUN_0033cf98`
/// inserts each `AIPhysPlayer` at a random place among those already in,
/// `uVar8 + FUN_003a16b8() % (count + 1)`, so every match draws its own),
/// read from the state's RAM (the vector at `0x4deae8`): actors 5, 0, 4,
/// 3, 1, 2, then the backboards, AIDetect, the referee and the ball.
const SAVE_STATE_1_ORDER: [usize; 6] = [5, 0, 4, 3, 1, 2];

/// Whether `cap`'s match is save state 1's, whose registry order the
/// state's RAM gives ([`SAVE_STATE_1_ORDER`]). Every other match draws its
/// own (the fresh games, the School's, a story game's: save state 4's
/// first NYC Rec Center game ran 3, 4, 0, 5, 2, 1), which no capture keeps.
fn order_known(cap: &Capture) -> bool {
    cap.updates.first().is_some_and(|u| u.frame() >= 196)
        && cap
            .records
            .iter()
            .zip(SAVE_STATE_1)
            .all(|(r, id)| r.player == Some(id))
}

/// Whether `a` updates before `b` in `cap`'s match: by save state 1's
/// registry for a capture that plays on from it (its updates past the
/// state's 196; the fresh games are matches of their own), else the order
/// its replay worlds keep ([`crate::HIGHEST_FIRST`]), the higher index
/// first.
fn updates_before(cap: &Capture, a: usize, b: usize) -> bool {
    if order_known(cap) {
        let at = |k: usize| SAVE_STATE_1_ORDER.iter().position(|&j| j == k);
        at(a) < at(b)
    } else {
        a > b
    }
}

/// The leap of `lob` replayed from its press as [`replay_jump`] replays it
/// (the captured row, stick and stick only, the disc's animations), the
/// ball with its holder at the press, up to the update whose state of the
/// leaper the lob read: the release's own when the leaper updates before
/// the passer ([`updates_before`]; every captured lob whose flight the
/// leaper's span sets says so, see
/// [`the_lobs_to_leapers_are_timed_and_aimed_as_fun_00162aa8`]).
/// Whether one order of the six actors puts each pair's first before its
/// second (the pairs make no cycle).
fn one_order(pairs: &[(usize, usize)]) -> bool {
    let mut left: Vec<(usize, usize)> = pairs.to_vec();
    let mut placed = [false; 6];
    while !left.is_empty() {
        let Some(free) = (0..6).find(|&a| !placed[a] && !left.iter().any(|&(_, b)| b == a)) else {
            return false;
        };
        placed[free] = true;
        left.retain(|&(a, _)| a != free);
    }
    true
}

fn replay_leap_to_lob(cap: &Capture, lob: &Lob, leaper_first: bool) -> Option<World> {
    let run = lob.run;
    let before = cap.get(run.press - 1)?;
    let k = run.actor;
    let mut w = dressed(
        replay_world_for(
            cap,
            before,
            k,
            before.word(k, "heading") as i32,
            before.motion(k),
            before.word(k, "hand"),
        ),
        cap,
    );
    for j in 0..6 {
        w.players[j].height = before.actor(j)["position"][1].as_f64().unwrap_or(0.0) as f32;
    }
    let holder = cap.actor_of(before.holder())?;
    w.ball.hold(holder);
    w.possession = w.players[holder].team;
    let r = run.row?;
    let row = &timing::MOVE_ROWS[r];
    w.start_leap(k, r as u8);
    captured_start(&mut w, k, row.script, before);
    let input_at = |n: i64| {
        let mut all = vec![Input::default(); 6];
        if let Some(x) = cap.get(run.press + n) {
            all[k] = Input::moving(stick(
                x.word(k, "requested_heading") as i32,
                x.word(k, "requested_intensity") as i32,
            ));
        }
        all
    };
    w.previous_inputs = input_at(0);
    let last = lob.release - run.press - i64::from(!leaper_first);
    for n in 0..=last {
        w.step(&input_at(n));
        if !w.players[k].leaping() {
            return None;
        }
    }
    Some(w)
}

#[test]
fn the_lobs_to_leapers_are_timed_and_aimed_as_fun_00162aa8() {
    // Each captured lob to a leaper: his leap replayed from its press to
    // the update the lob read, then the release's flight and point
    // (`World::lob_to_leaper`) from the captured places of the passer and
    // the leaper as that update began (the game's distance table) against
    // the ball's flight (+0x64) and point (+0x90) the original launched.
    // The leaper's span reads one update more when he updates before the
    // passer: in save state 1's match by its registry, in any other (whose
    // order its construction drew and no capture keeps) by the order the
    // lob's own flight shows, all of a match's lobs on one order of the six.
    let Some(captures) = load() else {
        return;
    };
    if crate::pose::disc_bank().get().is_none() {
        return;
    }
    let runs = jump_runs(captures);
    let lobs = captured_lobs(captures, &runs);
    let mut rows = Vec::new();
    let mut flights_exact = 0;
    let mut offsets = Vec::new();
    // Per capture without a known order, the pairs a lob's flight orders.
    let mut orders: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for lob in &lobs {
        let cap = &captures[lob.run.capture];
        let Some(start) = cap.get(lob.release - 1) else {
            continue;
        };
        let at: Vec<[f32; 2]> = (0..6).map(|j| start.floor(j)).collect();
        let (k, passer) = (lob.run.actor, lob.passer);
        let tries: Vec<bool> = if order_known(cap) {
            vec![updates_before(cap, k, passer)]
        } else {
            vec![k > passer, k < passer]
        };
        let launches: Vec<(i32, [f32; 3], bool)> = tries
            .iter()
            .filter_map(|&first| {
                let w = replay_leap_to_lob(cap, lob, first)?;
                let (n, point) = w.lob_to_leaper(passer, k, &at);
                Some((n, point, first))
            })
            .collect();
        let hits: Vec<&(i32, [f32; 3], bool)> = launches
            .iter()
            .filter(|l| i64::from(l.0) == lob.flight)
            .collect();
        let Some(&(n, point, first)) = hits.first().copied().or(launches.first()) else {
            continue;
        };
        // Only a lob whose flight the order decides (the span's, one
        // update apart) orders the pair.
        if !order_known(cap) && hits.len() == 1 && launches.len() == 2 {
            let pair = if first { (k, passer) } else { (passer, k) };
            orders.entry(lob.run.capture).or_default().push(pair);
        }
        flights_exact += usize::from(i64::from(n) == lob.flight);
        let d = ((point[0] - lob.point[0]).powi(2)
            + (point[1] - lob.point[1]).powi(2)
            + (point[2] - lob.point[2]).powi(2))
        .sqrt();
        offsets.push(d);
        rows.push(json!({
            "capture": cap.name, "leaper": lob.run.actor, "passer": lob.passer,
            "release": lob.release, "row": jump_name(lob.run),
            "flight": [n, lob.flight], "point": [point, lob.point], "off": d,
        }));
    }
    let checked = offsets.len();
    let median = median_of(&offsets);
    eprintln!(
        "lobs {} checked {checked} flights exact {flights_exact} point off median {median}",
        lobs.len()
    );
    report(
        "lobs",
        json!({"lobs": lobs.len(), "checked": checked, "flights_exact": flights_exact,
        "point_off_median": median, "rows": rows}),
    );
    // The twelve captures hold four lobs to leapers, all the computer's
    // (2026-10-17): every flight exact (38, 43, 42, 39: the leaper's span
    // less five each time, the table's flight shorter), the point 0.08 to
    // 0.11 from the original's (median 0.08). The story game's nine
    // (2026-10-21) order actors 3, 4 and 5 so.
    assert!(checked >= 4, "{checked} of {} lobs replayed", lobs.len());
    assert_eq!(flights_exact, checked, "flights");
    for (c, pairs) in &orders {
        assert!(
            one_order(pairs),
            "{}: the lobs order the actors in a cycle {pairs:?}",
            captures[*c].name
        );
    }
    let worst = offsets.iter().copied().fold(0.0, f32::max);
    assert!(median < 0.12 && worst < 0.2, "point off {median} {worst}");
}

/// The disc's animations, gait records, the roster's bodies and
/// handedness on a replay world.
fn dressed(mut w: World, cap: &Capture) -> World {
    w.poses = crate::pose::disc_bank();
    w.gait = crate::ai::capture::disc_gait();
    for (j, (body, left)) in bodies(cap).into_iter().zip(lefties(cap)).enumerate() {
        w.players[j].body = Some(body);
        w.players[j].left_handed = left;
    }
    w
}

/// The script on holder `k`'s channel recorded as the capture shows its
/// start: the clip the update before the press showed, the live hand and
/// his side then.
fn captured_start(w: &mut World, k: usize, script: u32, before: &Update) {
    let c = &before.actor(k)["channels"][0];
    let record = c["record"].as_i64().unwrap_or(-1);
    let from = (0..1144).contains(&record).then(|| {
        (
            record as u16,
            c["float_frame"].as_f64().unwrap_or(0.0) as f32,
        )
    });
    let p = &w.players[k];
    w.players[k].channel_start = Some(crate::held::ChannelStart {
        script,
        from,
        left: before.word(k, "hand") == 2,
        side: crate::held::other_side(p),
    });
}

#[test]
fn the_persons_shots_and_passes_hold_the_ball_on_their_clips_in_play() {
    // Each person's shot (as the replay above: the captured script, the
    // button and the stick as captured) and pass (its captured row) replayed
    // through World::step with the disc's animations and gait records, the
    // roster's bodies and handedness, the script's start as the capture
    // shows it: every update from the press while both hold the ball, the
    // sim's held ball against the original's relative to the holder's feet
    // and facing (the hand on the clip) and in the court (with the sim's
    // own jump and drift), and the release point (the ball against the
    // holder on the last update he holds it). Before: the rewrite's carried
    // point at the waist and its own release points.
    let Some(captures) = load() else {
        return;
    };
    if crate::pose::disc_bank().get().is_none() {
        return;
    }
    let radius = crate::ball::RADIUS;
    let carried = [0.0, 3.0, 0.9];
    let mut shots = HeldTrace::default();
    for shot in human_shots(captures) {
        let Some(script) = shot.script.filter(|_| shot.started == 0) else {
            continue;
        };
        let cap = &captures[shot.capture];
        let (Some(update), Some(before)) = (cap.get(shot.press), cap.get(shot.press - 1)) else {
            continue;
        };
        let k = shot.actor;
        let mut w = dressed(replay_world(cap, &shot, update), cap);
        let input_at = |n: usize, w: &World| {
            let mut all = vec![Input::default(); w.players.len()];
            let (h, i) = shot.stick.get(n).copied().unwrap_or((0, 0));
            let held = cap
                .get(shot.press + n as i64)
                .map_or(0, |x| x.word(k, "held"));
            let mut mine = Input::moving(stick(h, i)).with(sim_buttons(held));
            if (n as i64) < shot.held {
                mine = mine.with(button::SHOOT);
            }
            all[k] = mine;
            all
        };
        w.step(&input_at(0, &w));
        let captured = &timing::SHOT_SCRIPTS[script];
        let Action::Shooting {
            script: s,
            release,
            check,
            end,
            gate,
            latest,
            ..
        } = &mut w.players[k].action
        else {
            continue;
        };
        *s = script as u8;
        *release = captured.release;
        *check = captured.check;
        *end = captured.end;
        *gate = captured.gate;
        *latest = captured.latest;
        captured_start(&mut w, k, captured.script, before);
        // The press update placed the ball on the sim's own pick: place it
        // again on the captured script's clip.
        w.place_held(k, None);
        let mut ours = None;
        for n in 0..150i64 {
            if n > 0 {
                w.step(&input_at(n as usize, &w));
            }
            if w.ball.state == BallState::Held(k) {
                let p = &w.players[k];
                ours = Some(holder_frame(
                    w.ball.position,
                    [p.position[0], p.height, p.position[1]],
                    p.heading,
                ));
            }
            if !shots.update(&w, cap, shot.press, n, k, carried)
                && w.ball.state != BallState::Held(k)
            {
                break;
            }
        }
        let before_release = [0.0, crate::tuning::RELEASE_HAND_HEIGHT + radius, 0.5];
        shots.released(ours, captured_release(cap, shot.press, k), before_release);
    }
    let mut passes = HeldTrace::default();
    for cap in captures {
        for u in &cap.updates {
            if !matches!(u.phase(), 4 | 5) {
                continue;
            }
            for (k, &controller) in u.controllers().iter().enumerate() {
                if controller == 4
                    || u.holder() != cap.actors[k]
                    || u.word(k, "pressed") & 0x100 == 0
                {
                    continue;
                }
                let press = u.frame();
                let (Some(row), Some(before)) = (
                    cap.get(press + 1).and_then(|x| pass_row_at(x.pc(k))),
                    cap.get(press - 1),
                ) else {
                    continue;
                };
                let mut w = dressed(
                    replay_world_for(
                        cap,
                        u,
                        k,
                        before.word(k, "heading") as i32,
                        before.motion(k),
                        before.word(k, "hand"),
                    ),
                    cap,
                );
                let input_at = |n: i64, w: &World| {
                    let mut all = vec![Input::default(); w.players.len()];
                    let x = cap.get(press + n);
                    let (h, i) = x.map_or((0, 0), |x| {
                        (
                            x.word(k, "requested_heading") as i32,
                            x.word(k, "requested_intensity") as i32,
                        )
                    });
                    let mut mine = Input::moving(stick(h, i));
                    if x.is_some_and(|x| x.word(k, "held") & 0x100 != 0) {
                        mine = mine.with(button::PASS);
                    }
                    all[k] = mine;
                    all
                };
                w.step(&input_at(0, &w));
                let Some(windup) = w.windup.as_mut().filter(|x| x.from == k) else {
                    continue;
                };
                // The captured row: its script and its callback 38.
                windup.row = row as u8;
                windup.left = timing::PASS_ROWS[row].release;
                if let Action::Passing { row: r, .. } = &mut w.players[k].action {
                    *r = row as u8;
                }
                captured_start(&mut w, k, timing::PASS_ROWS[row].script, before);
                w.place_held(k, None);
                let mut ours = None;
                for n in 0..120i64 {
                    if n > 0 {
                        w.step(&input_at(n, &w));
                    }
                    if w.ball.state == BallState::Held(k) {
                        let p = &w.players[k];
                        ours = Some(holder_frame(
                            w.ball.position,
                            [p.position[0], p.height, p.position[1]],
                            p.heading,
                        ));
                    }
                    if !passes.update(&w, cap, press, n, k, carried)
                        && w.ball.state != BallState::Held(k)
                    {
                        break;
                    }
                }
                let height = crate::PassStyle::Normal.release().release_height;
                passes.released(ours, captured_release(cap, press, k), [0.0, height, 0.8]);
            }
        }
    }
    let (s, p) = (shots.json(), passes.json());
    eprintln!("shots {s}\npasses {p}");
    report("held_moves", json!({"shots": s, "passes": p}));
    assert!(shots.relative.len() > 100 && passes.relative.len() > 50);
}

#[test]
fn the_live_hand_follows_the_held_clip_in_play() {
    // `FUN_0014e060`'s write of actor+0x25c ([`crate::dribble::live_hand`])
    // against the played captures: on every held update outside a blend,
    // the live hand predicted from the captured clip and frame and the
    // hand of the update before, against the hand the capture keeps.
    let Some(captures) = load() else {
        return;
    };
    let bank = crate::pose::disc_bank();
    let Some(poses) = bank.get() else {
        return;
    };
    let mut tally: std::collections::BTreeMap<String, (u32, u32)> = Default::default();
    for cap in captures {
        for k in 1..cap.updates.len() {
            let (b, u) = (&cap.updates[k - 1], &cap.updates[k]);
            if u.frame() != b.frame() + 1 || !matches!(u.ball_state(), 3 | 4) || u.holder() == 0 {
                continue;
            }
            let Some(h) = cap.actor_of(u.holder()) else {
                continue;
            };
            if cap.actor_of(b.holder()) != Some(h) {
                continue;
            }
            let c = &u.actor(h)["channels"][0];
            let record = c["record"].as_i64().unwrap_or(-1);
            if !(0..1144).contains(&record)
                || c["flags"].as_u64().unwrap_or(0) & 0x20_0000 != 0
                || c["target"].as_u64() != Some(0xffff_ffff)
            {
                continue;
            }
            let time = c["float_frame"].as_f64().unwrap_or(0.0) as f32;
            let before = b.word(h, "hand") as u8;
            let want = u.word(h, "hand") as u8;
            let got = crate::dribble::live_hand(poses, record as u16, time, before);
            let n = poses.frames(record as u16).unwrap_or(1) as i32;
            let state = poses.state(record as u16, (time.rem_euclid(n as f32) as i32).min(n - 1));
            let kind = format!(
                "{} {}",
                if crate::dribble::dribbles(poses, record as u16) {
                    "dribbling"
                } else {
                    "other"
                },
                match state & 0xc {
                    0xc => "both",
                    8 => "right",
                    4 => "left",
                    _ => "none",
                }
            );
            let e = tally.entry(kind).or_default();
            e.0 += 1;
            e.1 += u32::from(got == want);
        }
    }
    eprintln!("{tally:?}");
    report(
        "live_hand",
        json!(
            tally
                .iter()
                .map(|(k, (n, ok))| json!([k, n, ok]))
                .collect::<Vec<_>>()
        ),
    );
    let (n, ok) = tally.values().fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    assert!(ok * 100 >= n * 95, "{ok} of {n}");
}

/// A shot's, dunk's or lay-up's script on a captured actor: from the update
/// whose channel 0 first shows it (its tick 0, the press) for `len`
/// updates while his channel plays a shot or move script.
struct JumpRun {
    capture: usize,
    actor: usize,
    press: i64,
    len: usize,
    shot: Option<usize>,
    row: Option<usize>,
}

/// The move script whose body the pc lies in: a shot's id or a move
/// row's index (the first row with that script), and its address.
fn jump_script_at(pc: i64) -> Option<(Option<usize>, Option<usize>, u32)> {
    if let Some(script) = move_script_at(pc) {
        let row = timing::MOVE_ROWS.iter().position(|r| r.script == script)?;
        return Some((None, Some(row), script));
    }
    let s = shot_script_at(pc)?;
    Some((Some(s), None, timing::SHOT_SCRIPTS[s].script))
}

fn jump_runs(captures: &[Capture]) -> Vec<JumpRun> {
    let mut out = Vec::new();
    for (c, cap) in captures.iter().enumerate() {
        for k in 0..6 {
            let mut n = 0;
            while n < cap.updates.len() {
                let u = &cap.updates[n];
                let start = jump_script_at(u.pc(k)).filter(|&(_, _, s)| {
                    u.pc(k) - i64::from(s) < 0x60
                        && n > 0
                        && cap.updates[n - 1].frame() == u.frame() - 1
                        && jump_script_at(cap.updates[n - 1].pc(k)).is_none()
                });
                let Some((shot, row, _)) = start else {
                    n += 1;
                    continue;
                };
                let mut m = n;
                while m + 1 < cap.updates.len()
                    && cap.updates[m + 1].frame() == cap.updates[m].frame() + 1
                    && jump_script_at(cap.updates[m + 1].pc(k)).is_some()
                {
                    m += 1;
                }
                out.push(JumpRun {
                    capture: c,
                    actor: k,
                    press: u.frame(),
                    len: m - n + 1,
                    shot,
                    row,
                });
                n = m + 1;
            }
        }
    }
    out
}

/// The script's name of a run.
fn jump_name(run: &JumpRun) -> &'static str {
    match (run.shot, run.row) {
        (Some(s), _) => timing::SHOT_SCRIPTS[s].name,
        (_, Some(r)) => timing::MOVE_ROWS[r].name,
        _ => "",
    }
}

/// The kind a run is measured under, by its script's name.
fn jump_kind(run: &JumpRun) -> &'static str {
    let name = jump_name(run);
    if name.starts_with("allydunk") {
        "alley-oop dunk"
    } else if name.starts_with("dunk") {
        "dunk"
    } else if name.starts_with("layup") {
        "lay-up"
    } else if name.contains("sidefade") {
        "side fade"
    } else if name.contains("hook") {
        "hook"
    } else if name.contains("fadeaway") {
        "fadeaway"
    } else if name.contains("runner") {
        "runner"
    } else if name.contains("heave") || name.contains("baseball") {
        "heave"
    } else {
        "jump shot"
    }
}

fn median_of(v: &[f32]) -> f32 {
    let mut v = v.to_vec();
    if v.is_empty() {
        return f32::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// One run replayed: the height and travel offsets each update (up to a
/// dunk's hang on the rim), the tops, the heights at the release and the
/// landing updates (sim, capture); a hang's first update and its
/// offsets (height, travel) while the hands hold the rim.
struct JumpReplay {
    height: Vec<f32>,
    /// The sim's height less the captured, while both are in the air.
    offset: Vec<Option<f32>>,
    travel: Vec<f32>,
    top: [f32; 2],
    release: Option<[f32; 2]>,
    landed: [Option<i64>; 2],
    hung: Option<i64>,
    hang: Vec<[f32; 2]>,
}

fn replay_jump(cap: &Capture, run: &JumpRun) -> Option<JumpReplay> {
    let before = cap.get(run.press - 1)?;
    cap.get(run.press)?;
    let k = run.actor;
    let mut w = dressed(
        replay_world_for(
            cap,
            before,
            k,
            before.word(k, "heading") as i32,
            before.motion(k),
            before.word(k, "hand"),
        ),
        cap,
    );
    for j in 0..6 {
        w.players[j].height = before.actor(j)["position"][1].as_f64().unwrap_or(0.0) as f32;
    }
    // The generator as the capture left it before the press: a dunk's
    // hang on the rim (callback 28's `r % 100`) then draws where the
    // original's did, not on the replay world's own seed.
    w.rng = generator_of(before);
    // The looks mid-play, as settled as the capture's were (it does not
    // record them): no actor's first update turns his head for the replay
    // world's sake.
    w.settle_looks();
    let hoop = crate::target_hoop(w.players[k].team);
    let rim = w.rim_spot(hoop);
    let script = match (run.shot, run.row) {
        (Some(s), _) => {
            w.begin_script(k, timing::SHOT_SCRIPTS[s].script);
            w.players[k].start_shot(hoop, rim, 0, false, s);
            timing::SHOT_SCRIPTS[s].script
        }
        (_, Some(r)) => {
            let row = &timing::MOVE_ROWS[r];
            let clock = (r as u8, row.release, row.end);
            w.begin_move(k, r as u8);
            if row.family == 2 {
                let to = w.rim_side(k, hoop, 1.8);
                w.players[k].start_layup(hoop, to, clock);
            } else {
                let to = w.rim_side(k, hoop, 1.2);
                w.players[k].start_dunk(hoop, to, crate::player::DunkStyle::Normal, clock);
            }
            row.script
        }
        _ => return None,
    };
    captured_start(&mut w, k, script, before);
    w.place_held(k, None);
    let input_at = |n: i64| {
        let mut all = vec![Input::default(); 6];
        if let Some(x) = cap.get(run.press + n) {
            let mut mine = Input::moving(stick(
                x.word(k, "requested_heading") as i32,
                x.word(k, "requested_intensity") as i32,
            ));
            if x.word(k, "held") & 0x80 != 0 {
                mine = mine.with(button::SHOOT);
            }
            all[k] = mine;
        }
        all
    };
    w.previous_inputs = input_at(0);
    let start_sim = w.players[k].position;
    let start_cap = before.floor(k);
    let release_tick = match (run.shot, run.row) {
        (Some(s), _) => i64::from(timing::SHOT_SCRIPTS[s].release),
        (_, Some(r)) => i64::from(timing::MOVE_ROWS[r].release),
        _ => -1,
    };
    let mut out = JumpReplay {
        height: Vec::new(),
        offset: Vec::new(),
        travel: Vec::new(),
        top: [0.0; 2],
        release: None,
        landed: [None; 2],
        hung: None,
        hang: Vec::new(),
    };
    for n in 0..run.len as i64 {
        w.step(&input_at(n));
        // To the move's end in the sim (the capture's run reads on into the
        // scripts after it).
        if !crate::jump::scripted(&w.players[k].action) && n > 0 {
            break;
        }
        let Some(x) = cap.get(run.press + n) else {
            break;
        };
        let p = &w.players[k];
        let y = x.actor(k)["position"][1].as_f64().unwrap_or(0.0) as f32;
        let f = x.floor(k);
        let ds = [p.position[0] - start_sim[0], p.position[1] - start_sim[1]];
        let dc = [f[0] - start_cap[0], f[1] - start_cap[1]];
        let travel = libm::sqrtf((ds[0] - dc[0]).powi(2) + (ds[1] - dc[1]).powi(2));
        // A dunk's hang on the rim (callback 28): the hands planted with no
        // gravity. From there the rim's own give (the hoop model's bones,
        // `FUN_00149150`, not ported) moves the original's hands, so the
        // hang is measured apart.
        if out.hung.is_none() && p.flight.flags & crate::jump::NO_GRAVITY != 0 {
            out.hung = Some(n);
        }
        if out.hung.is_some() {
            if p.flight.flags & crate::jump::NO_GRAVITY != 0 {
                out.hang.push([(p.height - y).abs(), travel]);
            }
        } else {
            out.height.push((p.height - y).abs());
            out.offset
                .push((p.height > 0.0 && y > 0.0).then_some(p.height - y));
            out.travel.push(travel);
        }
        out.top = [out.top[0].max(p.height), out.top[1].max(y)];
        if n == release_tick {
            out.release = Some([p.height, y]);
        }
        if n > 2 && out.landed[0].is_none() && p.height <= 0.0 && out.top[0] > 0.0 {
            out.landed[0] = Some(n);
        }
        if n > 2 && out.landed[1].is_none() && y <= 0.0 && out.top[1] > 0.0 {
            out.landed[1] = Some(n);
        }
    }
    Some(out)
}

#[test]
fn the_jumps_follow_their_scripts_in_play() {
    // Every shot, dunk and lay-up the played captures show from its press
    // (the first update its script is on the actor's channel 0), replayed
    // through World::step with the disc's animations from the captured
    // update before: the same script (or row), the stick and the shoot
    // button as captured, the clip and hand his channel showed. Each
    // update the sim's height against the captured actor's and his travel
    // on the floor since the press against the captured; per run the
    // jump's top, the height at the release (callback 21: the shot's ball,
    // the dunk's slam) and the update he lands.
    let Some(captures) = load() else {
        return;
    };
    if crate::pose::disc_bank().get().is_none() {
        return;
    }
    let runs = jump_runs(captures);
    let mut by_kind: std::collections::BTreeMap<&str, Vec<(&JumpRun, JumpReplay)>> =
        Default::default();
    for run in &runs {
        if let Some(r) = replay_jump(&captures[run.capture], run) {
            by_kind.entry(jump_kind(run)).or_default().push((run, r));
        }
    }
    let mut summary = serde_json::Map::new();
    let mut detail = serde_json::Map::new();
    for (kind, list) in &by_kind {
        let heights: Vec<f32> = list
            .iter()
            .flat_map(|(_, r)| r.height.iter().copied())
            .collect();
        let travels: Vec<f32> = list
            .iter()
            .flat_map(|(_, r)| r.travel.iter().copied())
            .collect();
        let within = |v: &[f32], d: f32| {
            v.iter().filter(|&&x| x <= d).count() as f32 / v.len().max(1) as f32
        };
        let mean = |f: &dyn Fn(&JumpReplay) -> Option<f32>| {
            let v: Vec<f32> = list.iter().filter_map(|(_, r)| f(r)).collect();
            v.iter().sum::<f32>() / v.len().max(1) as f32
        };
        let landed = list
            .iter()
            .filter(|(_, r)| r.landed[0].is_some() && r.landed[0] == r.landed[1])
            .count();
        let landed_near = list
            .iter()
            .filter(|(_, r)| match r.landed {
                [Some(a), Some(b)] => (a - b).abs() <= 1,
                _ => false,
            })
            .count();
        // The rise each update after the takeoff's blend (the script's
        // first 6 updates, whose root lowers him: `jump::RESYNC`): the
        // change of the offset between two updates in the air.
        let steps: Vec<f32> = list
            .iter()
            .flat_map(|(_, r)| {
                r.offset
                    .windows(2)
                    .skip(5)
                    .filter_map(|w| Some((w[1]? - w[0]?).abs()))
            })
            .collect();
        let hung: Vec<&JumpReplay> = list
            .iter()
            .filter(|(_, r)| r.hung.is_some())
            .map(|(_, r)| r)
            .collect();
        let hang_height: Vec<f32> = hung
            .iter()
            .flat_map(|r| r.hang.iter().map(|h| h[0]))
            .collect();
        let hang_travel: Vec<f32> = hung
            .iter()
            .flat_map(|r| r.hang.iter().map(|h| h[1]))
            .collect();
        let hang_landed: Vec<i64> = hung
            .iter()
            .filter_map(|r| Some(r.landed[0]? - r.landed[1]?))
            .collect();
        summary.insert(
            kind.to_string(),
            json!({
                "runs": list.len(),
                "updates": heights.len(),
                "height_median": median_of(&heights),
                "height_within_0.1": within(&heights, 0.1),
                "rise_after_the_blend_within_0.01": within(&steps, 0.01),
                "travel_median": median_of(&travels),
                "travel_within_0.25": within(&travels, 0.25),
                "travel_at_end_median": median_of(
                    &list.iter().filter_map(|(_, r)| r.travel.last().copied()).collect::<Vec<_>>()
                ),
                "top": [mean(&|r| Some(r.top[0])), mean(&|r| Some(r.top[1]))],
                "at_release": [
                    mean(&|r| r.release.map(|x| x[0])),
                    mean(&|r| r.release.map(|x| x[1]))
                ],
                "landed_on_the_update": landed,
                "landed_within_1": landed_near,
                "hangs": hung.len(),
                "hang_height_median": median_of(&hang_height),
                "hang_travel_median": median_of(&hang_travel),
                "hang_landing_offsets": hang_landed,
            }),
        );
        detail.insert(
            kind.to_string(),
            Value::Array(
                list.iter()
                    .map(|(run, r)| {
                        json!({
                            "capture": captures[run.capture].name, "actor": run.actor,
                            "press": run.press, "script": jump_name(run),
                            "top": r.top, "release": r.release, "landed": r.landed,
                            "hung": r.hung,
                            "height_median": median_of(&r.height),
                            "travel_end": r.travel.last(),
                        })
                    })
                    .collect(),
            ),
        );
    }
    let summary = Value::Object(summary);
    eprintln!("jumps {}", serde_json::to_string_pretty(&summary).unwrap());
    report("jumps", json!({"summary": summary, "runs": detail}));
    assert!(runs.len() > 100, "{} runs", runs.len());
    // The flights on their scripts ([`crate::jump`], 2026-10-17; the
    // twelve captures' 175 runs). Height off the captured actor's, median
    // over the runs' updates: jump shots 0.50 -> 0.000 (33% -> 100% of
    // updates within 0.1), side fades 0.51 -> 0.000, hooks 0.31 -> 0.000,
    // lay-ups 1.31 -> 0.000, dunks 0.71 -> 0.004 (19% -> 66%), alley-oop
    // dunks 3.53 -> 0.004 (12% -> 80%), heaves 1.48 -> 0, runners 1.03 ->
    // 0.095 (the first update's blend, labelled in `docs/TRACE.md`). Travel
    // on the floor since the press, median: jump shots 1.38 -> 0.19, side
    // fades 3.09 -> 0.02, dunks 1.63 -> 0.12, heaves 5.9 -> 0.08. Dunks
    // leave the ball at 3.46 up against the captured 3.41 (4.5 before).
    // 2026-10-18, the fourteen, with save state 1's actors 0 and 4 as the
    // disc's players 55 and 95 (their bodies had been the plain 1.2):
    // dunks 98% of updates within 0.1, alley-oop dunks 99.8%. The runners'
    // offset is the takeoff blend's root (labelled; the held clip's root,
    // actor+0xe4, is not captured), formed over the blend and kept: it
    // scales with the body (two of player 95's six runs hold 0.103 off,
    // 0.097 and 0.095 on the plain body), so their share within 0.1 (54%;
    // 71% on the plain body) is not pinned;
    // after the blend every kind with a flight rises as the original
    // (97% of the runners' updates within 0.01).
    for (kind, s) in summary.as_object().unwrap() {
        let f = |key: &str| s[key].as_f64().unwrap_or(f64::NAN);
        let pair = |key: &str| [0, 1].map(|j| s[key][j].as_f64().unwrap_or(f64::NAN));
        let (close, share) = match kind.as_str() {
            "runner" => (0.15, None),
            _ => (0.1, Some(0.9)),
        };
        assert!(
            f("height_median") <= close,
            "{kind} height {}",
            f("height_median")
        );
        if let Some(share) = share {
            assert!(
                f("height_within_0.1") >= share,
                "{kind} within {}",
                f("height_within_0.1")
            );
        }
        if kind != "heave" {
            assert!(
                f("rise_after_the_blend_within_0.01") >= 0.95,
                "{kind} rise {}",
                f("rise_after_the_blend_within_0.01")
            );
        }
        assert!(
            f("travel_median") <= 0.25,
            "{kind} travel {}",
            f("travel_median")
        );
        let [ours, theirs] = pair("top");
        assert!((ours - theirs).abs() < 0.15, "{kind} top {ours} {theirs}");
        let [ours, theirs] = pair("at_release");
        assert!(
            (ours - theirs).abs() < 0.25,
            "{kind} release {ours} {theirs}"
        );
    }
}

/// What a press chose: a shot script, a dunk's or lay-up's script, a
/// pass's receiver, or nothing (the old rules' kinds, a dunk or a lay-up).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Chose {
    Shot(usize),
    Dunk,
    Layup,
    Move(u32),
    Pass(usize),
    Nothing,
}

impl Chose {
    fn name(self) -> String {
        match self {
            Chose::Shot(s) => timing::SHOT_SCRIPTS[s].name.to_string(),
            Chose::Dunk => "a dunk".into(),
            Chose::Layup => "a lay-up".into(),
            Chose::Move(a) => timing::MOVE_ROWS
                .iter()
                .chain(timing::EXTRA_ROWS.iter())
                .find(|r| r.script == a)
                .map_or(format!("{a:#x}"), |r| r.name.to_string()),
            Chose::Pass(k) => format!("pass to {k}"),
            Chose::Nothing => "nothing".into(),
        }
    }
}

/// The original's held word as the simulation's buttons (L1 0x4000, R1
/// 0x8000, L2 0x10000, R2 0x20000, L3 0x800).
fn sim_buttons(held: i64) -> u16 {
    [
        (0x4000, button::L1),
        (0x8000, button::R1),
        (0x10000, button::L2),
        (0x20000, button::R2),
        (0x800, button::L3),
    ]
    .into_iter()
    .filter(|&(bit, _)| held & bit != 0)
    .map(|(_, b)| b)
    .fold(0, |a, b| a | b)
}

/// The rewrite's old shoot press (before 2026-10-18): a dunk with a turbo
/// where a dunk was on short of the rim's line and the bar held 0.08, a
/// lay-up moving over 4 at the rim within 9, else the jump shot (its
/// runner from long range).
fn old_shoot(w: &mut World, k: usize, turbos: u32) -> Chose {
    let p = &w.players[k];
    let hoop = crate::target_hoop(p.team);
    let rim = w.rim_spot(hoop);
    let to_rim = crate::distance(p.position, rim);
    let d = crate::normalize2(crate::sub(rim, p.position));
    let f = p.forward();
    let moving_in = p.speed > 4.0 && d[0] * f[0] + d[1] * f[1] > 0.5;
    let dunk_on = w.dunk_on(k, true) == 2 && p.position[0].abs() <= 36.5;
    if turbos > 0 && dunk_on && w.turbo_of(k) >= 0.08 {
        return Chose::Dunk;
    }
    if moving_in && to_rim < 9.0 {
        return Chose::Layup;
    }
    let long = w.is_long_range(p.position, hoop);
    Chose::Shot(first_shot(timing::shot_script(w, k, 0, long, 0, long)))
}

/// The rewrite's old pass target (before 2026-10-18): Off the Heezay with
/// two turbos standing with a defender within 3.8 in front, Back 2 Papa
/// moving over 4 at the rim 6 to 18 out (both to himself); else the
/// teammate best lined up with the stick, or with the facing at rest.
fn old_pass(w: &World, k: usize, stick: [f32; 2], turbos: u32) -> Chose {
    let p = &w.players[k];
    let rim = w.rim_spot(crate::target_hoop(p.team));
    let to_rim = crate::distance(p.position, rim);
    let d = crate::normalize2(crate::sub(rim, p.position));
    let f = p.forward();
    let toward = d[0] * f[0] + d[1] * f[1] > 0.7;
    if turbos >= 2 {
        if p.speed < 2.0 && w.defender_in_front(k, 3.8).is_some() {
            return Chose::Pass(k);
        }
        if p.speed > 4.0 && toward && (6.0..18.0).contains(&to_rim) {
            return Chose::Pass(k);
        }
    }
    let aim = if crate::math::len(stick[0], stick[1]) * 64.0 >= crate::input::MOVES as f32 {
        crate::normalize2(stick)
    } else {
        f
    };
    (0..w.players.len())
        .filter(|&j| j != k && w.players[j].team == p.team)
        .map(|j| {
            let v = crate::sub(w.players[j].position, p.position);
            let n = crate::math::len(v[0], v[1]).max(1e-3);
            (j, (v[0] * aim[0] + v[1] * aim[1]) / n)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(Chose::Nothing, |(j, _)| Chose::Pass(j))
}

/// A shot script by its first id in [`timing::SHOT_SCRIPTS`] (the side
/// hooks are listed twice).
fn first_shot(s: usize) -> usize {
    let address = timing::SHOT_SCRIPTS[s].script;
    timing::SHOT_SCRIPTS
        .iter()
        .position(|x| x.script == address)
        .unwrap_or(s)
}

/// The simulation's choice at a press: its action or windup after one
/// update with the press's input.
fn sim_choice(w: &World, k: usize) -> Chose {
    match w.players[k].action {
        Action::Shooting { script, .. } => Chose::Shot(first_shot(usize::from(script))),
        Action::Dunking { row, .. } | Action::Layup { row, .. } => {
            timing::move_row(row).map_or(Chose::Nothing, |r| Chose::Move(r.script))
        }
        _ => match (w.windup, w.ball.pass) {
            (Some(p), _) if p.from == k => Chose::Pass(p.to),
            (_, Some(p)) if p.from == k && w.ball.state == BallState::Pass => Chose::Pass(p.to),
            _ => Chose::Nothing,
        },
    }
}

#[test]
fn the_persons_presses_choose_the_originals_moves() {
    // Every press of the person's with the ball in live play (phases 4 and
    // 5): shoot (+0x7c & 0x80) and pass (& 0x500). The original's choice:
    // the script his channel started (shot, dunk, lay-up), or the receiver
    // the pad builder wrote to actor+0x74 (`FUN_00146278`). The port's: the
    // match as the update before the press left it (the passer's facing
    // word +0xb0 where the capture carries it, else +0xa8), one update of
    // `World::step` with the press's held word and stick, over 32 offsets
    // of the generator (the draws are the original's, the generator's
    // state at the press is not); agreement when the original's choice is
    // among the port's. The old rules (the rewrite's before this port,
    // re-stated here) the same way. Presses in the air (a rebound jump, a
    // lay-up) are counted apart: the match is rebuilt standing; so are the
    // right stick's presses (0x400 without 0x100), whose angle (pad[5])
    // the captures do not keep, and the passes with a teammate leaping
    // for an alley-oop (bit 15, in the air), which the rebuilt match
    // stands on the floor: for those the original's target is met with
    // the leaper rule's (the leaper). The press carries the active
    // camera's yaw as its view (schema 2's `camera_yaw`, the helper's
    // +0x94, `0x146510`), which turns a pass at rest. The stick is the
    // press update's, or the update before's where the press update reads
    // it at rest and the update before did not (the D-pad held through
    // the press, which the actor's words do not keep). Where the two
    // teammates' angles from the press's direction (or their angles plus
    // distances) lie within the plain pick's bonuses (38 units: 9, 9, 11,
    // 9) of each other, the bonuses decide, and the lane bonus is the AI's
    // own test (`FUN_0017f4d0(ai, 0)`, cached once a frame at ai+0x90 and
    // +0x94) on the AI's matchups, which the captures do not keep: those
    // passes are met apart, most of them to agree. While a Level 2
    // gamebreaker's staged sequence plays (the script manager's +0x50
    // state 3 to 7, schema 3's `script_words`) the pad builder does not
    // run for its actors: their pad words stand as the press before it
    // left them (the School's capture holds a shoot press 100 updates),
    // so those updates are counted apart. So are the advanced dunk's
    // presses (the four-turbo slot's id 48 from 8.5 or more out, the
    // School's Advanced Dunks lesson): the original plays `bbpass_1`
    // (`0x447b53`, state bits 0x90 at the press, on a channel the capture
    // does not keep while channel 0 runs on), whose toss off the board and
    // finish (callbacks 31 and 32) the sim does not play; the port's
    // `timing::press_move` must name the advanced row on every draw.
    let Some(captures) = load() else {
        return;
    };
    let mut rows = Vec::new();
    let (mut shots, mut passes, mut air, mut aims) = ([0usize; 3], [0usize; 3], 0usize, 0usize);
    let mut staged = 0usize;
    let mut advanced = [0usize; 2];
    // Passes to a leaping teammate: the presses, the original's target the
    // leaper.
    let mut leapers = [0usize; 2];
    // Passes at rest with the camera's yaw known: the presses, the port
    // agreeing.
    let mut at_rest = [0usize; 2];
    // Passes the bonuses decide: the presses, the port agreeing.
    let mut close = [0usize; 2];
    for cap in captures {
        for u in &cap.updates {
            if !matches!(u.phase(), 4 | 5) {
                continue;
            }
            for (k, &controller) in u.controllers().iter().enumerate() {
                let pressed = u.word(k, "pressed");
                if controller == 4 || u.holder() != cap.actors[k] || pressed & 0x580 == 0 {
                    continue;
                }
                let press = u.frame();
                let Some(before) = cap.get(press - 1) else {
                    continue;
                };
                if u.0["script_words"][8]
                    .as_i64()
                    .is_some_and(|s| (3..=7).contains(&s))
                {
                    staged += 1;
                    continue;
                }
                let shoot = pressed & 0x80 != 0;
                let height = u.actor(k)["position"][1].as_f64().unwrap_or(0.0);
                let original = if shoot {
                    (0..=3)
                        .filter_map(|d| cap.get(press + d))
                        .find_map(|x| {
                            shot_script_at(x.pc(k))
                                .map(|s| Chose::Shot(first_shot(s)))
                                .or_else(|| move_script_at(x.pc(k)).map(Chose::Move))
                        })
                        .unwrap_or(Chose::Nothing)
                } else {
                    let target = u.actor(k)["pass_target"].as_u64().unwrap_or(0);
                    cap.actor_of(target).map_or(Chose::Nothing, Chose::Pass)
                };
                if height > 0.0 || before.word(k, "bits") & 0x2108 != 0 {
                    air += 1;
                    rows.push(json!({"capture": cap.name, "press": press, "air": true,
                        "original": original.name()}));
                    continue;
                }
                if pressed & 0x580 == 0x400 {
                    aims += 1;
                    rows.push(json!({"capture": cap.name, "press": press, "aim": true,
                        "original": original.name()}));
                    continue;
                }
                let team = u.actor(k)["team"].as_i64();
                let leaping: Vec<usize> = (0..u.controllers().len())
                    .filter(|&j| {
                        j != k
                            && u.actor(j)["team"].as_i64() == team
                            && u.word(j, "bits") & 0x8000 != 0
                            && u.actor(j)["position"][1].as_f64().unwrap_or(0.0) > 0.0
                    })
                    .collect();
                if !shoot && !leaping.is_empty() {
                    leapers[0] += 1;
                    let to_leaper = matches!(original, Chose::Pass(t) if leaping.contains(&t));
                    leapers[1] += usize::from(to_leaper);
                    rows.push(
                        json!({"capture": cap.name, "press": press, "leapers": leaping,
                        "original": original.name(), "to_leaper": to_leaper}),
                    );
                    continue;
                }
                let facing = before.actor(k)["facing"]
                    .as_i64()
                    .unwrap_or_else(|| before.word(k, "heading"))
                    as i32;
                let base = replay_world_for(
                    cap,
                    before,
                    k,
                    facing,
                    before.motion(k),
                    before.word(k, "hand"),
                );
                let held = u.word(k, "held");
                let stick_of = if u.word(k, "requested_intensity") < 16
                    && before.word(k, "requested_intensity") >= 16
                {
                    before
                } else {
                    u
                };
                let s = stick(
                    stick_of.word(k, "requested_heading") as i32,
                    stick_of.word(k, "requested_intensity") as i32,
                );
                let mut input = Input::moving(s).with(sim_buttons(held));
                input = input.with(if shoot { button::SHOOT } else { button::PASS });
                let yaw = u.0["camera_yaw"].as_f64();
                if let Some(yaw) = yaw {
                    input.view = (((yaw as f32 * 162.974_66 + 0.5) as i32) & 0x3ff) as u16;
                }
                let turbos = input.turbos();
                if shoot && original == Chose::Nothing && u.word(k, "bits") & 0x90 == 0x90 {
                    let named = (0..32).all(|n| {
                        let mut w = base.clone();
                        for _ in 0..n {
                            w.rng.next_u32();
                        }
                        matches!(
                            timing::press_move(&mut w, k, input.buttons),
                            timing::PressMove::Row { advanced: true, .. }
                        )
                    });
                    advanced[0] += 1;
                    advanced[1] += usize::from(named);
                    rows.push(
                        json!({"capture": cap.name, "press": press, "advanced_dunk": true,
                        "named_by_the_port": named}),
                    );
                    continue;
                }
                let mut ours = std::collections::BTreeSet::new();
                let mut old = std::collections::BTreeSet::new();
                for n in 0..32 {
                    let mut w = base.clone();
                    for _ in 0..n {
                        w.rng.next_u32();
                    }
                    let mut o = w.clone();
                    let mut inputs = vec![Input::default(); 6];
                    inputs[k] = input;
                    w.step(&inputs);
                    ours.insert(sim_choice(&w, k));
                    old.insert(if shoot {
                        old_shoot(&mut o, k, turbos)
                    } else {
                        old_pass(&o, k, s, turbos)
                    });
                }
                let kind = |c: Chose| match c {
                    Chose::Move(a) => {
                        timing::MOVE_ROWS
                            .iter()
                            .find(|r| r.script == a)
                            .map_or(c, |r| {
                                if r.family == 0 {
                                    Chose::Dunk
                                } else {
                                    Chose::Layup
                                }
                            })
                    }
                    c => c,
                };
                let agree = ours.contains(&original);
                let agreed_before = old.contains(&original) || old.contains(&kind(original));
                let resting = !shoot && stick_of.word(k, "requested_intensity") < 16;
                if resting && yaw.is_some() {
                    at_rest[0] += 1;
                    at_rest[1] += usize::from(agree);
                }
                // The press's direction as `World::pass_press` reads it.
                let word = if crate::math::len(s[0], s[1]) * 64.0 >= 15.5 {
                    crate::passing::stick_word(input.stick())
                } else {
                    (timing::facing_word(base.players[k].wanted) + i32::from(input.view)) & 0x3ff
                };
                let mates: Vec<(i32, f32)> = (0..base.players.len())
                    .filter(|&j| j != k && base.players[j].team == base.players[k].team)
                    .map(|j| {
                        let (a, b) = (base.players[k].position, base.players[j].position);
                        let d = timing::angle_diff(word, crate::contact::angle(a, b)).abs();
                        (d, d as f32 + crate::distance(a, b))
                    })
                    .collect();
                let decided_by_bonuses = !shoot
                    && mates.len() == 2
                    && ((mates[0].0 - mates[1].0).abs() <= 38
                        || (mates[0].1 - mates[1].1).abs() <= 38.0);
                if decided_by_bonuses {
                    close[0] += 1;
                    close[1] += usize::from(agree);
                }
                let tally = if shoot { &mut shots } else { &mut passes };
                tally[0] += 1;
                tally[1] += usize::from(agreed_before);
                tally[2] += usize::from(agree);
                rows.push(json!({
                    "capture": cap.name, "press": press, "kind": if shoot { "shoot" } else { "pass" },
                    "held": format!("{held:#x}"), "original": original.name(),
                    "port": ours.iter().map(|c| c.name()).collect::<Vec<_>>(),
                    "old": old.iter().map(|c| c.name()).collect::<Vec<_>>(),
                    "agree": agree, "agreed_before": agreed_before,
                    "view": yaw.map(|_| input.view), "at_rest": resting,
                    "decided_by_bonuses": decided_by_bonuses,
                }));
            }
        }
    }
    report(
        "presses",
        json!({"shoot": {"presses": shots[0], "before": shots[1], "after": shots[2]},
            "pass": {"presses": passes[0], "before": passes[1], "after": passes[2]},
            "passes_at_rest_with_the_yaw": {"presses": at_rest[0], "agree": at_rest[1]},
            "passes_to_a_leaper": {"presses": leapers[0], "to_the_leaper": leapers[1]},
            "passes_the_bonuses_decide": {"presses": close[0], "agree": close[1]},
            "in_the_air": air, "right_stick_presses": aims,
            "in_a_staged_sequence": staged,
            "advanced_dunks": {"presses": advanced[0], "named_by_the_port": advanced[1]},
            "presses_listed": rows}),
    );
    assert_eq!(advanced[1], advanced[0], "{advanced:?}");
    assert!(shots[0] >= 60 && passes[0] >= 5, "{shots:?} {passes:?}");
    assert_eq!(shots[2], shots[0], "{rows:?}");
    // Every pass the bonuses do not decide agrees.
    assert_eq!(passes[2] + close[0] - close[1], passes[0], "{rows:?}");
    assert_eq!(leapers[1], leapers[0], "{rows:?}");
    assert!(close[1] * 4 >= close[0] * 3, "{close:?} {rows:?}");
}

mod school;

#[test]
fn the_persons_dives_follow_the_originals_in_play() {
    // The Dive lesson's two dives in the fourth School capture (the
    // person's actor 3 at updates 7200 and 8316, both on the far script
    // 0x4576d1): the world of the update before the press (the six where
    // they stand and face, the loose ball's place and velocity; the second
    // dive's fumbler barred, as the original bars him), the dive pressed,
    // then World::step with the disc's animations against the captured
    // actor: his hop leaves the floor and lands on the original's updates,
    // his height within 0.01 of the original's every update (the hop's
    // velocity, the clip root's rise), his place within 0.45 (the others
    // run on the sim's own AI and push him where theirs did not).
    let Some(captures) = load() else {
        return;
    };
    let Some(cap) = captures.iter().find(|c| c.name.contains("school-3")) else {
        return;
    };
    if crate::pose::disc_bank().get().is_none() {
        return;
    }
    let mut checked = 0;
    for (press, k, fumbler) in [(7200i64, 3usize, None), (8316, 3, Some(2usize))] {
        let Some(u) = cap.get(press - 1) else {
            continue;
        };
        let players = (0..6)
            .map(|j| {
                Player::new(
                    cap.records[j].team.min(1),
                    u.floor(j),
                    heading(u.word(j, "heading") as i32),
                )
                .with_ratings(cap.records[j].ratings())
            })
            .collect();
        let mut w = dressed(
            World::new(Hoop::native(), players, Rules::default(), 1),
            cap,
        );
        w.phase = Phase::Live;
        w.people = u.controllers().iter().map(|&c| c != 4).collect();
        w.ball.drop_loose();
        let b = u.ball();
        let read = |key: &str, q: usize| b[key][q].as_f64().unwrap_or(0.0) as f32;
        w.ball.position = [0, 1, 2].map(|q| read("position", q));
        w.ball.velocity = [0, 1, 2].map(|q| read("velocity", q) * crate::TICK_HZ as f32);
        w.ball.airborne = true;
        w.game_phase.phase = crate::game_phase::phase::LOOSE;
        if let Some(f) = fumbler {
            w.players[f].barred = 40;
        }
        let (mut ours_up, mut ours_down, mut orig_up, mut orig_down) = (None, None, None, None);
        for t in 0..90 {
            let mut inputs: Vec<Input> = (0..6)
                .map(|j| {
                    if w.people[j] {
                        Input::default()
                    } else {
                        Input::computer()
                    }
                })
                .collect();
            if t == 0 {
                inputs[k] = Input::default().with(crate::button::DIVE);
            }
            w.step(&inputs);
            let f = press + t;
            assert!(
                matches!(w.players[k].action, crate::Action::Dive { .. }),
                "{f}: not diving"
            );
            let Some(c) = cap.get(f) else {
                continue;
            };
            let cp = &c.actor(k)["position"];
            let at = |q: usize| cp[q].as_f64().unwrap_or(0.0) as f32;
            let p = &w.players[k];
            if p.height > 0.0 && ours_up.is_none() {
                ours_up = Some(f);
            }
            if ours_up.is_some() && p.height == 0.0 && ours_down.is_none() {
                ours_down = Some(f);
            }
            if at(1) > 0.0 && orig_up.is_none() {
                orig_up = Some(f);
            }
            if orig_up.is_some() && at(1) == 0.0 && orig_down.is_none() {
                orig_down = Some(f);
            }
            assert!(
                (p.height - at(1)).abs() < 0.01,
                "{f}: height {} against {}",
                p.height,
                at(1)
            );
            let off = ((p.position[0] - at(0)).powi(2) + (p.position[1] - at(2)).powi(2)).sqrt();
            assert!(off < 0.45, "{f}: {off} off the original's place");
            checked += 1;
        }
        assert_eq!(ours_up, orig_up, "{press}: the hop");
        assert_eq!(ours_down, orig_down, "{press}: the landing");
    }
    assert!(checked > 100, "{checked} updates compared");
}
