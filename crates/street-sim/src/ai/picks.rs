//! The screens: "set pick" (type 1, `FUN_00118508`), the roll that
//! follows it (type 2, `FUN_00117e80`), the handler's "run pick play"
//! (`FUN_00118f60`) and the screener's "pick off ball %d"
//! (`FUN_001199e0`). The pick and roll and the pick off ball plays
//! (`super::plays`) and a person's call for a pick (`super::playbook`)
//! queue them.
//!
//! A screen itself is the contact a still screener wins
//! (`crate::contact`). The screener's two pick presses (`0x400000` while
//! he waits at his spot, `0x4000000` as he steps into the defender) start
//! the original's pick animations (families `0x28a..0x28c`, which
//! `FUN_0016eb80` asks after); the simulation has no pick animation, so
//! the pad keeps the bits and the sim reads none of them, and "playing a
//! pick animation" is always no.
//!
//! Its timing constants are the static initialisers' (`FUN_00118188`
//! raw `0x118358..0x1183b8`, `FUN_00119630` raw `0x119800..0x119854`):
//! `0x4b4dc8` = fps × 1.5 (90), `0x4b4dcc` = 256 (256.4993 truncated),
//! `0x4b4dd0` = 5 (5.6206 truncated), `0x4b4efc` = 20 (20.411
//! truncated).

use super::court::{direction, dist, turn};
use super::offence::{CarrierPause, Drive, build, can_act, openness, path_open, quality, spot_in};
use super::pad::bits;
use super::plays::{can_pass_now, defender};
use super::spots::{PerimPlayer, PostCut};
use super::words::words;
use super::{Act, Brain, Ctx};

/// The frames per second the original's timings scale (`FUN_0030fa50`'s
/// +0x24).
const FPS: f32 = 60.0;
/// `0x4b4dc8`: how long a roll lasts (fps × 1.5).
const ROLL_FRAMES: u16 = 90;
/// `0x4b4dcc`: the roll's first turn away from the rim.
const ROLL_TURN: i32 = 256;
/// `0x4b4dd0`: how fast the roll's turn closes, a frame.
const ROLL_STEP: i32 = 5;
/// `0x4b4efc`: how far round the defender's circle the screener stands.
const SCREEN_ANGLE: i32 = 20;

fn who(v: i32) -> Option<usize> {
    usize::try_from(v).ok()
}

/// `FUN_00118ee8(node, j)`: which way round the rim `j` stands from the
/// screener: 1 when the turn from `j`'s angle to the screener's is not
/// negative, else -1.
fn way_round(ctx: &Ctx, screener: usize, j: usize) -> i32 {
    if turn(ctx.hoop_angle(screener), ctx.hoop_angle(j)) < 0 {
        -1
    } else {
        1
    }
}

/// A point `r` from the rim the player's side attacks along angle `a`
/// (the original adds the rim's x, ai+0xa0, and takes its z as 0).
fn round_rim(ctx: &Ctx, i: usize, a: i32, r: f32) -> [f32; 2] {
    let rim = ctx.goal(ctx.team(i));
    let d = direction(a);
    [rim[0] + d[0] * r, d[1] * r]
}

/// `FUN_0013bf60(node)`: the AI of the player holding the ball (any side).
fn holder_now(ctx: &Ctx) -> Option<usize> {
    ctx.w.handler()
}

words! {
    /// "set pick" (type 1; `FUN_00118508(ai, mode)`, vtable `0x438f90`:
    /// start `0x1188c8`, done `0x118778`, update `0x118a30`). The
    /// screener goes to a spot on the circle round the rim through the
    /// holder's defender, `SCREEN_ANGLE` round from him on the holder's
    /// side, waits there, steps into the defender, then rolls.
    pub(crate) struct SetPick {
        /// +0x1c: the holder when it was built, or -1.
        holder: i32,
        /// +0x20: his defender (`FUN_0017f238`).
        defender: i32,
        /// +0x24: the speed level (3, 2 within 5.5 of the spot).
        level: i32,
        /// +0x28: `0x3c68f0` posted (the defender within 5).
        posted: bool,
        /// +0x30: where the holder stood at the start.
        origin: [f32; 2],
        /// +0x40: which way round the rim the holder comes (±1).
        sign: i32,
        /// +0x44: finished.
        finished: bool,
        /// +0x48: set on arriving and during the pick; a count down
        /// (`FUN_00118f40`) once the holder has gone by.
        hold: i32,
        /// +0x4c: frames to arrive in.
        timeout: i32,
        /// +0x50: the mode (a person's call from wide: the side by the
        /// holder's z).
        mode: bool,
        /// +0x54: the defender came within 2.7.
        touched: bool,
        /// +0x58: frames to the end once the holder is clear.
        countdown: i32,
        /// +0x5c: frames to wait at the spot.
        wait: i32,
        /// +0x60: frames of the pick itself.
        anim: i32,
    }
}

impl SetPick {
    /// `FUN_00118508`: the holder is read now.
    pub fn new(ctx: &Ctx, mode: bool) -> Self {
        Self {
            holder: holder_now(ctx).map_or(-1, |h| h as i32),
            defender: -1,
            level: 3,
            mode,
            ..Default::default()
        }
    }

    fn holder(&self, ctx: &Ctx) -> Option<usize> {
        who(self.holder).filter(|&j| j < ctx.w.players.len())
    }

    fn defender(&self, ctx: &Ctx) -> Option<usize> {
        who(self.defender).filter(|&j| j < ctx.w.players.len())
    }

    /// `FUN_001188c8`: the holder must still hold the ball. The spot's
    /// side, the screener's AI+0x60, and the time to get there: `fps ×
    /// 1.1`, and `fps × (d − 10) × 0.1` more from beyond 10 of the holder.
    pub fn enter(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) {
        let Some(h) = self.holder(ctx).filter(|&h| holder_now(ctx) == Some(h)) else {
            self.finished = true;
            return;
        };
        let Some(d) = defender(ctx, h) else {
            self.finished = true;
            return;
        };
        self.defender = d as i32;
        self.sign = if self.mode {
            let side = ctx.side(ctx.team(h));
            if 0.0 <= ctx.pos(h)[1] { -side } else { side }
        } else {
            way_round(ctx, i, h)
        };
        b.pick_side = self.sign;
        let gap = ctx.between(i, h);
        self.origin = ctx.pos(h);
        self.timeout = (FPS * f32::from_bits(0x3f8c_cccd)) as i32;
        if 10.0 < gap {
            let more = (gap - 10.0) * f32::from_bits(0x3dcc_cccd);
            self.timeout += (FPS * more) as i32;
        }
    }

    /// `FUN_00118778`.
    pub fn done(&self, ctx: &Ctx, i: usize, age: u16) -> bool {
        if self.finished {
            return true;
        }
        let Some(h) = self.holder(ctx) else {
            return true;
        };
        // Playing a pick animation (`FUN_001817e0`): never in the sim.
        if self.anim != 0 {
            return false;
        }
        if self.hold == 0 && 20.0 < dist(ctx.pos(h), self.origin) {
            return true;
        }
        if ctx.hoop_dist(h) < ctx.hoop_dist(i) - 2.0 && ctx.hoop_dist(h) < 23.75 {
            return true;
        }
        if self.hold == 0 && self.timeout < i32::from(age) {
            return true;
        }
        if ctx.hoop_dist(h) < 13.0 {
            return true;
        }
        !self
            .defender(ctx)
            .is_some_and(|d| ctx.hoop_dist(d) <= 33.75)
    }

    /// `FUN_00118f40`: the hold's count down, true as it reaches 0.
    fn count_down(&mut self) -> bool {
        if self.hold < 1 {
            return false;
        }
        self.hold -= 1;
        self.hold == 0
    }

    /// `FUN_00118a30` (raw `0x118a30..0x118ee4`).
    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let (Some(h), Some(d)) = (self.holder(ctx), self.defender(ctx)) else {
            self.finished = true;
            return;
        };
        if !self.posted && ctx.between(i, d) <= 5.0 {
            // `0x3c68f0` {1, screener, defender}: the AI's game listener
            // takes it with an empty handler (`0x132da0`); nothing in
            // the simulation listens.
            self.posted = true;
        }
        if !self.touched {
            self.touched = ctx.between(i, d) <= f32::from_bits(0x402c_cccd);
        }
        let mut roll = false;
        if self.anim != 0 {
            if 0 < self.anim {
                self.anim -= 1;
            }
            self.hold = 1;
        } else if ctx.fallen(d) {
            roll = true;
        } else if 0 < self.wait {
            self.wait -= 1;
            if way_round(ctx, i, d) != self.sign {
                self.wait -= 1;
            }
            if 0 < self.wait {
                if self.wait < 3 {
                    b.face_point(ctx, i, ctx.pos(d), 1, 0);
                } else {
                    b.stop(ctx, i);
                    b.press(bits::PICK_STANCE);
                }
                self.tail(ctx, b, i, h, d);
                return;
            }
            if ctx.between(i, d) <= 4.5 {
                b.press(bits::PICK);
                b.face_point(ctx, i, ctx.pos(d), 1, 0);
                b.move_to(ctx, i, ctx.pos(d), 3, 0);
                self.anim = 5;
            } else {
                roll = true;
            }
        } else if self.count_down() {
            roll = true;
        } else {
            let a = ctx.hoop_angle(d) + self.sign * SCREEN_ANGLE;
            let spot = round_rim(ctx, i, a, ctx.hoop_dist(d));
            let gap = dist(ctx.pos(i), spot);
            if gap <= f32::from_bits(0x3fb3_3333) || ctx.between(i, d) < 3.0 {
                self.wait = (FPS * 0.25) as i32;
                self.hold = 1;
                b.face_point(ctx, i, ctx.pos(d), 1, 0);
                self.tail(ctx, b, i, h, d);
                return;
            }
            if gap < 5.5 {
                self.level = 2;
            }
            if self.level == 3 {
                b.press(bits::TURBO);
            }
            b.move_to(
                ctx,
                i,
                spot,
                self.level.clamp(0, 3) as usize,
                i32::from(5.0 <= gap),
            );
        }
        if roll {
            b.push(Act::Roll(Roll::new(-self.sign)));
            self.finished = true;
        }
        self.tail(ctx, b, i, h, d);
    }

    /// `0x118ddc..0x118ec4`: once the defender has touched the screener
    /// and the holder has gone by (the other way round the rim), the hold
    /// counts down half a second; once the holder is more than 20 from
    /// the defender, the pick ends `fps × 0.1` frames later. Meanwhile
    /// the screener's AI+0x5c tells the play.
    fn tail(&mut self, ctx: &Ctx, b: &mut Brain, i: usize, h: usize, d: usize) {
        if self.hold == 0 && self.touched && way_round(ctx, i, h) != self.sign {
            self.hold = (FPS * 0.5) as i32;
        }
        if 0 < self.countdown {
            self.countdown -= 1;
            if self.countdown == 0 {
                self.finished = true;
            }
            b.pick_end = true;
        } else if 20.0 < ctx.between(h, d) {
            self.countdown = (FPS * f32::from_bits(0x3dcc_cccd)) as i32;
            b.pick_end = true;
        }
    }
}

words! {
    /// The screener's roll to the rim after a pick (type 2;
    /// `FUN_00117e80(ai, way)`, vtable `0x438dc0`: done `FUN_00117ef8`,
    /// update `FUN_00117f68`): sprinting, he curls toward the rim from a
    /// quarter turn off it, the turn closing 5 units a frame.
    pub(crate) struct Roll {
        done: bool,
        /// +0x20: the turn off the line to the rim.
        turn: i32,
    }
}

impl Roll {
    pub fn new(way: i32) -> Self {
        Self {
            done: false,
            turn: if way < 0 { -ROLL_TURN } else { ROLL_TURN },
        }
    }

    /// `FUN_00117ef8`: `fps × 1.5` frames, or within 3 of the rim.
    pub fn done(&self, ctx: &Ctx, i: usize, age: u16) -> bool {
        self.done || ROLL_FRAMES < age || ctx.hoop_dist(i) < 3.0
    }

    /// `FUN_00117f68`: with the ball he drives; with nobody holding it
    /// and no pass coming to him (`FUN_0013abb8`) he is done; while a
    /// pass comes he stands.
    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if ctx.has_ball(i) {
            self.done = true;
            let d = Drive::new(ctx, i);
            b.push(Act::Drive(d));
            return;
        }
        if holder_now(ctx).is_none() {
            let coming = ctx.w.ball.state == crate::BallState::Pass
                && ctx.w.ball.pass.is_some_and(|p| p.to == i);
            if !coming {
                self.done = true;
            } else {
                b.stop(ctx, i);
            }
            return;
        }
        if self.turn == 0 {
            b.move_to(ctx, i, ctx.goal(ctx.team(i)), 3, 1);
            return;
        }
        let a = ctx.hoop_angle(i) + 0x200 - self.turn;
        b.steer(ctx, i, a, 3, 1, None);
        b.press(bits::TURBO);
        let was = self.turn;
        self.turn += if 0 <= was { -ROLL_STEP } else { ROLL_STEP };
        if (self.turn < 0) != (was < 0) {
            self.turn = 0;
        }
    }
}

words! {
    /// The handler's part of the pick and roll ("run pick play";
    /// `FUN_00118f60(ai, screener, zone, zone2, three, star)`, vtable
    /// `0x438f38`: done `0x119110`, update `0x119180`, target
    /// `0x119150`). To a spot of his zone pushed out to 27.25 from the
    /// rim; dribbling there (an embedded "carrier pause", +0x70) until
    /// the screen is near; round the screener 14 units of the rim's circle
    /// off him, a little farther out; then (the 3pt play) out to 26.75 for
    /// a shot of 5 or better, or a pass to the screener rolling, or a
    /// drive.
    pub(crate) struct RunPick {
        /// +0x1c.
        screener: i32,
        /// +0x20: the 3pt play.
        three_play: bool,
        /// +0x24: the star, or -1.
        star: i32,
        /// +0x28.
        done: bool,
        /// +0x30.
        spot: [f32; 2],
        /// +0x40: going to the spot.
        going: bool,
        /// +0x44: waiting for the screen.
        waiting: bool,
        /// +0x48: past the screener.
        past: bool,
        /// +0x4c: which way round (the screener's AI+0x60).
        side: i32,
        /// +0x50: passing to the screener.
        passed: bool,
        /// +0x54: the speed level round the screen.
        level: i32,
        /// +0x58: frames before it rises to 3.
        wait: i32,
        /// +0x5c: going out for the three.
        three: bool,
        /// +0x60: shooting.
        shoot: bool,
        /// +0x70: the embedded "carrier pause" `FUN_00101948(0, 7.0)`.
        pause: CarrierPause,
        pause_started: bool,
    }
}

impl RunPick {
    /// The constructor's spot (raw `0x119038..0x119100`): the zone's spot
    /// (`FUN_00180f80`, its draws), then along its line from the rim out
    /// to 27.25.
    pub fn new(
        ctx: &mut Ctx,
        i: usize,
        screener: usize,
        zone: i32,
        three_play: bool,
        star: Option<usize>,
    ) -> Self {
        let raw = spot_in(ctx, i, zone, false);
        let rim = ctx.goal(ctx.team(i));
        let v = [raw[0] - rim[0], raw[1] - rim[1]];
        let len = crate::math::len(v[0], v[1]);
        let q = 1.0 / len;
        let spot = [rim[0] + v[0] * 27.25 * q, rim[1] + v[1] * 27.25 * q];
        Self {
            screener: screener as i32,
            three_play,
            star: star.map_or(-1, |s| s as i32),
            spot,
            going: true,
            waiting: true,
            level: 2,
            wait: (FPS * f32::from_bits(0x3e19_999a)) as i32,
            pause: CarrierPause::new(0.0, 7.0, false),
            ..Default::default()
        }
    }

    /// vt+0x1c: the spot while he goes to it or out for the three.
    pub fn target(&self) -> Option<[f32; 2]> {
        (self.going || self.three).then_some(self.spot)
    }

    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        self.done || !ctx.has_ball(i)
    }

    fn screener(&self, ctx: &Ctx) -> Option<usize> {
        who(self.screener).filter(|&j| j < ctx.w.players.len())
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let Some(s) = self.screener(ctx) else {
            self.done = true;
            return;
        };
        let rim = ctx.goal(ctx.team(i));
        if self.going {
            b.move_to(ctx, i, self.spot, 3, 2);
            self.going = f32::from_bits(0x3fb3_3333) < dist(ctx.pos(i), self.spot);
            return;
        }
        let picking = ctx.setting_pick(s);
        if self.waiting {
            // `FUN_0013be90(+0x70)`: the embedded node's frame ("carrier
            // pause" is never done).
            if !self.pause_started {
                self.pause_started = true;
                self.pause.enter(ctx, i);
            }
            self.pause.update(ctx, b, i);
            if picking {
                self.side = ctx.pick_side(s);
                if ctx.between(i, s) < 8.0
                    || defender(ctx, i).is_some_and(|d| ctx.between(s, d) < 4.5)
                {
                    self.waiting = false;
                }
            }
            return;
        }
        if !self.past {
            if 0 < self.wait {
                self.wait -= 1;
            } else {
                self.level = 3;
            }
            // 14.722 and 28.944 truncated (`0x1192c0`, `0x1193a8`).
            let a = (ctx.hoop_angle(s) + self.side * 14) & 0x3ff;
            let to = round_rim(ctx, i, a, ctx.hoop_dist(s) + 1.0);
            b.move_to(ctx, i, to, self.level.clamp(0, 3) as usize, 1);
            let by = if ctx.hoop_angle(s) < ctx.hoop_angle(i) {
                1
            } else {
                -1
            };
            if by != self.side && picking {
                return;
            }
            self.past = true;
            if self.three_play {
                self.three = true;
                let a = ctx.hoop_angle(i) + self.side * 28;
                self.spot = round_rim(ctx, i, a, 26.75);
            }
            return;
        }
        if self.three {
            if dist(ctx.pos(i), self.spot) < f32::from_bits(0x3fb3_3333) {
                self.three = false;
                b.stop(ctx, i);
                if can_act(ctx, i) && 5 <= quality(ctx, i) {
                    self.shoot = true;
                    return;
                }
                b.pick_end = true;
                self.done = true;
            } else {
                b.move_to(ctx, i, self.spot, 3, 1);
            }
            return;
        }
        if self.shoot {
            b.stop(ctx, i);
            build(ctx, b, i, bits::SHOOT);
            return;
        }
        // The screener's openness wanted: the star's own (6, 7), the
        // star's screen (4, 5), else (5, 6).
        let (lo, hi) = if self.star == i as i32 {
            (6, 7)
        } else if self.star == s as i32 {
            (4, 5)
        } else {
            (5, 6)
        };
        let pass = can_pass_now(ctx, b, i, 6)
            && (self.passed
                || (openness(ctx, i) < 5
                    && facing_rim(ctx, s, 171)
                    && (hi <= openness(ctx, s) || (lo <= openness(ctx, s) && !path_open(ctx, i)))));
        if pass {
            self.passed = true;
            // `FUN_0013b840`: press pass to the screener.
            b.press(bits::PASS);
            b.pad.target = s as i32;
        } else if ctx.hoop_dist(i) < 16.0 {
            self.done = true;
            let d = Drive::new(ctx, i);
            b.push(Act::Drive(d));
        }
        b.move_to(ctx, i, rim, 3, 2);
    }
}

/// `FUN_0017ddc8(ai, k)`: the player faces the rim within `k` units.
fn facing_rim(ctx: &Ctx, j: usize, k: i32) -> bool {
    turn(ctx.angle_to_hoop(j), ctx.facing(j)).abs() <= k
}

words! {
    /// "pick off ball %d" (`FUN_001199e0(ai, man)`, vtable `0x439098`:
    /// done `0x119a60`, update `0x11a008`): the screener goes to 2.5 from
    /// his teammate's defender, on the line the teammate runs, waits
    /// `fps × 0.2` frames facing him and steps in.
    pub(crate) struct PickOffBall {
        /// +0x1c: the teammate the screen frees, or -1.
        man: i32,
        /// +0x20.
        done: bool,
        /// +0x24: his defender (`FUN_0017f238`).
        defender: i32,
        /// +0x28: the speed level (3, 2 within 6 of the spot).
        level: i32,
        /// +0x2c: frames of the wait at the spot.
        wait: i32,
        /// +0x30: frames both have moved away from the screener.
        away: i32,
    }
}

impl PickOffBall {
    pub fn new(ctx: &Ctx, man: Option<usize>) -> Self {
        Self {
            man: man.map_or(-1, |j| j as i32),
            defender: man.and_then(|j| defender(ctx, j)).map_or(-1, |d| d as i32),
            level: 3,
            ..Default::default()
        }
    }

    fn man(&self, ctx: &Ctx) -> Option<usize> {
        who(self.man).filter(|&j| j < ctx.w.players.len())
    }

    fn defender(&self, ctx: &Ctx) -> Option<usize> {
        who(self.defender).filter(|&j| j < ctx.w.players.len())
    }

    /// `FUN_00119d28(node, j)`: `j` moves (or, standing, faces) within
    /// 512 units of the line from the screener to him: a turn is never
    /// more than 512 (`FUN_0018cd78`; `0x44001fe8` = 512.5, truncated), so
    /// always. A screener more than 5 from the defender therefore counts
    /// up on every done test (two a frame) and gives up after five
    /// frames.
    fn moving_away(&self, _ctx: &Ctx, _j: usize) -> bool {
        true
    }

    /// `FUN_00119a60` (raw `0x119a60..0x119d24`): after `fps × 5`
    /// frames, once both have run away from the screener for over `fps ×
    /// 0.15` frames, with the defender over 40 away, or once the ball's
    /// holder is within 13 of the rim. As it ends, the screener follows
    /// up: to the perimeter when the third man heads into the paint, else
    /// a post cut to the block on the far side of the teammate's way
    /// (deep unless `r & 3` is 0).
    pub fn done(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize, age: u16) -> bool {
        if !self.done && (self.man(ctx).is_none() || 300 <= age) {
            self.done = true;
        }
        if !self.done {
            let d = self.defender(ctx);
            let far = |ctx: &Ctx, k: f32| d.is_none_or(|d| k < ctx.between(i, d));
            if self.wait == 0 && far(ctx, 5.0) {
                let both = self.man(ctx).is_some_and(|m| self.moving_away(ctx, m))
                    && d.is_some_and(|d| self.moving_away(ctx, d));
                if both {
                    self.away += 1;
                    if ((FPS * f32::from_bits(0x3e19_999a)) as i32) < self.away {
                        self.done = true;
                    }
                }
            }
            if !self.done && far(ctx, 40.0) {
                self.done = true;
            }
            if !self.done {
                let near = holder_now(ctx).is_some_and(|h| ctx.hoop_dist(h) < 13.0);
                self.done = near;
                if !near {
                    return false;
                }
            }
        }
        let Some(m) = self.man(ctx) else {
            return self.done;
        };
        let third = ctx.third_of(i, m);
        if third.is_some_and(|t| 4 < ctx.zone_ahead(t)) {
            b.push(Act::PerimPlayer(PerimPlayer::new(true)));
            return self.done;
        }
        let side = ctx.side(ctx.team(i)) as f32;
        let zone = if side * ctx.heading_for(m)[1] < side * ctx.pos(i)[1] {
            6
        } else {
            5
        };
        let r = ctx.rand();
        let c = PostCut::new(ctx, i, zone, r & 3 != 0, false);
        b.push(Act::PostCut(c));
        self.done
    }

    /// `FUN_0011a008` (raw `0x11a008..0x11a258`).
    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let (Some(m), Some(d)) = (self.man(ctx), self.defender(ctx)) else {
            self.done = true;
            return;
        };
        if 0 < self.wait {
            self.wait -= 1;
            match self.wait {
                0 => {
                    b.face_point(ctx, i, ctx.pos(d), 1, 0);
                    if ctx.between(i, d) < 4.5 {
                        b.press(bits::PICK);
                    }
                    self.done = true;
                }
                1 => b.face_point(ctx, i, ctx.pos(d), 1, 0),
                _ => {
                    b.press(bits::PICK_STANCE);
                    b.stop(ctx, i);
                }
            }
            return;
        }
        // The teammate's facing, turned an eighth of the way toward the
        // line from his defender to him (`sra` of the turn rounded toward
        // zero); the spot 2.5 from the defender that way.
        let f = ctx.facing(m);
        let t = turn(ctx.angle_between(d, m), f);
        let a = f + if t < 0 { (t + 7) >> 3 } else { t >> 3 };
        let k = direction(a);
        let p = ctx.pos(d);
        let spot = [p[0] + 2.5 * k[0], p[1] + 2.5 * k[1]];
        let gap = dist(ctx.pos(i), spot);
        if self.level == 3 && gap <= 6.0 {
            self.level = 2;
        }
        if ctx.between(i, d) <= 3.5 || gap < f32::from_bits(0x3fb3_3333) {
            self.wait = (FPS * f32::from_bits(0x3e4c_cccd)) as i32;
            b.face_point(ctx, i, ctx.pos(d), 1, 0);
            return;
        }
        b.move_to(ctx, i, spot, self.level.clamp(0, 3) as usize, 0);
    }
}
