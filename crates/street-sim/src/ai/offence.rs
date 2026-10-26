//! The ball handler: the pad builder for shots and tricks
//! (`FUN_00182280`) and the behaviours that use it.

use super::court::{angle_toward, dist};
use super::pad::bits;
use super::words::words;
use super::{Act, Brain, Ctx, skill};
use crate::player::TRICK_SLOT_MASKS;

/// The combos' levels (`0x43d3b0` and `0x43d408`, the same eleven
/// `{mask, level}` rows for dunks and tricks).
const COMBO_LEVELS: [i32; 11] = [4, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2];

/// A rating as the original reads it (`FUN_001d7d00`, 0..99).
fn rating(r: f32) -> i32 {
    (r * 99.0).round().clamp(0.0, 99.0) as i32
}

/// `FUN_0015f098`: the trick level a handles rating earns (`0x43f080`:
/// 81 → 4, 61 → 3, 41 → 2, else 1).
pub(super) fn trick_level(handles: i32) -> i32 {
    match handles {
        h if h >= 81 => 4,
        h if h >= 61 => 3,
        h if h >= 41 => 2,
        _ => 1,
    }
}

/// `FUN_00169208`: the dunk level a dunks rating earns (`0x43f0c0`:
/// 81 → 4, 61 → 3, 41 → 2, 21 → 1, else 0).
pub(super) fn dunk_level(dunks: i32) -> i32 {
    match dunks {
        d if d >= 81 => 4,
        d if d >= 61 => 3,
        d if d >= 41 => 2,
        d if d >= 21 => 1,
        _ => 0,
    }
}

/// `FUN_0015eff0` / `FUN_00169260`: one of the combos of `level`, by one
/// draw.
fn pick_combo(ctx: &mut Ctx, level: i32) -> u32 {
    let rows: Vec<usize> = (0..11).filter(|&k| COMBO_LEVELS[k] == level).collect();
    let r = ctx.rand();
    if rows.is_empty() {
        return 0;
    }
    TRICK_SLOT_MASKS[rows[r as usize % rows.len()]]
}

/// The game modes the AI tells apart (the game record's +0xe8): 1 Trick
/// Point Challenge, 2 Dunks Only, 6 Gamebreaker Wins, else 0.
pub(super) fn mode(ctx: &Ctx) -> i32 {
    match ctx.w.rules.goal {
        crate::Goal::TrickPoints(_) => 1,
        crate::Goal::Gamebreaker(_) => 6,
        _ if ctx.w.rules.dunks_only => 2,
        _ => 0,
    }
}

/// game+0x49c: set in the trick-point and Gamebreaker Wins games
/// (`FUN_0013cf58`).
fn chasing_points(ctx: &Ctx) -> bool {
    matches!(mode(ctx), 1 | 6)
}

/// `FUN_00182280`: build the handler's buttons for a request (juke 0x10,
/// shoot 0x80, block 0x1000), adding the turbo combos the original adds.
pub(super) fn build(ctx: &mut Ctx, b: &mut Brain, i: usize, mut req: u32) {
    let team = ctx.team(i);
    let r = ctx.w.players[i].ratings;
    let mut lo = 2;
    if req & bits::JUKE != 0 {
        let mut top = trick_level(rating(r.handles));
        let go;
        if top < 2 {
            let p = skill::percent(ctx, team, 20, 10, 0, 0);
            go = ((ctx.rand() % 100) as i32) < p;
            if go {
                top = 4;
            }
        } else {
            go = if chasing_points(ctx) {
                let p = skill::percent(ctx, team, 60, 80, 80, 100);
                ((ctx.rand() % 100) as i32) < p
            } else {
                ctx.rand() % 100 < 80
            };
            if top < 4 && go {
                let roll = ctx.rand() % 100;
                let p = skill::percent(ctx, team, 20, 10, 0, 0);
                if (roll as i32) < p {
                    lo = top + 1;
                    top = 4;
                }
            }
        }
        if 0 < top && ctx.rand() % 100 < 50 {
            req |= bits::TURBO;
        }
        if go {
            let roll = ctx.rand();
            let span = (top - lo + 1).max(1) as u32;
            req |= pick_combo(ctx, lo + (roll % span) as i32);
        }
    }
    if req & bits::SHOOT != 0 {
        req = shot_combo(ctx, i, team, req, lo);
    }
    if req & bits::BLOCK != 0 {
        req |= bits::TURBO;
    }
    b.press(req);
}

/// The shoot half of `FUN_00182280`: a gamebreaker with a full meter, a
/// jump shot (or, in the modes that chase points, a pass) away from the
/// rim, a dunk combo by the dunks rating near it.
fn shot_combo(ctx: &mut Ctx, i: usize, team: usize, mut req: u32, mut lo: i32) -> u32 {
    let oop = big_dunk_spot(ctx, i);
    if ctx.meter_full(team) {
        return req | bits::TWO_TURBOS;
    }
    if !dunkable(ctx, i) {
        return if mode_pass(ctx, i) {
            (req & !bits::SHOOT) | bits::TURBO | bits::PASS
        } else {
            req | bits::TURBO
        };
    }
    let mut top = dunk_level(rating(ctx.w.players[i].ratings.dunks));
    let mut go;
    if top < 2 {
        let p = skill::percent(ctx, team, 20, 10, 0, 0);
        go = ((ctx.rand() % 100) as i32) < p;
        if go {
            if oop {
                top = 4;
                go = false;
                req |= TRICK_SLOT_MASKS[0];
            } else {
                top = 3;
            }
        }
    } else {
        if top == 4 && oop {
            go = false;
            req |= TRICK_SLOT_MASKS[0];
        } else if chasing_points(ctx) {
            req |= bits::TURBO;
            let p = skill::percent(ctx, team, 60, 80, 100, 100);
            go = ((ctx.rand() % 100) as i32) < p;
        } else {
            go = ctx.rand() % 100 < 80;
        }
        if top < 3 && go {
            let roll = ctx.rand() % 100;
            let p = skill::percent(ctx, team, 20, 10, 0, 0);
            if (roll as i32) < p {
                if oop {
                    top = 4;
                    go = false;
                    req |= TRICK_SLOT_MASKS[0];
                } else {
                    lo = top + 1;
                    top = 3;
                }
            }
        }
    }
    if 0 < top && ctx.rand() % 100 < 50 {
        req |= bits::TURBO;
    }
    if go {
        let top = top.min(3);
        let roll = ctx.rand();
        let span = (top - lo + 1).max(1) as u32;
        req |= pick_combo(ctx, lo + (roll % span) as i32);
    }
    req
}

/// `FUN_0016c500(actor, 1) == 2` ([`crate::World::dunk_on`]), whether a
/// dunk is on: within 15 of the rim facing it, moving, with no opponent
/// within 4.5 in the cone to the rim, or standing within 7.5. The pad
/// builder (`0x182494`) and `FUN_00183d90` (`0x183dc4`) both ask it.
pub(super) fn dunkable(ctx: &Ctx, i: usize) -> bool {
    ctx.w.dunk_on(i, true) == 2
}

/// `FUN_00183d90`: within 14 of the rim with a dunk on.
pub(super) fn dunk_close(ctx: &Ctx, i: usize) -> bool {
    ctx.hoop_dist(i) < 14.0 && dunkable(ctx, i)
}

/// `FUN_00183c78`: in the modes that score other than by baskets, pass
/// rather than take a jump shot while the shot clock shows more than 4:
/// Dunks Only always; the trick-point game while the side has not won it
/// (the game's vt+0x7c, `FUN_0018c698`: its trick points past the target
/// and the other side's); Gamebreaker Wins 80% of the time until the meter
/// is full.
fn mode_pass(ctx: &mut Ctx, i: usize) -> bool {
    if ctx.shot_clock() <= 4 {
        return false;
    }
    match mode(ctx) {
        2 => true,
        1 => !ctx.w.basket_would_win(ctx.team(i), 0, 0),
        6 if !ctx.meter_full(ctx.team(i)) => ctx.rand() % 100 <= 0x4f,
        _ => false,
    }
}

/// `FUN_001826f0`: a clear lane, 12.5..14 out on the wings with nobody
/// within 5: the spot for the level-4 dunk.
fn big_dunk_spot(ctx: &mut Ctx, i: usize) -> bool {
    if !path_open(ctx, i) {
        return false;
    }
    let d = ctx.hoop_dist(i);
    if !(12.5 < d && d < 14.0) || 6.0 <= ctx.pos(i)[1].abs() {
        return false;
    }
    let Some(o) = ctx.nth_opponent(i, 0, false) else {
        return false;
    };
    if ctx.between(i, o) <= 5.0 {
        return false;
    }
    let a = ctx.hoop_angle(i);
    (182 < a && a < 330) || (694 < a && a < 842)
}

/// `FUN_00182148(margin, half)`: on the baseline: the player or the ball
/// beyond `36.75 + margin` in x, and (unless within 8 of the rim) seen
/// from the rim within `half` degrees of the baseline's direction.
fn on_baseline(ctx: &Ctx, i: usize, margin: f32, half: f32) -> bool {
    let edge = margin + 36.75;
    if !(ctx.pos(i)[0].abs() > edge || ctx.w.ball.position[0].abs() > edge) {
        return false;
    }
    if ctx.hoop_dist(i) <= 8.0 {
        return true;
    }
    let centre = if ctx.side(ctx.team(i)) > 0 {
        270.0
    } else {
        90.0
    };
    let lo = ((centre - half) * 2.844_444 + 0.5) as i32;
    let hi = ((centre + half) * 2.844_444 + 0.5) as i32;
    let a = ctx.hoop_angle(i);
    lo <= a && a <= hi
}

/// `FUN_0017deb0(margin)`: outside the long-range line by `margin`
/// (`|z| ≥ max(margin, 0.5) + 22`, or 23.75 + margin from the rim).
pub(super) fn outside(ctx: &Ctx, i: usize, margin: f32) -> bool {
    let m = margin.max(0.5);
    !(ctx.pos(i)[1].abs() < m + 22.0 && ctx.hoop_dist(i) < margin + 23.75)
}

/// The angle that walks the handler off the baseline toward the court's
/// middle (`0x101004`: 768.4978, or 256.49927 when attacking +x).
fn off_baseline(ctx: &Ctx, i: usize) -> i32 {
    if ctx.side(ctx.team(i)) == 1 { 256 } else { 768 }
}

/// `FUN_00181aa8`: the handler can act: holding the ball, not passing,
/// jumping, juking or down.
pub(super) fn can_act(ctx: &Ctx, i: usize) -> bool {
    ctx.has_ball(i)
        && !ctx.catching(i)
        && !ctx.blocking(i)
        && !ctx.rebounding(i)
        && !ctx.fallen(i)
        && !ctx.tricking(i)
}

/// `FUN_001816b0`: dribbling (actor+0x1a0 1, [`crate::Player::dribble`]).
pub(super) fn dribbling(ctx: &Ctx, i: usize) -> bool {
    ctx.w.players[i].dribble == 1
}

/// `FUN_00181508(ai, 0)`: in the middle of a shot, dunk or lay-up (or a
/// leap for an alley-oop), or, for everyone, while a pass to himself is in
/// the air (raw `0x181608..0x181634`: ball state 4 with its last holder,
/// ball+0xfc, its target, +0x100: Off the Heezay, Back 2 Papa, a trick's
/// own toss).
pub(super) fn busy(ctx: &Ctx, i: usize) -> bool {
    ctx.shooting(i)
        || ctx.dunking(i)
        || ctx.laying_up(i)
        || (ctx.leaping(i) && ctx.has_ball(i))
        || matches!(ctx.w.players[i].action, crate::Action::PumpFake { .. })
        || (ctx.w.ball.state == crate::BallState::Pass
            && ctx.w.ball.pass.is_some_and(|p| p.from == p.to))
        || ctx.w.windup.is_some_and(|w| w.from == w.to)
}

/// `FUN_001819f8`: juking (bit 10), or asking for a juke with turbo to
/// spare.
pub(super) fn juking(ctx: &Ctx, b: &Brain, i: usize) -> bool {
    ctx.tricking(i) || (ctx.has_ball(i) && b.pad.buttons & bits::JUKE != 0 && ctx.turbo(i) >= 0.2)
}

/// A defender as the contest (`FUN_001801e0`) reads him, against shooter
/// `i`.
pub(super) fn contest_view(ctx: &Ctx, i: usize, j: usize) -> crate::shooting::ContestDefender {
    crate::shooting::ContestDefender {
        distance_to_shooter: ctx.between(i, j),
        distance_to_rim: ctx.hoop_dist(j),
        height_ft: ctx.height_ft(j),
        blocks: ctx.rating(j, 5),
    }
}

/// `FUN_001805e8`: the shot's quality, 0..7 (`shooting`'s port), against
/// the nearest two defenders who are not down, as `FUN_0017fd08` walks
/// the three nearest opponents.
pub(super) fn quality(ctx: &Ctx, i: usize) -> i32 {
    let team = ctx.team(i);
    let mut up = (0..3)
        .filter_map(|n| ctx.nth_opponent(i, n, false))
        .filter(|&j| !ctx.fallen(j));
    let (first, second) = (up.next(), up.next());
    let shot = crate::shooting::ShotView {
        distance_to_rim: ctx.hoop_dist(i),
        shots: ctx.rating(i, 0),
        threes: ctx.rating(i, 1),
        dunks: ctx.rating(i, 2),
        outside: outside(ctx, i, 0.5),
        meter_full: ctx.meter_full(team),
        height_ft: ctx.height_ft(i),
    };
    crate::evaluate_shot_quality_tier(
        &shot,
        first.map(|j| contest_view(ctx, i, j)),
        second.map(|j| contest_view(ctx, i, j)),
    )
}

/// `FUN_0017f4d0(ai, 0)`: no opponent in the cone from behind the player
/// toward the rim. The cone's apex sits `r` behind the player, where
/// `r = lo · min(0.9 d, 20) / (hi − lo)`, `lo..hi` widening from 4..6
/// with the ball (5..7 to 6..8 by distance without), and its half-angle
/// is `atan(hi / (min(0.9 d, 20) + r))`, at most 171. Opponents count
/// when nearer than `min(0.9 d, 20)`, no more than a step farther from
/// the rim, and not down (nor in a block jump, unless the player is busy).
pub(super) fn path_open(ctx: &Ctx, i: usize) -> bool {
    path_open_ignoring(ctx, i, None)
}

/// `FUN_0017f4d0(ai, list)`: with a list of the defenders' men (the
/// defense strategy's `[3..5]`), the defenders on the player (other than
/// the one in his own slot) do not count, and any other within 3 shuts
/// the lane.
pub(super) fn path_open_ignoring(ctx: &Ctx, i: usize, men: Option<&[i32; 3]>) -> bool {
    let d = ctx.hoop_dist(i);
    if d < 3.0 {
        return true;
    }
    let (mut lo, mut hi) = if ctx.has_ball(i) {
        (4.0, 6.0)
    } else if d <= 8.0 {
        (5.0, 7.0)
    } else if 20.0 <= d {
        (6.0, 8.0)
    } else {
        let t = (d - 8.0) / 12.0;
        (t + 5.0, t + 7.0)
    };
    if lo < 1.0 {
        lo = 1.0;
    }
    if hi - lo < 0.5 {
        hi = lo + 0.5;
    }
    let reach = (d * 0.9).min(20.0);
    let r = (lo * reach / (hi - lo)).min(1000.0);
    let back = r + d;
    let a = ctx.hoop_angle(i);
    let rim = ctx.goal(ctx.team(i));
    let apex = [
        rim[0] - back * crate::input::native_sin_1024(a),
        rim[1] - back * crate::input::native_cos_1024(a),
    ];
    let half = super::court::units(crate::math::atan2(hi, reach + r)).min(171);
    let to_rim = ctx.angle_to_hoop(i);
    let own = ctx.slot(i);
    for (k, j) in ctx.squad(1 - ctx.team(i)).into_iter().enumerate() {
        if let Some(men) = men
            && k != own
            && men.get(k) == Some(&(i as i32))
        {
            continue;
        }
        if (ctx.blocking(j) && !busy(ctx, i)) || ctx.fallen(j) {
            continue;
        }
        let dj = ctx.between(i, j);
        if men.is_some() && dj < 3.0 {
            return false;
        }
        if dj < reach && ctx.hoop_dist(j) - 1.0 < d {
            let seen = angle_toward(apex, ctx.pos(j));
            if super::court::turn(seen, to_rim).abs() <= half {
                return false;
            }
        }
    }
    true
}

/// `FUN_0017fd90`: how open a player of the team with the ball is, 0..7:
/// the table at `0x3da4f8` (with the ball) or `0x3da5d0` (without) by the
/// distance to the rim and to the nearest standing opponent, less a step
/// for a second opponent within 8 and, without the ball, for a closer
/// option for the handler, each opponent in the passing lane and a long
/// shot by a poor three-point shooter.
pub(super) fn openness(ctx: &Ctx, i: usize) -> i32 {
    let team = ctx.team(i);
    if ctx.offence() != team {
        return 0;
    }
    let p = ctx.pos(i);
    if !(p[1].abs() <= 27.5 && p[0].abs() <= 43.0) {
        return 0;
    }
    let standing: Vec<usize> = ctx
        .by_distance(i)
        .into_iter()
        .filter(|&j| ctx.team(j) != team)
        .take(3)
        .filter(|&j| !ctx.fallen(j))
        .collect();
    let Some(&first) = standing.first() else {
        return 7;
    };
    let second = standing.get(1).copied();
    let has = ctx.has_ball(i);
    let holder = ctx.w.handler();
    let far = match holder {
        None => ctx.ball_dist(i),
        Some(h) if !has => ctx.between(i, h),
        Some(_) => 1.0,
    };
    let d = ctx.hoop_dist(i);
    let row = bucket(d, &[8.0, 19.0, 23.75, 31.75, 42.0, 100.0], 5);
    let mut gap = ctx.between(i, first);
    if !has && holder.is_some() {
        if 10.0 < far {
            gap *= ((120.0 - (far + far)) / 100.0).max(0.5);
        } else {
            gap = gap * (15.0 - far) / 10.0;
        }
    }
    let col = bucket(
        gap,
        &[4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.5, 20.0, 100.0],
        8,
    );
    let table = if has { &OPEN_WITH } else { &OPEN_WITHOUT };
    let mut v = table[row * 9 + col];
    if second.is_some_and(|j| ctx.between(i, j) < 8.0) {
        v -= 1;
    }
    if let (false, Some(h)) = (has, holder) {
        let other = ctx
            .squad(team)
            .into_iter()
            .find(|&j| j != i && j != h)
            .unwrap_or(h);
        if v < 6 && ctx.between(h, other) < far - 10.0 {
            v -= 1;
        }
        for k in ctx.squad(1 - team) {
            if in_lane(ctx, i, k) {
                v -= 1;
            }
        }
        if setting_pick(ctx, i) {
            v -= 1;
        }
        if ctx.game.phase == 5 && 19.0 <= d && rating(ctx.w.players[i].ratings.threes) < 50 {
            v -= 1;
        }
    }
    v.clamp(0, 7)
}

/// `FUN_001332e0(ai + 0xc) == 1`: the player's front behaviour is "set
/// pick".
fn setting_pick(ctx: &Ctx, i: usize) -> bool {
    ctx.setting_pick(i)
}

fn bucket(v: f32, bounds: &[f32], max: usize) -> usize {
    if v <= bounds[0] {
        return 0;
    }
    let mut k = 1;
    while k < max && bounds[k] < v {
        k += 1;
    }
    k
}

/// `FUN_00180a98`: opponent `k` stands in the passing lane between the
/// player and the ball.
fn in_lane(ctx: &Ctx, i: usize, k: usize) -> bool {
    let mut reach = ctx.ball_dist(i);
    if reach <= 20.0 {
        reach = reach.min(10.0);
    } else {
        reach *= 0.5;
    }
    let d = ctx.between(i, k);
    if d >= reach {
        return false;
    }
    let b = ctx.w.ball.position;
    let to_ball = angle_toward(ctx.pos(i), [b[0], b[2]]);
    let a = super::court::turn(to_ball, ctx.angle_between(i, k)).abs();
    if d < reach * 0.5 { a < 0x72 } else { a < 71 }
}

/// Openness with the ball (`0x3da4f8`): rows by distance to the rim,
/// columns by distance to the nearest opponent.
const OPEN_WITH: [i32; 54] = [
    1, 3, 6, 7, 7, 7, 7, 7, 7, 0, 2, 4, 5, 6, 6, 7, 7, 7, 0, 1, 3, 5, 6, 6, 7, 7, 7, 0, 0, 3, 5, 5,
    6, 7, 7, 7, 0, 0, 3, 5, 5, 6, 6, 7, 7, 0, 0, 3, 5, 5, 6, 6, 7, 7,
];
/// Openness without it (`0x3da5d0`).
const OPEN_WITHOUT: [i32; 54] = [
    1, 3, 5, 6, 7, 7, 7, 7, 7, 0, 2, 4, 5, 6, 6, 7, 7, 7, 0, 1, 3, 5, 5, 6, 7, 7, 7, 0, 0, 3, 4, 5,
    5, 7, 7, 7, 0, 0, 3, 4, 5, 5, 6, 7, 7, 0, 0, 3, 4, 5, 5, 6, 7, 7,
];

/// `FUN_00182878` (raw `0x182878..0x182c54`): whether a juke toward `at`
/// is worth it: shot clock at least 4, no person on the team, the ball
/// held 5 frames, turbo over 0.2, not out of a dribble he faked
/// (`FUN_00181750`, actor+0x1a0 below 0), no clear lane, on the floor, and
/// not standing still or in his catch (bit 9, `FUN_001816d0`) more than 17
/// out; then an opponent within 4.5 (4.9 after a recent event,
/// [`recent_event`]) or within 7 (7.5) and in the way (±142 units of the
/// line to `at`). An opponent within 7.5 in a counter stance
/// (`FUN_00181828`) puts it off unless `skill(0, 0, 4, 4)` is under 2.
pub(super) fn should_juke(ctx: &Ctx, i: usize, at: [f32; 2]) -> bool {
    if ctx.shot_clock() < 4 || ctx.human[ctx.team(i)] {
        return false;
    }
    if ctx.game.held < 5 || ctx.turbo(i) <= 0.2 {
        return false;
    }
    if ctx.w.players[i].dribble < 0 || path_open(ctx, i) || ctx.airborne(i) {
        return false;
    }
    if (!ctx.w.players[i].moving() || ctx.catching(i)) && ctx.hoop_dist(i) > 17.0 {
        return false;
    }
    let toward = angle_toward(ctx.pos(i), at);
    let team = ctx.team(i);
    for j in ctx.squad(1 - team) {
        let d = ctx.between(i, j);
        let (near, wide) = if chasing_points(ctx) && ctx.side(team) as f32 * ctx.pos(i)[0] > 0.0 {
            (skill::percent(ctx, team, 5, 8, 10, 30) as f32, 12.5)
        } else if recent_event(ctx, i) {
            (f32::from_bits(0x409c_cccd), 7.5)
        } else {
            (4.5, 7.0)
        };
        if d < 7.5 && ctx.countering(j) && skill::percent(ctx, team, 0, 0, 4, 4) >= 2 {
            return false;
        }
        if d < near {
            return true;
        }
        if d < wide && super::court::turn(ctx.angle_between(i, j), toward).abs() <= 142 {
            return true;
        }
    }
    false
}

/// `FUN_00181c68` (raw `0x181c68..0x181f48`): pass out of a jump shot or
/// lay-up (in the air at least 1 up) when a blocker closes in, or from the
/// baseline inside the arc. Never while a gamebreaker is in play
/// (`FUN_00140ff8`: game+0x434's object, made with ball+0x178 by
/// `FUN_001422f8`, busy; the rewrite's gamebreaker is the shot itself), and
/// dunks and alley-oop leaps (bits 7 and 15) are never passed out of.
/// Within 4 of the rim only a jump shot whose ball is
/// no more than 12 up and within 1.2 of the rim across the floor (ai+0xa0,
/// the rim's z taken as 0) is passed out of. The blocker must be the
/// nearest opponent, in a block jump within 6, no farther from the rim,
/// with the highest of his posed hands and head (`FUN_00181f50`) less than
/// 1 under the ball, and within 156 of the line to the rim.
fn should_pass(ctx: &Ctx, i: usize, careful: bool) -> bool {
    if matches!(
        ctx.w.players[i].action,
        crate::Action::Shooting {
            gamebreaker: 1..,
            ..
        }
    ) {
        return false;
    }
    if !busy(ctx, i) || ctx.w.players[i].height < 1.0 || ctx.shot_clock() <= 3 {
        return false;
    }
    if ctx.dunking(i) || ctx.leaping(i) {
        return false;
    }
    if ctx.hoop_dist(i) < 4.0 {
        // `0x181d3c..0x181dac`.
        if ctx.laying_up(i) {
            return false;
        }
        let b = ctx.w.ball.position;
        let rim = ctx.goal(ctx.team(i));
        return b[1] <= 12.0 && (rim[0] - b[0]).powi(2) + b[2] * b[2] < 1.44;
    }
    if on_baseline(ctx, i, 0.7, 85.0) && !outside(ctx, i, 0.5) {
        return true;
    }
    // Nor in the Block lessons (`0x181df8..0x181e38`).
    if careful || matches!(ctx.w.lesson, Some(9 | 11)) {
        return false;
    }
    let Some(o) = ctx.nth_opponent(i, 0, false) else {
        return false;
    };
    if !ctx.blocking(o) || ctx.between(i, o) > 6.0 || ctx.hoop_dist(o) > ctx.hoop_dist(i) {
        return false;
    }
    // `FUN_00181f50`: his posed hands and head (bones 15, 19, 21); without
    // the disc's animations the rewrite's standing reach above his feet.
    let hand = ctx
        .w
        .top_bone(o)
        .unwrap_or(ctx.w.players[o].height + ctx.w.players[o].ratings.reach);
    if hand + 1.0 <= ctx.w.ball.position[1] {
        return false;
    }
    super::court::turn(ctx.angle_to_hoop(i), ctx.angle_between(i, o)).abs() < 156
}

/// `FUN_00182028`: the teammate to pass to: the most open within 31.75 of
/// the rim (the nearer on a tie), else the nearest.
fn pass_target(ctx: &Ctx, i: usize) -> Option<usize> {
    let mut best: Option<(usize, i32, f32)> = None;
    for j in ctx.squad(ctx.team(i)) {
        if j == i || ctx.hoop_dist(j) >= 31.75 {
            continue;
        }
        let open = openness(ctx, j);
        let d = ctx.between(i, j);
        best = match best {
            Some((_, o, bd)) if open < o || (open == o && bd <= d) => best,
            _ => Some((j, open, d)),
        };
    }
    best.map(|(j, ..)| j).or_else(|| ctx.nth_teammate(i, 0))
}

words! {
    /// "shoot" (type 9; `FUN_00100e38`, update `FUN_00100fe0`).
    pub(crate) struct Shoot {
        done: bool,
        /// A pump fake is under way.
        faking: bool,
        /// The pump fake has been used.
        faked: bool,
        /// Wait for a better look before letting it go.
        patient: bool,
        first: bool,
        /// Pass out of a blocked shot only when careful (`[0xc]`).
        careful: bool,
        /// Step back behind the arc first.
        step_back: bool,
        /// Look once for a kick-out.
        look: bool,
    }
}

impl Shoot {
    /// The constructor's draws: the patience (1 in 4 skip it, unless the
    /// shooter is nearest the rim; always in the Block lessons) and the
    /// care (`skill(40, 50, 50, 100)`).
    pub fn new(ctx: &mut Ctx, i: usize) -> Self {
        let team = ctx.team(i);
        let nearest = ctx.nearest_to_rim() == Some(i);
        // The Block lessons (game vt+0xb4 9, 11) always wait, with no draw
        // (`0x100e94..0x100ed0`).
        let patient = matches!(ctx.w.lesson, Some(9 | 11)) || (!nearest && ctx.rand() & 3 != 0);
        let p = skill::percent(ctx, team, 40, 50, 50, 100);
        let careful = ((ctx.rand() % 100) as i32) < p;
        let step_back = !outside(ctx, i, 0.5) && rating(ctx.w.players[i].ratings.threes) >= 50;
        Self {
            first: true,
            look: true,
            patient,
            careful,
            step_back,
            ..Default::default()
        }
    }

    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        self.done || !ctx.has_ball(i)
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if juking(ctx, b, i) {
            self.faking = false;
            self.first = true;
            return;
        }
        if self.first && on_baseline(ctx, i, -2.0, 80.0) {
            b.steer(ctx, i, off_baseline(ctx, i), 3, 1, None);
            return;
        }
        self.first = false;
        if self.faking {
            if !busy(ctx, i) {
                self.faking = false;
            }
            return;
        }
        // `FUN_0013abf8`: the stick's strength class, moving above 15. In
        // the Block lessons the wait ends at once, moving or not
        // (`0x1010e0..0x101134`).
        let block_lesson = matches!(ctx.w.lesson, Some(9 | 11));
        if self.patient && ((b.pad.strength > 15 && quality(ctx, i) > 5) || block_lesson) {
            self.patient = false;
            b.stop(ctx, i);
            return;
        }
        if self.stepping_back(ctx, i) {
            b.steer(ctx, i, ctx.hoop_angle(i), 3, 1, None);
            return;
        }
        if self.kick_out(ctx, i) {
            if let Some(o) = ctx.nth_opponent(i, 0, true) {
                b.face(ctx, i, ctx.angle_between(o, i), 3, 0);
            }
            return;
        }
        // `FUN_001816b0`: not while dribbling (actor+0x1a0 1); never in
        // the Block lessons (`0x101188..0x1011c8`).
        if !self.faked
            && !block_lesson
            && can_act(ctx, i)
            && !dribbling(ctx, i)
            && quality(ctx, i) < 5
            && ctx.rand() & 0xf == 0
        {
            self.faking = true;
            self.faked = true;
            b.stop(ctx, i);
            b.press(bits::SHOOT);
            return;
        }
        if !ctx.human[ctx.team(i)] && should_pass(ctx, i, !self.careful) {
            self.done = true;
            let to = pass_target(ctx, i);
            b.push(Act::Pass(Pass::new(ctx, i, to)));
            return;
        }
        if !can_act(ctx, i) {
            return;
        }
        if !mode_pass(ctx, i) {
            build(ctx, b, i, bits::SHOOT);
            if ctx.hoop_dist(i) >= 14.0 {
                return;
            }
            let p = skill::percent(ctx, ctx.team(i), 0, 50, 100, 100);
            if p <= (ctx.rand() % 100) as i32 {
                return;
            }
        } else if dunk_close(ctx, i) {
            build(ctx, b, i, bits::SHOOT);
        } else {
            self.done = true;
            let drive = Drive::new(ctx, i);
            b.push(Act::Drive(drive));
            return;
        }
        let a = ctx.angle_to_hoop(i);
        b.face(ctx, i, a, 1, 0);
        b.steer(ctx, i, a, 3, 1, None);
    }

    /// `FUN_001013f8`: step back to the arc from 20.75..24.25 out while
    /// the shot clock allows and nobody is within 14.
    fn stepping_back(&mut self, ctx: &Ctx, i: usize) -> bool {
        if !self.step_back {
            return false;
        }
        if ctx.shot_clock() < 3 || busy(ctx, i) || !outside(ctx, i, -3.0) || outside(ctx, i, 0.5) {
            self.step_back = false;
            return false;
        }
        let team = ctx.team(i);
        for j in ctx
            .by_distance(i)
            .into_iter()
            .filter(|&j| ctx.team(j) != team)
            .take(3)
        {
            if ctx.between(i, j) < 14.0 && !ctx.fallen(j) {
                self.step_back = false;
                return false;
            }
        }
        true
    }

    /// `FUN_00101520`: once, with the shot clock under 5 and more than 14
    /// out, a defender within 15 and nearer the rim who would contest
    /// (`FUN_001801e0`).
    fn kick_out(&mut self, ctx: &Ctx, i: usize) -> bool {
        if !std::mem::take(&mut self.look) || ctx.shot_clock() >= 5 || ctx.hoop_dist(i) <= 14.0 {
            return false;
        }
        let Some(o) = ctx.nth_opponent(i, 0, true) else {
            return false;
        };
        if ctx.between(i, o) >= 15.0 || ctx.hoop_dist(o) >= ctx.hoop_dist(i) + 1.0 {
            return false;
        }
        let defender = contest_view(ctx, i, o);
        crate::shooting::evaluate_contest_angle(ctx.height_ft(i), ctx.hoop_dist(i), &defender) > 0
    }
}

words! {
    /// "pass" (type 8; `FUN_00100a08`, enter `FUN_00100ae0`, update
    /// `FUN_00100bb0`).
    pub(crate) struct Pass {
        /// The receiver, or -1.
        to: i32,
        /// Turn to the receiver first (a long pass from far out).
        turn_first: bool,
        toggle: bool,
        /// The frames the ball must be held before passing.
        wait: i32,
    }
}

impl Pass {
    /// The constructor draws the frames the ball must have been held
    /// before the pass goes: `r % (fps × 0.05) + fps × 0.1`, 6..8
    /// (`0x100a54..0x100acc`).
    pub fn new(ctx: &mut Ctx, _i: usize, to: Option<usize>) -> Self {
        let r = ctx.rand();
        Self {
            to: to.map_or(-1, |j| j as i32),
            wait: (r % 3) as i32 + 6,
            ..Default::default()
        }
    }

    pub fn enter(&mut self, ctx: &Ctx, i: usize) {
        self.turn_first =
            self.to >= 0 && 19.0 < ctx.hoop_dist(i) && 15.0 <= ctx.between(i, self.to as usize);
    }

    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        self.to < 0 || self.to as usize == i || !ctx.has_ball(i)
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if ctx.catching(i) {
            return;
        }
        if self.turn_first {
            b.face_point(ctx, i, ctx.pos(self.to as usize), 1, 0);
            self.turn_first = false;
            return;
        }
        if !ctx.live() || self.wait < ctx.game.held {
            b.stop(ctx, i);
            self.toggle = !self.toggle;
            if self.toggle {
                b.pad.target = self.to;
                b.press(bits::PASS);
            }
        }
    }
}

words! {
    /// "carrier pause" (type 3; `FUN_00101948`, enter `FUN_001019d8`,
    /// update `FUN_00101b50`): dribble near a spot, facing the rim,
    /// stepping away from the defender, juking him when he is close.
    pub(crate) struct CarrierPause {
        /// How far toward the rim the spot may be (`[0x1c]`).
        reach: f32,
        /// The juke odds' table (`[0x20]`: 1 is `(0, 40, 40, 100)`).
        eager: bool,
        /// The margin kept from the court's edge (`[0x24]`).
        margin: f32,
        spot: [f32; 2],
        step: [f32; 2],
        stepping: bool,
        wait: i32,
        faked: bool,
        timer: i32,
        backing: bool,
    }
}

impl CarrierPause {
    pub fn new(reach: f32, margin: f32, eager: bool) -> Self {
        Self {
            reach,
            margin,
            eager,
            ..Default::default()
        }
    }

    /// `FUN_001019d8`: the spot is the handler's place, or `reach` toward
    /// the rim, kept `margin` inside 42 × 26.5.
    pub fn enter(&mut self, ctx: &Ctx, i: usize) {
        let p = ctx.pos(i);
        let rim = ctx.goal(ctx.team(i));
        let d = ctx.hoop_dist(i);
        self.spot = if d <= self.reach {
            rim
        } else {
            let k = self.reach / d;
            [p[0] + (rim[0] - p[0]) * k, p[1] + (rim[1] - p[1]) * k]
        };
        let m = self.margin;
        self.spot[0] = self.spot[0].clamp(m - 42.0, 42.0 - m);
        self.spot[1] = self.spot[1].clamp(m - 26.5, 26.5 - m);
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if ctx.catching(i) || juking(ctx, b, i) {
            return;
        }
        // `0x101b8c..0x101bd8`: in the paint, unless he is dribbling
        // (`FUN_001816b0`), a step and nothing else.
        if ctx.zone(i) >= 5 && !dribbling(ctx, i) {
            self.choose_step(ctx, b, i);
            if self.stepping {
                b.move_to(ctx, i, self.step, 3, 2);
            }
            return;
        }
        if self.tick(ctx, b, i) {
            if self.timer > 0 {
                self.timer -= 1;
            } else {
                self.timer = 48; // fps × 0.8
            }
        }
    }

    /// One decision; true when it ends at the timer (`0x101f10`).
    fn tick(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) -> bool {
        if self.faked {
            if !busy(ctx, i) {
                self.faked = false;
            }
            return false;
        }
        if self.stepping {
            b.move_to(ctx, i, self.step, 3, 2);
            self.stepping = 1.4 < dist(ctx.pos(i), self.step);
            return false;
        }
        if self.wait > 0 {
            self.wait -= 1;
            if !self.back_in(ctx, b, i) {
                b.stop(ctx, i);
            }
            return false;
        }
        let open = openness(ctx, i);
        let gap = ctx
            .nth_opponent(i, 0, false)
            .map_or(100.0, |o| ctx.between(i, o));
        if self.timer == 0
            && can_act(ctx, i)
            && !dribbling(ctx, i)
            && !path_open(ctx, i)
            && 2 < open
            && open < 6
            && ctx.rand() & 7 == 0
        {
            self.faked = true;
            b.stop(ctx, i);
            b.press(bits::SHOOT);
            return true;
        }
        if 7.5 < gap || 2 < open {
            self.wait = 6; // fps × 0.1
            if self.back_in(ctx, b, i) {
                return true;
            }
            let facing = super::court::turn(ctx.angle_to_hoop(i), ctx.facing(i)).abs() <= 171;
            if !facing && 3.0 < ctx.hoop_dist(i) && 8.5 < gap {
                b.face_point(ctx, i, ctx.goal(ctx.team(i)), 1, 0);
            } else {
                b.stop(ctx, i);
            }
            return true;
        }
        self.backing = false;
        let o = ctx.nth_opponent(i, 0, false);
        let at = match o {
            Some(o) if ctx.hoop_dist(o) < ctx.hoop_dist(i) => ctx.pos(o),
            _ => ctx.goal(ctx.team(i)),
        };
        if should_juke(ctx, i, at) {
            let r = ctx.rand();
            let p = juke_odds(ctx, i, self.eager);
            if ((r % 100) as i32) < p {
                b.face_point(ctx, i, at, 1, 0);
                build(ctx, b, i, bits::JUKE);
                return true;
            }
        }
        self.choose_step(ctx, b, i);
        if self.stepping {
            b.move_to(ctx, i, at, 2, 2);
        }
        true
    }

    /// `FUN_00101f60`: walk in toward the rim while the defender stays
    /// more than 11 away (until he closes within 9), unless within 3.
    fn back_in(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) -> bool {
        let gap = ctx
            .nth_opponent(i, 0, false)
            .map_or(100.0, |o| ctx.between(i, o));
        self.backing = if self.backing { gap > 9.0 } else { gap > 11.0 };
        if ctx.hoop_dist(i) < 3.0 {
            self.backing = false;
        }
        if self.backing {
            b.move_to(ctx, i, ctx.goal(ctx.team(i)), 1, 1);
        }
        self.backing
    }

    /// `FUN_00102030`: a step `margin` from the spot, away from the
    /// defender give or take 107 units (one draw).
    fn choose_step(&mut self, ctx: &mut Ctx, _b: &mut Brain, i: usize) {
        let o = ctx.nth_opponent(i, 0, false);
        let from = o.map_or(self.spot, |o| ctx.pos(o));
        let a = angle_toward(self.spot, from);
        let r = ctx.rand();
        let a = (a + (r % 0x1ab) as i32 + 299) & 0x3ff;
        let d = super::court::direction(a);
        self.step = [
            self.spot[0] + self.margin * d[0],
            self.spot[1] + self.margin * d[1],
        ];
        self.stepping = 4.0 <= dist(ctx.pos(i), self.step);
    }
}

/// `FUN_001023c8` (through `FUN_00183be8`): the juke odds, `(3, 18, 20,
/// 100)` or, when eager, `(0, 40, 40, 100)`, and `skill(0, 5, 40, 40)`
/// more after a recent event ([`recent_event`]).
fn juke_odds(ctx: &Ctx, i: usize, eager: bool) -> i32 {
    let team = ctx.team(i);
    let odds = if eager {
        skill::percent(ctx, team, 0, 40, 40, 100)
    } else {
        skill::percent(ctx, team, 3, 18, 20, 100)
    };
    if recent_event(ctx, i) {
        odds + skill::percent(ctx, team, 0, 5, 40, 40)
    } else {
        odds
    }
}

/// `FUN_00183af8`: the combo ring is `i`'s side's (+0x144), the ball is in
/// the half it attacks (its x against `FUN_0017dcf0`'s side), and one of
/// the ring's first 22 event classes (`FUN_00132818`: the newest slot of
/// that class, the walk stopping at an empty one) was pushed within
/// `fps × 2` updates (`DAT_004b9d98`).
pub(super) fn recent_event(ctx: &Ctx, i: usize) -> bool {
    let team = ctx.team(i);
    let manager = &ctx.w.combo;
    if manager.side != team {
        return false;
    }
    let side = ctx.side(team);
    if (ctx.w.ball.position[0] >= 0.0) != (side == 1) {
        return false;
    }
    let now = ctx.w.scoring_now();
    let window = crate::TICK_HZ as i32 * 2;
    (0..22).any(|class| {
        let newest = manager
            .ring
            .slots
            .iter()
            .rev()
            .take_while(|s| !s.is_empty())
            .find(|s| s.class == class);
        newest.is_some_and(|s| now - s.tick < window)
    })
}

words! {
    /// "drive" (type 5; `FUN_001095d8`, enter `FUN_001096b8`, update
    /// `FUN_00109898`): run at the rim and go up when close enough.
    pub(crate) struct Drive {
        done: bool,
        /// Committed to the shot.
        committed: bool,
        /// Go up within this distance (5..16, drawn).
        reach: f32,
        /// Frames to the next juke decision.
        timer: i32,
        avoid: i32,
        /// Pass out of trouble only when careful.
        careful: bool,
        /// Try for the big dunk from the wing.
        big: bool,
        /// The rim spot, 0.79 short of the centre.
        target: [f32; 2],
        /// +0x20: the patient start (`FUN_00109df8`): beat a defender
        /// first.
        patient: bool,
        /// +0x24: the defender to beat, or -1.
        beat: i32,
        /// +0x28: in the paint, post up instead (`FUN_00115368`).
        post: bool,
        /// +0x60: a dunk met by a taller defender close (`FUN_001837d0`):
        /// shoot pressed every other frame (`0x3c6f78`).
        release: bool,
    }
}

impl Drive {
    /// `FUN_001095d8(ai, 0)`: one draw for `[0xf]`.
    pub fn new(ctx: &mut Ctx, i: usize) -> Self {
        let careful = ctx.rand() % 100 < 0x19;
        let mut target = ctx.goal(ctx.team(i));
        target[0] -= if 0.0 <= target[0] {
            0.793_225
        } else {
            -0.793_225
        };
        Self {
            avoid: 1,
            careful,
            target,
            beat: -1,
            ..Default::default()
        }
    }

    /// `FUN_001095d8(ai, 1)`: the patient drive, which first beats the
    /// nearest standing defender ([`Drive::patient_start`]).
    pub fn patient(ctx: &mut Ctx, i: usize) -> Self {
        Self {
            patient: true,
            ..Self::new(ctx, i)
        }
    }

    /// `FUN_001096b8`: a drive started (not patiently) in the paint
    /// without a full meter posts up instead ("post up", `FUN_00115368`,
    /// on its first update); otherwise draw the distance to go up from
    /// (5..16). Then the big-dunk flag (`skill(0, 10, 100, 100)`, never
    /// under 40 Dunks, 90% of the time not under 65).
    pub fn enter(&mut self, ctx: &mut Ctx, i: usize) {
        let team = ctx.team(i);
        if !self.patient && 4 < ctx.zone(i) && !ctx.meter_full(team) {
            self.post = true;
        } else {
            let r = ctx.rand();
            let unit = (r & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000);
            self.reach = unit * 11.0 + 5.0;
        }
        let p = skill::percent(ctx, team, 0, 10, 100, 100);
        self.big = ((ctx.rand() % 100) as i32) < p;
        let dunks = rating(ctx.w.players[i].ratings.dunks);
        if dunks < 40 || (dunks < 65 && ctx.rand() % 100 < 0x5a) {
            self.big = false;
        }
    }

    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        self.done || !ctx.has_ball(i)
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        // `FUN_00109898`: the post-up handed on, then the patient start
        // until the handler is open over 6.
        if self.post {
            self.done = true;
            let p = PostUp::new(ctx);
            b.push(Act::PostUp(p));
            return;
        }
        if self.patient {
            if 6 < openness(ctx, i) {
                self.patient = false;
            } else {
                self.patient_start(ctx, b, i);
            }
            return;
        }
        if juking(ctx, b, i) {
            self.committed = false;
            self.timer = 4; // fps × 0.07
            return;
        }
        let mut still = false;
        if self.committed {
            // Raw `0x10996c..0x109a18`: a pass out of a blocked shot; else,
            // in a dunk a taller defender meets, shoot pressed every other
            // frame from the next (`FUN_0013b7d8`, the press alone); else
            // the shot held.
            if should_pass(ctx, i, !self.careful) {
                let to = pass_target(ctx, i);
                b.push(Act::Pass(Pass::new(ctx, i, to)));
                self.done = true;
            } else if self.release {
                ctx.game.toggle = !ctx.game.toggle;
                if ctx.game.toggle {
                    b.press(bits::SHOOT);
                }
            } else if ctx.dunking(i) && dunk_blocked(ctx, i) {
                self.release = true;
            } else {
                build(ctx, b, i, bits::SHOOT);
            }
        } else if ctx.hoop_dist(i) < self.reach {
            self.reach = 100.0;
            if on_baseline(ctx, i, 0.0, 80.0) {
                b.steer(ctx, i, off_baseline(ctx, i), 3, 1, None);
                return;
            }
            if can_act(ctx, i) {
                if !path_open(ctx, i) && quality(ctx, i) < 3 {
                    self.done = true;
                } else if mode_pass(ctx, i) {
                    if dunk_close(ctx, i) {
                        build(ctx, b, i, bits::SHOOT);
                        still = true;
                        let a = ctx.angle_to_hoop(i);
                        b.face(ctx, i, a, 1, 0);
                        b.steer(ctx, i, a, 3, 1, None);
                    }
                } else {
                    self.committed = true;
                    build(ctx, b, i, bits::SHOOT);
                }
            }
        } else if self.big && big_dunk_spot(ctx, i) {
            self.reach = 100.0;
            self.committed = true;
            build(ctx, b, i, bits::SHOOT);
        }
        if self.committed {
            self.avoid = 0;
            b.face_point(ctx, i, ctx.goal(ctx.team(i)), 3, 0);
        } else if self.timer == 0 {
            self.timer = 6; // fps × 0.1
            self.avoid = self.juke_or_avoid(ctx, b, i);
        } else {
            self.timer -= 1;
        }
        b.sprint(ctx, i);
        if !still {
            b.move_to(ctx, i, self.target, 3, self.avoid);
        }
    }

    /// `FUN_00109df8` (raw `0x109df8..0x109f88`): with turbo to spare,
    /// go at the nearest standing defender and juke him as soon as a juke
    /// is on (`FUN_00182878`), turning to him (at least 57 units off) and
    /// ending the patience; without turbo, back away from a defender
    /// within 8, or stand. A defender who goes down is let go (and the
    /// nearest standing one taken next).
    fn patient_start(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let beat = usize::try_from(self.beat)
            .ok()
            .filter(|&d| d < ctx.w.players.len());
        if beat.is_some_and(|d| ctx.fallen(d)) {
            self.beat = -1;
            return;
        }
        if ctx.turbo(i) <= 0.2 {
            match ctx.nth_opponent(i, 0, true) {
                Some(o) if ctx.between(i, o) < 8.0 => {
                    let a = ctx.angle_between(o, i);
                    b.steer(ctx, i, a, 3, 1, None);
                }
                _ => b.stop(ctx, i),
            }
            return;
        }
        let Some(d) = beat else {
            match ctx.nth_opponent(i, 0, true) {
                Some(o) => self.beat = o as i32,
                None => self.patient = false,
            }
            return;
        };
        let at = ctx.pos(d);
        b.move_to(ctx, i, at, 3, 1);
        if should_juke(ctx, i, at) {
            // 57.2222 truncated (`0x109f40`).
            b.face_point(ctx, i, at, 3, 57);
            build(ctx, b, i, bits::JUKE);
            self.patient = false;
        }
    }

    /// `FUN_00109c80`: juke a defender in the way (odds `(0, 40, 60,
    /// 100)`) and steer straight; otherwise steer round players, hard
    /// (mode 2) a quarter of the time the juke odds miss by, or always
    /// with a full meter.
    fn juke_or_avoid(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) -> i32 {
        let open = path_open(ctx, i);
        if open && openness(ctx, i) > 4 {
            return 1;
        }
        if juking(ctx, b, i) {
            return 1;
        }
        let r = (ctx.rand() % 100) as i32;
        let p = skill::percent(ctx, ctx.team(i), 0, 40, 60, 100);
        let hard = (100 - p) * 3 / 4;
        let rim = ctx.goal(ctx.team(i));
        if should_juke(ctx, i, rim) && !open && r < p {
            b.face_point(ctx, i, rim, 3, 0);
            build(ctx, b, i, bits::JUKE);
            return 0;
        }
        if r < hard || ctx.meter_full(ctx.team(i)) {
            2
        } else {
            1
        }
    }
}

/// `FUN_00180c38`: take the long spot: always with a full meter, else
/// `r % 100 < Threes / 2 + 50`.
fn long_spot(ctx: &mut Ctx, i: usize) -> bool {
    if ctx.meter_full(ctx.team(i)) {
        return true;
    }
    let threes = rating(ctx.w.players[i].ratings.threes);
    ((ctx.rand() % 100) as i32) < threes / 2 + 0x32
}

/// `FUN_00180f80(ai, spot, zone, 0)`.
fn pick_spot(ctx: &mut Ctx, i: usize, sector: i32) -> [f32; 2] {
    spot_in(ctx, i, sector, false)
}

/// `FUN_00180f80(ai, spot, zone, long)`: a spot to work from in a zone,
/// in the court turned so the rim is at +36.75 and then turned back by
/// the side. Unless asked long, a shooter over 50 Threes is sent long
/// 80..100% of the time (one draw). The corners (`FUN_00180cd0`) sit
/// 0..3.4 off the baseline and 23.5..25.4 out (19..21.9 short); the wings
/// and top (`FUN_00180e28`) 26.25..29.6 from the rim (20.75..23.65 short)
/// at 614, 512 or 410 ±20 units; the paint's spots (zones 5 and 6) are
/// near or far on a coin ([`paint_spot`]). Every spot is kept 12 from
/// where the ball's owner is headed (`FUN_00181150`).
pub(super) fn spot_in(ctx: &mut Ctx, i: usize, zone: i32, long: bool) -> [f32; 2] {
    let mut long = long;
    if !long {
        let threes = rating(ctx.w.players[i].ratings.threes);
        if 0x32 < threes {
            let odds = if threes < 0x47 {
                if 0x3c < threes { 0x5a } else { 0x50 }
            } else {
                100
            };
            if ctx.rand() % 100 < odds {
                long = true;
            }
        }
    }
    let spot = match zone {
        0 | 4 => {
            let long = long || long_spot(ctx, i);
            let (base, span) = if long { (23.5, 20) } else { (19.0, 30) };
            let x = 36.75 - (ctx.rand() % 0x23) as f32 / 10.0;
            let z = base + (ctx.rand() % span) as f32 / 10.0;
            [x, if zone == 0 { -z } else { z }]
        }
        5 | 6 => {
            let near = (ctx.rand() ^ 1) & 1 != 0;
            return paint_spot(ctx, i, zone, near);
        }
        _ => {
            let centre = match zone {
                1 => 614,
                2 => 512,
                _ => 410,
            };
            let long = long || long_spot(ctx, i);
            let (base, span) = if long { (26.25, 0x23) } else { (20.75, 0x1e) };
            let a = centre + (ctx.rand() % 0x28) as i32 - 0x14;
            let r = base + (ctx.rand() % span) as f32 / 10.0;
            [
                r * crate::input::native_cos_1024(a) + 36.75,
                r * crate::input::native_sin_1024(a),
            ]
        }
    };
    let side = ctx.side(ctx.team(i)) as f32;
    keep_clear(ctx, i, [spot[0] * side, spot[1] * side])
}

/// `FUN_001812a8(ai, spot, zone, deep)` in the paint (zones 5 and 6): a
/// low-post spot 2.7 off the rim and 9 to the side when deep, else 10.7
/// and 8, give or take 0..1.75 at a random angle (two draws); kept clear
/// of the ball's owner. Zones below 5 take `FUN_00180f80`'s spot.
pub(super) fn paint_spot(ctx: &mut Ctx, i: usize, zone: i32, deep: bool) -> [f32; 2] {
    if zone < 5 {
        return spot_in(ctx, i, zone, deep);
    }
    let (inside, across) = if deep { (2.7, 9.0) } else { (10.7, 8.0) };
    let across = if zone == 5 { -across } else { across };
    let r = (ctx.rand() & 7) as f32 * 0.25;
    let a = (ctx.rand() & 0x3ff) as i32;
    let spot = [
        36.75 - inside + r * crate::input::native_cos_1024(a),
        across + r * crate::input::native_sin_1024(a),
    ];
    let side = ctx.side(ctx.team(i)) as f32;
    keep_clear(ctx, i, [spot[0] * side, spot[1] * side])
}

/// `FUN_00181150`: a spot within 12 of where the ball's owner (not the
/// player himself) is headed moves out to 12, kept inside 41 × 25.5.
fn keep_clear(ctx: &Ctx, i: usize, spot: [f32; 2]) -> [f32; 2] {
    let Some(o) = ctx.ball_owner().filter(|&o| o != i) else {
        return spot;
    };
    let t = ctx.heading_for(o);
    let (dx, dz) = (spot[0] - t[0], spot[1] - t[1]);
    let d = crate::math::len(dx, dz);
    if 12.0 <= d {
        return spot;
    }
    let mut out = [t[0] + dx * 12.0 / d, t[1] + dz * 12.0 / d];
    if 41.0 < out[0].abs() {
        out[0] = 41.0_f32.copysign(out[0]);
    }
    if 25.5 < out[1].abs() {
        out[1] = 25.5_f32.copysign(out[1]);
    }
    out
}

words! {
    /// "iso3pt" (type 0x11; `FUN_0010c0a8`, update `FUN_0010c158`): work
    /// for a long-range shot. Walk in to 28.35 when far; back out beyond
    /// the arc (by 1.4..4.9) when inside it; go at a defender within 15
    /// with jukes (`skill(50, 75, 100, 100)`); shoot from beyond the arc
    /// within 29.75 at quality 6 or more (7 with a full meter); else wait
    /// `fps × 0.4` frames, backing off and picking a spot.
    pub(crate) struct Iso {
        done: bool,
        attack: i32,
        /// +0x34: set (`fps × 0.15`) when the juke's odds miss; the rest
        /// of the attack then holds the pad as it is.
        missed: i32,
        toggle: bool,
        relocate: i32,
        margin: f32,
        walk: i32,
        wait: i32,
        waiting_spot: bool,
        defender: i32,
        spot: [f32; 2],
    }
}

impl Iso {
    pub fn new() -> Self {
        Self {
            defender: -1,
            ..Default::default()
        }
    }

    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        self.done || !ctx.has_ball(i)
    }

    /// `FUN_0010c8c0`: back out for a second and pick a spot.
    fn relocate(&mut self, ctx: &mut Ctx, i: usize) {
        self.relocate = 60;
        let r = ctx.rand();
        self.margin = (r % 0x23) as f32 / 10.0 + 1.4;
        let s = ctx.sector(i);
        self.spot = pick_spot(ctx, i, s);
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if juking(ctx, b, i) {
            self.attack = 0;
            self.walk = 0;
            self.wait = 0;
            self.relocate(ctx, i);
            return;
        }
        let side = ctx.side(ctx.team(i));
        let off_rim = if side == 1 { 256 } else { 768 };
        if self.wait > 0 {
            if ctx.turbo(i) > 0.2 {
                self.wait = 0;
                return;
            }
            self.wait -= 1;
            if self.waiting_spot {
                if dist(ctx.pos(i), self.spot) < 2.0 {
                    self.wait = 0;
                } else {
                    b.move_to(ctx, i, self.spot, 3, 2);
                }
                return;
            }
            if ctx.hoop_dist(i) <= 35.75 {
                b.steer(ctx, i, off_rim, 3, 0, None);
                return;
            }
            self.waiting_spot = true;
            let s = ctx.sector(i);
            self.spot = pick_spot(ctx, i, s);
            return;
        }
        if self.attack < 1 {
            if self.relocate > 0 {
                if !outside(ctx, i, self.margin) && ctx.pos(i)[1].abs() <= 23.0 {
                    let a = if 36.75 < side as f32 * ctx.pos(i)[0] {
                        off_rim
                    } else {
                        ctx.hoop_angle(i)
                    };
                    b.steer(ctx, i, a, 3, 1, None);
                    b.sprint(ctx, i);
                    self.relocate -= 1;
                } else {
                    b.stop(ctx, i);
                    self.relocate = 0;
                }
                return;
            }
            if self.walk > 0 {
                if 28.35 <= ctx.hoop_dist(i) {
                    let rim = ctx.goal(ctx.team(i));
                    if should_juke(ctx, i, rim) {
                        let p = skill::percent(ctx, ctx.team(i), 0, 0, 0x32, 0x46);
                        if ((ctx.rand() % 100) as i32) < p {
                            self.toggle = !self.toggle;
                            if self.toggle {
                                build(ctx, b, i, bits::JUKE);
                            }
                        }
                    }
                    let a = ctx.angle_to_hoop(i);
                    b.steer(ctx, i, a, 3, 1, None);
                    self.walk -= 1;
                } else {
                    self.walk = 0;
                }
                return;
            }
            let d = ctx.hoop_dist(i);
            if d <= 33.75 {
                if 0.3 < ctx.turbo(i) && quality(ctx, i) < 7 && 17.0 < d && self.near(ctx, i) {
                    // `FUN_0010c878`: `DAT_004b3dc0` (9) updates at him.
                    self.attack = 9;
                    self.missed = 0;
                    self.defender = ctx.nth_opponent(i, 0, true).map_or(-1, |o| o as i32);
                    return;
                }
                if d <= 29.75 {
                    if !outside(ctx, i, 0.5) {
                        self.relocate(ctx, i);
                        return;
                    }
                    let need = if ctx.meter_full(ctx.team(i)) { 7 } else { 6 };
                    if need <= quality(ctx, i) {
                        self.done = true;
                        let shot = Shoot::new(ctx, i);
                        b.push(Act::Shoot(shot));
                        return;
                    }
                    self.wait = 24; // fps × 0.4
                    self.waiting_spot = false;
                    return;
                }
            }
            // `FUN_0010c990`.
            self.walk = 9;
            let s = ctx.sector(i);
            self.spot = pick_spot(ctx, i, s);
            return;
        }
        self.attack -= 1;
        let Some(o) = usize::try_from(self.defender).ok() else {
            self.attack = 0;
            return;
        };
        // Down, or in his counter stance (`FUN_00181828`): the attack is
        // over; after a missed juke it holds (`0x10c2d4..0x10c308`).
        if ctx.fallen(o) || ctx.countering(o) {
            self.attack = 0;
            return;
        }
        if self.missed != 0 {
            return;
        }
        // His velocity not zero (`FUN_0013ab28`, actor+0x40), not catching.
        if ctx.between(i, o) <= 5.5 && ctx.w.players[i].moving() && !ctx.catching(i) {
            if ctx.turbo(i) <= 0.2 {
                self.attack = 0;
                self.relocate(ctx, i);
                return;
            }
            let p = skill::percent(ctx, ctx.team(i), 0x32, 0x4b, 100, 100);
            if ((ctx.rand() % 100) as i32) < p {
                self.toggle = !self.toggle;
                if self.toggle {
                    build(ctx, b, i, bits::JUKE);
                    b.face_point(ctx, i, ctx.pos(o), 3, 0);
                }
                return;
            }
            // The odds missed (`0x10c46c`): `fps × 0.15`, and the pad holds.
            self.missed = (crate::TICK_HZ as f32 * f32::from_bits(0x3e19_999a)) as i32;
            return;
        }
        b.move_to(ctx, i, ctx.pos(o), 3, 1);
    }

    /// `FUN_0010cc80`: the nearest standing opponent is within 15 and on
    /// the half the handler attacks.
    fn near(&self, ctx: &Ctx, i: usize) -> bool {
        let Some(o) = ctx.nth_opponent(i, 0, true) else {
            return false;
        };
        let side = ctx.side(ctx.team(i));
        let on_half = if 0.0 <= ctx.pos(o)[0] {
            side == 1
        } else {
            side == -1
        };
        on_half && ctx.between(i, o) < 15.0
    }
}

words! {
    /// "keep away" (type 0x12; `FUN_0010d4c8`, done `FUN_0010d520`,
    /// update `FUN_0010d590`, raw `0x10d590..0x10d7f8`): the best
    /// ballhandler keeps the ball from a side with a full meter. He stands
    /// until the nearest opponent comes within 10, then runs to a spot near
    /// half court (`FUN_0010d800`), waits there `fps × skill(1.5, 0.8,
    /// 0.6, 0.2)` frames (at least 2), and so on; on the way, every other
    /// juke he would make (`FUN_00182878`) against a standing opponent
    /// within 5.5 with turbo over 0.2 he makes. He steers round players
    /// gently for the first half of each `fps × 0.4` frames, hard after.
    pub(crate) struct KeepAway {
        /// +0x20, +0x28.
        spot: [f32; 2],
        /// +0x30.
        toggle: bool,
        /// +0x34.
        timer: i32,
        /// +0x38.
        wait: i32,
    }
}

impl KeepAway {
    pub fn new() -> Self {
        Self {
            wait: 1,
            ..Default::default()
        }
    }

    /// `FUN_0010d520`.
    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        !ctx.has_ball(i)
    }

    /// `FUN_0010d800`: across the court from the nearest opponent when
    /// he is on the player's side and wider (6.625 out), else 19.875 out
    /// on the player's side (6.625 beyond 13.25); x near half court,
    /// `r % 36 - 18 + side × (r & 7)` (two draws).
    fn relocate(&mut self, ctx: &mut Ctx, i: usize) {
        let z = ctx.pos(i)[1];
        let s = if 0.0 <= z { 1.0 } else { -1.0 };
        let mut k = s;
        if z.abs() <= 13.25 {
            let same = ctx
                .nth_opponent(i, 0, false)
                .filter(|&o| (0.0 <= ctx.pos(o)[1]) == (s == 1.0));
            k = match same {
                Some(o) if z.abs() < ctx.pos(o)[1].abs() => -s,
                _ => s * 3.0,
            };
        }
        let r1 = ctx.rand();
        let side = ctx.side(ctx.team(i));
        let r2 = ctx.rand();
        self.spot[1] = k * 26.5 * 0.25;
        self.spot[0] = ((r1 % 0x24) as i32 - 0x12 + side * (r2 & 7) as i32) as f32;
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let o = ctx.nth_opponent(i, 0, false);
        if 0 < self.wait {
            if self.wait == 1 {
                if o.is_some_and(|o| ctx.between(i, o) < 10.0) {
                    self.wait = 0;
                    self.relocate(ctx, i);
                } else {
                    b.stop(ctx, i);
                }
            } else {
                self.wait -= 1;
                b.stop(ctx, i);
            }
            return;
        }
        if dist(ctx.pos(i), self.spot) < f32::from_bits(0x3fb3_3333) {
            let f = skill::value(
                ctx,
                ctx.team(i),
                1.5,
                f32::from_bits(0x3f4c_cccd),
                f32::from_bits(0x3f19_999a),
                f32::from_bits(0x3e4c_cccd),
            );
            self.wait = ((60.0 * f) as i32).max(2);
            return;
        }
        let span = (60.0 * f32::from_bits(0x3ecc_cccd)) as i32;
        if self.timer == 0 {
            self.timer = span;
        } else {
            self.timer -= 1;
        }
        let avoid = if self.timer < span / 2 { 1 } else { 2 };
        b.move_to(ctx, i, self.spot, 3, avoid);
        if juking(ctx, b, i) {
            return;
        }
        let Some(o) = o else {
            return;
        };
        if ctx.fallen(o) || !should_juke(ctx, i, self.spot) {
            return;
        }
        if 5.5 <= ctx.between(i, o) || ctx.turbo(i) <= 0.2 {
            return;
        }
        self.toggle = !self.toggle;
        if !self.toggle {
            build(ctx, b, i, bits::JUKE);
        }
    }
}

words! {
    /// "post up" (type 6; `FUN_00115368`, vtable `0x438a58`: start
    /// `FUN_001154b8`, done `FUN_001155b8`, update `FUN_00115628`, raw
    /// `0x115628..0x115d40`): the handler works in the paint. Out of the
    /// paint he runs at the rim; in it, he first waits (shut in, `r %
    /// (fps × 0.4) + fps × 0.2` frames, standing, a pump fake one time in
    /// four after it at openness 3..5), then within his reach of the rim
    /// (6..10.9, drawn) makes his move: off the baseline, a pass out of a
    /// shut lane with a poor shot, straight at the rim when nobody is
    /// within 6 or the odds `skill(0, 50, 100, 100)` say so, else a turn by
    /// the defender's side (`0x3c9e70`: two angles in degrees by the
    /// eighth the defender stands in, a coin for the pair, a draw within
    /// it; straight at the rim when that heads for the baseline). Two
    /// frames along it, then each frame the shot (a pass out of a blocked
    /// lay-up, `FUN_00181c68`; in a dunk with a taller defender close,
    /// `FUN_001837d0`, shoot pressed every other frame; the modes
    /// that pass rather than shoot drive or dunk). Done after `fps × 4`
    /// frames.
    pub(crate) struct PostUp {
        /// +0x1c.
        done: bool,
        /// +0x20: frames since the move.
        count: i32,
        /// +0x24: how near the rim the move is made (6..10.9).
        near: f32,
        /// +0x28: drawn (10..11.9) and not read here.
        far: f32,
        /// +0x2c: frames of the first wait.
        wait: i32,
        /// +0x30: a pump fake under way.
        faking: bool,
        /// +0x34: the move's angle.
        angle: i32,
        /// +0x38: in the paint.
        in_paint: bool,
        /// +0x50: the avoid mode, chosen every `fps × 0.1` frames
        /// (`FUN_00115d48`), 1 after the move.
        avoid: i32,
        /// +0x54.
        timer: i32,
        /// +0x58: careful (always set: `r % 100 < 100`).
        careful: bool,
        /// +0x5c: pressing shoot every other frame (`0x3c9e68`).
        release: bool,
    }
}

impl PostUp {
    /// `FUN_00115368`: three draws.
    pub fn new(ctx: &mut Ctx) -> Self {
        let r = ctx.rand();
        let near = (r % 0x32) as f32 / 10.0 + 6.0;
        let r = ctx.rand();
        let far = (r % 0x14) as f32 / 10.0 + 10.0;
        let r = ctx.rand();
        Self {
            avoid: 1,
            near,
            far,
            careful: r % 100 < 100,
            ..Default::default()
        }
    }

    /// `FUN_001154b8`: a handler with an open lane or open over 4 makes
    /// his move at once; otherwise he waits `r % (fps × 0.4) + fps ×
    /// 0.2` frames.
    pub fn enter(&mut self, ctx: &mut Ctx, i: usize) {
        self.wait = if ctx.has_ball(i) && (path_open(ctx, i) || 4 < openness(ctx, i)) {
            0
        } else {
            let r = ctx.rand();
            let span = (60.0 * f32::from_bits(0x3ecc_ccce)) as u32;
            (r % span) as i32 + (60.0 * f32::from_bits(0x3e4c_cccd)) as i32
        };
        self.in_paint = 5 <= ctx.zone(i);
    }

    /// `FUN_001155b8`.
    pub fn done(&self, age: u16) -> bool {
        self.done || 240 < i32::from(age)
    }

    /// `FUN_00115d48`: the avoid mode: none needed with an open lane and
    /// a shot over 5; else juke a defender in the way at `skill(0, 40, 60,
    /// 100)` (mode 0), or by the same draw mode 0 below `(100 − p) × 3/4`,
    /// 2 below `(100 − that) × 3/4`, else 1.
    fn avoid_mode(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) -> i32 {
        if path_open(ctx, i) && 5 < quality(ctx, i) {
            return 1;
        }
        if juking(ctx, b, i) {
            return 1;
        }
        let p = skill::percent(ctx, ctx.team(i), 0, 0x28, 0x3c, 100);
        let h1 = ((100 - p) * 3) >> 2;
        let h2 = ((100 - h1) * 3) >> 2;
        let r = (ctx.rand() % 100) as i32;
        let rim = ctx.goal(ctx.team(i));
        if should_juke(ctx, i, rim) && r < p {
            if let Some(o) = ctx.nth_opponent(i, 0, false) {
                b.face_point(ctx, i, ctx.pos(o), 1, 0);
            }
            build(ctx, b, i, bits::JUKE);
            return 0;
        }
        if r < h1 {
            0
        } else if r < h2 {
            2
        } else {
            1
        }
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if juking(ctx, b, i) {
            return;
        }
        if !ctx.has_ball(i) {
            b.stop(ctx, i);
            return;
        }
        if self.faking {
            if !busy(ctx, i) {
                self.faking = false;
            }
            return;
        }
        if self.count != 0 {
            self.avoid = 1;
        } else if self.timer == 0 {
            self.timer = (60.0 * f32::from_bits(0x3dcc_cccd)) as i32;
            self.avoid = self.avoid_mode(ctx, b, i);
        } else {
            self.timer -= 1;
        }
        if juking(ctx, b, i) {
            return;
        }
        let rim = ctx.goal(ctx.team(i));
        if !self.in_paint {
            self.in_paint = 5 <= ctx.zone(i);
            if !self.in_paint {
                b.sprint(ctx, i);
                b.move_to(ctx, i, rim, 3, self.avoid);
            }
            return;
        }
        if 0 < self.wait {
            let open = path_open(ctx, i);
            if open {
                self.wait = 0;
            } else {
                self.wait -= 1;
            }
            b.stop(ctx, i);
            // `FUN_001816b0`: not while dribbling (actor+0x1a0 1).
            if 0 < self.wait || open || on_baseline(ctx, i, -2.0, 80.0) || dribbling(ctx, i) {
                return;
            }
            if ctx.rand() & 3 != 0 {
                return;
            }
            if (3..=5).contains(&openness(ctx, i)) {
                self.faking = true;
                b.press(bits::SHOOT);
            }
            return;
        }
        if self.count == 0 {
            if self.near < ctx.hoop_dist(i) {
                b.sprint(ctx, i);
                b.move_to(ctx, i, rim, 3, self.avoid);
                return;
            }
            // `FUN_00181750` (actor+0x1a0 ≥ 0): taken as set, as in the
            // juke test.
            if on_baseline(ctx, i, -2.0, 80.0) {
                self.near = 20.0;
                b.steer(ctx, i, off_baseline(ctx, i), 3, 1, None);
                return;
            }
            if quality(ctx, i) < 3 && !path_open(ctx, i) && 5 < ctx.shot_clock() {
                self.done = true;
                let to = pass_target(ctx, i);
                b.push(Act::Pass(Pass::new(ctx, i, to)));
                return;
            }
            self.choose_move(ctx, i);
        }
        self.count += 1;
        if 3 <= self.count {
            if should_pass(ctx, i, !self.careful) {
                self.done = true;
                let to = pass_target(ctx, i);
                b.push(Act::Pass(Pass::new(ctx, i, to)));
            } else if self.release {
                ctx.game.post_toggle = !ctx.game.post_toggle;
                if ctx.game.post_toggle {
                    b.press(bits::SHOOT);
                }
            } else if ctx.dunking(i) && dunk_blocked(ctx, i) {
                self.release = true;
            } else if !mode_pass(ctx, i) {
                build(ctx, b, i, bits::SHOOT);
            } else if dunk_close(ctx, i) {
                build(ctx, b, i, bits::SHOOT);
                let a = ctx.angle_to_hoop(i);
                b.face(ctx, i, a, 1, 0);
                b.steer(ctx, i, a, 3, 1, None);
                b.sprint(ctx, i);
                return;
            } else {
                self.done = true;
                let d = Drive::new(ctx, i);
                b.push(Act::Drive(d));
            }
        }
        b.sprint(ctx, i);
        b.face(ctx, i, self.angle, 1, 0);
        b.steer(ctx, i, self.angle, 3, 0, None);
    }

    /// The move (raw `0x115968..0x115b94`).
    fn choose_move(&mut self, ctx: &mut Ctx, i: usize) {
        let o = ctx.nth_opponent(i, 0, false);
        let p = skill::percent(ctx, ctx.team(i), 0, 0x32, 100, 100);
        let straight = o.is_none_or(|o| 6.0 < ctx.between(i, o)) || {
            let r = ctx.rand();
            ((r % 100) as i32) < p
        };
        if straight {
            self.angle = ctx.angle_to_hoop(i);
            return;
        }
        let o = o.unwrap_or(i);
        let ha = ctx.hoop_angle(i);
        let d = super::court::turn(ctx.angle_between(o, i), ha) & 0x3ff;
        let row = (d >> 7) as usize;
        let r = ctx.rand();
        let (lo, hi) = if r & 1 != 0 { (0, 1) } else { (2, 3) };
        let lo = POST_MOVES[row * 4 + lo];
        let hi = POST_MOVES[row * 4 + hi];
        let r = ctx.rand();
        let deg = lo + (r % (hi - lo + 1) as u32) as i32;
        // 2.84444 units a degree (`0x40360b3f`).
        let units = (deg as f32 * f32::from_bits(0x4036_0b3f) + 0.5) as i32;
        self.angle = (ha - 512 + units) & 0x3ff;
        if on_baseline(ctx, i, -2.0, 80.0) {
            let toward = if ctx.side(ctx.team(i)) == 1 { 768 } else { 256 };
            // 199.61 truncated (`0x43479c4d`).
            if toward - 199 < self.angle && self.angle < toward + 199 {
                self.angle = ctx.angle_to_hoop(i);
            }
        }
    }
}

/// `0x3c9e70`: the post-up's turns in degrees, by the eighth of the turn
/// the defender stands in round the handler (from the rim's line): two
/// pairs, a coin picks one.
const POST_MOVES: [i32; 32] = [
    315, 360, 180, 360, 340, 380, 300, 380, 340, 380, 340, 380, 340, 380, 340, 380, 340, 380, 340,
    380, 340, 380, 340, 380, 340, 380, 340, 420, 0, 45, 0, 180,
];

/// `FUN_001837d0` (raw `0x1837d0..0x1838ec`): no gamebreaker task busy
/// (`FUN_00140ff8`), in a dunk (bit 7, `FUN_00181670`) at least 1 up and 5
/// out, with the nearest opponent within 6, nearer the rim, and the
/// highest of his posed hands and head (`FUN_00181f50`, [`World::top_bone`])
/// above the ball (ball+0x34). Without the disc's animations the rewrite's
/// standing reach above his feet stands in for the bones; with them that
/// reach had a standing man's hands near 8.5 up, so most met dunks became
/// lay-ups in the air.
///
/// [`World::top_bone`]: crate::World::top_bone
fn dunk_blocked(ctx: &Ctx, i: usize) -> bool {
    if ctx.w.gamebreaker_task.is_some()
        || !ctx.dunking(i)
        || ctx.w.players[i].height < 1.0
        || ctx.hoop_dist(i) < 5.0
    {
        return false;
    }
    let Some(o) = ctx.nth_opponent(i, 0, false) else {
        return false;
    };
    if 6.0 <= ctx.between(i, o) || ctx.hoop_dist(i) <= ctx.hoop_dist(o) {
        return false;
    }
    let hand = ctx
        .w
        .top_bone(o)
        .unwrap_or(ctx.w.players[o].height + ctx.w.players[o].ratings.reach);
    ctx.w.ball.position[1] < hand
}

words! {
    /// The handler who feeds a post man or an isolation partner
    /// (`FUN_0011a580(handler, partner, zone, deep)`, vtable `0x4391f0`:
    /// done `0x11a760`, update `0x11a7a0`, raw `0x11a7a0..0x11aa24`,
    /// target `0x11aa28`). He goes to a spot in the zone beside the
    /// partner's (toward the partner's side of him, the wing for a corner;
    /// the top when they stand within 15 across; for a partner in the
    /// paint the zone `0x3cae78` gives), dribbling there in an embedded
    /// "carrier pause" (`(0, 9)`, +0x50). Once he is within 10 of the
    /// spot (or there, for a post partner), the frames the partner stands
    /// still where he is going (his zone, or within 16 of the rim deep,
    /// 18.5 not) are counted; after `fps × 0.2` of them the ball goes to
    /// him from beyond 38.75 or with a shot under 6, and after `fps × 0.4`
    /// anyway.
    pub(crate) struct Feeder {
        /// +0x1c.
        partner: i32,
        /// +0x20: the partner's zone.
        zone: i32,
        /// +0x24: a deep post.
        deep: bool,
        /// +0x28: the handler's zone.
        own: i32,
        /// +0x30.
        spot: [f32; 2],
        /// +0x40.
        going: bool,
        /// +0x44: frames the partner has stood ready.
        count: i32,
        /// +0x48.
        done: bool,
        /// +0x50.
        pause: CarrierPause,
        pause_started: bool,
    }
}

impl Feeder {
    /// The constructor's zone and spot (`FUN_00180f80`, its draws).
    pub fn new(ctx: &mut Ctx, h: usize, partner: usize, zone: i32, deep: bool) -> Self {
        let own = if zone < 5 {
            let side = ctx.side(ctx.team(h)) as f32;
            let mut own = if side * ctx.pos(partner)[1] < side * ctx.pos(h)[1] {
                if zone != 4 { zone + 1 } else { 3 }
            } else if zone != 0 {
                zone - 1
            } else {
                1
            };
            if zone != 2
                && own != ctx.sector(h)
                && (ctx.pos(h)[1] - ctx.pos(partner)[1]).abs() < 15.0
            {
                own = 2;
            }
            own
        } else {
            // `0x3cae78`: zone 5 {2, 1}, zone 6 {2, 3}, by deep.
            const POST: [i32; 4] = [2, 1, 2, 3];
            POST[((zone - 5).clamp(0, 1) * 2 + i32::from(deep)) as usize]
        };
        let spot = spot_in(ctx, h, own, false);
        Self {
            partner: partner as i32,
            zone,
            deep,
            own,
            spot,
            going: true,
            pause: CarrierPause::new(0.0, 9.0, false),
            ..Default::default()
        }
    }

    pub fn target(&self) -> Option<[f32; 2]> {
        self.going.then_some(self.spot)
    }

    /// `0x11a760`.
    pub fn done(&self, ctx: &Ctx, h: usize) -> bool {
        self.done || !ctx.has_ball(h)
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, h: usize) {
        let partner = usize::try_from(self.partner)
            .ok()
            .filter(|&j| j < ctx.w.players.len());
        let near = self.zone < 5 && dist(ctx.pos(h), self.spot) < 10.0;
        if (!self.going || near)
            && let Some(p) = partner
        {
            let ready = ctx.strength_class(p) == 0
                && if self.zone < 5 {
                    ctx.zone(p) == self.zone
                } else {
                    ctx.hoop_dist(p) < if self.deep { 16.0 } else { 18.5 }
                };
            self.count = if ready { self.count + 1 } else { 0 };
            let free =
                ctx.has_ball(h) && !juking(ctx, b, h) && !ctx.passing_now(h, b) && !ctx.leaping(h);
            if (60.0 * f32::from_bits(0x3e4c_cccd)) as i32 <= self.count
                && free
                && (38.75 < ctx.hoop_dist(h)
                    || quality(ctx, h) < 6
                    || (60.0 * f32::from_bits(0x3ecc_cccd)) as i32 <= self.count)
            {
                let pass = Pass::new(ctx, h, Some(p));
                b.push(Act::Pass(pass));
                self.going = false;
                self.done = true;
            }
        }
        if self.done {
            return;
        }
        if self.going {
            self.going = f32::from_bits(0x3fb3_3333) <= dist(ctx.pos(h), self.spot);
            b.move_to(ctx, h, self.spot, 3, 2);
        } else {
            // `FUN_0013be90(+0x50)`: "carrier pause" is never done.
            if !self.pause_started {
                self.pause_started = true;
                self.pause.enter(ctx, h);
            }
            self.pause.update(ctx, b, h);
        }
    }
}

/// `FUN_00182d10(ai, &kind, lob)` (raw `0x182d10..0x183310`; Ghidra
/// stops at the EE `sqrt.s` at `0x18319c`, `0x46050044`): a trick pass for
/// a handler who could pass now (`FUN_00182c58(ai, 6)`), on the floor,
/// with Handles 70+ and turbo of at least 0.2. Without `lob`, with the
/// stick at rest (strength under 16) and his defender within 12: with
/// Handles 60+, a defender more than 4 away, standing still and not in a
/// counter stance (bit 16, `FUN_00181828` at `0x182fd0`), is the one off his head (`kind` 1, Off the
/// Heezay); otherwise, with the stick (plus the camera's yaw) within 256
/// of the line to the rim, or for a lob, the one off the backboard: in
/// the half the side attacks, within 35 of the middle in x, and inside
/// the 20° wedge `(|z| − 3) · tan 20° ≤ 38 − |x|` in front of the board
/// (the polynomial sine and cosine of `0x3eb2b8c3` at `0x1830e8`); a
/// pass out (not a lob) only from within 8 across and 29 in x. Then the
/// odds: `skill(0, 20, 40, 60)` for a lob off the board, else `skill(5,
/// 5, 25, 25)` (one draw). `Some(heezay)` when it goes. The camera's yaw
/// (`0x402020`'s +0x94) is the rewrite's 0: the simulation has no camera.
pub(super) fn trick_pass_out(ctx: &mut Ctx, b: &Brain, i: usize, lob: bool) -> Option<bool> {
    if !super::plays::can_pass_now(ctx, b, i, 6) || ctx.airborne(i) {
        return None;
    }
    if rating(ctx.w.players[i].ratings.handles) < 0x46 || ctx.turbo(i) < 0.2 {
        return None;
    }
    let mut heezay = false;
    let mut ok = false;
    let side = ctx.side(ctx.team(i));
    let stick = b.pad.angle & 0x3ff;
    let mut board = lob;
    if !lob {
        let d = super::plays::defender(ctx, i);
        let gap = d.map_or(f32::MAX, |d| ctx.between(i, d));
        if b.pad.strength < 0x10 && gap < 12.0 {
            if rating(ctx.w.players[i].ratings.handles) >= 0x3c
                && 4.0 < gap
                && d.is_some_and(|d| !ctx.w.players[d].moving() && !ctx.countering(d))
            {
                heezay = true;
                ok = true;
            }
        } else {
            board = super::court::turn(ctx.angle_to_hoop(i), stick).abs() <= 0x100;
        }
    }
    if board {
        let p = ctx.pos(i);
        let half = if 0.0 <= p[0] { 1 } else { -1 };
        let wedge = (p[1].abs() - 3.0) * f32::from_bits(0x3eba_5a4f);
        if half == side.signum()
            && p[0].abs() < 35.0
            && (p[1].abs() - 3.0 < 0.0 || wedge <= 38.0 - p[0].abs())
        {
            ok = lob || (p[1].abs() < 8.0 && p[0].abs() < 29.0);
        }
    }
    if !ok {
        return None;
    }
    let team = ctx.team(i);
    let odds = if !heezay && lob {
        skill::percent(ctx, team, 0, 20, 40, 60)
    } else {
        skill::percent(ctx, team, 5, 5, 25, 25)
    };
    let r = ctx.rand();
    (((r % 100) as i32) < odds).then_some(heezay)
}

words! {
    /// The trick pass (type 0x17; `FUN_00100c60(ai, receiver, kind)`,
    /// vtable `0x4373f0`: start `0x100d48`, done `FUN_00100d50`, update
    /// `FUN_00100d70`): once the ball has been held past the drawn wait
    /// (`r % (fps × 0.05) + fps × 0.1`, 6..8 frames) and he is not already
    /// passing, he stops and throws it once (`FUN_00183318`): two turbos
    /// and pass. Its trajectory is the simulation's (Off the Heezay or
    /// Back 2 Papa by `pass_button`'s rules); the draws `FUN_00183318`
    /// makes for a pass off the board (Handles against `r % 80`, then one
    /// draw, or two) are made in order. A teammate's board pass uses the
    /// named target and the same queued waypoint, not an ordinary pass.
    pub(crate) struct TrickPass {
        /// +0x1c: the receiver (the passer himself for a pass out).
        to: i32,
        /// +0x20.
        wait: i32,
        /// +0x24: 1 off the defender's head.
        heezay: bool,
        /// +0x28.
        thrown: bool,
    }
}

/// `FUN_00183318`'s point on the board (`0x183318..0x183528`): `38 − r`
/// along the court on the side the team attacks, `15 − r` up (`r` the
/// ball's radius); then `rand() % 80` under the passer's Handles puts it
/// across where his line to the receiver's mirrored x meets the board,
/// within ±3, and `15 − rand · 0.4 · 3.5` up; else anywhere across `−3 −
/// r .. 3 + r` and `11.5 − r + rand · (r + 1.4)` up.
fn board_point(ctx: &mut Ctx, i: usize, to: i32) -> [f32; 3] {
    let team = ctx.team(i);
    let side = ctx.side(team) as f32;
    let r = ctx.w.ball.radius;
    let x = side * (38.0 - r);
    let unit = |v: u32| (v & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000);
    let p = ctx.pos(i);
    let q = usize::try_from(to)
        .ok()
        .filter(|&t| t < ctx.w.players.len())
        .map_or(p, |t| ctx.pos(t));
    if ((ctx.rand() % 0x50) as i32) < rating(ctx.w.players[i].ratings.handles) {
        let mirrored = crate::ball::bank_mirror(q[0], side, r);
        let f = (x - p[0]) / (mirrored - p[0]);
        let z = (f * q[1] + (1.0 - f) * p[1]).clamp(-3.0, 3.0);
        let y = 15.0 - unit(ctx.rand()) * 0.4 * 3.5;
        [x, y, z]
    } else {
        let z = (-3.0 - r) + (unit(ctx.rand()) * 2.0) * (r + 3.0);
        let y = (11.5 - r) + unit(ctx.rand()) * (r + 1.4);
        [x, y, z]
    }
}

impl TrickPass {
    /// `FUN_00100c60`: one draw.
    pub fn new(ctx: &mut Ctx, to: usize, heezay: bool) -> Self {
        let r = ctx.rand();
        let span = (60.0 * f32::from_bits(0x3d4c_ccce)) as u32;
        Self {
            to: to as i32,
            wait: (r % span) as i32 + (60.0 * f32::from_bits(0x3dcc_cccd)) as i32,
            heezay,
            thrown: false,
        }
    }

    /// `FUN_00100c60(ai, 0, 0)`: a person's call's trick pass, with no
    /// receiver (one draw).
    pub fn new_out(ctx: &mut Ctx) -> Self {
        let r = ctx.rand();
        let span = (60.0 * f32::from_bits(0x3d4c_ccce)) as u32;
        Self {
            to: -1,
            wait: (r % span) as i32 + (60.0 * f32::from_bits(0x3dcc_cccd)) as i32,
            heezay: false,
            thrown: false,
        }
    }

    /// `FUN_00100d50`.
    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        !ctx.has_ball(i)
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if self.thrown || ctx.catching(i) || ctx.game.held <= self.wait {
            return;
        }
        b.stop(ctx, i);
        if self.to < 0 {
            // No receiver (`0x100dcc..0x100e08`): the stick at the rim
            // (`FUN_0017da80`, level 3), no named target, two turbos and
            // pass: the press decides the trick pass.
            let angle = ctx.angle_to_hoop(i);
            b.face(ctx, i, angle, 3, 0);
            b.pad.target = -1;
            b.press(bits::TWO_TURBOS | bits::PASS);
            self.thrown = true;
            return;
        }
        let point = if self.heezay {
            [0.0; 3]
        } else {
            board_point(ctx, i, self.to)
        };
        // FUN_00183318 posts the waypoint for every recipient, then
        // presses two turbos and sets the named target (`FUN_00183318`).
        b.press(bits::TWO_TURBOS | bits::PASS);
        ctx.game.trick = i as i32 + 1;
        ctx.game.trick_off = match self.heezay {
            true => super::plays::defender(ctx, i).map_or(0, |d| d as i32 + 1),
            false => 0,
        };
        ctx.game.trick_point = point;
        b.pad.target = self.to;
        self.thrown = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BallState, Player, Rules, World};

    /// The drive committed to a dunk a taller defender meets (raw
    /// `0x1099c4..0x109a18`): from the update after the defender is seen,
    /// shoot is pressed every other update (the toggle `0x3c6f78`), and
    /// not held.
    #[test]
    fn a_drive_met_in_its_dunk_presses_shoot_every_other_update() {
        let players = vec![
            Player::new(0, [30.0, 0.0], 0.0),
            Player::new(1, [-30.0, 0.0], 0.0),
        ];
        let mut w = World::new(crate::tests::hoops(), players, Rules::default(), 1);
        w.ball.state = BallState::Held(0);
        let mut drive = Drive {
            committed: true,
            release: true,
            ..Default::default()
        };
        let (presses, _) = super::super::with_ctx(&w, |ctx| {
            (0..4)
                .map(|_| {
                    let mut b = Brain::default();
                    drive.update(ctx, &mut b, 0);
                    b.pad.buttons & bits::SHOOT != 0
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(presses, [true, false, true, false]);
    }

    /// `FUN_001837d0` weighs the ball against the highest of the nearer
    /// defender's posed hands and head (`FUN_00181f50`): a ball over a
    /// man standing flat-footed in the dunker's way is no block, one under
    /// his head is.
    #[test]
    fn a_dunk_is_met_by_the_defenders_posed_hands_and_head() {
        let poses = crate::pose::disc_bank();
        if poses.get().is_none() {
            return;
        }
        let hoop = crate::target_hoop(0);
        let mut w = World::new(
            crate::tests::hoops(),
            vec![
                Player::new(0, [0.0, 0.0], 0.0),
                Player::new(1, [0.0, 0.0], 0.0),
            ],
            Rules::default(),
            1,
        );
        w.poses = poses;
        w.gait = crate::ai::capture::disc_gait();
        let rim = w.rim_spot(hoop);
        let toward = if rim[0] >= 0.0 { 1.0 } else { -1.0 };
        w.players[0].position = [rim[0] - toward * 9.0, 0.0];
        w.players[1].position = [rim[0] - toward * 5.0, 0.0];
        w.players[0].height = 2.0;
        w.players[0].action = crate::Action::Dunking {
            hoop,
            elapsed: 0.3,
            from: w.players[0].position,
            to: rim,
            style: crate::player::DunkStyle::Normal,
            layup: false,
            tick: 18,
            release: 60,
            end: 80,
            row: 0,
            hang: None,
        };
        w.ball.state = BallState::Held(0);
        let top = w.top_bone(1).expect("posed");
        assert!((5.0..8.0).contains(&top), "{top}");
        let blocked = |w: &World| super::super::with_ctx(w, |ctx| dunk_blocked(ctx, 0)).0;
        w.ball.position = [w.players[0].position[0], top + 0.5, 0.0];
        assert!(!blocked(&w));
        w.ball.position[1] = top - 0.5;
        assert!(blocked(&w));
        // The rewrite's stand-in reach (his height and standing reach)
        // stood above the ball here.
        assert!(top + 0.5 < w.players[1].ratings.reach);
    }
}

words! {
    /// "playbook hold" (vtable `0x43ae88`, update `FUN_0012b7e0`; never
    /// done): the Dive lesson's holder (the playbook's
    /// [`super::playbook`] lesson branch) jukes (`FUN_00182280`, 0x10)
    /// every other update, unless he is juking already.
    pub(crate) struct LessonHold {
        /// +0x1c: toggled each update; the juke goes in while it is clear.
        off: bool,
    }
}

impl LessonHold {
    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if !self.off && !juking(ctx, b, i) {
            build(ctx, b, i, bits::JUKE);
        }
        self.off = !self.off;
    }
}

words! {
    /// The Trick Counter lesson's holder (type 0x10, `FUN_0010d090(ai,
    /// frames)`, vtable `0x437cd0`: done at that age, `FUN_0010d118`;
    /// update `FUN_0010d130`): facing his nearest opponent when that one
    /// is nearer the rim, else the rim, he jukes whenever he is not
    /// juking.
    pub(crate) struct LessonJuke {
        frames: i32,
    }
}

impl LessonJuke {
    pub fn new(frames: i32) -> Self {
        Self { frames }
    }

    pub fn done(&self, age: u16) -> bool {
        self.frames <= i32::from(age)
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let at = match ctx.nth_opponent(i, 0, false) {
            Some(o) if ctx.hoop_dist(o) < ctx.hoop_dist(i) => ctx.pos(o),
            _ => ctx.goal(ctx.team(i)),
        };
        if !juking(ctx, b, i) {
            b.face_point(ctx, i, at, 1, 0);
            build(ctx, b, i, bits::JUKE);
        }
    }
}
