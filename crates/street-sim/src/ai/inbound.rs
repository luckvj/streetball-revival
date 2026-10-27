//! The restart (game phase 6): the inbound strategies and the walks they
//! queue.
//!
//! A restart starts (`FUN_001406b8`) with every ready flag down
//! (game+0x404..0x418) and every actor driven by the computer
//! (`FUN_00140880`, actor+0x298), people included. The possessing team's
//! strategy (vtable `0x43b0e0`, start `FUN_0012bc40`, update
//! `FUN_0012c210`) sends its inbounder after the ball ("loose ball", mode
//! 3) and then to his spot behind a line ("throw inbound", `FUN_0010b658`,
//! update `FUN_0010b820`), and the other two to theirs ("inbound pos",
//! `FUN_0010ba18`, update `FUN_0010bb10`); the defending team's (vtable
//! `0x43b088`, start `FUN_0012c520`, update `FUN_0012c578`) sends its
//! three to theirs. Each raises his flag at his spot (`FUN_00140830`); the
//! inbounder raises his holding the ball. With all six up, the inbounder
//! waits his drawn frames and passes in; the game leaves phase 6 for
//! phase 4 once all six are up and nobody holds the ball
//! (`FUN_0013e4c8`, `FUN_00140840`).

use super::court::dist;
use super::loose::LooseBall;
use super::offence::Pass;
use super::pad::bits;
use super::words::words;
use super::{Act, Brain, Ctx};
use crate::restart::{self, Facts, Plan};

words! {
    /// The restart's shared state: the strategies' globals and the
    /// game's ready flags.
    pub(crate) struct Restart {
        /// `DAT_003ce9b8`: the row of the spot tables.
        row: i32,
        /// `DAT_004e7408..0x4e7410`: the slot at each spot.
        order: [i32; 3],
        /// `DAT_003ce9b0`: the defence has spots to take.
        defence: bool,
        /// `DAT_003ce9b4`: the possessing team chose its play again; the
        /// defence's radii grow by 10.
        replanned: bool,
        /// `DAT_003ce9bc`: frames to the possessing team's next look.
        check: i32,
        /// game+0x404..0x418, by player.
        ready: [bool; 6],
        /// The player (plus one) the possessing team's start hands the
        /// ball to (`FUN_0014cb28` from `FUN_0012bc40`); the world does it
        /// after the computer's frame. 0 for none.
        hand: i32,
    }
}

impl Restart {
    /// The play as it stands.
    pub fn plan(&self) -> Plan {
        Plan {
            row: self.row.clamp(0, 5) as usize,
            order: self.order.map(|s| s.clamp(0, 2) as usize),
        }
    }

    /// The six ready flags (game+0x404..0x418) are up. `FUN_00140840`
    /// also needs the restart's hold clear ([`super::Ctx::restart_ready`]).
    pub fn all_ready(&self) -> bool {
        self.ready.iter().all(|&r| r)
    }

    /// For [`super::State::validate`].
    pub fn valid(&self, players: usize) -> bool {
        (0..6).contains(&self.row)
            && self.order.iter().all(|s| (0..3).contains(s))
            && (0..=10).contains(&self.check)
            && (0..=players as i32).contains(&self.hand)
    }
}

/// `FUN_00140830(game, i, 1)`.
fn raise(ctx: &mut Ctx, i: usize) {
    if let Some(r) = ctx.game.restart.ready.get_mut(i) {
        *r = true;
    }
}

/// `FUN_00140830(game, i, 0)`.
fn lower(ctx: &mut Ctx, i: usize) {
    if let Some(r) = ctx.game.restart.ready.get_mut(i) {
        *r = false;
    }
}

/// The team's players' heights in feet by slot, both teams.
fn heights(ctx: &Ctx) -> [[f32; 3]; 2] {
    std::array::from_fn(|team| {
        let squad = ctx.squad(team);
        std::array::from_fn(|k| squad.get(k).map_or(6.5, |&j| ctx.height_ft(j)))
    })
}

/// Spot `k` of `team`'s table and its face point, read as the strategy
/// reads them now (the row and order as they stand).
fn spot(ctx: &Ctx, pushers: [usize; 3], team: usize, k: usize) -> ([f32; 2], [f32; 2]) {
    let plan = ctx.game.restart.plan();
    let s = restart::spot_at(plan.row, pushers, ctx.offence(), team, k, &heights(ctx));
    ([s.position[0], s.position[2]], [s.target[0], s.target[2]])
}

/// The possessing team's restart start, `FUN_0012bc40`.
pub(super) fn start_offence(ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
    let squad = ctx.squad(team);
    if squad.len() != 3 {
        return;
    }
    let holder = ctx.w.handler();
    let ball = match holder {
        Some(h) => ctx.pos(h),
        None => [ctx.w.ball.position[0], ctx.w.ball.position[2]],
    };
    let previous = ctx.game.restart.plan();
    let fresh_clock = ctx
        .w
        .rules
        .shot_clock
        .is_none_or(|full| ctx.w.shot_clock == full);
    let human: [bool; 3] = std::array::from_fn(|k| ctx.human_player(squad[k]));
    let facts = Facts {
        team,
        positions: std::array::from_fn(|k| ctx.pos(squad[k])),
        ball,
        holder: holder.filter(|&h| ctx.team(h) == team).map(|h| ctx.slot(h)),
        previous,
        fresh_clock,
        people: human.iter().filter(|&&p| p).count(),
        human,
        heights: heights(ctx)[team],
        lesson: ctx.w.lesson.is_some(),
    };
    let plan = restart::plan(&facts);
    // The row is written as it is found; the order only at the end, so
    // the walks' spots are pushed by the last restart's players.
    ctx.game.restart.row = plan.row as i32;
    let pushers = previous.order;
    let [inbounder, second, third] = plan.order.map(|k| squad[k]);
    let (spot0, face0) = spot(ctx, pushers, team, 0);
    brains[inbounder].clear(ctx, inbounder);
    brains[inbounder].replace(ctx, inbounder, Act::LooseBall(LooseBall::new(3)));
    let throw = ThrowInbound::new(ctx, second, spot0, face0);
    brains[inbounder].push(Act::ThrowInbound(throw));
    brains[second].clear(ctx, second);
    brains[third].clear(ctx, third);
    if let Some(h) = holder
        && h != inbounder
    {
        if ctx.team(h) == team {
            let pass = Pass::new(ctx, h, Some(inbounder));
            brains[h].replace(ctx, h, Act::Pass(pass));
        } else {
            ctx.game.restart.hand = inbounder as i32 + 1;
        }
    }
    for (k, j) in [(1, second), (2, third)] {
        let (at, face) = spot(ctx, pushers, team, k);
        let radius = restart::radius(plan.row, team, team, k);
        let walk = InboundPos::new(radius, at, face, Some(inbounder));
        brains[j].push(Act::InboundPos(walk));
    }
    let r = &mut ctx.game.restart;
    r.defence = true;
    r.order = plan.order.map(|k| k as i32);
    r.check = 10;
    r.replanned = false;
}

/// The defending team's restart start, `FUN_0012c520`: the three drop
/// what they were doing.
pub(super) fn start_defence(ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
    for j in ctx.squad(team) {
        brains[j].clear(ctx, j);
    }
}

/// The possessing team's restart update, `FUN_0012c210`: every 10
/// frames, unless a pass is in the air, an inbounder without the ball
/// goes after it again, one holding it without "throw inbound" at the
/// front takes it up again, and with the ball in anyone else's hands the
/// play is chosen again (with both their flags lowered).
pub(super) fn update_offence(ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
    let squad = ctx.squad(team);
    if squad.len() != 3 {
        return;
    }
    let r = &mut ctx.game.restart;
    if 0 < r.check {
        r.check -= 1;
    }
    if r.check != 0 {
        return;
    }
    r.check = 10;
    // Ball state 4: thrown, or pressed and still in the passer's hands.
    if ctx.w.ball.state == crate::BallState::Pass || ctx.w.windup.is_some() {
        return;
    }
    let plan = ctx.game.restart.plan();
    let inbounder = squad[plan.order[0]];
    let throw = |ctx: &mut Ctx, brains: &mut [Brain]| {
        let (at, face) = spot(ctx, plan.order, team, 0);
        let receiver = squad[plan.order[1]];
        let t = ThrowInbound::new(ctx, receiver, at, face);
        brains[inbounder].push(Act::ThrowInbound(t));
    };
    match ctx.w.handler() {
        None => {
            if brains[inbounder].front_kind() != 0xd {
                lower(ctx, inbounder);
                brains[inbounder].clear(ctx, inbounder);
                brains[inbounder].replace(ctx, inbounder, Act::LooseBall(LooseBall::new(3)));
                throw(ctx, brains);
            }
        }
        Some(h) if h == inbounder => {
            let throwing = matches!(
                brains[inbounder].queue.first().map(|n| &n.act),
                Some(Act::ThrowInbound(_))
            );
            if !throwing {
                lower(ctx, inbounder);
                brains[inbounder].clear(ctx, inbounder);
                throw(ctx, brains);
            }
        }
        Some(h) => {
            lower(ctx, h);
            lower(ctx, inbounder);
            start_offence(ctx, brains, team);
            ctx.game.restart.replanned = true;
        }
    }
}

/// The defending team's restart update, `FUN_0012c578`: once the
/// possessing team has chosen, each of the three takes his spot (the
/// radius 10 wider after a second choice).
pub(super) fn update_defence(ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
    if !ctx.game.restart.defence {
        return;
    }
    ctx.game.restart.defence = false;
    let squad = ctx.squad(team);
    if squad.len() != 3 {
        return;
    }
    let plan = ctx.game.restart.plan();
    for k in 0..3 {
        let j = squad[plan.order[k]];
        let mut radius = restart::radius(plan.row, ctx.offence(), team, k);
        if ctx.game.restart.replanned {
            radius += 10.0;
        }
        let (at, face) = spot(ctx, plan.order, team, k);
        let walk = InboundPos::new(radius, at, face, None);
        brains[j].replace(ctx, j, Act::InboundPos(walk));
    }
}

words! {
    /// "throw inbound" (type 0; `FUN_0010b658`, done `FUN_0010b7d0`,
    /// update `FUN_0010b820`): walk to the spot with the ball, slowing
    /// within 6 and 2, stop within 1.4, turn to the face point, and stand
    /// ready holding the ball; with all six ready, wait the drawn frames
    /// and press pass to the receiver every other frame. Done with all
    /// six ready and the ball gone.
    pub(crate) struct ThrowInbound {
        /// The receiver (+0x1c's player).
        receiver: i32,
        spot: [f32; 2],
        face: [f32; 2],
        /// +0x40, +0x44, +0x48, +0x4c, +0x50, +0x54.
        arrived: bool,
        delay: i32,
        turned: bool,
        level: i32,
        toggle: bool,
        told: bool,
    }
}

impl ThrowInbound {
    /// The constructor draws the wait before the pass,
    /// `r % (fps × 0.2) + fps × 0.1`: 6..17 frames (raw
    /// `0x10b6c4..0x10b740`); in a lesson (game+0x9c 4,
    /// `0x10b744..0x10b7a0`) `fps × 2`, or `fps × 0.5` when a tutorial page
    /// was due at the launch (`DAT_003c7af8` clear): the School's captures
    /// start their lessons 144 updates after the launch, 55 after one with
    /// a page.
    pub fn new(ctx: &mut Ctx, receiver: usize, spot: [f32; 2], face: [f32; 2]) -> Self {
        let r = ctx.rand();
        let fps = crate::TICK_HZ as f32;
        let delay = match ctx.w.lesson {
            Some(_) if ctx.w.lesson_page => (fps * 0.5) as i32,
            Some(_) => (fps * 2.0) as i32,
            None => (r % 12) as i32 + 6,
        };
        Self {
            receiver: receiver as i32,
            spot,
            face,
            delay,
            level: 3,
            ..Default::default()
        }
    }

    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        ctx.restart_ready() && !ctx.has_ball(i)
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if self.arrived {
            if !self.turned {
                self.turned = true;
                b.face_point(ctx, i, self.face, 1, 0);
                return;
            }
            if ctx.restart_ready() {
                b.stop(ctx, i);
                if 0 < self.delay {
                    self.delay -= 1;
                    return;
                }
                if !self.toggle {
                    // `FUN_0013b810`: the pass, to the receiver.
                    b.press(bits::PASS);
                    b.pad.target = self.receiver;
                }
                self.toggle = !self.toggle;
                return;
            }
            b.stop(ctx, i);
            if ctx.has_ball(i) {
                raise(ctx, i);
            }
            return;
        }
        // Its first frame (+0x54 clear) posts `0x3c6510`, the listeners'
        // slot 41 ([`crate::Event::InboundWalk`]).
        if !self.told {
            ctx.game.inbound_walk = i as i32 + 1;
        }
        self.told = true;
        let d = dist(ctx.pos(i), self.spot);
        self.arrived = d <= 1.4;
        if self.arrived {
            b.stop(ctx, i);
            return;
        }
        if self.level == 3 {
            if d <= 6.0 {
                self.level = 2;
            }
        } else if self.level == 2 && d <= 2.0 {
            self.level = 1;
        }
        b.move_to(ctx, i, self.spot, self.level as usize, 1);
    }
}

words! {
    /// "inbound pos" (type 0; `FUN_0010ba18`, done `FUN_0010bab8`, update
    /// `FUN_0010bb10`): walk to the spot (a jog within 3), stop within
    /// 1.4, turn to the face point (if 0x39 or more off), then stand
    /// ready; ready already within `radius` when that is over 2. Holding
    /// the ball he stands. Done with all six ready, turned and without
    /// the ball.
    pub(crate) struct InboundPos {
        radius: f32,
        spot: [f32; 2],
        face: [f32; 2],
        /// +0x44: the inbounder (the possessing team's walks), or -1.
        inbounder: i32,
        /// +0x48, +0x4c, +0x50.
        arrived: bool,
        level: i32,
        faced: bool,
    }
}

impl InboundPos {
    pub fn new(radius: f32, spot: [f32; 2], face: [f32; 2], inbounder: Option<usize>) -> Self {
        Self {
            radius,
            spot,
            face,
            inbounder: inbounder.map_or(-1, |j| j as i32),
            level: 3,
            ..Default::default()
        }
    }

    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        !ctx.has_ball(i) && self.faced && ctx.restart_ready()
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let d = dist(ctx.pos(i), self.spot);
        if 2.0 < self.radius && d < self.radius {
            raise(ctx, i);
        }
        if ctx.has_ball(i) {
            self.faced = false;
            b.stop(ctx, i);
            return;
        }
        if !self.arrived {
            if d < 1.4 {
                self.arrived = true;
                b.stop(ctx, i);
            } else {
                if d < 3.0 {
                    self.level = 2;
                }
                b.move_to(ctx, i, self.spot, self.level as usize, 1);
            }
        } else if !self.faced {
            self.faced = true;
            b.face_point(ctx, i, self.face, 1, 0x39);
        } else {
            b.stop(ctx, i);
            raise(ctx, i);
        }
    }

    /// Where the walk goes (vt+0x1c).
    pub fn spot(&self) -> [f32; 2] {
        self.spot
    }
}

impl ThrowInbound {
    /// Where the walk goes (vt+0x1c).
    pub fn spot(&self) -> [f32; 2] {
        self.spot
    }
}
