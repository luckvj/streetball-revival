//! Running the floor: "fast break" (type 0xc; `FUN_0010a378`, update
//! `FUN_0010a430`), "wait for hc" (vtable `0x437898`), "outlet" (type
//! 0xf; `FUN_0010af48`) and "open loose ball" (type 0xe; `FUN_0010ea98`,
//! update `FUN_0010ed58`).

use super::court::dist;
use super::offence::CarrierPause;
use super::pad::bits;
use super::spots::{PerimPlayer, PostPlayer};
use super::words::words;
use super::{Act, Brain, Ctx};

/// The lanes' spots past half court, by the AI's lane (`0x3c74a0`;
/// mirrored by the side).
const LANES: [[f32; 2]; 3] = [[7.0, -17.0], [7.0, 0.0], [7.0, 17.0]];

/// Where a player who is not the handler heads once over half court
/// (`0x3c74b8`): by his role (2, 3), his lane, the ball's side of the
/// court (the second half of each row when the ball is on the side the
/// court's z sign and the attack agree on), three `{zone, deep}` choices,
/// the first whose zone neither the owner nor the third man is headed
/// for; zone −1 settles where he is.
const SPOTS: [[[[i32; 2]; 6]; 3]; 2] = [
    [
        [[5, 1], [5, 0], [1, 0], [5, 1], [1, 0], [-1, 0]],
        [[6, 1], [2, 0], [-1, 0], [5, 1], [2, 0], [-1, 0]],
        [[6, 1], [3, 0], [-1, 0], [6, 1], [6, 0], [3, 0]],
    ],
    [
        [[1, 0], [0, 0], [2, 0], [1, 0], [0, 0], [2, 0]],
        [[3, 0], [2, 0], [4, 0], [1, 0], [2, 0], [0, 0]],
        [[3, 0], [4, 0], [2, 0], [3, 0], [4, 0], [0, 0]],
    ],
];

words! {
    /// "fast break": the lane the strategy dealt (AIPhysPlayer+0x54,
    /// `FUN_00125c70`) to just past half court, sprinting (the handler
    /// juking a defender in the way, odds `skill(0, 20, 60, 100)` every
    /// `fps × 0.2` frames), then a spot by the role and the lanes the
    /// others took, then "wait for hc" and a pause (or the handler's
    /// "carrier pause", the others' "perim player"/"post player").
    pub(crate) struct FastBreak {
        done: bool,
        spot: [f32; 2],
        first: bool,
        respot: i32,
        slow: i32,
        jog: i32,
        run: i32,
        juke: i32,
    }
}

impl FastBreak {
    /// `FUN_0010a378`.
    pub fn new() -> Self {
        Self {
            first: true,
            ..Default::default()
        }
    }

    /// `FUN_0010a3f0`: done, or the side lost the ball.
    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        self.done || ctx.offence() != ctx.team(i)
    }

    /// vt+0x24 (`0x10a420`): the spot it heads for.
    pub fn spot(&self) -> [f32; 2] {
        self.spot
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if !self.first {
            if !ctx.closing_on_ball(i) {
                if dist(ctx.pos(i), self.spot) <= 1.4 {
                    b.push(Act::WaitForHalfCourt);
                    let r = ctx.rand();
                    b.push(Act::Pause(super::offball::Pause::new((r % 12) as i32 + 6)));
                    let ball = ctx.w.ball.position;
                    b.face_point(ctx, i, [ball[0], ball[2]], 1, 0x39);
                    self.done = true;
                } else {
                    self.pace(ctx, i);
                    self.go(ctx, b, i);
                }
            } else {
                let r = ctx.rand();
                b.push(Act::Pause(super::offball::Pause::new((r % 12) as i32 + 6)));
                self.settle(ctx, b, i);
                b.stop(ctx, i);
            }
            return;
        }
        if self.respot == 0 || dist(ctx.pos(i), self.spot) < 1.0 {
            self.choose(ctx, b, i);
            self.respot = 30; // fps × 0.5
        } else {
            self.respot -= 1;
        }
        if !ctx.has_ball(i) {
            if ctx.w.handler().is_some() {
                self.pace(ctx, i);
            } else {
                self.jog = 0;
                self.slow = 0;
            }
            self.go(ctx, b, i);
            return;
        }
        let mut avoid = 2;
        let mut juked = false;
        if !super::offence::juking(ctx, b, i) {
            if self.juke < 1 {
                self.juke = 12; // fps × 0.2
                let p = super::skill::percent(ctx, ctx.team(i), 0, 20, 60, 100);
                if super::offence::should_juke(ctx, i, self.spot) && ((ctx.rand() % 100) as i32) < p
                {
                    avoid = 0;
                    juked = true;
                    super::offence::build(ctx, b, i, bits::JUKE);
                }
            } else {
                self.juke -= 1;
            }
        } else {
            self.juke = 60;
        }
        if !juked {
            b.sprint(ctx, i);
        }
        b.move_to(ctx, i, self.spot, 3, avoid);
    }

    /// `FUN_0010a828`: in the back court (or within 4 of half court) the
    /// spot is down the lane, the run straight across when the lane is
    /// farther across than ahead; over half court the first stage ends
    /// and a player without the ball takes a spot from [`SPOTS`], the
    /// handler a "carrier pause".
    fn choose(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let p = ctx.pos(i);
        let side = ctx.side(ctx.team(i)) as f32;
        let lane = (b.lane as usize).min(2);
        if side * p[0] < 4.0 {
            let lx = side * LANES[lane][0];
            let lz = side * LANES[lane][1];
            self.spot = [lx, lz];
            let dx = lx - p[0];
            let mut dz = (lz - p[1]).abs();
            self.spot[0] = if dx.abs() < dz {
                if dx < 0.0 {
                    dz = -dz;
                }
                p[0] + dz
            } else {
                p[0] + dx
            };
            return;
        }
        self.first = false;
        if ctx.has_ball(i) {
            b.push(Act::CarrierPause(CarrierPause::new(0.0, 9.0, false)));
            self.done = true;
            return;
        }
        if b.role == 1 {
            return;
        }
        let role = (b.role as usize).clamp(2, 3) - 2;
        let owner = ctx.ball_owner();
        let rows = &SPOTS[role][lane];
        let mut pick = 0;
        if let Some(o) = owner {
            let mut s = side;
            if ctx.pos(o)[1] < 0.0 {
                s = -s;
            }
            let half = if 0.0 <= s { 3 } else { 0 };
            pick = half;
            let third = ctx.third_man(i);
            match third {
                None => pick = half + 2,
                Some(t) => {
                    let taken = |row: [i32; 2]| headed(ctx, o, row) || headed(ctx, t, row);
                    if taken(rows[half]) {
                        pick = if taken(rows[half + 1]) {
                            half + 2
                        } else {
                            half + 1
                        };
                    }
                }
            }
        }
        let [zone, deep] = rows[pick];
        if zone == -1 {
            self.settle(ctx, b, i);
        } else {
            self.spot = super::offence::spot_in(ctx, i, zone, deep != 0);
        }
    }

    /// `FUN_0010aac8`: well ahead of the handler (over 30) a player slows
    /// to a walk for `fps × 0.4` frames; over 24 ahead he jogs for `fps ×
    /// 0.5`.
    fn pace(&mut self, ctx: &Ctx, i: usize) {
        if self.run != 0 {
            return;
        }
        let Some(h) = ctx.w.handler() else {
            return;
        };
        let side = ctx.side(ctx.team(i)) as f32;
        let ahead = (ctx.pos(i)[0] - ctx.pos(h)[0]) * side;
        if self.jog != 0 || ahead <= 30.0 {
            if 24.0 < ahead {
                self.jog = 30;
            }
        } else {
            self.slow = 24;
        }
    }

    /// `FUN_0010abf0`: the speed level the pace allows; a run lasts
    /// `fps × 0.3` frames.
    fn go(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) {
        let level = if 0 < self.run {
            self.slow = 0;
            self.jog = 0;
            self.run -= 1;
            3
        } else if 0 < self.slow {
            self.jog = 0;
            self.slow -= 1;
            1
        } else if 0 < self.jog {
            self.jog -= 1;
            2
        } else {
            self.run = 18;
            3
        };
        b.move_to(ctx, i, self.spot, level, 1);
    }

    /// `FUN_0010ace0`: settle into the half court.
    fn settle(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) {
        if ctx.has_ball(i) {
            b.push(Act::CarrierPause(CarrierPause::new(0.0, 9.0, false)));
        } else if b.role == 2 {
            b.push(Act::PostPlayer(PostPlayer::default()));
        } else {
            b.push(Act::PerimPlayer(PerimPlayer::new(false)));
        }
        self.done = true;
    }
}

/// `FUN_0010ae20`: a player is headed (`FUN_0013ab38`) for the zone, and
/// for the paint's side the same depth.
fn headed(ctx: &Ctx, j: usize, [zone, deep]: [i32; 2]) -> bool {
    let at = ctx.heading_for(j);
    if ctx.zone_at(j, at) != zone {
        return false;
    }
    zone < 5 || ((36.75 - at[0].abs() < 8.0) as i32) == deep
}

words! {
    /// "open loose ball" (type 0xe; `FUN_0010ea98`, update `FUN_0010ed58`,
    /// done `FUN_0010eaf8`): the scramble's man for his side's outlet: a
    /// spot on the low block by the ball (or the wing or top, when a
    /// teammate holds the block, `FUN_0010eb38`), there a look at the ball
    /// for `r % (fps × 0.2) + fps × 0.4` frames and a new spot; within 15
    /// of the ball he goes for it (a loose ball, mode 2).
    pub(crate) struct OpenLooseBall {
        done: bool,
        spot: [f32; 2],
        there: bool,
        wait: i32,
    }
}

impl OpenLooseBall {
    pub fn new(ctx: &mut Ctx, i: usize) -> Self {
        let mut s = Self::default();
        s.respot(ctx, i);
        s
    }

    pub fn done(&self, ctx: &Ctx) -> bool {
        self.done || ctx.w.handler().is_some()
    }

    /// `FUN_0010eb38`.
    fn respot(&mut self, ctx: &mut Ctx, i: usize) {
        let team = ctx.team(i);
        let side = ctx.side(team) as f32;
        let ball = ctx.w.ball.position;
        let near_side = side * ball[2] < 0.0;
        let block = if near_side { 6 } else { 5 };
        let deep = ctx.nth_of_team_to_rim(team, 0);
        let mate_in = |ctx: &Ctx, zone: i32| {
            ctx.squad(team)
                .into_iter()
                .any(|j| j != i && ctx.zone_at(j, ctx.heading_for(j)) == zone)
        };
        self.spot = if deep == Some(i) || !mate_in(ctx, block) {
            super::offence::paint_spot(ctx, i, block, true)
        } else {
            let first = if near_side { 3 } else { 1 };
            let zone = if mate_in(ctx, first) {
                if !mate_in(ctx, 2) {
                    2
                } else if first != 1 {
                    1
                } else {
                    3
                }
            } else {
                first
            };
            super::offence::spot_in(ctx, i, zone, true)
        };
        if dist(ctx.pos(i), self.spot) < 5.0 {
            self.spot = ctx.pos(i);
        }
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if !self.there {
            if 1.4 < dist(ctx.pos(i), self.spot) {
                b.move_to(ctx, i, self.spot, 3, 1);
            } else {
                self.there = true;
                let ball = ctx.w.ball.position;
                b.face_point(ctx, i, [ball[0], ball[2]], 1, 0x39);
                let r = ctx.rand();
                self.wait = (r % 12) as i32 + 24;
            }
        } else if self.wait != 0 {
            self.wait -= 1;
            b.stop(ctx, i);
        } else {
            self.respot(ctx, i);
            self.there = false;
        }
        if ctx.ball_dist(i) < 15.0 {
            self.done = true;
            b.push(Act::LooseBall(super::loose::LooseBall::new(2)));
        }
    }
}

/// "outlet" (type 0xf; `FUN_0010af48`, update `FUN_0010b000`): run for
/// the front court (angle 256 or 768, sprinting) until 2 past half court
/// (`FUN_0010af90`).
pub(super) fn outlet(ctx: &Ctx, b: &mut Brain, i: usize) {
    let a = if ctx.side(ctx.team(i)) == 1 { 768 } else { 256 };
    b.steer(ctx, i, a, 3, 1, None);
}

pub(super) fn outlet_done(ctx: &Ctx, i: usize) -> bool {
    2.0 < ctx.side(ctx.team(i)) as f32 * ctx.pos(i)[0]
}
