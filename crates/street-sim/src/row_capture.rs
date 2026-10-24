//! The rows a captured original drew for its passes, catches, dunks and
//! lay-ups against the sim's row filters on the same state (the sim's
//! frame is the original's): the sector the filter reads (`FUN_0018d768` through
//! [`timing::pass_sector`], [`timing::catch_bit`],
//! [`timing::approach_sector`]) against the captured row's mask, and
//! whether the captured row is among the filter's candidates
//! ([`timing::pass_candidates`], [`timing::catch_candidates`],
//! [`timing::move_candidates`]). The played captures (`played_capture`) and
//! the attract captures (`ai::capture`) build the cases; a row is named by
//! the script the channel's pc runs, so any table row playing that script
//! counts (the tables carry most passes twice, one row a hand).

use crate::{Hoop, Input, Phase, Player, Ratings, Rules, World, button, timing};
use serde_json::{Value, json};

/// The pad bits of `ai::pad::bits` the cases read.
const KICK: u32 = 0x800;
const TURBOS: [u32; 4] = [0x4000, 0x8000, 0x10000, 0x20000];
const TWO_TURBOS: u32 = 0x40000;

/// An actor on one captured update, in the original's frame.
#[derive(Clone, Debug)]
pub(crate) struct Actor {
    /// The sim's team: 0 for the side attacking +x.
    pub team: usize,
    pub position: [f32; 2],
    pub height: f32,
    /// actor+0xa8, the body's heading word.
    pub heading: i32,
    /// actor+0xb0, the facing word, where the capture carries it.
    pub facing: Option<i32>,
    /// Floor motion over the update (units an update).
    pub motion: [f32; 2],
    /// `motion` is actor+0x40 itself (the played captures), not the
    /// position's change over the update.
    pub velocity: bool,
    /// The live dribble hand (+0x25c), where captured.
    pub hand: Option<u8>,
    pub ratings: Ratings,
    pub computer: bool,
    /// The pad's held word (actor+0x78).
    pub held: u32,
    /// The stick as the original read it: heading units, intensity of 64.
    pub stick: (i32, i32),
}

/// A pass press: the actors on the update before it, the passer, the
/// receiver, the script the passer's channel ran after it.
pub(crate) struct PassCase {
    pub capture: String,
    pub update: i64,
    pub actors: Vec<Actor>,
    pub passer: usize,
    pub receiver: usize,
    pub script: u32,
    pub shot_clock: Option<f32>,
    /// The catch spot the release carried (the ball's target), where
    /// captured: it stands in for the receiver's destination
    /// (`FUN_0016c970`, his front behaviour's) and the lead, which the
    /// replay's world, without the brains, lacks.
    pub catch: Option<[f32; 2]>,
}

/// A pass's release: the actors on the release update (the receiver's
/// words as the release left them), the catch spot and the flight the
/// ball carries, the script the receiver's channel ran.
pub(crate) struct CatchCase {
    pub capture: String,
    pub update: i64,
    pub actors: Vec<Actor>,
    pub passer: usize,
    pub receiver: usize,
    pub spot: [f32; 2],
    pub flight: i32,
    pub script: u32,
}

/// A shoot press that played a dunk or lay-up row: the actors on the
/// update before it, the shooter, the script.
pub(crate) struct MoveCase {
    pub capture: String,
    pub update: i64,
    pub actors: Vec<Actor>,
    pub shooter: usize,
    pub script: u32,
    /// The shooter's held and pressed words on the press update.
    pub buttons: u32,
}

/// What a measurement counts: the cases, those whose captured row's mask
/// holds the sector the sim's filter reads, and those whose captured row
/// is among the sim's candidates.
#[derive(Default, Debug)]
pub(crate) struct Tally {
    pub cases: usize,
    pub sector: usize,
    pub candidate: usize,
    pub rows: Vec<Value>,
}

impl Tally {
    pub fn json(&self) -> Value {
        json!({"cases": self.cases, "sector": self.sector, "candidate": self.candidate,
            "rows": self.rows})
    }
}

/// The ratings of a capture's record list as `street-game`'s setup reads
/// them (`ai::capture`'s `disc_teams`).
pub(crate) fn ratings(list: &[i64]) -> Ratings {
    let r = |k: usize| (list.get(k).copied().unwrap_or(50) as f32 / 99.0).clamp(0.0, 1.0);
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

/// The sim's heading for an original angle word (`word + PI`: the word
/// points (-sin, -cos), the heading (sin, cos)).
fn heading(units: i32) -> f32 {
    (units + 512).rem_euclid(1024) as f32 * (2.0 * std::f32::consts::PI / 1024.0)
}

/// The sim's stick (court axes) for the original's stick angle and
/// intensity: the angle moves along (-sin, -cos).
fn stick(heading: i32, intensity: i32) -> [f32; 2] {
    let units = (heading + 512).rem_euclid(1024) as f32;
    let radians = units * (2.0 * std::f32::consts::PI / 1024.0);
    let len = intensity as f32 / 64.0;
    [libm::sinf(radians) * len, libm::cosf(radians) * len]
}

/// The captured actors as the sim's players, each facing the word `word`
/// picks, moving at his captured pace along it.
fn world(actors: &[Actor], word: impl Fn(&Actor) -> i32) -> World {
    let players = actors
        .iter()
        .map(|a| {
            let mut p = Player::new(a.team, a.position, heading(word(a))).with_ratings(a.ratings);
            p.speed = libm::sqrtf(a.motion[0] * a.motion[0] + a.motion[1] * a.motion[1])
                * crate::TICK_HZ as f32;
            if a.motion != [0.0; 2] {
                p.course = crate::math::atan2(a.motion[0], a.motion[1]);
            }
            // His velocity (actor+0x40, `Player::moving`) as the capture
            // tells it: where it is captured, moving while it is not zero;
            // else the locomotion's moving path sets it under a stick of 16
            // or more, and under that its stand path zeroes it, whatever the
            // clips' roots carry him.
            let moving = if a.velocity {
                a.motion != [0.0; 2]
            } else {
                a.stick.1 >= crate::input::MOVES
            };
            if moving {
                p.gait = crate::player::Gait::Walk;
            }
            p.height = a.height.max(0.0);
            if let Some(hand @ (1 | 2)) = a.hand {
                p.gait_channel.hand = hand;
            }
            p
        })
        .collect();
    let mut w = World::new(Hoop::native(), players, Rules::default(), 1);
    w.phase = Phase::Live;
    w
}

/// The union of the masks of the rows playing `script`.
fn mask<T>(rows: &[T], script: u32, of: impl Fn(&T) -> (u32, u32)) -> u32 {
    rows.iter()
        .map(&of)
        .filter(|&(s, _)| s == script)
        .fold(0, |m, (_, f)| m | f)
}

/// The passes: the passer's heading the body's word (the filter reads the
/// rendered root's yaw, `FUN_00154230`; the facing word on the update
/// before a computer's press already points at the receiver). Rows that
/// read no sector (0xff), the self passes and the trick rows without a
/// release are left out.
pub(crate) fn passes(cases: &[PassCase]) -> Tally {
    let mut t = Tally::default();
    for c in cases {
        let m = mask(&timing::PASS_ROWS, c.script, |r| (r.script, r.flags));
        if m == 0 || m & 0xff == 0xff || m & 0x60_0000 != 0 {
            continue;
        }
        if !timing::PASS_ROWS
            .iter()
            .any(|r| r.script == c.script && r.release > 0)
        {
            continue;
        }
        let mut w = world(&c.actors, |a| a.heading);
        if let Some(clock) = c.shot_clock {
            w.shot_clock = clock;
        }
        // A computer's pad carries its stick as a person's does; only a
        // person's kick is L3 (a computer draws the kick rows by rating).
        let p = &c.actors[c.passer];
        let mut input = Input::moving(stick(p.stick.0, p.stick.1));
        if p.computer {
            input.buttons |= Input::computer().buttons;
        } else if p.held & KICK != 0 {
            input = input.with(button::L3);
        }
        let aim = c.catch.map_or_else(
            || w.pass_aim(c.passer, c.receiver),
            |spot| crate::passing::PassAim {
                spot,
                lead: false,
                reach: 0.0,
            },
        );
        let s = timing::pass_sector(&w, c.passer, c.receiver, &aim, &input);
        let candidates = timing::pass_candidates(&w, c.passer, c.receiver, &aim, &input);
        let sector_ok = m & (1 << s) != 0;
        let candidate_ok = candidates.as_ref().is_some_and(|rows| {
            rows.iter()
                .any(|&k| timing::PASS_ROWS[k].script == c.script)
        });
        t.cases += 1;
        t.sector += usize::from(sector_ok);
        t.candidate += usize::from(candidate_ok);
        // What kept the captured row's own entries out.
        let why: Vec<&str> = timing::pass_verdicts(&w, c.passer, c.receiver, &aim, &input)
            .map(|v| {
                timing::PASS_ROWS
                    .iter()
                    .zip(v)
                    .filter(|(r, _)| r.script == c.script)
                    .filter_map(|(_, why)| why)
                    .collect()
            })
            .unwrap_or_else(|| vec!["handoff"]);
        t.rows.push(json!({
            "capture": c.capture, "update": c.update, "script": c.script,
            "mask": m & 0xff, "sector": s, "candidate": candidate_ok,
            "candidates": candidates.map(|r| r.len()), "why": why,
            "passer": c.actors[c.passer].position, "motion": c.actors[c.passer].motion,
            "heading": c.actors[c.passer].heading, "facing": c.actors[c.passer].facing,
            "stick": c.actors[c.passer].stick, "height": c.actors[c.passer].height,
            "hand": c.actors[c.passer].hand,
            "receiver": c.actors[c.receiver].position, "rmotion": c.actors[c.receiver].motion,
            "spot": aim.spot, "lead": aim.lead,
        }));
    }
    t
}

/// The catches of receivers on the ground: the receiver's heading the
/// facing word the release left (the plain path reads it; the paths that
/// turn him write it), else the body's word.
pub(crate) fn catches(cases: &[CatchCase]) -> Tally {
    let mut t = Tally::default();
    for c in cases {
        let m = mask(&timing::CATCH_ROWS, c.script, |r| (r.script, r.flags));
        if m == 0 || m == u32::MAX {
            continue;
        }
        let r = &c.actors[c.receiver];
        if r.height > 0.0 {
            continue;
        }
        let w = world(&c.actors, |a| a.facing.unwrap_or(a.heading));
        let spot = c.spot;
        let bit = timing::catch_bit(&w, c.passer, c.receiver, spot);
        let candidates = timing::catch_candidates(&w, c.passer, c.receiver, spot, c.flight, false);
        let sector_ok = m & bit != 0;
        let candidate_ok = candidates
            .iter()
            .any(|&k| timing::CATCH_ROWS[k].script == c.script);
        t.cases += 1;
        t.sector += usize::from(sector_ok);
        t.candidate += usize::from(candidate_ok);
        t.rows.push(json!({
            "capture": c.capture, "update": c.update, "script": c.script,
            "mask": m & 0xff, "bit": bit, "candidate": candidate_ok,
            "candidates": candidates.len(),
        }));
    }
    t
}

/// The dunks and lay-ups the filter chose: the two-turbo slot dunks (the
/// slot's row whatever the approach) and the alley-oop finishes are left
/// out; the captured row counts as a candidate under any outcome of the
/// Dunks-rating gates the shooter's rating allows.
pub(crate) fn moves(cases: &[MoveCase]) -> Tally {
    let mut t = Tally::default();
    for c in cases {
        let Some(row) = timing::MOVE_ROWS
            .iter()
            .find(|r| r.script == c.script && r.release > 0 && r.mask & 0x10_0000 == 0)
        else {
            continue;
        };
        let turbos = TURBOS.iter().filter(|&&b| c.buttons & b != 0).count();
        if c.buttons & TWO_TURBOS != 0 || turbos >= 2 {
            continue;
        }
        let m = mask(&timing::MOVE_ROWS, c.script, |r| (r.script, r.mask));
        let w = world(&c.actors, |a| a.heading);
        let s = timing::approach_sector(&w, c.shooter);
        let dunks = (w.players[c.shooter].ratings.dunks * 99.0).round() as i32;
        let gates = [
            timing::MoveGates::default(),
            timing::MoveGates {
                need_easy: true,
                ..Default::default()
            },
            timing::MoveGates {
                no_easy: true,
                ..Default::default()
            },
            timing::MoveGates {
                no_hard: true,
                ..Default::default()
            },
        ];
        let allowed: Vec<_> = gates
            .into_iter()
            .filter(|g| {
                if dunks < 21 {
                    !g.no_easy && !g.no_hard
                } else if dunks < 60 {
                    g.no_hard
                } else {
                    g.no_easy || g.no_hard
                }
            })
            .collect();
        // The held word's turbo bit (0x2000) picks the unlocks.
        let turbo = c.buttons & 0x2000 != 0;
        let candidate_ok = allowed.iter().any(|&g| {
            timing::press_candidates(&w, c.shooter, g, turbo)
                .iter()
                .any(|&k| timing::MOVE_ROWS[k].script == c.script)
        });
        let sector_ok = m & (1 << s) != 0;
        t.cases += 1;
        t.sector += usize::from(sector_ok);
        t.candidate += usize::from(candidate_ok);
        t.rows.push(json!({
            "capture": c.capture, "update": c.update, "script": c.script, "name": row.name,
            "mask": m & 0xff, "sector": s, "candidate": candidate_ok,
        }));
    }
    t
}
