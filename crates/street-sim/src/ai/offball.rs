//! The small behaviours the strategies and plays queue for players off
//! the ball: "pause" (type 0x10; `FUN_0010f748`), the walk to a point
//! (type 0x18; `FUN_0010f278`), "perim pause" (`FUN_00111d50`) and "wait
//! for toss" (`FUN_0011af98`).

use super::court::dist;
use super::words::words;
use super::{Brain, Ctx};

words! {
    /// "pause" (`FUN_0010f748`, update `FUN_0010f7e8`): let go of the
    /// stick for a number of frames (done at that age, `0x10f7d0`).
    pub(crate) struct Pause {
        frames: i32,
    }
}

impl Pause {
    pub fn new(frames: i32) -> Self {
        Self { frames }
    }

    pub fn done(&self, age: u16) -> bool {
        self.frames <= i32::from(age)
    }

    pub fn update(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) {
        b.stop(ctx, i);
    }
}

words! {
    /// The walk to a point (type 0x18; `FUN_0010f278(ai, point, stop,
    /// level, slow, frames)`, done `FUN_0010f318`, update `FUN_0010f3b8`):
    /// done after `frames` or within 0.75 (letting go of the stick when
    /// `stop`); the speed level drops one within 3 when `slow` (never
    /// below a walk).
    pub(crate) struct Goto {
        spot: [f32; 2],
        stop: bool,
        level: i32,
        slow: bool,
        frames: i32,
    }
}

impl Goto {
    /// Only two functions build it, and the disc never runs either: the
    /// strategy of vtable `0x43aa60` (its start `0x125460`, update
    /// `0x1254c8`, the walks `0x1254f0`), which no constructor makes, and
    /// `0x1847c8`, which nothing calls or points to.
    #[allow(dead_code)]
    pub fn new(spot: [f32; 2], stop: bool, level: i32, slow: bool, frames: i32) -> Self {
        Self {
            spot,
            stop,
            level,
            slow,
            frames,
        }
    }

    pub fn spot(&self) -> [f32; 2] {
        self.spot
    }

    /// `FUN_0010f318`.
    pub fn done(&self, ctx: &Ctx, i: usize, age: u16) -> bool {
        self.frames <= i32::from(age) || dist(ctx.pos(i), self.spot) < 0.75
    }

    /// The done test lets go of the stick as it ends the walk.
    pub fn stops(&self) -> bool {
        self.stop
    }

    pub fn update(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) {
        let mut level = self.level;
        if level != 1 && self.slow && dist(ctx.pos(i), self.spot) < 3.0 {
            level -= 1;
        }
        b.move_to(ctx, i, self.spot, level.max(0) as usize, 1);
    }
}

words! {
    /// "perim pause" (`FUN_00111d50`, done `FUN_00111db0`, update
    /// `FUN_00111e20`): face the rim once and stand, for `frames`, or
    /// until the ball's owner comes within 10 (or there is none). On a
    /// person's side an open lane shortens the wait.
    pub(crate) struct PerimPause {
        frames: i32,
        face: bool,
    }
}

impl PerimPause {
    pub fn new(frames: i32) -> Self {
        Self { frames, face: true }
    }

    pub fn done(&self, ctx: &Ctx, i: usize, age: u16) -> bool {
        if self.frames <= i32::from(age) {
            return true;
        }
        ctx.ball_owner().is_none_or(|o| ctx.between(i, o) <= 10.0)
    }

    pub fn update(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) {
        if self.face {
            b.face_point(ctx, i, ctx.goal(ctx.team(i)), 1, 0x39);
            self.face = false;
        } else {
            b.stop(ctx, i);
        }
        if ctx.human[ctx.team(i)] && super::offence::path_open(ctx, i) {
            self.frames -= 1;
        }
    }
}
