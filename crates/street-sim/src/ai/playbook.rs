//! The playbook (`FUN_00126f60`, start `FUN_00127318`, update
//! `FUN_001276f8`; vtable `0x43aee0`): the offence's strategy in the
//! front court. The trick-point, Dunks Only and Gamebreaker Wins games
//! run its subclass (`FUN_00124028`, vtable `0x43a998`: start
//! `FUN_00124088`, update `FUN_00124270`).
//!
//! Each frame it hands out the roles, runs the gamebreaker calls, starts
//! a play when none runs (`FUN_001281c8`, weighted by `FUN_00128708`;
//! the subclass's `FUN_00124ad0`), runs it, and then makes its checks:
//! the shot (`FUN_001290b8`), the pass (`FUN_00128b30`), the trick pass
//! out (`FUN_00128ec0`), the penetration (`FUN_001298c8`), the post cut
//! (`FUN_00129a20`) and the alley-oop (`FUN_00129d98`). The plays are in
//! [`super::plays`].

use super::moves::{Follower, Leaper, can_leap};
use super::offence::{KeepAway, Pass, Shoot, busy, juking, openness, outside, path_open, quality};
use super::picks::SetPick;
use super::plays::{Play, can_pass, holder, kind, meter_ok, suits};
use super::spots::{PerimPlayer, PostCut};
use super::words::words;
use super::{Act, Brain, Ctx, pad::bits, skill};

words! {
    pub(crate) struct Playbook {
        /// `[0x23]`: the play running.
        play: Play,
        /// `[0x24]`: the last play to run, or -1.
        prev: i32,
        /// `[0x2c]` (+0xb0): a play to start whenever one is chosen, or
        /// -1. Its constructor sets -1 and no game changes it; the
        /// rewrite's tests set it.
        forced: i32,
        /// `[0x25]`: frames to the next pass check (the "pass override"
        /// while it counts).
        pass_wait: i32,
        /// `[0x26]`: frames to the next pass-out check.
        pass_out_wait: i32,
        /// `[0x27]`: frames before a person's side may call a pick play.
        human_wait: i32,
        /// `[0x28]`: frames the playbook holds off after calling a shot
        /// (the "shot override").
        shot_hold: i32,
        /// `[0x29]`, `[0x2a]`: the shot check's two timers.
        shot_timer: i32,
        shot_wait: i32,
        /// `[0x2b]`: shoot once the shot clock is at or under this
        /// (1..4, drawn).
        shot_mark: i32,
        /// `[0x32]`: the star, or -1.
        star: i32,
        /// `[0x36]`, `[0x37]`: frames the handler's lane has stood open,
        /// and how many start a penetration.
        pen_count: i32,
        pen_mark: i32,
        /// `[0x38..0x3a]`, `[0x3b]`: by slot, frames a teammate's lane has
        /// stood open, and how many send him on a post cut.
        cut_counts: [i32; 3],
        cut_mark: i32,
        /// `[0x3c]`: frames to the next alley-oop check.
        oop_wait: i32,
        /// `[0x3d]`: play for the meter (only plays fit for it, and none
        /// of the ratings' extra weight); `[0x3e]`: always pull up;
        /// `[0x3f]`: Easy against a person; `[0x45]`: keep the ball away
        /// while the other side's meter is full.
        meter: bool,
        pull_up: bool,
        easy: bool,
        keep_away: bool,
        /// `[0x43]`, `[0x44]`: the teammate a pick was called for, and a
        /// wait.
        pick: i32,
        pick_wait: i32,
    }
}

/// The default play list (`0x3ce438`).
const DEFAULT: [i32; 13] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13];

/// `0x3ce470`, `0x3ce480`: the alley-oop check's bonus and the second
/// leaper's odds, by the difficulty as `FUN_00129d98` reads it.
const OOP_BONUS: [i32; 3] = [10, 20, 30];
const OOP_FOLLOW: [i32; 3] = [5, 30, 40];

fn who(v: i32) -> Option<usize> {
    usize::try_from(v).ok()
}

fn id(j: Option<usize>) -> i32 {
    j.map_or(-1, |j| j as i32)
}

impl Playbook {
    pub fn new() -> Self {
        Self {
            play: Play::none(),
            prev: -1,
            forced: -1,
            star: -1,
            pick: -1,
            ..Default::default()
        }
    }

    /// The players it names.
    pub fn named(&self) -> [i32; 6] {
        let [a, b, c, d] = self.play.named();
        [a, b, c, d, self.star, self.pick]
    }

    /// `FUN_00127318` (the subclass's `FUN_00124088`).
    pub fn start(&mut self, ctx: &mut Ctx, team: usize) {
        let sub = subclass(ctx);
        self.play = Play::none();
        self.prev = -1;
        self.pass_wait = 18;
        if !sub {
            self.pass_out_wait = 18;
        }
        self.human_wait = 60;
        self.shot_timer = 0;
        self.shot_hold = 0;
        self.pick = -1;
        self.pick_wait = 0;
        let r = ctx.rand();
        self.pen_count = 0;
        self.shot_mark = (r & 3) as i32 + 1;
        self.oop_wait = 6;
        self.cut_counts = [0; 3];
        self.star = id(star_of(ctx, team));
        let (pen, cut) = if sub {
            self.meter = false;
            self.pull_up = false;
            self.easy = false;
            (0.15, 0.25)
        } else {
            // Catch-up table 1 (`0x3dc13d`): how keen a side this far
            // behind is to play for its meter.
            let t = skill::table1(ctx, team);
            let r = ctx.rand();
            self.pull_up = t == 100;
            self.meter = ((r % 100) as i32) < t;
            if skill::difficulty(ctx, team) == 0 {
                // The game's vt+0x44(0, side), read and dropped, and a
                // draw dropped too.
                ctx.rand();
                self.easy = true;
            } else {
                self.easy = false;
            }
            let mut pen = skill::value(
                ctx,
                team,
                f32::from_bits(0x3eaa_aa3b),
                f32::from_bits(0x3f2a_aae3),
                f32::from_bits(0x3e2a_ab8a),
                f32::from_bits(0x3e2a_ab8a),
            );
            let mut cut = skill::value(
                ctx,
                team,
                f32::from_bits(0x3fea_aa8f),
                f32::from_bits(0x400a_aab9),
                1.0,
                1.0,
            );
            if !self.easy {
                if !self.pull_up && !self.meter {
                    if basket_ends_game(ctx, team, false) && 0.15 < pen {
                        pen = 0.15;
                    }
                } else {
                    pen = pen.max(0.25);
                    cut = cut.max(1.25);
                }
            }
            (pen, cut)
        };
        let r = ctx.rand();
        self.pen_mark = (r % 5) as i32 + (60.0 * pen) as i32;
        let r = ctx.rand();
        self.cut_mark = (r % 5) as i32 + (60.0 * cut) as i32;
        let p = skill::percent(ctx, team, 0, 50, 100, 100);
        let r = ctx.rand();
        self.keep_away = ((r % 100) as i32) < p;
    }
}

/// The trick-point, Dunks Only and Gamebreaker Wins games (modes 1, 2
/// and 6) run the subclass.
fn subclass(ctx: &Ctx) -> bool {
    matches!(super::offence::mode(ctx), 1 | 2 | 6)
}

/// The game's vt+0x7c asked as `(side, 2, gamebreaker)` (`FUN_00129590`,
/// `FUN_00129748`, `FUN_00121c78`): an inside basket by `team`, a Level 1
/// gamebreaker's or none, would win the game, by the game object's class
/// ([`crate::World::basket_would_win`]).
pub(super) fn basket_ends_game(ctx: &Ctx, team: usize, gamebreaker: bool) -> bool {
    ctx.w.basket_would_win(team, 2, u8::from(gamebreaker))
}

/// `FUN_0012a7d0`: the star, the side's biggest share of its overall
/// ratings (rating 9), each share scaled, once both are known, by the
/// side's (points × 2 + shots tried) per second played over his own
/// (stats 0 and 10 and actor+0x2a4, [`crate::stats::Stats::star_weights`]);
/// the first of equals. The weights' percentages (+0xcc..+0xd4) have no
/// reader here.
fn star_of(ctx: &Ctx, team: usize) -> Option<usize> {
    let squad: Vec<usize> = ctx.squad(team).iter().take(3).copied().collect();
    let ratings: Vec<i32> = squad.iter().map(|&j| ctx.rating(j, 9)).collect();
    let weights = ctx.w.stats.star_weights(team, &squad, &ratings);
    let mut best: Option<(usize, f32)> = None;
    for (&j, &weight) in squad.iter().zip(&weights) {
        if best.is_none_or(|(_, b)| b < weight) {
            best = Some((j, weight));
        }
    }
    best.map(|(j, _)| j)
}

/// `FUN_0012aaa8`: once a play ends, the star is kept only with 15 or
/// more seconds on the shot clock as vt+0x5c reads it (whole seconds mod
/// 60, so never in the original's normal games): there is none after.
fn refresh_star(ctx: &Ctx, pb: &mut Playbook, team: usize) {
    pb.star = if ctx.shot_clock() < 900 {
        -1
    } else {
        id(star_of(ctx, team))
    };
}

/// `FUN_00181508(ai, 1)`: busy in a shot, or (a computer player)
/// holding the ball with a shot asked for this frame or queued.
fn busy_shot(ctx: &Ctx, brains: &[Brain], h: usize) -> bool {
    busy(ctx, h)
        || (!ctx.human_player(h)
            && ctx.has_ball(h)
            && (ctx.pads.get(h).is_some_and(|&p| p & bits::SHOOT != 0)
                || brains[h].front_kind() == 9))
}

/// `FUN_001285d0`: the play is over, or its handler pauses within 12 of
/// the rim.
fn play_done(ctx: &Ctx, pb: &Playbook, brains: &[Brain], team: usize) -> bool {
    if !pb.play.running() {
        return false;
    }
    if pb.play.done(ctx, brains, team) {
        return true;
    }
    holder(ctx, team).is_some_and(|h| brains[h].front_kind() == 3 && ctx.hoop_dist(h) < 12.0)
}

/// `FUN_00128ae0` (`FUN_00124e08`): start play `k` with the star.
fn begin(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize, k: i32) {
    pb.play = Play::begin(ctx, brains, team, k, who(pb.star));
}

/// `FUN_00128668` (`FUN_00124e58`): play `k` may start: it suits `j`
/// (when one is given), it is not the last play, and (with the meter
/// mode, not in the subclass) it may run with a full meter.
fn check(ctx: &Ctx, pb: &Playbook, team: usize, k: i32, j: Option<usize>, sub: bool) -> bool {
    if j.is_some_and(|j| !suits(ctx, team, k, j)) {
        return false;
    }
    if pb.prev == k {
        return false;
    }
    sub || !pb.meter || meter_ok(k)
}

/// `FUN_00128708(rating, k)`: one more entry for play `k` when the star
/// rates over 49 at `rating` and the play suits him (not the last play).
fn rate(ctx: &Ctx, pb: &Playbook, team: usize, list: &mut Vec<i32>, rating: usize, k: i32) {
    let Some(s) = who(pb.star) else {
        return;
    };
    if ctx.rating(s, rating) > 0x31 && suits(ctx, team, k, s) && pb.prev != k {
        list.push(k);
    }
}

/// `FUN_00128a68`: one of the list, one draw.
fn pick(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize, list: &[i32]) {
    if list.is_empty() {
        return;
    }
    let r = ctx.rand();
    let k = list[(r % list.len() as u32) as usize];
    begin(ctx, pb, brains, team, k);
}

/// `FUN_001281c8` (`FUN_00124ad0` for the subclass): with the side
/// holding the ball and the handler free (no pass or shot under way, the
/// pass check's wait over, no shot just called), a play.
fn select(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize, sub: bool) {
    let Some(h) = holder(ctx, team) else {
        pb.play = Play::none();
        return;
    };
    if pb.pass_wait != 0 || pb.shot_hold != 0 {
        return;
    }
    if ctx.passing_now(h, &brains[h]) || busy_shot(ctx, brains, h) || ctx.catching(h) {
        return;
    }
    if pb.forced != -1 {
        let k = pb.forced;
        begin(ctx, pb, brains, team, k);
        return;
    }
    if sub {
        // `FUN_00124ed8` answers yes whenever the side holds the ball
        // (both of its exits return 1): the subclass always penetrates.
        begin(ctx, pb, brains, team, kind::PENETRATION);
        return;
    }
    if pb.pull_up {
        begin(ctx, pb, brains, team, kind::PULL_UP);
        return;
    }
    if penetrate_now(ctx, pb, team, h) {
        begin(ctx, pb, brains, team, kind::PENETRATION);
        return;
    }
    let list = play_list(ctx, pb, team, h);
    pick(ctx, pb, brains, team, &list);
}

/// `FUN_001281c8`'s lists after the penetration: the plays that suit the
/// star with his ratings' extra entries, else (no star, or none suits
/// him) the default list, each play once for the handler and once
/// for anyone.
pub(super) fn play_list(ctx: &Ctx, pb: &Playbook, team: usize, h: usize) -> Vec<i32> {
    let mut list = Vec::new();
    if let Some(s) = who(pb.star) {
        for k in 0..16 {
            if check(ctx, pb, team, k, Some(s), false) {
                list.push(k);
            }
        }
        if !pb.meter {
            rate(ctx, pb, team, &mut list, 7, kind::PENETRATION);
            rate(ctx, pb, team, &mut list, 7, kind::ISOLATION);
            rate(ctx, pb, team, &mut list, 4, kind::BACK_DOWN);
            rate(ctx, pb, team, &mut list, 4, kind::POST_UP);
        }
        if skill::difficulty(ctx, team) != 0 {
            rate(ctx, pb, team, &mut list, 1, kind::ISOLATION_3PT);
            rate(ctx, pb, team, &mut list, 1, kind::ISOLATION_3PT);
        }
        rate(ctx, pb, team, &mut list, 1, kind::PICK_AND_ROLL_3PT);
    }
    if !list.is_empty() {
        return list;
    }
    for k in DEFAULT {
        if check(ctx, pb, team, k, Some(h), false) {
            list.push(k);
        }
        if check(ctx, pb, team, k, None, false) {
            list.push(k);
        }
    }
    if skill::difficulty(ctx, team) != 0 {
        let k = kind::ISOLATION_3PT;
        if check(ctx, pb, team, k, Some(h), false) {
            list.push(k);
        }
        if check(ctx, pb, team, k, None, false) {
            list.push(k);
        }
        if check(ctx, pb, team, k, None, false) && 0x28 <= ctx.rating(h, 1) {
            list.push(k);
        }
    }
    list
}

/// `FUN_001287e0` (raw `0x1287e0..0x128a5c`): penetrate now. With the
/// handler in the front court ahead of both teammates (still in the back
/// court) the star always does, anyone else `skill(0, 10, 10, 40)`% of the
/// time (`(10, 0, 20, 10)` playing for the meter; one draw); the loop
/// leaves for the lane as soon as a teammate is in the front court.
/// Otherwise a handler with an open lane does when in the paint, when no
/// teammate is more than 5 nearer the rim, or when no defender is more
/// than 2 nearer.
fn penetrate_now(ctx: &mut Ctx, pb: &Playbook, team: usize, h: usize) -> bool {
    let front = |ctx: &Ctx, j: usize| {
        let x = ctx.pos(j)[0];
        let side = ctx.side(ctx.team(j));
        (0.0 <= x && side == 1) || (x < 0.0 && side == -1)
    };
    if front(ctx, h) && ctx.squad(team).iter().all(|&j| j == h || !front(ctx, j)) {
        if pb.star == h as i32 {
            return true;
        }
        let p = if pb.meter {
            skill::percent(ctx, team, 10, 0, 20, 10)
        } else {
            skill::percent(ctx, team, 0, 10, 10, 40)
        };
        let r = ctx.rand();
        if ((r % 100) as i32) < p {
            return true;
        }
    }
    lane_penetrates(ctx, team, h)
}

/// `FUN_001287e0`'s lane test: the handler's lane open, and he in the
/// paint, or no teammate more than 5 nearer the rim, or no defender more
/// than 2 nearer.
fn lane_penetrates(ctx: &Ctx, team: usize, h: usize) -> bool {
    if !path_open(ctx, h) {
        return false;
    }
    if 5 <= ctx.zone(h) {
        return true;
    }
    let d = ctx.hoop_dist(h);
    if ctx
        .squad(team)
        .iter()
        .all(|&j| j == h || d <= ctx.hoop_dist(j) + 5.0)
    {
        return true;
    }
    ctx.squad(1 - team)
        .iter()
        .take(3)
        .all(|&o| d <= ctx.hoop_dist(o) + 2.0)
}

/// The playbook (`FUN_001276f8`; the subclass's `FUN_00124270`).
pub(super) fn update(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize) {
    let sub = subclass(ctx);
    let full = ctx.meter_full(team);
    // `0x127720`: a full meter puts the side in the meter mode.
    if !sub && full {
        pb.meter = true;
    }
    // `FUN_0018bf80`: the side's man with the ball, or a pass's
    // receiver, is the handler (1); of the other two the lower slot is
    // the post man (2), the higher the perimeter man (3) (`0x3dce68`).
    let mut perim = None;
    if let Some(o) = ctx.ball_owner().filter(|&o| ctx.team(o) == team) {
        brains[o].role = 1;
        let (a, b) = (ctx.other_of(o, 0), ctx.other_of(o, 1));
        if let Some(a) = a {
            brains[a].role = 2;
        }
        if let Some(b) = b {
            brains[b].role = 3;
        }
        perim = b;
    }
    // `[0x43]`: the pick's teammate, while he sets it or runs off it
    // (types 1 and 2).
    if let Some(p) = who(pb.pick)
        && !(1..=2).contains(&brains[p].front_kind())
    {
        pb.pick = -1;
    }
    if pb.pick_wait > 0 {
        pb.pick_wait -= 1;
    }
    if ctx.human[team] {
        // `0x1277d8`: a person's side.
        if pb.human_wait > 0 {
            pb.human_wait -= 1;
        }
        pb.play = Play::none();
        if let Some(q) = perim
            && (path_open(ctx, q) || ctx.rand() & 1 != 0)
        {
            brains[q].role = 2;
        }
        post_cut_check(ctx, pb, brains, team);
        oop_check(ctx, pb, brains, team, sub);
        // `0x127a38..`: with no pick under way (`FUN_0012ab38`) and the
        // wait over, the first teammate placed for one (`FUN_0012abe0`)
        // sets a pick for the person (`FUN_0012b0c0`). Game vt+0xb4 is
        // never 0xf in the rewrite's games.
        if let Some(h) = ctx.w.handler()
            && !pick_under_way(ctx, brains, team)
            && pb.human_wait == 0
        {
            let squad = ctx.squad(team);
            if let Some(&j) = squad.iter().take(3).find(|&&j| pick_placed(ctx, pb, h, j)) {
                pick_call(ctx, pb, brains, team, j);
            }
        }
        return;
    }
    // The special games' branches (game vt+0xb4 of 8..11 and 0x10) do
    // not apply: vt+0xb4 is -1 in every game the rewrite plays.
    let h = holder(ctx, team);
    if pocket_check(ctx, team) {
        if let Some(h) = h {
            brains[h].replace(ctx, h, Act::Pocket(false));
        }
        return;
    }
    if cancel_check(ctx, team) {
        pb.play = Play::none();
        if let Some(h) = h {
            brains[h].replace(ctx, h, Act::Cancel(false));
        }
        return;
    }
    if keep_away(ctx, pb, brains, team, sub) {
        return;
    }
    if pb.play.running() {
        pb.prev = pb.play.kind;
        if play_done(ctx, pb, brains, team) {
            refresh_star(ctx, pb, team);
            select(ctx, pb, brains, team, sub);
        }
    } else {
        select(ctx, pb, brains, team, sub);
    }
    if pb.play.running() {
        let mut play = pb.play;
        play.run(ctx, brains, team);
        pb.play = play;
        if play_done(ctx, pb, brains, team) {
            refresh_star(ctx, pb, team);
            pb.play = Play::none();
        }
    }
    shot_check(ctx, pb, brains, team, sub);
    if sub {
        pass_out_check(ctx, pb, brains, team);
        pass_check(ctx, pb, brains, team);
    } else {
        pass_check(ctx, pb, brains, team);
        pass_out_check(ctx, pb, brains, team);
    }
    penetrate_check(ctx, pb, brains, team);
    post_cut_check(ctx, pb, brains, team);
    oop_check(ctx, pb, brains, team, sub);
}

/// `FUN_0012ab38`: a teammate without the ball is setting a pick or
/// rolling off one (types 1 and 2).
fn pick_under_way(ctx: &Ctx, brains: &[Brain], team: usize) -> bool {
    ctx.squad(team)
        .into_iter()
        .take(3)
        .any(|j| !ctx.has_ball(j) && (1..=2).contains(&brains[j].front_kind()))
}

/// `FUN_0012abe0(j)`: a teammate placed to set a pick for a person
/// holding the ball: no pick called lately (`[0x44]`), `j` without the
/// ball, out of the paint and within 33.75 of the rim, no more than 2
/// farther from it than the holder and within 15 of him, inside 398 units
/// of where he faces (398.72 truncated); the holder moving, a person, out
/// of the paint, with his defender within 33.75 of the rim and 15 of him.
fn pick_placed(ctx: &Ctx, pb: &Playbook, h: usize, j: usize) -> bool {
    if 1 <= pb.pick_wait || ctx.has_ball(j) || 4 < ctx.zone(j) || 33.75 < ctx.hoop_dist(j) {
        return false;
    }
    if !ctx.w.players[h].moving() || !ctx.human_player(h) {
        return false;
    }
    let Some(d) = super::plays::defender(ctx, h) else {
        return false;
    };
    if 33.75 < ctx.hoop_dist(d) || 4 < ctx.zone(h) {
        return false;
    }
    if ctx.hoop_dist(h) + 2.0 < ctx.hoop_dist(j) || 15.0 < ctx.between(h, j) {
        return false;
    }
    let off = super::court::turn(ctx.angle_between(h, j), ctx.facing(h)).abs();
    off <= 398 && ctx.between(h, d) <= 15.0
}

/// `FUN_0012b0c0(screener)`: on a side a person plays, the screener sets a
/// pick for the holder ("set pick", in the mode for a holder wide of
/// 27.75), the third man goes to the perimeter, and the side may not call
/// another for a second (`[0x27]`).
fn pick_call(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize, s: usize) {
    let Some(h) = ctx.w.handler() else {
        return;
    };
    if !ctx.human[team] {
        return;
    }
    let mode = 27.75 < ctx.pos(h)[0].abs();
    let p = SetPick::new(ctx, mode);
    brains[s].replace(ctx, s, Act::SetPick(p));
    for j in ctx.squad(team).into_iter().take(3) {
        if j != h && j != s {
            brains[j].replace(ctx, j, Act::PerimPlayer(PerimPlayer::new(true)));
        }
    }
    pb.human_wait = 60;
}

/// `FUN_0012ae70` (vt+0x44, both versions): a person's strategy call.
/// With the holder in a shot, a lay-up or a dunk every teammate leaps for
/// an alley-oop (and the alley-oop check waits `fps × 3`). Outside the
/// paint, with no teammate (on screen, `FUN_00181808`, which the rewrite
/// takes as always) setting or rolling off a pick, the nearest teammate
/// sets one (`[0x43]`; not within `[0x44]`'s wait); otherwise (in the
/// paint, or a pick already under way) the call is cancelled: everyone to
/// the perimeter, and no call for `fps × 0.6` frames.
pub(super) fn call(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize) {
    let Some(h) = ctx.w.handler() else {
        return;
    };
    let squad = ctx.squad(team);
    if busy(ctx, h) || ctx.laying_up(h) || ctx.leaping(h) || ctx.dunking(h) {
        pb.oop_wait = 180;
        for &j in squad.iter().take(3) {
            brains[j].replace(ctx, j, Act::Leaper(Leaper::new(false)));
        }
        return;
    }
    if ctx.zone(h) < 5 {
        let hs = ctx.slot(h);
        let picking = squad
            .iter()
            .take(3)
            .enumerate()
            .any(|(k, &j)| k != hs && (1..=2).contains(&brains[j].front_kind()));
        if !picking {
            if pb.pick_wait != 0 {
                return;
            }
            let t = ctx.nth_teammate(h, 0);
            pb.pick = id(t);
            if let Some(t) = t {
                pick_call(ctx, pb, brains, team, t);
            }
            return;
        }
    }
    pb.pick_wait = 36;
    for &j in squad.iter().take(3) {
        brains[j].replace(ctx, j, Act::PerimPlayer(PerimPlayer::new(true)));
    }
}

/// `FUN_0012b1f8` (vt+0x74; the subclass's `FUN_00124fb8`): with the
/// keep-away flag (`[0x45]`), the side's meter not full and the other
/// side's full, with more than `skill(24, 16, 16, 10)` on the shot clock,
/// the side keeps the ball away: the best ballhandler (`FUN_0012b380`)
/// runs "keep away" (`FUN_0010d4c8`), anyone else passes to him; and the
/// playbook does nothing else that frame (even with nobody holding the
/// ball). A holder on the other side stands for the side's man in his
/// slot (`FUN_0018c018` through `FUN_0018beb8`). The subclass
/// (`FUN_00124fb8`) also keeps away in Gamebreaker Wins played to a Level
/// 2 (game+0xf8 1) with its own meter at 45,000 or more, short of full and
/// none pocketed (meter+0x20), keep-away flag or not.
fn keep_away(ctx: &mut Ctx, pb: &Playbook, brains: &mut [Brain], team: usize, sub: bool) -> bool {
    let usual = pb.keep_away && !ctx.meter_full(team) && ctx.meter_full(1 - team);
    let racing = sub
        && ctx.w.rules.goal == crate::Goal::Gamebreaker(2)
        && !ctx.meter_full(team)
        && !ctx.w.teams[team.min(1)].pocketed
        && 45_000 <= ctx.w.teams[team.min(1)].meter;
    if !usual && !racing {
        return false;
    }
    if ctx.shot_clock() <= skill::percent(ctx, team, 24, 16, 16, 10) {
        return false;
    }
    let squad = ctx.squad(team);
    let Some(h) = ctx
        .w
        .handler()
        .and_then(|o| squad.get(ctx.slot(o).min(2)).copied())
    else {
        return true;
    };
    // `FUN_0012b380`: the most Handles, the later slot on a tie.
    let mut best = (-1, h);
    for &j in squad.iter().take(3) {
        let r = ctx.rating(j, 7);
        if best.0 <= r {
            best = (r, j);
        }
    }
    let b = best.1;
    if h == b {
        if brains[h].front_kind() != 0x12 {
            brains[h].replace(ctx, h, Act::KeepAway(KeepAway::new()));
        }
    } else if brains[h].front_kind() != 8 {
        let p = Pass::new(ctx, h, Some(b));
        brains[h].replace(ctx, h, Act::Pass(p));
    }
    true
}

/// `FUN_001296b0`: the handler shoots; the play ends and the playbook
/// holds off its other calls for `fps × 0.5` frames.
fn shoot(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize) {
    let Some(h) = holder(ctx, team) else {
        return;
    };
    pb.play = Play::none();
    pb.shot_hold = 30;
    let s = Shoot::new(ctx, h);
    brains[h].replace(ctx, h, Act::Shoot(s));
}

/// `FUN_00129748` (raw `0x129748..0x1298a0`): with the side's meter full,
/// shoot (for the gamebreaker) at once when under 2 s are left of its 30
/// (team meter+0x18, [`crate::TeamState::meter_hold`], below `fps × 2`,
/// `0x1297a8..0x1297c4`), else a shot of 7 or better within 33.75: from
/// beyond the arc, or when a gamebreaker inside basket would end the
/// game, or with the side more than 4 up.
fn meter_shot(ctx: &Ctx, team: usize, h: usize) -> bool {
    if !ctx.meter_full(team) {
        return false;
    }
    if ctx.w.teams[team.min(1)].meter_hold < 2 * crate::TICK_HZ as i32 {
        return true;
    }
    if quality(ctx, h) <= 6 || 33.75 <= ctx.hoop_dist(h) {
        return false;
    }
    if outside(ctx, h, 0.5) || basket_ends_game(ctx, team, true) {
        return true;
    }
    4 < ctx.w.score[team] as i64 - ctx.w.score[1 - team] as i64
}

/// `FUN_00129590` (vt+0x84): a shooter of 60+ Threes, the man nearest the
/// rim (game+0x3ec), beyond the arc within 29.75 with nobody within 8,
/// when an inside basket would not end the game.
fn open_three(ctx: &Ctx, team: usize, h: usize) -> bool {
    0x3b < ctx.rating(h, 1)
        && !basket_ends_game(ctx, team, false)
        && ctx.nearest_to_rim() == Some(h)
        && outside(ctx, h, 0.5)
        && ctx.hoop_dist(h) < 29.75
        && ctx
            .nth_opponent(h, 0, false)
            .is_none_or(|o| 8.0 < ctx.between(h, o))
}

/// `FUN_001290b8` (the subclass's `FUN_00124618`): when to shoot. By the
/// shot clock (and the meters: the other side's full and ours not is the
/// strictest) a shot of at least `now` goes at once on the timer's frames
/// (every `skill(0.2, 0.1, 0, 0)` s), one of at least `need` (2 less on
/// Easy) one time in eight every `fps × 0.6` frames; at the clock's mark
/// always. Only from 12..29.75 out, not past a defender on a drive, and
/// from beyond the arc not for a shooter under 50 Threes with time left.
/// The subclass shoots only a dunk (`FUN_00183d90`).
fn shot_check(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize, sub: bool) {
    let h = holder(ctx, team);
    let sc = ctx.shot_clock();
    let ours = ctx.meter_full(team);
    let theirs = ctx.meter_full(1 - team);
    let (now, mut need) = if ours || !theirs {
        match sc {
            ..3 => (5, 3),
            3..=6 => (5, 5),
            7..=8 => (6, 5),
            9..=15 => (6, 6),
            _ => (7, 6),
        }
    } else {
        (7, 6)
    };
    if pb.easy {
        need -= 2;
    }
    pb.shot_timer = if pb.shot_timer < 1 {
        (60.0 * skill::value(ctx, team, 0.2, 0.1, 0.0, 0.0)) as i32
    } else {
        pb.shot_timer - 1
    };
    if pb.shot_wait > 0 {
        pb.shot_wait -= 1;
    }
    let Some(h) = h else {
        pb.shot_hold = 0;
        pb.shot_timer = 0;
        pb.shot_wait = 0;
        return;
    };
    if brains[h].front_kind() == 9 {
        pb.shot_hold = 0;
        pb.shot_timer = 0;
        pb.shot_wait = 0;
        return;
    }
    if pb.shot_hold > 0 {
        pb.shot_hold -= 1;
        return;
    }
    if sc <= pb.shot_mark {
        shoot(ctx, pb, brains, team);
        return;
    }
    if pb.play.kind == kind::ALLEY_OOP
        || ctx.rebounding(h)
        || ctx.catching(h)
        || busy_shot(ctx, brains, h)
        || juking(ctx, &brains[h], h)
    {
        return;
    }
    if ours {
        if meter_shot(ctx, team, h) {
            shoot(ctx, pb, brains, team);
        } else if !sub && pb.play.kind != kind::PULL_UP {
            begin(ctx, pb, brains, team, kind::PULL_UP);
        }
        return;
    }
    if !sub && open_three(ctx, team, h) {
        shoot(ctx, pb, brains, team);
        return;
    }
    if pb.play.running() && !pb.play.allows(ctx, team, 1) {
        return;
    }
    if outside(ctx, h, 0.0) && ctx.rating(h, 1) < 0x32 && 4 < sc {
        return;
    }
    let d = ctx.hoop_dist(h);
    if 29.75 <= d || d <= 12.0 {
        return;
    }
    if matches!(brains[h].front_kind(), 5 | 6)
        && ctx
            .squad(1 - team)
            .iter()
            .take(3)
            .any(|&o| d < ctx.hoop_dist(o))
    {
        return;
    }
    let dunk = |ctx: &Ctx| !sub || super::offence::dunk_close(ctx, h);
    #[cfg(test)]
    probe::count(quality(ctx, h), need);
    if pb.shot_timer == 0 && now <= quality(ctx, h) && dunk(ctx) {
        shoot(ctx, pb, brains, team);
        return;
    }
    if pb.shot_wait != 0 || quality(ctx, h) < need || !dunk(ctx) {
        return;
    }
    pb.shot_wait = 36;
    if ctx.rand() & 7 != 0 {
        return;
    }
    shoot(ctx, pb, brains, team);
}

/// `FUN_00128b30`: with more than 3 on the shot clock, every so often
/// (`skill(0.3, 0, 0, 0)` s, or a second after a pass) the handler,
/// not open himself (4 or less) nor in the paint with an open lane, passes
/// to the teammate in the front court beyond 6 of him who has stood open
/// longest (over half a second, a tenth when two defenders crowd the
/// handler outside the pick and roll) and is the most open, if 6 or more.
fn pass_check(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize) {
    let h = holder(ctx, team);
    if h.is_some_and(|h| ctx.catching(h)) {
        pb.pass_wait = 0;
    }
    let Some(h) = h else {
        return;
    };
    if ctx.shot_clock() <= 3 {
        return;
    }
    if pb.pass_wait >= 1 {
        pb.pass_wait -= 1;
        return;
    }
    if (pb.play.running() && !pb.play.allows(ctx, team, 0))
        || pb.shot_hold != 0
        || pb.play.kind == kind::ALLEY_OOP
        || busy_shot(ctx, brains, h)
        || !can_pass(ctx, brains, h, 6)
        || ctx.catching(h)
        || (4 < ctx.zone(h) && path_open(ctx, h))
        || 4 < openness(ctx, h)
    {
        return;
    }
    let crowded = super::plays::double_teamed(ctx, h, 8.5)
        && !matches!(pb.play.kind, kind::PICK_AND_ROLL | kind::PICK_AND_ROLL_3PT);
    let need = if crowded { 6 } else { 30 };
    let mut best: Option<(usize, i32)> = None;
    for j in ctx.squad(team).into_iter().take(3) {
        if j == h || ctx.between(h, j) <= 6.0 {
            continue;
        }
        if 5.0 < ctx.up_court(j) && need < ctx.open_frames(j) {
            let o = openness(ctx, j);
            if best.is_none_or(|(_, b)| b < o) {
                best = Some((j, o));
            }
        }
    }
    match best {
        Some((j, o)) if 6 <= o => {
            pb.play = Play::none();
            let p = Pass::new(ctx, h, Some(j));
            brains[h].replace(ctx, h, Act::Pass(p));
            pb.pass_wait = 60;
        }
        _ => pb.pass_wait = (60.0 * skill::value(ctx, team, 0.3, 0.0, 0.0, 0.0)) as i32,
    }
}

/// `FUN_00128ec0`: the trick pass out (Off the Heezay or Back 2 Papa,
/// [`super::offence::trick_pass_out`], `FUN_00182d10`): a covered handler
/// (openness under 5) out of the paint or with a shut lane throws the
/// trick pass to himself ("trick pass", type 0x17, `FUN_00100c60`); the
/// play ends and the check waits `fps × 3` frames, or `fps × 0.5` when he
/// does not.
fn pass_out_check(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize) {
    let Some(h) = holder(ctx, team) else {
        return;
    };
    if ctx.shot_clock() <= 3 {
        return;
    }
    if pb.pass_out_wait >= 1 {
        pb.pass_out_wait -= 1;
        return;
    }
    if (pb.play.running() && !pb.play.allows(ctx, team, 0))
        || pb.shot_hold != 0
        || pb.play.kind == kind::ALLEY_OOP
        || pb.pass_wait != 0
        || busy_shot(ctx, brains, h)
        || ctx.catching(h)
        || (4 < ctx.zone(h) && path_open(ctx, h))
        || 4 < openness(ctx, h)
    {
        return;
    }
    let Some(heezay) = super::offence::trick_pass_out(ctx, &brains[h], h, false) else {
        pb.pass_out_wait = 30;
        return;
    };
    pb.play = Play::none();
    let t = super::offence::TrickPass::new(ctx, h, heezay);
    brains[h].replace(ctx, h, Act::TrickPass(t));
    pb.pass_out_wait = 180;
}

/// `FUN_001298c8`: frames the handler's lane stands open (counted only
/// while he is free and no penetration runs); at the drawn mark, within
/// 25.75, a penetration (when the play allows).
fn penetrate_check(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize) {
    let Some(h) = holder(ctx, team) else {
        pb.pen_count = 0;
        return;
    };
    if pb.play.kind == kind::PENETRATION
        || pb.play.kind == kind::ALLEY_OOP
        || pb.pass_wait != 0
        || pb.shot_hold != 0
        || busy_shot(ctx, brains, h)
        || ctx.passing_now(h, &brains[h])
        || ctx.rebounding(h)
        || !path_open(ctx, h)
    {
        pb.pen_count = 0;
        return;
    }
    pb.pen_count += 1;
    let allowed = !pb.play.running() || pb.play.allows(ctx, team, 2);
    if allowed && pb.pen_mark <= pb.pen_count && ctx.hoop_dist(h) < 25.75 {
        begin(ctx, pb, brains, team, kind::PENETRATION);
    }
}

/// `FUN_00129a20`: by slot, frames a teammate's lane stands open (not the
/// handler's, nor the pick's man's). Unless the handler's own lane is
/// open within 22, a teammate at the drawn mark 10..28.75 out cuts: on a
/// computer side as the "post cut" play (when the play allows), on a
/// person's side straight to his block (a "post cut", unless a teammate
/// is headed there).
fn post_cut_check(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize) {
    let Some(h) = holder(ctx, team) else {
        pb.cut_counts = [0; 3];
        return;
    };
    if pb.play.kind == kind::POST_CUT
        || pb.play.kind == kind::ALLEY_OOP
        || pb.shot_hold != 0
        || busy_shot(ctx, brains, h)
        || ctx.passing_now(h, &brains[h])
        || ctx.rebounding(h)
        || ctx.catching(h)
        || juking(ctx, &brains[h], h)
    {
        pb.cut_counts = [0; 3];
        return;
    }
    let squad = ctx.squad(team);
    for (k, &j) in squad.iter().enumerate().take(3) {
        pb.cut_counts[k] = if j == h || !path_open(ctx, j) || pb.pick == j as i32 {
            0
        } else {
            pb.cut_counts[k] + 1
        };
    }
    if path_open(ctx, h) && ctx.hoop_dist(h) < 22.0 {
        return;
    }
    for (k, &j) in squad.iter().enumerate().take(3) {
        let d = ctx.hoop_dist(j);
        if pb.cut_mark > pb.cut_counts[k] || d <= 10.0 || 28.75 < d {
            continue;
        }
        if !ctx.human[team] {
            if !pb.play.running() || pb.play.allows(ctx, team, 3) {
                pb.star = j as i32;
                begin(ctx, pb, brains, team, kind::POST_CUT);
                return;
            }
        } else if ctx.zone_ahead(j) < 5 {
            let low = if ctx.across(j) < 0.0 { 5 } else { 6 };
            let taken = squad
                .iter()
                .take(3)
                .any(|&o| o != j && ctx.zone_ahead(o) == low);
            if !taken {
                let c = PostCut::new(ctx, j, low, true, false);
                brains[j].replace(ctx, j, Act::PostCut(c));
            }
        }
    }
}

/// `FUN_00129d98`: the alley-oop. Every so often (each `fps × 0.25`
/// frames after a miss), with the side's meter not full and the handler
/// within 33.75 (6..9 only in a dunk) and free, more than 4 on the
/// shot clock and nobody leaping yet (a leaper, or bit 15), each teammate who could
/// leap (`FUN_0012a568`: within 33.5 across, a dunk on and an open lane)
/// is asked in turn: his Dunks / 5 percent (on Easy against a person his
/// side's Dunks + 5, the computer side's Dunks / 20 + 5), 5 more for the
/// star, 10/20/30 more (`0x3ce470`) by the difficulty with the handler in
/// a dunk. A computer side starts the "alley oop" play for him; a
/// person's teammate leaps. Then, `0x3ce480`'s 5/30/40%, a second leaper
/// (Dunks% each) 6..33.75 out and more than 43 round the rim from him
/// follows. The check waits `fps × 6` frames after one.
fn oop_check(ctx: &mut Ctx, pb: &mut Playbook, brains: &mut [Brain], team: usize, sub: bool) {
    let _ = sub;
    let h = holder(ctx, team);
    if pb.oop_wait >= 1 {
        pb.oop_wait -= 1;
        return;
    }
    // In a lesson only the Alley-Oop lesson (vt+0xb4 -1 or 5); not with a
    // full meter, nor with a gamebreaker ball in play (`FUN_00141018`).
    if ctx.w.lesson.is_some_and(|l| l != 5) || ctx.meter_full(team) || ctx.w.gamebreaker_ball() != 0
    {
        return;
    }
    let Some(h) = h else {
        return;
    };
    if pb.play.kind == kind::ALLEY_OOP {
        return;
    }
    let d = ctx.hoop_dist(h);
    if d < 9.0 && (!ctx.dunking(h) || d < 6.0) {
        return;
    }
    if 33.75 < d || (busy_shot(ctx, brains, h) && !ctx.dunking(h)) || ctx.passing_now(h, &brains[h])
    {
        return;
    }
    if pb.play.running() && !pb.play.allows(ctx, team, 4) {
        return;
    }
    if ctx.shot_clock() <= 4 {
        return;
    }
    let squad = ctx.squad(team);
    if squad
        .iter()
        .take(3)
        .any(|&j| (!ctx.human_player(j) && brains[j].front_kind() == 4) || ctx.leaping(j))
    {
        return;
    }
    let persons = ctx.human[0] || ctx.human[1];
    let easy = ctx.w.rules.difficulty == crate::Difficulty::Easy;
    let level = if !persons {
        1
    } else {
        let d = match ctx.w.rules.difficulty {
            crate::Difficulty::Easy => 0,
            crate::Difficulty::Medium => 1,
            crate::Difficulty::Hard => 2,
        };
        if ctx.human[team] { 2 - d } else { d }
    };
    for &p in squad.iter().take(3) {
        let fit = can_leap(ctx, p) && path_open(ctx, p) && ctx.pos(p)[0].abs() < 33.5;
        if !fit {
            continue;
        }
        let dunks = ctx.rating(p, 2);
        let mut odds = if !persons || !easy {
            dunks / 5
        } else if !ctx.human[team] {
            if dunks == 0 { 0 } else { dunks / 20 + 5 }
        } else if dunks == 0 {
            0
        } else {
            dunks + 5
        };
        if pb.star == p as i32 && dunks != 0 {
            odds += 5;
        }
        if ctx.dunking(h) && dunks != 0 {
            odds += OOP_BONUS[level];
        }
        let r = ctx.rand();
        let mut called = false;
        if ((r % 100) as i32) < odds {
            if !ctx.human[team] {
                if !ctx.meter_full(team) && !pb.pull_up && pb.pass_wait == 0 && pb.shot_hold == 0 {
                    pb.oop_wait = 360;
                    called = true;
                    pb.star = p as i32;
                    begin(ctx, pb, brains, team, kind::ALLEY_OOP);
                }
            } else {
                called = true;
                pb.oop_wait = 360;
                brains[p].replace(ctx, p, Act::Leaper(Leaper::new(false)));
            }
        } else {
            pb.oop_wait = 15;
        }
        if called {
            let r = ctx.rand();
            if ((r % 100) as i32) < OOP_FOLLOW[level] {
                let mut second = None;
                for &q in squad.iter().take(3) {
                    if q == p || q == h {
                        continue;
                    }
                    let r = ctx.rand();
                    if ((r % 100) as i32) <= ctx.rating(q, 2) {
                        second = Some(q);
                        break;
                    }
                }
                if let Some(q) = second {
                    let apart = super::court::turn(ctx.hoop_angle(p), ctx.hoop_angle(q)).abs();
                    let dq = ctx.hoop_dist(q);
                    if 0x2b < apart && dq < 33.75 && 6.0 < dq {
                        brains[q].replace(ctx, q, Act::Follower(Follower::new(p)));
                    }
                }
            }
            return;
        }
    }
}

/// The gates `FUN_00141098` and `FUN_00141190` share: a full meter, no
/// gamebreaker ball in play (`FUN_00141018`), the side on offence
/// (game+0x110) with the ball held (ball+0xf4), its own not pocketed
/// (meter+0x20). `FUN_00140960` (a person's side with game+0x488 set)
/// never holds for the computer's sides the playbook asks for.
fn pocket_gates(ctx: &Ctx, team: usize) -> bool {
    ctx.meter_full(team)
        && ctx.w.gamebreaker_ball() == 0
        && ctx.offence() == team
        && ctx.w.handler().is_some()
        && !ctx.w.teams[team].pocketed
}

/// `FUN_00141098`: a gamebreaker can go in the pocket: the gates, not in
/// No Gamebreakers (type 3), and in a lesson (game+0x9c 4) only the Pocket
/// lesson (vt+0xb4 0x17).
fn can_pocket(ctx: &Ctx, team: usize) -> bool {
    pocket_gates(ctx, team)
        && ctx.w.rules.game_type() != 3
        && ctx.w.lesson.is_none_or(|l| l == 0x17)
}

/// `FUN_00141190`: the other side's pocket can be cancelled: the gates, the
/// other side pocketed, and in a lesson only the Cancel lesson (0x18).
fn can_cancel(ctx: &Ctx, team: usize) -> bool {
    pocket_gates(ctx, team)
        && ctx.w.teams[1 - team].pocketed
        && ctx.w.lesson.is_none_or(|l| l == 0x18)
}

/// The game's vt+0xc4, by the game object's class:
///
/// - the points and lesson games (`FUN_0012ebf0`) and the trick-point
///   game (`FUN_0018c8e0`): with a pocket possible, one draw: pocket when
///   neither side, its score (vt+0x9c, the trick points in the trick-point
///   game) with the asking side's last gain between two meter fills
///   (stats+0x9e8 less +0x9e4, [`crate::stats::Stats::fills`], added to
///   both sides' scores) and a margin (`r % 3 + 2`; the trick-point game's
///   `r % 75000 + 25000`), reaches the target (game+0xf8);
/// - Gamebreaker Wins (`FUN_00145d28`), only to a Level 2 (game+0xf8 1):
///   it clears game+0x4c0; a pocket possible pockets unless a cancel is
///   possible too, when it pockets by one draw with the chance of the
///   other side's meter in percent (`0x3d4120`: 100 under 10, 75 under 20,
///   50 under 30, else none), else sets game+0x4c0 for the cancel; with
///   only a cancel possible it sets game+0x4c0.
///
/// The type both checks test (game+0x184 by side, its +0x40) is the
/// side's database team ([`World::side_teams`](crate::World::side_teams)):
/// Team Whitewater's side ([`crate::WHITEWATER`]) never pockets, before
/// any draw.
pub(super) fn pocket_check(ctx: &mut Ctx, team: usize) -> bool {
    use crate::game_class::Class;
    let class = ctx.w.class();
    if class == Class::GamebreakerWins {
        ctx.game.gb_cancel = false;
        if ctx.w.rules.goal != crate::Goal::Gamebreaker(2) {
            return false;
        }
        if !can_pocket(ctx, team) {
            if can_cancel(ctx, team) {
                ctx.game.gb_cancel = true;
            }
            return false;
        }
        if !can_cancel(ctx, team) {
            return true;
        }
        // `cvt.w.s` of the other side's meter in percent.
        let percent = (ctx.w.teams[1 - team].meter as f32 * 100.0 / 50_000.0) as i32;
        let chance = match percent {
            ..10 => 100,
            10..20 => 75,
            20..30 => 50,
            _ => 0,
        };
        let r = ctx.rand();
        if ((r % 100) as i32) < chance {
            return true;
        }
        ctx.game.gb_cancel = true;
        return false;
    }
    if whitewater(ctx, team) || !can_pocket(ctx, team) {
        return false;
    }
    let target = match ctx.w.rules.goal {
        crate::Goal::Points(n) | crate::Goal::TrickPoints(n) => i64::from(n),
        crate::Goal::Gamebreaker(_) => 0,
    };
    let [before, last] = ctx.w.stats.fills[team.min(1)];
    let gain = i64::from(last) - i64::from(before);
    let r = ctx.rand();
    let margin = if class == Class::TrickPoints {
        i64::from(r % 75_000) + 25_000
    } else {
        i64::from(r % 3) + 2
    };
    let short = |t: usize| i64::from(ctx.w.class_score(t)) + gain + margin < target;
    short(team) && short(1 - team)
}

/// The game's vt+0xcc, by the game object's class:
///
/// - the points, lesson and trick-point games (`FUN_0012ed28`,
///   `FUN_0018ca20`): once the other side has pocketed (game+0xfc), the
///   first time a cancel is possible it is asked once, 8% of the time, and
///   always for Team Whitewater's side (the draw still made);
/// - Gamebreaker Wins (`FUN_00145e70`), only to a Level 2: a cancel
///   possible and game+0x4c0 set by the pocket check, which it clears.
pub(super) fn cancel_check(ctx: &mut Ctx, team: usize) -> bool {
    if ctx.w.class() == crate::game_class::Class::GamebreakerWins {
        if ctx.w.rules.goal != crate::Goal::Gamebreaker(2) || !can_cancel(ctx, team) {
            return false;
        }
        let set = ctx.game.gb_cancel;
        ctx.game.gb_cancel = false;
        return set;
    }
    if !ctx.game.cancel[team] || !can_cancel(ctx, team) {
        return false;
    }
    let r = ctx.rand();
    ctx.game.cancel[team] = false;
    r % 100 < 8 || whitewater(ctx, team)
}

/// The side plays as Team Whitewater (its team record's type 0x2c).
fn whitewater(ctx: &Ctx, team: usize) -> bool {
    ctx.w.side_teams.get(team) == Some(&crate::WHITEWATER)
}

/// The shot check's quality gate as the capture comparison counts it:
/// the qualities reached (by value) and how many met the need.
#[cfg(test)]
pub(super) mod probe {
    use std::cell::RefCell;
    thread_local! {
        static GATE: RefCell<([u32; 8], u32)> = const { RefCell::new(([0; 8], 0)) };
    }
    pub fn count(quality: i32, need: i32) {
        GATE.with(|g| {
            let mut g = g.borrow_mut();
            g.0[quality.clamp(0, 7) as usize] += 1;
            if need <= quality {
                g.1 += 1;
            }
        });
    }
    /// The counts so far, cleared.
    pub fn take() -> ([u32; 8], u32) {
        GATE.with(|g| std::mem::take(&mut *g.borrow_mut()))
    }
}
