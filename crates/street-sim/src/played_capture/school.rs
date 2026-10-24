//! Street School's lessons against captures of the original running them
//! (schema 3 of `tools/pine_played_capture.py`: the School game's lesson
//! manager, game+0x4c0..+0x500, its listener and the user's lesson words;
//! `%TEMP%/street-played-school-<n>.jsonl`, docs/TRACE.md "Street School
//! as a capture suite"). The captures play the lessons from the in-game
//! lesson menu with the pad, one person on his side.

use super::{Capture, SchoolSample, Update, load, report};
use crate::{Phase, Player, Rules, World, school, street::GamebreakerTask};
use serde_json::json;

/// One lesson of a capture: its launch and its results by update.
#[derive(Debug)]
struct Lesson {
    capture: usize,
    lesson: usize,
    /// The update of the launch (the first sample with the lesson set).
    launch: i64,
    /// The start the manager recorded (+0x4e4: the restart's end).
    start: Option<i64>,
    /// Each result the listener set: the update, the delay it left, the
    /// result.
    results: Vec<(i64, i64, bool)>,
    /// The update the lesson ended in (+0x4cc back to -1), the result it
    /// committed and the user's lesson nibble after it.
    commit: Option<(i64, bool, Option<u64>)>,
}

/// The user's nibble for `lesson` (`FUN_001eef88`: lesson 0 the high
/// nibble of the payload's fifth word, +0x68).
fn nibble(user: &[u64], lesson: usize) -> Option<u64> {
    let word = 4usize.checked_sub(lesson / 8)?;
    let shift = 28 - 4 * (lesson % 8);
    // `lesson_user` starts at +0x54: the flags, then the five words.
    user.get(1 + word).map(|w| (w >> shift) & 0xf)
}

fn lessons(captures: &[Capture]) -> Vec<Lesson> {
    let mut out = Vec::new();
    for (c, cap) in captures.iter().enumerate() {
        let mut open: Option<Lesson> = None;
        let mut last: Option<&SchoolSample> = None;
        for s in &cap.school {
            let (lesson, delay, result, start) = (s.words[3], s.words[4], s.words[5], s.words[9]);
            let before = last.replace(s);
            if lesson >= 0 && before.is_none_or(|b| b.words[3] != lesson) {
                out.extend(open.take());
                open = Some(Lesson {
                    capture: c,
                    lesson: lesson as usize,
                    launch: s.frame,
                    start: None,
                    results: Vec::new(),
                    commit: None,
                });
            }
            let Some(l) = open.as_mut() else {
                continue;
            };
            if lesson < 0 {
                l.commit = before.map(|b| (s.frame, b.words[5] != 0, nibble(&s.user, l.lesson)));
                out.extend(open.take());
                continue;
            }
            if start != -1 && l.start.is_none() {
                l.start = Some(start);
            }
            if before.is_some_and(|b| b.words[3] == lesson && delay > b.words[4]) {
                l.results.push((s.frame, delay, result != 0));
            }
        }
        out.extend(open);
    }
    out
}

#[test]
fn the_lessons_commit_their_results_after_the_managers_delay() {
    // `FUN_0018f6b0` sets +0x4d0 to `cvt.w.s(fps × 0.9)` (54), or `× 1.5`
    // (90) for Cancel Gamebreaker, and a later callback sets it again;
    // `FUN_0018eec0` counts it down an update at a time (holding it at 1
    // while a script plays) and commits: +0x4cc back to -1 the update the
    // count reaches 0, the user's nibble 8 for a pass (12 after a failure),
    // 4 for a failure (`FUN_001eecd8`). The game's update raises the
    // counter, then runs the shot clock, the meters and this manager, then
    // the actors and the ball post theirs: a sample read after the
    // update's manager shows `commit - frame` left, one read before it
    // `commit - frame + 1`. The simulation's lesson manager
    // (`street_sim::school`) keeps the same delays and order.
    let Some(captures) = load() else {
        return;
    };
    let runs = lessons(captures);
    if runs.is_empty() {
        return;
    }
    let mut rows = Vec::new();
    let mut off = Vec::new();
    let mut committed = 0;
    for l in &runs {
        let cap = &captures[l.capture];
        let Some((at, passed, nib)) = l.commit else {
            continue;
        };
        let Some(&(set, first, result)) = l.results.last() else {
            off.push((
                cap.name.clone(),
                l.lesson,
                "a commit with no result".to_owned(),
            ));
            continue;
        };
        committed += 1;
        let delay = i64::from(school::delay(l.lesson));
        // Every read from the result to the commit: the count down by one
        // an update (the read before or after the update's manager).
        let lags: Vec<i64> = cap
            .school
            .iter()
            .filter(|s| {
                !s.repeat && (set..at).contains(&s.frame) && s.words[3] >= 0 && s.words[4] > 0
            })
            .map(|s| at - s.frame - s.words[4])
            .collect();
        let steady = lags.iter().all(|&g| (-1..=0).contains(&g));
        let nibble_ok = nib.is_some_and(|n| if passed { n & 8 != 0 } else { n == 4 });
        rows.push(json!({
            "capture": cap.name, "lesson": l.lesson, "launch": l.launch, "start": l.start,
            "first_read": [set, first], "commit": at, "passed": passed, "nibble": nib,
            "results": l.results.len(), "lags": [lags.iter().min(), lags.iter().max()],
        }));
        if !steady || !(delay - 1..=delay).contains(&first) || passed != result || !nibble_ok {
            off.push((
                cap.name.clone(),
                l.lesson,
                format!("first read {first} at {set}, commit {at}, lags {lags:?}, passed {passed}/{result}, nibble {nib:?}"),
            ));
        }
    }
    report(
        "school_commits",
        json!({"lessons": runs.len(), "committed": committed, "off": off, "rows": rows}),
    );
    assert!(committed >= 40, "{committed}");
    assert!(off.is_empty(), "{off:?}");
}

/// The world the update before a lesson's launch shows: the six where they
/// stand, facing as they face, the ball with its holder, the person's
/// controller words.
fn world_at(cap: &Capture, u: &Update) -> World {
    let players = (0..6)
        .map(|j| {
            Player::new(
                cap.records[j].team.min(1),
                u.floor(j),
                super::heading(u.word(j, "heading") as i32),
            )
            .with_ratings(cap.records[j].ratings())
        })
        .collect();
    let mut w = World::new(crate::Hoop::native(), players, Rules::default(), 1);
    w.phase = Phase::Live;
    w.people = u.controllers().iter().map(|&c| c != 4).collect();
    // The coaches' team+0x88: the slots the computer drove.
    for team in 0..2 {
        let computer = std::array::from_fn(|k| !w.people[3 * team + k]);
        w.ai.coaches[team].set_computer(computer);
    }
    if let Some(h) = cap.actor_of(u.holder()) {
        w.ball.hold(h);
    }
    let b = &u.ball()["position"];
    w.ball.position = [0, 1, 2].map(|q| b[q].as_f64().unwrap_or(0.0) as f32);
    w
}

#[test]
fn the_lessons_launch_into_their_restart_spots_as_fun_0018ec28() {
    // The launch (`FUN_0018ec28`, from the lesson menu) runs the restart
    // for the lesson's side (vt+0x14c), snaps both teams to their restart
    // spots with the row forced to 2 (vt+0x154, `FUN_001407c8`;
    // `FUN_0012bc40`'s `0x12be00`) and refills the shot clock: the update
    // of the launch shows the six at the spots `World::launch_lesson` puts
    // them on from the update before it.
    let Some(captures) = load() else {
        return;
    };
    let runs = lessons(captures);
    if runs.is_empty() {
        return;
    }
    let mut rows = Vec::new();
    let (mut placed, mut checked) = (0, 0);
    for l in &runs {
        let cap = &captures[l.capture];
        // The menu holds the game: the launch's update is the held one, the
        // first update after it shows the spots.
        let next = cap
            .school
            .iter()
            .find(|s| !s.repeat && s.frame > l.launch)
            .map(|s| s.frame);
        let (Some(before), Some(after)) = (cap.get(l.launch), next.and_then(|f| cap.get(f))) else {
            continue;
        };
        let mut w = world_at(cap, before);
        let Some(person) = school::persons_actor(&w) else {
            continue;
        };
        let side = school::expected_side(l.lesson, w.team_of(person));
        w.launch_lesson(l.lesson as u8, side, false, &[]);
        let far: Vec<f32> = (0..6)
            .map(|j| {
                let a = w.players[j].position;
                let b = after.floor(j);
                ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
            })
            .collect();
        let holder = cap.actor_of(after.holder());
        let worst = far.iter().copied().fold(0.0_f32, f32::max);
        checked += 1;
        placed += usize::from(worst < 0.5 && holder == w.handler());
        rows.push(json!({
            "capture": cap.name, "lesson": l.lesson, "frame": after.frame(),
            "phase": after.phase(), "off": far, "holder": [holder, w.handler()],
            "possession": [after.0["possession"].clone(), side],
            "before": (0..6).map(|j| before.floor(j)).collect::<Vec<_>>(),
            "before_holder": cap.actor_of(before.holder()),
            "ball": before.0["ball"]["position"].clone(),
            "original": (0..6).map(|j| after.floor(j)).collect::<Vec<_>>(),
            "sim": (0..6).map(|j| w.players[j].position).collect::<Vec<_>>(),
        }));
    }
    report(
        "school_launches",
        json!({"checked": checked, "placed": placed, "rows": rows}),
    );
    assert_eq!(placed, checked, "{rows:?}");
}

/// A sample as the manager's readiness checks read it (`school::Court`):
/// the capture's words where it has them; the ones it lacks are the
/// dribble (actor+0x1a0, here 1 once the holder has stepped out of his
/// catch since his take: `FUN_00163d40` sets it at his first locomotion
/// record with the ball), the dunk test (the sim's on the sample's
/// positions) and the AI's front behaviour (taken as none). The distances
/// to the rim are the next sample's: the game's update fills its tables
/// (`FUN_0013f748`, game+0x374) from the positions before the manager runs.
struct Sample<'a> {
    cap: &'a Capture,
    u: &'a Update,
    tables: &'a Update,
    world: World,
    dribbling: Option<usize>,
}

impl Sample<'_> {
    fn possession(&self) -> usize {
        self.u.0["possession"].as_i64().unwrap_or(0).clamp(0, 1) as usize
    }
}

impl school::Court for Sample<'_> {
    fn holder(&self) -> Option<usize> {
        self.cap.actor_of(self.u.holder())
    }
    fn person(&self) -> Option<usize> {
        self.u.controllers().iter().position(|&c| c != 4)
    }
    fn team(&self, i: usize) -> usize {
        self.cap.records[i].team.min(1)
    }
    fn slot(&self, i: usize) -> usize {
        (0..i).filter(|&j| self.team(j) == self.team(i)).count()
    }
    fn in_slot(&self, team: usize, k: usize) -> Option<usize> {
        (0..6).filter(|&j| self.team(j) == team).nth(k)
    }
    fn position(&self, i: usize) -> [f32; 2] {
        self.u.floor(i)
    }
    fn hoop_dist(&self, i: usize) -> f32 {
        self.tables.distance(i)
    }
    fn between(&self, a: usize, b: usize) -> f32 {
        let (p, q) = (self.u.floor(a), self.u.floor(b));
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
    }
    fn angle_to_hoop(&self, i: usize) -> i32 {
        let side = self.cap.sides[self.possession()] as f32;
        let p = self.u.floor(i);
        crate::timing::angle_units(crate::math::atan2(p[0] - 36.75 * side, p[1]))
    }
    fn bits(&self, i: usize) -> u32 {
        self.u.word(i, "bits") as u32
    }
    fn dribbling(&self, i: usize) -> bool {
        self.dribbling == Some(i)
    }
    fn down(&self, i: usize) -> bool {
        self.u.actor(i)["channels"][0]["flags"]
            .as_u64()
            .unwrap_or(0)
            & 0x40
            != 0
    }
    fn dunk_on(&self, i: usize) -> i32 {
        self.world.dunk_on(i, true)
    }
    fn front_kind(&self, _i: usize) -> i32 {
        0
    }
    fn turbo(&self, team: usize) -> f32 {
        self.u.0["turbo"][crate::turbo::COMPUTER_SLOT[team.min(1)]]
            .as_f64()
            .unwrap_or(0.0) as f32
    }
    fn meter(&self, team: usize) -> f32 {
        self.u.0[format!("meter{team}")].as_f64().unwrap_or(0.0) as f32
    }
    fn pad(&self) -> u32 {
        school::Court::person(self).map_or(0, |p| self.u.word(p, "held") as u32)
    }
    fn phase(&self) -> i32 {
        self.u.phase() as i32
    }
    fn ball_state(&self) -> i32 {
        self.u.ball_state() as i32
    }
}

#[test]
fn the_prompt_follows_the_readiness_checks_as_fun_0018f740() {
    // Each update where the original's manager looked at the prompt
    // (`FUN_0018eec0`: no result pending, the cooldown +0x4e0 run out, the
    // expected side with the ball), its decision is in the words it left:
    // with the prompt down (+0x4d8 1) and the lesson started (+0x4e4 set)
    // it showed it (+0x4d8 0); with it up it hid it (+0x4e0 back to 45).
    // The manager of update `f` reads the court the update before left:
    // the sample at `f - 1`, `f - start` updates after the start.
    // `school::ready` on that sample makes the same decision: in the seven
    // captures 16,118 of 16,162 (99.7%, every lesson 95% or better); the
    // rest sit an update off the original's at a take or a hand's move,
    // where the samples cannot show the dribble word.
    let Some(captures) = load() else {
        return;
    };
    let mut rows: std::collections::BTreeMap<usize, [usize; 2]> = Default::default();
    let mut off = Vec::new();
    let hold = i64::from(school::prompt_hold());
    for cap in captures {
        let samples: Vec<&SchoolSample> = cap.school.iter().filter(|s| !s.repeat).collect();
        let mut taken: Option<(usize, bool)> = None;
        let mut last_holder = None;
        for pair in samples.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let Some(u) = cap.get(a.frame) else {
                continue;
            };
            // The dribble, as near as the samples show it.
            let holder = cap.actor_of(u.holder());
            if holder != last_holder {
                taken = holder.map(|h| (h, false));
                last_holder = holder;
            } else if let (Some((h, moved)), Some(prev)) = (taken.as_mut(), cap.get(a.frame - 1))
                && !*moved
                && u.word(*h, "bits") & 0x200 == 0
            {
                let (p, q) = (u.floor(*h), prev.floor(*h));
                *moved = (p[0] - q[0]).abs() + (p[1] - q[1]).abs() > 1e-4;
            }
            let Some(next) = cap.get(b.frame) else {
                continue;
            };
            if b.frame != a.frame + 1 || u.0["torn"].as_bool() == Some(true) {
                continue;
            }
            let (lesson, pending, may_show, cooldown, start) =
                (a.words[3], a.words[4], a.words[6], a.words[8], a.words[9]);
            if lesson < 0 || b.words[3] != lesson || pending != 0 || cooldown > 1 {
                continue;
            }
            let lesson = lesson as usize;
            let sample = Sample {
                cap,
                u,
                tables: next,
                world: world_at(cap, u),
                dribbling: taken.and_then(|(h, moved)| moved.then_some(h)),
            };
            let Some(person) = school::Court::person(&sample) else {
                continue;
            };
            let expected = school::expected_side(lesson, school::Court::team(&sample, person));
            let possession = u.0["possession"].as_i64().unwrap_or(-1);
            if possession != -1 && possession != expected as i64 {
                continue;
            }
            let decided = if may_show != 0 {
                if start == -1 {
                    continue;
                }
                b.words[6] == 0
            } else {
                b.words[8] != hold
            };
            let ours = school::ready(lesson, &sample, b.frame - start);
            let row = rows.entry(lesson).or_default();
            row[0] += 1;
            if ours == decided {
                row[1] += 1;
            } else if off.len() < 60 {
                off.push(json!({
                    "capture": cap.name, "frame": b.frame, "lesson": lesson, "original": decided,
                    "ours": ours, "shown": may_show == 0, "holder": holder,
                    "bits": holder.map(|h| u.word(h, "bits")), "dribbling": sample.dribbling,
                }));
            }
        }
    }
    let (looked, agreed) = rows.values().fold((0, 0), |(n, k), r| (n + r[0], k + r[1]));
    report(
        "school_prompts",
        json!({"looked": looked, "agreed": agreed, "by_lesson": rows, "off": off}),
    );
    if looked == 0 {
        return;
    }
    assert!(looked > 1000, "{looked}");
    assert!(agreed * 100 >= looked * 99, "{agreed}/{looked}: {off:?}");
}

#[test]
fn the_gamebreaker_tasks_end_as_fun_00144ef8() {
    // Each gamebreaker task the School's captures hold (game+0x434: the
    // Gamebreaker lesson's shot, the Level 2 lesson's staged script), its
    // words (+0x2c busy, +0x30 state, +0x34 wait, +0x38 kind) update by
    // update against [`GamebreakerTask::step`] from the press: the shot's
    // waits 60 (40 for a dunker), asks for its two replays and posts its
    // end 63 updates after the press, its meter emptied that update (no
    // replay shown in a lesson); the staged one posts it the update after
    // the script manager's state leaves 3..7. The simulation's press runs
    // the task's first update in its own step (the process runs before
    // the next game update), so its end comes 62 of its steps on.
    let Some(captures) = load() else {
        return;
    };
    let mut rows = Vec::new();
    let mut checked = [0usize; 2];
    for cap in captures {
        let task = |u: &Update| u.0["gb_task"].as_u64().unwrap_or(0) != 0;
        let words = |u: &Update| -> Vec<i64> {
            (0..7)
                .map(|k| u.0["gb_task_words"][k].as_i64().unwrap_or(0))
                .collect()
        };
        // The script manager's state as the update left it (its last
        // read, a repeat one included).
        let script = |frame: i64| {
            cap.school
                .iter()
                .rev()
                .find(|s| s.frame == frame)
                .map(|s| s.script)
        };
        let mut k = 1;
        while k < cap.updates.len() {
            if !task(&cap.updates[k]) || task(&cap.updates[k - 1]) {
                k += 1;
                continue;
            }
            let first = &cap.updates[k];
            let press = first.frame() - 1;
            // The end: the busy word clear, or the task deleted (the game's
            // update deletes it once the word is clear).
            let Some(end) = (k..cap.updates.len())
                .find(|&j| !task(&cap.updates[j]) || words(&cap.updates[j])[0] == 0)
            else {
                break;
            };
            let kind = words(first)[3];
            // The holder as the press's update left him (bit 7, the dunk).
            let dunking = cap.updates[k - 1].holder() != 0
                && cap
                    .actor_of(cap.updates[k - 1].holder())
                    .is_some_and(|h| cap.updates[k - 1].word(h, "bits") & 0x80 != 0);
            let mut sim = GamebreakerTask {
                kind: kind as u8,
                state: 0,
                count: 0,
            };
            let mut off = Vec::new();
            let mut ended = None;
            for j in k..=end {
                let u = &cap.updates[j];
                let staged = script(u.frame() - 1).is_some_and(|s| (3..=7).contains(&s));
                if sim.step(dunking, staged) {
                    ended = Some(u.frame());
                    break;
                }
                let w = words(u);
                if j < end && (i64::from(sim.state), i64::from(sim.count)) != (w[1], w[2]) {
                    off.push((
                        u.frame(),
                        [sim.state as i64, sim.count as i64],
                        [w[1], w[2]],
                    ));
                }
            }
            let gone = cap.updates[end].frame();
            let shooter = cap.actor_of(words(first)[6] as u64).unwrap_or(0);
            let side = cap.records[shooter].team.min(1);
            let meter = |u: &Update| u.0[format!("meter{side}")].as_f64().unwrap_or(-1.0);
            let emptied = meter(&cap.updates[end]) == 0.0;
            rows.push(json!({
                "capture": cap.name, "press": press, "kind": kind, "dunking": dunking,
                "ended": ended, "gone": gone, "after_the_press": gone - press,
                "meter_before": meter(&cap.updates[end - 1]), "meter_at_end": meter(&cap.updates[end]),
                "off": off,
            }));
            assert!(off.is_empty() && ended == Some(gone) && emptied, "{rows:?}");
            if kind == 1 {
                let wait = crate::street::GAMEBREAKER_WAIT[usize::from(dunking)];
                assert_eq!(gone - press, 3 + i64::from(wait), "{rows:?}");
            }
            checked[usize::from(kind == 1)] += 1;
            k = end + 1;
        }
    }
    report(
        "gamebreaker_tasks",
        json!({"staged": checked[0], "shots": checked[1], "rows": rows}),
    );
    assert!(checked[1] >= 1 && checked[0] >= 1, "{checked:?}");
}

#[test]
fn the_staged_level_2s_replay_command_plays_as_the_games_replay() {
    // The School's Level 2 run (its script found by the script manager's
    // pointer, +0x34 less the bank's +0x30: the bank's `GamLR2`): on its
    // last frame its replay command (category 1, opcode 0, at its frame
    // plus its span) put the script manager in its state 5 and asked the
    // replay manager for the span (`FUN_00334b30`), and the next kept
    // sample shows the replay over: the script manager still in state 5
    // with its next state 3 waiting (`FUN_003362a0` at the replay's end,
    // mode 7), its replay frame +0x5c moved on, the manager recording
    // again with its shown count (+0x3c), speed (+0x40) and frames left
    // (+0x64) as the replay left them. The simulation's manager and the
    // script manager's replay ([`crate::replay`]), asked as `update` asks
    // them, leave the same, bit for bit; and the replay held the game's
    // count for its updates on the wall clock.
    let Some(captures) = load() else {
        return;
    };
    let Some(path) = std::env::var_os("STREET_ISO") else {
        return;
    };
    use crate::replay::{Gates, Manager, Objects, Slot, StagedReplay, mode};
    use street_assets::timeline::{Payload, ScriptBank};
    let bank = ScriptBank::read(&mut street_assets::iso::Iso::open(path).unwrap()).unwrap();
    let word = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let script = |u: &Update, k: usize| u.0["script_words"][k].as_i64().unwrap_or(-1);
    let replay = |u: &Update, k: usize| u.0["replay_words"][k].as_u64().unwrap_or(0) as u32;
    let mut rows = Vec::new();
    for cap in captures {
        for k in 1..cap.updates.len() {
            let (before, u) = (&cap.updates[k - 1], &cap.updates[k]);
            if script(u, 8) != 5 || script(before, 8) != 3 {
                continue;
            }
            let offset = script(u, 1) - script(u, 0);
            let (index, timeline) = bank
                .scripts
                .iter()
                .enumerate()
                .find(|(_, s)| i64::from(s.offset) == offset)
                .unwrap();
            let (from, frames, bytes) = timeline
                .records
                .iter()
                .find_map(|r| match &r.payload {
                    Payload::Command(c) if c.dispatch == 1 && c.opcode == 0 => {
                        Some((r.frame, r.duration, c.bytes.clone()))
                    }
                    _ => None,
                })
                .unwrap();
            let percent = match word(&bytes, 16) {
                p @ 25..=150 => p as u8,
                _ => 100,
            };
            // The model: a full ring, the request and the speed, then the
            // updates until the manager's end (the script manager's step
            // first in each, as in `update`).
            let mut m = Manager::default();
            for t in 0..300 {
                m.update(
                    Slot {
                        tick: t,
                        ..Slot::default()
                    },
                    &Objects::default,
                    &crate::replay::Director::default(),
                    Gates::default(),
                );
            }
            let held = Gates {
                scripted: true,
                task_busy: true,
            };
            let mut staged = StagedReplay::new(index, false, from, percent, word(&bytes, 8) as u8);
            m.request(1.0, frames as usize, 0, mode::STAGED, held);
            m.set_speed_now(f32::from(percent) / 100.0);
            let (mut busy, mut updates) = (0u32, 0u32);
            loop {
                if m.busy() {
                    staged.step();
                }
                m.update(
                    Slot::default(),
                    &Objects::default,
                    &crate::replay::Director::default(),
                    held,
                );
                updates += 1;
                busy += u32::from(m.busy());
                if m.ended {
                    break;
                }
                assert!(updates < 1000);
            }
            // The update asking to stop is busy and steps nothing.
            let shown = busy - 1;
            let wall = (u.0["wall_time_ns"].as_u64().unwrap()
                - before.0["wall_time_ns"].as_u64().unwrap()) as f64
                / 1e9;
            let expect = f64::from(updates) / 59.94;
            let row = json!({
                "capture": cap.name, "frame": u.frame(), "script": timeline.name,
                "span": [from, from + frames], "percent": percent,
                "last_frame": script(before, 10), "replay_frame": [script(u, 11), staged.frame],
                "shown": [replay(u, 5), shown],
                "speed": [replay(u, 6), m.speed().to_bits()],
                "remaining": [replay(u, 15), m.remaining().to_bits()],
                "wall_s": wall, "updates": updates, "model_s": expect,
            });
            assert_eq!(script(before, 10), i64::from(from + frames), "{row}");
            assert_eq!(
                (script(u, 9), replay(u, 0), replay(u, 2)),
                (3, 1, 0),
                "{row}"
            );
            assert_eq!(replay(u, 5), shown, "{row}");
            assert_eq!(replay(u, 6), m.speed().to_bits(), "{row}");
            assert_eq!(replay(u, 15), m.remaining().to_bits(), "{row}");
            assert_eq!(script(u, 11), i64::from(staged.frame), "{row}");
            assert!((wall - expect).abs() < 0.05, "{row}");
            rows.push(row);
        }
    }
    report("staged_replays", json!({ "rows": rows }));
    assert!(!rows.is_empty());
}

#[test]
fn a_lessons_gamebreaker_replays_play_on_the_wall_clock() {
    // A lesson shows the gamebreaker task's two replays as any game does:
    // between two kept samples a game update apart the manager's shown
    // count (+0x3c) went 0 to 109, then 109 to 166 (0.8125 and 0.53125
    // over the last 90 frames), and the wall clock ran their updates (the
    // capture tool keeps no sample while only the manager moves).
    let Some(captures) = load() else {
        return;
    };
    let replay = |u: &Update| u.0["replay_words"][5].as_u64().unwrap_or(0);
    let mut rows = Vec::new();
    for cap in captures {
        for k in 1..cap.updates.len() {
            let (before, u) = (&cap.updates[k - 1], &cap.updates[k]);
            let (a, b) = (replay(before), replay(u));
            if a == b || u.0["script_words"][8].as_i64() == Some(5) {
                continue;
            }
            let wall = (u.0["wall_time_ns"].as_u64().unwrap()
                - before.0["wall_time_ns"].as_u64().unwrap()) as f64
                / 1e9;
            // Its updates: those shown, the one asking to stop and the one
            // restoring the live game.
            let expect = (b + 2) as f64 / 59.94;
            rows.push(json!({
                "capture": cap.name, "frames": [before.frame(), u.frame()],
                "shown": [a, b], "wall_s": wall, "model_s": expect,
            }));
            assert!(u.frame() - before.frame() <= 1, "{rows:?}");
            assert!((wall - expect).abs() < 0.06, "{rows:?}");
        }
    }
    report("lesson_replays", json!({ "rows": rows }));
    if !rows.is_empty() {
        assert!(rows.len() >= 2, "{rows:?}");
    }
}
