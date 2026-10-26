//! Where the ball meets the players' bones: each actor's tests at the end of
//! his update, which sweep the ball's last move against his limbs
//! (`FUN_00137db8`, [`pose::sweep`]) and decide whether he catches it, it
//! comes off his body, or he picks it up. Traced raw 2026-10-16
//! (docs/TRACE.md "The ball on the bones").
//!
//! The actor's update `FUN_00156f48` runs the side with the ball's update
//! `FUN_0015a060` (actor+0x194 1; Ghidra never decompiled it: its tail is
//! raw `0x15ba14..0x15c568`) or the other side's `FUN_0015da28` (2; tail
//! `0x15e3bc..0x15e8d4`), after his channel steps and before it counts the
//! bars down (`0x157300`) and moves him (`0x1573e0`): the tests read his
//! bones as the channel shows them this update at his place before he
//! moves, and the ball's last move (ball+0xa0).
//!
//! Both tails return at once for a barred actor (+0x1a8), a held ball, or
//! the pickup clip (record 73); a rebound jumper has his own catch
//! (`FUN_0016ed18`, [`crate::rebound`]), a block jumper his block test
//! (`FUN_0016d888`, [`crate::block`]), a diver his (`FUN_0015c598`). The
//! tests read the model as the last update posed it (the pose step runs
//! after the tail): the catch row's channel steps after them
//! ([`World::catching_tick`]).
//!
//! The offence's tail first (`0x15bad0..0x15bf28`): an actor in a dunk, his
//! catch row or a leap (state bits 7, 9, 15: `0x8280`) takes any free ball
//! within [`NEAR`] of his bones ([`World::near_ball`]): a dunker, or a
//! leaper for his own pass, by the hand his clip holds out; anyone else by
//! his pelvis or either hand; with bit 9 the other side is barred
//! [`CATCH_BAR`] updates. With those bits and the pass still in flight
//! (ball+0x60 below +0x64) the tail ends there (`0x15bf3c..0x15bf6c`): a
//! receiver on his catch row is never swept while the pass flies. Then,
//! by the ball's state (ball+0x180):
//!
//! - loose (1; the offence also the toss, 2): an actor on the floor within
//!   reach ([`World::pickup_reach`], `FUN_0016cd28`) takes it
//!   (`FUN_0014cb08`), and his side has the ball (`FUN_00169138`);
//! - on the rim (6): nothing;
//! - a pass (4): the defence, with the pass more than three quarters flown
//!   and `r % 100 < trunc(Steals / 99 × 15)`, sweeps it, and a limb met
//!   knocks it ([`World::body_knock`]); the offence sweeps it unless he
//!   threw it (and it is not to himself);
//! - a shot (5) or a rebound (0), or anything while his cooldown runs
//!   (+0x1cc): the offence first takes it within reach unless he is in a
//!   rebound or block jump, his catch row (bit 9) or a leap; then both
//!   sweep it: an actor on the floor catches a rebound with any limb, the
//!   pass's receiver catches it with the forearm and hand whose hand the
//!   clip's frame holds out (the state's 8 the right, bones 18..19; 4 the
//!   left, 14..15), and every other limb met knocks it off.
//!
//! The receiver's catch row is the script the release parks
//! (`FUN_001611d8`, [`Catching`]) and his update starts once the flight
//! left is at most his +0x1b0 (`0x15a974`; then +0x1b0 is -1): it zeroes
//! his velocity on the standing rows, sets bit 9, blends six updates to
//! the catch clip and plays it to its catch frame over the rest of the
//! pass's flight (`8d ee F fd`); a receiver moving when it starts plays it
//! on channel 2 over his run ([`Catching::lower`]). The release aims the
//! pass at the hand of that clip posed at its catch frame where the
//! receiver will stand ([`World::catch_target`]), so the ball comes within
//! [`NEAR`] of his bones 0..5 updates before it arrives.

use crate::ball::{BallState, Pass};
use crate::game_phase::ball as state;
use crate::pose::{self, Frame, Place, Pose};
use crate::timing::{self, CATCH_ROWS};
use crate::trick::{self, Run};
use crate::{Action, Event, OpeningToss, Phase, World};

include!("touch_tables.rs");

/// The scoop's script (`0x45748a`, record 73 `standpickupball`; the
/// executable's along its plain path, `tests/trick_disc.rs`): the velocity
/// zeroed, a 3-update blend to the clip, frame 7 over 10 updates, callback
/// 0 (`FUN_001775d8`: the take if nobody holds the ball), frame 13 over 6,
/// callbacks 7 and 1.
pub const SCOOP_STEPS: &[trick::Step] = &[
    trick::Step::Stop,
    trick::Step::Bits(0),
    trick::Step::Blend {
        n: 3,
        start: 0,
        records: [73, 73],
    },
    trick::Step::Wait { to: 7, n: 10 },
    trick::Step::Call(0),
    trick::Step::Wait { to: 13, n: 6 },
    trick::Step::Call(7),
    trick::Step::Call(1),
    trick::Step::End,
];

/// The scoop's bound on the ball's height (`0x15c404`, 2.5).
pub const SCOOP_HEIGHT: f32 = 2.5;

/// The scoop's bound on the ball's rise or fall, units an update (6 mph a
/// frame: `5280 / (fps * 3600) * 6`, `0x15c46c..0x15c490`).
pub const SCOOP_RISE: f32 = 5280.0 / (60.0 * 3600.0) * 6.0;

/// The ball's share of the gap to the scooper it moves an update
/// (`0x15c4a4..0x15c4ec`, one fifteenth).
pub const SCOOP_PULL: f32 = 15.0;

/// A low, slow ball being scooped: the script's clock, the clip held
/// through its blend and the facing (+0xb0) toward the ball.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scooping {
    pub run: Run,
    pub from: Option<(u16, f32)>,
    pub facing: i32,
}

/// Channel 2's first bone (`FUN_001563a8`: +0x74 9, +0x7c 13 bones): the
/// spine and everything above it, where a running catch's row plays
/// ([`pose::Poses::overlay`]).
pub const UPPER_BODY: usize = 9;

/// `DAT_004b99b8` (`fps / 6`, set at boot): the updates an actor whose body
/// knocked the ball keeps sweeping it whatever its state (actor+0x1cc).
pub const COOLDOWN: u16 = 10;

/// The offence's first take (`0x15bad0..0x15bd68`): the ball within this
/// of the bones it reads (the pelvis and the hands) for an actor in a dunk,
/// his catch row or a leap (state bits 7, 9, 15: `0x8280`).
pub const NEAR: f32 = 1.0;

/// The updates the other side is barred (actor+0x1a8) when that take comes
/// with bit 9 set, the catch row's (`0x15beec..0x15bf28`, `0x14`).
pub const CATCH_BAR: u16 = 20;

/// `DAT_004b84b0`: a pass's timer (ball+0x158) is its flight and this many
/// updates more (`FUN_0014c0c0`); when it runs out the ball is loose
/// (`FUN_0014d1f8`, event 0x4f).
pub const PASS_TAIL: i32 = 5;

/// `FUN_0016cd28`: the reach across (x, z) within which an actor takes a
/// ball at his body's height.
pub const PICKUP_REACH: f32 = 3.0;

/// `FUN_0016cd28`: how far above his highest bone (the head, either hand)
/// or below his lowest (either foot) the ball may be.
pub const PICKUP_SLACK: f32 = 1.0;

/// `0x43f6e0` (`FUN_00171e08`): a word per lay-up kind (actor+0x26c, the
/// lay-up row's id); bit 0 turns the lay-up's backspin round
/// (`FUN_0014aaf8`, `0x14ac20..0x14ac48`, `0x14b6fc..0x14b724`).
pub const LAYUP_FLAGS: [u8; 28] = [
    2, 0, 0, 1, 4, 0, 0, 0, 0, 2, 2, 0, 1, 1, 2, 0, 2, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0,
];

/// The receiver's catch row playing, from the release (`FUN_001611d8`
/// starts its script, [`CATCH_SCRIPTS`]) until he has the ball, the pass
/// is gone or the script ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Catching {
    pub player: usize,
    /// The row of [`CATCH_ROWS`].
    pub row: u8,
    pub run: Run,
    /// The clip he played at the release (record and frame), held through
    /// the blend.
    pub from: Option<(u16, f32)>,
    /// Bit 9 of his state word (actor+0xa4), which the script sets and
    /// clears: while it is set he takes the ball only with his hands.
    pub catching: bool,
    /// The velocity he coasts on, units an update: his own at the
    /// script's first update on a running row, none on the others (`97`).
    pub drift: [f32; 2],
    /// Callback 5's facing (+0xb0, the angle to the ball when it ran), the
    /// facing word he turns toward at 46 units an update
    /// ([`World::catching_turn`]).
    pub facing: Option<i32>,
    /// The row's script parked (actor+0x1bc, `FUN_001611d8`): it starts in
    /// his update once the pass's flight left is at most his +0x1b0
    /// ([`crate::Player::received_flight`]) and he is on the floor
    /// (`0x15a974..0x15ab4c`); until then nothing of the row plays.
    pub parked: bool,
    /// Actor+0x1ac: on the floor at the release on a row without flag
    /// 0x800 (`FUN_001611d8`, `0x161360..0x161388`, `0x16248c..0x162498`),
    /// which lets a receiver moving when the script starts play it over
    /// his run ([`Catching::lower`]).
    pub floor: bool,
    /// Channel 0 under a running catch (`0x15aa44..0x15aadc`), whose row
    /// plays on channel 2 (bones 9..21, [`UPPER_BODY`]): his gait's cycle
    /// as the script found it (record and frame), carried on at its rate
    /// (frames an update; `FUN_00163d40(actor, 0, 1)`, `DAT_003d70b0` 1 on
    /// the disc). None for a row on channel 0.
    pub lower: Option<(u16, f32, f32)>,
}

impl Catching {
    pub fn steps(&self) -> &'static [trick::Step] {
        CATCH_SCRIPTS[usize::from(self.row).min(CATCH_SCRIPTS.len() - 1)]
    }

    /// The script opens with `97` (the velocity zeroed) and its clip's
    /// root moves him; the running rows instead lock his run's channel
    /// (callback 30, flag 2) and leave him coasting on his velocity. On
    /// both the stick no longer steers him: the played captures' 67 of 74
    /// moving receivers keep the velocity they had at the release until
    /// the catch (within 0.04 of the straight line, median).
    pub fn holds(&self) -> bool {
        self.steps().first() == Some(&trick::Step::Stop)
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

impl World {
    /// The point the release aims a pass at (`FUN_001611d8`, raw
    /// `0x1624c0..0x162a5c`): the receiver posed in `row`'s clip at its
    /// catch frame ([`timing::CatchRow::record`], `frame`) where he will
    /// stand (`at`), facing the ball on the rows with flag 0x10000
    /// (`FUN_0016f2b0`, from his place at the release) and otherwise his
    /// wanted facing (+0xb0, as `FUN_001636c0` and the spot path leave it);
    /// his pelvis moved by the clip's root at that frame, turned by the
    /// facing at his height scale; the point his right hand (bone 19) on
    /// the row's flag 0x400, his left (15) without it, their midpoint on
    /// 0x100. A running receiver ([`World::running_catch`],
    /// `0x1625bc..0x162740`) is posed as his catch will show him: the row's
    /// clip at its catch frame from the spine up over channel 0's clip
    /// carried on by the pass's `flight` ([`World::lower_at`]; bones 0..8
    /// copied over the row's pose, `0x162704..0x16271c`), the pelvis at that
    /// clip's root height and none of its travel (his run carries him).
    /// None without the disc's animations. A spot run (`run`,
    /// [`crate::receiver::SpotRun`]) poses him facing the ball and not
    /// running (`0x162300..0x162310`: +0x1ac 0).
    pub(crate) fn catch_target(
        &self,
        to: usize,
        row: usize,
        at: [f32; 2],
        flight: i32,
        run: bool,
    ) -> Option<[f32; 3]> {
        let poses = self.poses.get()?;
        let r = CATCH_ROWS.get(row)?;
        let p = &self.players[to];
        let word = if r.flags & 0x10000 != 0 || run {
            let b = self.ball.position;
            let a = crate::math::atan2(p.position[0] - b[0], p.position[1] - b[2]);
            (a * 162.974_66 + 0.5) as i32 & 0x3ff
        } else {
            timing::facing_word(p.wanted)
        };
        let frame = Frame {
            record: r.record,
            time: f32::from(r.frame),
            looping: false,
        };
        let root = poses.root(frame)?;
        let body = p.body();
        let running = !run && self.running_catch(to, p.height <= 0.0 && r.flags & 0x800 == 0);
        let (rx, rz) = if running {
            (0.0, 0.0)
        } else {
            (root[0] * body.height, root[2] * body.height)
        };
        let theta = word as f32 * std::f32::consts::TAU / 1024.0;
        let yaw = [
            0.0,
            crate::math::sin(theta * 0.5),
            0.0,
            crate::math::cos(theta * 0.5),
        ];
        let off = pose::rotate(yaw, [rx, 0.0, rz]);
        let place = Place {
            position: [at[0] + off[0], 0.0, at[1] + off[2]],
            word,
        };
        let pose = match self.lower_at(to, flight).filter(|_| running) {
            Some(lower) => poses.overlay(lower, frame, None, UPPER_BODY, body, place)?,
            None => poses.pose(frame, body, place)?,
        };
        let right = pose.joints[pose::RIGHT_HAND].position;
        let left = pose.joints[pose::LEFT_HAND].position;
        Some(if r.flags & 0x100 != 0 {
            std::array::from_fn(|k| (right[k] + left[k]) * 0.5)
        } else if r.flags & 0x400 != 0 {
            right
        } else {
            left
        })
    }

    /// The release parks receiver `to`'s catch row (`FUN_001611d8`,
    /// `0x162438..0x162450`: actor+0x1bc the row's script); his update
    /// starts it ([`World::catching_start`]). The row's flags 0x400 and 0x200
    /// set his live hand, the right and the left (actor+0x25c,
    /// `0x16256c..0x162590`).
    pub(crate) fn start_catching(&mut self, to: usize, row: usize) {
        self.catch_hand(to, row);
        if self.poses.get().is_none() || row >= CATCH_SCRIPTS.len() {
            self.catching = None;
            return;
        }
        self.catching = Some(Catching {
            player: to,
            row: row as u8,
            run: Run::new(false, 0),
            from: None,
            catching: false,
            drift: [0.0; 2],
            facing: None,
            parked: true,
            floor: self.players[to].height <= 0.0 && CATCH_ROWS[row].flags & 0x800 == 0,
            lower: None,
        });
    }

    /// The catch row's flags 0x400 and 0x200 set receiver `to`'s live
    /// hand, the right and the left (actor+0x25c, `0x16256c..0x162590`).
    pub(crate) fn catch_hand(&mut self, to: usize, row: usize) {
        if let Some(r) = CATCH_ROWS.get(row)
            && r.flags & 0x600 != 0
        {
            self.players[to].gait_channel.hand = if r.flags & 0x400 != 0 { 1 } else { 2 };
        }
    }

    /// Whether receiver `to` plays his catch row over his run
    /// (`0x15a9d0..0x15aa60`; the release's aim reads the same at
    /// `0x162554..0x1625f4`): moving (actor+0x40 not zero), on the floor at
    /// the release on a row without 0x800 (+0x1ac 1). Any row: a standing
    /// row's `97` then stops him under it.
    fn running_catch(&self, to: usize, floor: bool) -> bool {
        floor && self.players[to].moving()
    }

    /// Channel 0's clip under a running catch as the release poses it
    /// (`0x1625f8..0x1626e0`): his gait's record at its whole frame plus
    /// its rate times `flight` (`DAT_003d65a0`), wrapped within the clip;
    /// in a blend (flag 0x200000) the target at its start.
    fn lower_at(&self, to: usize, flight: i32) -> Option<Frame> {
        let poses = self.poses.get()?;
        let g = &self.players[to].gait_channel;
        let time = if g.blend_counter > 0 {
            0.0
        } else {
            let n = poses.frames(g.row)?.max(1) as f32;
            (g.integer as f32 + g.frame_step * flight as f32).rem_euclid(n)
        };
        Some(Frame {
            record: g.row,
            time,
            looping: true,
        })
    }

    /// Receiver `i`'s catch row once its script has started (none while
    /// it is parked).
    pub(crate) fn catch_row_playing(&self, i: usize) -> Option<Catching> {
        self.catching.filter(|c| c.player == i && !c.parked)
    }

    /// Receiver `i`'s catch row still his: the pass flying to him and him
    /// free (otherwise it is dropped). A parked script his channel's bar
    /// cancels (`0x15a9c8`; the rewrite's moves and reactions stand in for
    /// the channel's 0x40) still leaves his +0x1b0 at -1.
    fn catching_kept(&mut self, i: usize) -> Option<Catching> {
        let c = self.catching.filter(|c| c.player == i)?;
        let pass_on =
            self.ball.state == BallState::Pass && self.ball.pass.is_some_and(|p| p.to == i);
        if !pass_on || !matches!(self.players[i].action, Action::None) {
            if c.parked && pass_on {
                self.players[i].received_flight = -1;
            }
            self.catching = None;
            return None;
        }
        Some(c)
    }

    /// Receiver `i`'s parked row, before his tests: the script starts in
    /// his update (`0x15a974`, in the side's update before its tail) and
    /// its first step runs at once (its bits and the blend's start, which
    /// shows the clip it starts from). True when it started this update.
    pub(crate) fn catching_start(&mut self, i: usize) -> bool {
        let Some(mut c) = self.catching_kept(i) else {
            return false;
        };
        if !c.parked {
            return false;
        }
        // `0x15a974..0x15ab4c`: the parked script starts once the flight
        // left (ball+0x64 - +0x60) is at most his +0x1b0 (the flight, or
        // the row's own count: `catch_stand_4r`/`4l` 20, `catch_stand_5`
        // 15, `catch_post_2` 10) with him on the floor; +0x1bc is cleared
        // and +0x1b0 set to -1, so a row with its own count is open to his
        // next pass only while a catch's script has not started (the
        // filter's `0x162028`).
        let left = self.ball.flight_ticks() - self.ball.flown_ticks();
        let p = &self.players[i];
        if left > p.received_flight || p.height > 0.0 {
            return false;
        }
        c.parked = false;
        self.players[i].received_flight = -1;
        self.begin_script(i, CATCH_ROWS[usize::from(c.row)].script);
        // His face (`FUN_00159190(receiver, 1, fps × 2)`, `0x15ab24..
        // 0x15ab38`).
        self.face(i, 2 * crate::TICK_HZ as i32);
        let f = self.gait_frame(i);
        c.from = Some((f.record, f.time));
        // Moving with +0x1ac: channel 0 carries his gait on and the row
        // runs on channel 2 (`a2 = 2`), which copies channel 0's state to
        // blend from (`FUN_0019c320`).
        if self.running_catch(i, c.floor) {
            let g = &self.players[i].gait_channel;
            c.lower = Some((f.record, f.time, g.frame_step));
        }
        self.catch_step(i, c);
        true
    }

    /// Receiver `i`'s catch script, one update of his channel, after his
    /// tests: the tests read the model as the last update posed it (the
    /// pose step, `0x157618..`, runs after the actor's tail in
    /// `FUN_00156f48`), so the channel's step this update shows in the next
    /// update's tests (the played captures' standing catches come on the
    /// update the ball is first within [`NEAR`] of the pelvis or a hand of
    /// the clip at the last update's frame and place: 229 of 231, one an
    /// update early; the sweep's hand rule on this update's frame gives
    /// 174). Its flight
    /// wait lasts the pass's updates left; callback 5 turns his wanted
    /// facing to the ball; the non-running rows' clip root carries him
    /// (`FUN_0019a658`), as a trick's does. His coast comes after
    /// ([`World::catching_coast`]).
    pub(crate) fn catching_tick(&mut self, i: usize) {
        let Some(c) = self.catching_kept(i) else {
            return;
        };
        if c.parked {
            return;
        }
        self.catch_step(i, c);
    }

    fn catch_step(&mut self, i: usize, mut c: Catching) {
        let left = (self.ball.flight_ticks() - self.ball.flown_ticks()).clamp(1, 255);
        c.run.flight = left as u8;
        if c.run.tick < 0 && !c.holds() {
            let v = self.players[i].velocity();
            c.drift = v.map(|k| k * crate::TICK_SECONDS);
        }
        let steps = c.steps();
        let before = c.run.shown();
        let ran = c.run.step(steps, self.players[i].height);
        let after = c.run.shown();
        // Channel 0 steps with it: the gait's cycle at its rate.
        if let Some(lower) = &mut c.lower {
            lower.1 += lower.2;
        }
        if ran.set_bits & 0x200 != 0 {
            c.catching = true;
        }
        // `9b` replaces the state word: the post catches' (`0xa00`, rows
        // 582..) keep him posting (bit 11), the others' end it.
        if ran.set_bits != 0 {
            self.players[i].posting = ran.set_bits & 0x800 != 0;
        }
        if ran.clear_bits & 0x200 != 0 {
            c.catching = false;
        }
        if ran.toward_ball {
            // `FUN_00174eb0(0, 0)`: +0xb0 the angle to the ball; the
            // facing turns to it at the walk's 46 an update
            // ([`World::catching_turn`]).
            let p = &self.players[i];
            let b = self.ball.position;
            let wanted = crate::math::atan2(b[0] - p.position[0], b[2] - p.position[1]);
            self.players[i].wanted = wanted;
            c.facing = Some(timing::facing_word(wanted));
        }
        if c.holds()
            && let Some(poses) = self.poses.get()
        {
            let step = trick::root_step(poses, before, after);
            let p = &mut self.players[i];
            let m = trick::root_motion(step, p.body(), timing::facing_word(p.heading));
            p.position[0] += m[0];
            p.position[1] += m[1];
            p.clamp_to_court();
        }
        self.catching = if c.run.done { None } else { Some(c) };
    }

    /// Receiver `i`'s move while his catch row plays (`FUN_00156f48`'s
    /// position += velocity, after the tests): the stick no longer steers
    /// him ([`Catching::holds`]); on a running row he coasts on the velocity
    /// he had, on the others it was zeroed (`97`). His walk adds nothing.
    pub(crate) fn catching_coast(&mut self, i: usize) {
        let Some(c) = self.catch_row_playing(i) else {
            return;
        };
        let p = &mut self.players[i];
        p.speed = 0.0;
        p.position[0] += c.drift[0];
        p.position[1] += c.drift[1];
        p.clamp_to_court();
    }

    /// Receiver `i`'s facing while his catch row holds him (`FUN_00168ea8`
    /// after his walk): toward callback 5's angle at 46 units an update.
    /// The walk would take his wanted facing from his course; the row's
    /// script keeps the one callback 5 gave.
    /// `from` his facing before his walk turned him.
    pub(crate) fn catching_turn(&mut self, i: usize, from: f32) {
        let Some(to) = self.catch_row_playing(i).and_then(|c| c.facing) else {
            return;
        };
        let p = &mut self.players[i];
        let now = timing::facing_word(from);
        let word = crate::guard::turn_toward(now, to, crate::guard::TURN);
        p.heading = crate::restart::heading_radians(word);
        p.wanted = crate::restart::heading_radians(to);
    }

    /// Receiver `i` posed in his catch row, while it plays: the whole body
    /// in the row's clip, or on a running catch the pose step's channels
    /// (`0x157618..0x157714`): channel 0's clip carried on
    /// ([`Catching::lower`]) under the row's from the spine up (channel 2
    /// with flag 4, bones 9..21, blended from the gait as the script found
    /// it), as the release aimed the pass ([`World::catch_target`]).
    pub(crate) fn catching_pose(&self, i: usize) -> Option<Pose> {
        let c = self.catch_row_playing(i)?;
        let poses = self.poses.get()?;
        let (body, place) = (self.players[i].body(), self.place(i));
        let shown = c.run.shown();
        let Some((record, time, _)) = c.lower else {
            return trick::pose(poses, shown, c.from, body, place);
        };
        let upper = Frame {
            record: shown.record?,
            time: shown.frame,
            looping: false,
        };
        let from = match (shown.blend, c.from) {
            (Some(w), Some((record, time))) => Some((
                Frame {
                    record,
                    time,
                    looping: true,
                },
                w,
            )),
            _ => None,
        };
        let lower = Frame {
            record,
            time,
            looping: true,
        };
        poses.overlay(lower, upper, from, UPPER_BODY, body, place)
    }

    /// The `_X` state word of the frame `i`'s posed clip shows (channel
    /// 0's +0x14): its hands (8 the right, 4 the left).
    fn shown_state(&self, i: usize) -> u32 {
        let Some(poses) = self.poses.get() else {
            return 0;
        };
        if let Some(c) = self.catch_row_playing(i) {
            // Over a run, channel 0 is the gait's.
            if let Some((record, time, _)) = c.lower {
                return poses.state(record, time as i32);
            }
            let shown = c.run.shown();
            return shown
                .record
                .map_or(0, |r| poses.state(r, shown.frame as i32));
        }
        // A scripted move's clip (a leaper's), else the gait's.
        if let Some(c) = self.channel(i).filter(|c| !c.gait)
            && let Some(r) = c.shown.record
        {
            return poses.state(r, c.shown.frame as i32);
        }
        let f = self.gait_frame(i);
        poses.state(f.record, f.time as i32)
    }

    /// `FUN_0016cd28`: the ball within [`PICKUP_REACH`] of `i` across, and
    /// no higher than his highest bone (the head, 21; the hands, 19 and 15)
    /// nor lower than his lowest (the feet, 4 and 8) by more than
    /// [`PICKUP_SLACK`].
    pub(crate) fn pickup_reach(&self, i: usize, pose: &Pose) -> bool {
        let p = &self.players[i];
        let b = self.ball.position;
        if crate::distance(p.position, [b[0], b[2]]) > PICKUP_REACH {
            return false;
        }
        let y = |k: usize| pose.joints[k].position[1];
        let low = y(4).min(y(8));
        let high = y(21).max(y(19)).max(y(15));
        let at = b[1];
        !((at < low || high < at)
            && (at - high).abs() > PICKUP_SLACK
            && (at - low).abs() > PICKUP_SLACK)
    }

    /// The tests at the end of actor `i`'s update (see the module docs),
    /// before he moves: his bones as his channel shows them (a catch row
    /// stepped by [`World::catching_tick`] first) against the ball's last
    /// move. Without the disc's animations the rewrite's own rules stand
    /// in (`World::catcher`, the arrival catch, the pass's stand-in
    /// interception).
    pub(crate) fn touch_ball(&mut self, i: usize) {
        let Some(poses) = self.poses.arc() else {
            return;
        };
        // The tip's pass is caught so too: the tip's take gives the sides
        // their roles (`FUN_00169138`), and the receiver runs his side's
        // update; the referee's toss is the jumpers' (`FUN_0015eaa0`).
        let tip_pass =
            self.opening_toss == OpeningToss::Flight && self.ball.state == BallState::Pass;
        // A restart's loose ball (phase 6, through the net): the restarting
        // side's tail takes it as in play (`0x15c3c8`: no phase test), the
        // other side's update leaves it in phase 6 (`FUN_0015da28`'s pickup
        // asks the phase not to be 6).
        let restart_pickup = matches!(self.phase, Phase::DeadBall { .. })
            && self.restarting()
            && self.ball.state == BallState::Loose
            && self.players[i].team == self.possession;
        if (self.phase != Phase::Live && !restart_pickup)
            || (self.opening_toss != OpeningToss::None && !tip_pass)
            || self.sequence.is_some()
            || !matches!(
                self.ball.state,
                BallState::Pass | BallState::Shot | BallState::Loose
            )
        {
            return;
        }
        let p = &self.players[i];
        if p.barred > 0 {
            return;
        }
        match p.action {
            Action::None
            | Action::Passing { .. }
            | Action::Stealing { .. }
            | Action::Trick { .. } => {}
            Action::Jumping { block: Some(_), .. } if p.height <= 0.0 => {}
            // A leaper (bit 15) runs his side's tests in the air on his
            // leap's clip: the near take and the sweep ([`crate::leap`]).
            _ if p.leaping() => {}
            // A dunker without the ball (bit 7): the near take alone (the
            // rest of his tests are the rewrite's to pose).
            Action::Dunking { layup: false, .. } => {}
            // A reaction plays a clip that bars him (the rewrite's reading
            // of the channel's 0x40, `FUN_0016ea70`); the jumps and the
            // dive test elsewhere ([`crate::dive`]), the moves the rewrite
            // does not pose not at all.
            _ => return,
        }
        let Some(pose) = self.actor_pose(i) else {
            return;
        };
        let offence = p.team == self.possession;
        let on_floor = p.height <= 0.0;
        let cooled = p.cooldown == 0;
        let native = self.native_ball_state();
        let dunking = !p.leaping() && matches!(p.action, Action::Dunking { .. });
        if offence {
            // `0x15bad0..0x15bf28`: in a dunk, his catch row or a leap,
            // the ball near his bones is his; `0x15bf3c..0x15bf6c`: with
            // any of those and the pass still in flight (ball+0x60 below
            // +0x64), nothing more.
            let bits = self.near_bits(i);
            if bits != 0 {
                if self.near_ball(i, &pose, bits) {
                    self.near_take(i, bits);
                    return;
                }
                if self.ball.state == BallState::Pass
                    && self.ball.flown_ticks() < self.ball.flight_ticks()
                {
                    return;
                }
            }
        }
        if dunking {
            return;
        }
        match native {
            state::LOOSE => {
                // `0x15c3c8` / `0x15e8d0`: the floor's ball, scooped when it
                // is low and slow.
                if on_floor && self.pickup_reach(i, &pose) {
                    if self.scoops(i) {
                        self.start_scoop(i);
                    } else {
                        self.give(i);
                    }
                }
                return;
            }
            state::RIM => return,
            state::PASS => {
                let Some(pass) = self.ball.pass else {
                    return;
                };
                if offence && pass.from == i && pass.from != pass.to && cooled {
                    return;
                }
                // A trick pass to himself on its way out (off a defender's
                // head or the board): the original keeps it in his hands
                // through the passout row in state 4 and throws it up
                // past his bones, out of `FUN_0016cd28`'s reach. The head
                // trajectory remains the rewrite's own, so retain its
                // first-leg exemption (the return is a plain pass).
                if offence
                    && pass.from == i
                    && pass.from == pass.to
                    && matches!(
                        pass.via,
                        crate::ball::Via::Head { .. } | crate::ball::Via::Glass { .. }
                    )
                {
                    return;
                }
                if !offence {
                    // `0x15e7d0..0x15e898`: more than three quarters flown,
                    // then the Steals roll.
                    let flown =
                        self.ball.flown_ticks() as f32 / self.ball.flight_ticks().max(1) as f32;
                    if flown <= 0.75 {
                        return;
                    }
                    let steals = (self.players[i].ratings.steals * 99.0)
                        .round()
                        .clamp(0.0, 99.0);
                    let odds = (steals / 99.0 * 15.0) as u32;
                    if self.rng.next_u32() % 100 >= odds {
                        return;
                    }
                    if let Some(hit) = self.sweep(i, &pose, &poses) {
                        self.body_knock(i, hit);
                    }
                    return;
                }
            }
            state::SHOT | state::REBOUND => {}
            _ if cooled => return,
            _ => {}
        }
        if offence {
            // Bits 8, 9, 12, 15 (`0x9300`): a rebound or block jump, the
            // catch row, a leap; or a dive: only the sweep.
            let busy = self.catching.is_some_and(|c| c.player == i && c.catching)
                || matches!(self.players[i].action, Action::Jumping { .. })
                || self.players[i].leaping();
            if !busy && self.pickup_reach(i, &pose) {
                self.give(i);
                return;
            }
        }
        let Some(hit) = self.sweep(i, &pose, &poses) else {
            return;
        };
        if on_floor && native == state::REBOUND {
            self.give(i);
            return;
        }
        if offence
            && native == state::PASS
            && let Some(pass) = self.ball.pass
            && pass.to == i
        {
            let hands = self.shown_state(i);
            let hand = hit.bones.1;
            if (hands & 8 != 0 && hand == pose::RIGHT_HAND)
                || (hands & 4 != 0 && hand == pose::LEFT_HAND)
            {
                self.catching = None;
                self.receive(pass);
                return;
            }
            if hand == 21 && pass.from == i {
                return;
            }
        }
        self.body_knock(i, hit);
    }

    /// The bits of `i`'s state word the offence's first take reads
    /// (`0x15bad0`, `0x8280`): 0x80 a dunk, 0x200 his catch row's (set and
    /// cleared by its script, [`Catching::catching`]), 0x8000 a leap.
    fn near_bits(&self, i: usize) -> u32 {
        let p = &self.players[i];
        let mut bits = 0;
        if p.leaping() {
            bits |= 0x8000;
        } else if matches!(p.action, Action::Dunking { layup: false, .. }) {
            bits |= 0x80;
        }
        if self.catch_row_playing(i).is_some_and(|c| c.catching) || self.passout_catching(i) {
            bits |= 0x200;
        }
        bits
    }

    /// `0x15bb48..0x15bd68`: the ball (ball+0x30) within [`NEAR`] of the
    /// model's bones. A leaper the pass is to himself from (ball+0x100 is
    /// +0xfc, the holder at the press) or a dunker: the hand the shown
    /// clip's frame holds out (channel 0's +0x14: 8 the right, bone 0x13;
    /// 4 the left, 0xf); anyone else with the bits (a receiver on his catch
    /// row, a leaper for a teammate's lob): the pelvis (bone 0) or either
    /// hand.
    fn near_ball(&self, i: usize, pose: &Pose, bits: u32) -> bool {
        let b = self.ball.position;
        let near = |k: usize| pose::length(sub(pose.joints[k].position, b)) <= NEAR;
        let to_self = self.ball.pass.is_some_and(|p| p.from == p.to);
        if (to_self && bits & 0x8000 != 0) || bits & 0x80 != 0 {
            let hands = self.shown_state(i);
            (hands & 8 != 0 && near(pose::RIGHT_HAND)) || (hands & 4 != 0 && near(pose::LEFT_HAND))
        } else {
            near(pose::PELVIS) || near(pose::RIGHT_HAND) || near(pose::LEFT_HAND)
        }
    }

    /// The near take (`0x15bd70..0x15bf28`): a low, slow ball is scooped
    /// ([`World::start_scoop`]'s rule), any other is his (`FUN_0014cb08`);
    /// with bit 9 set the other side is barred [`CATCH_BAR`] updates
    /// (`FUN_00140920` over its three, +0x1a8). (A dunk or a leap also sets
    /// `DAT_003d658c` 2, which nothing here reads.)
    fn near_take(&mut self, i: usize, bits: u32) {
        if self.scoops(i) {
            self.start_scoop(i);
        } else {
            self.give(i);
        }
        if bits & 0x200 != 0 {
            let team = self.players[i].team;
            for p in self.players.iter_mut().filter(|p| p.team != team) {
                p.barred = CATCH_BAR;
            }
        }
    }

    /// `FUN_00137db8` for `i`: the ball's last move against his limbs.
    fn sweep(&self, i: usize, pose: &Pose, poses: &pose::Poses) -> Option<pose::Hit> {
        let to = self.ball.position;
        let from = sub(to, self.ball.moved);
        pose::sweep(
            pose,
            from,
            to,
            self.ball.radius,
            poses.reach(self.players[i].body()),
        )
    }

    /// Whether `i`, with the floor's ball in reach, scoops it instead of
    /// taking it (`0x15c400..0x15c49c`, `0x15e8fc..0x15e9b8`): it is at most
    /// [`SCOOP_HEIGHT`] up and rises or falls at most [`SCOOP_RISE`] an
    /// update, he stands on the floor, is not already scooping (record
    /// 73), and his channel does not bar him (flag 0x40, `FUN_0016ea70`:
    /// the reaction clips stand in) nor his state bit 2 (`FUN_0016ea30`,
    /// not kept by the rewrite).
    fn scoops(&self, i: usize) -> bool {
        let p = &self.players[i];
        let b = &self.ball;
        b.position[1] <= SCOOP_HEIGHT
            && (b.velocity[1] / crate::TICK_HZ as f32).abs() <= SCOOP_RISE
            && p.height <= 0.0
            && !matches!(
                p.action,
                Action::Scoop { .. }
                    | Action::Stumbling {
                        reaction: Some(_),
                        ..
                    }
            )
    }

    /// The scoop's start (`0x15c4a4..0x15c538`): the ball goes free toward
    /// him at [`SCOOP_PULL`]'s share of the gap an update, keeping its own
    /// rise (`FUN_0014c408`, his side's last on the ball), his facing
    /// (+0xb0) turns to it (`FUN_0016f2b0`), and his channel plays
    /// [`SCOOP_STEPS`]. (`FUN_00169138` gives his side the roles at once;
    /// the rewrite's possession waits for the take.)
    fn start_scoop(&mut self, i: usize) {
        let p = &self.players[i];
        let b = self.ball.position;
        let hz = crate::TICK_HZ as f32;
        let gap = [p.position[0] - b[0], p.position[1] - b[2]];
        self.ball.velocity = [
            gap[0] / SCOOP_PULL * hz,
            self.ball.velocity[1],
            gap[1] / SCOOP_PULL * hz,
        ];
        self.ball.touch[p.team.min(1)] = Some(i as u8);
        self.ball.long[p.team.min(1)] = false;
        let wanted = crate::math::atan2(b[0] - p.position[0], b[2] - p.position[1]);
        let f = self.gait_frame(i);
        let p = &mut self.players[i];
        p.wanted = wanted;
        p.speed = 0.0;
        p.action = Action::Scoop {
            play: Scooping {
                run: Run::new(false, 0),
                from: Some((f.record, f.time)),
                facing: timing::facing_word(wanted),
            },
        };
    }

    /// One update of `i`'s scoop: the channel, the facing turning 46 an
    /// update toward the ball's, callback 0's take when nobody holds the
    /// ball, the end.
    pub(crate) fn scoop_tick(&mut self, i: usize) {
        let Action::Scoop { mut play } = self.players[i].action else {
            return;
        };
        let ran = play.run.step(SCOOP_STEPS, self.players[i].height);
        let p = &mut self.players[i];
        p.speed = 0.0;
        let now = timing::facing_word(p.heading);
        let word = crate::guard::turn_toward(now, play.facing, crate::guard::TURN);
        p.heading = crate::restart::heading_radians(word);
        p.wanted = crate::restart::heading_radians(play.facing);
        p.action = if play.run.done {
            Action::None
        } else {
            Action::Scoop { play }
        };
        // Callback 0 (`FUN_001775d8`) takes it if nobody holds it, with no
        // phase test: a restart's scoop by the restarting side (started in
        // phase 6 by [`World::touch_ball`]) takes it too, or the restart
        // waited out its limit with the ball at his feet.
        let restart_pickup = matches!(self.phase, Phase::DeadBall { .. })
            && self.restarting()
            && self.players[i].team == self.possession;
        if ran.take
            && !matches!(self.ball.state, BallState::Held(_))
            && (self.phase == Phase::Live || restart_pickup)
        {
            self.give(i);
        }
    }

    /// `FUN_0014cb08` from the tests: `i` takes the ball (the give, slot 24),
    /// and his side has it (`FUN_00169138`). A pass is caught by its
    /// receiver ([`World::receive`]); any other ball is a pickup, a rebound
    /// when a shot was last in the air.
    fn give(&mut self, i: usize) {
        if self.ball.state == BallState::Pass
            && let Some(pass) = self.ball.pass
            && pass.to == i
        {
            self.catching = None;
            self.receive(pass);
            return;
        }
        if self.catching.is_some_and(|c| c.player == i) {
            self.catching = None;
        }
        self.take_ball(i);
        self.events.push(Event::Pickup { player: i });
    }

    /// The ball off `i`'s body (`FUN_00137ed8(0.15)`, [`crate::Ball::deflect`]):
    /// moving into the limb it bounces off it and goes free
    /// (`FUN_0014c408`: a shot is loose, a ball on the rim or a rebound a
    /// rebound, which posts the block, slot 16, when his side does not have
    /// the ball and it was a shot); his cooldown starts (+0x1cc) unless it
    /// runs.
    pub(crate) fn body_knock(&mut self, i: usize, hit: pose::Hit) {
        let native = self.native_ball_state();
        let shooter = self.ball.shot.map(|s| s.shooter);
        if self.ball.deflect(&hit, 0.15) {
            self.ball.rebound = matches!(native, state::RIM | state::REBOUND);
            self.ball.touch[self.players[i].team.min(1)] = Some(i as u8);
            self.ball.long[self.players[i].team.min(1)] = false;
            if self.catching.is_some() {
                self.catching = None;
            }
            match shooter {
                Some(s) if native == state::SHOT && self.players[i].team != self.possession => {
                    self.credit_block(i, s, false);
                }
                _ => self.events.push(Event::Deflection { player: i }),
            }
        }
        let p = &mut self.players[i];
        if p.cooldown == 0 {
            p.cooldown = COOLDOWN;
        }
    }

    /// The release's bars (`FUN_00173570`, raw `0x173810..0x17391c`): the
    /// passer and his teammates other than the receiver may not touch the
    /// ball for the flight (actor+0x1a8), the receiver's bar is lifted; on a
    /// pass from a passer on the floor to a receiver not leaping and 15 or
    /// more away (`FUN_0016e948`), the other side for 35% of it.
    pub(crate) fn release_bars(&mut self, from: usize, to: usize, flight: i32) {
        let team = self.players[from].team;
        let n = flight.clamp(0, i32::from(u16::MAX)) as u16;
        for j in 0..self.players.len() {
            if self.players[j].team != team {
                continue;
            }
            self.players[j].barred = if j == to && j != from { 0 } else { n };
        }
        let leaping = self.players[to].leaping();
        if self.players[from].height <= 0.0
            && !leaping
            && crate::distance(self.players[from].position, self.players[to].position) >= 15.0
        {
            let bar = (flight as f32 * f32::from_bits(0x3eb3_3333)) as u16;
            for j in 0..self.players.len() {
                if self.players[j].team != team {
                    self.players[j].barred = bar;
                }
            }
        }
    }

    /// A ball that goes down through an armed ring outside a shot (a pass,
    /// a ball knocked or bounced loose: `FUN_001515a8`'s post, event 0x55,
    /// `FUN_00141818`): a basket for the side that attacks that ring, the
    /// player of that side the ball last went from ([`crate::Ball`]'s
    /// `touch`; the side's first player when none has), worth a long
    /// shot's points when that side's kind is a long shot's (3) and the
    /// inside points otherwise (`FUN_0012e890`). The rewrite counts it in
    /// the statistics as a jump shot; the original passes the ball's last
    /// release kind (+0x184: 1 a pass, 0 a knock) to them.
    pub(crate) fn loose_basket(&mut self) {
        let x = self.ball.position[0];
        let hoop = self
            .hoops
            .iter()
            .position(|h| (h.rim[0] >= 0.0) == (x >= 0.0))
            .unwrap_or(0);
        let Some(team) = (0..2).find(|&t| crate::target_hoop(t) == hoop) else {
            return;
        };
        let scorer = self.ball.touch[team]
            .map(usize::from)
            .filter(|&p| p < self.players.len() && self.players[p].team == team)
            .or_else(|| self.players.iter().position(|p| p.team == team));
        let long = self.ball.long[team];
        self.catching = None;
        self.ball.drop_loose();
        if let Some(p) = scorer {
            self.score_basket(p, crate::street::Basket::Jumper { long });
        }
    }

    /// `FUN_0016f900(actor, 0)` from the actor's update (`0x1579d8..
    /// 0x157a08`): in live play an actor who has been on the court
    /// (actor+0x324, set while `FUN_0016f530` finds him within ±40.5 by
    /// ±25, cleared in the restart's phase 6) is kept within ±(40.5 + 0.4
    /// y) by ±(25 + 0.4 y), y his height. The rewrite reads "has been on
    /// the court" from where he stood before this update's move, which is
    /// the same once the bound holds him. With the disc's animations only
    /// (the rules that read bones need it: a coasting receiver's lead
    /// assumes it); without them the rewrite's wider court stands.
    pub(crate) fn contain(&mut self, i: usize, before: [f32; 2]) {
        if self.poses.get().is_none() || self.phase != Phase::Live {
            return;
        }
        if before[0].abs() > 40.5 || before[1].abs() > 25.0 {
            return;
        }
        let p = &mut self.players[i];
        let margin = p.height * 0.4;
        p.position[0] = p.position[0].clamp(-40.5 - margin, 40.5 + margin);
        p.position[1] = p.position[1].clamp(-25.0 - margin, 25.0 + margin);
    }

    /// Whether a lay-up by `i` leaves with its backspin turned round: his
    /// lay-up (bit 13) of family 2 whose kind's word has bit 0
    /// ([`LAYUP_FLAGS`]).
    pub(crate) fn layup_reversed(&self, i: usize) -> bool {
        let Action::Layup { row, .. } = self.players[i].action else {
            return false;
        };
        let Some(r) = timing::MOVE_ROWS.get(usize::from(row)) else {
            return false;
        };
        r.family == 2
            && LAYUP_FLAGS
                .get(usize::from(r.id))
                .is_some_and(|f| f & 1 != 0)
    }
}

/// Whether a pass is caught by the receiver's bones ([`World::touch_ball`]):
/// one with a catch row, or a lob to a leaper (`FUN_00162aa8`), with the
/// disc's animations. It flies on past its flight until the tests take it
/// or its timer runs out ([`PASS_TAIL`]).
pub(crate) fn bone_catch(w: &World, pass: &Pass) -> bool {
    w.poses.get().is_some()
        && (usize::from(pass.catch) < CATCH_ROWS.len()
            || w.players.get(pass.to).is_some_and(|p| p.leaping()))
}
