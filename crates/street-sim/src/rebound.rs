//! Rebounds as the original takes them in the air: the rebound jump (actor
//! state bit 8, `FUN_0015ce50`), the aim its script's game callback 36
//! gives it (`FUN_00175858`) and the catch (`FUN_0016ed18`). Traced raw
//! 2026-10-16 (docs/TRACE.md "Dunks and rebounds on the ball's states").
//!
//! A jump pressed while the ball is a rebound coming off (ball+0x180 0),
//! a shot that has touched the hoop (6) or loose (1) is a rebound jump; at
//! any other ball a block jump (`FUN_0015d3a0`, the rewrite's
//! [`Action::Jumping`]). The rebound jump plays one of seventeen scripts,
//! picked by where the ball will be; most leave the floor at their
//! callback 36, which sends the jumper up so that his hand meets the ball
//! on its way. While he is in the air his hand takes any ball nobody holds
//! within reach.
//!
//! The rewrite's own, labelled where it is used: the original poses the
//! jumper's hand bones at the clip frame its script names; the rewrite has
//! no bones, so the hand is his standing reach above his feet
//! ([`crate::Ratings::reach`]), carried rigidly on the jump (as the
//! tip-off's, [`crate::toss::TIP_REACH`]), and the standing catch's the
//! point the original's script choice measures; the hand takes the ball
//! only once he has left the floor (the crouch before it holds the hands
//! low); the arms' bone sweep (`FUN_00137db8`), which also catches with
//! the forearms and knocks the ball off the body, and a pass in the air
//! are not taken. The goaltend pair (kind 6) sets bit 12 with its `9b`,
//! not 8: with the disc's animations it is a block jump ([`crate::block`]:
//! callback 35's aim, the block test's hands).

use crate::ball::RIM_ENTRY;
use crate::pose::{self, Frame};
use crate::shooting::DUNK_PACE;
use crate::{
    Action, BallState, OpeningToss, Phase, World, distance, game_phase, target_hoop, timing,
};

/// One rebound jump script: its address in the executable, the script
/// tick its game callback 36 sends the jumper up at (35 for the goaltend
/// pair, kind 6; 0 for the standing catch, which has no jump), the clip
/// frame that callback poses the hand at, and the updates its last wait
/// (`8d ee H C`) holds him after he lands (the standing catch's: its whole
/// length, a 4-update blend and a 55-update wait). Read from the scripts:
/// each opens with callback 47's branch (`84 2f`, eleven of the seventeen;
/// the goaltend pair's `84 30` is callback 48's, [`crate::block`]), callback
/// 6's turn, a 4-update blend to the path's clip and a wait (6 to 10
/// updates in all) before the callback, then `8d ee F ef 00` (frame F in the
/// updates the callback aimed for), `8d ee G fe` (until he lands), the
/// tail, and callbacks 37, 7 and 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Script {
    pub address: u32,
    pub jump: u8,
    pub frame: u8,
    pub tail: u8,
    /// The blend's record on callback 47's plain path, then on its branch
    /// (the same twice for the four without a branch).
    pub records: [u16; 2],
    /// Callback 6's turn on each path, units (`83 6 (a, 10, 0)`), or none
    /// (the standing catch).
    pub turn: Option<[i16; 2]>,
    /// The clip frame the blend starts the record at.
    pub start: u8,
    /// The frames the waits reach: before the aim (over `jump - 4`
    /// updates; the standing catch's whole wait, over `tail - 4`), by the
    /// landing (`8d ee G fe`) and at the tail's end.
    pub before: u8,
    pub land: u8,
    pub last: u8,
}

#[allow(clippy::too_many_arguments)]
const fn script(
    address: u32,
    jump: u8,
    frame: u8,
    tail: u8,
    records: [u16; 2],
    turn: Option<[i16; 2]>,
    start: u8,
    [before, land, last]: [u8; 3],
) -> Script {
    Script {
        address,
        jump,
        frame,
        tail,
        records,
        turn,
        start,
        before,
        land,
        last,
    }
}

const LEFT_RIGHT: Option<[i16; 2]> = Some([-128, 128]);
const STRAIGHT: Option<[i16; 2]> = Some([0, 0]);
const HALF_TURN: Option<[i16; 2]> = Some([512, 512]);
const QUARTERS: Option<[i16; 2]> = Some([-256, 256]);

/// The rebound jump's scripts (`rebound_*` in the animation records),
/// decoded from their bytes.
pub const SCRIPTS: [Script; 17] = [
    script(0x442ca0, 8, 9, 6, [419, 419], STRAIGHT, 0, [4, 22, 26]),
    script(0x442505, 9, 12, 6, [414, 415], LEFT_RIGHT, 0, [5, 22, 26]),
    script(0x4425cd, 9, 15, 12, [416, 416], HALF_TURN, 0, [5, 23, 31]),
    script(0x442660, 8, 9, 13, [434, 435], QUARTERS, 0, [3, 18, 27]),
    script(0x442728, 7, 16, 6, [412, 413], LEFT_RIGHT, 3, [6, 24, 28]),
    script(0x4427f0, 9, 17, 6, [410, 411], LEFT_RIGHT, 3, [7, 27, 31]),
    script(0x4428b8, 6, 12, 6, [418, 417], LEFT_RIGHT, 0, [3, 23, 27]),
    script(0x442980, 8, 17, 6, [433, 432], LEFT_RIGHT, 9, [13, 26, 30]),
    script(0x442d33, 8, 13, 6, [420, 420], STRAIGHT, 2, [6, 22, 26]),
    script(0x442a48, 8, 11, 6, [424, 423], LEFT_RIGHT, 2, [6, 19, 23]),
    script(0x442e59, 8, 13, 6, [422, 421], STRAIGHT, 1, [5, 22, 26]),
    script(0x442dc6, 8, 11, 6, [431, 431], STRAIGHT, 0, [4, 22, 26]),
    script(0x442b10, 8, 15, 6, [426, 425], LEFT_RIGHT, 1, [5, 24, 28]),
    script(0x442bd8, 8, 15, 6, [428, 427], LEFT_RIGHT, 0, [4, 23, 27]),
    script(0x441f0c, 10, 13, 0, [449, 448], None, 0, [5, 0, 0]),
    script(0x441fbc, 8, 14, 0, [451, 450], None, 2, [5, 0, 0]),
    script(0x442f10, 0, 0, 59, [430, 429], None, 0, [31, 31, 31]),
];

/// The blend each script opens with, updates.
const BLEND: u16 = 4;

/// The scripts by kind (the lists at `0x3d7138`, counted at `0x43f818`),
/// as indexes into [`SCRIPTS`]: by where the ball will be, 0 ahead of him,
/// 1 behind, 2 low ahead, 3 low behind, 4 near the rim ahead, 5 near the
/// rim behind; 6 a defender's at a slow ball over the ring, 7 a standing
/// catch, 8 under the basket. The attract captures' 37 rebound jumps
/// played kind 2's script 14 times, 0's 10, 4's (two shared with 5 and 8)
/// 8, 3's 3, and two none of these.
pub const KINDS: [&[u8]; 9] = [
    &[0],
    &[1, 2, 3, 4, 5, 6, 7],
    &[8],
    &[9],
    &[10, 11, 12],
    &[12, 13],
    &[14, 15],
    &[16],
    &[11],
];

/// The standing catch kind.
const STANDING: usize = 7;
/// The goaltend kind (a defender at a slow ball over the ring).
const GOALTEND: usize = 6;

/// How far from the ball a hand takes it: `max(1.5, ball radius)`
/// (`FUN_0016ed18`, `FUN_0015ce50`).
pub const CATCH_REACH: f32 = 1.5;

/// `DAT_004b8780` (set once by `FUN_0015ce50`, `0x15ce98..0x15cecc`): the
/// point the script choice holds the ball to, in the model's frame (it
/// looks along -z), times the skeleton's scale (skeleton+0x40).
pub const HAND_POINT: [f32; 3] = [
    0.0,
    f32::from_bits(0x40ef_4450),
    f32::from_bits(0xbfad_09c0),
];

/// The updates ahead the script choice looks at the ball: `trunc(fps ×
/// 0.33826)` (`0x3ead31f3`) for the standing catch, then `fps / 2`.
pub const AHEAD: [usize; 2] = [20, 30];

/// The rebound jump's height: `2 + 4 × Rebounds / 99` at most (callback
/// 36, `0x175b68..0x175bf8`), 1 at least.
const JUMP_TOP: (f32, f32) = (2.0, 4.0);

/// The ball's place and velocity (units an update) `n` updates ahead, for
/// `n` from 1: its own update run on a copy (`FUN_001537d0`, the
/// prediction ring `FUN_001538f8` fills with the look-ahead flag set).
pub(crate) fn ball_path(w: &World, n: usize) -> Vec<([f32; 3], [f32; 3])> {
    let mut b = w.ball.clone();
    let rims = w.hoops.map(|h| h.rim);
    let how = crate::ball::Step {
        watch: true,
        staged: w.sequence.is_some(),
        ..crate::ball::Step::default()
    };
    (0..n)
        .map(|_| {
            b.step(rims, how);
            (b.position, b.velocity.map(|v| v * crate::TICK_SECONDS))
        })
        .collect()
}

fn len3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `FUN_0016f580(actor, 1)`: under the basket, behind the rim's middle
/// within 2 across or behind the backboard within 10 (either basket).
pub(crate) fn under_basket(at: [f32; 2]) -> bool {
    let (x, z) = (at[0].abs(), at[1].abs());
    (z <= 2.0 && x > 37.0) || (x > 38.0 && z <= 10.0)
}

/// Callback 47 (`FUN_001779f8`, raw `0x1779f8..0x177ce0`): the ball's place
/// `fps / 2` updates on lies to one side of his facing word: the angle from
/// it to him (`FUN_0016f2b0`'s form) less the word (`FUN_0018cd78`) is 0 or
/// less (`slti 1`; the `movz` keeps the 1 then). The `84` takes the
/// branch's clip on that answer.
pub(crate) fn branch_47(at: [f32; 2], word: i32, path: &[([f32; 3], [f32; 3])]) -> bool {
    let ball = path[AHEAD[1] - 1].0;
    let toward = crate::ordinary::native_heading([at[0], 0.0, at[1]], ball);
    timing::angle_diff(toward, word) < 1
}

/// What a rebound jumper's channel 0 shows ([`World::rebound_clock`]).
struct Clock {
    record: u16,
    frame: f32,
    /// Through the blend: the clip held (none before the first blend) and
    /// the incoming clip's weight.
    blend: Option<(Option<(u16, f32)>, f32)>,
}

/// The script's clip frame on tick `tick`: its start through the blend,
/// then each wait's even steps to its frame (`8d`): the one before the aim,
/// the aim's (`8d ee F ef 00`) over the `meet` updates callback 36 aimed
/// for, the landing's over the rest of the `airtime` (or to the update he
/// landed on), the tail's over its updates. The standing catch's one wait
/// spans its tail after the blend.
fn clip_frame(s: &Script, tick: u16, meet: u8, airtime: u16, landed: Option<u16>) -> f32 {
    let mut frame = f32::from(s.start);
    if tick < BLEND {
        return frame;
    }
    let mut at = BLEND;
    let jump = u16::from(s.jump);
    let waits: [(u8, u16); 4] = if jump == 0 {
        [
            (s.before, u16::from(s.tail).saturating_sub(BLEND).max(1)),
            (s.before, 0),
            (s.before, 0),
            (s.before, 0),
        ]
    } else {
        let meet = u16::from(meet).max(1);
        let air = match landed {
            Some(t) => t.saturating_sub(jump + meet),
            None => airtime.saturating_sub(meet),
        };
        [
            (s.before, jump.saturating_sub(BLEND).max(1)),
            (s.frame, meet),
            (s.land, air.max(1)),
            (s.last, u16::from(s.tail).max(1)),
        ]
    };
    for (k, (to, n)) in waits.into_iter().enumerate() {
        if n == 0 {
            break;
        }
        if k == 1 && tick < jump {
            return frame;
        }
        let to = f32::from(to);
        if tick < at + n {
            let done = f32::from(tick - at + 1);
            return frame + (to - frame) * done / f32::from(n);
        }
        frame = to;
        at += n;
    }
    frame
}

impl World {
    /// Callback 37 (`FUN_001774f8`), called as a rebound script's tail
    /// ends (all but the goaltend pair's, kind 6): the rebounder holding
    /// the ball with two or more of the other side within 5 of him (the
    /// distance table, `FUN_0016e948`: the places the update began with)
    /// plays the clearout at once (`clearout01`, `0x456d02`), no turbo
    /// spent. Seven of the attract captures' ten clearouts are these.
    pub(crate) fn rebound_landed(&mut self, i: usize, script: u8, start: &[[f32; 2]]) {
        if matches!(script, 14 | 15) || self.ball.state != BallState::Held(i) {
            return;
        }
        let team = self.players[i].team;
        let at = start.get(i).copied().unwrap_or(self.players[i].position);
        let near = (0..self.players.len())
            .filter(|&j| {
                self.players[j].team != team
                    && distance(
                        at,
                        start.get(j).copied().unwrap_or(self.players[j].position),
                    ) <= 5.0
            })
            .count();
        if near >= 2 {
            self.start_clearout(i);
        }
    }
    /// The rebound jump's hand for player `i`: the rewrite's, his standing
    /// reach above his feet, or for the standing catch the point the
    /// script choice measured ([`HAND_POINT`]).
    pub(crate) fn rebound_hand(&self, i: usize) -> [f32; 3] {
        let p = &self.players[i];
        if let Action::ReboundJump { script, .. } = p.action
            && SCRIPTS[usize::from(script)].jump == 0
        {
            return self.hand_point(i);
        }
        [p.position[0], p.height + p.ratings.reach, p.position[1]]
    }

    /// [`HAND_POINT`] turned to where player `i` looks and scaled by his
    /// size: the skeleton's scale is the model's `height × 1.2 / 78`
    /// (docs/FORMATS.md), here his height from his reach (the rewrite's).
    fn hand_point(&self, i: usize) -> [f32; 3] {
        let p = &self.players[i];
        let scale = p.height_ft() * 12.0 * 1.2 / 78.0;
        let ahead = -HAND_POINT[2] * scale;
        let f = p.forward();
        [
            p.position[0] + f[0] * ahead,
            p.height + HAND_POINT[1] * scale,
            p.position[1] + f[1] * ahead,
        ]
    }

    /// `FUN_0015ce50`: player `i` starts a rebound jump. The script's kind
    /// comes from the ball [`AHEAD`]: a ball 20 ahead within reach of the
    /// [`HAND_POINT`] while he stands is the standing catch (7); else, 30
    /// ahead, no higher than his standing reach (`height × 1.2 × 1.25`) is
    /// low, at 12 or above it is a goaltend for a defender when within the
    /// ring (`r + 0.79322` across of the rim the side with the ball
    /// attacks, `r + 15` up) and slower than 10 mph a frame (6), and near
    /// when within 4 of that rim across, or he is; under the basket is 8
    /// (`FUN_0016f580(actor, 1)`); otherwise behind him (the sector
    /// `FUN_0018d768` of his facing to it 7, 0 or 1; 4 is straight ahead)
    /// 3 low, 5 near, else 1, ahead of him 2 low, 4 near, else 0. He faces
    /// the way he moves first (actor+0xb0 = +0xa8). The script is `r %
    /// count` of the kind's list (`FUN_0019c2f8` plays it).
    pub(crate) fn start_rebound_jump(&mut self, i: usize) {
        let path = ball_path(self, AHEAD[1]);
        let kind = self.rebound_kind(i, &path);
        let list = KINDS[kind];
        let pick = list[(self.rng.next_u32() % list.len() as u32) as usize];
        // The goaltend pair's `9b` sets bit 12, not 8: with the disc's
        // animations a block jump ([`crate::block`]: callback 35's aim, the
        // block test's hands).
        if kind == GOALTEND && self.poses.get().is_some() {
            let script = crate::block::GOALTEND[usize::from(pick == 15)];
            let jump = self.new_block_jump(i, script, false);
            self.players[i].action = Action::Jumping {
                elapsed: 0.0,
                high: false,
                block: Some(jump),
            };
            return;
        }
        let s = SCRIPTS[usize::from(pick)];
        let standing = s.jump == 0;
        // Callback 47 (`84 2f`, `FUN_001779f8`) on the facing he moves with
        // (+0xb0 = +0xa8), then callback 6's turn of the path taken.
        let p = &self.players[i];
        let word = timing::facing_word(p.heading);
        let branch = s.records[0] != s.records[1] && branch_47(p.position, word, &path);
        let from = self.poses.get().map(|poses| {
            let f = self.gait_frame(i);
            let held = poses
                .frames(f.record)
                .map_or(f.time, |n| f.time.rem_euclid(n as f32));
            (f.record, held)
        });
        // Callback 6 (`FUN_00174f30`): his wanted facing (+0xb0) turned to
        // the ball plus the path's turn, unless that is within 10 of it;
        // the facing then turns to it ([`World::rebound_turn`]).
        let mut want = word;
        if let Some(turns) = s.turn {
            let toward = self.callback_6_toward(i, &path);
            let to = (toward + i32::from(turns[usize::from(branch)])) & 0x3ff;
            if 10 <= timing::angle_diff(word, to).abs() {
                want = to;
            }
        }
        self.players[i].action = Action::ReboundJump {
            script: pick,
            tick: 0,
            vy: 0.0,
            drift: [0.0; 2],
            up: false,
            meet: 0,
            end: if standing {
                u16::from(s.tail)
            } else {
                u16::MAX
            },
            barred: false,
            branch,
            from,
            airtime: 0,
            word: want,
        };
    }

    /// Callback 6's turn: the facing toward the jump's word, 46 an update
    /// from the update after the press (the press update turns as the gait
    /// did), as the block jump's callback 5 (the attract captures' rebound
    /// jumps turn by 46 an update: 51 of their 87 turning updates, the rest
    /// the last part of a turn or the press update's).
    fn rebound_turn(&mut self, i: usize) {
        let Action::ReboundJump { tick, word, .. } = self.players[i].action else {
            return;
        };
        if tick == 0 {
            return;
        }
        let now = timing::facing_word(self.players[i].heading);
        let d = timing::angle_diff(word, now).clamp(-crate::block::TURN, crate::block::TURN);
        if d != 0 {
            self.players[i].heading = crate::restart::heading_radians((now + d) & 0x3ff);
        }
    }

    /// Callback 6's angle (`FUN_00174f30`, raw `0x174f30..0x1751f4`): from
    /// the ball to player `i` (`FUN_0016f2b0`'s form), the ball's place
    /// `fps / 3` updates on when nobody holds it (`FUN_001537d0`).
    fn callback_6_toward(&self, i: usize, path: &[([f32; 3], [f32; 3])]) -> i32 {
        let p = &self.players[i];
        let feet = [p.position[0], p.height, p.position[1]];
        let ball = match self.ball.state {
            BallState::Held(_) => self.ball.position,
            _ => path[(crate::TICK_HZ / 3) as usize - 1].0,
        };
        crate::ordinary::native_heading(feet, ball)
    }

    fn rebound_kind(&self, i: usize, path: &[([f32; 3], [f32; 3])]) -> usize {
        let p = &self.players[i];
        let r = self.ball.radius;
        let reach = CATCH_REACH.max(r);
        let rim = self.rim_spot(target_hoop(self.possession));
        if p.height == 0.0 {
            let at = path[AHEAD[0] - 1].0;
            if len3(sub3(self.hand_point(i), at)) < reach {
                return STANDING;
            }
        }
        let (at, v) = path[AHEAD[1] - 1];
        let low = at[1] <= p.height_ft() * 1.2 * 1.25;
        let (mut goaltend, mut near) = (false, false);
        if !low && 12.0 <= at[1] {
            let across = distance([at[0], at[2]], rim);
            goaltend = at[1] <= r + 15.0
                && across <= r + f32::from_bits(0x3f4b_10cb)
                && self.possession != p.team
                && len3(v) <= DUNK_PACE[0];
            near = across <= 4.0 || distance(p.position, rim) <= 4.0;
        }
        let sector = crate::timing::sector(
            crate::timing::facing_word(p.heading),
            p.position,
            [at[0], at[2]],
        );
        let behind = matches!(sector, 7 | 0 | 1);
        if under_basket(p.position) {
            8
        } else if goaltend {
            6
        } else if behind {
            if low {
                3
            } else if near {
                5
            } else {
                1
            }
        } else if low {
            2
        } else if near {
            4
        } else {
            0
        }
    }

    /// The rebound script's game callback 36 (`FUN_00175858`, raw
    /// `0x175b68..0x175fec`) at its jump tick: player `i` leaves the floor
    /// so that his hand meets the ball. The highest jump is `2 + 4 ×
    /// Rebounds / 99` (`v_max = √(2g top)`), the lowest 1 (`v_min = √(2g)`),
    /// across at most `0.75 v_max` an update. From `trunc(v_max / g) + 1`
    /// updates down to 1 it tries the ball's place `n` ahead (raised to
    /// the hand's height when lower, until one has worked): the velocity
    /// that brings the hand there in `n` updates, `(P − hand) / n` plus
    /// `g n / 2` up, passing over one that would meet the ball coming down
    /// (but the first, when it is 1). It keeps the first that works (rising
    /// within the bounds, across within its limit), then any that works
    /// with a smaller speed, or while none has worked any smaller one; the
    /// rise is then held to its bounds and the speed across to its limit.
    /// The jump lasts until he lands (`FUN_0019bed8`); `meet` is the
    /// updates it aimed for (without a working one, those to the top of
    /// the jump), the script's `8d ee F ef 00` wait.
    pub(crate) fn rebound_aim(&mut self, i: usize) {
        let g = crate::toss::JUMP_GRAVITY;
        let hand = self
            .rebound_aim_hand(i)
            .unwrap_or_else(|| self.rebound_hand(i));
        let p = &self.players[i];
        let rebounds = (p.ratings.rebounds * 99.0).round().clamp(0.0, 99.0);
        let top = (rebounds / 99.0).clamp(0.0, 1.0) * JUMP_TOP.1 + JUMP_TOP.0;
        let v_max = (top * 2.0 * g).sqrt();
        let v_min = (2.0 * g).sqrt();
        let across = v_max * 0.75;
        let updates = (v_max / g) as i32 + 1;
        let path = ball_path(self, updates.max(0) as usize);
        let mut best = [0.0_f32; 3];
        let mut meet = -1;
        let mut works = false;
        let mut n = updates;
        while n > 0 {
            let k = n as f32;
            let at = path[(n - 1) as usize].0;
            let mut y = at[1];
            if y < hand[1] {
                if works {
                    n -= 1;
                    continue;
                }
                y = hand[1];
            }
            let v = [
                (at[0] - hand[0]) / k,
                (y - hand[1]) / k + g * 0.5 * k,
                (at[2] - hand[2]) / k,
            ];
            if (meet != -1 || n >= 2) && v[1] - g * k < 0.0 {
                n -= 1;
                continue;
            }
            let bad = v[1] < v_min || v_max < v[1] || (v[0] * v[0] + v[2] * v[2]).sqrt() > across;
            if meet == -1 {
                meet = n;
                works = !bad;
                best = v;
            } else if bad && works {
            } else if !works && !bad {
                works = true;
                meet = n;
                best = v;
            } else if len3(v) < len3(best) {
                meet = n;
                best = v;
            }
            n -= 1;
        }
        best[1] = best[1].clamp(v_min, v_max);
        let side = (best[0] * best[0] + best[2] * best[2]).sqrt();
        if across < side {
            best[0] *= across / side;
            best[2] *= across / side;
        }
        if !works {
            meet = (best[1] / g) as i32;
        }
        let airtime = u16::from(timing::airtime(best[1], self.players[i].height));
        if let Action::ReboundJump {
            vy,
            drift,
            up,
            meet: aimed,
            airtime: air,
            ..
        } = &mut self.players[i].action
        {
            *vy = best[1];
            *drift = [best[0], best[2]];
            *up = true;
            *aimed = meet.clamp(0, 255) as u8;
            *air = airtime;
        }
        // His face for the jump (`FUN_00159190(actor, 1, airtime)`,
        // `0x175fd0..0x175fd8`).
        self.face(i, i32::from(airtime));
    }

    /// The clip and frame rebound jumper `i`'s channel 0 shows, and the
    /// blend's held clip and weight through the script's first 4 updates
    /// ([`Clock`]).
    fn rebound_clock(&self, i: usize) -> Option<Clock> {
        let Action::ReboundJump {
            script,
            tick,
            meet,
            end,
            branch,
            from,
            airtime,
            ..
        } = self.players[i].action
        else {
            return None;
        };
        let s = SCRIPTS[usize::from(script)];
        let landed =
            (end != u16::MAX && s.jump != 0).then(|| end.saturating_sub(u16::from(s.tail)));
        Some(Clock {
            record: s.records[usize::from(branch)],
            frame: clip_frame(&s, tick, meet, airtime, landed),
            blend: (tick < BLEND).then(|| (from, f32::from(tick + 1) / f32::from(BLEND))),
        })
    }

    /// Rebound jumper `i`'s channel 0 ([`World::rebound_clock`]): the
    /// script's clip and frame, the clip held through its blend.
    pub(crate) fn rebound_channel(&self, i: usize) -> Option<crate::held::Channel> {
        let c = self.rebound_clock(i)?;
        let Action::ReboundJump { tick, .. } = self.players[i].action else {
            return None;
        };
        Some(crate::held::Channel {
            shown: crate::trick::Shown {
                record: Some(c.record),
                frame: c.frame,
                blend: c.blend.map(|(_, w)| w),
            },
            from: c.blend.and_then(|(from, _)| from),
            counter: c.blend.map(|_| (tick + 1, BLEND)),
            gait: false,
        })
    }

    /// Callback 36's hand (`FUN_00175858`, raw `0x175858..0x175b64`): the
    /// path's clip posed at the script's frame where he stands, facing his
    /// wanted facing (+0xb0), and `FUN_0014da20`'s point (flag 0, out by
    /// the ball's radius) by that frame's hands: both, 0xc, the point
    /// between; bit 8 the right, else the left. None without the disc's
    /// animations.
    fn rebound_aim_hand(&self, i: usize) -> Option<[f32; 3]> {
        let poses = self.poses.get()?;
        let Action::ReboundJump { script, branch, .. } = self.players[i].action else {
            return None;
        };
        let s = SCRIPTS[usize::from(script)];
        let record = s.records[usize::from(branch)];
        let body = self.players[i].body();
        let mut place = self.place(i);
        place.word = match self.players[i].action {
            Action::ReboundJump { word, .. } => word,
            _ => place.word,
        };
        let at = Frame {
            record,
            time: f32::from(s.frame),
            looping: false,
        };
        let posed = poses.pose(at, body, place)?;
        let f = i32::from(s.frame);
        let state = poses.state(record, f);
        let r = Some(self.ball.radius);
        let hand = |k| pose::hand_point(&posed, i32::from(record), f, k, body, r);
        Some(if state & 0xc == 0xc {
            let (a, b) = (hand(1), hand(2));
            std::array::from_fn(|k| (a[k] + b[k]) * 0.5)
        } else {
            hand(if state & 8 != 0 { 1 } else { 2 })
        })
    }

    /// The hands `FUN_0016ed18` tries against the ball: channel 0's clip at
    /// its whole frame (+0x50, +0x5c) on the model as it is posed this
    /// update, `FUN_0014da20`'s points (flag 1) of both hands on 0xc, else
    /// the right on bit 8 and the left without it. None without the disc's
    /// animations.
    fn rebound_hands(&self, i: usize) -> Option<Vec<[f32; 3]>> {
        let poses = self.poses.get()?;
        let c = self.rebound_clock(i)?;
        let body = self.players[i].body();
        let place = self.place(i);
        let to = Frame {
            record: c.record,
            time: c.frame,
            looping: false,
        };
        let posed = match c.blend {
            Some((Some((record, time)), w)) => {
                let from = Frame {
                    record,
                    time,
                    looping: false,
                };
                poses.blend(from, to, w, body, place)?
            }
            _ => poses.pose(to, body, place)?,
        };
        let f = c.frame as i32;
        let state = poses.state(c.record, f);
        let hand = |k| pose::hand_point(&posed, i32::from(c.record), f, k, body, None);
        Some(if state & 0xc == 0xc {
            vec![hand(1), hand(2)]
        } else {
            vec![hand(if state & 8 != 0 { 1 } else { 2 })]
        })
    }

    /// A rebound jumper's script at the update before his step: callback
    /// 36 at its jump tick ([`World::rebound_aim`]).
    pub(crate) fn rebound_jump_tick(&mut self, i: usize) {
        self.rebound_turn(i);
        if let Action::ReboundJump {
            script, tick, up, ..
        } = self.players[i].action
        {
            let jump = SCRIPTS[usize::from(script)].jump;
            if jump != 0 && !up && tick == u16::from(jump) {
                self.rebound_aim(i);
            }
        }
    }

    /// `FUN_0016ed18`: rebound jumper `i`'s hand takes the ball, after his
    /// update and before the ball's (`FUN_0015da28`, the actor's update).
    /// Not while he is barred, nor from behind the backboard
    /// (`FUN_0016e830`: the ball above 10.5 in front of it, within 3 of
    /// the middle, and he beyond 38); the hand within [`CATCH_REACH`] of
    /// the ball. A shot on the rim (6) he may not take inside the ring
    /// (within 0.793225 across of the rim the side with the ball attacks,
    /// 12 up); outside it he takes it when `r % 100` is at most `(2 ×
    /// Rebounds + Blocks + Speed + Power) / 5` held to 5..90, else it slips
    /// him for the rest of the jump (actor+0x1a8, the airtime left). The
    /// catch is the give (`FUN_0014cb08`, slot 24): a rebound, the phase
    /// out of 3 to the catcher's court.
    pub(crate) fn rebound_catch(&mut self, i: usize) {
        let Action::ReboundJump {
            script, up, barred, ..
        } = self.players[i].action
        else {
            return;
        };
        let standing = SCRIPTS[usize::from(script)].jump == 0;
        if barred
            || !(up || standing)
            || self.phase != Phase::Live
            || self.opening_toss != OpeningToss::None
            || !matches!(self.ball.state, BallState::Shot | BallState::Loose)
        {
            return;
        }
        let b = self.ball.position;
        let p = &self.players[i];
        if b[1] > 10.5 && b[0].abs() < 38.0 && b[2].abs() < 3.0 && p.position[0].abs() > 38.0 {
            return;
        }
        let reach = CATCH_REACH.max(self.ball.radius);
        let hands = self
            .rebound_hands(i)
            .unwrap_or_else(|| vec![self.rebound_hand(i)]);
        if !hands.into_iter().any(|h| len3(sub3(h, b)) <= reach) {
            return;
        }
        if self.native_ball_state() == game_phase::ball::RIM {
            let rim = self.rim_spot(target_hoop(self.possession));
            if distance([b[0], b[2]], rim) < RIM_ENTRY {
                return;
            }
            let r = &p.ratings;
            let odds =
                ((2 * r.native(3) + r.native(5) + r.native(8) + r.native(4)) / 5).clamp(5, 90);
            if (self.rng.next_u32() % 100) as i32 > odds {
                if let Action::ReboundJump { barred, .. } = &mut self.players[i].action {
                    *barred = true;
                }
                return;
            }
        }
        self.take_ball(i);
        // His face (`FUN_00159190(catcher, 2, fps × 2)`, `0x16f1c0..
        // 0x16f1d4`).
        self.face(i, 2 * crate::TICK_HZ as i32);
        self.events.push(crate::Event::Pickup { player: i });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scripts_lists_match_the_executables_counts() {
        // `0x43f818`: 1, 7, 1, 1, 3, 2, 2, 1, 1.
        let counts: Vec<usize> = KINDS.iter().map(|k| k.len()).collect();
        assert_eq!(counts, [1, 7, 1, 1, 3, 2, 2, 1, 1]);
        assert!(
            KINDS
                .iter()
                .flat_map(|k| k.iter())
                .all(|&s| usize::from(s) < SCRIPTS.len())
        );
        // Only the standing catch has no jump; it lasts its blend and wait.
        let standing: Vec<_> = SCRIPTS.iter().filter(|s| s.jump == 0).collect();
        assert_eq!(standing.len(), 1);
        assert_eq!((standing[0].address, standing[0].tail), (0x442f10, 59));
        assert_eq!(KINDS[STANDING], [16]);
    }

    #[test]
    fn under_the_basket_is_behind_the_rim_or_the_board() {
        assert!(under_basket([37.5, 1.0]));
        assert!(under_basket([-38.5, 9.0]));
        assert!(!under_basket([37.5, 3.0]));
        assert!(!under_basket([36.0, 0.0]));
        assert!(!under_basket([39.0, 11.0]));
    }
}
