//! Loose balls and rebounds: "loose ball" (type 0xd; `FUN_0010dd68`,
//! update `FUN_0010e0a0`), the rebound behaviours the rebound strategies
//! deal ("prepare rebound", "drift up court", "rebound", "goal tend",
//! "box out"), and the loose-ball scramble the teams run (`FUN_0012cb40` /
//! `FUN_0012cbb0`).

use super::court::dist;
use super::pad::bits;
use super::words::words;
use super::{Act, Brain, Ctx};
use crate::BallState;

words! {
    /// "loose ball": go after the ball. `mode` is how hard (1..3; 3
    /// sprints). Ported from `FUN_0010e0a0` (raw `0x10e0a0..0x10e9a8`): on
    /// the floor and not in a rebound jump, once its wait is over, the run
    /// at the mode's speed level (3 is 2 while he is barred, `FUN_0017dea0`),
    /// with the turbo at 3. A ball above his height plus 2 (`FUN_001819a8`)
    /// is met where it comes down to that height, or jumped for; a lower
    /// one is run at, or dived for (raw `0x10e504..0x10e7dc`, once a
    /// behaviour: `dive`, +0x28, set by the constructor `FUN_0010dd68`;
    /// [`LooseBall::dives`]); within 1.5 he stops, and in a restart he
    /// waits out his bar and his cooldown. Every update ends with the
    /// failsafe that hands him a ball left free for ten seconds
    /// ([`LooseBall::failsafe`], `0x10e87c..0x10e968`).
    pub(crate) struct LooseBall {
        mode: i32,
        dive: bool,
        /// +0x20: updates to wait before going on (a restart's stop: his
        /// +0x1a8 and +0x1cc, `0x10e81c..0x10e84c`).
        wait: i32,
        /// +0x24: at the high ball's landing point and not jumping for it:
        /// he stands there (`0x10e344`, `0x10e4a4`).
        there: bool,
        /// +0x2c: the rebound press in the pad the last update left (bit
        /// 20 of `FUN_0013a970`'s word): no second press while it is held.
        pressed: bool,
        /// +0x30: the updates the ball has lain free ([`LooseBall::failsafe`]).
        free: i32,
    }
}

/// The skill blends the behaviour reads (`FUN_0017dda8`): the loose time
/// before a jump for a high ball, and before a dive.
const JUMP_WAIT: [f32; 4] = [1.2, 0.8, 0.8, 0.6];
const DIVE_WAIT: [f32; 4] = [1.6, 1.0, 1.0, 0.6];

impl LooseBall {
    pub fn new(mode: i32) -> Self {
        Self {
            mode,
            dive: true,
            ..Default::default()
        }
    }

    /// The loose time `blend` asks for, in updates (`trunc(fps * skill)`).
    fn frames(ctx: &Ctx, team: usize, blend: [f32; 4]) -> i32 {
        let [a, b, c, d] = blend;
        (crate::TICK_HZ as f32 * super::skill::value(ctx, team, a, b, c, d)) as i32
    }

    /// The dive (`0x10e504..0x10e7dc`) for `i`, the ball low, at `level`:
    /// still allowed this behaviour, nobody on his side a person
    /// (game+0x9a), play on (`FUN_00140558`), the ball under 7 high and
    /// loose for `trunc(fps * skill(1.6, 1.0, 1.0, 0.6))` updates
    /// (game+0x440), at speed level 3, 6..14 from it (`FUN_0018d160`); no
    /// teammate nearer it or diving (`FUN_001817a0`); an opponent within 3
    /// farther than he is from it (that is the dive), but none within 4 of
    /// it or within half his distance; and the ball within 42 of the middle
    /// along and 26.5 across (`0x10e728..0x10e768`, its position).
    fn dives(&self, ctx: &mut Ctx, i: usize, level: i32) -> bool {
        if !self.dive || level != 3 {
            return false;
        }
        let team = ctx.team(i);
        if ctx.squad(team).into_iter().any(|j| ctx.human_player(j)) || !ctx.live() {
            return false;
        }
        let ball = ctx.w.ball.position;
        let wait = Self::frames(ctx, team, DIVE_WAIT);
        if !(ball[1] < 7.0 && wait <= ctx.game.loose && ctx.w.ball.state == BallState::Loose) {
            return false;
        }
        let d = ctx.ball_dist(i);
        if !(6.0 < d && d < 14.0) {
            return false;
        }
        let nearer = ctx.squad(team).into_iter().any(|j| {
            j != i
                && (ctx.ball_dist(j) < d
                    || matches!(ctx.w.players[j].action, crate::Action::Dive { .. }))
        });
        if nearer {
            return false;
        }
        let mut contest = false;
        for j in ctx.squad(team ^ 1) {
            let dj = ctx.ball_dist(j);
            if dj - 3.0 < d {
                contest = true;
                if dj < 4.0 || dj < d * 0.5 {
                    return false;
                }
            }
        }
        contest && ball[0].abs() <= 42.0 && ball[2].abs() <= 26.5
    }

    /// The jump for a high ball's last checks (`0x10e254..0x10e290`,
    /// `0x10e42c..0x10e468`): the nearest opponent standing
    /// (`FUN_0017f2e0(i, 0, 0)`) within 15, and no teammate within 10 in
    /// the air or a rebound jump (`FUN_0010e9a8`).
    fn jump_clear(ctx: &Ctx, i: usize) -> bool {
        let near = ctx
            .nth_opponent(i, 0, false)
            .is_some_and(|j| ctx.between(i, j) < 15.0);
        let team = ctx.team(i);
        near && !ctx
            .squad(team)
            .into_iter()
            .any(|j| j != i && ctx.between(i, j) < 10.0 && (ctx.airborne(j) || ctx.rebounding(j)))
    }

    /// The jump (`0x10e470..0x10e48c`): two turbos and the rebound, unless
    /// the rebound is held from the last update.
    fn jump(&self, b: &mut Brain) {
        if !self.pressed {
            b.press(bits::TWO_TURBOS);
            b.press(bits::REBOUND);
        }
    }

    /// vt+0x3c (`0x10f150`): the mode.
    pub fn mode(&self) -> i32 {
        self.mode
    }

    /// Done once a player holds the ball (`FUN_0010e010`).
    pub fn done(&self, ctx: &Ctx) -> bool {
        ctx.w.handler().is_some()
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if self.wait > 0 {
            self.wait -= 1;
        }
        if !(ctx.airborne(i) || ctx.rebounding(i)) {
            if self.wait == 0 {
                self.go(ctx, b, i);
            } else {
                b.stop(ctx, i);
            }
        }
        self.pressed = b.pad.buttons & bits::REBOUND != 0;
        self.failsafe(ctx, i);
    }

    /// `0x10e87c..0x10e968`, every update: the updates the ball lies free
    /// (nobody holding it, `FUN_0013bf60` and ball+0xf4; not falling,
    /// ball+0x188) and, with nobody on either side a person (game+0x9a and
    /// +0x9b), loose (ball state 1), or in a restart (phase 6) lying 10.5
    /// up or higher, as on the rim or the top of the glass. Anything else
    /// starts the count again. At `fps × 10` the ball is his (the take,
    /// `FUN_0014cb28`; [`super::Game::free_take`]).
    fn failsafe(&mut self, ctx: &mut Ctx, i: usize) {
        let persons = ctx.human[0] || ctx.human[1];
        let ball = &ctx.w.ball;
        let free = !matches!(ball.state, BallState::Held(_))
            && !ball.airborne
            && ((!persons && ctx.game.ball == 1)
                || (ctx.game.phase == 6 && 10.5 <= ball.position[1]));
        if !free {
            self.free = 0;
            return;
        }
        self.free += 1;
        if 10 * crate::TICK_HZ as i32 <= self.free && ctx.game.free_take == 0 {
            ctx.game.free_take = i as i32 + 1;
        }
    }

    /// `0x10e12c..0x10e858`: the behaviour's step on the floor.
    fn go(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let mut level = self.mode;
        if level == 3 && ctx.w.players[i].barred != 0 {
            level = 2;
        }
        let ball = ctx.w.ball.position;
        let team = ctx.team(i);
        // Per update, as the ball keeps them (+0x40, +0x118).
        let hz = crate::TICK_HZ as f32;
        let v = ctx.w.ball.velocity.map(|c| c / hz);
        let g = crate::GRAVITY / (hz * hz);
        let h = ctx.height_ft(i) + 2.0;
        let above = ball[1] - h;
        if 0.0 < above {
            let wait = Self::frames(ctx, team, JUMP_WAIT);
            if wait < ctx.game.loose
                && above < 7.0
                && 4.5 < above
                && v[1] <= 0.0
                && ctx.ball_dist(i) < 3.5
                && Self::jump_clear(ctx, i)
            {
                self.jump(b);
                return;
            }
            // Where it comes down to h (`0x10e298..0x10e318`).
            let t = v[1] / g + (2.0 * (ball[1] + 0.5 * v[1] * v[1] / g - h) / g).sqrt();
            let at = [ball[0] + v[0] * t, ball[2] + v[2] * t];
            let d = dist(ctx.pos(i), at);
            if d <= 1.4 || self.there {
                // `0x10e350..0x10e4a8`: jump while it comes down within
                // 0.3..0.37 s of him, else stand there.
                if wait < ctx.game.loose
                    && d <= 4.0
                    && 4.5 < above
                    && (hz * 0.3) as i32 as f32 <= t
                    && t <= (hz * f32::from_bits(0x3ebb_bc2c)) as i32 as f32
                    && Self::jump_clear(ctx, i)
                {
                    self.jump(b);
                } else {
                    self.there = true;
                    b.stop(ctx, i);
                }
                return;
            }
            if level == 3 {
                b.sprint(ctx, i);
            }
            b.move_to(ctx, i, at, level as usize, 0);
            return;
        }
        if ctx.ball_dist(i) <= 1.5 {
            b.stop(ctx, i);
            // A restart (phase 6): wait out his bar and his cooldown.
            if ctx.game.phase == 6 {
                let p = &ctx.w.players[i];
                let hold = p.barred as i32 | p.cooldown as i32;
                if hold != 0 {
                    self.wait = hold;
                }
            }
            return;
        }
        if level == 3 {
            b.sprint(ctx, i);
        }
        self.there = false;
        if self.dives(ctx, i, level) {
            // The dive's press, and the run toward where the ball will be
            // ten updates on at speed level 3 (`0x10e778..0x10e7dc`).
            self.dive = false;
            b.press(bits::DIVE);
            b.move_to(ctx, i, [ball[0] + v[0] * 10.0, ball[2] + v[2] * 10.0], 3, 0);
            return;
        }
        b.move_to(ctx, i, [ball[0], ball[2]], level as usize, 0);
    }
}

words! {
    /// "prepare rebound" (`FUN_001161e8`, update `FUN_00116338`): after
    /// `r % (fps × 0.3) + fps × 0.1` frames, walk to the spot the rebound
    /// strategy dealt (`0x3cf620`, `FUN_0012d790`), at the given speed
    /// level until 2.5 from it, and stop within 1.25. The side's man
    /// nearest the rim jumps for a ball falling off the rim above 12.5
    /// (turbo and rebound, `0x116470`) while within 10 of the rim with no
    /// teammate up, then waits `r % (fps × 0.2) + fps × 0.2` frames. Done
    /// once the ball is neither shot nor on the rim.
    pub(crate) struct PrepareRebound {
        spot: [f32; 2],
        level: i32,
        done: bool,
        there: bool,
        wait: i32,
        jump: i32,
    }
}

impl PrepareRebound {
    pub fn new(ctx: &mut Ctx, spot: [f32; 2], level: i32) -> Self {
        let r = ctx.rand();
        let wait = (r % 18) as i32 + 6;
        let r = ctx.rand();
        Self {
            spot,
            level,
            wait,
            jump: (r % 18) as i32,
            ..Default::default()
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if !matches!(ctx.game.ball, 5 | 6) {
            b.stop(ctx, i);
            self.done = true;
            return;
        }
        if !self.there && self.wait <= i32::from(b.age) {
            let d = dist(ctx.pos(i), self.spot);
            if d < 1.25 {
                b.stop(ctx, i);
                self.there = true;
            } else {
                let level = if 2.5 < d { self.level as usize } else { 2 };
                b.move_to(ctx, i, self.spot, level, 1);
            }
        } else {
            b.stop(ctx, i);
        }
        let team = ctx.team(i);
        if ctx.w.ball.velocity[1] < 0.0
            && ctx.game.ball == 6
            && !ctx.rebounding(i)
            && ctx.rebounders(team) == 0
            && ctx.hoop_dist(i) < 10.0
            && ctx.nth_of_team_to_rim(team, 0) == Some(i)
        {
            if self.jump < 1 {
                if ctx.w.ball.position[1] <= 12.5 {
                    return;
                }
                self.there = false;
                b.press(bits::TWO_TURBOS);
                b.press(bits::REBOUND);
                let r = ctx.rand();
                self.jump = (r % 12) as i32 + 12;
            } else {
                self.jump -= 1;
            }
        }
    }
}

words! {
    /// "drift up court" (`FUN_00116580`, update `FUN_00116648`): the third
    /// man back. After `r % (fps × 0.2) + fps × 0.2` frames standing,
    /// walk toward the basket the side defends, at speed level 2 for
    /// `fps × 0.3` frames whenever past 13 into the front court or with an
    /// opponent behind him (`FUN_00116778`), else level 1. Done once
    /// someone holds the ball.
    pub(crate) struct DriftUpCourt {
        done: bool,
        wait: i32,
        hurry: i32,
    }
}

impl DriftUpCourt {
    pub fn new(ctx: &mut Ctx) -> Self {
        let r = ctx.rand();
        Self {
            wait: (r % 12) as i32 + 12,
            ..Default::default()
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if ctx.w.handler().is_some() {
            b.stop(ctx, i);
            self.done = true;
            return;
        }
        if i32::from(b.age) < self.wait {
            b.stop(ctx, i);
            return;
        }
        let offence = ctx.offence();
        let target = ctx.goal(1 - offence);
        let side = ctx.side(offence) as f32;
        let level = if 13.0 < side * ctx.pos(i)[0] || Self::beaten(ctx, i) {
            self.hurry = 18;
            2
        } else if self.hurry < 1 {
            1
        } else {
            self.hurry -= 1;
            2
        };
        b.move_to(ctx, i, target, level, 1);
    }

    /// `FUN_00116778`: on offence, an opponent is nearer the end his side
    /// attacks.
    fn beaten(ctx: &Ctx, i: usize) -> bool {
        let them = 1 - ctx.team(i);
        if them == ctx.offence() {
            return false;
        }
        let dir = ctx.side(them) as f32;
        let x = ctx.pos(i)[0];
        ctx.squad(them)
            .into_iter()
            .any(|o| dir * x < dir * ctx.pos(o)[0])
    }
}

words! {
    /// "rebound" (type 0xb; `FUN_00116860`, update `FUN_00116cd8`, done
    /// `FUN_00116c28`): go for a ball coming off the rim. The constructor
    /// times the jump from the player's height (feet × 1.2 × 1.25 the
    /// standing reach, the jump top 2 + Rebounds × 4 / 99 above it): when
    /// the ball will still be above the jump top after the rise plus
    /// `fps × 0.13333` frames, it waits for the ball to fall to it; when
    /// it will already be below even the standing reach + 2, the player
    /// chases it as a loose ball instead. The wait gains `fps × skill(0.25,
    /// 0.25, 0, 0)` and `r % (fps × skill(0.216667, 0.066667, 0.216667,
    /// 0.066667))` frames. Until then he runs to where the ball will be;
    /// then he jumps (turbo and rebound) unless a teammate within 12 is up
    /// (one fewer counts for the defence against a full meter) or he
    /// stands under the backboard, in which case he chases it loose.
    pub(crate) struct Rebound {
        done: bool,
        level: i32,
        target: [f32; 2],
        jumped: bool,
        wait: i32,
    }
}

impl Rebound {
    /// The constructor; `Err` when the player is to chase the ball loose
    /// instead (`0x116a50`: `FUN_0010dd68(ai, 3)` pushed, the node done).
    /// The ball's speeds are per frame here, as the original keeps them.
    pub fn new(ctx: &mut Ctx, i: usize) -> Result<Self, LooseBall> {
        let ball = &ctx.w.ball;
        let g = crate::GRAVITY.abs() / 3600.0;
        let pos = ball.position;
        let vel = ball.velocity.map(|v| v / 60.0);
        let p = &ctx.w.players[i];
        let rebounds = (p.ratings.rebounds * 99.0).round().clamp(0.0, 99.0);
        let reach = p.height_ft() * 1.2 * 1.25;
        let low = reach + 2.0;
        let top = low + rebounds * 4.0 / 99.0;
        let rise = ((top - reach) * 2.0 / g).sqrt() as i32;
        let lead = rise + 8; // fps × 0.13333
        let t = lead as f32;
        let at = |t: f32| -g * t * t * 0.5 + vel[1] * t + pos[1];
        let mut s = Self {
            level: 2,
            ..Default::default()
        };
        let time;
        if at(t) < top {
            let t = ((low - reach) * 2.0 / g).sqrt() + 8.0;
            if at(t) < low {
                return Err(LooseBall::new(3));
            }
            time = t;
        } else {
            let apex_t = vel[1] / g;
            let apex = at(apex_t);
            let fall = ((apex - top) * 2.0 / g).sqrt();
            time = (apex_t + fall).max(t);
            s.wait = time as i32 - lead;
        }
        let team = ctx.team(i);
        s.wait += (60.0 * super::skill::value(ctx, team, 0.25, 0.25, 0.0, 0.0)) as i32;
        let r = ctx.rand();
        let span = (60.0
            * super::skill::value(ctx, team, 0.216_667, 0.066_667, 0.216_667, 0.066_667))
            as u32;
        s.wait += (r % span.max(1)) as i32;
        s.target = [pos[0] + time * vel[0], pos[2] + time * vel[2]];
        if dist(ctx.pos(i), s.target) <= 1.4 {
            s.level = 0;
        }
        Ok(s)
    }

    /// `FUN_00116c28`.
    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        self.done
            || ctx.rebounding(i)
            || ctx.blocking(i)
            || ctx.airborne(i)
            || ctx.game.phase == 6
            || ctx.w.handler().is_some()
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let d = dist(ctx.pos(i), self.target);
        if i32::from(b.age) < self.wait {
            if self.level < 2 || 1.75 < d {
                if d <= 1.4 {
                    self.level = 0;
                }
            } else {
                self.level = 1;
            }
            b.move_to(ctx, i, self.target, self.level as usize, 1);
        } else if self.level != 0 && 7.0 < d {
            self.done = true;
            b.push(Act::LooseBall(LooseBall::new(3)));
        } else {
            let team = ctx.team(i);
            let mut up = ctx
                .squad(team)
                .into_iter()
                .filter(|&j| j != i)
                .filter(|&j| {
                    ctx.rebounding_or_asking(j) && !ctx.fallen(j) && ctx.between(i, j) < 12.0
                })
                .count();
            if 0 < up && ctx.offence() != team && ctx.meter_full(team) {
                up -= 1;
            }
            let p = ctx.pos(i);
            if up == 0 && !(38.0 <= p[0].abs() && p[1].abs() < 4.0) {
                if !self.jumped {
                    b.press(bits::TWO_TURBOS);
                    b.press(bits::REBOUND);
                }
            } else {
                self.done = true;
                b.push(Act::LooseBall(LooseBall::new(3)));
            }
            b.sprint(ctx, i);
            let ball = ctx.w.ball.position;
            b.move_to(ctx, i, [ball[0], ball[2]], 3, 0);
        }
        self.jumped = b.pad.buttons & bits::REBOUND != 0;
    }
}

words! {
    /// "goal tend" (`FUN_00116fb8`, start `FUN_00117010`, update
    /// `FUN_001171b0`): the defence's man under the rim. He picks a spot
    /// 2..7 out from the rim toward the ball and the frames the ball takes
    /// to reach it, give or take (`r % (fps × 0.1) − 2`), runs there, and
    /// when the ball is within `fps / 3` frames and he within 5 asks the
    /// pad builder for a block (while more than `fps / 6` frames remain).
    pub(crate) struct GoalTend {
        done: bool,
        spot: [f32; 2],
        there: bool,
        left: i32,
    }
}

impl GoalTend {
    pub fn enter(&mut self, ctx: &mut Ctx) {
        let rim = ctx.goal(ctx.offence());
        let ball = ctx.w.ball.position;
        let a = super::court::angle_toward(rim, [ball[0], ball[2]]);
        let r = ctx.rand();
        let out = (r % 50) as f32 / 10.0 + 2.0;
        let d = super::court::direction(a);
        self.spot = [rim[0] + out * d[0], rim[1] + out * d[1]];
        let far = dist(self.spot, [ball[0], ball[2]]);
        let v = ctx.w.ball.velocity;
        let speed = crate::math::len(v[0], v[2]) / 60.0;
        self.left = if 0.0 < speed {
            let t = (far / speed) as i32;
            let r = ctx.rand();
            t - ((r % 6) as i32 - 2)
        } else {
            0
        };
    }

    pub fn done(&self) -> bool {
        self.done
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let d = dist(ctx.pos(i), self.spot);
        self.left -= 1;
        if d < 5.0 && self.left - 20 < 1 {
            if 10 < self.left {
                super::offence::build(ctx, b, i, bits::BLOCK);
            }
            self.done = true;
        }
        if d < 1.5 {
            b.stop(ctx, i);
            self.there = true;
        } else if !self.there {
            b.move_to(ctx, i, self.spot, 3, 1);
        }
    }
}

words! {
    /// "box out" (type 0x13; `FUN_00117290`, update `FUN_00117318`, done
    /// once the ball is no longer a shot, `FUN_00117300`): sent by the
    /// offence's rebound strategy against a person who camps in the
    /// paint. Every `fps × 0.2` frames he picks the camper nearest the
    /// ball (opponents nearer the rim than the shot came from, in the paint
    /// over `fps × 6` frames, `FUN_001174e8`) and runs at him, the speed
    /// level by the gap every `fps × 0.15` frames, stopping within 1.4;
    /// within 4 he presses the pad's 0x4000000 (an action the rewrite does
    /// not map) once and stands facing the rim.
    pub(crate) struct BoxOut {
        target: i32,
        frames: i32,
        level: i32,
        settle: i32,
        pushed: bool,
        facing: bool,
    }
}

impl BoxOut {
    pub fn new() -> Self {
        Self {
            target: -1,
            frames: 12,
            level: 2,
            facing: true,
            ..Default::default()
        }
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if self.pushed {
            if self.facing {
                self.facing = false;
                b.face_point(ctx, i, ctx.goal(ctx.team(i)), 1, 0x39);
            } else {
                b.stop(ctx, i);
            }
            return;
        }
        self.frames += 1;
        let target = usize::try_from(self.target).ok();
        let Some(t) = target.filter(|_| self.frames < 12) else {
            self.choose(ctx, i);
            return;
        };
        let d = ctx.between(i, t);
        if d < 4.0 {
            self.pushed = true;
        } else {
            self.settle += 1;
            if 9 <= self.settle {
                self.level = if 4.0 < d { 3 } else { 2 };
                self.settle = 0;
            }
        }
        if self.level == 3 {
            b.sprint(ctx, i);
        }
        b.move_to(ctx, i, ctx.pos(t), self.level as usize, 1);
    }

    /// `FUN_001174e8`.
    fn choose(&mut self, ctx: &Ctx, i: usize) {
        let them = 1 - ctx.team(i);
        let shot = ctx.shot_distance();
        let ball = ctx.w.ball.position;
        self.target = ctx
            .squad(them)
            .into_iter()
            .filter(|&o| ctx.hoop_dist(o) < shot && 360 < ctx.game.paint[o.min(5)])
            .min_by(|&a, &b| {
                let da = dist(ctx.pos(a), [ball[0], ball[2]]);
                let db = dist(ctx.pos(b), [ball[0], ball[2]]);
                da.total_cmp(&db)
            })
            .map_or(-1, |o| o as i32);
        self.frames = 0;
    }
}

words! {
    /// The loose-ball scramble (`FUN_0012cbb0`, phase 2, both sides):
    /// the nearest player who is up and on the floor chases hardest (mode
    /// 3); within 6 frames the rest join: a mode-2 player within 20 goes
    /// in (1), a player not the side's third nearest, 40 or more away or
    /// 25 across, hangs back (2), else goes in (1). After 6 frames the
    /// tail `FUN_0018b0a0` takes over the others
    /// ([`super::strategy::scramble_tail`]).
    pub(crate) struct Scramble {
        modes: [i32; 3],
        timer: i32,
    }
}

impl Scramble {
    /// `FUN_0012cb40`.
    pub fn start(&mut self, ctx: &Ctx, brains: &mut [Brain], team: usize) {
        self.modes = [0; 3];
        self.timer = 0;
        for i in ctx.squad(team) {
            brains[i].clear(ctx, i);
        }
    }

    pub fn update(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize, timer: i32) {
        self.timer += 1;
        let squad = ctx.squad(team);
        let by_ball = ctx.by_ball(team);
        let up = |j: usize| {
            !ctx.dunking(j)
                && !ctx.leaping(j)
                && !ctx.fallen(j)
                && !ctx.blocking(j)
                && !ctx.rebounding(j)
                && !ctx.airborne(j)
        };
        let lead = by_ball.iter().take(2).copied().find(|&j| up(j));
        let third = by_ball.get(2).copied();
        if let Some(j) = lead {
            let k = slot_in(&squad, j);
            let chasing = brains[j].queue.first().map(|n| n.act.kind()) == Some(0xd);
            if !(self.modes[k] == 3 && chasing) {
                self.send(ctx, brains, j, k, 3);
            }
        }
        if self.timer > 6 {
            super::strategy::scramble_tail(ctx, brains, team, timer);
            return;
        }
        // `0x12cd68..0x12ce84`.
        let ball = ctx.w.ball.position;
        for (k, &j) in squad.iter().enumerate().take(3) {
            if Some(j) == lead || self.modes[k] >= 3 {
                continue;
            }
            let d = ctx.ball_dist(j);
            if self.modes[k] == 2 && d <= 20.0 {
                self.send(ctx, brains, j, k, 1);
                continue;
            }
            if self.modes[k] < 2
                && Some(j) != third
                && (40.0 <= d || 25.0 < (ctx.pos(j)[1] - ball[2]).abs())
            {
                self.send(ctx, brains, j, k, 2);
                continue;
            }
            if self.modes[k] == 0 {
                self.send(ctx, brains, j, k, 1);
            }
        }
    }

    /// `FUN_0012cec0`: put a computer player on the ball.
    fn send(&mut self, ctx: &Ctx, brains: &mut [Brain], j: usize, k: usize, mode: i32) {
        if ctx.human_player(j) {
            return;
        }
        self.modes[k] = mode;
        brains[j].replace(ctx, j, Act::LooseBall(LooseBall::new(mode)));
    }
}

fn slot_in(squad: &[usize], j: usize) -> usize {
    squad.iter().position(|&x| x == j).unwrap_or(0).min(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Player, Rules, World};

    fn loose_world() -> World {
        let players = vec![
            Player::new(0, [10.0, 0.0], 0.0),
            Player::new(1, [-10.0, 0.0], 0.0),
        ];
        let mut w = World::new(crate::tests::hoops(), players, Rules::default(), 1);
        w.ball.drop_loose();
        w.ball.airborne = false;
        w.ball.position = [0.0, 0.5, 0.0];
        w
    }

    /// `0x10e87c..0x10e968`: the count runs while the ball lies free and
    /// loose with nobody a person, starts again at any update it falls,
    /// and at `fps × 10` asks the take for him.
    #[test]
    fn a_ball_left_free_ten_seconds_is_handed_to_the_chaser() {
        let mut w = loose_world();
        let ((first, again, last), _) = super::super::with_ctx(&w, |ctx| {
            ctx.game.ball = 1;
            let mut lb = LooseBall::new(2);
            for _ in 0..599 {
                lb.failsafe(ctx, 1);
            }
            let first = ctx.game.free_take;
            lb.failsafe(ctx, 1);
            let again = ctx.game.free_take;
            ctx.game.free_take = 0;
            lb.free = 0;
            for _ in 0..300 {
                lb.failsafe(ctx, 1);
            }
            (first, again, lb.free)
        });
        assert_eq!((first, again, last), (0, 2, 300));
        // Falling: the count starts again.
        w.ball.airborne = true;
        let (free, _) = super::super::with_ctx(&w, |ctx| {
            ctx.game.ball = 1;
            let mut lb = LooseBall::new(2);
            lb.free = 500;
            lb.failsafe(ctx, 1);
            lb.free
        });
        assert_eq!(free, 0);
    }

    /// With a person playing only a restart's ball lying 10.5 up counts.
    #[test]
    fn with_a_person_only_a_restarts_high_ball_counts() {
        let mut w = loose_world();
        let (low, _) = super::super::with_ctx(&w, |ctx| {
            ctx.human[0] = true;
            ctx.game.ball = 1;
            ctx.game.phase = 6;
            let mut lb = LooseBall::new(2);
            lb.failsafe(ctx, 1);
            lb.free
        });
        w.ball.position[1] = 10.6;
        let (high, _) = super::super::with_ctx(&w, |ctx| {
            ctx.human[0] = true;
            ctx.game.ball = 1;
            ctx.game.phase = 6;
            let mut lb = LooseBall::new(2);
            lb.failsafe(ctx, 1);
            lb.free
        });
        assert_eq!((low, high), (0, 1));
    }
}
