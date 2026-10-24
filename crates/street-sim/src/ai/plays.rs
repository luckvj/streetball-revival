//! The playbook's sixteen plays (`FUN_00126f60` builds them, in this
//! order): each a small object with a header (the team, the star, the
//! handler, the post man (role 2), the perimeter man (role 3), an
//! invalid flag and, for the two-variant plays, the variant) and its own
//! fields, run through a vtable: +0x1c done, +0x24 start, +0x2c update,
//! +0x34/+0x3c/+0x44/+0x4c/+0x54 whether the playbook's pass, shot,
//! penetrate, post-cut and alley-oop checks may interrupt it, +0x5c
//! whether it may run with a full meter, +0x64 whether it suits a
//! player.
//!
//! The rewrite keeps the one running play (the original's objects keep
//! their fields between runs, but every start sets what its update and
//! done test read). Its fields are the original's words from +0x28 on.
//!
//! The pick plays (6..9) run the screens of [`super::picks`]: "set
//! pick" `FUN_00118508`, "run pick play" `FUN_00118f60` and "pick off
//! ball %d" `FUN_001199e0`; the screen itself, the contact a still
//! screener wins, is `crate::contact`'s.
//!

use super::moves::{CarrierMove, Leaper};
use super::offence::{
    CarrierPause, Drive, Iso, Pass, Shoot, busy, can_act, openness, outside, path_open, quality,
};
use super::picks::{PickOffBall, RunPick, SetPick};
use super::spots::{CutKind, CutMove, PerimPlayer, PostCut};
use super::words::words;
use super::{Act, Brain, Ctx, skill};

/// The plays by their slot in the playbook.
pub(super) mod kind {
    pub const NONE: i32 = -1;
    /// "post up" (vtable `0x43a428`).
    pub const POST_UP: i32 = 0;
    /// "back down" (`0x439638`).
    pub const BACK_DOWN: i32 = 1;
    /// "isolation" and "isolation 3pt" (`0x439968`, variant 0 and 1).
    pub const ISOLATION: i32 = 2;
    pub const ISOLATION_3PT: i32 = 3;
    /// "penetration" (`0x439c18`).
    pub const PENETRATION: i32 = 4;
    /// "cutter" (`0x4397b8`).
    pub const CUTTER: i32 = 5;
    /// "pick and roll" and "pick and roll 3pt" (`0x439dd0`).
    pub const PICK_AND_ROLL: i32 = 6;
    pub const PICK_AND_ROLL_3PT: i32 = 7;
    /// "pick off ball" and its 3pt variant (`0x439f80`).
    pub const PICK_OFF_BALL: i32 = 8;
    pub const PICK_OFF_BALL_3PT: i32 = 9;
    /// "pull up" (`0x43a5a0`).
    pub const PULL_UP: i32 = 10;
    /// "pen and kick" (`0x439b00`).
    pub const PEN_AND_KICK: i32 = 11;
    /// "rotate" (`0x43a718`).
    pub const ROTATE: i32 = 12;
    /// "post and kick" (`0x43a110`).
    pub const POST_AND_KICK: i32 = 13;
    /// "post cut" (`0x43a290`).
    pub const POST_CUT: i32 = 14;
    /// "alley oop" (`0x4394b0`).
    pub const ALLEY_OOP: i32 = 15;
}

/// The play's own words, by their offset in the original's object.
const V28: usize = 0;
const V2C: usize = 1;
const V30: usize = 2;
const V34: usize = 3;
const V38: usize = 4;
const V3C: usize = 5;
const V40: usize = 6;
const V44: usize = 7;
const V48: usize = 8;
const V4C: usize = 9;

words! {
    /// The running play (`AIPlaybook+0x8c`).
    pub(crate) struct Play {
        /// Which (see [`kind`]), or -1.
        kind: i32,
        /// +4: frames run.
        age: i32,
        /// +0x14..+0x20: the star, the handler, the post man and the
        /// perimeter man (players, or -1).
        star: i32,
        handler: i32,
        post: i32,
        perim: i32,
        /// +0x24.
        invalid: bool,
        /// +0x28..+0x4c.
        v: [i32; 10],
    }
}

fn who(v: i32) -> Option<usize> {
    usize::try_from(v).ok()
}

fn id(j: Option<usize>) -> i32 {
    j.map_or(-1, |j| j as i32)
}

/// `FUN_0017d550`: the play side's man holding the ball.
pub(super) fn holder(ctx: &Ctx, team: usize) -> Option<usize> {
    ctx.w.handler().filter(|&h| ctx.team(h) == team)
}

/// `FUN_0017d5d8`: the play side's man with the ball or a pass's
/// receiver.
fn owner(ctx: &Ctx, team: usize) -> Option<usize> {
    ctx.ball_owner().filter(|&h| ctx.team(h) == team)
}

fn put(ctx: &mut Ctx, brains: &mut [Brain], j: Option<usize>, act: Act) {
    if let Some(j) = j {
        brains[j].replace(ctx, j, act);
    }
}

/// "carrier pause" as the plays build it: `FUN_00101948(0, 9.0, ai, 0)`.
fn carrier_pause() -> Act {
    Act::CarrierPause(CarrierPause::new(0.0, 9.0, false))
}

fn perim_player() -> Act {
    Act::PerimPlayer(PerimPlayer::new(true))
}

/// "post up" (`FUN_00115368`, type 6).
fn post_up(ctx: &mut Ctx, _j: usize) -> Act {
    Act::PostUp(super::offence::PostUp::new(ctx))
}

/// `FUN_0011a580(handler, partner, zone, deep)`: the handler who feeds a
/// post man or an isolation partner.
fn feeder(ctx: &mut Ctx, h: usize, partner: usize, zone: i32, deep: bool) -> Act {
    Act::Feeder(super::offence::Feeder::new(ctx, h, partner, zone, deep))
}

fn cut_to(ctx: &mut Ctx, j: usize, zone: i32, long: bool, again: bool) -> Act {
    Act::CutMove(CutMove::new(ctx, j, CutKind::To, zone, long, again))
}

fn post_cut(ctx: &mut Ctx, j: usize, zone: i32, deep: bool, again: bool) -> Act {
    Act::PostCut(PostCut::new(ctx, j, zone, deep, again))
}

fn pass(ctx: &mut Ctx, from: usize, to: Option<usize>) -> Act {
    Act::Pass(Pass::new(ctx, from, to))
}

impl Play {
    pub fn none() -> Self {
        Self {
            kind: kind::NONE,
            star: -1,
            handler: -1,
            post: -1,
            perim: -1,
            ..Default::default()
        }
    }

    pub fn running(&self) -> bool {
        self.kind != kind::NONE
    }

    pub fn named(&self) -> [i32; 4] {
        let mut out = [self.star, self.handler, self.post, self.perim];
        // The words that name players (the cutter, the partners, the pass
        // targets) are checked where they are read.
        for v in &mut out {
            if *v < -1 {
                *v = -1;
            }
        }
        out
    }

    fn get(&self, k: usize) -> i32 {
        self.v[k]
    }

    fn set(&mut self, k: usize, x: i32) {
        self.v[k] = x;
    }

    /// A word that names a player, checked against the players (a
    /// snapshot's words are not validated one by one).
    fn man(&self, ctx: &Ctx, k: usize) -> Option<usize> {
        who(self.v[k]).filter(|&j| j < ctx.w.players.len())
    }

    fn variant(&self) -> bool {
        matches!(
            self.kind,
            kind::ISOLATION_3PT | kind::PICK_AND_ROLL_3PT | kind::PICK_OFF_BALL_3PT
        )
    }

    fn h(&self) -> Option<usize> {
        who(self.handler)
    }

    fn post_man(&self) -> Option<usize> {
        who(self.post)
    }

    fn perim_man(&self) -> Option<usize> {
        who(self.perim)
    }

    fn star_man(&self) -> Option<usize> {
        who(self.star)
    }

    /// `FUN_00128ae0`'s `FUN_0017d4d0(play, star)` and `FUN_00186868`:
    /// the header from the side's roles, then the play's start.
    pub fn begin(
        ctx: &mut Ctx,
        brains: &mut [Brain],
        team: usize,
        kind: i32,
        star: Option<usize>,
    ) -> Self {
        let roles: Vec<u8> = brains.iter().map(|b| b.role).collect();
        let handler = holder(ctx, team);
        let post = ctx.with_role(team, 2, &roles);
        let perim = ctx.with_role(team, 3, &roles);
        let mut p = Self {
            kind,
            age: 0,
            star: id(star),
            handler: id(handler),
            post: id(post),
            perim: id(perim),
            invalid: handler.is_none() || post.is_none() || perim.is_none(),
            v: [0; 10],
        };
        p.v[V28] = i32::from(p.variant());
        p.start(ctx, brains, team);
        p
    }

    /// `FUN_001868a0`: unless done, the update, and a frame older.
    pub fn run(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        if !self.done(ctx, brains, team) {
            self.update(ctx, brains, team);
            self.age += 1;
        }
    }

    /// vt+0x24.
    fn start(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        match self.kind {
            kind::POST_UP => self.start_post_up(ctx, brains, team),
            kind::BACK_DOWN => self.start_back_down(ctx, brains),
            kind::ISOLATION | kind::ISOLATION_3PT => self.start_isolation(ctx, brains, team),
            // `FUN_0011e528`.
            kind::PENETRATION => {
                if let Some(h) = self.h() {
                    let d = Drive::new(ctx, h);
                    brains[h].replace(ctx, h, Act::Drive(d));
                }
            }
            kind::CUTTER => self.start_cutter(ctx, brains),
            kind::PICK_AND_ROLL | kind::PICK_AND_ROLL_3PT => self.start_pick_and_roll(ctx, brains),
            kind::PICK_OFF_BALL | kind::PICK_OFF_BALL_3PT => self.start_pick_off_ball(ctx, brains),
            kind::PULL_UP => self.start_pull_up(ctx, brains, team),
            kind::PEN_AND_KICK => self.start_pen_and_kick(ctx, brains),
            kind::ROTATE => self.start_rotate(ctx, brains),
            kind::POST_AND_KICK => self.start_post_and_kick(ctx, brains, team),
            kind::POST_CUT => self.start_post_cut(ctx, brains),
            kind::ALLEY_OOP => self.start_alley_oop(ctx, brains, team),
            _ => {}
        }
    }

    /// vt+0x2c.
    fn update(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        match self.kind {
            kind::POST_UP => self.update_post_up(ctx, brains),
            kind::ISOLATION | kind::ISOLATION_3PT => self.update_isolation(ctx, brains, team),
            kind::CUTTER => self.update_cutter(ctx, brains),
            kind::PICK_AND_ROLL | kind::PICK_AND_ROLL_3PT => {
                self.update_pick_and_roll(ctx, brains, team);
            }
            kind::PICK_OFF_BALL | kind::PICK_OFF_BALL_3PT => {
                self.update_pick_off_ball(ctx, brains, team);
            }
            kind::PULL_UP => self.update_pull_up(ctx, brains, team),
            kind::PEN_AND_KICK => self.update_pen_and_kick(ctx, brains),
            kind::ROTATE => self.update_rotate(ctx, brains),
            kind::POST_AND_KICK => self.update_post_and_kick(ctx, brains, team),
            kind::POST_CUT => self.update_post_cut(ctx, brains),
            kind::ALLEY_OOP => self.update_alley_oop(ctx, brains, team),
            // Back down and penetration have none (`0x186ae8`).
            _ => {}
        }
    }

    /// vt+0x1c.
    pub fn done(&self, ctx: &Ctx, brains: &[Brain], team: usize) -> bool {
        if !self.running() {
            return true;
        }
        let age = self.age;
        match self.kind {
            // `FUN_001211f8`.
            kind::POST_UP => {
                if self.invalid || self.perim_man().is_some_and(|j| ctx.has_ball(j)) {
                    return true;
                }
                if id(owner(ctx, team)) != self.post && 210 < age {
                    return true;
                }
                420 < age
            }
            // `FUN_0011bc60`.
            kind::BACK_DOWN => {
                self.invalid || !self.h().is_some_and(|h| ctx.has_ball(h)) || 360 < age
            }
            kind::ISOLATION | kind::ISOLATION_3PT => self.done_isolation(ctx, brains, team),
            // `FUN_0011e4a8`.
            kind::PENETRATION => {
                !(holder(ctx, team).is_some()
                    && id(holder(ctx, team)) == self.handler
                    && age <= 360)
            }
            // `FUN_0011c230`.
            kind::CUTTER => {
                if self.invalid {
                    return true;
                }
                let h = self.h();
                if h.is_some_and(|h| ctx.has_ball(h) && !ctx.passing_now(h, &brains[h]))
                    && 180 < age
                {
                    return true;
                }
                if 390 < age {
                    return true;
                }
                match holder(ctx, team) {
                    None => false,
                    Some(o) => !(id(Some(o)) == self.handler || id(Some(o)) == self.get(V28)),
                }
            }
            // `FUN_0011edb0` (raw `0x11edb0..0x11ee3c`): `fps × 6`
            // frames, or the third man has the ball (not while a hand-off
            // is under way).
            kind::PICK_AND_ROLL | kind::PICK_AND_ROLL_3PT => {
                if self.invalid || 360 < age {
                    return true;
                }
                if self.get(V34) != -1 {
                    return false;
                }
                self.get(V30) != -1 && id(holder(ctx, team)) == self.get(V30)
            }
            // `FUN_0011f510` (raw `0x11f510..0x11f624`): the post man has
            // the ball, `fps × 6` frames (`fps × 4` before the cut), the
            // cutter has the ball, or he stands still in the zone he cut
            // to.
            kind::PICK_OFF_BALL | kind::PICK_OFF_BALL_3PT => {
                if self.invalid || self.post_man().is_some_and(|j| ctx.has_ball(j)) || 360 < age {
                    return true;
                }
                if self.get(V30) == 0 {
                    return 240 < age;
                }
                let Some(q) = self.perim_man() else {
                    return true;
                };
                ctx.has_ball(q) || (ctx.zone(q) == self.get(V38) && ctx.strength_class(q) == 0)
            }
            // `FUN_001218e8`.
            kind::PULL_UP => {
                if self.invalid {
                    return true;
                }
                if holder(ctx, team).is_none() {
                    return false;
                }
                if self.h().is_some_and(|h| ctx.has_ball(h)) {
                    return false;
                }
                !(self.get(V28) >= 0 && self.man(ctx, V28).is_some_and(|j| ctx.has_ball(j)))
            }
            // `FUN_0011da50`.
            kind::PEN_AND_KICK => {
                self.invalid || self.post_man().is_some_and(|j| ctx.has_ball(j)) || 354 < age
            }
            // `FUN_00122420`.
            kind::ROTATE => self.invalid || 480 < age,
            // `FUN_001200e0`.
            kind::POST_AND_KICK => {
                self.invalid || ctx.game.ball == 5 || 420 < age || self.get(V34) < self.get(V30)
            }
            // `FUN_00120bc0`.
            kind::POST_CUT => match holder(ctx, team) {
                Some(o) if id(Some(o)) != self.star => {
                    self.get(V2C) != 0 || id(Some(o)) != self.handler
                }
                _ => false,
            },
            // `FUN_0011b460`.
            kind::ALLEY_OOP => self.invalid || self.star < 0,
            _ => true,
        }
    }

    /// vt+0x34..+0x54: may the playbook's check `which` (0 pass, 1 shot,
    /// 2 penetrate, 3 post cut, 4 alley-oop) interrupt the play.
    pub fn allows(&self, ctx: &Ctx, team: usize, which: usize) -> bool {
        match self.kind {
            // `0x1223d0..0x1223f0`: nothing interrupts a pull up.
            kind::PULL_UP => false,
            // `0x11bab8`: no pass check during an alley-oop.
            kind::ALLEY_OOP => which != 0,
            kind::ISOLATION | kind::ISOLATION_3PT => {
                // `FUN_0011d6a8`: the plain isolation, or a 3pt one that
                // drives, always.
                let open = !self.variant() || self.get(V48) != 0;
                match which {
                    // `FUN_0011d6f0`.
                    1 => {
                        open || holder(ctx, team)
                            .is_some_and(|o| ctx.hoop_dist(o) < 14.0 || outside(ctx, o, 0.5))
                    }
                    // `FUN_0011d780`.
                    2 => open || holder(ctx, team).is_some_and(|o| ctx.hoop_dist(o) < 14.0),
                    4 => true,
                    _ => open,
                }
            }
            kind::ROTATE => match which {
                // `FUN_001225a0`: one of the two others stands open over 5.
                0 => holder(ctx, team).is_some_and(|o| {
                    (0..2).any(|k| ctx.other_of(o, k).is_some_and(|j| openness(ctx, j) > 5))
                }),
                // `FUN_00122630`: the holder's shot is 6 or better.
                1 => holder(ctx, team).is_some_and(|o| quality(ctx, o) >= 6),
                _ => true,
            },
            _ => true,
        }
    }
}

/// vt+0x5c: may the play run with a full meter.
pub(super) fn meter_ok(kind: i32) -> bool {
    matches!(
        kind,
        kind::ISOLATION_3PT
            | kind::PICK_AND_ROLL_3PT
            | kind::PICK_OFF_BALL_3PT
            | kind::PULL_UP
            | kind::PEN_AND_KICK
            | kind::ROTATE
            | kind::POST_AND_KICK
    )
}

/// vt+0x64: does the play suit player `j` (the holder, or the star).
pub(super) fn suits(ctx: &Ctx, team: usize, kind: i32, j: usize) -> bool {
    match kind {
        // `FUN_00121500`.
        kind::POST_UP => !ctx.has_ball(j) && can_post(ctx, team, j),
        // `FUN_0011bce0`.
        kind::BACK_DOWN => {
            if !ctx.has_ball(j) {
                return false;
            }
            let Some(d) = defender(ctx, j) else {
                return true;
            };
            let taller = ctx.height_ft(d) - ctx.height_ft(j);
            if 0.0 < taller {
                if 0.25 < taller {
                    return false;
                }
                // The original reads the player's rating at index
                // `(defender's power < 4)`: shots or threes.
                let k = usize::from(ctx.rating(d, 4) < 4);
                return ctx.rating(j, k) != 0;
            }
            true
        }
        // `FUN_0011d628`.
        kind::ISOLATION | kind::ISOLATION_3PT => {
            let three = kind == kind::ISOLATION_3PT;
            if !ctx.has_ball(j) && !partner_ok(ctx, team, three, j) {
                return false;
            }
            !three || ctx.rating(j, 1) > 0x2c
        }
        // `FUN_0011e578`.
        kind::PENETRATION => ctx.has_ball(j),
        // `FUN_0011c728`.
        kind::CUTTER => !ctx.has_ball(j),
        // `FUN_0011f210`.
        kind::PICK_AND_ROLL | kind::PICK_AND_ROLL_3PT => {
            let three = kind == kind::PICK_AND_ROLL_3PT;
            if ctx.has_ball(j) && screener_ok(ctx, three, j) {
                return true;
            }
            !three && -5.0 < ctx.up_court(j)
        }
        // `FUN_0011fd70`.
        kind::PICK_OFF_BALL => true,
        kind::PICK_OFF_BALL_3PT => ctx.rating(j, 1) > 0x31,
        // `FUN_0011e160`, `FUN_00120758`.
        kind::PEN_AND_KICK | kind::POST_AND_KICK => ctx.rating(j, 1) > 0x31,
        // `FUN_00122b20`.
        kind::ROTATE => !ctx.has_ball(j) && ctx.rating(j, 1) > 0x31,
        // `0x11b9b0`.
        _ => false,
    }
}

/// `FUN_00121398`: a post man for the post up: not deep in the back court
/// (or near the holder across), and not giving away more than a quarter
/// foot to his defender (nor any, unless stronger).
fn can_post(ctx: &Ctx, team: usize, j: usize) -> bool {
    if ctx.up_court(j) <= -6.0 {
        let Some(o) = holder(ctx, team) else {
            return false;
        };
        if 15.0 <= (ctx.pos(o)[0] - ctx.pos(j)[0]).abs() {
            return false;
        }
    }
    if let Some(d) = defender(ctx, j) {
        let taller = ctx.height_ft(d) - ctx.height_ft(j);
        if 0.0 < taller && (0.25 < taller || ctx.rating(j, 4) <= ctx.rating(d, 4)) {
            return false;
        }
    }
    true
}

/// `FUN_0011d380`: an isolation partner: Handles 46+ (Threes 45+ for the
/// 3pt one), and ahead of the holder by more than 15, or the holder
/// within 28.75 and the partner within 52 of the rim.
fn partner_ok(ctx: &Ctx, team: usize, three: bool, j: usize) -> bool {
    let Some(o) = holder(ctx, team) else {
        return false;
    };
    if three {
        if ctx.rating(j, 1) < 0x2d {
            return false;
        }
    } else if ctx.rating(j, 7) < 0x2e {
        return false;
    }
    let side = ctx.side(team) as f32;
    if side * ctx.pos(o)[0] - 15.0 < side * ctx.pos(j)[0] {
        return true;
    }
    ctx.hoop_dist(o) <= 28.75 && ctx.hoop_dist(j) <= 52.0
}

/// `FUN_0011e9e8`: a screener for the pick and roll: Handles over 49
/// (and Threes 50+ for the 3pt one).
fn screener_ok(ctx: &Ctx, three: bool, j: usize) -> bool {
    ctx.rating(j, 7) > 0x31 && (!three || ctx.rating(j, 1) >= 0x32)
}

impl Play {
    /// `FUN_00120ff8`.
    fn start_post_up(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        if self.invalid {
            return;
        }
        self.set(V28, 0);
        let r = ctx.rand();
        self.set(V30, ((r ^ 1) & 1) as i32);
        let Some(h) = self.h() else {
            return;
        };
        let post = match self.star_man() {
            Some(s) if s != h && can_post(ctx, team, s) => Some(s),
            _ => {
                let ok = |ctx: &Ctx, k: usize| {
                    ctx.other_of(h, k).is_some_and(|j| can_post(ctx, team, j))
                };
                let u = usize::from(ok(ctx, 0)) | usize::from(ok(ctx, 1)) << 1;
                if u == 0 {
                    self.invalid = true;
                    self.set(V30, 0);
                    return;
                }
                // `0x3cc8b8`: the post man by which are fit, a coin
                // between both.
                const PICK: [usize; 8] = [0, 0, 0, 0, 1, 1, 0, 1];
                let r = ctx.rand();
                ctx.other_of(h, PICK[u * 2 + (r & 1) as usize])
            }
        };
        let Some(post) = post else {
            self.invalid = true;
            return;
        };
        self.post = post as i32;
        let perim = ctx.third_of(post, h);
        self.perim = id(perim);
        let side = ctx.side(team) as f32;
        let hz = side * ctx.pos(h)[1];
        let pz = perim.map_or(0.0, |p| side * ctx.pos(p)[1]);
        self.set(V2C, if pz < hz { 6 } else { 5 });
        let f = feeder(ctx, h, post, self.get(V2C), self.get(V30) != 0);
        brains[h].replace(ctx, h, f);
        let c = post_cut(ctx, post, self.get(V2C), self.get(V30) != 0, true);
        brains[post].replace(ctx, post, c);
        put(ctx, brains, perim, perim_player());
    }

    /// `FUN_001212c8`: the post man posts up once he has the ball.
    fn update_post_up(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        let Some(p) = self.post_man() else {
            return;
        };
        if ctx.has_ball(p) && self.get(V28) == 0 {
            self.set(V28, 1);
            let a = post_up(ctx, p);
            brains[p].replace(ctx, p, a);
        }
    }

    /// `FUN_0011bb10`.
    fn start_back_down(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        if self.invalid {
            return;
        }
        let Some(h) = self.h() else {
            return;
        };
        brains[h].clear(ctx, h);
        if ctx.zone(h) == 2 || 28.75 < ctx.hoop_dist(h) {
            let zone = if ctx.across(h) < 0.0 { 1 } else { 3 };
            let m = CarrierMove::new(ctx, h, zone, false);
            brains[h].push(Act::CarrierMove(m));
        }
        let a = post_up(ctx, h);
        brains[h].push(a);
        put(ctx, brains, self.perim_man(), perim_player());
        put(ctx, brains, self.post_man(), perim_player());
    }

    /// `FUN_0011cad8`.
    fn start_isolation(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        if self.invalid {
            return;
        }
        let Some(h) = self.h() else {
            return;
        };
        let three = self.variant();
        for k in [V38, V44, V48, V3C] {
            self.set(k, 0);
        }
        let partner = match self.star_man() {
            Some(s) => s,
            None => {
                let ok = |ctx: &Ctx, k: usize| {
                    ctx.other_of(h, k)
                        .is_some_and(|j| partner_ok(ctx, team, three, j))
                };
                let u = usize::from(ok(ctx, 0)) | usize::from(ok(ctx, 1)) << 1;
                // Without a partner, or for the plain isolation one time
                // in eight, the handler goes alone.
                if u == 0 || (!three && ctx.rand() & 7 == 0) {
                    h
                } else {
                    // `0x3cb938`.
                    const PICK: [usize; 8] = [0, 0, 0, 0, 1, 1, 0, 1];
                    let r = ctx.rand();
                    ctx.other_of(h, PICK[u * 2 + (r & 1) as usize]).unwrap_or(h)
                }
            }
        };
        if partner != h {
            self.set(V3C, 1);
            self.set(V2C, partner as i32);
            let mut a = ctx.other_of(partner, 0);
            let mut b = ctx.other_of(partner, 1);
            if b == Some(h) {
                std::mem::swap(&mut a, &mut b);
            }
            self.set(V30, id(a));
            self.set(V34, id(b));
            let pz = ctx.across(h);
            let bz = b.map_or(0.0, |b| ctx.across(b));
            self.set(V40, if pz < bz { 1 } else { 3 });
            let f = feeder(ctx, h, partner, self.get(V40), false);
            brains[h].replace(ctx, h, f);
            self.set(V48, i32::from(!three));
            let c = cut_to(ctx, partner, self.get(V40), true, true);
            brains[partner].replace(ctx, partner, c);
            put(ctx, brains, b, perim_player());
            return;
        }
        self.set(V2C, h as i32);
        self.set(V40, if 0.0 < ctx.across(h) { 3 } else { 1 });
        let open = path_open(ctx, h);
        let go = !three || (open && ctx.hoop_dist(h) <= 22.0);
        if go {
            if ctx.zone(h) < 5 {
                if !open {
                    self.set(V48, i32::from(!three));
                    let m = CarrierMove::new(ctx, h, self.get(V40), true);
                    brains[h].replace(ctx, h, Act::CarrierMove(m));
                    // `FUN_0011d5a8`.
                    let reach = if three && self.get(V48) == 0 {
                        -3.0
                    } else {
                        3.0
                    };
                    brains[h].push(Act::CarrierPause(CarrierPause::new(reach, 9.0, true)));
                } else {
                    self.set(V48, 1);
                    let d = Drive::new(ctx, h);
                    brains[h].replace(ctx, h, Act::Drive(d));
                }
            } else {
                self.set(V48, 1);
                let a = post_up(ctx, h);
                brains[h].replace(ctx, h, a);
            }
        } else {
            self.set(V48, 0);
            brains[h].replace(ctx, h, Act::Iso(Iso::new()));
        }
        let a = ctx.other_of(h, 0);
        let b = ctx.other_of(h, 1);
        self.set(V30, id(a));
        self.set(V34, id(b));
        put(ctx, brains, a, perim_player());
        put(ctx, brains, b, perim_player());
    }

    /// `FUN_0011cf18`.
    fn update_isolation(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        let Some(p) = self.man(ctx, V2C) else {
            return;
        };
        if self.get(V38) == 0
            && holder(ctx, team).is_none()
            && (ctx.zone(p) == self.get(V40) || 4 < ctx.zone(p))
        {
            self.set(V38, 1);
            put(ctx, brains, self.man(ctx, V30), perim_player());
            put(ctx, brains, self.man(ctx, V34), perim_player());
        }
        if self.get(V2C) != self.handler && ctx.has_ball(p) && self.get(V44) == 0 {
            brains[p].replace(ctx, p, carrier_pause());
            self.set(V44, 1);
            return;
        }
        if brains[p].front_kind() != 3 {
            return;
        }
        self.set(V44, self.get(V44) + 1);
        let n = self.get(V44);
        if self.get(V48) == 0 && self.variant() {
            if 9 < n {
                let a = if !can_act(ctx, p) || 30.75 <= ctx.hoop_dist(p) || quality(ctx, p) < 6 {
                    Act::Iso(Iso::new())
                } else {
                    Act::Shoot(Shoot::new(ctx, p))
                };
                brains[p].replace(ctx, p, a);
            }
        } else if 60 < n || (9 < n && path_open(ctx, p)) {
            let d = Drive::new(ctx, p);
            brains[p].replace(ctx, p, Act::Drive(d));
        }
    }

    /// `FUN_0011d1d0`.
    fn done_isolation(&self, ctx: &Ctx, brains: &[Brain], team: usize) -> bool {
        if self.invalid || 96 < self.get(V44) {
            return true;
        }
        let limit = if self.variant() {
            (60.0 * skill::value(ctx, team, 3.5, 4.5, 5.0, 5.0)) as i32
        } else {
            360
        };
        if limit < self.age {
            return true;
        }
        let Some(o) = holder(ctx, team) else {
            return false;
        };
        brains[o].front_kind() == 0x11 && ctx.rating(o, 1) <= 0x2c
    }

    /// `FUN_0011c120`.
    fn start_cutter(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        if self.invalid {
            return;
        }
        let Some(h) = self.h() else {
            return;
        };
        for k in [V30, V34, V38, V3C] {
            self.set(k, 0);
        }
        if self.star < 0 || self.star == self.handler {
            self.star = self.perim;
        }
        self.set(V28, self.star);
        let zone = ctx.sector(h);
        let m = CarrierMove::new(ctx, h, zone, true);
        brains[h].replace(ctx, h, Act::CarrierMove(m));
        brains[h].push(carrier_pause());
        self.cutter_cut(ctx, brains, true);
        let third = self.man(ctx, V28).and_then(|c| ctx.third_of(h, c));
        self.set(V2C, id(third));
        put(ctx, brains, third, perim_player());
    }

    /// `FUN_0011c560` (`first`) and the update's re-cut: a cutter from 20
    /// out or farther cuts in by the net (`FUN_0011c5c0`: the zone by his
    /// and the handler's sectors, `0x3cb8d0`); closer, on the first call,
    /// he cuts to a spot by the handler's sector (`FUN_0011c658`).
    fn cutter_cut(&mut self, ctx: &mut Ctx, brains: &mut [Brain], first: bool) {
        let (Some(c), Some(h)) = (self.man(ctx, V28), self.h()) else {
            return;
        };
        if 20.0 <= ctx.hoop_dist(c) {
            const ZONES: [i32; 25] = [
                2, 3, 4, 2, 2, 3, 3, 4, 4, 3, 4, 4, 0, 0, 1, 1, 0, 0, 1, 1, 2, 2, 0, 1, 2,
            ];
            let row = ctx.sector(c).clamp(0, 4) as usize;
            let col = ctx.sector_ahead(h).clamp(0, 4) as usize;
            let a = Act::CutMove(CutMove::new(
                ctx,
                c,
                CutKind::NetIn,
                ZONES[row * 5 + col],
                false,
                true,
            ));
            brains[c].replace(ctx, c, a);
            self.set(V30, 3);
        } else if first {
            let s = ctx.sector_ahead(h);
            let zone = if s == 2 {
                if ctx.across(c) < 0.0 { 0 } else { 4 }
            } else if s < 2 {
                3
            } else {
                1
            };
            let a = cut_to(ctx, c, zone, true, true);
            brains[c].replace(ctx, c, a);
            self.set(V30, 0);
        }
    }

    /// `FUN_0011c300`.
    fn update_cutter(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        let (Some(c), Some(h)) = (self.man(ctx, V28), self.h()) else {
            return;
        };
        if ctx.has_ball(c) {
            if self.get(V3C) == 0 {
                self.set(V3C, 1);
                let d = Drive::new(ctx, c);
                brains[c].replace(ctx, c, Act::Drive(d));
            }
            return;
        }
        if !ctx.has_ball(h) {
            if self.get(V38) == 0 {
                self.set(V38, 1);
                put(ctx, brains, self.man(ctx, V2C), perim_player());
            }
            return;
        }
        if ctx.passing_now(h, &brains[h]) {
            return;
        }
        if self.get(V30) < 1 {
            if 20.0 <= ctx.hoop_dist(c) {
                self.cutter_cut(ctx, brains, false);
            }
        } else if ctx.hoop_dist(c) < 20.0 {
            // The cutter's facing (actor+0xa8, `FUN_0013aae8`; raw
            // `0x11c3a8`), not his stick, within 200 of the rim.
            let off = super::court::turn(ctx.facing(c), ctx.angle_to_hoop(c)).abs();
            if off < 200 {
                self.set(V30, self.get(V30) - 1);
                if self.get(V30) == 0 {
                    if can_pass(ctx, brains, h, 6) {
                        let a = pass(ctx, h, Some(c));
                        brains[h].replace(ctx, h, a);
                    } else {
                        self.set(V30, 1);
                    }
                }
            }
        }
        if self.age < 178 || !can_pass(ctx, brains, h, 6) {
            return;
        }
        let Some(s) = self.star_man().filter(|&s| s != h) else {
            return;
        };
        if 32.0 <= ctx.hoop_dist(s) {
            return;
        }
        let a = pass(ctx, h, Some(s));
        brains[h].replace(ctx, h, a);
    }

    /// `FUN_0011e900`: a handler who would not screen well (Handles 50+,
    /// and Threes 50+ for the 3pt one) passes to one who would, the star
    /// first; otherwise the screen is set (`FUN_0011eac0`).
    fn start_pick_and_roll(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        if self.invalid {
            return;
        }
        let Some(h) = self.h() else {
            return;
        };
        for k in [V38, V44, V48] {
            self.set(k, 0);
        }
        // +0x34, +0x30 and +0x2c are pointers, null at the start; -1 here
        // (player 0 is a player).
        for k in [V34, V30, V2C] {
            self.set(k, -1);
        }
        let three = self.variant();
        if screener_ok(ctx, three, h) {
            self.set_screen(ctx, brains);
            return;
        }
        if let Some(s) = self.star_man().filter(|&s| s != h)
            && screener_ok(ctx, three, s)
        {
            self.hand_to(ctx, brains, h, s);
            return;
        }
        for j in [self.perim_man(), self.post_man()].into_iter().flatten() {
            if screener_ok(ctx, three, j) {
                self.hand_to(ctx, brains, h, j);
                return;
            }
        }
        self.set_screen(ctx, brains);
    }

    /// `FUN_0011ea68`.
    fn hand_to(&mut self, ctx: &mut Ctx, brains: &mut [Brain], h: usize, j: usize) {
        let a = pass(ctx, h, Some(j));
        brains[h].replace(ctx, h, a);
        self.set(V34, j as i32);
    }

    /// `FUN_0011eac0`: a handler whose defender is beyond 20 has no screen
    /// to use, nor one whose teammates are both deep in the back court
    /// (`FUN_0011ee40`/`FUN_0011eea8`: `side × x` −5 or less); the play
    /// ends. Of the two teammates up the court the star screens, else the
    /// nearer to the handler (+0x2c); the other (+0x30) goes to the
    /// perimeter. Outside the paint the screener cuts to the zone beside
    /// the handler's (+0x3c, a corner taken as its wing) on the
    /// screener's side of him round the rim (+0x40); from the paint he
    /// posts up on the handler's side (wing 1 and block 5, or 3 and 6).
    /// The handler runs "run pick play" (`FUN_00118f60`); both pick flags
    /// (AI+0x5c) clear.
    fn set_screen(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        let Some(h) = self.h() else {
            return;
        };
        if defender(ctx, h).is_none_or(|d| 20.0 < ctx.between(h, d)) {
            self.invalid = true;
            return;
        }
        let team = ctx.team(h);
        let up = |ctx: &Ctx, k: usize| {
            holder(ctx, team).is_some_and(|o| {
                ctx.other_of(o, k)
                    .is_some_and(|j| -5.0 < ctx.side(team) as f32 * ctx.pos(j)[0])
            })
        };
        let which = u8::from(up(ctx, 0)) | (u8::from(up(ctx, 1)) << 1);
        if which == 0 {
            self.invalid = true;
            return;
        }
        // `FUN_0018abd0(team, handler, k)` through `FUN_0017d6b0`.
        let (Some(a), Some(b)) = (ctx.other_of(h, 0), ctx.other_of(h, 1)) else {
            self.invalid = true;
            return;
        };
        let screener = match which {
            3 if self.star == a as i32 || self.star == b as i32 => self.star as usize,
            3 if ctx.between(h, b) <= ctx.between(h, a) => b,
            1 | 3 => a,
            _ => b,
        };
        let third = if screener != a { a } else { b };
        self.set(V2C, screener as i32);
        self.set(V30, third as i32);
        if ctx.zone(screener) < 5 {
            let zone = match ctx.sector(h) {
                0 => 1,
                4 => 3,
                s => s,
            };
            self.set(V3C, zone);
            let to = if ctx.hoop_angle(screener) < ctx.hoop_angle(h) {
                zone - 1
            } else {
                zone + 1
            };
            self.set(V40, to);
            let c = cut_to(ctx, screener, to, true, false);
            brains[screener].replace(ctx, screener, c);
        } else {
            let (zone, to) = if (ctx.side(team) as f32) * ctx.pos(h)[1] < 0.0 {
                (1, 5)
            } else {
                (3, 6)
            };
            self.set(V3C, zone);
            self.set(V40, to);
            let c = post_cut(ctx, screener, to, true, true);
            brains[screener].replace(ctx, screener, c);
        }
        let run = RunPick::new(
            ctx,
            h,
            screener,
            self.get(V3C),
            self.variant(),
            self.star_man(),
        );
        brains[h].replace(ctx, h, Act::RunPick(run));
        brains[third].replace(ctx, third, perim_player());
        brains[h].pick_end = false;
        brains[screener].pick_end = false;
    }

    /// `FUN_0011ef18` (raw `0x11ef18..0x11f128`): while a hand-off is
    /// under way, once the receiver has the ball he is the handler and the
    /// screen is set (a pass to anyone else ends the play). Then either
    /// pick flag (AI+0x5c) up ends the play; until the pick is called,
    /// the frames each of the two has stood still (`FUN_0013abf8`) are
    /// counted, and once both stand still in their zones, or both have
    /// stood still over `fps × 0.4` frames, the screener sets the pick
    /// ("set pick", mode 0); a pick already in front counts too; then the
    /// third man goes to the perimeter.
    fn update_pick_and_roll(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        if self.get(V34) != -1 {
            let Some(o) = holder(ctx, team) else {
                return;
            };
            if o as i32 == self.get(V34) {
                self.set(V34, -1);
                self.handler = o as i32;
                self.set_screen(ctx, brains);
            } else if o as i32 != self.handler {
                self.invalid = true;
            }
            return;
        }
        let (Some(h), Some(s)) = (self.h(), self.man(ctx, V2C)) else {
            self.invalid = true;
            return;
        };
        if brains[h].pick_end || brains[s].pick_end {
            self.invalid = true;
            return;
        }
        if self.get(V38) != 0 {
            return;
        }
        let still = |ctx: &Ctx, j: usize, k: i32| {
            if ctx.strength_class(j) == 0 { k + 1 } else { 0 }
        };
        self.set(V44, still(ctx, h, self.get(V44)));
        self.set(V48, still(ctx, s, self.get(V48)));
        let placed = ctx.zone(h) == self.get(V3C)
            && ctx.strength_class(h) == 0
            && ctx.zone(s) == self.get(V40)
            && ctx.strength_class(s) == 0;
        // `fps × 0.4` (raw `0x11f040`).
        let waited = (60.0 * f32::from_bits(0x3ecc_cccd)) as i32;
        if placed || (waited < self.get(V44) && waited < self.get(V48)) {
            self.set(V38, 1);
            let p = SetPick::new(ctx, false);
            brains[s].replace(ctx, s, Act::SetPick(p));
        } else if brains[s].front_kind() == 1 {
            self.set(V38, 1);
        }
        if self.get(V38) != 0 {
            put(ctx, brains, self.man(ctx, V30), perim_player());
        }
    }

    /// `FUN_0011f628`: a play off a screen away from the ball. Either
    /// teammate more than 10 into the back court ends it. A handler who is
    /// not the star goes to the top himself (`FUN_0011f7c0`) from a wing,
    /// the top or the paint away from the baseline; from a corner or the
    /// baseline (and a star handler from anywhere) he hands the ball to a
    /// teammate fit to take it (`FUN_0011f850`), the perimeter man first,
    /// who cuts to the top (`FUN_0011f940`); with neither fit a star
    /// handler hands it to the more open (the perimeter man on a tie).
    fn start_pick_off_ball(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        if self.invalid {
            return;
        }
        for k in [V2C, V30, V34] {
            self.set(k, 0);
        }
        let (Some(h), Some(p), Some(q)) = (self.h(), self.post_man(), self.perim_man()) else {
            return;
        };
        let z = ctx.zone(h);
        if ctx.up_court(p) < -10.0 || ctx.up_court(q) < -10.0 {
            self.invalid = true;
            return;
        }
        // `FUN_0017e440(ai, 0)`: within 8 of the baseline.
        let baseline = 36.75 - ctx.pos(h)[0].abs() < 8.0;
        if self.star != self.handler && z != 4 && z != 0 && (z < 5 || !baseline) {
            self.go_to_top(ctx, brains);
            return;
        }
        if self.fit(ctx, q) {
            self.hand_off_ball(ctx, brains, q, p);
        } else if self.fit(ctx, p) {
            self.hand_off_ball(ctx, brains, p, q);
        } else if self.star != self.handler {
            self.go_to_top(ctx, brains);
        } else if openness(ctx, p) <= openness(ctx, q) {
            self.hand_off_ball(ctx, brains, q, p);
        } else {
            self.hand_off_ball(ctx, brains, p, q);
        }
    }

    /// `FUN_0011f7c0`: the handler to the top ("carrier move" zone 2,
    /// then "carrier pause" `(0, 9)`); with the star the post man, he and
    /// the perimeter man trade places.
    fn go_to_top(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        let Some(h) = self.h() else {
            return;
        };
        let m = CarrierMove::new(ctx, h, 2, true);
        brains[h].replace(ctx, h, Act::CarrierMove(m));
        brains[h].push(carrier_pause());
        if self.star == self.post {
            std::mem::swap(&mut self.post, &mut self.perim);
        }
    }

    /// `FUN_0011f850`: a teammate fit to take the ball: not the star, on
    /// a wing or the top, open 3 or more, and on a wing only on the
    /// handler's side of the court (by z).
    fn fit(&self, ctx: &Ctx, j: usize) -> bool {
        if j as i32 == self.star {
            return false;
        }
        let z = ctx.zone(j);
        if z == 0 || 3 < z || openness(ctx, j) < 3 {
            return false;
        }
        if z == 2 {
            return true;
        }
        let Some(h) = self.h() else {
            return false;
        };
        (0.0 <= ctx.pos(j)[1]) == (0.0 <= ctx.pos(h)[1])
    }

    /// `FUN_0011f940(to, other)`: the hand-off: pass to `to`, who cuts to
    /// the top ("cut to" zone 2).
    fn hand_off_ball(&mut self, ctx: &mut Ctx, brains: &mut [Brain], to: usize, other: usize) {
        let Some(h) = self.h() else {
            return;
        };
        self.perim = to as i32;
        self.post = other as i32;
        self.set(V34, 1);
        let a = pass(ctx, h, Some(to));
        brains[h].replace(ctx, h, a);
        let c = cut_to(ctx, to, 2, false, false);
        brains[to].replace(ctx, to, c);
    }

    /// `FUN_0011f9c0`: after a hand-off, once the new handler has it, he
    /// goes to the top. Once the handler stands at the top, the perimeter
    /// man cuts to his wing (+0x38) and the post man to his block
    /// (+0x3c). Once the perimeter man is out of the paint and the post
    /// man in it, the perimeter man cuts in by the net (to a corner or a
    /// wing, one draw) and the post man screens his defender ("pick off
    /// ball"). Then the handler passes to the cutter, if able, once the
    /// cutter and where he is headed are both beyond 11 of the rim and he
    /// will be there within 55 frames at his speed.
    fn update_pick_off_ball(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        let (Some(h), Some(p), Some(q)) = (self.h(), self.post_man(), self.perim_man()) else {
            return;
        };
        if self.get(V34) != 0 {
            if !ctx.has_ball(q) {
                return;
            }
            self.set(V34, 0);
            self.perim = h as i32;
            self.handler = q as i32;
            let m = CarrierMove::new(ctx, q, 2, true);
            brains[q].replace(ctx, q, Act::CarrierMove(m));
            brains[q].push(carrier_pause());
            return;
        }
        let side = ctx.side(team) as f32;
        if self.get(V2C) == 0 {
            if ctx.zone(h) != 2 {
                return;
            }
            self.set(V2C, 1);
            let mut wing = ctx.sector(q);
            if wing == 2 {
                wing = if side * ctx.pos(q)[1] < 0.0 { 1 } else { 3 };
            }
            self.set(V38, wing);
            let c = cut_to(ctx, q, wing, false, true);
            brains[q].replace(ctx, q, c);
            let low = if side * ctx.pos(p)[1] < 0.0 { 5 } else { 6 };
            self.set(V3C, low);
            let c = post_cut(ctx, p, low, true, true);
            brains[p].replace(ctx, p, c);
            return;
        }
        if self.get(V30) != 0 {
            if !can_pass(ctx, brains, h, 6) {
                return;
            }
            let rim = ctx.goal(team);
            let ahead = predicted(ctx, q);
            let headed = ctx.heading_for(q);
            if super::court::dist(rim, ahead) <= 11.0 || super::court::dist(rim, headed) <= 11.0 {
                return;
            }
            // The speed a frame (actor+0x40, the length of the velocity).
            let v = ctx.w.players[q].velocity();
            let speed = crate::math::len(v[0], v[1]) * crate::TICK_SECONDS;
            if speed * 55.0 <= super::court::dist(ctx.pos(q), headed) {
                return;
            }
            let a = pass(ctx, h, Some(q));
            brains[h].replace(ctx, h, a);
            return;
        }
        if 4 < ctx.zone(q) || ctx.zone(p) < 5 {
            return;
        }
        self.set(V30, 1);
        let mut zone = if 1 < ctx.sector(q) { 0 } else { 3 };
        let r = ctx.rand();
        zone += (r & 1) as i32;
        self.set(V38, zone);
        let c = Act::CutMove(CutMove::new(
            ctx,
            q,
            CutKind::NetIn,
            zone,
            self.variant(),
            false,
        ));
        brains[q].replace(ctx, q, c);
        let s = PickOffBall::new(ctx, Some(q));
        brains[p].replace(ctx, p, Act::PickOffBall(s));
    }

    /// `FUN_00121870`.
    fn start_pull_up(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        if self.invalid {
            return;
        }
        for k in [V2C, V28, V30, V34] {
            self.set(k, 0);
        }
        self.set(V28, -1);
        let Some(h) = self.h() else {
            return;
        };
        if self.star == self.handler || ctx.zone(h) < 5 || !self.kick_to_shooter(ctx, brains, team)
        {
            self.pull_up(ctx, brains, team);
        }
    }

    /// `FUN_00121c78`: a long shot from an isolation ("iso3pt"), or, for
    /// a handler under 45 Threes (or when a gamebreaker inside basket
    /// would end the game) with Dunks over 49, a drive; unless he is
    /// already at it, shooting or passing.
    fn pull_up(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        let Some(h) = self.h() else {
            return;
        };
        let k = brains[h].front_kind();
        self.set(V34, 0);
        let mut drive = false;
        if (ctx.rating(h, 1) < 0x2d || super::playbook::basket_ends_game(ctx, ctx.team(h), true))
            && ctx.rating(h, 2) > 0x31
        {
            drive = true;
            self.set(V34, 1);
        }
        let (a, b) = if drive { (5, 6) } else { (0x11, 0x11) };
        if k == 3 || (k != a && k != b && k != 9 && k != 8) {
            let act = if drive {
                Act::Drive(Drive::new(ctx, h))
            } else {
                Act::Iso(Iso::new())
            };
            brains[h].replace(ctx, h, act);
        }
        let _ = team;
    }

    /// `FUN_00121e78`: kick to the star (Threes 50+, or anyone with a full
    /// meter) if he is not the handler; else to the better long shooter
    /// of the other two who stands open (3+, then any), in the front court
    /// and in a zone of the arc (`FUN_00121dc8`). True when a pass went.
    fn kick_to_shooter(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) -> bool {
        let Some(h) = self.h() else {
            return false;
        };
        if let Some(s) = self.star_man()
            && (ctx.meter_full(team) || 0x31 < ctx.rating(s, 1))
        {
            if s == h {
                return false;
            }
            self.kick(ctx, brains, h, s);
            return true;
        }
        let (Some(post), Some(perim)) = (self.post_man(), self.perim_man()) else {
            return false;
        };
        let (a, b) = if ctx.rating(perim, 1) < ctx.rating(post, 1) {
            (post, perim)
        } else {
            (perim, post)
        };
        let open = |ctx: &Ctx, j: usize, min: i32| {
            min <= openness(ctx, j) && ctx.zone(j) < 5 && ctx.in_court(j) && 0.0 <= ctx.up_court(j)
        };
        for (j, min) in [(a, 3), (b, 3), (a, 0), (b, 0)] {
            if open(ctx, j, min) {
                self.kick(ctx, brains, h, j);
                return true;
            }
        }
        false
    }

    /// `FUN_00122000`.
    fn kick(&mut self, ctx: &mut Ctx, brains: &mut [Brain], h: usize, j: usize) {
        self.set(V28, j as i32);
        self.set(V2C, 1);
        let a = pass(ctx, h, Some(j));
        brains[h].replace(ctx, h, a);
    }

    /// `FUN_00121960`.
    fn update_pull_up(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        let Some(o) = holder(ctx, team) else {
            self.set(V30, 0);
            return;
        };
        self.set(V30, self.get(V30) + 1);
        if o as i32 != self.handler {
            let roles: Vec<u8> = brains.iter().map(|b| b.role).collect();
            self.handler = o as i32;
            self.post = id(ctx.with_role(team, 2, &roles));
            self.perim = id(ctx.with_role(team, 3, &roles));
        }
        if busy(ctx, o) || ctx.passing_now(o, &brains[o]) {
            return;
        }
        if self.get(V2C) != 0 {
            self.set(V2C, 0);
            self.set(V28, -1);
            self.pull_up(ctx, brains, team);
            return;
        }
        if self.get(V34) == 0 && 4 < ctx.zone_ahead(o) {
            if self.star != self.handler {
                self.kick_to_shooter(ctx, brains, team);
                return;
            }
            self.pull_up(ctx, brains, team);
            return;
        }
        if self.patience(ctx, team) < self.get(V30) && 5 < ctx.shot_clock() {
            self.star = -1;
            if !self.kick_to_shooter(ctx, brains, team) {
                self.pull_up(ctx, brains, team);
            }
            return;
        }
        if self.get(V34) == 0 && super::court::dist(ctx.pos(o), ctx.heading_for(o)) < 4.0 {
            self.pull_up(ctx, brains, team);
        }
    }

    /// `FUN_00121af8`: how long the pull up waits before looking to kick:
    /// `fps × 4`, less up to 1.5 s for Threes under 50 and up to 1 s for
    /// Handles under 50; a driver `fps × 24`.
    fn patience(&self, ctx: &Ctx, team: usize) -> i32 {
        if self.get(V34) != 0 {
            return 1440;
        }
        let mut t = 240;
        if let Some(o) = holder(ctx, team) {
            let threes = ctx.rating(o, 1);
            let handles = ctx.rating(o, 7);
            if threes < 0x32 {
                t -= (60.0 * ((50.0 - threes as f32) * 1.5 / 50.0)) as i32;
            }
            if handles < 0x32 {
                t -= (60.0 * ((50.0 - handles as f32) / 50.0)) as i32;
            }
        }
        t
    }

    /// `FUN_0011dad8`: a handler with Handles under 40 (or far out across)
    /// first hands the ball to a better ballhandler (`FUN_0011dc38`).
    fn start_pen_and_kick(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        if self.invalid {
            return;
        }
        let Some(h) = self.h() else {
            return;
        };
        for k in [V28, V2C, V30] {
            self.set(k, 0);
        }
        if self.handler != self.star && ctx.pos(h)[0].abs() <= 32.0 && 0x27 < ctx.rating(h, 7) {
            self.pen_setup(ctx, brains);
            return;
        }
        let better = |ctx: &Ctx, j: Option<usize>| {
            j.is_some_and(|j| j as i32 != self.star && ctx.rating(h, 7) < ctx.rating(j, 7))
        };
        let (post, perim) = (self.post_man(), self.perim_man());
        if better(ctx, perim) {
            self.hand_off(ctx, brains, perim, post);
        } else if better(ctx, post) {
            self.hand_off(ctx, brains, post, perim);
        } else {
            self.pen_setup(ctx, brains);
        }
    }

    /// `FUN_0011dc38`.
    fn hand_off(
        &mut self,
        ctx: &mut Ctx,
        brains: &mut [Brain],
        to: Option<usize>,
        other: Option<usize>,
    ) {
        let (Some(h), Some(to)) = (self.h(), to) else {
            return;
        };
        self.set(V28, 1);
        let a = pass(ctx, h, Some(to));
        brains[h].replace(ctx, h, a);
        let c = cut_to(ctx, to, 2, true, false);
        brains[to].replace(ctx, to, c);
        self.post = id(other);
        self.perim = to as i32;
    }

    /// `FUN_0011dfc0`: the handler to the top ("carrier move" zone 2, then
    /// "carrier pause"); of the other two the better long shooter (the
    /// star, else Threes, swapped when the first is under 50 and below
    /// the second) cuts to the corner on his side (0 or 4) and the other
    /// to the post on that side (6 or 5).
    fn pen_setup(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        let Some(h) = self.h() else {
            return;
        };
        let m = CarrierMove::new(ctx, h, 2, true);
        brains[h].replace(ctx, h, Act::CarrierMove(m));
        brains[h].push(carrier_pause());
        if self.star == self.post {
            self.post = self.perim;
            self.perim = self.star;
        } else if let (Some(p), Some(q)) = (self.post_man(), self.perim_man())
            && ctx.rating(q, 1) < 0x32
            && ctx.rating(q, 1) < ctx.rating(p, 1)
        {
            std::mem::swap(&mut self.post, &mut self.perim);
        }
        let (Some(shooter), Some(big)) = (self.post_man(), self.perim_man()) else {
            return;
        };
        let (low, corner) = if ctx.across(shooter) < 0.0 {
            (6, 0)
        } else {
            (5, 4)
        };
        self.set(V34, low);
        let c = cut_to(ctx, shooter, corner, false, true);
        brains[shooter].replace(ctx, shooter, c);
        let p = post_cut(ctx, big, low, false, true);
        brains[big].replace(ctx, big, p);
    }

    /// `FUN_0011dce0`.
    fn update_pen_and_kick(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        let (Some(h), Some(big)) = (self.h(), self.perim_man()) else {
            return;
        };
        if self.get(V28) != 0 {
            // The hand-off: once the new handler has it, set up.
            if !ctx.has_ball(big) {
                return;
            }
            self.set(V28, 0);
            std::mem::swap(&mut self.handler, &mut self.perim);
            self.pen_setup(ctx, brains);
            return;
        }
        if self.get(V2C) == 0 {
            // Once the handler waits at the top (or stands in zone 2
            // beyond the arc) with the big on his block, the handler
            // drives and the big steps out to the wing.
            if brains[h].front_kind() != 3 && (ctx.zone(h) != 2 || !outside(ctx, h, 0.5)) {
                return;
            }
            if ctx.zone(big) != self.get(V34) && brains[big].front_kind() != 7 {
                return;
            }
            self.set(V2C, 1);
            // `FUN_001095d8(ai, 1)`: the patient drive.
            let d = Drive::patient(ctx, h);
            brains[h].replace(ctx, h, Act::Drive(d));
            let r = ctx.rand();
            let wing = if self.get(V34) == 5 {
                (r & 1) as i32
            } else if r & 1 != 0 {
                3
            } else {
                4
            };
            self.set(V34, wing);
            let c = cut_to(ctx, big, wing, false, false);
            brains[big].replace(ctx, big, c);
            return;
        }
        if self.get(V30) != 0 {
            if !ctx.has_ball(h) {
                self.invalid = true;
            }
            return;
        }
        if can_pass(ctx, brains, h, 7) {
            let free = ctx
                .nth_opponent(big, 0, true)
                .is_none_or(|o| 14.0 < ctx.between(big, o))
                || (4 <= openness(ctx, big)
                    && ctx.hoop_dist(h) < 18.75
                    && super::court::dist(ctx.pos(big), ctx.heading_for(big)) < 13.0);
            if free {
                self.set(V30, 1);
                let a = pass(ctx, h, Some(big));
                brains[h].replace(ctx, h, a);
                brains[h].push(perim_player());
            }
        }
        if self.get(V30) == 0 && brains[h].front_kind() != 5 {
            self.invalid = true;
        }
    }

    /// `FUN_00122490`.
    fn start_rotate(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        if self.invalid {
            return;
        }
        for k in [V28, V2C, V30, V34, V38, V3C, V40] {
            self.set(k, 0);
        }
        if self.star >= 0 && self.star == self.post {
            self.post = self.perim;
            self.perim = self.star;
        }
        let Some(h) = self.h() else {
            return;
        };
        let s = if 0.0 <= ctx.pos(h)[1] { 1 } else { -1 };
        self.set(V44, if ctx.side(ctx.team(h)) * s < 0 { 0 } else { 4 });
        let m = CarrierMove::new(ctx, h, self.get(V44), false);
        brains[h].replace(ctx, h, Act::CarrierMove(m));
        brains[h].push(carrier_pause());
    }

    /// `FUN_00122670`.
    fn update_rotate(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        let (Some(h), Some(p), Some(q)) = (self.h(), self.post_man(), self.perim_man()) else {
            return;
        };
        let near = |ctx: &Ctx| super::court::dist(ctx.pos(h), ctx.heading_for(h));
        let side = self.get(V44) == 0;
        if self.get(V28) == 0 {
            if 25.0 <= near(ctx) {
                return;
            }
            self.set(V28, 1);
            let c = post_cut(ctx, p, if side { 6 } else { 5 }, false, false);
            brains[p].replace(ctx, p, c);
            return;
        }
        if self.get(V2C) == 0 {
            if 10.0 <= near(ctx) {
                return;
            }
            self.set(V2C, 1);
            let c = cut_to(ctx, p, 2, true, false);
            brains[p].replace(ctx, p, c);
            let c = post_cut(ctx, q, if side { 6 } else { 5 }, true, false);
            brains[q].replace(ctx, q, c);
            return;
        }
        if self.get(V30) == 0 {
            if ctx.zone(p) != 2 {
                return;
            }
            self.set(V30, 1);
            let open = openness(ctx, p);
            if open < 2 || (open < 3 && ctx.strength_class(p) == 0) {
                let c = post_cut(ctx, p, if side { 5 } else { 6 }, true, false);
                brains[p].replace(ctx, p, c);
                let m = CarrierMove::new(ctx, h, if side { 1 } else { 3 }, false);
                brains[h].replace(ctx, h, Act::CarrierMove(m));
                brains[h].push(carrier_pause());
                self.set(V38, 1);
            } else {
                let a = pass(ctx, h, Some(p));
                brains[h].replace(ctx, h, a);
            }
            let c = cut_to(ctx, q, if side { 4 } else { 0 }, true, false);
            brains[q].replace(ctx, q, c);
            return;
        }
        if self.get(V38) == 0 {
            if self.get(V40) < 1 {
                if self.get(V34) == 0 {
                    let corner = if side { 4 } else { 0 };
                    let z = ctx.zone_ahead(q);
                    if z != corner {
                        self.invalid = true;
                        return;
                    }
                    if !ctx.has_ball(p) || ctx.zone(q) != z {
                        return;
                    }
                    self.set(V34, 1);
                    if openness(ctx, q) < 3 {
                        self.invalid = true;
                        return;
                    }
                    let a = pass(ctx, p, Some(q));
                    brains[p].replace(ctx, p, a);
                } else {
                    if !ctx.has_ball(q) || !can_act(ctx, q) {
                        return;
                    }
                    if quality(ctx, q) < 5 {
                        self.invalid = true;
                        return;
                    }
                    let s = Shoot::new(ctx, q);
                    brains[q].replace(ctx, q, Act::Shoot(s));
                }
                return;
            }
            self.set(V40, self.get(V40) - 1);
            if self.get(V40) != 0 {
                return;
            }
            self.invalid = true;
            return;
        }
        self.set(V3C, self.get(V3C) + 1);
        if ctx.strength_class(p) == 0 {
            self.invalid = true;
            return;
        }
        if !can_pass(ctx, brains, h, 6) {
            return;
        }
        if openness(ctx, p) < 4 && (self.get(V3C) <= 12 || openness(ctx, p) < 3) {
            return;
        }
        self.set(V38, 0);
        let a = pass(ctx, h, Some(p));
        brains[h].replace(ctx, h, a);
        self.set(V40, 45);
    }

    /// `FUN_00120188`.
    fn start_post_and_kick(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        if self.invalid {
            return;
        }
        if self.post == self.star {
            std::mem::swap(&mut self.post, &mut self.perim);
        }
        self.set(V28, 0);
        self.set(V2C, 1);
        self.set(V30, 0);
        self.set(V40, 0);
        let f = skill::value(ctx, team, 1.5, 1.0, 1.0, 0.5);
        self.set(V38, 0);
        self.set(V3C, 0);
        self.set(V34, (60.0 * f) as i32 + 18);
        let (Some(h), Some(p), Some(q)) = (self.h(), self.post_man(), self.perim_man()) else {
            return;
        };
        let side = ctx.side(team) as f32;
        let hz = side * ctx.pos(h)[1];
        let qz = side * ctx.pos(q)[1];
        if hz < 0.0 {
            self.set(V44, 1);
            self.set(V48, 5);
            self.set(V4C, if hz < qz { 2 } else { 0 });
        } else {
            self.set(V48, 6);
            self.set(V44, 3);
            self.set(V4C, if hz < qz { 4 } else { 2 });
        }
        let m = CarrierMove::new(ctx, h, self.get(V44), true);
        brains[h].replace(ctx, h, Act::CarrierMove(m));
        brains[h].push(carrier_pause());
        let c = post_cut(ctx, p, self.get(V48), true, true);
        brains[p].replace(ctx, p, c);
        let c = cut_to(ctx, q, self.get(V4C), true, true);
        brains[q].replace(ctx, q, c);
    }

    /// `FUN_001203f0`.
    fn update_post_and_kick(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        let (Some(h), Some(p), Some(q)) = (self.h(), self.post_man(), self.perim_man()) else {
            return;
        };
        if self.get(V28) == 0 {
            // The entry: once the post man has stood on his block for
            // `fps × 0.2` frames with the handler waiting, feed him.
            if ctx.strength_class(p) != 0
                || brains[h].front_kind() != 3
                || ctx.zone(p) != self.get(V48)
            {
                self.set(V40, 0);
                return;
            }
            if self.get(V40) < 12 {
                self.set(V40, self.get(V40) + 1);
                return;
            }
            if double_teamed(ctx, p, 5.0) {
                self.invalid = true;
                return;
            }
            self.set(V28, 1);
            let a = pass(ctx, h, Some(p));
            brains[h].replace(ctx, h, a);
            let r = ctx.rand();
            if r % 100 < 0x4b {
                let z = self.get(V44);
                self.set(V44, if z <= self.get(V4C) { z - 1 } else { z + 1 });
            }
            let c = Act::CutMove(CutMove::new(
                ctx,
                h,
                CutKind::Out,
                self.get(V44),
                true,
                false,
            ));
            brains[h].push(c);
            return;
        }
        if self.get(V2C) == 0 {
            // After the kick: the receiver shoots a good look.
            if self.get(V38) != 0 || self.get(V3C) == 0 {
                return;
            }
            let Some(o) = holder(ctx, team).filter(|&o| o != p) else {
                return;
            };
            if !can_act(ctx, o) {
                return;
            }
            self.set(V3C, 0);
            if quality(ctx, o) < 5 {
                self.invalid = true;
                return;
            }
            let s = Shoot::new(ctx, o);
            brains[o].replace(ctx, o, Act::Shoot(s));
            return;
        }
        if !ctx.has_ball(p) {
            return;
        }
        self.set(V30, self.get(V30) + 1);
        if ctx.catching(p) {
            return;
        }
        if path_open(ctx, p) {
            let d = Drive::new(ctx, p);
            brains[p].replace(ctx, p, Act::Drive(d));
            self.set(V2C, 0);
            self.set(V38, 1);
            return;
        }
        if !can_pass(ctx, brains, p, 6) {
            return;
        }
        if self.get(V30) == self.get(V34) {
            let Some(s) = self.star_man().filter(|&s| s != p) else {
                return;
            };
            self.set(V2C, 0);
            self.set(V3C, 1);
            let a = pass(ctx, p, Some(s));
            brains[p].replace(ctx, p, a);
            return;
        }
        if !double_teamed(ctx, p, 5.0) && openness(ctx, q) < 6 && openness(ctx, h) < 6 {
            return;
        }
        self.set(V2C, 0);
        self.set(V3C, 1);
        let to = if openness(ctx, q) < openness(ctx, h) {
            h
        } else {
            q
        };
        let a = pass(ctx, p, Some(to));
        brains[p].replace(ctx, p, a);
    }

    /// `FUN_00120ab8`.
    fn start_post_cut(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        if self.invalid {
            return;
        }
        self.set(V2C, 0);
        let (Some(h), Some(s)) = (self.h(), self.star_man()) else {
            self.invalid = true;
            return;
        };
        if h == s {
            self.invalid = true;
            return;
        }
        self.set(V28, 0);
        brains[h].replace(ctx, h, carrier_pause());
        let zone = if ctx.across(s) < 0.0 { 5 } else { 6 };
        let c = post_cut(ctx, s, zone, true, true);
        brains[s].replace(ctx, s, c);
    }

    /// `FUN_00120c20`: on the second frame the handler passes to the
    /// cutter, who drives once he has it.
    fn update_post_cut(&mut self, ctx: &mut Ctx, brains: &mut [Brain]) {
        let (Some(h), Some(s)) = (self.h(), self.star_man()) else {
            return;
        };
        if self.age == 2 {
            let a = pass(ctx, h, Some(s));
            brains[h].replace(ctx, h, a);
        } else if self.get(V28) == 0 && ctx.has_ball(s) {
            self.set(V28, 1);
            let d = Drive::new(ctx, s);
            brains[s].replace(ctx, s, Act::Drive(d));
        }
        self.set(V2C, i32::from(ctx.w.handler().is_none()));
    }

    /// `FUN_0011b488`: the star leaps; the lob goes once he is `9` up
    /// (`skill(10, 10, 0, 0)`% of the time), else 6 (5 when the ball's
    /// owner is beyond 22).
    fn start_alley_oop(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        if self.invalid {
            return;
        }
        self.set(V28, 0);
        let Some(s) = self.star_man() else {
            return;
        };
        brains[s].replace(ctx, s, Act::Leaper(Leaper::new(false)));
        let p = skill::percent(ctx, team, 10, 10, 0, 0);
        let r = ctx.rand();
        let high = ((r % 100) as i32) < p;
        let mut at = if high { 9.0f32 } else { 6.0 };
        if !high && owner(ctx, team).is_some_and(|o| 22.0 < ctx.hoop_dist(o)) {
            at -= 1.0;
        }
        self.set(V2C, at.to_bits() as i32);
    }

    /// `FUN_0011b590`: while the star leaps (or a pass is in the air, or
    /// the handler dunks), once he is high enough the handler lobs to him
    /// (from beyond 23.75 one time in ten the play just ends; a handler who
    /// cannot pass now gives up 90% of the time), off the board when the
    /// trick pass's choice says so (`FUN_00182d10(ai, kind, 1)`, type
    /// 0x17), else a plain pass.
    fn update_alley_oop(&mut self, ctx: &mut Ctx, brains: &mut [Brain], team: usize) {
        let (Some(s), Some(h)) = (self.star_man(), self.h()) else {
            self.invalid = true;
            return;
        };
        let k = brains[s].front_kind();
        let going = k == 4
            || k == 8
            || k == 0x17
            || ctx.game.ball == 4
            || holder(ctx, team).is_some_and(|o| ctx.leaping(o));
        if !going {
            self.invalid = true;
            return;
        }
        if self.get(V28) != 0 || !ctx.has_ball(h) {
            return;
        }
        let at = f32::from_bits(self.get(V2C) as u32);
        if ctx.w.players[s].height <= at {
            return;
        }
        self.set(V28, 1);
        if 23.75 < ctx.hoop_dist(h) && ctx.rand() % 100 < 10 {
            self.invalid = true;
            return;
        }
        // `FUN_00181b58` (`FUN_0015f898`): the handler's clip lets him pass
        // unless he is down (the channel's 0x40, `FUN_0016ea70`) or on no
        // clip (its list `0x43f098` holds only -1); a trick does not bar it
        // (the pass press is taken in any move, `World::passes_out`).
        let ready = !ctx.fallen(h);
        if !ready && ctx.rand() % 100 < 0x5a {
            self.invalid = true;
            return;
        }
        let a = if super::offence::trick_pass_out(ctx, &brains[h], h, true).is_some() {
            Act::TrickPass(super::offence::TrickPass::new(ctx, s, false))
        } else {
            pass(ctx, h, Some(s))
        };
        brains[h].replace(ctx, h, a);
    }
}

/// `FUN_00182c58(ai, q)`: the holder can pass now: holding, not juking,
/// passing or dunking, and beyond 38.75 or with a shot under quality `q`.
pub(super) fn can_pass(ctx: &Ctx, brains: &[Brain], h: usize, q: i32) -> bool {
    can_pass_now(ctx, &brains[h], h, q)
}

/// [`can_pass`] for a handler whose brain is `b`.
pub(super) fn can_pass_now(ctx: &Ctx, b: &Brain, h: usize, q: i32) -> bool {
    ctx.has_ball(h)
        && !super::offence::juking(ctx, b, h)
        && !ctx.passing_now(h, b)
        && !ctx.leaping(h)
        && (38.75 < ctx.hoop_dist(h) || quality(ctx, h) < q)
}

/// `FUN_0013b900`: where a player will be: in the air, his landing (the
/// rewrite reads his place: the sim's jumps keep no frames left); on the
/// floor, where his front behaviour takes him (`FUN_00133380`, read as
/// [`Ctx::heading_for`]).
fn predicted(ctx: &Ctx, j: usize) -> [f32; 2] {
    if ctx.airborne(j) {
        ctx.pos(j)
    } else {
        ctx.heading_for(j)
    }
}

/// `FUN_00181420(r, ai)`: the two nearest opponents both within `r`, not
/// both 4 or more farther from the rim than the player.
pub(super) fn double_teamed(ctx: &Ctx, i: usize, r: f32) -> bool {
    let (Some(a), Some(b)) = (ctx.nth_opponent(i, 0, false), ctx.nth_opponent(i, 1, false)) else {
        return false;
    };
    if r < ctx.between(i, a) || r < ctx.between(i, b) {
        return false;
    }
    let d = ctx.hoop_dist(i);
    !(d + 4.0 <= ctx.hoop_dist(a) && d + 4.0 <= ctx.hoop_dist(b))
}

/// `FUN_0017f238`: the defender on a player: the one whose second man
/// (the defense strategy's `[0x0c..]`) he is, else the one in his slot.
pub(super) fn defender(ctx: &Ctx, j: usize) -> Option<usize> {
    let them = ctx.squad(1 - ctx.team(j));
    them.iter()
        .copied()
        .find(|&d| ctx.matchup(d, 2) == Some(j))
        .or_else(|| them.get(ctx.slot(j).min(2)).copied())
}
