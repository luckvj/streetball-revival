//! The handler's walk to a spot ("carrier move") and the alley-oop
//! behaviours the plays and the playbook's alley-oop check queue: the
//! leaper (`FUN_00100100`) and the second leaper who follows him
//! (`FUN_00100360`).

use super::court::dist;
use super::offence::{Pass, build, juking, should_juke};
use super::pad::bits;
use super::words::words;
use super::{Act, Brain, Ctx, skill};

words! {
    /// "carrier move" (type 0; `FUN_00101698(ai, zone, long)`, update
    /// `FUN_001017a8`, done `0x1017a0` once there): the handler walks to
    /// a spot in a zone (`FUN_00180f80`), sprinting and steering round
    /// players, juking a defender in the way (odds `skill(0, 20, 40,
    /// 80)`, `FUN_00183be8`) at most every `r % (fps × 0.5)` frames, or
    /// `r % (fps × 0.5) + fps × 0.5` after a juke.
    pub(crate) struct CarrierMove {
        spot: [f32; 2],
        there: bool,
        wait: i32,
    }
}

impl CarrierMove {
    /// The constructor draws the first wait, then the spot.
    pub fn new(ctx: &mut Ctx, i: usize, zone: i32, long: bool) -> Self {
        let r = ctx.rand();
        let wait = (r % 30) as i32;
        let spot = super::offence::spot_in(ctx, i, zone, long);
        Self {
            spot,
            there: false,
            wait,
        }
    }

    pub fn spot(&self) -> [f32; 2] {
        self.spot
    }

    pub fn done(&self) -> bool {
        self.there
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if dist(ctx.pos(i), self.spot) < 1.5 {
            self.there = true;
            b.stop(ctx, i);
            return;
        }
        let mut juked = false;
        if juking(ctx, b, i) {
            juked = true;
            let r = ctx.rand();
            self.wait = (r % 30) as i32 + 30;
        } else if self.wait < 1 {
            if should_juke(ctx, i, self.spot) {
                let p = skill::percent(ctx, ctx.team(i), 0, 20, 40, 80);
                let r = ctx.rand();
                if ((r % 100) as i32) < p {
                    juked = true;
                    build(ctx, b, i, bits::JUKE);
                    b.move_to(ctx, i, self.spot, 3, 0);
                }
            }
        } else {
            self.wait -= 1;
        }
        if !juked {
            b.sprint(ctx, i);
            b.move_to(ctx, i, self.spot, 3, 2);
        }
    }
}

/// `FUN_00181848`: without the ball and able to leap for an alley-oop
/// (`FUN_0016c450`, [`crate::World::can_leap`]: not in a jump shot, a dunk,
/// a hang or a leap, a dunk on (`FUN_0016c500` with flag 0), the ball held
/// or passed, on the floor and within 35 of the middle along the court).
pub(super) fn can_leap(ctx: &Ctx, i: usize) -> bool {
    !ctx.has_ball(i) && ctx.w.can_leap(i)
}

words! {
    /// "alley oop" leaper (type 4; `FUN_00100100(ai, follow)`, update
    /// `FUN_001001c0`, done `FUN_00100170`): run at the rim (to within
    /// 5) and, while a leap is possible, hold turbo and press shoot every
    /// other frame (the leap's press off the ball, [`crate::leap`]). In the
    /// air with the ball (not as a follower) he passes on to a teammate
    /// leaping too. Done after the leap, or at once if no leap began by the
    /// second frame.
    pub(crate) struct Leaper {
        follow: bool,
        done: bool,
        leaped: bool,
        moving: bool,
        toggle: bool,
    }
}

impl Leaper {
    pub fn new(follow: bool) -> Self {
        Self {
            follow,
            done: false,
            leaped: false,
            moving: true,
            toggle: true,
        }
    }

    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        self.done || (self.leaped && !ctx.leaping(i))
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if ctx.leaping(i) {
            self.leaped = true;
            if ctx.has_ball(i) && !self.follow {
                let team = ctx.team(i);
                let other = ctx
                    .squad(team)
                    .into_iter()
                    .find(|&j| j != i && ctx.leaping(j));
                if let Some(j) = other {
                    self.done = true;
                    let pass = Pass::new(ctx, i, Some(j));
                    b.push(Act::Pass(pass));
                }
            }
        }
        if !self.moving || ctx.hoop_dist(i) <= 5.0 {
            b.stop(ctx, i);
            self.moving = false;
        } else {
            let rim = ctx.goal(ctx.team(i));
            b.move_to(ctx, i, rim, 3, 1);
        }
        if can_leap(ctx, i) {
            b.press(bits::TURBO);
            if self.toggle {
                b.press(bits::SHOOT);
                self.toggle = false;
            } else {
                self.toggle = true;
            }
        } else if !self.leaped && 1 < b.age {
            self.done = true;
        }
    }
}

words! {
    /// The second leaper (type 4; `FUN_00100360(ai, leaper)`, update
    /// `FUN_00100470`, done `FUN_001003c8`): run at the rim, sprinting,
    /// to within 10, then turn to it and stand; once the first leaper
    /// starts down, leap too ("alley oop" with the follow flag). Done once
    /// that leap is queued, or when the first is no longer leaping, or is
    /// coming down from above 7.5.
    pub(crate) struct Follower {
        leaper: i32,
        started: bool,
        moving: bool,
    }
}

impl Follower {
    pub fn new(leaper: usize) -> Self {
        Self {
            leaper: leaper as i32,
            started: false,
            moving: true,
        }
    }

    /// The first leaper's height (`FUN_0013ab18` +4) and whether he is on
    /// the way down (his rise below 0, `FUN_0013ab28` +4).
    fn first(ctx: &Ctx, p: usize) -> (f32, bool) {
        let q = &ctx.w.players[p];
        (q.height, q.rise < 0.0)
    }

    pub fn done(&self, ctx: &Ctx) -> bool {
        let Ok(p) = usize::try_from(self.leaper) else {
            return true;
        };
        if self.started || p >= ctx.w.players.len() || ctx.fronts.get(p) != Some(&4) {
            return true;
        }
        let (height, falling) = Self::first(ctx, p);
        !(!falling || height < 7.5)
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let rim = ctx.goal(ctx.team(i));
        if !self.moving {
            b.stop(ctx, i);
            let Ok(p) = usize::try_from(self.leaper) else {
                return;
            };
            if p < ctx.w.players.len() && Self::first(ctx, p).1 {
                self.started = true;
                b.push(Act::Leaper(Leaper::new(true)));
            }
        } else if 10.0 < ctx.hoop_dist(i) {
            b.move_to(ctx, i, rim, 3, 1);
            b.sprint(ctx, i);
        } else {
            self.moving = false;
            b.face_point(ctx, i, rim, 1, 0x39);
        }
    }
}
