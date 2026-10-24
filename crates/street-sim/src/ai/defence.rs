//! "defense" (type 10; `FUN_001062b8`, update `FUN_00106490`): guard a
//! man (the team's matchup, `FUN_00123d40`), block his shots
//! (`FUN_00106a88`), counter his tricks (`FUN_00107710`), reach for the
//! ball (`FUN_00107388`), and otherwise stand between him and the basket
//! (`FUN_00107c40` with the spot from `FUN_00108148`).

use super::court::{angle_toward, dist};
use super::offence::busy;
use super::pad::bits;
use super::words::words;
use super::{Brain, Ctx, skill};

/// The defensive gap by the man's distance from the basket (`0x3c6a00`):
/// `{distance, off the ball, on the ball}`, interpolated, ending in a
/// zero row.
const GAPS: [[f32; 3]; 7] = [
    [42.0, 20.0, 15.0],
    [27.0, 10.0, 4.0],
    [22.0, 8.0, 3.5],
    [19.0, 4.5, 3.5],
    [8.0, 3.5, 3.5],
    [2.0, 3.5, 3.5],
    [0.0, 0.0, 0.0],
];

/// Speed level reaches (`0x3c69e0`, `FUN_001080d0`), scaled by `skill(3,
/// 1.8, 1, 1)`: the distance to the spot each level covers, and the
/// distance the current level holds out to before a higher one may start.
const LEVELS: [[f32; 2]; 4] = [[0.8, 1.6], [1.2, 1.9], [1.7, 2.5], [200.0, 200.0]];

words! {
    pub(crate) struct Defense {
        /// The man (-1 none) and the player to help on.
        man: i32,
        help: i32,
        /// Frames to the next matchup check.
        recheck: i32,
        /// The speed level and the frames it has held.
        level: i32,
        held: i32,
        hold: i32,
        steal_timer: i32,
        counter_timer: i32,
        /// Frames the man has been shooting, and in the air.
        shooting: i32,
        flying: i32,
        /// The block decision for the current shot.
        fresh: bool,
        early: bool,
        late: bool,
        delay: i32,
        shooter: i32,
        /// Frames the man has stood still.
        still: i32,
        gamble_timer: i32,
        /// `[0x18]`: the block's bite on a pump fake (the spot stands 4
        /// off a shooter instead of 2.5).
        bite: bool,
        /// `[0x1f]`, `[0x20]`: the three-second draws: sag off a man who
        /// can drive (the spot's 6.8), and off a man whose teammate sets a
        /// pick (5 less).
        sag: bool,
        pick_sag: bool,
        /// `[0x21]`, `[0x22]`: frames to the next press draw, and the
        /// press (the spot's 3 or 4).
        press_timer: i32,
        press: bool,
        /// `[0x12]`: frames to the next pass jump check.
        pass_timer: i32,
    }
}

impl Defense {
    /// `FUN_001062b8`: the timers start at `fps × 0.2` (steal) and
    /// `fps × 0.125` (counter); the level hold is `fps × skill(1, 0.25,
    /// 0.25, 0.15)` frames.
    pub fn new(ctx: &Ctx, i: usize) -> Self {
        let hold = (60.0 * skill::value(ctx, ctx.team(i), 1.0, 0.25, 0.25, 0.15)) as i32;
        Self {
            man: -1,
            help: -1,
            steal_timer: 12,
            counter_timer: 7,
            fresh: true,
            shooter: -1,
            hold,
            held: hold,
            ..Default::default()
        }
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let team = ctx.team(i);
        if self.gamble_timer == 0 {
            // `0x106544..0x1065e0`: two draws every three seconds, the
            // spot's sags (`[0x1f]`, `[0x20]`).
            let p = skill::percent(ctx, team, 0, 0, 0x5e, 100);
            self.sag = ((ctx.rand() % 100) as i32) < p;
            self.gamble_timer = 180;
            let p = skill::percent(ctx, team, 0x46, 100, 0x46, 100);
            self.pick_sag = ((ctx.rand() % 100) as i32) < p;
        } else {
            self.gamble_timer -= 1;
        }
        self.press_draw(ctx, i);
        let handler = ctx.w.handler().filter(|&h| ctx.team(h) != team);
        match handler {
            Some(h) if busy(ctx, h) => {
                self.shooting += 1;
                if ctx.airborne(h) {
                    self.flying += 1;
                }
            }
            _ => {
                self.shooting = 0;
                self.flying = 0;
            }
        }
        if self.recheck > 0 {
            self.recheck -= 1;
        }
        // `0x106868..0x1068b4`: not while a pass (ball state 4) has left
        // the passer's hands within `fps / 3` frames (`DAT_004b38f4`, 19).
        let settled = ctx.game.ball != 4
            || ctx.w.handler().is_some()
            || ctx.game.ball_age > (60.0 * f32::from_bits(0x3eaa_aa9f)) as i32;
        if self.man < 0 || (self.recheck == 0 && settled) {
            let man = ctx.matchup(i, 0);
            let help = ctx.matchup(i, 1);
            self.man = man.map_or(-1, |m| m as i32);
            self.help = help.map_or(-1, |m| m as i32);
            self.recheck = if man.is_some_and(|m| ctx.slot(m) != ctx.slot(i)) || help.is_some() {
                (60.0 * skill::value(ctx, team, 0.25, 0.2, 0.14, 0.1)) as i32
            } else {
                1
            };
        }
        if self.man < 0 {
            b.stop(ctx, i);
            return;
        }
        let man = self.man as usize;
        if ctx.has_ball(man) && !ctx.w.players[man].moving() {
            self.still += 1;
        } else {
            self.still = 0;
        }
        // `0x1069f4..0x106a20`: nothing more while in the counter stance
        // (`FUN_00181828`), a block jump (`FUN_001818b0`) or a swipe
        // (`FUN_00181650`): the pad keeps the stick it was given. Off
        // balance or in a rebound jump he goes on as usual.
        if ctx.blocking(i) || matches!(ctx.w.players[i].action, crate::Action::Stealing { .. }) {
            return;
        }
        if self.counter(ctx, b, i) || self.block(ctx, b, i) || self.steal(ctx, b, i) {
            return;
        }
        self.position(ctx, b, i);
    }

    /// `0x1065e8..0x1067ac`: the press (`[0x22]`). With neither meter full
    /// and the ball short of 25 into the half this defender attacks, no
    /// press; else every `r % (fps × 0.6) + fps × 1.4` frames a draw on the
    /// ball's owner: `skill(0, 20, 10, 100)` when he stands more than 20
    /// into his own back court, plus `skill(0, 0, 100, 100)` with a full
    /// meter.
    fn press_draw(&mut self, ctx: &mut Ctx, i: usize) {
        let team = ctx.team(i);
        let full = ctx.meter_full(0) || ctx.meter_full(1);
        if !full && (ctx.side(team) as f32) * ctx.w.ball.position[0] < 25.0 {
            self.press_timer = 0;
            self.press = false;
            return;
        }
        if self.press_timer != 0 {
            self.press_timer -= 1;
            return;
        }
        let Some(owner) = ctx.ball_owner() else {
            return;
        };
        let r = ctx.rand();
        let span = (60.0 * f32::from_bits(0x3f19_999a)) as u32;
        let least = (60.0 * f32::from_bits(0x3fb3_3333)) as i32;
        self.press_timer = (r % span) as i32 + least;
        let mut p = 0;
        if (ctx.side(ctx.team(owner)) as f32) * ctx.pos(owner)[0] < -20.0 {
            p = skill::percent(ctx, team, 0, 20, 10, 100);
        }
        if full {
            p += skill::percent(ctx, team, 0, 0, 100, 100);
        }
        self.press = ((ctx.rand() % 100) as i32) < p;
    }

    /// `FUN_00107ae0`: how far a reach goes this frame, `skill(8.5, 6,
    /// 6, 6) − (r % 150) / 100 + 0.5`, two more from in front.
    fn reach(&self, ctx: &mut Ctx, i: usize) -> f32 {
        let base = skill::value(ctx, ctx.team(i), 8.5, 6.0, 6.0, 6.0);
        let r = ctx.rand();
        let mut v = base - (r % 150) as f32 / 100.0 + 0.5;
        if in_front(ctx, i) {
            v += 2.0;
        }
        v
    }

    /// `FUN_00107388`: every `fps × 0.2` frames, a reach for the ball of a
    /// handler who is not shooting and stands on the floor within reach,
    /// from nearer the basket. The odds: `skill(0, 0, 8, 16)`, plus (with
    /// no person on the side) Steals / 16 and `skill(0, 0, 5, 15)` from in
    /// front, plus `skill(0, 0, 30, 50)` when he has stood still, plus
    /// `skill(0, 0, 6, 16)` with a full meter.
    fn steal(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) -> bool {
        // A lesson (game+0x9c 4, `0x1073c0`) returns at once, before the
        // timer.
        if ctx.w.lesson.is_some() {
            return false;
        }
        let reach = self.reach(ctx, i);
        if self.steal_timer >= 1 {
            self.steal_timer -= 1;
            return false;
        }
        self.steal_timer = 12;
        let team = ctx.team(i);
        let Some(h) = ctx.w.handler().filter(|&h| ctx.team(h) != team) else {
            return false;
        };
        if busy(ctx, h) || ctx.airborne(h) {
            return false;
        }
        if ctx.ball_dist(i) >= reach && ctx.between(i, h) >= reach {
            return false;
        }
        if ctx.hoop_dist(i) - 1.0 >= ctx.hoop_dist(h) {
            return false;
        }
        let mut p = skill::percent(ctx, team, 0, 0, 8, 16);
        if !ctx.human[team] {
            let steals = (ctx.w.players[i].ratings.steals * 99.0).round() as i32;
            p += steals / 16;
            if in_front(ctx, i) {
                p += skill::percent(ctx, team, 0, 0, 5, 15);
            }
        }
        if self.standing(ctx, i) {
            p += skill::percent(ctx, team, 0, 0, 30, 50);
        }
        if !ctx.human[team] && (ctx.meter_full(0) || ctx.meter_full(1)) {
            p += skill::percent(ctx, team, 0, 0, 6, 16);
        }
        if ((ctx.rand() % 100) as i32) < p {
            b.face_point(ctx, i, ctx.pos(h), 1, 0);
            b.press(bits::STEAL);
            b.sprint(ctx, i);
            return true;
        }
        false
    }

    /// `FUN_001089d8`: the man has stood still `fps × skill(1.5, 0.8,
    /// 0.3, 0.2)` frames. (The original also counts a handler who has not
    /// moved 40 frames' worth; not modelled.)
    fn standing(&self, ctx: &Ctx, i: usize) -> bool {
        let n = (60.0 * skill::value(ctx, ctx.team(i), 1.5, 0.8, 0.3, 0.2)) as i32;
        self.still >= n
    }

    /// `FUN_00107710` (raw `0x107710..0x107adc`): every `fps × 0.125`
    /// frames, with turbo over 0.2, in front of the ball's holder
    /// ([`ball_holder`]) and no teammate already in the counter stance
    /// (`FUN_00181828`, actor+0xa6 bit 0), a trick counter (steal with two
    /// turbos). While a trick pass to himself is under way (game+0x160 not
    /// −2, [`trick_pass`]) only the defender it goes off (game+0x148)
    /// tries, at `skill(10, 15, 35, 50)`. Otherwise against a handler on
    /// the floor and not shooting, within reach and no nearer his own rim
    /// than a step behind the defender: `skill(8, 8, −4, −4)` (at least 0)
    /// if he is not juking (`FUN_001819f8`); while his channel 0 blends
    /// (flag 0x200000, the trick's opening `8e`) nothing, and the check
    /// comes again next update; else `skill(−13, −17, 40, 50)` plus
    /// `skill(−0.25, −0.33333, 0.75, 1) × (Steals − Handles)` (the
    /// difference kept to 0..50; at least 0). After a juke or a trick pass
    /// the next check waits `fps` frames. Every check past the reach draws
    /// once more for the roll.
    fn counter(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) -> bool {
        // The whole check is skipped in a lesson (game+0x9c 4, `0x107740`).
        if ctx.w.lesson.is_some() {
            return false;
        }
        if self.counter_timer >= 1 {
            self.counter_timer -= 1;
            return false;
        }
        self.counter_timer = 7;
        if ctx.turbo(i) < 0.2 || !in_front(ctx, i) {
            return false;
        }
        let team = ctx.team(i);
        if ctx.squad(team).into_iter().any(|j| countering(ctx, j)) {
            return false;
        }
        let reach = self.reach(ctx, i);
        let Some(h) = ball_holder(ctx).filter(|&h| ctx.team(h) != team) else {
            return false;
        };
        let mut p = 0;
        if let Some(off) = trick_pass(ctx) {
            if off == Some(i) {
                p = skill::percent(ctx, team, 10, 15, 35, 50);
                self.counter_timer = 60;
            }
        } else if !busy(ctx, h)
            && !ctx.airborne(h)
            && (ctx.ball_dist(i) < reach || ctx.between(i, h) < reach)
            && ctx.hoop_dist(i) - 1.0 < ctx.hoop_dist(h)
        {
            let juking = ctx.tricking(h)
                || (ctx.pads.get(h).is_some_and(|&p| p & bits::JUKE != 0) && ctx.turbo(h) >= 0.2);
            if !juking {
                p = skill::percent(ctx, team, 8, 8, -4, -4).max(0);
            } else if blending(ctx, h) {
                // `0x10793c..0x10795c`: not while the handler's channel 0
                // blends (flag 0x200000, the trick's opening `8e`); the
                // check comes again next update.
                self.counter_timer = 0;
            } else {
                p = skill::percent(ctx, team, -13, -17, 40, 50);
                // `0x107974..0x107998`: 0xbe800000, 0xbeaaa64c, 0x3f400000,
                // 1.0.
                let scale = skill::value(ctx, team, -0.25, f32::from_bits(0xbeaa_a64c), 0.75, 1.0);
                let edge = (ctx.rating(i, 6) - ctx.rating(h, 7)) as f32;
                p += (scale * edge.clamp(0.0, 50.0)) as i32;
                p = p.max(0);
                self.counter_timer = 60;
            }
        }
        if ((ctx.rand() % 100) as i32) < p {
            b.face_point(ctx, i, ctx.pos(h), 1, 0);
            b.press(bits::STEAL | bits::TWO_TURBOS);
            return true;
        }
        false
    }

    /// `FUN_00106a88` (raw `0x106a88..0x107194`): a block. The ball's
    /// holder (`FUN_0013bf60`) in a shot, dunk, lay-up or leap with the ball
    /// (`FUN_00181508`):
    ///
    /// - on a pump fake (actor+0x1d0) the defender who bit (`[0x18]`) goes
    ///   up at once when close enough (no turbo);
    /// - for each shot, once: early or late (`skill(0, 20, 88, 100)`),
    ///   the frames to wait after he leaves the floor (`r % (fps ×
    ///   0.05)`, none against a full meter) and the bite (`skill(100, 100,
    ///   20, 20)`, halved against a full meter);
    /// - with the shooter in the air and close enough: a late jump once he
    ///   has been shooting `fps × skill(0.95, 0.95, 0.45, 0.45)` frames,
    ///   an early one when he has been up the wait (or shooting `fps ×
    ///   0.3`), facing him, with turbo.
    ///
    /// Without such a holder: a late jump left over goes up at the shot
    /// just let go (ball state 5) when close enough; and every `fps /
    /// 12 + 1` frames of a pass in the air to a leaping receiver, a
    /// defender in its lane (`FUN_001838f0`) with nobody of his side up
    /// goes up at `skill(0, 5, 50, 100)` (the pass jump). The game
    /// modes that skip it all (vt+0xb4 1..5, 9, 0xb, 0xd, 0xe, 0x12,
    /// 0x13, 0x15; 0xc always bites) are the special games the rewrite
    /// does not play, and the passout specials' last leg (game+0x160
    /// ±1), which the pass jump also takes, is not modelled.
    fn block(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) -> bool {
        let team = ctx.team(i);
        let h = ctx
            .w
            .handler()
            .or_else(|| {
                // `0x106c74..0x106cd4`: a pass to himself (ball+0xfc the
                // passer, +0x100 the receiver) by a man leaping or dunking.
                let pass = ctx
                    .w
                    .ball
                    .pass
                    .filter(|_| ctx.w.ball.state == crate::BallState::Pass)?;
                (pass.from == pass.to && (ctx.leaping(pass.from) || ctx.dunking(pass.from)))
                    .then_some(pass.from)
            })
            .filter(|&h| ctx.team(h) != team);
        let Some(h) = h.filter(|&h| busy(ctx, h)) else {
            self.fresh = true;
            self.early = false;
            self.bite = false;
            let late = std::mem::take(&mut self.late);
            if late
                && self.shooter >= 0
                && ctx.w.ball.state == crate::BallState::Shot
                && self.can_reach(ctx, i, self.shooter as usize)
            {
                super::offence::build(ctx, b, i, bits::BLOCK);
                return true;
            }
            if h.is_some()
                || ctx.w.handler().is_some()
                || ctx.w.ball.state != crate::BallState::Pass
            {
                return false;
            }
            return self.pass_jump(ctx, b, i);
        };
        if self.bite && matches!(ctx.w.players[h].action, crate::Action::PumpFake { .. }) {
            // `0x106e94..0x106ed0`: the fake bitten on.
            if self.can_reach(ctx, i, h) {
                b.press(bits::BLOCK);
                self.bite = false;
                return true;
            }
            return false;
        }
        if self.fresh {
            let p = skill::percent(ctx, team, 0, 0x14, 0x58, 100);
            let r = ctx.rand();
            self.shooter = h as i32;
            self.fresh = false;
            self.early = ((r % 100) as i32) < p;
            self.late = !self.early;
            let mut p2 = skill::percent(ctx, team, 100, 100, 0x14, 0x14);
            if ctx.meter_full(1 - team) {
                // `0x106f80..0x106f8c`: no wait, and half the bite.
                self.delay = 0;
                p2 /= 2;
            } else {
                let r = ctx.rand();
                self.delay = (r % 3) as i32;
            }
            // `[0x18]`: `r % 100 < skill(100, 100, 20, 20)` (half of it
            // against a full meter).
            self.bite = ((ctx.rand() % 100) as i32) < p2;
        }
        if !self.early && !self.late {
            return false;
        }
        if !ctx.airborne(h) {
            return false;
        }
        if !self.can_reach(ctx, i, h) {
            return false;
        }
        if self.late {
            let wait = (60.0 * skill::value(ctx, team, 0.95, 0.95, 0.45, 0.45)) as i32;
            if self.shooting < wait {
                return false;
            }
            b.press(bits::BLOCK);
            self.late = false;
            return true;
        }
        if self.flying < self.delay && self.shooting < 18 {
            return false;
        }
        self.early = false;
        b.face_point(ctx, i, ctx.pos(h), 3, 0);
        b.press(bits::BLOCK);
        b.sprint(ctx, i);
        true
    }

    /// `0x106d6c..0x106e7c`: the pass jump, tried every `DAT_004b3900 + 1`
    /// frames (`fps × 0.0833334`, 5) while a pass is in the air: its
    /// receiver leaping (bit 15, `FUN_00181770`), none of the side up
    /// or asking to go up within 12 of the ball (`FUN_0018af90`), the
    /// defender in the pass's lane ([`in_lane`]), then `r % 100 <
    /// skill(0, 5, 50, 100)` through the pad builder.
    fn pass_jump(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) -> bool {
        if self.pass_timer != 0 {
            self.pass_timer -= 1;
            return false;
        }
        self.pass_timer = (60.0 * f32::from_bits(0x3daa_aab4)) as i32;
        let team = ctx.team(i);
        let Some(owner) = ctx.ball_owner() else {
            return false;
        };
        if !ctx.leaping(owner) || up_near_ball(ctx, team) != 0 || !in_lane(ctx, i) {
            return false;
        }
        let r = ctx.rand();
        let p = skill::percent(ctx, team, 0, 5, 0x32, 100);
        if ((r % 100) as i32) < p {
            super::offence::build(ctx, b, i, bits::BLOCK);
            return true;
        }
        false
    }

    /// `FUN_00107198` (raw `0x107198..0x107380`): close enough to block.
    /// Not when his nearest teammate (`FUN_0017f3d8(.., 0)`) is rising
    /// into a block jump below 1.5 within 5 of him (bit 12 or a computer's
    /// block asked this frame, `FUN_001818b0(.., 1)`, and actor+0x44
    /// above 0), nor when two of his side are up or asking within 12 of
    /// the ball (`FUN_0018af90`); then within `7 − r1/10` of the shooter
    /// and no more than `2 − r2/10` farther from the rim (`r` drawn
    /// modulo `skill(1, 1, 1, 20)`, from the game's distance tables).
    fn can_reach(&self, ctx: &mut Ctx, i: usize, shooter: usize) -> bool {
        let team = ctx.team(i);
        if let Some(m) = ctx.nth_teammate(i, 0)
            && up_or_asking(ctx, m)
            && ctx.w.players[m].height < 1.5
            && ctx.w.players[m].rise > 0.0
            && ctx.between(i, m) < 5.0
        {
            return false;
        }
        if up_near_ball(ctx, team) >= 2 {
            return false;
        }
        let k = skill::percent(ctx, team, 1, 1, 1, 20).max(1) as u32;
        let a = (ctx.rand() % k) as f32 / 10.0;
        let k = skill::percent(ctx, team, 1, 1, 1, 20).max(1) as u32;
        let c = 2.0 - (ctx.rand() % k) as f32 / 10.0;
        ctx.between(i, shooter) <= 7.0 - a && ctx.hoop_dist(i) - c < ctx.hoop_dist(shooter)
    }

    /// `FUN_00107c40`: run to the spot at a speed level, or stand and face
    /// the man. The distance to the spot that picks the level:
    ///
    /// - nothing within 3.5 of a spot a standing player already holds
    ///   ([`occupied`]), when the man has no ball and stands (his stick's
    ///   strength class `FUN_0013abf8` 0);
    /// - with a player to help on, halved beyond twice the first level's
    ///   reach;
    /// - else, standing (level 0) within 3.1 of it, nothing when the
    ///   defender is nearer the rim than a man who stands; otherwise a
    ///   third off a man without the ball, no pass coming to him, outside
    ///   the paint (zone under 5).
    ///
    /// The level is the first whose reach (`0x3c69e0` × `skill(3, 1.8, 1,
    /// 1)`) the distance is within; a post defender on a man posting up
    /// with the ball within 4 stands (the sim has neither). A lower level
    /// applies at once; a higher one waits while the distance is within the
    /// current level's second reach, then rises a level at a time (from
    /// standing to 1, or to 2 when it needs 2 or more) once the level has
    /// been held `hold` frames (raw `0x107f40..0x107fbc`). Running, he
    /// steers at the spot (turbo at level 3); on the update he comes to a
    /// stand he faces the man (the middle of the man and the player to
    /// help on), and standing he lets go of the stick.
    fn position(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) {
        let man = self.man as usize;
        let mut spot = self.spot(ctx, i, man);
        let help = usize::try_from(self.help).ok();
        if let Some(help) = help {
            let other = self.spot(ctx, i, help);
            let w1 = ctx.hoop_dist(man).min(3.0);
            let w2 = ctx.hoop_dist(help).min(3.0);
            if w1 + w2 > 0.0 {
                spot = [
                    (other[0] * w1 + spot[0] * w2) / (w1 + w2),
                    (other[1] * w1 + spot[1] * w2) / (w1 + w2),
                ];
            }
        }
        let scale = skill::value(ctx, ctx.team(i), 3.0, 1.8, 1.0, 1.0);
        let thr = |level: usize, column: usize| scale * LEVELS[level][column];
        let mut d = dist(ctx.pos(i), spot);
        if d <= 3.5 && !ctx.has_ball(man) && ctx.strength_class(man) == 0 && occupied(ctx, i, spot)
        {
            d = 0.0;
        }
        // `0x107da8..0x107e94`.
        let receiving = |j: usize| {
            ctx.w.ball.state == crate::BallState::Pass && ctx.w.ball.pass.is_some_and(|p| p.to == j)
        };
        let third = |d: f32| {
            if !ctx.has_ball(man) && !receiving(man) && ctx.zone(man) < 5 {
                d * 0.333_33
            } else {
                d
            }
        };
        if help.is_some() && 2.0 * thr(0, 0) < d {
            d *= 0.5;
        } else if self.level == 0
            && d <= 3.1
            && ctx.hoop_dist(i) < ctx.hoop_dist(man)
            && ctx.strength_class(man) == 0
        {
            d = 0.0;
        } else {
            d = third(d);
        }
        let mut level = 0;
        while level < 3 && thr(level, 0) < d {
            level += 1;
        }
        if self.held < self.hold {
            self.held += 1;
        }
        if ctx.posting_up(man)
            && ctx.has_ball(man)
            && ctx.post_defending(i)
            && ctx.between(i, man) < 4.0
        {
            level = 0;
        }
        let old = self.level;
        let current = self.level as usize;
        if current <= level && d <= thr(current, 1) {
            // Held at the current level (no change, the hold runs on).
        } else {
            if current < level {
                level = if current == 0 {
                    if level < 2 { 1 } else { 2 }
                } else {
                    current + 1
                };
            }
            if level < current || self.held >= self.hold {
                self.level = level as i32;
                self.held = 0;
            }
        }
        if self.level == old || self.level != 0 {
            if self.level == 3 {
                b.sprint(ctx, i);
            }
            let a = angle_toward(ctx.pos(i), spot);
            b.steer(ctx, i, a, self.level as usize, 0, None);
        } else {
            let mut at = ctx.pos(man);
            if let Some(help) = help {
                let h = ctx.pos(help);
                at = [(at[0] + h[0]) / 2.0, (at[1] + h[1]) / 2.0];
            }
            b.face_point(ctx, i, at, 1, 0x39);
        }
    }
}

/// `FUN_00108b48`: of the defender's two nearest players (game+0x23c),
/// one within 3.5 of him stands (strength class 0) within 3.5 of the spot.
fn occupied(ctx: &Ctx, i: usize, spot: [f32; 2]) -> bool {
    for j in ctx.by_distance(i).into_iter().take(2) {
        if 3.5 < ctx.between(i, j) {
            return false;
        }
        if ctx.strength_class(j) == 0 && dist(ctx.pos(j), spot) <= 3.5 {
            return true;
        }
    }
    false
}

words! {
    /// The defense strategy (`FUN_00122e98`, vtable `0x43a820`; start
    /// `FUN_00122ed8`, update `FUN_00123110`), one object for phases 4
    /// and 5. The matchups by the defender's slot (`FUN_00123d40`): the
    /// man (`[3..5]`), the player to help on (`[6..8]`, cleared every
    /// frame) and the man kept from frame to frame (`[9..11]`).
    pub(crate) struct DefenseCoach {
        man: [i32; 3],
        help: [i32; 3],
        second: [i32; 3],
        /// `[0xc]`: frames the switches wait after a pass.
        delay: i32,
        /// `[0xd]`: frames the handler has had an open lane.
        open: i32,
        /// `[0xe]`, `[0xf]`: the turbo marks (`0x3ccdf8` by the catch-up
        /// step).
        low: f32,
        high: f32,
        /// `[0x10]`: frames of the double team a person called for.
        double: i32,
        /// `[0x11]`, `[0x12]`: the screened defender and the frames to
        /// his switch.
        screen: i32,
        screened: i32,
        /// `[0x13]`: the defender a pick put down.
        picked: i32,
    }
}

impl DefenseCoach {
    pub fn new() -> Self {
        Self {
            man: [-1; 3],
            help: [-1; 3],
            second: [-1; 3],
            screened: -1,
            picked: -1,
            ..Default::default()
        }
    }

    /// `FUN_00122ed8`: everyone on defense (`FUN_001062b8(ai, 0)`), each
    /// guarding the opponent in his slot. The original also swaps the
    /// back-court matchups and picks up the handler here; the update's
    /// first loop copies `[9..11]` over them in the same frame, so the
    /// rewrite leaves them out.
    pub fn start(&mut self, ctx: &mut Ctx, brains: &mut [super::Brain], team: usize) {
        let squad = ctx.squad(team);
        let them = ctx.squad(1 - team);
        for (k, &j) in squad.iter().enumerate().take(3) {
            let d = Defense::new(ctx, j);
            brains[j].replace(ctx, j, super::Act::Defense(d));
            let man = them.get(k).map_or(-1, |&m| m as i32);
            self.help[k] = -1;
            self.second[k] = man;
            self.man[k] = man;
        }
        self.delay = 0;
        self.open = 0;
        let k = skill::behind(ctx, team).clamp(0, 6) as usize;
        (self.low, self.high) = TURBO_MARKS[k];
        self.screen = 0;
        self.double = 0;
        self.picked = -1;
    }

    /// The players it names beyond the matchups.
    pub fn named(&self) -> [i32; 2] {
        [self.screened, self.picked]
    }

    /// `FUN_00123d88` (vt+0x44): a person's call doubles the ball for
    /// `fps × 2` frames.
    pub fn call(&mut self) {
        self.double = 120;
    }

    /// `FUN_00123110`.
    pub fn update(&mut self, ctx: &mut Ctx, team: usize) {
        for k in 0..3 {
            self.help[k] = -1;
            self.man[k] = self.second[k];
        }
        let k = skill::behind(ctx, team).clamp(0, 6) as usize;
        if self.delay == 0 {
            // `0x1231d4`: the switches wait while a pass is in its first
            // frames: `fps × {0.7, 0.66667, 0.63333, 0.6, 0.58333,
            // 0.56667, 0.55}[k]`, or the receiver's distance from the rim
            // over `skill(0.73, 0.85, 0.85, 1.06)` if less, minus the
            // frames the pass has flown.
            if ctx.game.ball == 4 && ctx.game.ball_age <= 4 {
                const WAIT: [i32; 7] = [42, 40, 37, 36, 35, 34, 33];
                let most = WAIT[k];
                let speed = skill::value(ctx, team, 0.73, 0.85, 0.85, 1.06);
                self.delay = match ctx.ball_owner() {
                    Some(r) => ((ctx.hoop_dist(r) / speed) as i32).min(most),
                    None => most,
                };
                self.delay = (self.delay - ctx.game.ball_age).max(0);
            }
        } else {
            self.picked = -1;
            self.delay -= 1;
        }
        if self.delay == 0 {
            self.switches(ctx, team);
        } else {
            self.open = 0;
            self.picked = -1;
            self.screen = 0;
        }
        self.second = self.man;
        if self.double > 0 {
            self.double -= 1;
            // The defender nearest the handler, of those not on him,
            // doubles him.
            if let Some(h) = ctx.w.handler() {
                let squad = ctx.squad(team);
                let mut best: Option<(usize, f32)> = None;
                for (k, &j) in squad.iter().enumerate().take(3) {
                    if self.man[k] != h as i32 {
                        let d = ctx.between(j, h);
                        if best.is_none_or(|(_, b)| d < b) {
                            best = Some((k, d));
                        }
                    }
                }
                if let Some((k, _)) = best {
                    self.man[k] = h as i32;
                }
            }
        }
        ctx.matchups[team] = (self.man, self.help, self.second);
    }

    /// `FUN_00123510`: who switches onto the ball: a defender screened
    /// down (after `fps × skill(0.25, 0.17, 0.09, 0.09)` frames), the
    /// picked defender's teammate when the screen beats his man, or after
    /// the handler has had an open lane (or full openness) for `fps ×
    /// skill(1.15, 1, 0.317, 0.1667)` frames, the nearest defender who can
    /// help; a person who steps up takes his man; and a defender whose
    /// man sets a pick near the ball helps on the handler.
    fn switches(&mut self, ctx: &mut Ctx, team: usize) {
        let squad = ctx.squad(team);
        let slot = |j: usize| squad.iter().position(|&x| x == j).unwrap_or(0).min(2);
        let mut help_on: Option<usize> = None;
        let Some(owner) = ctx.ball_owner() else {
            self.open = 0;
            self.picked = -1;
            return;
        };
        let d = self.defender_of(ctx, team, owner);
        let mut switched = false;
        let mut beaten = false;
        let screened_check = |s: &mut Self, ctx: &mut Ctx, switched: &mut bool| {
            if s.screen < 1 {
                if let Some(d) = d
                    && ctx.fallen(d)
                    && let Some(by) = ctx.screener_of(d)
                    && by != owner
                {
                    s.screen = (60.0 * skill::value(ctx, team, 0.25, 0.17, 0.09, 0.09)) as i32;
                    s.screened = d as i32;
                }
            } else if let Ok(e) = usize::try_from(s.screened) {
                if !ctx.fallen(e) {
                    match ctx.screener_of(e) {
                        Some(by) if by == owner => s.screen = 0,
                        by => {
                            s.screen -= 1;
                            if s.screen == 0 {
                                *switched = true;
                                let k = by.map_or(0, |b| ctx.slot(b) % 3);
                                s.switch(ctx, team, k);
                            }
                        }
                    }
                } else {
                    s.screen = 0;
                }
            }
        };
        match usize::try_from(self.picked) {
            Err(_) => screened_check(self, ctx, &mut switched),
            Ok(p) if ctx.fallen(p) => screened_check(self, ctx, &mut switched),
            Ok(p) => {
                if let Some(d) = d
                    && ctx.between(p, owner) < ctx.between(d, owner)
                {
                    let (hp, ho) = (ctx.hoop_dist(p), ctx.hoop_dist(owner));
                    beaten = if hp <= ho {
                        true
                    } else {
                        ctx.hoop_dist(d) > ho
                    };
                }
            }
        }
        if beaten {
            let p = self.picked as usize;
            self.switch(ctx, team, slot(p));
        } else {
            let ignore = self.man;
            let lane = super::offence::path_open_ignoring(ctx, owner, Some(&ignore));
            let skip = !lane && super::offence::openness(ctx, owner) != 7;
            if skip {
                if self.picked >= 0 {
                    self.open = 0;
                }
            } else {
                self.open += 1;
                let need = (60.0 * skill::value(ctx, team, 1.15, 1.0, 0.317, 0.1667)) as i32;
                if !switched && need < self.open {
                    let ho = ctx.hoop_dist(owner);
                    let mut best = 100.0;
                    for (k, &j) in squad.iter().enumerate().take(3) {
                        if ctx.fallen(j) {
                            continue;
                        }
                        let hj = ctx.hoop_dist(j);
                        let near = ho < 6.0 || hj < ho + 3.0;
                        let candidate = near || (hj < ho + 7.0 && ctx.strength_class(owner) < 2);
                        if candidate {
                            let dj = ctx.between(owner, j);
                            if dj < best {
                                best = dj;
                                help_on = Some(k);
                            }
                        }
                    }
                }
            }
        }
        if let Some(k) = help_on
            && self.man[k] != owner as i32
        {
            self.switch(ctx, team, k);
        }
        if ctx.human[team]
            && let Some(n) = ctx.nth_opponent(owner, 0, false)
            && ctx.human_player(n)
            && !ctx.fallen(n)
            && self.man[slot(n)] != owner as i32
            && ctx.hoop_dist(owner) > 10.0
            && ctx.hoop_dist(n) < ctx.hoop_dist(owner)
        {
            self.switch(ctx, team, slot(n));
        }
        if let Some(e) = self.defender_of(ctx, team, owner) {
            for k in 0..3 {
                let Ok(m) = usize::try_from(self.man[k]) else {
                    continue;
                };
                if m != owner
                    && self.help[k] < 0
                    && !ctx.human_player(m)
                    && ctx.setting_pick(m)
                    && ctx.between(m, e) < 7.0
                {
                    self.help[k] = owner as i32;
                }
            }
        }
    }

    /// `FUN_0017f238` for the handler: the defender whose kept man is
    /// him, else the defender in his slot.
    fn defender_of(&self, ctx: &Ctx, team: usize, owner: usize) -> Option<usize> {
        let squad = ctx.squad(team);
        (0..3)
            .find(|&k| self.second[k] == owner as i32)
            .and_then(|k| squad.get(k).copied())
            .or_else(|| squad.get(ctx.slot(owner).min(2)).copied())
    }

    /// `FUN_00123a68`: defender `k` takes the ball. In the owner's back
    /// court he only helps; a deep defender (nearest the rim) leaves his
    /// man only when the man is not 3 nearer the rim than a handler over
    /// 15 out; the defender on the ball keeps him while within 5.2 of a
    /// handler who is dribbling in place, or is still nearer the rim.
    fn switch(&mut self, ctx: &mut Ctx, team: usize, k: usize) {
        self.picked = -1;
        self.screen = 0;
        self.help[k] = -1;
        let Some(owner) = ctx.ball_owner() else {
            return;
        };
        let squad = ctx.squad(team);
        let Some(&d) = squad.get(k) else {
            return;
        };
        // `0x123ae0`: in the owner's back court the defender only helps.
        let attack = ctx.side(ctx.team(owner)) as f32;
        if attack * ctx.pos(owner)[0] < 0.0 {
            self.help[k] = owner as i32;
            return;
        }
        if ctx.has_ball(owner)
            && ctx.hoop_dist(owner) > 15.0
            && ctx.nth_of_team_to_rim(team, 0) == Some(d)
            && let Ok(m) = usize::try_from(self.man[k])
            && ctx.hoop_dist(m) + 3.0 < ctx.hoop_dist(owner)
        {
            self.help[k] = owner as i32;
            return;
        }
        // Who guards him now (none: no switch).
        let Some(j) = (0..3).find(|&j| j != k && self.man[j] == owner as i32) else {
            return;
        };
        let e = squad[j];
        if ctx.has_ball(owner) && ctx.posting_up(owner) {
            let behind = !ctx.post_defending(e)
                && (ctx.fallen(e) || ctx.hoop_dist(owner) >= ctx.hoop_dist(e));
            if !behind && ctx.between(e, owner) < 5.2 {
                return;
            }
        }
        if ctx.fallen(e) && ctx.nth_opponent(owner, 0, false) == Some(e) {
            self.picked = e as i32;
        }
        let was = self.man[k];
        self.man[k] = owner as i32;
        self.man[j] = was;
    }
}

/// The defense's turbo marks, low and high, by the catch-up step
/// (`0x3ccdf8`).
const TURBO_MARKS: [(f32, f32); 7] = [
    (0.7, 0.8),
    (0.6, 0.7),
    (0.5, 0.6),
    (0.4, 0.5),
    (0.3, 0.4),
    (0.2, 0.3),
    (0.1, 0.2),
];

/// `FUN_00107b80`: in front of the ball's holder ([`ball_holder`]): nearer
/// the basket and within ±71 of his facing.
fn in_front(ctx: &Ctx, i: usize) -> bool {
    let team = ctx.team(i);
    let Some(h) = ball_holder(ctx).filter(|&h| ctx.team(h) != team) else {
        return false;
    };
    if ctx.hoop_dist(i) >= ctx.hoop_dist(h) {
        return false;
    }
    super::court::turn(ctx.facing(h), ctx.angle_between(h, i)).abs() <= 71
}

/// `FUN_0013bf60` as the defence reads it: the ball's holder (ball+0xf4).
/// State4 includes the held windup: all231 self-pass windup samples in
/// the three attract captures keep the passer, all929 released samples
/// have0. The earlier reading of the first73 as flight samples was wrong.
/// Raw0x13bf7c..0x13bfb4 returns no AI holder when ball+0xf4 is0.
fn ball_holder(ctx: &Ctx) -> Option<usize> {
    ctx.w.handler()
}

/// game+0x160 not −2 (`FUN_00142038`): a trick pass to himself asked
/// this frame (`FUN_00142068`) or in its held windup, with the defender it
/// goes off (game+0x148), none off the board. A released pass has no holder
/// for this defensive check, even while the special's identity persists.
fn trick_pass(ctx: &Ctx) -> Option<Option<usize>> {
    if ctx.game.trick != 0 {
        return Some(usize::try_from(ctx.game.trick_off - 1).ok());
    }
    ctx.w.passout.filter(|po| !po.tossed).map(|po| po.defender)
}

/// Channel 0's flag 0x200000 (`FUN_0019c2e0(actor + 0xc4, 0)` +0x48): a
/// blend under way, as the sim keeps them (a trick's, a reaction's, a
/// script's; the gait's own blends are not modelled).
fn blending(ctx: &Ctx, h: usize) -> bool {
    match ctx.w.players[h].action {
        crate::Action::Trick { play, .. } => play.run.blend_counter().is_some(),
        _ => ctx.w.channel(h).is_some_and(|c| c.counter.is_some()),
    }
}

/// actor+0xa6 bit 0 (`FUN_0016e9b0`, the AI's `FUN_00181828`): in the
/// trick counter's stance.
fn countering(ctx: &Ctx, j: usize) -> bool {
    ctx.countering(j)
}

impl Defense {
    /// `FUN_00108148`: where defender `i` guards `man`: `gap` from him on
    /// the line to the basket, never past it. The gap:
    ///
    /// - against a post-up (actor bit 11, or a computer man whose front
    ///   behaviour is type 7): 4 when the defender stands more than 71
    ///   units round the rim from him and no more than 3 farther out,
    ///   else 1, or 2.2 within 2.9 of him;
    /// - against a handler standing still more than 33.75 out (the
    ///   press), 3; with the press flag (`[0x22]`), 3 on the ball or 4 on
    ///   a man the ball is passed to deep in his back court;
    /// - against a shooter, 2.5 (4 after biting on the fake, `[0x18]`);
    /// - else the table (`0x3c6a00`) by his distance from the basket, the
    ///   on-ball column for the handler (or a pass's receiver after the
    ///   flight's first `fps / 3` frames) unless he faces more than 142
    ///   units away from the defender beyond 5; off the ball 8 more when
    ///   the pass goes to a man who, with the defender, stands nearest
    ///   the rim, and the side's best shot-blocker (team+0xc4,
    ///   `FUN_0018b340`: height in feet plus Blocks · 6 / 100) 5 more off
    ///   a man more than 12 beyond the gap from the rim.
    ///
    /// With the three-second sag (`[0x1f]`), unless the man is posting up,
    /// in a shot or dunk, or there is a press (all ignored while he jukes),
    /// the gap is at least 11 off a juking handler (whose level hold is
    /// then cut to at most `fps / 10` from done, node `[0xc]`/`[0xd]`,
    /// taken as the rewrite's `held`/`hold`), at least 6.8 off a man with
    /// turbo 0.2 or more who has the ball more than 11 out (and is not
    /// standing still in a press) or is receiving a pass, and 2.5 more
    /// when the defender and the man are the two nearest the rim. With the pick sag (`[0x20]`) off the
    /// handler 5 less when one of his computer teammates sets a pick
    /// (front behaviour type 1) within 10 of the defender; else the gap is
    /// at most his distance from the rim.
    ///
    /// The actor's dribble state (`FUN_00181750`, actor+0x1a0 not
    /// negative) is taken as true: the sim keeps no picked-up dribble.
    pub(super) fn spot(&mut self, ctx: &Ctx, i: usize, man: usize) -> [f32; 2] {
        let team = ctx.team(i);
        let own = ctx.goal(1 - team);
        let m = ctx.pos(man);
        let to = [own[0] - m[0], own[1] - m[1]];
        let far = crate::math::len(to[0], to[1]).max(1.0);
        let has = ctx.has_ball(man);
        let press = has && self.standing(ctx, i) && 33.75 < ctx.hoop_dist(man);
        // `FUN_0013abb8`: a pass in flight to him (ball state 4, no holder).
        let receiving = ctx.w.ball.state == crate::BallState::Pass
            && ctx.w.ball.pass.is_some_and(|p| p.to == man);
        let posting =
            ctx.posting_up(man) || (!ctx.human_player(man) && ctx.fronts.get(man) == Some(&7));
        // The two nearest the rim (game+0x3ec, +0x3f0).
        let mut by_rim: Vec<usize> = (0..ctx.w.players.len()).collect();
        by_rim.sort_by(|&a, &b| ctx.hoop_dist(a).total_cmp(&ctx.hoop_dist(b)));
        let nearest_two =
            |a: usize, b: usize| by_rim.len() >= 2 && by_rim[0] == a && by_rim[1] == b;
        let mut post_defence = false;
        let mut gap;
        if posting {
            // `0x108290..0x108380`.
            post_defence = true;
            let around = super::court::turn(ctx.hoop_angle(i), ctx.hoop_angle(man)).abs();
            gap = if 71 < around && ctx.hoop_dist(man) <= ctx.hoop_dist(i) + 3.0 {
                4.0
            } else if ctx.between(i, man) >= 2.9 {
                1.0
            } else {
                2.2
            };
        } else if press {
            gap = 3.0;
        } else if self.press
            && (has || (receiving && (ctx.side(ctx.team(man)) as f32) * m[0] < -10.0))
        {
            gap = if has { 3.0 } else { 4.0 };
        } else if has && busy(ctx, man) {
            gap = if self.bite { 4.0 } else { 2.5 };
        } else {
            let on_ball = if has {
                true
            } else {
                receiving && ctx.game.ball_age >= (60.0 * f32::from_bits(0x3eaa_aa9f)) as i32
            };
            let column = if on_ball
                && !(142
                    < super::court::turn(ctx.angle_to_hoop(man), ctx.angle_between(man, i)).abs()
                    && 5.0 < ctx.between(i, man))
            {
                2
            } else {
                1
            };
            gap = interpolate(far, column);
            if !has {
                if receiving {
                    if nearest_two(man, i) || nearest_two(i, man) {
                        gap += 8.0;
                    }
                } else if self.blocker(ctx, team) == Some(i) && ctx.hoop_dist(man) - gap > 12.0 {
                    gap += 5.0;
                }
            }
        }
        if self.sag {
            let juking = ctx.tricking(man)
                || (has
                    && ctx.pads.get(man).is_some_and(|&p| p & bits::JUKE != 0)
                    && ctx.turbo(man) >= 0.2);
            let held_off = !juking
                && (ctx.posting_up(man) || self.press || busy(ctx, man) || ctx.leaping(man));
            if !held_off {
                let mut least = gap;
                let turbo = ctx.turbo(man);
                if !has {
                    if receiving && 0.2 <= turbo {
                        least = 6.8;
                    }
                } else if juking {
                    // `0x10870c..0x10875c`: 11 off a juking handler, and
                    // the level's hold at most `fps / 10` from done.
                    least = 11.0;
                    let most = (60.0 * f32::from_bits(0x3dcc_cccd)) as i32;
                    if most < self.hold - self.held {
                        self.held = self.hold - most;
                    }
                } else if !(press && !ctx.w.players[man].moving())
                    && 0.2 <= turbo
                    && 11.0 < ctx.hoop_dist(man)
                {
                    least = 6.8;
                }
                if !post_defence && nearest_two(i, man) {
                    least += 2.5;
                }
                gap = gap.max(least);
            }
        }
        let mut picked = false;
        if self.pick_sag && has {
            for j in ctx.squad(ctx.team(man)).into_iter().take(3) {
                if j != man
                    && !ctx.human_player(j)
                    && ctx.setting_pick(j)
                    && ctx.between(i, j) < 10.0
                {
                    gap -= 5.0;
                    picked = true;
                    break;
                }
            }
        }
        if !picked && ctx.hoop_dist(man) < gap {
            gap = ctx.hoop_dist(man);
        }
        [m[0] + to[0] / far * gap, m[1] + to[1] / far * gap]
    }

    /// team+0xc4 (`FUN_0018b340`): the side's best shot-blocker, height in
    /// feet plus Blocks · 6 / 100, the first of equals.
    fn blocker(&self, ctx: &Ctx, team: usize) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for j in ctx.squad(team).into_iter().take(3) {
            let v = ctx.height_ft(j) + (ctx.rating(j, 5) as f32 * 6.0) / 100.0;
            if best.is_none_or(|(_, b)| b < v) {
                best = Some((j, v));
            }
        }
        best.map(|(j, _)| j)
    }
}

/// `FUN_001818b0(j, 1)`: in a block jump (bit 12), or a computer player
/// whose pad asks for one this frame (its pad word, still last frame's
/// for those the AI has not run yet).
fn up_or_asking(ctx: &Ctx, j: usize) -> bool {
    ctx.blocking(j)
        || (!ctx.human_player(j) && ctx.pads.get(j).is_some_and(|&p| p & bits::BLOCK != 0))
}

/// `FUN_0018af90`: how many of `team` are up or asking (`up_or_asking`)
/// within 12 of the ball (the game's table, game+0x3d4).
fn up_near_ball(ctx: &Ctx, team: usize) -> usize {
    ctx.squad(team)
        .into_iter()
        .filter(|&j| up_or_asking(ctx, j) && ctx.ball_dist(j) < 12.0)
        .count()
}

/// `FUN_001838f0` (raw `0x1838f0..0x183aec`): defender `i` in a pass's
/// lane. The ball in the air (state 4, no holder); on the floor: he is
/// nearer the pass's point (ball+0x90) than the ball is, 10..22 from the
/// ball, ahead of it along the pass (the dot product above 0) and within
/// 5 of its line.
fn in_lane(ctx: &Ctx, i: usize) -> bool {
    let ball = &ctx.w.ball;
    if ball.state != crate::BallState::Pass {
        return false;
    }
    let at = [ball.position[0], ball.position[2]];
    // The point the pass is aimed at: where its flight left carries it
    // across the floor.
    let left = ball.flight_left.max(0.0);
    let to = [
        at[0] + ball.velocity[0] * left,
        at[1] + ball.velocity[2] * left,
    ];
    let me = ctx.pos(i);
    let near = dist(me, at);
    if !(dist(me, to) < dist(at, to) && 10.0 < near && near < 22.0) {
        return false;
    }
    let a = [me[0] - at[0], me[1] - at[1]];
    let d = [to[0] - at[0], to[1] - at[1]];
    let dot = a[0] * d[0] + a[1] * d[1];
    if dot <= 0.0 {
        return false;
    }
    let t = dot / (d[0] * d[0] + d[1] * d[1]);
    let off = [a[0] - d[0] * t, a[1] - d[1] * t];
    (off[0] * off[0] + off[1] * off[1]).sqrt() < 5.0
}

/// `0x1084fc..0x10856c`: the first row the distance is beyond, and the
/// line from the row before it.
fn interpolate(d: f32, column: usize) -> f32 {
    let mut k = 0;
    while k < GAPS.len() - 1 && d <= GAPS[k][0] {
        k += 1;
    }
    if k == 0 {
        return GAPS[0][column];
    }
    let (a, b) = (GAPS[k - 1], GAPS[k]);
    (a[column] - b[column]) * (d - b[0]) / (a[0] - b[0]) + b[column]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gap_follows_the_discs_table() {
        assert_eq!(interpolate(50.0, 1), 20.0);
        assert_eq!(interpolate(27.0, 2), 4.0);
        // Halfway from 22 (8) to 27 (10) off the ball.
        assert!((interpolate(24.5, 1) - 9.0).abs() < 1e-5);
        // Within 2 the gap closes with the zero row.
        assert!((interpolate(1.0, 1) - 1.75).abs() < 1e-5);
    }
}
