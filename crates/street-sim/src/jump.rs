//! The shooters', dunkers' and lay-up jumpers' flights on their scripts, as
//! the original moves an actor. Traced raw 2026-10-17 (docs/TRACE.md "The
//! jumps on their scripts").
//!
//! The actor's update (`FUN_00156f48`) runs his side's update, then his
//! channels (`FUN_0019c658`, the script's opcodes and the clip's steps),
//! then moves him: his place plus his velocity (actor+0x30 += +0x40,
//! `0x1573d8`), then, unless channel 0's flags hold 0x40000, gravity (y up
//! `g / 2`, the rise up `g`, `g = −32.152233 / fps² × 1.2596`), and on the
//! floor (y at most 0 falling) he stands (`0x1574fc`); last his facing turns
//! toward the wanted one (`FUN_00168ea8`, the vtable's +0x64: 46 units an
//! update, 23 for a dunker or a lay-up jumper (bits 7, 13), 34 while his
//! points are planted (0x80000)).
//!
//! The scripts drive the velocity and the clip's root:
//! - `9f v` (`FUN_00194c40`): the rise `v` (× 60 / fps), x and z kept; the
//!   jump shots, 0.25 (0.2125, 0.225, 0.15 for a few).
//! - `a1 81 a v w` (`FUN_00194d58`'s default): `v` along the wanted facing
//!   (+0xb0) turned by `a`, `w` up: the hooks.
//! - `a1 f0 a v w`: `v` along the line to the rim (`FUN_0016f260`) turned by
//!   `a`, `w` up: the fadeaway (−512), the runner, the lay-ups.
//! - callback 18 (`FUN_00172788`): the side fades, 15 / fps up and 7.2 /
//!   fps along the line to the rim turned by the clip ([`side_fade`]).
//! - `9e` (`FUN_001988b0`): the dunk's aim ([`Aim`]): a hand posed at the
//!   row's slam frame sent to a point by the ring `n` updates on.
//! - `ac` (`FUN_001998f0`): the lay-up's reach ([`Reach`]): the speed
//!   across held to the ring's distance over the airtime, and the hand kept
//!   in front of the rim.
//! - `aa k` (`FUN_00194848`): `k` times the last update's displacement
//!   (actor+0x220), after the slam.
//! - `97` the velocity zeroed; `93`/`95` plant the hands (no gravity) or
//!   the feet, `96` lets go.
//! - `91 w` (`FUN_00195858`) the flags: the clip's root moves him on x and
//!   z (`FUN_0019a658`, `0x19b4bc..0x19b70c`: the root's change times his
//!   height scale, turned by his facing) unless 0x1000 (set by the jumps'
//!   `91 .. 10 ..` and by `9e`); 0x10000 lowers him by the root's rise
//!   (`0x19b684..0x19b704`) until the clip's next whole frame outside a
//!   blend: the takeoff's few updates.
//!
//! A landing wait (`8d .. fe`) lasts `FUN_0019bed8(g, vy, y)` from its
//! start, so the scripts end when the actor lands.
//!
//! Without the disc's animations the clip's root does not move him and the
//! aimed hands stand in at his reach above his feet.

use crate::trick::{CURRENT, Step};
use crate::{Action, World, timing};

/// Channel 0's flag: the clip's root moves him only on y (`91`, `9e`).
pub const ROOT_OFF: u32 = 0x1000;
/// Channel 0's flag: his height follows the physics alone while the
/// clip's root rises at the takeoff (`0x19b684`).
pub const RESYNC: u32 = 0x10000;
/// Channel 0's flag: the side with the ball turns his wanted facing to
/// the rim each update (`FUN_0015a060`, `0x15a104..0x15a11c`; the side
/// fades' and the fadeaway's `91`).
pub const RIM_FACING: u32 = 0x400;
/// Channel 0's flag: no gravity (`93`, the planted hands).
pub const NO_GRAVITY: u32 = 0x40000;
/// Channel 0's flag: planted points (`93`, `95`).
pub const PLANTED: u32 = 0x80000;
/// Channel 0's flag: the planted points let go at the next motion step
/// (`96`).
pub const UNPLANT: u32 = 0x2000000;

/// `FUN_0019bed8`'s and the motion's gravity, units an update² (negative).
pub fn gravity() -> f32 {
    f32::from_bits(0x4200_9be3) / (60.0 * 60.0) * f32::from_bits(0xbfa1_3ba4)
}

/// `9e`'s operands (raw `0x198994..0x198b5c`): the one-hand pick `side`
/// (`0x85` the right hand's bone, 1; `0x86` the left's, −1), the hands'
/// `mid`point (`0x84`), the `second` bones (`0x83`: 0x12, 0xe for 0x13,
/// 0xf), the `direction` (`(direction − 1) × 0x80` round the rim from the
/// approach; 1 the baseline's side), the `updates` to the meet and the
/// clip `frame` the hand is posed at; the `records` posed ([`CURRENT`]:
/// the channel's), the second for the left hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    pub second: bool,
    pub mid: bool,
    pub side: i8,
    pub direction: u8,
    pub updates: u8,
    pub frame: u8,
    pub records: [u16; 2],
}

/// `ac`'s operands: `cap` the speed across (`sel` 1), the `updates` the
/// hand is carried, the clip `frame` it is posed at, `ahead` of it toward
/// the baseline (× his height scale).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    pub cap: bool,
    pub updates: u8,
    pub frame: u8,
    pub ahead: f32,
}

/// `93`'s and `95`'s operands (`FUN_00193780`, `FUN_00194228`, raw
/// `0x194228..0x19477c`): the two `bones` planted at their midpoint, or the
/// one the `pick` selector takes: `0xec` by the side of the court (his z
/// against the side his team attacks), `0xf2` the second for a
/// left-hander, `0xf3`/`0xf4` the one further back/ahead along his pelvis's
/// facing, `0xf7` by the clip's foot flags (0x40 the second, 0x200 the
/// first), `0xf8` by the live hand (2 the second); `keep` (`0xf1`) keeps
/// a planted pair of the same bones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plant {
    pub hands: bool,
    pub keep: bool,
    pub pick: u8,
    pub bones: [u8; 2],
}

/// The jump steps one update ran, in order.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ops {
    steps: [Option<Step>; 8],
}

impl Ops {
    pub(crate) fn push(&mut self, step: Step) {
        if let Some(slot) = self.steps.iter_mut().find(|s| s.is_none()) {
            *slot = Some(step);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Step> + '_ {
        self.steps.iter().map_while(|s| *s)
    }
}

/// An actor's motion on a move's script (actor+0x40, the channel's flags).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flight {
    /// Units an update (x, y, z).
    pub velocity: [f32; 3],
    /// Channel 0's flags as the script writes them.
    pub flags: u32,
    /// The script tick his landing wait ends, once it has started (-1 none).
    pub landing: i16,
    /// The last update's displacement (actor+0x220).
    pub moved: [f32; 3],
    /// The script and its updates the last step ran (the channel stands
    /// still through a person's release gate).
    pub script: u32,
    pub updates: i32,
    /// The planted bones (actor+0x100, +0x104) and their point (+0xf0..),
    /// held while [`PLANTED`] is set; the point moves with the velocity.
    pub bones: [u8; 2],
    pub point: [f32; 3],
    /// A dunk's updates to its aim's meet (actor+0x31c: `9e`'s count, an
    /// update less each update of his side's after it, `0x15a14c..
    /// 0x15a178`; 0 before the aim), which a block jumper's aim reads.
    pub meet: u8,
    /// The slide's deceleration (actor+0x184, `a0`): units an update off
    /// his speed each motion step until he stops (`0x157530..0x1575d0`).
    pub slide: f32,
    /// A dunk's or lay-up's knock window (`a6 1`, contact kind 1): its
    /// updates left, counted down each update.
    pub knock: u8,
    /// A held script's kind-2 window (`a6 2`), the same way.
    pub shove: u8,
    /// The men the move has met (actor+0x33c, one bit each,
    /// `FUN_00172008`) and those it beat (+0x358's count); cleared with
    /// the move (`FUN_00171fd0` at the script's end).
    pub met: u8,
    pub beat: u8,
    /// The jump's airtime and the updates since it began (actor+0x64 and
    /// +0x60: the launches `9f`, `a1` and the dive's set them, the actor
    /// update counts +0x60 up, `0x15713c`).
    pub air: u16,
    pub aloft: u16,
}

impl Flight {
    pub fn new(velocity: [f32; 3]) -> Self {
        Self {
            velocity,
            landing: -1,
            ..Self::default()
        }
    }
}

/// The moves whose scripts move him: a shot, a dunk, a leap, a lay-up, a
/// dive or a pass on its row's script (the putback keeps its own clock, as
/// does a passout to himself, [`crate::passout`]).
pub fn scripted(action: &Action) -> bool {
    match *action {
        Action::Shooting { .. } | Action::Layup { .. } | Action::Dive { .. } => true,
        Action::Dunking { row, .. } => timing::move_row(row).is_some(),
        // From the floor or the air (`air_*`, flag 0x400: out of a jump
        // shot, a dunk or a leap): the row's `91` turns his locomotion off
        // (0x2), its `97` stops him or he coasts, the clip's root carries
        // him.
        Action::Passing { row, .. } => {
            usize::from(row) < timing::PASS_ROWS.len() && crate::passout::toss(row).is_none()
        }
        _ => false,
    }
}

/// Channel 0's flags let the locomotion (`FUN_00163d40`) run: no 0x2, or
/// 0x2 with 0x10 (`0x15b084`).
pub fn walks(flags: u32) -> bool {
    flags & 0x2 == 0 || flags & 0x10 != 0
}

/// The facing turn of `FUN_00168ea8` for his move, units an update.
pub fn turn_rate(action: &Action, flags: u32) -> i32 {
    if flags & PLANTED != 0 {
        return 34;
    }
    match action {
        Action::Dunking { .. } | Action::Layup { .. } => 23,
        _ => crate::guard::TURN,
    }
}

/// `FUN_0016f580(actor, 0)`: behind the board at the end his team attacks.
pub(crate) fn behind_board(at: [f32; 2], side: f32) -> bool {
    at[0] * side >= 0.0 && crate::rebound::under_basket(at)
}

/// Callback 18 (`FUN_00172788`, raw `0x172788..0x1729f4`), word 1: the
/// side fades' jump: 15 / fps up, 7.2 / fps along the line to the rim
/// turned by the clip on channel 0 (326, 327 half a turn; 316..319 half a
/// turn back; 320 and 322 a quarter back, 321 and 323 a quarter on);
/// behind the board on 320..323 the line is the baseline's (0x300 at the
/// −x end, 0x100 at +x) and the drift 1.35 times.
pub fn side_fade(record: u16, to_rim: i32, behind: bool, rim_x: f32) -> [f32; 3] {
    let up = 15.0 / 60.0;
    let mut drift = f32::from_bits(0x40e6_6666) / 60.0;
    let (mut base, mut turn) = (to_rim, 0);
    match record {
        326 | 327 => turn = 0x200,
        316..=319 => turn = -0x200,
        320..=323 if behind => {
            base = if rim_x < 0.0 { 0x300 } else { 0x100 };
            drift *= f32::from_bits(0x3fac_cccd);
        }
        320 | 322 => turn = -0x100,
        321 | 323 => turn = 0x100,
        _ => {}
    }
    let a = base + turn;
    [
        -drift * crate::input::native_sin_1024(a),
        up,
        -drift * crate::input::native_cos_1024(a),
    ]
}

/// `FUN_00198618(near, dir, angle, limit)`: an aim from behind the rim's
/// line (the target beyond the rim toward the baseline at `near`'s end)
/// turned back to `limit` either side of the baseline's line (0x300 at +x,
/// 0x100 at −x), else 0.
pub fn behind_turn(near: f32, dir: i32, angle: i32, limit: i32) -> i32 {
    let s = crate::input::native_sin_1024(angle);
    let behind = if -s >= 0.0 { 1 } else { -1 };
    if (near >= 0.0 && behind != 1) || (near < 0.0 && behind != -1) {
        return 0;
    }
    let base = if near > 0.0 { 768 } else { 256 };
    let sign = if timing::angle_diff(dir & 0x3ff, base) > -1 {
        1
    } else {
        -1
    };
    let r = sign * limit + base - (angle & 0x3ff);
    if (r < 0 && sign == -1) || (r >= 0 && sign == 1) {
        r
    } else {
        0
    }
}

/// Radians to units as the aim converts them (`FUN_0018d768`'s `x ×
/// 162.97466 + 0.5`, truncated, without its turn's mask).
fn units(radians: f32) -> i32 {
    (radians * 162.974_66 + 0.5) as i32
}

/// `atan2(x, z)` as the aim's polynomial gives it, radians (libm's here).
fn atan2(x: f32, z: f32) -> f32 {
    crate::math::atan2(x, z)
}

/// What a `9e` aim needs of the clip it poses: the right and left hand
/// bones (0x13, 0xf) and the second ones (0x12, 0xe) at the aim's frame,
/// posed facing a word, and the clip's hand flags there.
pub trait Hands {
    /// The bones of `record` at `frame` facing `word`: `[0x13, 0xf, 0x12,
    /// 0xe]` and the state's hand flags.
    fn bones(&self, record: u16, frame: u8, word: i32) -> ([[f32; 3]; 4], u32);
}

/// `9e`'s aim (`FUN_001988b0`, raw `0x1988b0..0x1998e8`): the velocity, the
/// wanted facing it leaves and the bones' record.
///
/// The hand: posed at the aim's frame facing the wanted facing, the right
/// bone (0x13, or 0x12 with `second`) when the frame's flags hold the right
/// hand (8; the left too, only for a one-hand pick of the right), else the
/// left (0xf, 0xe, on the second record) when they hold it, else the right;
/// records 996 and 997 swap them; the `mid` point of the two. The approach
/// is half a turn from the wanted facing, turned `(direction − 1) × 0x80`
/// (direction 1: the baseline's side). One hand: the radius 0.89322 plus
/// 0.125 (none with `second`), the height 11.6667 (12.3333 with
/// `second`), an approach from behind the rim's line turned back to it
/// ([`behind_turn`], 256), the facing turned with it and the hand posed
/// again. Two hands or a pick: the hands' line (`atan2` of their x and z,
/// half their distance `d`) puts the approach a quarter round from it (the
/// nearer to the approach), then `asin(d / R)` either way picks the side
/// toward the end ([`behind_turn`] 114 on it), the radius `sqrt(R² − d²)`
/// (at least 0.15865) at 11.6; a pick goes `acos(r / R)` round by its side
/// at the full radius. A lay-up jumper (bit 13, not 8) aims the ball's
/// radius lower; records 891 and 892 the radius and 1 higher. The point is
/// the rim the side attacks less the radius along the approach; the
/// velocity `(point − hand) / n` plus `g n / 2` up.
#[allow(clippy::too_many_arguments)]
pub fn aim(
    hands: &dyn Hands,
    aim: Aim,
    record: u16,
    want: i32,
    near: f32,
    rim_x: f32,
    bits: u32,
    ball_radius: f32,
) -> ([f32; 3], i32) {
    let r_base = f32::from_bits(0x3f64_aa65);
    let current = |r: u16| if r == CURRENT { record } else { r };
    let (first, second_record) = (current(aim.records[0]), current(aim.records[1]));
    let mut word = want & 0x3ff;
    let (bones, flags) = hands.bones(first, aim.frame, word);
    // The bone (s6) and the record posed (s7).
    let right = if aim.second { 2 } else { 0 };
    let left = if aim.second { 3 } else { 1 };
    let mut bone = 0usize;
    let mut posed = first;
    if !aim.mid {
        let take_left = |fp: i8, f: u32| fp == -1 || f & 8 == 0;
        if flags & 8 != 0 {
            if aim.side == 1 || flags & 4 == 0 {
                bone = right;
            } else if take_left(aim.side, flags) {
                bone = left;
                posed = second_record;
            }
        } else if flags & 4 != 0 && take_left(aim.side, flags) {
            bone = left;
            posed = second_record;
        }
    }
    if (996..=997).contains(&posed) {
        bone = match bone {
            0 => 1,
            1 => 0,
            b => b,
        };
    }
    let pose_at = |w: i32| hands.bones(posed, aim.frame, w).0;
    // The first hand is the first record's pose; the record picked for
    // the left hand is posed only again after a turn.
    let mut posed_bones = bones;
    let midpoint = |b: &[[f32; 3]; 4]| std::array::from_fn(|k| (b[0][k] + b[1][k]) * 0.5);
    let mut hand: [f32; 3] = if aim.mid {
        midpoint(&posed_bones)
    } else {
        posed_bones[bone]
    };
    let mut s3 = (word + 512) & 0x3ff;
    let mut radius = r_base;
    if bits & 0x2000 != 0 {
        radius += ball_radius + 1.0;
    }
    if aim.direction == 1 {
        s3 = if 0.0 < near { 0x100 } else { 0x300 };
    } else if aim.direction >= 2 {
        s3 = (s3 + (i32::from(aim.direction) - 1) * 0x80) & 0x3ff;
    }
    let mut height = f32::from_bits(0x4139_999a);
    if aim.mid || aim.side != 0 {
        let b = &posed_bones;
        let (dx, dz) = (b[0][0] - b[1][0], b[0][2] - b[1][2]);
        let line = units(atan2(dx, dz)) & 0x3ff;
        let half = (dx * dx + dz * dz).sqrt() * 0.5;
        let (p1, p2) = ((line - 256) & 0x3ff, (line + 256) & 0x3ff);
        let d1 = timing::angle_diff(p1, s3).abs();
        let d2 = timing::angle_diff(p2, s3).abs();
        s3 = if d1 < d2 { p1 } else { p2 };
        let f6 = (half / radius).clamp(-1.0, 1.0);
        let across = units(libm::asinf(f6)) & 0x3ff;
        let (q1, q2) = ((s3 - across) & 0x3ff, (s3 + across) & 0x3ff);
        let lean = crate::input::native_sin_1024(q2) - crate::input::native_sin_1024(q1);
        let v1 = if lean >= 0.0 { 1 } else { -1 };
        let chosen = if near >= 0.0 {
            if v1 == 1 { q1 } else { q2 }
        } else if v1 == -1 {
            q1
        } else {
            q2
        };
        let t = behind_turn(near, s3, chosen, 114);
        if t != 0 {
            word = (word + t) & 0x3ff;
            posed_bones = pose_at(word);
            hand = match aim.side {
                1 => posed_bones[0],
                -1 => posed_bones[1],
                _ => midpoint(&posed_bones),
            };
            s3 += t;
        }
        let mut inner = if half < radius {
            (radius * radius - half * half).sqrt()
        } else {
            0.0
        };
        inner = inner.max(f32::from_bits(0x3e22_73d6));
        if aim.side == 0 {
            radius = inner;
        } else {
            let c = units(std::f32::consts::FRAC_PI_2 - libm::asinf(inner / r_base));
            s3 += i32::from(aim.side) * c;
            radius = r_base;
        }
    } else {
        if aim.second {
            height = f32::from_bits(0x4145_5552);
        } else {
            radius += 0.125;
            height = f32::from_bits(0x413a_aaae);
        }
        let t = behind_turn(near, s3, s3, 256);
        if t != 0 {
            word = (word + t) & 0x3ff;
            posed_bones = pose_at(word);
            hand = posed_bones[bone];
            s3 += t;
        }
    }
    if bits & 0x2000 != 0 && bits & 0x100 == 0 {
        height -= ball_radius;
    } else if (891..=892).contains(&posed) {
        height += ball_radius + 1.0;
    }
    let point = [
        rim_x - radius * crate::input::native_sin_1024(s3),
        height,
        -radius * crate::input::native_cos_1024(s3),
    ];
    let n = f32::from(aim.updates.max(1));
    let g = gravity();
    (
        [
            (point[0] - hand[0]) / n,
            (point[1] - hand[1]) / n - g * 0.5 * n,
            (point[2] - hand[2]) / n,
        ],
        word,
    )
}

/// `ac` (`FUN_001998f0`, raw `0x1998f0..0x199e14`): with `cap`, the speed
/// across held to `(rim distance − 0.79323 − 0.04335) / airtime`; then the
/// ball at the hand posed at the frame facing the wanted facing, `ahead` ×
/// his height scale (plus the ball's radius) toward the baseline, carried
/// `updates` at the velocity across: past the ring's front (35.91, within
/// `r + 0.83645` of the middle across) or else 38, the velocity on x takes
/// him back to it.
#[allow(clippy::too_many_arguments)]
pub fn reach(
    reach: Reach,
    velocity: [f32; 3],
    ball: [f32; 3],
    rim_distance: f32,
    airtime: u8,
    end_x: f32,
    scale: f32,
    ball_radius: f32,
) -> [f32; 3] {
    let mut v = velocity;
    if reach.cap {
        let limit = (rim_distance - f32::from_bits(0x3f4b_10cb) - f32::from_bits(0x3d31_0cb3))
            / f32::from(airtime.max(1));
        let across = (v[0] * v[0] + v[2] * v[2]).sqrt();
        if limit < across && across > 0.0 {
            v[0] = v[0] / across * limit;
            v[2] = v[2] / across * limit;
        }
    }
    let mut at = ball;
    let push = ball_radius + scale * reach.ahead;
    at[0] += if end_x < 0.0 { -push } else { push };
    let n = f32::from(reach.updates);
    at[0] += v[0] * n;
    at[2] += v[2] * n;
    let front = f32::from_bits(0x420f_a77a);
    let left = if at[2].abs() <= ball_radius + f32::from_bits(0x3f56_2196) {
        if end_x >= 0.0 {
            front - at[0]
        } else {
            -front - at[0]
        }
    } else {
        end_x - at[0]
    };
    let past = if end_x >= 0.0 {
        left < 0.0
    } else {
        left >= 0.0
    };
    if past {
        v[0] += left / n.max(1.0);
    }
    v
}

impl World {
    /// The rim `i`'s team attacks, x (`FUN_001404e0`: side × 36.75).
    fn attack_x(&self, i: usize) -> f32 {
        self.rim_spot(crate::target_hoop(self.players[i].team))[0]
    }

    /// `FUN_0016f260`: the angle from `i` toward the rim his team attacks
    /// (the game's table at +0x35c, built before the actors update, turned
    /// half round), from where he stood.
    pub(crate) fn to_rim(&self, i: usize, at: [f32; 2]) -> i32 {
        let rim = self.rim_spot(crate::target_hoop(self.players[i].team));
        timing::angle_units(crate::math::atan2(at[0] - rim[0], at[1] - rim[1]))
    }

    /// Callback 28 (`FUN_00172400`, raw `0x172400..0x1725f8`): whether
    /// dunker `i` hangs on the rim after his slam. Never once the game is
    /// won (the game's vt+0xbc), nor in live play when an inside basket by
    /// his side would win it (vt+0x7c, [`World::basket_would_win`]); then
    /// `r = rand() % 100`; never while a
    /// teammate hangs (state bit 14, `FUN_0016ea50`: his hands planted on
    /// the rim) or when the ball has touched the hoop (state 6) and is not
    /// his; then by the ball's last shooter (+0xf8, else its holder): his
    /// own, `r > 1` after a made roll (`0x3d6580`) and `r > 49` after a
    /// missed one; nobody's, `r > 89`; another's, no. Not modelled, the
    /// rewrite's reading: the Script Manager's replay states (3..7 always
    /// hang), the hoop object's state words (+0x80 set with +0x78 clear
    /// bars it).
    pub(crate) fn hangs(&mut self, i: usize) -> bool {
        if matches!(self.phase, crate::Phase::Over { .. }) {
            return false;
        }
        let team = self.players[i].team;
        // In live play (`FUN_00140558`), never when an inside basket by
        // his side at the ball's gamebreaker would win (the game's vt+0x7c
        // asked as `(side, 2, ball+0x178)`, `0x1724e0..0x172548`), before
        // the draw.
        if self.game_phase.live() && self.basket_would_win(team, 2, self.gamebreaker_ball()) {
            return false;
        }
        let r = self.rng.next_u32() % 100;
        let hanging = |p: &crate::Player| {
            matches!(
                p.action,
                Action::Dunking {
                    hang: Some(true),
                    ..
                }
            ) && p.flight.flags & NO_GRAVITY != 0
        };
        if self.players.iter().any(|p| p.team == team && hanging(p)) {
            return false;
        }
        let shooter = self
            .ball
            .shooter
            .map(usize::from)
            .or_else(|| self.handler());
        if self.native_ball_state() == crate::game_phase::ball::RIM && shooter != Some(i) {
            return false;
        }
        match shooter {
            None => r > 89,
            Some(s) if s == i => {
                if self.ball.rolled {
                    r > 1
                } else {
                    r > 49
                }
            }
            Some(_) => false,
        }
    }

    /// One update of `i`'s flight on his move's script, after his step (the
    /// release and the fadeaway's switch done): the steps his channel ran
    /// (their velocities and flags), the clip's root, the motion with
    /// gravity, his facing's turn. `at` where he stood as the update began.
    pub(crate) fn flight_tick(&mut self, i: usize, at: [f32; 2]) {
        let action = self.players[i].action;
        if !scripted(&action) {
            self.players[i].flight = Flight::default();
            return;
        }
        let mut f = self.players[i].flight;
        let Some(mut m) = self.move_steps(i, f.landing) else {
            self.players[i].flight = Flight::default();
            return;
        };
        if m.ran.hang_test {
            // The script's `84`/`85 28` this update: callback 28 answers
            // (its draw in the channel's step), and the update runs on the
            // path it picks.
            let hangs = self.hangs(i);
            if let Action::Dunking { hang, end, .. } = &mut self.players[i].action {
                *hang = Some(hangs);
                if hangs {
                    // The hang's path runs past the row's walked end: the
                    // move is over when its script is (below).
                    *end = u8::MAX;
                }
            }
            if let Some(again) = self.move_steps(i, f.landing) {
                m = again;
            }
        }
        if m.ran.land && f.landing < 0 {
            // The landing wait starts this update: `FUN_0019bed8` from the
            // rise this update's steps leave (`aa` before it, a dunk's).
            let mut vy = f.velocity[1];
            for step in m.ran.ops.iter() {
                match step {
                    Step::Stop => vy = 0.0,
                    Step::Rise(v) => vy = v,
                    Step::Carry(k) => vy = f.moved[1] * k,
                    _ => {}
                }
            }
            let tick = (m.updates - 1).clamp(0, i32::from(i16::MAX)) as i16;
            let land = tick.saturating_add(i16::from(timing::airtime(vy, self.players[i].height)));
            if land != m.land_at
                && let Some(again) = self.move_steps(i, land)
            {
                m = again;
            }
        }
        // His side's update, before his channel: a dunk's meet counts down
        // (`0x15a14c`), the wanted facing turns to the rim under the flags
        // the last update left.
        f.meet = if matches!(action, Action::Dunking { .. }) {
            f.meet.saturating_sub(1)
        } else {
            0
        };
        if f.flags & RIM_FACING != 0 && self.players[i].team == self.possession {
            let w = self.to_rim(i, at);
            self.players[i].wanted = crate::restart::heading_radians(w);
        }
        // Callback 5 (`FUN_00174eb0(actor, 0, 0)`) on a dive: his wanted
        // facing to the ball (`FUN_0016f2b0`).
        if m.ran.toward_ball && matches!(action, Action::Dive { .. }) {
            let p = &self.players[i];
            let b = self.ball.position;
            let word = timing::angle_units(crate::math::atan2(
                p.position[0] - b[0],
                p.position[1] - b[2],
            ));
            self.players[i].wanted = crate::restart::heading_radians(word);
        }
        let stepped = !(m.script == f.script && m.updates == f.updates);
        let fresh = m.script != f.script;
        // The knock window's count down, then this update's `a6 1`; the
        // jump's clock.
        f.knock = f.knock.saturating_sub(1);
        f.shove = f.shove.saturating_sub(1);
        f.aloft = f.aloft.saturating_add(1);
        if stepped && let Some((_, n)) = m.ran.knock {
            f.knock = n;
        }
        if stepped && let Some((_, n)) = m.ran.shove {
            f.shove = n;
        }
        f.script = m.script;
        f.updates = m.updates;
        let before_place = [
            self.players[i].position[0],
            self.players[i].height,
            self.players[i].position[1],
        ];
        if stepped {
            if m.ran.land && f.landing < 0 {
                f.landing = m.land_at;
            }
            // The span's step under the flags the update began with, the
            // steps, then the new span's first step under theirs.
            let before = if fresh { None } else { m.before };
            let mid = m.ran.mid.unwrap_or(m.after);
            self.flight_root(i, &mut f, before, mid, m.from);
            // The model as the update before left it, for a plant.
            let prev = m
                .ran
                .ops
                .iter()
                .any(|s| matches!(s, Step::Plant(_)))
                .then(|| self.previous_pose(i, &m))
                .flatten();
            let shown = m.ran.mid.unwrap_or(m.after);
            for step in m.ran.ops.iter() {
                self.flight_op(i, &mut f, step, shown, at, prev.as_ref());
                if matches!(
                    step,
                    Step::Rise(_)
                        | Step::Hop { .. }
                        | Step::Toward { .. }
                        | Step::Dive { .. }
                        | Step::Aim(_)
                        | Step::Reach(_)
                ) {
                    f.air = u16::from(timing::airtime(f.velocity[1], self.players[i].height));
                    f.aloft = 0;
                }
            }
            if m.ran.mid.is_some() {
                self.flight_root(i, &mut f, Some(mid), m.after, m.from);
            }
        }
        // The motion step (`0x1573d8..0x157530`).
        let g = gravity();
        if f.flags & UNPLANT != 0 && f.flags & (NO_GRAVITY | PLANTED) != 0 {
            f.flags &= !(UNPLANT | NO_GRAVITY | PLANTED);
        }
        let p = &mut self.players[i];
        p.position[0] += f.velocity[0];
        p.position[1] += f.velocity[2];
        p.height += f.velocity[1];
        f.point[0] += f.velocity[0];
        f.point[2] += f.velocity[2];
        if f.flags & NO_GRAVITY == 0 {
            p.height += g * 0.5;
            f.velocity[1] += g;
        }
        if p.height <= 0.0 && f.velocity[1] <= 0.0 {
            f.velocity[1] = 0.0;
            p.height = 0.0;
        }
        // The slide (actor+0x184, `0x157530..0x1575d0`): the speed across
        // less the deceleration each update, which ends once he stops.
        if f.slide != 0.0 {
            let speed = (f.velocity[0] * f.velocity[0] + f.velocity[2] * f.velocity[2]).sqrt();
            // Stopped (under 0.0001) or slower than one update's slide.
            if speed < f32::from_bits(0x38d1_b717) || speed < f.slide {
                f.slide = 0.0;
                f.velocity[0] = 0.0;
                f.velocity[2] = 0.0;
            } else {
                f.velocity[0] -= f.slide * f.velocity[0] / speed;
                f.velocity[2] -= f.slide * f.velocity[2] / speed;
            }
        }
        f.moved = [
            p.position[0] - before_place[0],
            p.height - before_place[1],
            p.position[1] - before_place[2],
        ];
        p.rise = f.velocity[1];
        // `FUN_00168ea8`: the facing toward the wanted one.
        let rate = turn_rate(&action, f.flags);
        let now = timing::facing_word(p.heading);
        let want = timing::facing_word(p.wanted);
        let word = crate::guard::turn_toward(now, want, rate);
        if word != now {
            p.heading = crate::restart::heading_radians(word);
        }
        p.flight = f;
        // The model posed, the planted points held (`0x157800..0x157954`).
        if f.flags & (PLANTED | NO_GRAVITY) != 0 {
            self.hold_planted(i, &m);
        }
        // A hang's move, and a dive, are over with their script once he is
        // down; a pass on the floor once its `91` gives the locomotion back
        // with the ball gone (the walk takes up the velocity he coasts on).
        let thrown = self.windup.is_none_or(|w| w.from != i);
        let p = &mut self.players[i];
        let walks = matches!(p.action, Action::Passing { .. }) && thrown && walks(f.flags);
        if (m.done || walks)
            && p.height <= 0.0
            && matches!(
                p.action,
                Action::Dunking {
                    hang: Some(true),
                    ..
                } | Action::Dive { .. }
                    | Action::Passing { .. }
            )
        {
            if matches!(p.action, Action::Passing { .. }) {
                p.speed = crate::math::len(f.velocity[0], f.velocity[2]) * crate::TICK_HZ as f32;
            }
            p.action = Action::None;
            p.height = 0.0;
        }
    }

    /// The model as `i`'s channel showed it the update before, at his place
    /// before he moves: the script's last frame, or at its first update the
    /// clip the channel played before it.
    fn previous_pose(&self, i: usize, m: &crate::held::MoveSteps) -> Option<crate::pose::Pose> {
        let poses = self.poses.get()?;
        let p = &self.players[i];
        let place = self.place(i);
        match m.before {
            Some(shown) => crate::trick::pose(poses, shown, m.from, p.body(), place),
            None => {
                let (record, time) = m.from?;
                poses.pose(
                    crate::pose::Frame {
                        record,
                        time,
                        looping: true,
                    },
                    p.body(),
                    place,
                )
            }
        }
    }

    /// The planted points held (`FUN_00156f48`, `0x157800..0x157954`, after
    /// the model is posed): his x and z moved by the planted point less
    /// the planted bones' midpoint as the channel shows them at his place,
    /// his height too for the hands (0x40000).
    fn hold_planted(&mut self, i: usize, m: &crate::held::MoveSteps) {
        let Some(poses) = self.poses.arc() else {
            return;
        };
        let p = &self.players[i];
        let f = p.flight;
        let Some(pose) = crate::trick::pose(&poses, m.after, m.from, p.body(), self.place(i))
        else {
            return;
        };
        let [a, b] = f
            .bones
            .map(|k| pose.joints[usize::from(k).min(crate::pose::BONES - 1)].position);
        let mid: [f32; 3] = std::array::from_fn(|k| (a[k] + b[k]) * 0.5);
        let p = &mut self.players[i];
        if f.flags & NO_GRAVITY != 0 {
            p.height += f.point[1] - mid[1];
        }
        if f.flags & PLANTED != 0 {
            p.position[0] += f.point[0] - mid[0];
            p.position[1] += f.point[2] - mid[2];
        }
    }

    /// `93`/`95`'s point (raw `0x194350..0x194704`): the bones `plant`
    /// picks, as the model was posed the update before (`pose`), their
    /// midpoint (a pick's one bone itself); the feet keep x and z.
    fn plant(
        &self,
        i: usize,
        f: &mut Flight,
        plant: Plant,
        pose: &crate::pose::Pose,
        record: Option<u16>,
    ) {
        let held = f.flags & (PLANTED | NO_GRAVITY) != 0 && f.flags & UNPLANT == 0;
        let (bones, point) = self.plant_point(i, plant, pose, record, held.then_some(f.bones));
        f.bones = bones;
        f.point = if plant.hands {
            point
        } else {
            [point[0], f.point[1], point[2]]
        };
    }

    /// `plant`'s bones and their midpoint on `pose` (the model as the
    /// update before left it); `held` the pair already planted, which
    /// `keep` (`0xf1`) keeps when they are the same bones.
    pub(crate) fn plant_point(
        &self,
        i: usize,
        plant: Plant,
        pose: &crate::pose::Pose,
        record: Option<u16>,
        held: Option<[u8; 2]>,
    ) -> ([u8; 2], [f32; 3]) {
        let p = &self.players[i];
        let [a, b] = plant.bones.map(usize::from);
        let (mut s3, mut s2) = (a, b);
        let same = held.is_some_and(|h| {
            let [u, v] = h.map(usize::from);
            (a == u || a == v) && (b == u || b == v)
        });
        if plant.keep && same {
            let [u, v] = held.unwrap_or_default().map(usize::from);
            (s3, s2) = (u, v);
        } else {
            let bone = |k: usize| pose.joints[k.min(crate::pose::BONES - 1)].position;
            let second = match plant.pick {
                // `FUN_001404e0`'s side and his z.
                0xec => {
                    let side = crate::restart::side(p.team);
                    let z = p.position[1];
                    Some((side < 0.0 && 0.0 < z) || (side > 0.0 && z < 0.0))
                }
                0xf2 => Some(p.left_handed),
                0xf3 | 0xf4 => {
                    // Along the pelvis's facing (`FUN_00154230`).
                    let w = crate::trick::pelvis_word(pose);
                    let ahead = |k: usize| {
                        let q = bone(k);
                        -q[0] * crate::input::native_sin_1024(w)
                            - q[2] * crate::input::native_cos_1024(w)
                    };
                    let (za, zb) = (ahead(a), ahead(b));
                    Some(if plant.pick == 0xf3 {
                        zb < za
                    } else {
                        za <= zb
                    })
                }
                0xf7 => {
                    let state = self
                        .poses
                        .get()
                        .zip(record)
                        .map_or(0, |(poses, r)| poses.state(r, 0));
                    if state & 0x2400 != 0x2400 {
                        if state & 0x40 != 0 {
                            s3 = s2;
                        }
                        if state & 0x200 != 0 {
                            s2 = s3;
                        }
                    }
                    None
                }
                0xf8 => Some(p.gait_channel.hand == 2),
                _ => None,
            };
            match second {
                Some(true) => s3 = s2,
                Some(false) => s2 = s3,
                None => {}
            }
        }
        let (u, v) = (
            pose.joints[s3.min(crate::pose::BONES - 1)].position,
            pose.joints[s2.min(crate::pose::BONES - 1)].position,
        );
        let point: [f32; 3] = std::array::from_fn(|k| (u[k] + v[k]) * 0.5);
        ([s3 as u8, s2 as u8], point)
    }

    fn flight_op(
        &mut self,
        i: usize,
        f: &mut Flight,
        step: Step,
        shown: crate::trick::Shown,
        at: [f32; 2],
        prev: Option<&crate::pose::Pose>,
    ) {
        let record = shown.record;
        match step {
            Step::Flags(w) => f.flags = (f.flags & 0xffff_0000) | w,
            Step::Stop => f.velocity = [0.0; 3],
            Step::Rise(v) => f.velocity[1] = v,
            Step::Hop { turn, speed, rise } => {
                let a = timing::facing_word(self.players[i].wanted) + i32::from(turn);
                f.velocity = [
                    -speed * crate::input::native_sin_1024(a),
                    rise,
                    -speed * crate::input::native_cos_1024(a),
                ];
            }
            Step::Toward { turn, speed, rise } => {
                let a = self.to_rim(i, at) + i32::from(turn);
                f.velocity = [
                    -speed * crate::input::native_sin_1024(a),
                    rise,
                    -speed * crate::input::native_cos_1024(a),
                ];
            }
            Step::Want(turn) => {
                let w = (self.to_rim(i, at) + i32::from(turn)) & 0x3ff;
                self.players[i].wanted = crate::restart::heading_radians(w);
            }
            Step::Call(18) => {
                let side = crate::restart::side(self.players[i].team);
                let behind = behind_board(self.players[i].position, side);
                f.velocity = side_fade(
                    record.unwrap_or(0),
                    self.to_rim(i, at),
                    behind,
                    self.attack_x(i),
                );
            }
            Step::Carry(k) => f.velocity = f.moved.map(|c| c * k),
            Step::Slide(k) => {
                // `FUN_001957d8`: a negative value is the default 0.5.
                f.slide = if k < 0.0 { 0.5 } else { k };
            }
            Step::Dive { speed, rise } => {
                let p = &self.players[i];
                let at = [p.position[0], p.height, p.position[1]];
                let root = self.root_height(shown.record, shown.frame).unwrap_or(0.0);
                let (v, word) = crate::dive::dive_hop(
                    speed,
                    rise,
                    at,
                    root,
                    self.ball.position,
                    self.ball.velocity.map(|c| c * crate::TICK_SECONDS),
                    self.ball.radius,
                    p.body().height,
                    record == Some(71),
                );
                f.velocity = v;
                self.players[i].wanted = crate::restart::heading_radians(word);
            }
            Step::Plant(plant) => {
                if let Some(pose) = prev {
                    self.plant(i, f, plant, pose, record);
                }
                f.flags &= !UNPLANT;
                if plant.hands {
                    f.flags |= NO_GRAVITY | PLANTED;
                } else {
                    f.flags = (f.flags & !NO_GRAVITY) | PLANTED;
                }
            }
            Step::Unplant => f.flags |= UNPLANT,
            Step::Aim(a) => {
                f.flags |= ROOT_OFF;
                // `0x199858`: a dunker's or leaper's meet (bits 7, 15).
                f.meet = a.updates;
                let p = &self.players[i];
                let near = self.attack_x(i).abs() * if p.position[0] < 0.0 { -1.0 } else { 1.0 };
                let bits = crate::block::holder_bits(&p.action);
                let want = timing::facing_word(p.wanted);
                let record = record.unwrap_or(0);
                let hands = PosedHands {
                    w: self,
                    i,
                    root: self.root_height(shown.record, shown.frame),
                };
                let (v, word) = aim(
                    &hands,
                    a,
                    record,
                    want,
                    near,
                    self.attack_x(i),
                    bits,
                    self.ball.radius,
                );
                f.velocity = v;
                self.players[i].wanted = crate::restart::heading_radians(word);
                // `FUN_00159190(actor, r & 1 ? 2 : 1, n)`: the face.
                self.rng.next_u32();
                self.face(i, i32::from(a.updates));
            }
            Step::Reach(r) => {
                let p = &self.players[i];
                let rim = self.rim_spot(crate::target_hoop(p.team));
                let distance = crate::distance(at, rim);
                let airtime = timing::airtime(f.velocity[1], p.height);
                let end_x = if p.position[0] >= 0.0 { 38.0 } else { -38.0 };
                let ball = self.reach_ball(
                    i,
                    record.unwrap_or(0),
                    r.frame,
                    self.root_height(record, shown.frame),
                );
                f.velocity = reach(
                    r,
                    f.velocity,
                    ball,
                    distance,
                    airtime,
                    end_x,
                    p.body().height,
                    self.ball.radius,
                );
            }
            _ => {}
        }
    }

    /// The clip's root height at `frame` of `record` (court units at scale
    /// 1): the model's root as the channel's step leaves it (actor+0xe4).
    fn root_height(&self, record: Option<u16>, frame: f32) -> Option<f32> {
        let poses = self.poses.get()?;
        let root = poses.root(crate::pose::Frame {
            record: record?,
            time: frame,
            looping: false,
        })?;
        Some(root[1])
    }

    /// The ball at `i`'s hand posed on `record` at `frame` facing his
    /// wanted facing (`FUN_0014da20` flag 0 by the frame's hands, both
    /// hands' midpoint on 0xc), the model's root at the height the channel
    /// now gives it (`root`, [`aim_pose`]); without the disc's animations
    /// his reach above his feet.
    fn reach_ball(&self, i: usize, record: u16, frame: u8, root: Option<f32>) -> [f32; 3] {
        let p = &self.players[i];
        let stand_in = [p.position[0], p.height + p.ratings.reach, p.position[1]];
        let Some(poses) = self.poses.get() else {
            return stand_in;
        };
        let body = p.body();
        let mut place = self.place(i);
        place.word = timing::facing_word(p.wanted);
        let Some(pose) = aim_pose(poses, record, frame, body, place, root) else {
            return stand_in;
        };
        let state = poses.state(record, i32::from(frame));
        crate::dribble::in_hand(
            &pose,
            record,
            i32::from(frame),
            state,
            body,
            self.ball.radius,
        )
    }

    /// The clip's root on `i` (`FUN_0019a658`, raw `0x19b3c0..0x19b70c`):
    /// its change from `before` to `after` (none in a blend or across a
    /// change of clip, [`crate::trick::root_step`]), times his height
    /// scale: on x and z turned by his facing unless [`ROOT_OFF`]; his
    /// height lowered by its rise while [`RESYNC`], which a step to a new
    /// whole frame outside a blend clears; on the floor falling he stands.
    /// Without the disc's animations, none.
    fn flight_root(
        &mut self,
        i: usize,
        f: &mut Flight,
        before: Option<crate::trick::Shown>,
        after: crate::trick::Shown,
        from: Option<(u16, f32)>,
    ) {
        let Some(poses) = self.poses.arc() else {
            return;
        };
        let p = &mut self.players[i];
        let body = p.body();
        if let Some(before) = before
            && f.flags & ROOT_OFF == 0
        {
            let step = crate::trick::root_step(&poses, before, after);
            let m = crate::trick::root_motion(step, body, timing::facing_word(p.heading));
            p.position[0] += m[0];
            p.position[1] += m[1];
        }
        // A script's first update may show no clip yet (its pick is
        // still to come): the channel holds the clip it had, so there is
        // no step to take and the resync stays for the first one.
        if f.flags & RESYNC != 0 && after.record.is_some() {
            // The model's root height (actor+0xe4) as the channel showed it
            // before (the clip held through the blend at the script's
            // first update) and now, a blend's mixed.
            let now = root_y(&poses, Some(after), from);
            let then = match before {
                Some(b) if b.record.is_some() => root_y(&poses, Some(b), from),
                _ => root_y(&poses, None, from),
            };
            if let (Some(now), Some(then)) = (now, then) {
                p.height -= (now - then) * body.height;
            }
            if p.height < 0.0 && f.velocity[1] <= 0.0 {
                p.height = 0.0;
            }
            let frame = before.map_or(-1, |b| b.frame as i32);
            if after.blend.is_none() && (after.frame as i32) != frame {
                f.flags &= !RESYNC;
            }
        }
    }
}

/// The model's root height a channel shows (at scale 1): the clip's root
/// at its frame, in a blend mixed from the clip held (`from`) by its
/// weight as the blend poses it ([`crate::pose::Poses::blend`]); `None`
/// shown the held clip's own.
fn root_y(
    poses: &crate::pose::Poses,
    shown: Option<crate::trick::Shown>,
    from: Option<(u16, f32)>,
) -> Option<f32> {
    let held = || {
        let (record, time) = from?;
        poses
            .root(crate::pose::Frame {
                record,
                time,
                looping: true,
            })
            .map(|r| r[1])
    };
    let Some(shown) = shown else {
        return held();
    };
    let to = poses.root(crate::pose::Frame {
        record: shown.record?,
        time: shown.frame,
        looping: false,
    })?[1];
    match shown.blend {
        Some(w) => match held() {
            Some(a) => Some(a + (to - a) * w.clamp(0.0, 1.0)),
            None => Some(to),
        },
        None => Some(to),
    }
}

/// `FUN_00198740`'s pose: `record`'s rotations at `frame` on the model
/// at `place`, its root at the height the channel's step left it (`root`,
/// actor+0xe4: `FUN_0019bde0` writes the clip's own root translation to
/// locals the aim never reads), or the clip's at that frame without one.
pub fn aim_pose(
    poses: &crate::pose::Poses,
    record: u16,
    frame: u8,
    body: crate::pose::Body,
    place: crate::pose::Place,
    root: Option<f32>,
) -> Option<crate::pose::Pose> {
    let at = crate::pose::Frame {
        record,
        time: f32::from(frame),
        looping: false,
    };
    let mut pose = poses.pose(at, body, place)?;
    if let (Some(now), Some(then)) = (root, poses.root(at)) {
        let dy = (now - then[1]) * body.height;
        for j in &mut pose.joints {
            j.position[1] += dy;
        }
    }
    Some(pose)
}

/// The posed bones a `9e` aim reads: the disc's clips at `i`'s place, the
/// root at the height the channel gives it now, or without them his reach
/// above his feet.
struct PosedHands<'a> {
    w: &'a World,
    i: usize,
    root: Option<f32>,
}

impl Hands for PosedHands<'_> {
    fn bones(&self, record: u16, frame: u8, word: i32) -> ([[f32; 3]; 4], u32) {
        let p = &self.w.players[self.i];
        let stand_in = [p.position[0], p.height + p.ratings.reach, p.position[1]];
        let Some(poses) = self.w.poses.get() else {
            return ([stand_in; 4], 8);
        };
        let mut place = self.w.place(self.i);
        place.word = word & 0x3ff;
        let Some(pose) = aim_pose(poses, record, frame, p.body(), place, self.root) else {
            return ([stand_in; 4], 8);
        };
        let j = |b: usize| pose.joints[b].position;
        (
            [j(19), j(15), j(18), j(14)],
            poses.state(record, i32::from(frame)),
        )
    }
}
