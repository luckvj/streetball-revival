//! The team strategies' starts and updates (the AITeam lists, `team.rs`):
//! the tip, the scramble's tail, the rebounds, the fast break and the
//! end of the game. The defense, the playbook and the inbounds
//! (`inbound.rs`) keep their own modules.

use super::court::dist;
use super::fastbreak::{FastBreak, OpenLooseBall};
use super::loose::{BoxOut, DriftUpCourt, GoalTend, LooseBall, PrepareRebound, Rebound};
use super::team::{Coach, strategy};
use super::words::words;
use super::{Act, Brain, Ctx, skill};

/// Strategy vt+0x14: the start.
pub(super) fn start(ctx: &mut Ctx, coach: &mut Coach, brains: &mut [Brain], team: usize) {
    match coach.strategy {
        // `FUN_0012e4a0`: everyone waits for the toss.
        strategy::TIP => {
            for j in ctx.squad(team) {
                brains[j].push(Act::WaitForToss);
            }
        }
        strategy::SCRAMBLE => {
            let mut s = coach.scramble;
            s.start(ctx, brains, team);
            coach.scramble = s;
        }
        strategy::REBOUND => coach.rebound.start(ctx, brains, team),
        // `0x12de60`: the defence sets up two frames later.
        strategy::REBOUND_D => coach.rebound.wait = 3,
        strategy::FAST_BREAK => coach.fast_break.start(ctx, brains, team),
        strategy::DEFENSE => coach.defense.start(ctx, brains, team),
        strategy::PLAYBOOK => coach.playbook.start(ctx, team),
        strategy::INBOUND => super::inbound::start_offence(ctx, brains, team),
        strategy::INBOUND_D => super::inbound::start_defence(ctx, brains, team),
        // `FUN_001258c8` / `FUN_00125460`.
        strategy::CLEAR | strategy::LEAVE => {
            for j in ctx.squad(team) {
                brains[j].clear(ctx, j);
            }
        }
        _ => {}
    }
}

/// Strategy vt+0x1c: the update.
pub(super) fn update(ctx: &mut Ctx, coach: &mut Coach, brains: &mut [Brain], team: usize) {
    match coach.strategy {
        strategy::SCRAMBLE => {
            let mut s = coach.scramble;
            s.update(ctx, brains, team, coach.timer);
            coach.scramble = s;
        }
        strategy::REBOUND => coach.rebound.update(ctx, brains, team, coach.timer),
        strategy::REBOUND_D => coach.rebound.update_d(ctx, brains, team, coach.timer),
        strategy::FAST_BREAK => coach.fast_break.update(ctx, brains, team),
        strategy::DEFENSE => coach.defense.update(ctx, team),
        strategy::INBOUND => super::inbound::update_offence(ctx, brains, team),
        strategy::INBOUND_D => super::inbound::update_defence(ctx, brains, team),
        strategy::PLAYBOOK => {
            let mut pb = coach.playbook;
            super::playbook::update(ctx, &mut pb, brains, team);
            coach.playbook = pb;
        }
        _ => {}
    }
}

/// Strategy vt+0x44: a person's call (`FUN_0018ac98`).
pub(super) fn call(ctx: &mut Ctx, coach: &mut Coach, brains: &mut [Brain], team: usize) {
    match coach.strategy {
        strategy::DEFENSE => coach.defense.call(),
        strategy::PLAYBOOK => {
            let mut pb = coach.playbook;
            super::playbook::call(ctx, &mut pb, brains, team);
            coach.playbook = pb;
        }
        _ => {}
    }
}

/// Strategy vt+0x34 and vt+0x3c: the turbo marks below which the side
/// stops sprinting and above which it starts again. The base strategy's
/// are 0 (`0x18c360`): never save.
pub(super) fn turbo_marks(ctx: &Ctx, coach: &Coach, team: usize) -> (f32, f32) {
    match coach.strategy {
        // `0x12d180`, `0x12d198`.
        strategy::SCRAMBLE => (0.4, 0.5),
        // `FUN_00126b38`, `FUN_00126b78`.
        strategy::FAST_BREAK => (
            skill::value(ctx, team, 0.8, 0.8, 0.7, 0.5),
            skill::value(ctx, team, 0.9, 0.9, 0.7, 0.65),
        ),
        // `0x123ff8`, `0x124000`.
        strategy::DEFENSE => (coach.defense.low, coach.defense.high),
        // `FUN_0012adf0`, `FUN_0012ae30`: off Easy 0.25 and 0.35.
        strategy::PLAYBOOK => {
            if skill::difficulty(ctx, team) != 0 {
                (0.25, 0.35)
            } else {
                (0.4, 0.55)
            }
        }
        _ => (0.0, 0.0),
    }
}

/// `FUN_0018b0a0`: after the scramble's first frames, on the frames the
/// team's timer runs out, each player but the side's nearest to the ball
/// who is not rebounding and is 15 from the ball and from where it will
/// be in 10 frames (or is the third nearest) takes up his job: with his
/// side last on the ball, "open loose ball" in its front court or
/// "outlet" behind it; else "defense".
pub(super) fn scramble_tail(ctx: &mut Ctx, brains: &mut [Brain], team: usize, timer: i32) {
    if timer != 0 {
        return;
    }
    let Ok(last) = usize::try_from(ctx.game.last_team) else {
        return;
    };
    let by_ball = ctx.by_ball(team);
    let nearest = by_ball.first().copied();
    let third = by_ball.get(2).copied();
    let ball = ctx.w.ball.position;
    let v = ctx.w.ball.velocity;
    let soon = [ball[0] + v[0] / 6.0, ball[2] + v[2] / 6.0];
    for j in ctx.squad(team) {
        if Some(j) == nearest || brains[j].front_kind() == 0xb {
            continue;
        }
        let away = 15.0 < ctx.ball_dist(j) && 15.0 < dist(ctx.pos(j), soon);
        if !(away || Some(j) == third) || ctx.human_player(j) {
            continue;
        }
        if last == team {
            let side = ctx.side(team) as f32;
            if 0.0 <= side * ctx.pos(j)[0] {
                if brains[j].front_kind() != 0xe {
                    let o = OpenLooseBall::new(ctx, j);
                    brains[j].replace(ctx, j, Act::OpenLooseBall(o));
                }
            } else if brains[j].front_kind() != 0xf {
                brains[j].replace(ctx, j, Act::Outlet);
            }
        } else if brains[j].front_kind() != 10 {
            let d = super::defence::Defense::new(ctx, j);
            brains[j].replace(ctx, j, Act::Defense(d));
        }
    }
}

words! {
    /// The rebound strategies (`FUN_0012d1c8`; the offence's vtable
    /// `0x43b308`, the defence's `0x43b2b8`).
    pub(crate) struct ReboundCoach {
        /// `[3]`: the ball has come off the rim.
        off_rim: bool,
        /// `[4..6]`: the slots dealt "prepare rebound" or "drift".
        slots: [i32; 3],
        /// `[7]`: the offence's box-out flag; the defence's set-up delay.
        wait: i32,
    }
}

impl ReboundCoach {
    /// `FUN_0012db90`: the spots, the two nearest the rim to their spots
    /// (`FUN_0012d300`), the third back (`FUN_0012d478`); and against a
    /// person who has camped in the paint over `fps × 6` frames, the
    /// box-out at `skill(0, 0, 10, 75)`.
    fn start(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        ctx.deal_rebound_spots();
        self.off_rim = false;
        self.prepare(ctx, brains, team, 0);
        self.prepare(ctx, brains, team, 1);
        self.third(ctx, brains, team);
        let them = 1 - team;
        let camped = ctx
            .squad(them)
            .into_iter()
            .take(3)
            .any(|o| 360 < ctx.game.paint[o.min(5)]);
        self.wait = i32::from(camped);
        if camped {
            let p = skill::percent(ctx, team, 0, 0, 10, 0x4b);
            self.wait = i32::from(((ctx.rand() % 100) as i32) < p);
        }
    }

    /// `FUN_0012dcb0`: the base update, and while the shot flies the
    /// box-out for the first man (not nearer the rim than the shooter)
    /// not already boxing out.
    fn update(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize, timer: i32) {
        self.base(ctx, brains, team, timer);
        if self.wait == 0 || ctx.game.ball != 5 {
            return;
        }
        let Some(shooter) = ctx.w.ball.shot.map(|s| s.shooter) else {
            return;
        };
        let boxing = ctx
            .squad(team)
            .into_iter()
            .any(|j| !ctx.human_player(j) && brains[j].front_kind() == 0x13);
        if boxing {
            return;
        }
        for k in 0..3 {
            let Some(j) = ctx.nth_of_team_to_rim(team, k) else {
                return;
            };
            if !ctx.human_player(j) && j != shooter {
                brains[j].replace(ctx, j, Act::BoxOut(BoxOut::new()));
                return;
            }
        }
    }

    /// `FUN_0012de70`: two frames after the shot, the spots and the two
    /// nearest the rim (the third too with a full meter); then, off a
    /// person's side and unless a gamebreaker shot meets a full meter, a
    /// goal-tend at `skill(0, 0, 50, 80)` (`skill(0, 0, 10, 40)` while the
    /// gamebreaker task runs, game+0x434's `FUN_00140ff8`) for the nearest of the
    /// three who is up.
    fn update_d(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize, timer: i32) {
        if 0 < self.wait {
            self.wait -= 1;
        }
        if self.wait == 0 {
            self.base(ctx, brains, team, timer);
            return;
        }
        if self.wait != 1 {
            return;
        }
        ctx.deal_rebound_spots();
        self.off_rim = false;
        self.prepare(ctx, brains, team, 0);
        self.prepare(ctx, brains, team, 1);
        let full = ctx.meter_full(team);
        if full {
            self.prepare(ctx, brains, team, 2);
        }
        let gamebreaker = ctx.w.ball.shot.is_some_and(|s| s.gamebreaker > 0);
        // Never in a lesson (game+0x9c 4, `0x12df58`).
        if !ctx.human[team] && (!full || gamebreaker) && ctx.w.lesson.is_none() {
            // `0x12df68..0x12df9c`.
            let p = if ctx.w.gamebreaker_task.is_some() {
                skill::percent(ctx, team, 0, 0, 10, 40)
            } else {
                skill::percent(ctx, team, 0, 0, 0x32, 0x50)
            };
            if ((ctx.rand() % 100) as i32) < p {
                for k in 0..3 {
                    let Some(j) = ctx.nth_of_team_to_rim(team, k) else {
                        break;
                    };
                    if !ctx.blocking(j) && !ctx.fallen(j) {
                        brains[j].replace(ctx, j, Act::GoalTend(GoalTend::default()));
                        break;
                    }
                }
            }
        }
    }

    /// `FUN_0012d208`: when the ball first comes off the rim, the jumpers
    /// (`FUN_0012d540`); while it is off the rim, the side's nearest to it
    /// chases it (mode 3) unless rebounding or chasing already; and the
    /// scramble's tail.
    fn base(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize, timer: i32) {
        if !self.off_rim {
            if ctx.game.ball != 0 {
                return;
            }
            self.off_rim = true;
            jumpers(ctx, brains, team);
            return;
        }
        if ctx.game.ball == 0
            && let Some(&n) = ctx.by_ball(team).first()
        {
            let front = brains[n].queue.first().map(|node| &node.act);
            let skip = match front {
                Some(Act::Rebound(_)) => true,
                Some(Act::LooseBall(l)) => l.mode() == 3,
                _ => false,
            };
            if !skip && !ctx.human_player(n) {
                brains[n].replace(ctx, n, Act::LooseBall(LooseBall::new(3)));
            }
        }
        scramble_tail(ctx, brains, team, timer);
    }

    /// `FUN_0012d300(k)`: the side's k-th nearest the rim to his spot,
    /// the second at level 2 on a coin (or always, for the offence from
    /// over 19 out, on a second coin), else 3.
    fn prepare(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize, k: usize) {
        let Some(j) = ctx.nth_of_team_to_rim(team, k) else {
            return;
        };
        brains[j].clear(ctx, j);
        let ours = ctx.offence() == team;
        let mut level = 3;
        if k == 1 {
            let far = ours && 19.0 <= ctx.hoop_dist(j) && ctx.rand() & 1 != 0;
            if far || (ours && ctx.rand() & 1 != 0) {
                level = 2;
            }
        }
        if !ctx.human_player(j) {
            let spot = ctx.rebound_spot(j);
            let p = PrepareRebound::new(ctx, spot, level);
            brains[j].push(Act::PrepareRebound(p));
        }
        self.slots[k.min(2)] = ctx.slot(j) as i32;
    }

    /// `FUN_0012d478(2)`: the third man drifts back, unless on defence
    /// within 18 of the rim (then he too prepares).
    fn third(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        let Some(j) = ctx.nth_of_team_to_rim(team, 2) else {
            return;
        };
        brains[j].clear(ctx, j);
        if ctx.offence() == team || 18.0 <= ctx.hoop_dist(j) {
            if !ctx.human_player(j) {
                let d = DriftUpCourt::new(ctx);
                brains[j].push(Act::DriftUpCourt(d));
            }
        } else {
            self.prepare(ctx, brains, team, 2);
        }
    }
}

/// `FUN_0012d540`: where the ball comes down to 6 high; every one of the
/// side within 10 of it, or within 8 of the rim on the side the ball is
/// heading, goes for the rebound, and the nearest to it goes again (each
/// "rebound" draws once).
fn jumpers(ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
    let ball = &ctx.w.ball;
    let g = crate::GRAVITY.abs() / 3600.0;
    let pos = ball.position;
    let vel = ball.velocity.map(|v| v / 60.0);
    let apex_t = vel[1] / g;
    let apex = pos[1] + vel[1] * apex_t - g * 0.5 * apex_t * apex_t;
    let t = apex_t + ((apex - 6.0) * 2.0 / g).max(0.0).sqrt();
    let land = [pos[0] + t * vel[0], pos[2] + t * vel[2]];
    let mut best = (100.0, None);
    for j in ctx.squad(team) {
        let d = dist(ctx.pos(j), land);
        let go = d < 10.0 || {
            let near = ctx.hoop_dist(j) < 8.0;
            let side = if 0.0 <= ctx.pos(j)[1] { 1 } else { -1 };
            let heading = if 0.0 <= vel[2] { 1 } else { -1 };
            near && side == heading
        };
        if go {
            go_rebound(ctx, brains, j);
        }
        if d < best.0 {
            best = (d, Some(j));
        }
    }
    if let (_, Some(j)) = best {
        go_rebound(ctx, brains, j);
    }
}

fn go_rebound(ctx: &mut Ctx, brains: &mut [Brain], j: usize) {
    let act = match Rebound::new(ctx, j) {
        Ok(r) => Act::Rebound(r),
        Err(l) => Act::LooseBall(l),
    };
    if !ctx.human_player(j) {
        brains[j].replace(ctx, j, act);
    }
}

words! {
    /// The fast break (vtable `0x43abc8`; start `FUN_00125b60`, update
    /// `FUN_00125c70`).
    pub(crate) struct FastBreakCoach {
        /// `[3]`: the lanes are dealt; `[4]`: the roles are.
        lanes: bool,
        roles: bool,
        /// `[5]`: the holder they were dealt for (team+0xa0).
        holder: i32,
        /// `[6]`: which of two players in one lane takes which.
        flip: i32,
        /// `[7]`: the shot-clock shot was called.
        shot: bool,
        /// `[8]`: frames to the next look for a pass.
        wait: i32,
        /// `[9]`: the first look ahead is still to come.
        first: bool,
    }
}

/// The lanes the three take by the lanes they stand in (`0x3cddb0`: by
/// the handler's, then the other two's, three to a row).
const LANE_TABLE: [i32; 81] = [
    1, 0, 2, 1, 0, 2, 1, 0, 2, 1, 2, 0, 0, 1, 2, 0, 1, 2, 1, 2, 0, 0, 2, 1, 0, 1, 2, 1, 0, 2, 1, 0,
    2, 1, 0, 2, 1, 2, 0, 1, 0, 2, 1, 0, 2, 1, 2, 0, 1, 2, 0, 1, 2, 0, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2,
    1, 0, 2, 0, 1, 1, 0, 2, 1, 2, 0, 1, 2, 0, 1, 0, 2,
];

impl FastBreakCoach {
    /// `FUN_00125b60`: one draw thrown away, the first pass look after
    /// `r % (fps × 0.25) + fps × 0.2` frames, and everyone on the break.
    fn start(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        self.lanes = false;
        self.roles = false;
        self.holder = -1;
        let _ = ctx.rand();
        self.flip = 0;
        let r = ctx.rand();
        self.shot = false;
        self.first = true;
        self.wait = (r % 15) as i32 + 12;
        for j in ctx.squad(team) {
            if !ctx.human_player(j) {
                brains[j].replace(ctx, j, Act::FastBreak(FastBreak::new()));
            }
        }
    }

    /// `FUN_00125c70`.
    fn update(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        let holder = ctx.holder_of[team];
        if !self.lanes || self.holder != holder || ctx.human[team] {
            self.lanes = true;
            self.holder = holder;
            self.deal(ctx, brains, team);
        }
        if ctx.human[team] {
            return;
        }
        // `0x125ecc..0x125f60`: the checks run whoever holds the ball; the
        // passes' wait (`FUN_00126288`) counts down before it looks for a
        // holder, so it runs through an outlet pass's flight.
        let h = ctx.w.handler().filter(|&h| ctx.team(h) == team);
        if super::playbook::pocket_check(ctx, team) {
            if let Some(h) = h {
                brains[h].replace(ctx, h, Act::Pocket(false));
            }
        } else if super::playbook::cancel_check(ctx, team) {
            if let Some(h) = h {
                brains[h].replace(ctx, h, Act::Cancel(false));
            }
        } else if !self.clock_shot(ctx, brains, team) {
            self.passes(ctx, brains, team);
        }
    }

    /// The lanes and roles (`0x125c98..0x125ec8`).
    fn deal(&mut self, ctx: &Ctx, brains: &mut [Brain], team: usize) {
        let squad = ctx.squad(team);
        let Some(&h) = squad.get((self.holder.max(0) as usize) % 3) else {
            return;
        };
        let k = ctx.slot(h).min(2);
        let (ka, kb) = [(1, 2), (0, 2), (0, 1)][k];
        let (Some(&a), Some(&b)) = (squad.get(ka), squad.get(kb)) else {
            return;
        };
        let lane = |j: usize| ctx.lane(j) as usize;
        let row = lane(h) * 27 + lane(a) * 9 + lane(b) * 3;
        brains[h].lane = LANE_TABLE[row] as u8;
        if lane(a) == lane(b) {
            brains[a].lane = LANE_TABLE[row + self.flip as usize + 1] as u8;
            brains[b].lane = LANE_TABLE[row + (self.flip ^ 1) as usize + 1] as u8;
        } else {
            brains[a].lane = LANE_TABLE[row + 1] as u8;
            brains[b].lane = LANE_TABLE[row + 2] as u8;
        }
        // `FUN_00125ff8` / `FUN_00126098`: a person keeps the lane he
        // runs in, a computer teammate taking his.
        if !self.lanes_held(ctx, brains, &squad) {
            self.swap_lanes(ctx, brains, &squad, false);
            if !self.lanes_held(ctx, brains, &squad) {
                self.swap_lanes(ctx, brains, &squad, true);
            }
        }
        brains[h].role = 1;
        let ahead = |j: usize| 0.0 < ctx.pos(j)[0] * ctx.side(team) as f32;
        if !self.roles || brains[a].role == 1 || brains[b].role == 1 || (!ahead(a) && !ahead(b)) {
            self.roles = true;
            let dir = ctx.side(team) as f32;
            if dir * ctx.pos(b)[0] < dir * ctx.pos(a)[0] {
                brains[a].role = 2;
                brains[b].role = 3;
            } else {
                brains[b].role = 2;
                brains[a].role = 3;
            }
        }
    }

    fn lanes_held(&self, ctx: &Ctx, brains: &[Brain], squad: &[usize]) -> bool {
        squad
            .iter()
            .all(|&j| !ctx.human_player(j) || ctx.lane(j) == i32::from(brains[j].lane))
    }

    fn swap_lanes(&self, ctx: &Ctx, brains: &mut [Brain], squad: &[usize], any: bool) {
        for (k, &j) in squad.iter().enumerate() {
            if !ctx.human_player(j) {
                continue;
            }
            let at = ctx.lane(j);
            if at == i32::from(brains[j].lane) {
                continue;
            }
            for (n, &m) in squad.iter().enumerate() {
                let free = any || m as i32 != self.holder;
                if free && n != k && !ctx.human_player(m) && i32::from(brains[m].lane) == at {
                    brains[m].lane = brains[j].lane;
                    brains[j].lane = at as u8;
                    break;
                }
            }
        }
    }

    /// `FUN_00126a98`: under 2 on the shot clock the holder shoots, once.
    fn clock_shot(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) -> bool {
        if !self.shot
            && let Some(h) = ctx.w.handler().filter(|&h| ctx.team(h) == team)
            && ctx.shot_clock() < 2
        {
            let s = super::offence::Shoot::new(ctx, h);
            brains[h].replace(ctx, h, Act::Shoot(s));
            self.shot = true;
        }
        self.shot
    }

    /// `FUN_00126288`: the break's passes. The first look (still in the
    /// back court): the man furthest ahead, 10 past the handler, gets it
    /// with a full meter, at `skill(0, 15, 25, 30)`, or when open 6+ (not
    /// against the other side's full meter); else the nearest teammate
    /// within 20 at `(his Handles − the handler's) × 5 / 2`. After: the
    /// man nearest the rim when 5 nearer it than their deepest; the open
    /// man ahead (`FUN_00180c20` over `fps × 0.5`, openness 5+ less one
    /// under 50 Handles) at `100 − Handles` when the handler is covered
    /// (openness less 3 under 25 Handles, 2 under 50, below 2; or 2..3
    /// with someone open ahead, `FUN_00126950`). A pass waits `fps`
    /// frames; a miss `fps × 0.5`.
    fn passes(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        if 0 < self.wait {
            self.wait -= 1;
            return;
        }
        let Some(h) = ctx.w.handler().filter(|&h| ctx.team(h) == team) else {
            return;
        };
        if super::offence::juking(ctx, &brains[h], h)
            || ctx.passing_now(h, &brains[h])
            || super::offence::busy(ctx, h)
        {
            return;
        }
        let handles =
            |ctx: &Ctx, j: usize| (ctx.w.players[j].ratings.handles * 99.0).round() as i32;
        let pass = |s: &mut Self, ctx: &mut Ctx, brains: &mut [Brain], to: usize| {
            if !ctx.human_player(h) {
                let p = super::offence::Pass::new(ctx, h, Some(to));
                brains[h].replace(ctx, h, Act::Pass(p));
            }
            s.wait = 60;
        };
        if !self.first {
            let deep = ctx.nth_of_team_to_rim(team, 0);
            let their = ctx.nth_of_team_to_rim(1 - team, 0);
            if let (Some(n), Some(m)) = (deep, their)
                && n != h
                && ctx.hoop_dist(n) + 5.0 < ctx.hoop_dist(m)
            {
                pass(self, ctx, brains, n);
                return;
            }
            let hd = handles(ctx, h);
            let mut open = super::offence::openness(ctx, h);
            if hd < 25 {
                open -= 3;
            } else if hd < 50 {
                open -= 2;
            }
            if 3 < open {
                return;
            }
            if 2 <= open && !ahead_open(ctx, team, h) {
                return;
            }
            let dir = ctx.side(team) as f32;
            let mut best: Option<usize> = None;
            let mut best_open = 0;
            for j in ctx.squad(team) {
                if j == h || dir * ctx.pos(j)[0] <= dir * ctx.pos(h)[0] - 10.0 {
                    continue;
                }
                if 8.0 < ctx.between(h, j) && 30 < ctx.open_frames(j) {
                    let mut o = super::offence::openness(ctx, j);
                    if handles(ctx, j) < 50 {
                        o -= 1;
                    }
                    if best_open < o {
                        best = Some(j);
                        best_open = o;
                    }
                }
            }
            if let Some(j) = best
                && 4 < best_open
                && ((ctx.rand() % 100) as i32) < 100 - hd
            {
                pass(self, ctx, brains, j);
                return;
            }
            self.wait = 30;
            return;
        }
        let s = ctx.side(team) as f32;
        let x = ctx.pos(h)[0];
        if (0.0 <= x && s == 1.0) || (x < 0.0 && s == -1.0) {
            self.first = false;
            return;
        }
        let mut ahead: Option<usize> = None;
        let mut far = s * x + 10.0;
        for j in ctx.squad(team) {
            if j != h && far < s * ctx.pos(j)[0] {
                ahead = Some(j);
                far = s * ctx.pos(j)[0];
            }
        }
        let q = skill::percent(ctx, team, 0, 0xf, 0x19, 0x1e);
        let mut to = None;
        if let Some(a) = ahead {
            if ctx.meter_full(team) {
                to = Some(a);
            } else if !ctx.meter_full(1 - team) {
                let r = ctx.rand();
                if ((r % 100) as i32) < q || 6 <= super::offence::openness(ctx, a) {
                    to = Some(a);
                }
            }
        }
        if to.is_none()
            && let Some(m) = ctx.nth_teammate(h, 0)
            && ctx.between(h, m) < 20.0
        {
            let r = ctx.rand();
            if (r % 100) < ((handles(ctx, m) - handles(ctx, h)) * 5 / 2).max(0) as u32 {
                to = Some(m);
            }
        }
        if let Some(t) = to {
            pass(self, ctx, brains, t);
        }
        self.first = false;
    }
}

/// `FUN_00126950`: a teammate not 10 behind the handler has been open
/// `fps × 0.8` frames and is open 6+.
fn ahead_open(ctx: &Ctx, team: usize, h: usize) -> bool {
    let dir = ctx.side(team) as f32;
    ctx.squad(team).into_iter().any(|j| {
        j != h
            && dir * ctx.pos(h)[0] < dir * ctx.pos(j)[0] + 10.0
            && 48 <= ctx.open_frames(j)
            && 5 < super::offence::openness(ctx, j)
    })
}
