//! Blocks as the original makes them: the block jump (actor state bit 12,
//! `FUN_0015d3a0`), the aim its script's game callback 35 gives it
//! (`FUN_00176080`) and the block test the actor's update runs every
//! update of it (`FUN_0016d888`). Traced raw 2026-10-16 (docs/TRACE.md
//! "Blocks and the dribble on the original's bones"; the decomp cuts all
//! three at `FUN_0030fa50`, which returns the frame-rate block and is no
//! assertion).
//!
//! A jump pressed with the ball held, passed, tossed or shot and not yet on
//! the hoop is a block jump. It picks one of thirteen scripts by where the
//! ball is and will be (the rebound jump's goaltend pair, whose `9b` sets
//! bit 12 as these do, plays the same way); each faces the jumper toward
//! the ball (callback 5:
//! actor+0xb0 the angle to it, the facing turning 46 an update), blends
//! four updates to its clip (callback 48 picks the arm: the mirrored record
//! when the ball 15 updates on lies to one side of that line), waits, and
//! at its callback 35 sends him up so that his posed hand meets the ball.
//! Every update of the jump his posed hands are tried against the ball
//! (and his forearms by the limb sweep): a hand in reach knocks it loose
//! at 0.35 an update along his facing and 0.125 up (the turbo block's clip,
//! 462/463 and the glass pair, whose `9b` also sets bit 19, catch it), a
//! limb deflects it.
//!
//! The held ball lies on whatever clip the holder plays (the gait's
//! dribble, a shot, dunk, lay-up, trick or pass: [`crate::held`]), and the
//! holder's head (callback 35's bone 21) is that clip's posed head
//! ([`World::actor_pose`]); only without the disc's animations is the ball
//! the carried point ([`crate::ball::Ball::carry`]) and the head at his
//! height. The jump flies the callback's velocity, and its first one or two
//! airborne updates rise by the clip's root drop (channel flag 0x10000, set
//! just before every script's aim, [`World::block_resync`]).
//!
//! The rewrite's own, labelled where it is used: the clip's root moving him
//! across (the scripts' first `91` leaves 0x1000 clear) is not modelled. The
//! glass pair's aimed jump (`a9`, `FUN_00199e20`) is read only in outline;
//! they aim as callback 35 does.

use crate::pose::{self, Body, Frame, Place, Pose, Poses};
use crate::rules::Difficulty;
use crate::{Action, BallState, Event, World, contact, game_phase, target_hoop, timing};

/// How long a script's wait lasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    /// `8d ee F C`: C updates.
    Fixed(u8),
    /// `8d ee F ef o`: the updates callback 35 aimed for, plus `o`.
    Meet(i8),
    /// `8d ee F fe`: until he lands (the airtime left).
    Landing,
}

/// One wait: the clip frame it reaches and how long it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wait {
    pub frame: u8,
    pub span: Span,
}

const fn wait(frame: u8, span: Span) -> Wait {
    Wait { frame, span }
}

/// One block jump script, read from its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Script {
    pub address: u32,
    /// The blend's record on callback 48's plain path, then on its branch
    /// (the glass pair: the `ec` selector's primary, then its alternate).
    pub records: [u16; 2],
    /// The blend's updates on each path.
    pub blend: [u8; 2],
    /// The clip frame the blend starts the record at.
    pub start: u8,
    /// The waits before the aim.
    pub before: &'static [Wait],
    /// The clip frame the aim poses the hand at.
    pub frame: u8,
    /// The waits after it, the last until he lands.
    pub after: &'static [Wait],
    /// The glass pair (callback 29 and `a9` in place of callbacks 5, 48
    /// and 35; `9b` sets bit 19 with bit 12, so their hands catch).
    pub glass: bool,
}

use Span::{Fixed, Landing, Meet};

/// The block jump's scripts (`block_*` in the animation records): the
/// thirteen of `FUN_0015d3a0`'s lists, then the rebound jump's goaltend
/// pair (`FUN_0015ce50`'s kind 6), whose `9b` sets bit 12 as these do.
pub const SCRIPTS: [Script; 15] = [
    Script {
        address: 0x441a7d,
        records: [440, 441],
        blend: [4, 4],
        start: 0,
        before: &[wait(2, Fixed(3))],
        frame: 13,
        after: &[wait(13, Meet(0)), wait(19, Landing)],
        glass: false,
    },
    Script {
        address: 0x441b29,
        records: [442, 443],
        blend: [4, 4],
        start: 0,
        before: &[wait(1, Fixed(2))],
        frame: 13,
        after: &[wait(13, Meet(0)), wait(19, Landing)],
        glass: false,
    },
    // The turbo block (and kind 11, under the basket): no callback 48.
    Script {
        address: 0x441bd5,
        records: [437, 437],
        blend: [4, 4],
        start: 0,
        before: &[wait(4, Fixed(6))],
        frame: 10,
        after: &[wait(10, Meet(0)), wait(22, Landing)],
        glass: false,
    },
    Script {
        address: 0x441c58,
        records: [438, 439],
        blend: [4, 4],
        start: 0,
        before: &[wait(2, Fixed(3))],
        frame: 9,
        after: &[wait(9, Meet(0)), wait(23, Landing)],
        glass: false,
    },
    Script {
        address: 0x441d04,
        records: [444, 445],
        blend: [4, 4],
        start: 0,
        before: &[wait(2, Fixed(3))],
        frame: 10,
        after: &[wait(10, Meet(0)), wait(24, Landing)],
        glass: false,
    },
    Script {
        address: 0x441db0,
        records: [446, 447],
        blend: [4, 4],
        start: 0,
        before: &[wait(4, Fixed(4))],
        frame: 26,
        after: &[wait(16, Meet(-10)), wait(32, Fixed(16)), wait(41, Landing)],
        glass: false,
    },
    Script {
        address: 0x441e60,
        records: [457, 456],
        blend: [4, 4],
        start: 0,
        before: &[wait(4, Fixed(5))],
        frame: 15,
        after: &[wait(15, Meet(0)), wait(20, Landing)],
        glass: false,
    },
    Script {
        address: 0x44206c,
        records: [453, 452],
        blend: [4, 4],
        start: 0,
        before: &[wait(5, Fixed(4))],
        frame: 12,
        after: &[wait(12, Meet(0)), wait(14, Fixed(8)), wait(21, Landing)],
        glass: false,
    },
    Script {
        address: 0x44211c,
        records: [455, 454],
        blend: [4, 4],
        start: 0,
        before: &[wait(4, Fixed(4))],
        frame: 11,
        after: &[wait(9, Meet(-4)), wait(13, Fixed(8)), wait(23, Landing)],
        glass: false,
    },
    // Its clips (462, 463) catch the ball they reach.
    Script {
        address: 0x4421cc,
        records: [463, 462],
        blend: [4, 4],
        start: 0,
        before: &[wait(5, Fixed(5))],
        frame: 13,
        after: &[wait(13, Meet(0)), wait(23, Landing)],
        glass: false,
    },
    Script {
        address: 0x442273,
        records: [460, 461],
        blend: [4, 4],
        start: 1,
        before: &[wait(5, Fixed(5))],
        frame: 15,
        after: &[wait(15, Fixed(20)), wait(25, Landing)],
        glass: true,
    },
    Script {
        address: 0x4422fa,
        records: [465, 464],
        blend: [4, 4],
        start: 1,
        before: &[wait(5, Fixed(5))],
        frame: 11,
        after: &[wait(11, Fixed(20)), wait(15, Fixed(10)), wait(23, Landing)],
        glass: true,
    },
    Script {
        address: 0x44239b,
        records: [459, 458],
        blend: [4, 6],
        start: 0,
        before: &[wait(4, Fixed(4))],
        frame: 14,
        after: &[wait(12, Meet(-3)), wait(18, Fixed(9)), wait(24, Landing)],
        glass: false,
    },
    Script {
        address: 0x441f0c,
        records: [449, 448],
        blend: [4, 4],
        start: 0,
        before: &[wait(5, Fixed(6))],
        frame: 13,
        after: &[wait(11, Meet(-4)), wait(15, Fixed(8)), wait(24, Landing)],
        glass: false,
    },
    Script {
        address: 0x441fbc,
        records: [451, 450],
        blend: [4, 4],
        start: 2,
        before: &[wait(5, Fixed(4))],
        frame: 14,
        after: &[wait(12, Meet(-4)), wait(16, Fixed(8)), wait(24, Landing)],
        glass: false,
    },
];

/// The goaltend pair in [`SCRIPTS`], by the rebound jump's script index
/// ([`crate::rebound::SCRIPTS`] 14 and 15).
pub const GOALTEND: [u8; 2] = [13, 14];

/// The scripts by kind (the lists at `0x3d7160..0x3d71ec` through
/// `0x3d71f0`, counted at `0x43f840`), as indexes into [`SCRIPTS`]. With a
/// holder: 0 one not shooting; 1 a dunker's from behind within 5; 2 and 3
/// a shooter's from between him and the rim, far from the basket or near
/// it; 4 and 5 the same from elsewhere. At a ball in the air: 6 the glass
/// (Blocks 80 or more, the ball 28 updates on at the board), 7 a shot in
/// front of him near the rim, 8 a shot he is between and the rim, far; 9
/// and 10 any other, far or near ([`Scene::far`]); 11 under the basket.
pub const KINDS: [&[u8]; 12] = [
    &[5],
    &[3, 4, 6],
    &[0, 1, 5],
    &[5],
    &[0, 1, 5],
    &[5],
    &[10, 11],
    &[7, 12, 9],
    &[0, 1, 5, 8],
    &[0, 1, 5],
    &[5],
    &[2],
];

/// The turbo block (two turbos held, 0.5 of the bar: `0x15d3d4..`).
pub const TURBO: u8 = 2;
/// The turbo block's cost and the bar it needs.
pub const TURBO_COST: f32 = 0.5;
/// The facing's turn toward callback 5's word, units an update (measured:
/// 95 of the attract captures' block-jump turns are 46, the rest the
/// remainders; the turning code is not traced).
pub const TURN: i32 = 46;
/// The hands' reach (`DAT_003d9058`), before the Easy adjustments.
pub const REACH: f32 = 1.0;
/// The knock: 0.35 an update along the facing, 0.125 up.
pub const KNOCK: [f32; 2] = [0.35, 0.125];
/// The block test's ring: a ball on the rim within this of its middle
/// across is not touched (`0x3f4b10cb`).
pub const RING: f32 = f32::from_bits(0x3f4b_10cb);

/// A block jump under way (the original's channel and actor fields the
/// script and callback 35 keep).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jump {
    /// Into [`SCRIPTS`].
    pub script: u8,
    /// Callback 48's branch (the record and the blend).
    pub branch: bool,
    /// The clip the jumper played at the press (record and frame), held
    /// through the blend.
    pub from: Option<(u16, f32)>,
    /// Updates since the press (the press update is 0).
    pub tick: u16,
    /// Callback 5's facing word (actor+0xb0).
    pub word: i32,
    /// Callback 35's velocity, units an update: the rise, and across.
    pub vy: f32,
    pub drift: [f32; 2],
    /// In the air (callback 35 has run).
    pub up: bool,
    /// The updates callback 35 aimed for (channel+0x20, at least 15).
    pub meet: u8,
    /// The jump's airtime (actor+0x64) and the updates since the aim
    /// (+0x60).
    pub airtime: u16,
    pub flown: u16,
    /// The turbo block (actor+0x360): its clip catches the ball.
    pub turbo: bool,
}

/// What channel 0 shows on a tick: its record and fractional frame, or the
/// held clip through the blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub record: Option<u16>,
    pub frame: f32,
}

impl Jump {
    fn script(&self) -> &'static Script {
        &SCRIPTS[usize::from(self.script)]
    }

    /// The record the blend plays.
    pub fn record(&self) -> u16 {
        self.script().records[usize::from(self.branch)]
    }

    fn blend_len(&self) -> u16 {
        u16::from(self.script().blend[usize::from(self.branch)])
    }

    /// The tick callback 35 runs at: the blend and the waits before it.
    pub fn aim_tick(&self) -> u16 {
        self.blend_len()
            + self
                .script()
                .before
                .iter()
                .map(|w| match w.span {
                    Fixed(n) => u16::from(n),
                    _ => 0,
                })
                .sum::<u16>()
    }

    /// The incoming clip's weight through the blend, then None.
    pub fn blend(&self) -> Option<f32> {
        let n = self.blend_len();
        (self.tick < n).then(|| f32::from(self.tick + 1) / f32::from(n))
    }

    /// A wait's updates from tick `start` (the aim's waits once it has
    /// run; until then a meet or landing wait has not begun).
    fn span(&self, w: Wait, start: u16) -> u16 {
        match w.span {
            Fixed(n) => u16::from(n),
            Meet(o) => (i32::from(self.meet) + i32::from(o)).max(1) as u16,
            Landing => {
                let since = start.saturating_sub(self.aim_tick());
                self.airtime.saturating_sub(since).max(1)
            }
        }
    }

    /// The clip's frame on this tick: its start through the blend, then
    /// each wait's even steps to its frame (`8d`).
    pub fn frame(&self) -> f32 {
        let s = self.script();
        let mut at = self.blend_len();
        if self.tick < at {
            return f32::from(s.start);
        }
        let mut frame = f32::from(s.start);
        let aim = self.aim_tick();
        for w in s.before.iter().chain(s.after.iter()) {
            if at >= aim && !self.up {
                return frame;
            }
            let n = self.span(*w, at);
            let to = f32::from(w.frame);
            if self.tick < at + n {
                let k = f32::from(self.tick - at + 1);
                return frame + (to - frame) * k / f32::from(n);
            }
            frame = to;
            at += n;
        }
        frame
    }

    /// Channel 0 on this tick.
    pub fn shown(&self) -> Shown {
        match (self.blend(), self.from) {
            (Some(_), Some((record, time))) => Shown {
                record: Some(record),
                frame: time,
            },
            (Some(_), None) => Shown {
                record: None,
                frame: 0.0,
            },
            (None, _) => Shown {
                record: Some(self.record()),
                frame: self.frame(),
            },
        }
    }

    /// The jumper's pose on this tick.
    pub fn pose(&self, poses: &Poses, body: Body, place: Place) -> Option<Pose> {
        let to = Frame {
            record: self.record(),
            time: self.frame(),
            looping: false,
        };
        match (self.blend(), self.from) {
            (Some(w), Some((record, time))) => {
                let from = Frame {
                    record,
                    time,
                    looping: false,
                };
                poses.blend(from, to, w, body, place)
            }
            _ => poses.pose(to, body, place),
        }
    }

    /// The clip's frame on tick `t`, in the air from the aim on.
    fn frame_on(&self, t: u16) -> f32 {
        Jump {
            tick: t,
            up: self.up && t >= self.aim_tick(),
            ..*self
        }
        .frame()
    }

    /// Channel flag 0x10000 still set on this tick's step: from the aim
    /// (which sets it) until a step has reached a new whole frame (which
    /// clears it after its own correction, [`World::block_resync`]).
    pub fn resyncing(&self) -> bool {
        let aim = self.aim_tick();
        if !self.up || self.tick < aim || aim == 0 {
            return false;
        }
        (aim..self.tick).all(|t| self.frame_on(t) as i32 == self.frame_on(t - 1) as i32)
    }

    /// The script is over: its last wait has run out on the landing.
    pub fn done(&self) -> bool {
        if !self.up {
            return false;
        }
        let s = self.script();
        let mut at = self.blend_len();
        for w in s.before.iter().chain(s.after.iter()) {
            at += self.span(*w, at);
        }
        self.tick + 1 >= at
    }
}

/// Bits of the holder's state word (actor+0xa4) as the rewrite's actions
/// show them, read from the captures' launches (ball state 3 to 5): a jump
/// shot sets bit 3 (0x8), a lay-up bits 3 and 13 (0x2008; a dunk that turns
/// into one too), a dunk bit 7 (0x80), an alley-oop's leap bit 15 (0x8000).
/// The attract captures' 30 jump shots from 15 or more out carry bit 3
/// alone and their 29 dunks bit 7 (`ai::Ctx::shooting` and friends).
pub(crate) fn holder_bits(action: &Action) -> u32 {
    match action {
        Action::Shooting { .. } => 0x8,
        Action::Dunking { layup: true, .. } | Action::Layup { .. } => 0x2008,
        a if crate::leap::leaping(a) => 0x8000,
        Action::Dunking { .. } => 0x80,
        _ => 0,
    }
}

/// The signed difference `a - b` wrapped to -512..=512 (`FUN_0018cd78`).
fn turn(a: i32, b: i32) -> i32 {
    let d = a - b;
    if d.abs() > 0x200 {
        if d < 0 { d + 0x400 } else { d - 0x400 }
    } else {
        d
    }
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn len3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// What `FUN_0015d3a0` reads for its choice: the jumper, the holder or
/// the ball, and the court (in the original's frame).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scene {
    /// The jumper's place on the floor, his side byte (`FUN_00156db0`:
    /// +1 attacks +x) and the other side's, his Blocks (0..99).
    pub at: [f32; 2],
    pub side: i32,
    pub other: i32,
    pub blocks: i32,
    /// The holder: his place on the floor, his state bits (+0xa4) and the
    /// eighth about his facing the jumper stands in (`FUN_0018d9f0`).
    pub holder: Option<([f32; 2], u32, i32)>,
    /// The ball now (ball+0x30), 28 updates on (`FUN_001537d0`), and
    /// whether it is a shot (state 5).
    pub ball: [f32; 3],
    pub ahead: [f32; 3],
    pub shot: bool,
    pub radius: f32,
    /// The rim the side with the ball attacks (game+0x110).
    pub rim: [f32; 2],
}

impl Scene {
    /// `FUN_0015d920`: the block far from the basket. The ball within 32
    /// across of the middle, then the jumper 15 or more from the offence's
    /// rim; nearer, within 36 of the middle and the ball on his own end
    /// (`FUN_0016f2b0`, the angle to it, against 0x200 by his side).
    pub fn far(&self) -> bool {
        if self.ball[0].abs() > 32.0 {
            return false;
        }
        if crate::distance(self.at, self.rim) >= 15.0 {
            return true;
        }
        if self.at[0].abs() > 36.0 {
            return false;
        }
        let to_ball = contact::angle(self.at, [self.ball[0], self.ball[2]]);
        if self.side < 1 {
            to_ball >= 0x200
        } else {
            to_ball <= 0x200
        }
    }

    /// `FUN_0016fef8(actor, 0x100)`: the jumper between the ball (or its
    /// holder) and the offence's rim, within a quarter turn of its line
    /// from the rim and nearer to it.
    pub fn between(&self) -> bool {
        let b = match self.holder {
            Some((at, ..)) => at,
            None => [self.ball[0], self.ball[2]],
        };
        let d = turn(
            contact::angle(b, self.rim),
            contact::angle(self.at, self.rim),
        )
        .abs();
        d <= 0x100 && crate::distance(self.at, self.rim) <= crate::distance(b, self.rim)
    }
}

/// `FUN_0015d3a0`'s kind (raw `0x15d474..0x15d888`): under the basket 11;
/// with a holder 0 (not in a shot-like move), 1..5; at a ball in the air
/// 6..10 ([`KINDS`]).
pub fn kind(s: &Scene) -> usize {
    if crate::rebound::under_basket(s.at) {
        return 11;
    }
    let far = s.far();
    if let Some((at, bits, octant)) = s.holder {
        if bits & 0xa088 == 0 {
            return 0;
        }
        if bits & 0x2008 != 0 {
            if octant == 0 && crate::distance(s.at, at) <= 5.0 {
                return 1;
            }
            return if far { 4 } else { 5 };
        }
        return match (s.between(), far) {
            (true, true) => 2,
            (true, false) => 3,
            (false, true) => 4,
            (false, false) => 5,
        };
    }
    // The glass (`0x15d58c..0x15d7b8`): the board the other side attacks,
    // at 38 - r, the ball's height 28 updates on held to 11.5..15 and its z
    // to ±3.
    let (r, at) = (s.radius, s.ahead);
    let other = s.other as f32;
    let glass = [
        other * (38.0 - r),
        at[1].clamp(11.5, 15.0),
        at[2].clamp(-3.0, 3.0),
    ];
    let [x, z] = s.at;
    if s.blocks >= 80
        && z.abs() >= 1.0
        && at[2].abs() >= r + RING
        && (z >= 0.0) == (at[2] >= 0.0)
        && z.abs() <= 3.0
        && x.abs() <= 36.0
        && other * (38.0 - x) <= 6.0
        && len3(sub3(glass, at)) <= 1.0
    {
        return 6;
    }
    let to_rim = crate::distance(s.at, s.rim);
    let to_ball = contact::angle(s.at, [s.ball[0], s.ball[2]]);
    let ahead = turn(contact::angle(s.at, s.rim), to_ball).abs() < 0x100;
    if ahead && to_rim <= 10.0 && s.shot {
        return 7;
    }
    if s.between() && to_rim >= 10.0 && s.shot {
        return 8;
    }
    if far { 9 } else { 10 }
}

/// Callback 35's bounds: the hand it aims, the rise's bounds, the speed
/// across, the updates it looks ahead, the lowest point and whether at a
/// shot the latest working meet wins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub hand: [f32; 3],
    pub v_top: f32,
    pub v_min: f32,
    pub across: f32,
    pub n: i32,
    pub floor: f32,
    pub latest: bool,
}

/// Callback 35's choice: the velocity (units an update) and the updates
/// it aimed for (-1 with none tried).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    pub velocity: [f32; 3],
    pub meet: i32,
}

/// Callback 35's search (raw `0x176cc0..0x177040`): from `n` updates down
/// to 1 the point `at(k)` (raised to the floor, and to the hand's height
/// until one has worked), the velocity `(P - hand) / k` plus `g k / 2` up,
/// passing over one that would meet coming down (but the first, when it is
/// 1); the first kept, then one that works (the rise within its bounds,
/// across within its limit) over one that does not, then a smaller speed
/// (at a shot the latest working); the rise held to its bounds and the
/// speed across to its limit.
pub fn aim(b: Bounds, at: impl Fn(i32) -> [f32; 3]) -> Aim {
    let g = crate::toss::JUMP_GRAVITY;
    let hand = b.hand;
    let mut best = [0.0_f32; 3];
    let mut meet = -1;
    let mut works = false;
    let mut k = b.n;
    while k > 0 {
        let t = k as f32;
        let mut p = at(k);
        if b.floor > 0.0 && p[1] < b.floor {
            p[1] = b.floor;
        }
        if p[1] < hand[1] {
            if works {
                k -= 1;
                continue;
            }
            p[1] = hand[1];
        }
        let v = [
            (p[0] - hand[0]) / t,
            (p[1] - hand[1]) / t + g * 0.5 * t,
            (p[2] - hand[2]) / t,
        ];
        if (meet != -1 || k >= 2) && v[1] - g * t < 0.0 {
            k -= 1;
            continue;
        }
        let bad = v[1] < b.v_min || b.v_top < v[1] || (v[0] * v[0] + v[2] * v[2]).sqrt() > b.across;
        if meet == -1 {
            meet = k;
            works = !bad;
            best = v;
        } else if bad && works {
        } else if !works && !bad {
            works = true;
            meet = k;
            best = v;
        } else if !b.latest {
            if len3(v) < len3(best) {
                meet = k;
                best = v;
            }
        } else if works && k < meet {
            meet = k;
            best = v;
        }
        k -= 1;
    }
    best[1] = best[1].clamp(b.v_min, b.v_top);
    let side = (best[0] * best[0] + best[2] * best[2]).sqrt();
    if b.across < side {
        best[0] *= b.across / side;
        best[2] *= b.across / side;
    }
    Aim {
        velocity: best,
        meet,
    }
}

/// The rise callback 35 allows: the top `low` (`2.5 + 3.5 × Blocks / 99`,
/// 1 more for a person's jumper on Easy) or `high` (`5.44 + 3.56 × Blocks
/// / 99`, at an alley-oop's leaper), the lowest 2.
pub fn rises(blocks: i32, easy: bool) -> (f32, f32, f32) {
    let g = crate::toss::JUMP_GRAVITY;
    let b = (blocks as f32 / 99.0).clamp(0.0, 1.0);
    let low = b * 3.5 + 2.5 + if easy { 1.0 } else { 0.0 };
    let high = b * 3.56 + 5.44;
    (
        (2.0 * g * low).sqrt(),
        (2.0 * g * high).sqrt(),
        (4.0 * g).sqrt(),
    )
}

/// `FUN_0016f580(actor, 0)`: behind the board at the end his own side
/// attacks.
pub(crate) fn behind_own_board(x: f32, z: f32, side: i32) -> bool {
    if (x >= 0.0) != (side >= 0) {
        return false;
    }
    crate::rebound::under_basket([x, z])
}

impl World {
    /// The holder for the block jump's rules: the player holding the ball
    /// (ball+0xf4).
    fn block_holder(&self) -> Option<usize> {
        match self.ball.state {
            BallState::Held(h) => Some(h),
            _ => None,
        }
    }

    /// The person on a team plays on Easy (`FUN_00172220` 0 with the story
    /// record's difficulty 0).
    fn easy_person(&self, team: usize) -> bool {
        self.rules.difficulty == Difficulty::Easy && self.teams[team].human
    }

    /// `FUN_0015d3a0`: player `i` presses jump with the ball not a rebound
    /// (the rewrite's [`Action::Jumping`]). With the disc's animations the
    /// script is chosen and started; without them, or when two turbos ask
    /// for the turbo block with less than half a bar (no jump at all),
    /// None.
    pub(crate) fn start_block_jump(&mut self, i: usize, turbos: u32) -> Option<Jump> {
        self.poses.get()?;
        let script = if turbos >= 2 {
            // `actor+0x78 & 0x40000`: the turbo block for half the bar.
            if self.turbo_of(i) < TURBO_COST {
                return None;
            }
            if !self.rules.unlimited_turbo {
                self.drain_turbo(i, TURBO_COST);
            }
            TURBO
        } else {
            let kind = kind(&self.block_scene(i));
            let list = KINDS[kind];
            list[(self.rng.next_u32() % list.len() as u32) as usize]
        };
        Some(self.new_block_jump(i, script, turbos >= 2))
    }

    /// The block jump `script` for player `i` (from `FUN_0015d3a0`'s
    /// choice, or the rebound jump's goaltend pair).
    pub(crate) fn new_block_jump(&mut self, i: usize, script: u8, turbo: bool) -> Jump {
        let poses = self.poses.arc();
        let from = self.gait_frame(i);
        let held = poses
            .as_ref()
            .and_then(|p| p.frames(from.record))
            .map_or(from.time, |n| from.time.rem_euclid(n as f32));
        // Callback 5 (`FUN_00174eb0(0, 0)`): his facing word to the ball.
        let p = &self.players[i];
        let b = self.ball.position;
        let word = contact::angle(p.position, [b[0], b[2]]);
        let s = &SCRIPTS[usize::from(script)];
        let branch = if s.glass {
            // The `ec` selector: the alternate when he stands on the far
            // side of the middle from his side's attack (z × side < 0).
            let side = self.sides()[p.team.min(1)] as f32;
            p.position[1] * side < 0.0
        } else if s.records[0] == s.records[1] {
            false
        } else {
            self.block_branch(i, word)
        };
        Jump {
            script,
            branch,
            from: Some((from.record, held)),
            // Counted up before each update's step ([`World::block_jump_tick`]):
            // the press update is the script's tick 0.
            tick: u16::MAX,
            word,
            vy: 0.0,
            drift: [0.0; 2],
            up: false,
            meet: 0,
            airtime: 0,
            flown: 0,
            turbo: script == TURBO && turbo,
        }
    }

    /// Callback 48 (`FUN_00177ce8`): the ball's place 15 updates on
    /// (`fps / 4`) to the side of his facing word: the angle from him to it
    /// (not wrapped) less the word, 0 or less, takes the branch.
    fn block_branch(&self, i: usize, word: i32) -> bool {
        let path = crate::rebound::ball_path(self, 15);
        let at = path[14].0;
        let p = self.players[i].position;
        let a = (crate::math::atan2(p[0] - at[0], p[1] - at[2]) * 162.974_66 + 0.5) as i32;
        turn(a, word) < 1
    }

    /// The scene `FUN_0015d3a0` reads for player `i` ([`kind`]).
    pub(crate) fn block_scene(&self, i: usize) -> Scene {
        let p = &self.players[i];
        let holder = self.block_holder().map(|h| {
            (
                self.players[h].position,
                holder_bits(&self.players[h].action),
                self.octant(h, i),
            )
        });
        let ahead = if holder.is_none() {
            crate::rebound::ball_path(self, 28)[27].0
        } else {
            self.ball.position
        };
        Scene {
            at: p.position,
            side: self.sides()[p.team.min(1)],
            other: self.sides()[(p.team ^ 1).min(1)],
            blocks: p.ratings.native(5),
            holder,
            ball: self.ball.position,
            ahead,
            shot: self.native_ball_state() == game_phase::ball::SHOT,
            radius: self.ball.radius,
            rim: self.rim_spot(target_hoop(self.possession)),
        }
    }

    /// Callback 5's turn: the facing toward the jump's word, 46 an update
    /// from the update after the press (the press update turns as the gait
    /// did).
    pub(crate) fn block_turn(&mut self, i: usize) {
        let Action::Jumping { block: Some(j), .. } = self.players[i].action else {
            return;
        };
        if j.tick == 0 {
            return;
        }
        let now = timing::facing_word(self.players[i].heading);
        let d = turn(j.word, now).clamp(-TURN, TURN);
        if d != 0 {
            self.players[i].heading = crate::restart::heading_radians((now + d) & 0x3ff);
        }
    }

    /// A block jumper's script at the update before his step: the tick,
    /// the turn, and callback 35 at its tick ([`World::block_aim`]).
    pub(crate) fn block_jump_tick(&mut self, i: usize) {
        if let Action::Jumping { block: Some(j), .. } = &mut self.players[i].action {
            j.tick = j.tick.wrapping_add(1);
        }
        self.block_turn(i);
        if let Action::Jumping { block: Some(j), .. } = self.players[i].action
            && !j.up
            && j.tick == j.aim_tick()
        {
            self.block_aim(i);
        }
        self.block_resync(i);
    }

    /// The takeoff's root (`FUN_0019a658`, raw `0x19b684..0x19b704`): every
    /// block script sets channel flag 0x10000 just before its aim (`91 02
    /// 00 01 00`, then callback 35 or the glass pair's `a9`), so the
    /// channel's steps lower the jumper by his clip's root rise, times his
    /// height scale, until a step reaches a new whole frame: the first one
    /// or two airborne updates rise by the clip's drop as its legs extend
    /// (0.03 on `0x441b29`, 0.1 on `0x441a7d`, 0.28 on `0x441db0`), before
    /// the motion step adds his velocity. Without the disc's animations,
    /// none.
    fn block_resync(&mut self, i: usize) {
        let Action::Jumping { block: Some(j), .. } = self.players[i].action else {
            return;
        };
        if !j.resyncing() {
            return;
        }
        let Some(poses) = self.poses.get() else {
            return;
        };
        let root = |time: f32| {
            poses
                .root(Frame {
                    record: j.record(),
                    time,
                    looping: false,
                })
                .map(|r| r[1])
        };
        let (Some(now), Some(then)) = (root(j.frame()), root(j.frame_on(j.tick - 1))) else {
            return;
        };
        let p = &mut self.players[i];
        p.height -= (now - then) * p.body().height;
        if p.height < 0.0 && j.vy <= 0.0 {
            p.height = 0.0;
        }
    }

    /// The hand callback 35 aims: the posed clip's hand points
    /// (`FUN_0014da20` flag 0, moved out by the ball's radius) by its state
    /// (bit 8 the right, else the left; both, 0xc, the point between), at
    /// clip frame `frame`, facing `word`.
    fn block_hand(&self, i: usize, record: u16, frame: f32, word: i32) -> Option<[f32; 3]> {
        let poses = self.poses.get()?;
        let body = self.players[i].body();
        let mut place = self.place(i);
        place.word = word;
        let pose = poses.pose(
            Frame {
                record,
                time: frame,
                looping: false,
            },
            body,
            place,
        )?;
        let f = frame as i32;
        let state = poses.state(record, f);
        let r = self.ball.radius;
        let at = |hand| pose::hand_point(&pose, i32::from(record), f, hand, body, Some(r));
        Some(if state & 0xc == 0xc {
            let (a, b) = (at(1), at(2));
            std::array::from_fn(|k| (a[k] + b[k]) * 0.5)
        } else {
            at(if state & 8 != 0 { 1 } else { 2 })
        })
    }

    /// Callback 35 (`FUN_00176080`, raw `0x176080..0x1771ac`) at the
    /// script's aim: player `i` leaves the floor so that his hand, posed at
    /// the script's frame facing callback 5's word, meets the ball (or a
    /// point by its holder). The highest jump is `2.5 + 3.5 × Blocks / 99`
    /// (1 more for a person's jumper on Easy), `5.44 + 3.56 × Blocks / 99`
    /// at an alley-oop's leaper (holder bit 15); the lowest 2; across at
    /// most 0.57 of the top rise. At a ball in the air it tries the ball's
    /// place `n` ahead from the updates to the top down to 1, held to the
    /// flight's updates left less 3 for a shot or a pass; at a holder, a
    /// point above his head: for a jump shot or lay-up (bits 3, 13) his
    /// head's peak plus 0.36 of his height plus 3, at least 14, 3 toward the
    /// rim from him; for a dunk or a leap (7, 15) 13 on the floor, else
    /// his head's way plus 0.36 of his height, at least 13; for anything
    /// else 18, by him within 6 (straight up if he faces the jumper within
    /// a quarter) or 3 from himself toward him. The velocity is the
    /// rebound aim's (`(P - hand) / n` plus `g n / 2` up); it keeps the
    /// first that works, then a working one with less speed (at a shot, the
    /// latest), holds the rise and the speed across to their bounds. The
    /// airtime is `FUN_0019bed8`; the meet at least 15 updates; the face
    /// reaction draws twice (`r & 1`, `FUN_00159190`).
    pub(crate) fn block_aim(&mut self, i: usize) {
        let Action::Jumping { block: Some(j), .. } = self.players[i].action else {
            return;
        };
        let g = crate::toss::JUMP_GRAVITY;
        let s = &SCRIPTS[usize::from(j.script)];
        let p = self.players[i].clone();
        let hand = self
            .block_hand(i, j.record(), f32::from(s.frame), j.word)
            .unwrap_or([p.position[0], p.height + p.ratings.reach, p.position[1]]);
        let easy = self.easy_person(p.team);
        let (v_low, v_high, v_min) = rises(p.ratings.native(5), easy);
        let state = self.native_ball_state();
        // The holder (or an alley-oop's receiver in the air).
        let mut holder = self.block_holder();
        if holder.is_none()
            && state == game_phase::ball::PASS
            && let Some(pass) = self.ball.pass
            && self.players[pass.to].leaping()
        {
            holder = Some(pass.to);
        }
        let mut target = Target::default();
        let (v_top, across, mut n) = match holder {
            None => {
                let v = v_low;
                let mut n = (v / g) as i32 + 1;
                if matches!(state, game_phase::ball::SHOT | game_phase::ball::PASS) {
                    let left = (self.ball.flight_left / crate::TICK_SECONDS).round() as i32 - 3;
                    n = n.min(left);
                }
                target.latest = state == game_phase::ball::SHOT;
                (v, v * 0.57, n)
            }
            Some(h) => {
                let q = &self.players[h];
                let head = self.actor_pose(h).map_or(
                    [q.position[0], q.height + q.height_ft(), q.position[1]],
                    |pose| pose.joints[21].position,
                );
                // The holder's velocity (actor+0x40): a shot's, dunk's or
                // lay-up's flight on its script ([`crate::jump`]).
                let [vx, vz] = if crate::jump::scripted(&q.action) {
                    [q.flight.velocity[0], q.flight.velocity[2]]
                } else {
                    q.velocity().map(|c| c * crate::TICK_SECONDS)
                };
                let bits = holder_bits(&q.action);
                let view = Holder {
                    bits,
                    head,
                    velocity: [vx, q.rise, vz],
                    height: q.height,
                    height_ft: q.height_ft(),
                    position: q.position,
                    facing: timing::facing_word(q.heading),
                    passing: matches!(q.action, Action::Passing { .. }),
                    left: match q.action {
                        Action::Shooting { tick, release, .. } => {
                            Some(i32::from(release) - i32::from(tick))
                        }
                        // A dunker's meet (actor+0x31c, `0x176c90`): his
                        // aim's updates left, 0 before it.
                        Action::Dunking { .. } if crate::jump::scripted(&q.action) => {
                            Some(i32::from(q.flight.meet))
                        }
                        _ => None,
                    },
                };
                let rim = self.rim_spot(target_hoop(self.possession));
                let (t, v, across, n) = holder_target(&view, p.position, rim, v_low, v_high);
                target = t;
                (v, across, n)
            }
        };
        n = n.max(1);
        let path = if holder.is_none() {
            crate::rebound::ball_path(self, n as usize)
        } else {
            Vec::new()
        };
        let Aim {
            velocity: best,
            meet,
        } = aim(
            Bounds {
                hand,
                v_top,
                v_min,
                across,
                n,
                floor: target.floor,
                latest: target.latest,
            },
            |k| match holder {
                None => path[(k - 1) as usize].0,
                Some(_) => target.at(k as f32, g),
            },
        );
        let airtime = u16::from(timing::airtime(best[1], p.height));
        // `FUN_00159190(actor, r & 1 ? 2 : 1, airtime)`: the face.
        self.rng.next_u32();
        self.face(i, i32::from(airtime));
        if let Action::Jumping { block: Some(j), .. } = &mut self.players[i].action {
            j.vy = best[1];
            j.drift = [best[0], best[2]];
            j.up = true;
            j.meet = meet.max(15).clamp(0, 255) as u8;
            j.airtime = airtime;
            j.flown = 0;
        }
        // Slot 6 at the callback's end: the pump fake's watch.
        self.watch_jumper(i);
    }

    /// `FUN_0016d888`: block jumper `i`'s hands against the ball, in his
    /// update (`FUN_0015da28`) before his step: the bones are the last
    /// update's pose (`FUN_00156f48` poses the model after the actor's
    /// tests; the played captures' knocks come on the first update that
    /// pose reaches, not this update's), the ball where the last ball update
    /// left it. Whether the block test took the ball (knocked, caught or
    /// deflected).
    pub(crate) fn block_test(&mut self, i: usize) -> bool {
        let Action::Jumping { block: Some(j), .. } = self.players[i].action else {
            return false;
        };
        // The press update: the gait's pose, no block clip yet.
        if j.tick == u16::MAX {
            return false;
        }
        let Some(poses) = self.poses.arc() else {
            return false;
        };
        if self.phase != crate::Phase::Live || self.opening_toss != crate::OpeningToss::None {
            return false;
        }
        let p = self.players[i].clone();
        if p.barred > 0 {
            return false;
        }
        let b = self.ball.position;
        // `FUN_0016e830`: nothing from behind the backboard.
        if b[1] > 10.5 && b[0].abs() < 38.0 && b[2].abs() < 3.0 && p.position[0].abs() > 38.0 {
            return false;
        }
        // The game's bars on the jumper (`FUN_0015da28`, `0x15e9..`): a
        // gamebreaker's shot coming down or from inside the arc, and a
        // jumper behind his own board within 4.5 of the offence's rim.
        if let Some(shot) = self.ball.shot
            && shot.gamebreaker > 0
            && (self.ball.velocity[1] < 0.0 || shot.points == 1)
        {
            self.players[i].barred = 1;
            return false;
        }
        let side = self.sides()[p.team.min(1)];
        let rim = self.rim_spot(target_hoop(self.possession));
        if behind_own_board(p.position[0], p.position[1], side)
            && crate::distance(p.position, rim) <= 4.5
        {
            self.players[i].barred = 1;
            return false;
        }
        let shown = j.shown();
        let Some(record) = shown.record else {
            return false;
        };
        let f = shown.frame as i32;
        let state = poses.state(record, f);
        let mut adjust = 0.0;
        if self.easy_person(p.team) {
            // 1.5 at a pass to oneself, or to a leaper from a passer not in
            // a dunk or a leap (bits 7, 15; ball+0xfc, +0x100).
            adjust = match (self.ball.state, self.ball.pass) {
                (BallState::Pass, Some(pass)) => {
                    let leaping = self.players[pass.to].leaping();
                    let busy = holder_bits(&self.players[pass.from].action) & 0x8080 != 0;
                    if pass.from == pass.to || (leaping && !busy) {
                        1.5
                    } else {
                        1.0
                    }
                }
                _ => 1.0,
            };
        } else if self.rules.difficulty == Difficulty::Easy
            && !self.teams[p.team].human
            && self.teams[p.team ^ 1].human
        {
            adjust = -0.5;
        }
        if state & 0xc == 0 || state & 0x240 == 0 {
            return false;
        }
        let reach = self.ball.radius.max(REACH + adjust);
        let body = p.body();
        let Some(pose) = j.pose(&poses, body, self.place(i)) else {
            return false;
        };
        let at = |hand| pose::hand_point(&pose, i32::from(record), f, hand, body, None);
        let mut touch = if state & 0xc == 0xc {
            len3(sub3(at(1), b)) <= reach || len3(sub3(at(2), b)) <= reach
        } else {
            len3(sub3(at(if state & 8 != 0 { 1 } else { 2 }), b)) <= reach
        };
        let from: [f32; 3] = std::array::from_fn(|k| b[k] - self.ball.moved[k]);
        let hit = pose::sweep(&pose, from, b, self.ball.radius, poses.reach(body));
        if !touch && let Some(hit) = hit {
            // The arms' segments count as hands: 14..15 (the left) and
            // 18..19 (the right), by the clip's hands.
            let (a, c) = hit.bones;
            let left = (14..16).contains(&a) && (14..16).contains(&c);
            let right = (18..20).contains(&a) && (18..20).contains(&c);
            touch = (state & 8 != 0 && right) || (state & 4 != 0 && left);
        }
        let holder = self.block_holder();
        let owner = holder
            .or(self.ball.shot.map(|s| s.shooter))
            .or(self.ball.pass.map(|p| p.from));
        if !touch {
            return self.block_deflect(i, hit, holder, owner);
        }
        let native = self.native_ball_state();
        if native == game_phase::ball::RIM {
            if crate::distance([b[0], b[2]], rim) < RING {
                return false;
            }
            let r = &p.ratings;
            let odds =
                ((2 * r.native(3) + r.native(5) + r.native(8) + r.native(4)) / 5).clamp(5, 90);
            if odds < (self.rng.next_u32() % 100) as i32 {
                self.players[i].barred = j.airtime.saturating_sub(j.flown);
                return false;
            }
        }
        // A shot late in its flight (more than 70% flown) is let go
        // `(progress - 0.5) × 200` in 100 (20 less for a person's jumper
        // on Easy).
        let glass = SCRIPTS[usize::from(j.script)].glass;
        let progress = if holder.is_none() && native == game_phase::ball::SHOT && !glass {
            1.0 - self.ball.flight_left / self.ball.flight_time.max(1e-6)
        } else {
            0.0
        };
        if progress >= 0.7 {
            let roll = (self.rng.next_u32() % 100) as i32;
            let easy = if self.easy_person(p.team) { 20 } else { 0 };
            if ((progress - 0.5) * 200.0) as i32 - easy >= roll {
                self.players[i].barred = j.airtime.saturating_sub(j.flown);
                return false;
            }
        }
        if self.ball.shot.is_some_and(|s| s.gamebreaker > 0) && self.ball.velocity[1] < 0.0 {
            return false;
        }
        if let Action::Jumping { block: Some(j), .. } = &mut self.players[i].action {
            j.vy *= 0.5;
        }
        // The faces (`FUN_00159190(.., fps × 2)`, `0x16dfa0..0x16dfec`):
        // the jumper's, the owner's by `r & 1`.
        self.face(i, 2 * crate::TICK_HZ as i32);
        if let Some(o) = owner {
            self.rng.next_u32();
            self.face(o, 2 * crate::TICK_HZ as i32);
        }
        let catches = (record == 437 && j.turbo)
            || matches!(record, 462 | 463)
            || SCRIPTS[usize::from(j.script)].glass;
        if catches {
            // `FUN_0014cb08`: the give and its messages (slot 16 for a shot
            // caught in the air, [`World::post_take`]).
            self.take_ball(i);
            self.events.push(Event::Pickup { player: i });
            return true;
        }
        self.block_knock(i, record, holder, owner);
        true
    }

    /// The knock (`0x16e318..0x16e6e0`): along the jumper's facing turned
    /// by `(r & 0xff) - 0x80`, half a turn more on the clips that block
    /// from in front (438, 439, 444, 445), a quarter either way on 452/458
    /// and 453/459; 0.35 an update across and 0.125 up, a quarter of that
    /// on the turbo block's clip (437); the jumper and the ball's owner
    /// barred `r % 15 + 12` updates or the airtime left; spinning about a
    /// random axis at `0.075 + u × 0.075`; free (`FUN_0014c408`), which
    /// posts the block (slot 16) when the jumper's side is not the one with
    /// the ball and the ball was a shot or in a shooter's, dunker's or
    /// leaper's hands.
    fn block_knock(&mut self, i: usize, record: u16, holder: Option<usize>, owner: Option<usize>) {
        let word = timing::facing_word(self.players[i].heading);
        let turn = (self.rng.next_u32() & 0xff) as i32 - 0x80;
        let mut a = (word - turn) & 0x3ff;
        a += match record {
            438 | 439 | 445 | 444 => 0x200,
            452 | 458 => 0x100,
            453 | 459 => -0x100,
            _ => 0,
        };
        let a = a & 0x3ff;
        let k = if record == 437 { 0.25 } else { 1.0 };
        let v = [
            -crate::input::native_sin_1024(a) * KNOCK[0] * k,
            KNOCK[1] * k,
            -crate::input::native_cos_1024(a) * KNOCK[0] * k,
        ];
        let bar = |w: &mut World, j: usize| {
            let mut n = (w.rng.next_u32() % 15 + 12) as u16;
            if w.players[j].height > 0.0
                && let Some(left) = w.airtime_left(j)
            {
                n = n.max(left);
            }
            w.players[j].barred = n;
        };
        bar(self, i);
        if let Some(o) = owner {
            bar(self, o);
        }
        let mut axis = [0.0_f32; 3];
        for c in &mut axis {
            *c = (self.rng.next_u32() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000) - 0.5;
        }
        let l = len3(axis);
        let axis = if l <= 0.0001 {
            [0.0, 1.0, 0.0]
        } else {
            axis.map(|c| c / l)
        };
        let u = (self.rng.next_u32() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000);
        let spin = u * 0.075 + 0.075;
        let native = self.native_ball_state();
        let shooting = holder.is_some_and(|h| holder_bits(&self.players[h].action) != 0);
        let blocked = self.players[i].team != self.possession
            && (native == game_phase::ball::SHOT || shooting);
        let rim = native == game_phase::ball::RIM || native == game_phase::ball::REBOUND;
        self.ball.drop_loose();
        self.ball.rebound = rim;
        self.ball.velocity = v.map(|c| c / crate::TICK_SECONDS);
        self.ball.spin_axis = axis;
        self.ball.spin = spin;
        self.ball.airborne = true;
        self.ball.flight_left = 0.0;
        self.ball.flight_time = 0.0;
        if blocked {
            let shooter = owner.unwrap_or(i);
            self.credit_block(i, shooter, shooting);
        } else {
            self.events.push(Event::Deflection { player: i });
        }
    }

    /// The limbs' knock when no hand reached (`0x16e6e8..0x16e7f4`): at a
    /// held ball only for a jumper with Blocks 65 or more over the owner's
    /// Dunks and the owner in a dunk or a leap (bits 7, 15); at a free
    /// ball unless he is barred. The limb met deflects it (`FUN_00137ed8(0.15)`), the owner
    /// barred as by a knock.
    fn block_deflect(
        &mut self,
        i: usize,
        hit: Option<pose::Hit>,
        holder: Option<usize>,
        owner: Option<usize>,
    ) -> bool {
        let blocks = self.players[i].ratings.native(5);
        if let Some(h) = holder {
            let q = &self.players[h];
            let up = holder_bits(&q.action) & 0x8080 != 0;
            if blocks < 65 || q.ratings.native(2) >= blocks || !up {
                return false;
            }
        }
        let Some(hit) = hit else {
            return false;
        };
        if let Some(o) = owner {
            let mut n = (self.rng.next_u32() % 15 + 12) as u16;
            if self.players[o].height > 0.0
                && let Some(left) = self.airtime_left(o)
            {
                n = n.max(left);
            }
            self.players[o].barred = n;
        }
        if self.ball.deflect(&hit, 0.15) {
            self.events.push(Event::Deflection { player: i });
        }
        // `0x16e7d8..0x16e7f4`: his cooldown (+0x1cc), unless it runs.
        if self.players[i].cooldown == 0 {
            self.players[i].cooldown = crate::touch::COOLDOWN;
        }
        true
    }

    /// The updates a jumper has left in the air (actor+0x64 - +0x60), for
    /// the block jump's and the rebound jump's flights.
    fn airtime_left(&self, i: usize) -> Option<u16> {
        match self.players[i].action {
            Action::Jumping { block: Some(j), .. } if j.up => {
                Some(j.airtime.saturating_sub(j.flown))
            }
            _ => None,
        }
    }

    /// `FUN_00181f50` (raw `0x181f50..0x182020`): the highest of player
    /// `i`'s posed hands and head (bones 15, 19 and 21 of actor+0x108,
    /// model vt+0x18), his block jump's clip while he is in one (its press
    /// update still the gait's). The computer's pass out of a shot
    /// (`FUN_00181c68`) reads it of the blocker. None without the disc's
    /// animations.
    pub(crate) fn top_bone(&self, i: usize) -> Option<f32> {
        let pose = match self.players[i].action {
            Action::Jumping { block: Some(j), .. } if j.tick != u16::MAX => {
                j.pose(self.poses.get()?, self.players[i].body(), self.place(i))?
            }
            _ => self.actor_pose(i)?,
        };
        Some(
            [pose::LEFT_HAND, pose::RIGHT_HAND, pose::HEAD]
                .map(|b| pose.joints[b].position[1])
                .into_iter()
                .fold(f32::MIN, f32::max),
        )
    }
}

/// What callback 35 reads of the ball's holder (or an alley-oop's
/// receiver in the air): his state bits ([`holder_bits`]), his head (the
/// posed head, joint 21), his velocity (actor+0x40: x, his rise, z), his
/// height off the floor and in feet, his place, his facing word, whether
/// he is passing, and the updates left to his release or meet (a shot's
/// release less its tick, a dunker's meet actor+0x31c, a leap's slam).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Holder {
    pub bits: u32,
    pub head: [f32; 3],
    pub velocity: [f32; 3],
    pub height: f32,
    pub height_ft: f32,
    pub position: [f32; 2],
    pub facing: i32,
    pub passing: bool,
    pub left: Option<i32>,
}

/// Callback 35 at a holder (raw `0x176a00..0x176cc0`): the point, the top
/// rise, the speed across and the updates ahead, for a jumper standing at
/// `jumper` and the rim the holder attacks. A point above his head: for a
/// jump shot or lay-up (bits 3, 13) his head's peak plus 0.36 of his height
/// plus 3, at least 14, 3 toward the rim from him; for a dunk or a leap
/// (7, 15) 13 on the floor, else his head's way plus 0.36 of his height,
/// at least 13; for anything else 18, by him within 6 (straight up if he
/// faces the jumper within a quarter) or 3 from himself toward him.
pub(crate) fn holder_target(
    q: &Holder,
    jumper: [f32; 2],
    rim: [f32; 2],
    v_low: f32,
    v_high: f32,
) -> (Target, f32, f32, i32) {
    let g = crate::toss::JUMP_GRAVITY;
    let bits = q.bits;
    let v = if bits & 0x8000 != 0 { v_high } else { v_low };
    let mut n = (v / g) as i32 + 1;
    let mut across = v * 0.57;
    let mut target = Target::default();
    let head = q.head;
    let [vx, vy, vz] = q.velocity;
    target.from = head;
    target.velocity = [vx, vy, vz];
    let peak = head[1] + vy * vy / (2.0 * g);
    let tall = q.height_ft * 1.2 * 0.3;
    let toward = |from: [f32; 2], word: i32, d: f32| {
        let a = word as f32 * std::f32::consts::TAU / 1024.0;
        [
            from[0] - crate::math::sin(a) * d,
            from[1] - crate::math::cos(a) * d,
        ]
    };
    let to_rim = contact::angle(q.position, rim);
    let rim_side = toward([0.0, 0.0], to_rim, 3.0);
    if bits & 0xa088 == 0 {
        target.y = Some(if q.passing && q.height > 0.0 {
            target.floor = 18.0;
            peak + tall + 3.0
        } else {
            18.0
        });
        n = 20;
        let d = crate::distance(jumper, q.position);
        if d <= 6.0 {
            across *= 0.75;
            target.offset = [rim_side[0], 0.0, rim_side[1]];
            let faces = (q.facing - contact::angle(q.position, jumper)) & 0x3ff;
            let faces = faces.min(0x400 - faces);
            if faces < 0x101 {
                across *= 0.25;
                target.x = Some(jumper[0]);
                target.z = Some(jumper[1]);
            }
        } else {
            let a = contact::angle(jumper, q.position);
            let at = toward(jumper, a, 3.0);
            target.x = Some(at[0]);
            target.z = Some(at[1]);
        }
    } else if bits & 0x2008 != 0 {
        if q.height == 0.0 {
            target.y = Some(14.0);
            n = 20;
        } else {
            n = ((vy / g) as i32).max(if bits & 0x2000 != 0 { 15 } else { 3 });
            target.floor = 14.0;
            target.y = Some(peak + tall + 3.0);
        }
        across *= 0.75;
        target.offset = [rim_side[0], 0.0, rim_side[1]];
    } else {
        if q.height == 0.0 {
            target.y = Some(13.0);
            n = 20;
        } else {
            target.floor = 13.0;
            target.offset[1] = tall;
            n = n.min(q.left.unwrap_or(n) - 3);
        }
        across *= 0.75;
    }
    (target, v, across, n)
}

/// Callback 35's point by a holder: his head moved on `n` updates, or
/// fixed coordinates, plus an offset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Target {
    from: [f32; 3],
    velocity: [f32; 3],
    offset: [f32; 3],
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
    /// The lowest height (callback 35's `f30` with its flag `fp`).
    pub(crate) floor: f32,
    /// At a shot the latest working meet wins (`s6`).
    latest: bool,
}

impl Target {
    pub(crate) fn at(&self, t: f32, g: f32) -> [f32; 3] {
        let x = self
            .x
            .unwrap_or(self.from[0] + t * self.velocity[0] + self.offset[0]);
        let y = self
            .y
            .unwrap_or(self.from[1] + t * self.velocity[1] - 0.5 * g * t * t + self.offset[1]);
        let z = self
            .z
            .unwrap_or(self.from[2] + t * self.velocity[2] + self.offset[2]);
        [x, y, z]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kinds_lists_match_the_executables_counts() {
        // `0x43f840`: 1, 3, 3, 1, 3, 1, 2, 3, 4, 3, 1, 1.
        let counts: Vec<usize> = KINDS.iter().map(|k| k.len()).collect();
        assert_eq!(counts, [1, 3, 3, 1, 3, 1, 2, 3, 4, 3, 1, 1]);
        assert!(
            KINDS
                .iter()
                .flat_map(|k| k.iter())
                .all(|&s| usize::from(s) < SCRIPTS.len())
        );
        assert_eq!(SCRIPTS[usize::from(TURBO)].address, 0x441bd5);
    }

    fn jump(script: u8) -> Jump {
        Jump {
            script,
            branch: true,
            from: Some((9, 4.0)),
            tick: 0,
            word: 0,
            vy: 0.0,
            drift: [0.0; 2],
            up: false,
            meet: 15,
            airtime: 60,
            flown: 0,
            turbo: false,
        }
    }

    #[test]
    fn the_scripts_take_off_at_their_captured_ticks() {
        // The attract captures' block jumps leave the floor 7 (0x441a7d),
        // 6 (0x441b29), 10 (0x441bd5), 8 (0x441db0, 0x44211c, 0x44239b
        // plain), 9 (0x4421cc) updates after the press.
        let ticks: Vec<(u32, u16)> = [0u8, 1, 2, 5, 8, 9, 12]
            .iter()
            .map(|&s| (SCRIPTS[usize::from(s)].address, jump(s).aim_tick()))
            .collect();
        assert_eq!(
            ticks,
            [
                (0x441a7d, 7),
                (0x441b29, 6),
                (0x441bd5, 10),
                (0x441db0, 8),
                (0x44211c, 8),
                (0x4421cc, 9),
                (0x44239b, 10),
            ]
        );
        assert_eq!(
            Jump {
                branch: false,
                ..jump(12)
            }
            .aim_tick(),
            8
        );
    }

    #[test]
    fn the_clip_steps_to_each_waits_frame() {
        // The first capture's 0x441db0 at update 730 (record 447): frame
        // words 1..4 on ticks 4..7, then 6, 8, 11, 13, 16 (16 in the meet
        // less 10 with the meet 15), 17..32 over 16, then on to 41.
        let mut j = jump(5);
        let words: Vec<i32> = (4..8)
            .map(|tick| Jump { tick, ..j }.frame() as i32)
            .collect();
        assert_eq!(words, [1, 2, 3, 4]);
        j.up = true;
        let words: Vec<i32> = (8..14)
            .map(|tick| Jump { tick, ..j }.frame() as i32)
            .collect();
        assert_eq!(words, [6, 8, 11, 13, 16, 17]);
        assert_eq!(Jump { tick: 28, ..j }.frame() as i32, 32);
        assert_eq!(j.shown().record, Some(9));
        assert_eq!(Jump { tick: 4, ..j }.shown().record, Some(447));
        // Landing: the airtime from the aim, here 60.
        assert!(!Jump { tick: 66, ..j }.done());
        assert!(Jump { tick: 67, ..j }.done());
    }
}
