//! The held ball on whatever clip the holder's channel plays, as the
//! original's ball update places it, and the clips of the moves that hold
//! it. Traced raw 2026-10-16 (docs/TRACE.md "The held ball on the
//! holder's clip").
//!
//! `FUN_0014e060` runs every update the ball has a holder (ball+0xf4; a
//! pass's windup, ball state 4 until the row's callback 38, included). It
//! reads the holder's channel 0 (channel 1 or 2 only when its flags have
//! 4, which no held ball of the played captures shows), and places the
//! ball by that clip: on a dribbling clip (a frame with 0x10) in flight
//! between the hand and the floor ([`crate::dribble`]), on any other in
//! the hand its frame holds out (`FUN_0014da20` on the model's bones: both
//! hands' midpoint on 0xc, the right on 8, else the left), whatever the
//! move: the gait, a trick, a shot, a dunk, a lay-up, a pass, a catch. The
//! release callbacks then launch the ball from where it lies (the played
//! captures' first free step is the launch velocity from the held ball of
//! the update before, every pass and shot), so the release points are
//! the clips' hands.
//!
//! Through a blend (the channel's flag 0x200000, from the blend opcode
//! `8e`, `FUN_001978b0`) the clip read is the blend's target at its start
//! frame (+0x54, +0x60), on the model posed in the blend, and the ball
//! eases from where it was when the blend began: the opcode stores the
//! ball's offset from the actor in his own frame (`FUN_00193300`: channel
//! +0xa0, valid with +0xb0), unless the hands of the clip going out
//! (+0x14) and of the target's start meet (both, the same one, or the one
//! the target holds), and the ball update lays it at the stored point
//! times `1 − c/n` plus the hand's times `c/n` (`0x14f2c4..0x14f40c`, `c`,
//! `n` the blend's counter and length). Waits and play-outs clear +0xb0; a
//! blend starting while it is still set stores anew without the test.
//!
//! The live hand (actor+0x25c, [`crate::dribble::live_hand`]) follows the
//! clip the ball is held on; the receiver's catch row's flags 0x400 and
//! 0x200 set it at the release (`FUN_001611d8`, `0x16256c`).
//!
//! The moves' clips: the shot, dunk, lay-up and pass scripts compiled from
//! the executable along the path the simulation takes ([`SHOT_STEPS`],
//! [`FAKE_STEPS`], [`MOVE_STEPS`], [`PASS_STEPS`]; `tests/trick_disc.rs`
//! reads them again) and run from the press on [`crate::trick::Run`]'s
//! clock to the action's script tick, as the display plays them. The
//! blends' alternates are picked at the press ([`ChannelStart`]): `f8`
//! the live left hand, `f2` a left-hander (actor+0xc0), `ec` his side.
//!
//! The jumps' own opcodes are steps the flight reads ([`crate::jump`]), and
//! a `98` record test over its two branches picks one by the channel's
//! record. The rewrite's own, labelled: `a9` (the glass aim) and the
//! planted points' correction (`93`..`95` only set their flags) are not
//! modelled; a blend's foot-synced start (`-9`, one pass row, `0x454f5c`)
//! starts at frame 0. The gait channel's own blends (a change of row over
//! 6 updates) show through [`World::channel`] too.

use crate::pose;
use crate::trick::{self, Run, Shown, Step};
use crate::{Action, BallState, Player, World, timing};

use crate::jump;
use crate::trick::CURRENT;
use crate::trick::Select::{Handed, Side};
use crate::trick::Step::{
    Aim, Bits, Blend, Call, Carry, Clear, Dive, Either, End, Flags, Flight, Hang, Hop, Knock, Land,
    Pick, Plant, PlayOut, Reach, Rise, Shove, Slide, Stop, Test, Toward, Unplant, Wait, Want,
};

include!("held_tables.rs");

/// What a script on a player's channel 0 started from, recorded at its
/// press: the simulation runs the script again from its start each update
/// to know the clip ([`World::channel`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelStart {
    /// The script's address (a record for another script is not read).
    pub script: u32,
    /// The clip the channel showed (record and frame), held through the
    /// first blend.
    pub from: Option<(u16, f32)>,
    /// The live left hand (`f8`) and his side (`ec`) at the press.
    pub left: bool,
    pub side: bool,
}

/// What a player's channel 0 shows this update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channel {
    /// The record and frame (a blend's target at its start frame) and the
    /// blend's weight.
    pub shown: Shown,
    /// The clip held through a blend.
    pub from: Option<(u16, f32)>,
    /// A blend's counter and length (+0x70, +0x6c).
    pub counter: Option<(u16, u16)>,
    /// His gait (the cycle wraps): no script.
    pub gait: bool,
}

/// The ball's offset from its holder in his own frame, stored by a blend's
/// start (channel +0xa0 while +0xb0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub player: usize,
    pub local: [f32; 3],
}

/// `ec`'s test (`FUN_001978b0`, raw `0x197a00..0x197a58`): the alternate
/// when his z lies on the other side of zero from the side his team
/// attacks (`FUN_001404e0`'s sign).
pub fn other_side(p: &Player) -> bool {
    let side = crate::restart::side(p.team);
    let z = p.position[1];
    (side > 0.0 && z < 0.0) || (side < 0.0 && z > 0.0)
}

/// The yaw of facing word `word` (the pose's, `pose::place`).
fn yaw(word: i32) -> [f32; 4] {
    let theta = word as f32 * std::f32::consts::TAU / 1024.0;
    [
        0.0,
        crate::math::sin(theta * 0.5),
        0.0,
        crate::math::cos(theta * 0.5),
    ]
}

/// The script, its steps and the updates it has run for a scripted move;
/// a fadeaway's the check it jumped at from its shot.
struct Scripted {
    address: u32,
    steps: &'static [Step],
    updates: i32,
    check: Option<i32>,
    /// A dunk's hang on the rim, once callback 28 has answered.
    hang: Option<bool>,
}

/// The script tick a pump fake is over at (`FAKE_STEPS`): shot `script`'s
/// path through callback 20's answer (`85 14` falls through, the heave's
/// `84 14` jumps) run to its `End`, where the channel goes on to the
/// stand. The played captures' fakes: the stepback's at 18, `legkick_1`'s
/// at 22, the heave's at 8 (the tap's update the script's 12, 14 and 8).
pub fn fake_end(script: usize) -> Option<i32> {
    static ENDS: std::sync::OnceLock<Vec<Option<i32>>> = std::sync::OnceLock::new();
    let ends = ENDS.get_or_init(|| {
        FAKE_STEPS
            .iter()
            .map(|steps| {
                if steps.is_empty() {
                    return None;
                }
                let mut run = Run::new(false, 0);
                for _ in 0..600 {
                    run.step(steps, 0.0);
                    if run.done {
                        return Some(i32::from(run.tick));
                    }
                }
                None
            })
            .collect()
    });
    ends.get(script).copied().flatten()
}

/// The steps of a dunk's, lay-up's or leap's row ([`timing::move_row`]).
pub fn move_row_steps(row: u8) -> Option<&'static [Step]> {
    let r = usize::from(row);
    MOVE_STEPS
        .get(r)
        .or_else(|| {
            let r = r.checked_sub(MOVE_STEPS.len())?;
            LEAP_STEPS.get(r).or_else(|| {
                let r = r.checked_sub(LEAP_STEPS.len())?;
                EXTRA_STEPS.get(r).or_else(|| {
                    let r = r.checked_sub(EXTRA_STEPS.len())?;
                    PUTBACK_STEPS
                        .get(r)
                        .or_else(|| FINISH_STEPS.get(r.checked_sub(PUTBACK_STEPS.len())?))
                })
            })
        })
        .copied()
}

fn scripted(action: &Action) -> Option<Scripted> {
    let plain = |address: u32, steps: &'static [Step], tick: i32| Scripted {
        address,
        steps,
        updates: tick,
        check: None,
        hang: None,
    };
    match *action {
        Action::Shooting {
            script,
            tick,
            stall,
            fade,
            check,
            ..
        } => {
            let s = usize::from(script);
            let tick = i32::from(tick) - i32::from(stall);
            let address = timing::SHOT_SCRIPTS.get(s)?.script;
            if fade && s == timing::FADEAWAY {
                // The check's `84 17` jumps into `jumpshot_1_fadeaway`: the
                // channel goes on from the shot's clip at the check.
                Some(Scripted {
                    address,
                    steps: SHOT_STEPS[s],
                    updates: tick - i32::from(check),
                    check: Some(i32::from(check)),
                    hang: None,
                })
            } else {
                Some(plain(address, SHOT_STEPS[s], tick))
            }
        }
        Action::PumpFake { script, tick, .. } => {
            let s = usize::from(script);
            let address = timing::SHOT_SCRIPTS.get(s)?.script;
            let steps = FAKE_STEPS[s];
            (!steps.is_empty()).then(|| plain(address, steps, i32::from(tick)))
        }
        Action::Dunking {
            row, tick, hang, ..
        } => {
            let address = timing::move_row(row)?.script;
            Some(Scripted {
                hang,
                ..plain(address, move_row_steps(row)?, i32::from(tick))
            })
        }
        Action::Layup { row, tick, .. } => {
            let address = timing::move_row(row)?.script;
            Some(plain(address, move_row_steps(row)?, i32::from(tick)))
        }
        Action::Dive { script, tick } => {
            let s = usize::from(script);
            let address = *timing::DIVE_SCRIPTS.get(s)?;
            Some(plain(address, DIVE_STEPS[s], i32::from(tick)))
        }
        Action::Passing { row, tick, .. } => {
            let r = usize::from(row);
            let address = timing::PASS_ROWS.get(r)?.script;
            Some(plain(address, PASS_STEPS[r], i32::from(tick)))
        }
        _ => None,
    }
}

/// `steps` run `updates` updates from their press (the press update the
/// first), the blends picked by `start`, a landing wait lasting until tick
/// `land_at` (-1: one update), callback 28's answer `hang`, and what the
/// last update ran.
#[allow(clippy::too_many_arguments)]
fn replay(
    steps: &'static [Step],
    updates: i32,
    left: bool,
    handed: bool,
    side: bool,
    land_at: i16,
    hang: Option<bool>,
    flight: impl Fn(i32) -> u8,
) -> (Run, trick::Ran) {
    let mut run = Run::new(left, 0).picking(handed, side);
    run.land_at = land_at;
    run.hang = hang;
    let mut ran = trick::Ran::default();
    for k in 0..updates.clamp(0, 600) {
        run.flight = flight(k);
        ran = run.step(steps, 0.0);
        if run.done {
            break;
        }
    }
    (run, ran)
}

/// A scripted move's channel as the flight reads it ([`World::move_steps`]).
pub(crate) struct MoveSteps {
    /// The script's address and the updates its channel has run.
    pub script: u32,
    pub updates: i32,
    /// What it showed the update before, and now.
    pub before: Option<Shown>,
    pub after: Shown,
    /// What this update ran (its steps saw `ran.mid`).
    pub ran: trick::Ran,
    /// The landing tick the replay used.
    pub land_at: i16,
    /// The clip held through its first blend.
    pub from: Option<(u16, f32)>,
    /// The script has run out (its `End` this update or before).
    pub done: bool,
}

impl World {
    /// A script about to start on `i`'s channel 0 at `address`: the clip
    /// it starts from, his live hand and his side, for [`World::channel`].
    pub(crate) fn begin_script(&mut self, i: usize, address: u32) {
        self.release_box_out(i);
        let from = self.channel(i).and_then(|c| match c.shown.blend {
            Some(_) => c.from,
            None => c.shown.record.map(|r| (r, c.shown.frame)),
        });
        let p = &mut self.players[i];
        p.channel_start = Some(ChannelStart {
            script: address,
            from,
            left: p.gait_channel.hand == 2,
            side: other_side(p),
        });
    }

    /// A dunk's or lay-up's script about to start (`row` of
    /// [`timing::MOVE_ROWS`]; none for the putback's own clock).
    pub(crate) fn begin_move(&mut self, i: usize, row: u8) {
        if let Some(r) = timing::move_row(row) {
            self.begin_script(i, r.script);
        }
    }

    /// `i`'s scripted move's channel as the last update left it: its run
    /// replayed to now (the leaper's span the lob reads, [`crate::leap`]).
    pub(crate) fn move_run(&self, i: usize) -> Option<Run> {
        let p = &self.players[i];
        let s = scripted(&p.action)?;
        let first = p.channel_start.filter(|x| x.script == s.address);
        let (left, side) = first.map_or((p.gait_channel.hand == 2, other_side(p)), |x| {
            (x.left, x.side)
        });
        let (run, _) = replay(
            s.steps,
            s.updates,
            left,
            p.left_handed,
            side,
            p.flight.landing,
            s.hang,
            |_| 0,
        );
        Some(run)
    }

    /// What `i`'s scripted move's channel will show `ahead` updates on, its
    /// run replayed that far (`FUN_00330f98` reads the channel's frame plus
    /// its step times the updates).
    pub(crate) fn move_shown_ahead(&self, i: usize, ahead: i32) -> Option<crate::trick::Shown> {
        let p = &self.players[i];
        let s = scripted(&p.action)?;
        let first = p.channel_start.filter(|x| x.script == s.address);
        let (left, side) = first.map_or((p.gait_channel.hand == 2, other_side(p)), |x| {
            (x.left, x.side)
        });
        let (run, _) = replay(
            s.steps,
            s.updates + ahead,
            left,
            p.left_handed,
            side,
            p.flight.landing,
            s.hang,
            |_| 0,
        );
        Some(run.shown())
    }

    /// What `i`'s channel 0 shows: a trick's or a reaction's script, a
    /// catch row's (in the flight or after the catch), a shot's, a dunk's,
    /// a lay-up's or a pass's ([`SHOT_STEPS`]..), else his gait. None
    /// without the disc's animations.
    pub fn channel(&self, i: usize) -> Option<Channel> {
        self.poses.get()?;
        let p = &self.players[i];
        let of = |run: &Run, from: Option<(u16, f32)>| Channel {
            shown: run.shown(),
            from,
            counter: run.blend_counter(),
            gait: false,
        };
        match &p.action {
            Action::Trick { play, .. } => return Some(of(&play.run, play.from)),
            Action::Scoop { play } => return Some(of(&play.run, play.from)),
            Action::Stumbling {
                reaction: Some(r), ..
            } => return Some(of(&r.run, r.from)),
            // The rebound jump's and the block jump's scripts on their own
            // clocks ([`World::rebound_channel`], [`crate::block::Jump`]).
            Action::ReboundJump { .. } => return self.rebound_channel(i),
            Action::Jumping { block: Some(j), .. } => {
                return Some(Channel {
                    shown: Shown {
                        record: Some(j.record()),
                        frame: j.frame(),
                        blend: j.blend(),
                    },
                    from: j.from,
                    counter: j.blend().map(|_| (j.tick + 1, j.blend_len())),
                    gait: false,
                });
            }
            _ => {}
        }
        if let Some(c) = self.catch_row_playing(i) {
            return Some(of(&c.run, c.from));
        }
        let start = p.channel_start;
        let started = |address: u32| start.filter(|s| s.script == address);
        if let Some(s) = scripted(&p.action) {
            // A fadeaway's press was its shot's.
            let shot = s.check.and_then(|_| {
                let a = start?.script;
                timing::SHOT_SCRIPTS.iter().position(|x| x.script == a)
            });
            let first = match shot {
                Some(_) => start,
                None => started(s.address),
            };
            let (left, side) = first.map_or((p.gait_channel.hand == 2, other_side(p)), |s| {
                (s.left, s.side)
            });
            let gait = || {
                let f = self.gait_frame(i);
                Some((f.record, f.time))
            };
            let from = match (shot, s.check) {
                (Some(k), Some(check)) => {
                    let (run, _) = replay(
                        SHOT_STEPS[k],
                        check,
                        left,
                        p.left_handed,
                        side,
                        -1,
                        None,
                        |_| 0,
                    );
                    let shown = run.shown();
                    match shown.blend {
                        Some(_) => first.and_then(|s| s.from),
                        None => shown.record.map(|r| (r, shown.frame)),
                    }
                }
                _ => first.and_then(|s| s.from),
            }
            .or_else(gait);
            let landing = p.flight.landing;
            let (run, _) = replay(
                s.steps,
                s.updates,
                left,
                p.left_handed,
                side,
                landing,
                s.hang,
                |_| 0,
            );
            if run.done {
                return Some(self.gait_channel_shown(i));
            }
            return Some(of(&run, from));
        }
        // The catch row holding him after the catch: its script from the
        // release (the display's clock, `street_game::channel`).
        if let Some(c) = self.catch.filter(|c| c.player == i && self.tick < c.until)
            && let Some(row) = timing::CATCH_ROWS.get(usize::from(c.row))
        {
            let steps = crate::touch::CATCH_SCRIPTS[usize::from(c.row)];
            let first = started(row.script);
            let (left, side) = first.map_or((false, false), |s| (s.left, s.side));
            let updates = (self.tick as i64 - c.since as i64 + 1).clamp(0, 600) as i32;
            let flight = i32::from(c.flight);
            let (run, _) = replay(steps, updates, left, false, side, -1, None, |k| {
                (flight - k).clamp(1, 255) as u8
            });
            if !run.done {
                let from = first.and_then(|s| s.from).or_else(|| {
                    let f = self.gait_frame(i);
                    Some((f.record, f.time))
                });
                return Some(of(&run, from));
            }
        }
        Some(self.gait_channel_shown(i))
    }

    /// `i`'s shot, dunk or lay-up script for his flight ([`crate::jump`]),
    /// with or without the disc's animations: run to this update and the
    /// one before, what this update ran, the landing wait to `landing` or,
    /// when it has not started, to the landing his rise and height now give
    /// (`FUN_0019bed8` at the wait's start).
    pub(crate) fn move_steps(&self, i: usize, landing: i16) -> Option<MoveSteps> {
        let p = &self.players[i];
        let s = scripted(&p.action)?;
        let start = p.channel_start;
        let first = match s.check {
            Some(_) => start,
            None => start.filter(|x| x.script == s.address),
        };
        let (left, side) = first.map_or((p.gait_channel.hand == 2, other_side(p)), |x| {
            (x.left, x.side)
        });
        let tick = (s.updates - 1).clamp(-1, i32::from(i16::MAX)) as i16;
        let land_at = if landing >= 0 {
            landing
        } else {
            tick.saturating_add(i16::from(timing::airtime(p.flight.velocity[1], p.height)))
        };
        let (after, ran) = replay(
            s.steps,
            s.updates,
            left,
            p.left_handed,
            side,
            land_at,
            s.hang,
            |_| 0,
        );
        let before = (s.updates > 1).then(|| {
            replay(
                s.steps,
                s.updates - 1,
                left,
                p.left_handed,
                side,
                land_at,
                s.hang,
                |_| 0,
            )
            .0
            .shown()
        });
        // The clip held through the first blend (a fadeaway's: its shot's at
        // the check), as [`World::channel`] reads it.
        let shot = s.check.and_then(|_| {
            let a = start?.script;
            timing::SHOT_SCRIPTS.iter().position(|x| x.script == a)
        });
        let from = match (shot, s.check) {
            (Some(k), Some(check)) => {
                let (run, _) = replay(
                    SHOT_STEPS[k],
                    check,
                    left,
                    p.left_handed,
                    side,
                    -1,
                    None,
                    |_| 0,
                );
                let shown = run.shown();
                match shown.blend {
                    Some(_) => first.and_then(|x| x.from),
                    None => shown.record.map(|r| (r, shown.frame)),
                }
            }
            _ => first.and_then(|x| x.from),
        }
        .or_else(|| {
            self.poses.get()?;
            let f = self.gait_frame(i);
            Some((f.record, f.time))
        });
        Some(MoveSteps {
            script: s.address,
            updates: s.updates,
            before,
            after: after.shown(),
            ran,
            land_at,
            from,
            done: after.done,
        })
    }

    fn gait_channel_shown(&self, i: usize) -> Channel {
        let f = self.gait_frame(i);
        // A change of row blends from the old row's clip held at its frame
        // (flag 0x200000, the counter +0x70 over +0x6c, 1 on its first
        // update), as [`World::actor_pose`] poses it.
        let g = &self.players[i].gait_channel;
        let n = crate::pose::GAIT_BLEND;
        let blending = (1..=n).contains(&g.blend_counter) && g.blend_from.is_some();
        Channel {
            shown: Shown {
                record: Some(f.record),
                frame: f.time,
                blend: blending.then(|| (n + 1 - g.blend_counter) as f32 / n as f32),
            },
            from: if blending { g.blend_from } else { None },
            counter: blending.then_some((g.blend_counter as u16, n as u16)),
            gait: true,
        }
    }

    /// `i` posed in a scripted move's clip (a shot, a dunk, a lay-up, a
    /// pass, a catch row after the catch), while one plays.
    pub(crate) fn move_pose(&self, i: usize) -> Option<pose::Pose> {
        let poses = self.poses.get()?;
        let c = self.channel(i)?;
        if c.gait {
            return None;
        }
        trick::pose(
            poses,
            c.shown,
            c.from,
            self.players[i].body(),
            self.place(i),
        )
    }

    /// The ball's offset from `h` in his own frame, as the update begins
    /// (what a blend starting in his update stores, `FUN_00193300`).
    pub(crate) fn held_offset(&self) -> Option<(usize, [f32; 3])> {
        let BallState::Held(h) = self.ball.state else {
            return None;
        };
        let place = self.place(h);
        let b = self.ball.position;
        let d = std::array::from_fn(|k| b[k] - place.position[k]);
        let q = yaw(place.word);
        Some((h, pose::rotate([-q[0], -q[1], -q[2], q[3]], d)))
    }

    /// `FUN_0014e060` for holder `h` with the disc's animations: the ball
    /// on the clip his channel shows (through a blend eased from the
    /// stored offset), his live hand after it, the ball's velocity his own.
    /// `before` the ball's offset as this update began
    /// ([`World::held_offset`]). False without the disc's animations.
    pub(crate) fn place_held(&mut self, h: usize, before: Option<(usize, [f32; 3])>) -> bool {
        let Some(poses) = self.poses.arc() else {
            return false;
        };
        let Some(c) = self.channel(h) else {
            return false;
        };
        let p = &self.players[h];
        let body = p.body();
        let place = self.place(h);
        let radius = self.ball.radius;
        let Some(record) = c.shown.record else {
            return false;
        };
        let mut dribbled = None;
        let (point, hand) = match c.shown.blend {
            None => {
                let at =
                    crate::dribble::ball_place(&poses, record, c.shown.frame, body, place, radius)
                        .map(|(at, d)| {
                            dribbled = d;
                            at
                        });
                let hand =
                    crate::dribble::live_hand(&poses, record, c.shown.frame, p.gait_channel.hand);
                (at, hand)
            }
            Some(_) => {
                // The target at its start frame, on the model posed in the
                // blend (`0x14e23c..0x14e318`, `0x14f13c..0x14f17c`).
                let start = c.shown.frame;
                let hand = crate::dribble::live_hand(&poses, record, start, p.gait_channel.hand);
                let state = poses.state(record, start as i32);
                let at = if crate::dribble::dribbles(&poses, record) && state & 0xc == 0 {
                    crate::dribble::ball_point(&poses, record, start, body, place, radius)
                } else {
                    trick::pose(&poses, c.shown, c.from, body, place).map(|pose| {
                        crate::dribble::in_hand(&pose, record, start as i32, state, body, radius)
                    })
                };
                (at, hand)
            }
        };
        let Some(point) = point else {
            return false;
        };
        // The blend's stored offset (`FUN_00193300`) and the ease.
        let anchor = self.held_anchor.filter(|a| a.player == h);
        self.held_anchor = None;
        let mut at = point;
        if let (Some(_), Some((counter, length))) = (c.shown.blend, c.counter) {
            let anchor = if counter <= 1 {
                // The blend began this update: it stores the offset unless
                // the hands going out and coming in meet (a blend begun
                // while one is stored stores again).
                let out = c.from.map_or(0, |(r, f)| poses.state(r, f as i32) & 0xc);
                let target = poses.state(record, c.shown.frame as i32) & 0xc;
                let meet = out & target != 0 && (out == target || target == 4 || target == 8);
                match before {
                    Some((b, local)) if b == h && (anchor.is_some() || !meet) => {
                        Some(Anchor { player: h, local })
                    }
                    _ => None,
                }
            } else {
                anchor
            };
            if let Some(a) = anchor {
                let q = yaw(place.word);
                let o = pose::rotate(q, a.local);
                let f = f32::from(counter) / f32::from(length);
                at = std::array::from_fn(|k| (place.position[k] + o[k]) * (1.0 - f) + point[k] * f);
                self.held_anchor = Some(a);
            }
        }
        if dribbled == Some(crate::dribble::Dribbled::Floor) {
            (self.ball.spin_axis, self.ball.spin) =
                crate::dribble::roll(self.ball.position, point, radius);
        } else if let Some(d) = dribbled {
            self.dribble_bounce(h, d, at[1]);
        }
        let p = &mut self.players[h];
        p.gait_channel.hand = hand;
        let [vx, vz] = p.velocity();
        self.ball.position = at;
        self.ball.velocity = [vx, p.rise / crate::TICK_SECONDS, vz];
        true
    }

    /// The dribble's bounce (`FUN_0014e060`, raw `0x14f410..0x14f5b8`):
    /// one flag for every holder (`DAT_003d5600`, [`World::dribble_armed`])
    /// is armed while the ball falls (in flight before the floor's share
    /// of its way, or lower than the update before, ball+0x84), and once
    /// armed the first other update on a dribbling clip with the ball
    /// under 2 (now or the update before) posts the floor event
    /// (`0x3c6670`, [`Event::Dribble`], with twice the height the ball fell
    /// this update) and disarms it. A clip that carries
    /// the ball on the floor or hand to hand leaves the flag be.
    pub(crate) fn dribble_bounce(&mut self, h: usize, d: crate::dribble::Dribbled, y: f32) {
        let before = self.ball.position[1];
        let falling = match d {
            crate::dribble::Dribbled::Flight { t, fall } => t <= fall || y < before,
            crate::dribble::Dribbled::Hand => false,
            crate::dribble::Dribbled::Floor => return,
        };
        if falling {
            self.dribble_armed = true;
        } else if self.dribble_armed && (y < 2.0 || before < 2.0) {
            self.dribble_armed = false;
            self.events.push(crate::Event::Dribble {
                holder: h,
                speed: (y - before).abs() * 2.0,
            });
        }
    }
}
