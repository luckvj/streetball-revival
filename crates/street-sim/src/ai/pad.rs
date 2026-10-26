//! The computer's pad: the same word people's controllers fill
//! (`FUN_0012f2c8`'s action bits), a stick angle and a stick strength,
//! and the motion helpers that steer it (`FUN_0013af58` and friends).

use super::court::{angle_toward, direction, turn};
use super::words::words;
use super::{Brain, Ctx};
use crate::{Input, button};

/// The pad word's bits (`FUN_0012f2c8`, `USRGAME*.CFG`'s actions).
pub(super) mod bits {
    /// Juke (with the ball); 0x800000 is held alongside while pressed.
    pub const JUKE: u32 = 0x10;
    pub const SHOOT: u32 = 0x80;
    pub const PASS: u32 = 0x100;
    pub const STEAL: u32 = 0x200;
    /// L3, the kick pass.
    pub const KICK: u32 = 0x800;
    pub const BLOCK: u32 = 0x1000;
    /// Any turbo button held (`FUN_001308a0`).
    pub const TURBO: u32 = 0x2000;
    /// The four turbo buttons: L1, L2, R1, R2 (`0x3d01d0`).
    pub const L1: u32 = 0x4000;
    pub const L2: u32 = 0x8000;
    pub const R1: u32 = 0x10000;
    pub const R2: u32 = 0x20000;
    /// Two or more turbo buttons held.
    pub const TWO_TURBOS: u32 = 0x40000;
    pub const REBOUND: u32 = 0x100000;
    /// The dive for a loose ball (`0x10e780`, [`crate::dive`]).
    pub const DIVE: u32 = 0x1000000;
    /// The post man's call for a lob ("post pause"): the hold the post
    /// call reads (actor+0x78 & 0x800000, [`crate::World::post_call`]).
    pub const POST_CALL: u32 = 0x800000;
    /// The screener's stance at his spot ("set pick", "pick off ball"):
    /// the hold the actor reads (actor+0x78 & 0x400000,
    /// [`crate::World::pick_stance`]).
    pub const PICK_STANCE: u32 = 0x400000;
    pub const STRATEGY: u32 = 0x2000000;
    /// The screener steps into the defender (the pick animations' too).
    pub const PICK: u32 = 0x4000000;
    pub const POCKET: u32 = 0x8000000;
    pub const CANCEL: u32 = 0x10000000;
}

/// Stick strengths by speed level (`0x3d17d8`): still, walk, jog, run on
/// the 0..64 scale.
pub(super) const SPEEDS: [i32; 4] = [0, 38, 58, 64];
/// How close a target must be before a step toward it holds the old
/// angle, by speed level (`0x3d17f0`).
const ARRIVE: [f32; 4] = [0.5, 1.0, 1.5, 2.0];
/// Frames a speed level must hold before rising (`0x4b7614`, fps × 0.15).
const LEVEL_HOLD: i64 = 9;
/// The steering history (`0x4b760c`, fps × 0.2) and how much of it the
/// zigzag test reads (`0x4b7610`, fps × 0.12).
pub(super) const HISTORY: usize = 12;
const ZIGZAG: usize = 7;

words! {
    /// The pad the AI fills (AIPlayer+4): the action word (cleared every
    /// frame), the stick angle and strength (kept until changed), and a
    /// pass's receiver (+0x24).
    pub(crate) struct Pad {
        buttons: u32,
        angle: i32,
        strength: i32,
        /// The receiver of a pass, or -1.
        target: i32,
    }
}

words! {
    /// The steering state (AIPlayer+0x3c..+0x4c).
    pub(crate) struct Motion {
        /// The last angles steered, newest first.
        history: [i32; HISTORY],
        /// The speed level and the frame it last changed.
        level: i32,
        level_tick: i64,
    }
}

/// The pad as the simulation's [`Input`]: the turbo combos keep the
/// original's mask (the sim's trick slots read L1 0x4000, R1 0x8000, L2
/// 0x10000, R2 0x20000, `NativeTricks::slot`), so a combo the original
/// picks plays the same slot here.
pub(super) fn to_input(ctx: &Ctx, i: usize, pad: &Pad) -> Input {
    let b = pad.buttons;
    let mut out = if pad.strength > 0 {
        let d = direction(pad.angle);
        let s = pad.strength.min(64) as f32 / 64.0;
        Input::moving([d[0] * s, d[1] * s])
    } else {
        Input::default()
    };
    let mut held = 0;
    let combos = [
        (bits::L1, button::L1),
        (bits::L2, button::R1),
        (bits::R1, button::L2),
        (bits::R2, button::R2),
    ];
    for (bit, key) in combos {
        if b & bit != 0 {
            held |= key;
        }
    }
    let count = (held & button::TURBOS).count_ones();
    if b & bits::TWO_TURBOS != 0 && count < 2 {
        held |= button::L1 | button::R1;
    } else if b & bits::TURBO != 0 && count == 0 {
        held |= button::R1;
    }
    if b & bits::SHOOT != 0 {
        // Without the ball on the side that has it, shoot with a turbo is
        // the alley-oop's leap (`FUN_001001c0`, [`crate::leap`]).
        held |= button::SHOOT;
    }
    if b & bits::PASS != 0 {
        held |= button::PASS;
        // The sim picks a pass's receiver by the stick: point it at them
        // (a pass to oneself, the trick pass out, keeps the stick).
        if pad.target >= 0
            && pad.target as usize != i
            && (pad.target as usize) < ctx.w.players.len()
        {
            let to = ctx.pos(pad.target as usize);
            let d = direction(angle_toward(ctx.pos(i), to));
            out.stick = Input::moving(d).stick;
        }
    }
    if b & (bits::JUKE | bits::STEAL) != 0 {
        held |= button::TRICK;
    }
    if b & (bits::BLOCK | bits::REBOUND) != 0 {
        held |= button::JUMP;
    }
    if b & bits::KICK != 0 {
        held |= button::L3;
    }
    if b & bits::DIVE != 0 {
        held |= button::DIVE;
    }
    if b & bits::POST_CALL != 0 {
        held |= button::TRICK;
    }
    if b & bits::STRATEGY != 0 {
        held |= button::R3;
    }
    if b & bits::POCKET != 0 {
        held = (held & !button::TURBOS) | button::SELECT;
    }
    if b & bits::CANCEL != 0 {
        // The sim cancels with Select and exactly one turbo.
        held = (held & !button::TURBOS) | button::SELECT | button::R1;
    }
    let mut out = out.with(held);
    // The screener's pick press and stance, not buttons ([`button::PICK`],
    // [`button::PICK_STANCE`]).
    if b & bits::PICK != 0 {
        out.buttons |= button::PICK;
    }
    if b & bits::PICK_STANCE != 0 {
        out.buttons |= button::PICK_STANCE;
    }
    out
}

impl Brain {
    /// `FUN_0013b7d8`: press buttons this frame.
    pub fn press(&mut self, buttons: u32) {
        self.pad.buttons |= buttons;
    }

    /// `FUN_0013a908`: fill the steering history with the facing.
    fn reset_history(&mut self, facing: i32) {
        self.motion.history = [facing; HISTORY];
    }

    /// `FUN_0013ac30`: let go of the stick.
    pub fn stop(&mut self, ctx: &Ctx, i: usize) {
        self.pad.strength = 0;
        self.reset_history(ctx.facing(i));
        self.motion.level = 0;
        self.motion.level_tick = ctx.tick - LEVEL_HOLD - 1;
    }

    /// `FUN_0013b210`: turn to `angle` (when it is at least `min_turn` off
    /// the facing) at a speed level, without steering.
    pub fn face(&mut self, ctx: &Ctx, i: usize, angle: i32, level: usize, min_turn: i32) -> bool {
        let angle = angle & 0x3ff;
        if turn(angle, ctx.facing(i)).abs() < min_turn {
            return false;
        }
        self.motion.history = [angle; HISTORY];
        self.pad.strength = SPEEDS[level];
        self.pad.angle = angle;
        true
    }

    /// `FUN_0013b2f0`: turn toward a point.
    pub fn face_point(&mut self, ctx: &Ctx, i: usize, at: [f32; 2], level: usize, min_turn: i32) {
        let angle = angle_toward(ctx.pos(i), at);
        self.face(ctx, i, angle, level, min_turn);
    }

    /// `FUN_0013aed8` / `FUN_0013ac80`: run toward a point at a speed
    /// level, steering round players by `avoid`.
    pub fn move_to(&mut self, ctx: &Ctx, i: usize, to: [f32; 2], level: usize, avoid: i32) {
        let angle = angle_toward(ctx.pos(i), to);
        self.steer(ctx, i, angle, level, avoid, Some(to));
    }

    /// `FUN_0013af58`: steer along `angle` at a speed level. A faster
    /// level waits `LEVEL_HOLD` frames after the last change; a slower one
    /// applies at once. Running (level 2 and up) bends round players in
    /// the way (`FUN_0017e858`). With a target, a turn of under 6 keeps
    /// the old angle and a target within reach of the level is steered at
    /// directly; the rest is smoothed (`FUN_0017ec38`).
    pub fn steer(
        &mut self,
        ctx: &Ctx,
        i: usize,
        angle: i32,
        level: usize,
        avoid: i32,
        target: Option<[f32; 2]>,
    ) {
        let mut angle = angle & 0x3ff;
        let avoid = self.avoid_mode(ctx, i, avoid);
        let mut level = level as i32;
        let current = self.motion.level;
        if current < level {
            if ctx.tick - self.motion.level_tick > LEVEL_HOLD {
                self.motion.level = level;
            } else {
                level = current;
            }
        } else if level < current {
            self.motion.level = level;
            self.motion.level_tick = ctx.tick;
        }
        if level == 0 {
            self.pad.strength = 0;
            return;
        }
        let gap = target.map_or(0.0, |t| super::court::dist(ctx.pos(i), t));
        if avoid != 0 && level >= 2 && (target.is_none() || gap >= 2.0) {
            angle = self.avoid(ctx, i, angle, avoid, target);
        }
        match target {
            Some(_) if turn(angle, self.motion.history[0]).abs() < 6 => {
                // Hardly a change: keep the old angle.
                angle = self.motion.history[0];
                self.motion.history.copy_within(0..HISTORY - 1, 1);
            }
            // Close to the target: straight at it, unsmoothed.
            Some(_) if gap < ARRIVE[level as usize] => {
                angle = self.remember(angle);
            }
            _ => angle = self.smooth(ctx, i, angle, avoid),
        }
        self.pad.strength = SPEEDS[level as usize];
        self.pad.angle = angle;
    }

    /// AIPhysPlayer vt+0x44 (`FUN_0017e7d8`): mode 2 drops to 1 past 35
    /// toward the rim the team attacks.
    fn avoid_mode(&self, ctx: &Ctx, i: usize, avoid: i32) -> i32 {
        if avoid == 2 && ctx.side(ctx.team(i)) as f32 * ctx.pos(i)[0] > 35.0 {
            1
        } else {
            avoid
        }
    }

    /// AIPhysPlayer vt+0x3c: keep off the court's edges (`FUN_0017ec78`),
    /// then smooth the turn (`FUN_0013b598`).
    fn smooth(&mut self, ctx: &Ctx, i: usize, angle: i32, avoid: i32) -> i32 {
        let angle = edges(ctx, i, angle);
        let mut out = angle;
        let h = &self.motion.history;
        let first = turn(angle, h[0]);
        let span = if avoid == 2 { HISTORY } else { ZIGZAG };
        if first != 0 {
            // A turn that changes direction against the recent ones is a
            // zigzag: keep the old angle.
            let mut sign = 0;
            let mut steps = vec![first];
            steps.extend((1..HISTORY).map(|k| turn(h[k - 1], h[k])));
            for &d in steps.iter().take(span) {
                if d != 0 {
                    let s = d.signum();
                    if sign != 0 && sign != s {
                        out = h[0];
                        return self.remember(out);
                    }
                    sign = s;
                }
            }
        }
        // Turns of 15..198 units are taken 14 at a time (14.7222 and
        // 199.611 truncated, `0x13b59c`, `0x13b704`).
        let size = first.abs();
        if size < 199 && 14 < size {
            out = h[0] + if first < 0 { -14 } else { 14 };
        }
        self.remember(out)
    }

    /// Push an angle onto the history. The original keeps the unmasked
    /// sum of a bend; the rewrite masks it to the turn so a long run of
    /// bends cannot drift out of `turn`'s range.
    fn remember(&mut self, angle: i32) -> i32 {
        let angle = angle & 0x3ff;
        self.motion.history.copy_within(0..HISTORY - 1, 1);
        self.motion.history[0] = angle;
        angle
    }

    /// AIPhysPlayer vt+0x34 (`FUN_0017e858`): bend round the nearest
    /// players ahead, by the grid at `0x4b9da0` (filled at `0x1841a0`).
    fn avoid(&self, ctx: &Ctx, i: usize, angle: i32, mode: i32, target: Option<[f32; 2]>) -> i32 {
        let range = match mode {
            1 => 16.0,
            2 => 18.0,
            _ => return angle,
        };
        let gap = target.map_or(100.0, |t| super::court::dist(ctx.pos(i), t));
        let (c, s) = (
            crate::input::native_cos_1024(angle),
            crate::input::native_sin_1024(angle),
        );
        let me = ctx.pos(i);
        let team = ctx.team(i);
        let mut total = 0;
        let mut hits = 0;
        let mut urgent = false;
        for j in ctx.by_distance(i).into_iter().take(5) {
            let d = ctx.between(i, j);
            if d >= range || hits >= 2 {
                break;
            }
            let o = ctx.pos(j);
            let (dx, dz) = (o[0] - me[0], o[1] - me[1]);
            let ahead = -(s * dx + c * dz) - 0.5;
            if !(0.0 < ahead && ahead < range) {
                continue;
            }
            let side = c * dx - s * dz;
            if !(side < range * 0.5 && -range * 0.5 < side) {
                continue;
            }
            let row = (ahead / (range * 0.125)) as usize;
            let col = ((side + range * 0.5) / (range * 0.125)) as usize;
            let mut v = AVOID[row.min(7)][col.min(7)];
            if hits == 1 {
                v /= 4;
                v = if total < 0 { -v.abs() } else { v.abs() };
            }
            let mate = ctx.team(j) == team;
            if mate {
                v /= 2;
            }
            total += v;
            hits += 1;
            if !mate && (hits == 1 || d <= 4.0) {
                urgent = true;
            }
        }
        if mode == 2 && urgent {
            if gap <= 2.0 {
                return angle + total;
            }
            let scale = if gap <= 25.0 {
                ((gap - 2.0) * 150.0 / 23.0 + 150.0) as i32
            } else {
                300
            };
            total = (scale * total / 100).clamp(-256, 256);
        }
        angle + total
    }
}

/// `FUN_0017ec78`: in live play, turn away from the court's ends (40.5)
/// and sides (25).
fn edges(ctx: &Ctx, i: usize, angle: i32) -> i32 {
    if !ctx.live() {
        return angle;
    }
    let p = ctx.pos(i);
    let mut a = angle;
    if 40.5 <= p[0] {
        if 512 < a {
            a = if 768 < a { 0 } else { 512 };
        }
    } else if p[0] <= -40.5 && a < 512 {
        a = if 256 < a { 512 } else { 0 };
    }
    if 25.0 <= p[1] {
        if 256 < a && a < 768 {
            a = if 512 < a { 768 } else { 256 };
        }
    } else if p[1] <= -25.0 {
        a = a.clamp(256, 768);
    }
    a
}

/// The steering grid (`0x4b9da0`): rows by distance ahead in eighths of
/// the range, columns by distance across.
const AVOID: [[i32; 8]; 8] = [
    [-16, -33, -67, -79, 80, 68, 34, 17],
    [-16, -33, -67, -67, 80, 80, 34, 17],
    [-13, -27, -56, -56, 57, 57, 28, 14],
    [-13, -25, -50, -50, 51, 51, 26, 14],
    [-10, -22, -45, -45, 46, 46, 23, 11],
    [-10, -19, -39, -39, 40, 40, 20, 11],
    [-8, -16, -33, -33, 34, 34, 17, 9],
    [-8, -8, -16, -16, 17, 17, 9, 9],
];
