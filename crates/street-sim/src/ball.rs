//! The ball: held, in flight to a hoop or a teammate, or loose and bouncing.
//! This is only its motion; who gets it is up to the [`World`](crate::World).
//!
//! Every ball nobody holds moves by one routine, [`Ball::step`], a port of
//! the original's `FUN_0014d1f8` (which `FUN_0014a968` runs for the ball
//! whenever no player holds it, whatever its state): a shot, a pass and a
//! loose ball meet the same floor, backboard, collar, rim and net, and
//! carry the same spin.

use crate::shooting::{
    NATIVE_AIM_HEIGHT, NATIVE_BACKBOARD_BOUNDS_Y, NATIVE_BACKBOARD_BOUNDS_Z,
    NATIVE_NET_BOTTOM_HEIGHT, NATIVE_NET_CONE_SCALE, NATIVE_RIM_COLLAR_X, NATIVE_RIM_COLLAR_Z_MAX,
    NATIVE_RIM_ENTRY_CONE_ANGLE, NATIVE_RIM_HEIGHT, NATIVE_RIM_OUTER_RADIUS, NATIVE_RIM_RADIUS,
};
use crate::street::Basket;
use crate::{GRAVITY, Player, TICK_SECONDS, tuning};

/// Native physics radius: 0x43edc8[0] * 0.5 * 0.75, FUN_0014a300.
pub const RADIUS: f32 = 1.25 * 0.5 * 0.75;
pub const WOMENS_RADIUS: f32 = 1.2 * 0.5 * 0.75;

pub fn valid_radius(radius: f32) -> bool {
    radius == RADIUS || radius == WOMENS_RADIUS
}

/// Standard gameplay shot flight time (`FUN_001505a0`, called from
/// `0014aaf8`). Besides the requested angle, the launcher enforces a
/// half-second flight (`DAT_004b849c` = fps×0.5 = 30 frames, set at
/// `00153e34..00153e40`) and, for releases below 14 units, enough time to
/// rise to 14 and fall to the destination (`00150750..00150794`,
/// `00150798..001507b4`). This also handles a nearly vertical putback
/// without inventing a different arc when the angle cannot reach the rim
/// (`0015071c..00150738` skips the angle time when `R·tanθ−dy ≤ 0`).
/// Staged Level 2 shots come here too: their ball command is the
/// shooter's own shot (`FUN_00333ef8`, `World::staged_shot`), at the
/// launcher's angle for his distance. (`FUN_00334330`'s
/// `t = max(cue frames, 15)` is a toss to a floor point, not a shot.)
/// The original's `tan` is its own polynomial sine over `√(1 − sin²)`
/// (`0x1505b4..0x150698`), not `libm`'s. It also takes `sqrt.s` of
/// `climb − rise` with no sign test (`0x15078c..0x150790`), which the EE
/// answers with `√|x|`; the rewrite drops that term when it is negative,
/// which only a target above 14 units could reach.
pub(crate) fn shot_flight_time(from: [f32; 3], to: [f32; 3], angle: f32) -> f32 {
    let dx = to[0] - from[0];
    let dz = to[2] - from[2];
    let range = libm::sqrtf(dx * dx + dz * dz);
    let rise = to[1] - from[1];
    let vertical = range * libm::tanf(angle) - rise;
    let angle_time = if vertical > 0.0 {
        libm::sqrtf(2.0 * vertical / GRAVITY)
    } else {
        0.0
    };
    let mut minimum: f32 = 0.5;
    if from[1] < 14.0 {
        let climb = 14.0 - from[1];
        let descent = climb - rise;
        if descent >= 0.0 {
            minimum = minimum
                .max(libm::sqrtf(2.0 * climb / GRAVITY) + libm::sqrtf(2.0 * descent / GRAVITY));
        }
    }
    angle_time.max(minimum)
}

// Ball tuning from `FUN_0014a300` at the default table entries (0.5 in
// `0x43edd8` for the restitutions and in `0x43ede8` for the frictions).

/// The floor's restitution (`ball+0x138`, `-1.2t² + 2.2t` at
/// `0x0014a540..0x0014a564`).
pub const FLOOR_RESTITUTION: f32 = 0.8;
/// The floor's friction (`ball+0x13c`, `-1.6t² + 2.6t`).
const FLOOR_FRICTION: f32 = 0.9;
/// The backboard's restitution (`ball+0x120`, `-t² + 2t`).
pub const BOARD_RESTITUTION: f32 = 0.75;
/// The backboard's friction (`ball+0x124`, `-1.2t² + 2.2t`).
const BOARD_FRICTION: f32 = 0.8;
/// The rim's restitution at its back and below (`ball+0x128`,
/// `-0.8t² + 1.8t`); also the collar's.
const RIM_RESTITUTION_BACK: f32 = 0.7;
/// The collar's friction (`ball+0x12c`, `-0.4t² + 1.4t`).
const COLLAR_FRICTION: f32 = f32::from_bits(0x3f19_999a);
/// The rim's restitution on top at its front (`ball+0x130`,
/// `-0.6t² + 1.6t`).
const RIM_RESTITUTION_FRONT: f32 = 0.65;
/// The rim's friction (`ball+0x134`, `-0.4t² + 1.4t`).
const RIM_FRICTION: f32 = f32::from_bits(0x3f19_999a);

/// Backspin a jump shot or lay-up leaves the hand with, radians per update
/// (`DAT_004b8548` = 3 × 2π / 60, set at `0x00153fb8..0x00154024`; given
/// by `FUN_0014aaf8` at `0x0014bdac..0x0014be78`).
pub const SHOT_SPIN: f32 = f32::from_bits(0x3ea0_d97d);
/// A pass's spin, radians per update (`DAT_004b8544` = 2 × 2π / 60, given
/// by `FUN_0014c0c0`).
pub const PASS_SPIN: f32 = f32::from_bits(0x3e56_7751);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BallState {
    /// In a player's hands (or dribbled by them).
    Held(usize),
    /// On its way to a hoop ([`Ball::shot`]).
    Shot,
    /// On its way to a teammate ([`Ball::pass`]).
    Pass,
    /// Free: bouncing or rolling until someone picks it up.
    Loose,
}

/// A shot in the air.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub shooter: usize,
    pub hoop: usize,
    pub made: bool,
    /// Points for a make: 1, or 2 from beyond the arc.
    pub points: u32,
    /// 0, or the level of the gamebreaker this shot is.
    pub gamebreaker: u8,
    /// A lay-up rather than a jump shot.
    pub layup: bool,
    /// Defenders (by player bit) who have had their one try at goaltending it.
    pub tried: u8,
    /// The basket of a dunk or alley-oop dunk the launcher's dunk path
    /// sent down through the ring ([`crate::World::launch_dunk`]);
    /// `None` for a jump shot or lay-up.
    pub slam: Option<Basket>,
}

/// A pass in the air.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pass {
    pub from: usize,
    pub to: usize,
    /// All set for a pass to oneself, which no defender goes for (the
    /// rewrite's own exemption); 0 otherwise.
    pub tried: u8,
    /// What the ball bounces off on the way, if anything.
    pub via: Via,
    /// The receiver's catch row ([`crate::timing::CATCH_ROWS`]), drawn at
    /// the release as `FUN_001611d8` draws it; [`NO_CATCH`] for a pass
    /// with no catch animation (the trick passes and the toss).
    pub catch: u8,
    /// The update it was released in (the catch script's start).
    pub since: u64,
}

/// [`Pass::catch`] for no catch row.
pub const NO_CATCH: u8 = u8::MAX;

/// The first leg of a pass that bounces off something.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    Direct,
    /// Off the floor (e.g. self-toss bounce trick 23).
    Floor,
    /// Off a defender's head (Off the Heezay).
    Head {
        defender: usize,
    },
    /// Off the backboard of `hoop` (Back 2 Papa).
    Glass {
        hoop: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ball {
    /// Selected native physics radius; preserved in network snapshots.
    pub radius: f32,
    /// Centre of the ball.
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub state: BallState,
    /// The shot while [`BallState::Shot`].
    pub shot: Option<Shot>,
    /// The pass while [`BallState::Pass`].
    pub pass: Option<Pass>,
    /// Seconds until a shot or pass arrives.
    pub(crate) flight_left: f32,
    /// The whole flight's seconds (`ball+0x64`, against the frames flown
    /// `ball+0x60`), for how far a pass has come.
    pub(crate) flight_time: f32,
    /// The axis the ball spins about, unit length (`ball+0x70`).
    pub spin_axis: [f32; 3],
    /// How fast it spins about [`Ball::spin_axis`], radians per update
    /// (`ball+0x14c`).
    pub spin: f32,
    /// Falling under gravity (`ball+0x188`); clear while it rolls on the
    /// floor, the rim, the collar or the top of the glass.
    pub(crate) airborne: bool,
    /// Gone down through a ring since the net last caught it
    /// (`ball+0x18c`).
    pub(crate) through: bool,
    /// The next bounce is a bank shot's or a bounce pass's first: it meets
    /// the glass or the floor without friction (`ball+0x198`).
    pub(crate) bank: bool,
    /// Has not gone through a ring since it was last held (`ball+0x1a8`,
    /// set by the catch `FUN_0014cb28`, cleared by `FUN_001515a8`): a ball
    /// coming up through a ring from below is stopped under it.
    pub(crate) armed: bool,
    /// `+0x140`: the court's ceiling height, or below 0 for none
    /// ([`ceiling_for_court`]).
    pub ceiling: f32,
    /// `+0x184` 3 or 4: the ball left a dunk, so the net keeps half its
    /// speed rather than a tenth (`FUN_00153550`).
    pub dunked: bool,
    /// A shot's ball come off the rim or the glass (ball+0x180 0) rather
    /// than let go, knocked away or through the net (1): the game's phase
    /// stays 3 until such a ball falls below 6 ([`crate::game_phase`]).
    /// Set when the timer of a shot that touched the hoop runs out
    /// ([`Ball::touched`], 6 to 0); cleared by the floor or the ceiling
    /// (0 to 1), a hold, a throw and a launch.
    pub(crate) rebound: bool,
    /// A shot in the air that has touched the backboard, the collar or the
    /// rim (ball+0x180 6, `FUN_0014d1f8` raw `0x14d6b4..0x14d6c8`): the
    /// floor no longer ends it, a rebound jumper may not take it inside
    /// the ring (`FUN_0016ed18`), and when its timer runs out it is a
    /// rebound ([`Ball::rebound`]) rather than loose.
    pub(crate) touched: bool,
    /// `+0xa0`: how far the ball moved in the last update (the limbs'
    /// sweep, [`crate::pose::sweep`], runs along it).
    pub(crate) moved: [f32; 3],
    /// `+0x104`/`+0x108` by side: the player of that side the ball's
    /// basket outside a shot goes to (the original's last to free it,
    /// `FUN_0014c408`; the rewrite's: the last to hold, let go or knock
    /// it); `FUN_0014a790` starts each side at its first actor (here
    /// none: the side's first player is taken then).
    pub(crate) touch: [Option<u8>; 2],
    /// `+0x15c`/`+0x160` by side, 3 (a long shot's): such a basket is
    /// worth the long shot's points (`FUN_0012e890`). The launcher writes
    /// the shooter's side its shot's kind and the other 2; a pass both 2; a
    /// knock its side 2; `FUN_0014a790` both 3.
    pub(crate) long: [bool; 2],
    /// `+0xf8`: the last to shoot or dunk it (the launcher's,
    /// `FUN_0014aaf8`), until someone holds it (`FUN_0014cb28` clears it).
    /// Callback 28 reads it ([`crate::World::hangs`]).
    pub(crate) shooter: Option<u8>,
    /// `0x3d6580`: the release callback's last roll (`rand() % 100 <
    /// percent`, a shot's or a dunk's), which callback 28 reads.
    pub(crate) rolled: bool,
}

impl Ball {
    /// A shot's or pass's whole flight in updates (`ball+0x64`).
    pub fn flight_ticks(&self) -> i32 {
        (self.flight_time * crate::TICK_HZ as f32).round() as i32
    }

    /// Updates of the flight flown so far (`ball+0x60`).
    pub fn flown_ticks(&self) -> i32 {
        ((self.flight_time - self.flight_left) * crate::TICK_HZ as f32).round() as i32
    }
}

/// How near the rim's centre a ball must cross the rim plane to score
/// (`FUN_001515a8`'s `0.793225`); also the radius of the rim's ring.
pub const RIM_ENTRY: f32 = 0.793225;
/// The rim's tube radius (`FUN_001526d0`'s `0.043225`).
pub const RIM_TUBE: f32 = 0.043225;

/// The backboard's face, across the floor from the court's middle
/// (`FUN_00151928`'s box starts at 38.0).
pub const BOARD_FACE: f32 = 38.0;

/// The two rims' centres a free ball's contacts use, in the
/// [`World::hoops`](crate::World::hoops) order.
pub type Rims = [[f32; 3]; 2];

/// The original's rims ([`crate::Hoop::native`]).
pub fn native_rims() -> Rims {
    crate::Hoop::native().map(|h| h.rim)
}

/// What one update of a free ball met.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Contact {
    /// The backboard, the collar or the rim (`FUN_0014d1f8`'s `bVar4`).
    pub touched: bool,
    /// The backboard (`FUN_00151928`, kept in `s5`).
    pub board: bool,
    /// The rim or the collar (`FUN_001526d0`, `FUN_00152418`: slot 30).
    pub rim: bool,
    /// The floor (`FUN_00153278`).
    pub floor: bool,
    /// Down through the ring (`FUN_001515a8`).
    pub scored: bool,
}

/// The flags [`Ball::step`] reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Step {
    /// The shot launcher's look-ahead (`ball+0x190`): a basket does not
    /// disarm the ball, and the board-disable record is not read. (The
    /// game also skips its announcements then; the sim has none.)
    pub watch: bool,
    /// The board-disable record (`FUN_00148760`, game+0x134): a trick
    /// pass's leg off the glass of this side (1 for +x, -1 for -x) aimed
    /// 13.25 or more high. While the ball is on that side the backboard,
    /// the collar, the rim and the basket meet nothing (each returns 0,
    /// `0x1515f4`, `0x15197c`, `0x15246c`, `0x152738`).
    pub board_off: Option<f32>,
    /// No net (`ball+0x194`, set by the look-ahead).
    pub no_net: bool,
    /// A staged Level 2 finish (flown while its sequence plays): the
    /// script's own make, aimed at the scene's rim on the script's timing,
    /// passes the rim and the collar untouched. The sim's own.
    pub staged: bool,
}

/// What the shot launcher's look-ahead saw of one flight
/// (`FUN_0014aaf8`, `0014b510..0014b988`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Forecast {
    /// The flight went down through the ring.
    pub scored: bool,
    /// The launcher's ranking: one per update watched (a quarter while the
    /// ball rolls with no speed across or up), or -1 for a flight it
    /// rejects (behind the backboard, or 600 updates).
    pub score: f32,
    /// Updates until the launcher stopped watching: the ball below the rim
    /// plane and not rising, or gone from around the basket.
    pub ticks: u32,
    /// The update of the last contact the launcher saw (the backboard, the
    /// collar, the rim, the floor or the ceiling, or the ring gone through
    /// when it stopped watching), counting the flight it skipped ahead; 0
    /// for none. The shot's timer (`ball+0x158`) is one more
    /// (`0x14b75c..0x14b9e0`: the skip `s6` plus the watched update `s0`,
    /// plus one).
    pub last: u32,
}

/// `FUN_0014fa60`: the point behind the backboard to aim a bank shot at so
/// that it comes off the glass (restitution [`BOARD_RESTITUTION`]) to `x`.
/// `side` is the basket's side, ±1.
pub(crate) fn bank_mirror(x: f32, side: f32, radius: f32) -> f32 {
    x + side * ((BOARD_FACE - radius) - side * x) * (1.0 / BOARD_RESTITUTION + 1.0)
}

/// `FUN_0014faf0`: whether a bank shot from `at` can hit the glass, i.e. the
/// line to the mirrored point `side × (35.91355 − r)` crosses the board's
/// plane within 3 of its middle.
pub(crate) fn bank_possible(at: [f32; 3], side: f32, radius: f32, board_z: f32) -> bool {
    let plane = BOARD_FACE - radius;
    if plane - side * at[0] <= 0.0 {
        return false;
    }
    let mirrored = bank_mirror(side * (35.91355 - radius), side, radius);
    let along = (side * plane - at[0]) / (mirrored - at[0]);
    ((1.0 - along) * (at[2] - board_z)).abs() < 3.0
}

/// `FUN_0014fc00`: a make must come down steeply enough to pass the ring.
/// The entry angle is `asin((r + 0.09294706) × 1.2606764)` (the second
/// constant is 1/0.793225); a bank shot first spreads it over the speed the
/// glass keeps (`atan(tan θ · |(e·x, z)| / |(x, z)|)`). The time to arrive at
/// that angle, `√(2(d·tan θ + dy)/g)`, gets the gameplay minimums of
/// [`shot_flight_time`]; when the requested `angle` would arrive sooner the
/// launch angle becomes `atan(g·T²/d − tan θ)`. The game's own polynomial
/// asin, atan, sin and cos are replaced by `libm`'s.
pub(crate) fn steepen_entry(
    angle: f32,
    from: [f32; 3],
    aim: [f32; 3],
    radius: f32,
    bank: bool,
) -> f32 {
    let across = libm::sqrtf(aim[0] * aim[0] + aim[2] * aim[2]);
    if across <= 0.0001 {
        return angle;
    }
    let mut entry =
        libm::asinf((radius + f32::from_bits(0x3dbe_5b08)) * f32::from_bits(0x3fa1_5dd8));
    if bank {
        let kept = aim[0] * BOARD_RESTITUTION;
        entry =
            libm::atanf(libm::tanf(entry) * libm::sqrtf(kept * kept + aim[2] * aim[2]) / across);
    }
    let height = libm::tanf(entry) * across + aim[1];
    let mut time = if height > 0.0 {
        libm::sqrtf(2.0 * height / GRAVITY)
    } else {
        0.0
    };
    let mut minimum: f32 = 0.5;
    if from[1] < 14.0 {
        let climb = 14.0 - from[1];
        minimum = minimum.max(
            libm::sqrtf(2.0 * climb / GRAVITY) + libm::sqrtf(2.0 * (climb - aim[1]) / GRAVITY),
        );
    }
    time = time.max(minimum);
    let to = [from[0] + aim[0], from[1] + aim[1], from[2] + aim[2]];
    if shot_flight_time(from, to, angle) < time {
        libm::atanf(GRAVITY * time * time / across - libm::tanf(entry))
    } else {
        angle
    }
}

/// How far from the rim's centre across the floor the ball passed the rim
/// plane (11.91355) between `before` and `after`: the point is
/// `before * f + after * (1 - f)` with `f = (after.y - rim) / (after.y -
/// before.y)`, or `after` when the two are within 0.0001 up
/// (`FUN_001515a8`, `0x0015163c..0x00151704`).
fn crossing(before: [f32; 3], after: [f32; 3], hoop_rim: [f32; 3]) -> f32 {
    let fall = after[1] - before[1];
    let f = if fall.abs() <= 0.0001 {
        0.0
    } else {
        (after[1] - NATIVE_RIM_HEIGHT) / fall
    };
    let at = |k: usize| before[k] * f + after[k] * (1.0 - f);
    let (dx, dz) = (at(0) - hoop_rim[0], at(2) - hoop_rim[2]);
    libm::sqrtf(dx * dx + dz * dz)
}

/// `FUN_001515a8`: the ball went down through the rim plane (11.91355)
/// between `before` and `after`, at a point within [`RIM_ENTRY`] of the
/// rim's centre across the floor.
pub fn crossed_rim(before: [f32; 3], after: [f32; 3], hoop_rim: [f32; 3]) -> bool {
    let rim = NATIVE_RIM_HEIGHT;
    after[1] < rim && before[1] >= rim && crossing(before, after, hoop_rim) <= RIM_ENTRY
}

type V3 = [f32; 3];
const UP: V3 = [0.0, 1.0, 0.0];
const DOWN: V3 = [0.0, -1.0, 0.0];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `a × b`, as the game's `vopmula`/`vopmsub` pair computes it.
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn len(a: V3) -> f32 {
    libm::sqrtf(dot(a, a))
}

/// `FUN_00150b90`: the offset from the nearest point of the ring (radius
/// [`RIM_ENTRY`] about the rim's centre, its tube's centre 11.9567747 up;
/// straight at +z when `p` is over the middle) to `p`, and its length.
fn ring_offset(p: V3, hoop: [f32; 3]) -> (V3, f32) {
    let flat = [p[0] - hoop[0], 0.0, p[2] - hoop[2]];
    let l = len(flat);
    let dir = if l <= 0.0001 {
        [0.0, 0.0, 1.0]
    } else {
        scale(flat, 1.0 / l)
    };
    let ring = [
        dir[0] * RIM_ENTRY + hoop[0],
        NATIVE_AIM_HEIGHT,
        dir[2] * RIM_ENTRY + hoop[2],
    ];
    let off = sub(p, ring);
    (off, len(off))
}

/// The rim on the ball's side of the court: the game keeps `±36.75` by the
/// sign of x (`ball+0x150`, `0x0014d2bc..0x0014d2e4`).
fn rim_on_side(rims: Rims, x: f32) -> [f32; 3] {
    let on_side = |rim: &[f32; 3]| (rim[0] > 0.0) == (x > 0.0);
    rims.into_iter().find(on_side).unwrap_or(rims[0])
}

/// The ceiling height per court index (`0x43edf8`, read by `FUN_0014a300`
/// through the config's court `FUN_001e0bc0()+0xcc`): 25.58 on court 6,
/// 33.77 on court 8, none (-1) elsewhere.
pub const COURT_CEILINGS: [f32; 11] = [
    -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 25.58, -1.0, 33.77, -1.0, -1.0,
];

/// [`COURT_CEILINGS`] for a court index (none past the table).
pub fn ceiling_for_court(index: usize) -> f32 {
    COURT_CEILINGS.get(index).copied().unwrap_or(-1.0)
}

impl Ball {
    pub fn loose_at(position: [f32; 3]) -> Self {
        // `FUN_0014a790`: at rest, not spinning, about +x, able to score.
        Self {
            radius: RADIUS,
            position,
            velocity: [0.0; 3],
            state: BallState::Loose,
            shot: None,
            pass: None,
            flight_left: 0.0,
            flight_time: 0.0,
            spin_axis: [1.0, 0.0, 0.0],
            spin: 0.0,
            airborne: false,
            through: false,
            bank: false,
            armed: true,
            ceiling: -1.0,
            dunked: false,
            rebound: false,
            touched: false,
            moved: [0.0; 3],
            touch: [None; 2],
            long: [true; 2],
            shooter: None,
            rolled: false,
        }
    }

    /// `FUN_00137ed8(e, actor, ball, hit)`: a ball moving into the limb it
    /// met bounces off it with restitution `e` (0.15 from the actor's
    /// update), set out to touch the limb, spinning about the normal across
    /// its slide (its tangential speed over its radius an update, at most
    /// pi/10), no faster than 15 mph (`5280 / (60 * 3600) * 15` an update),
    /// and goes free (`FUN_0014c408`). A ball moving away from the limb
    /// goes on untouched. Whether it bounced.
    pub(crate) fn deflect(&mut self, hit: &crate::pose::Hit, e: f32) -> bool {
        let dt = crate::TICK_SECONDS;
        let l = crate::pose::length(hit.normal);
        if l <= 0.0 {
            return false;
        }
        let n = hit.normal.map(|c| c / l);
        let v = self.velocity.map(|c| c * dt);
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let vn = dot(v, n);
        if vn >= 0.0 {
            return false;
        }
        let mut out: [f32; 3] = std::array::from_fn(|k| v[k] - n[k] * ((1.0 + e) * vn));
        let point: [f32; 3] =
            std::array::from_fn(|k| hit.nearest[k] + n[k] * (self.radius + hit.radius));
        let s = dot(out, n);
        let slide: [f32; 3] = std::array::from_fn(|k| out[k] - n[k] * s);
        let axis = [
            n[1] * slide[2] - n[2] * slide[1],
            n[2] * slide[0] - n[0] * slide[2],
            n[0] * slide[1] - n[1] * slide[0],
        ];
        let la = crate::pose::length(axis);
        let (axis, spin) = if la <= 0.0001 {
            ([0.0, 1.0, 0.0], 0.0)
        } else {
            (
                axis.map(|c| c / la),
                (crate::pose::length(slide) / self.radius).min(std::f32::consts::PI / 10.0),
            )
        };
        let cap = 5280.0 / (60.0 * 3600.0) * 15.0;
        let speed = crate::pose::length(out);
        if cap < speed {
            out = out.map(|c| c * cap / speed);
        }
        self.drop_loose();
        self.position = point;
        self.velocity = out.map(|c| c / dt);
        self.spin_axis = axis;
        self.spin = spin;
        self.airborne = true;
        self.flight_left = 0.0;
        self.flight_time = 0.0;
        true
    }

    /// Into `player`'s hands (`FUN_0014cb28`: no speed or spin, armed to
    /// score again, airborne when let go).
    pub fn hold(&mut self, player: usize) {
        self.state = BallState::Held(player);
        self.shooter = None;
        self.shot = None;
        self.pass = None;
        self.velocity = [0.0; 3];
        self.spin = 0.0;
        self.through = false;
        self.bank = false;
        self.armed = true;
        self.airborne = true;
        self.dunked = false;
        self.rebound = false;
        self.touched = false;
    }

    /// Let go of the ball where it is.
    pub(crate) fn drop_loose(&mut self) {
        self.state = BallState::Loose;
        self.shot = None;
        self.pass = None;
        self.rebound = false;
        self.touched = false;
    }

    /// Velocity to go from `from` to `to` in `t` seconds under gravity.
    pub(crate) fn launch(&mut self, from: [f32; 3], to: [f32; 3], t: f32) {
        self.position = from;
        self.velocity = [
            (to[0] - from[0]) / t,
            (to[1] - from[1]) / t + 0.5 * GRAVITY * t,
            (to[2] - from[2]) / t,
        ];
        self.flight_left = t;
        self.flight_time = t;
        self.airborne = true;
        self.through = false;
        self.bank = false;
        self.rebound = false;
        self.touched = false;
    }

    /// The dunk path's flight (`FUN_0014aaf8` with a dunk, raw
    /// `0x14bc08..0x14bd28` and the look-ahead's tries): from `from` to
    /// `to` at `pace` units an update, `T = |to - from| / pace` updates,
    /// the velocity the aim over `T` plus half the gravity times `T` up,
    /// as [`Ball::launch`] flies it; at `T` of 0.0001 or less straight
    /// down at the pace. Returns `T`.
    pub(crate) fn launch_at_pace(&mut self, from: [f32; 3], to: [f32; 3], pace: f32) -> f32 {
        let t = len(sub(to, from)) / pace;
        if t > 0.0001 {
            self.launch(from, to, t * TICK_SECONDS);
        } else {
            self.launch(from, from, TICK_SECONDS);
            self.velocity = [0.0, -pace / TICK_SECONDS, 0.0];
        }
        t
    }

    /// Spin about `velocity × n` at `rate` radians per update, or none
    /// when the velocity is along `n` (the launchers' `0x0014bdac..
    /// 0x0014be78` and `FUN_0014c0c0`).
    fn spin_from(&mut self, n: V3, rate: f32) {
        let c = cross(self.velocity.map(|v| v * TICK_SECONDS), n);
        let l = len(c);
        if l > 0.0001 {
            self.spin_axis = scale(c, 1.0 / l);
            self.spin = rate;
        } else {
            self.spin = 0.0;
        }
    }

    /// The backspin a jump shot or lay-up leaves the hand with
    /// ([`SHOT_SPIN`] about `velocity × up`), the axis turned round for a
    /// `reversed` lay-up (bit 13, `player+0x1e4 == 2` and `FUN_00171e08 &
    /// 1`: [`crate::World::layup_reversed`]; raw `0x14bdac..0x14be78`).
    pub(crate) fn shot_spin(&mut self, reversed: bool) {
        self.spin_from(UP, SHOT_SPIN);
        if reversed && self.spin != 0.0 {
            self.spin_axis = self.spin_axis.map(|c| -c);
        }
    }

    /// Throw from `from` to arrive at `to` in `t` seconds, spinning as
    /// `FUN_0014c0c0` spins a pass.
    pub(crate) fn throw(&mut self, from: [f32; 3], to: [f32; 3], t: f32, pass: Pass) {
        self.launch(from, to, t);
        self.spin_from(UP, PASS_SPIN);
        self.state = BallState::Pass;
        self.pass = Some(pass);
        self.shot = None;
    }

    /// Throw with explicitly supplied velocity (e.g. bounce self-toss). A
    /// bounce pass (`FUN_0014c0c0` with its fourth argument 1) spins about
    /// `velocity × down` and meets the floor the first time without
    /// friction (`ball+0x198`).
    pub(crate) fn throw_with_velocity(
        &mut self,
        from: [f32; 3],
        velocity: [f32; 3],
        t: f32,
        pass: Pass,
    ) {
        self.position = from;
        self.velocity = velocity;
        self.flight_left = t;
        self.flight_time = t;
        self.airborne = true;
        self.through = false;
        self.rebound = false;
        self.touched = false;
        self.bank = matches!(pass.via, Via::Floor);
        self.spin_from(if self.bank { DOWN } else { UP }, PASS_SPIN);
        self.state = BallState::Pass;
        self.pass = Some(pass);
        self.shot = None;
    }

    /// Carried at the waist in front of `player`; how it is dribbled is up to
    /// the renderer.
    pub(crate) fn carry(&mut self, player: &Player) {
        let [fx, fz] = player.forward();
        self.position = [
            player.position[0] + fx * 0.9,
            3.0 + player.height,
            player.position[1] + fz * 0.9,
        ];
        self.velocity = [0.0; 3];
    }

    /// One update of a ball nobody holds, as `FUN_0014d1f8` runs it.
    ///
    /// The ball moves (and falls, if it is airborne, `ball+0x188`). Then,
    /// unless it is on the floor ([`Ball::floor`]) or the ceiling
    /// ([`Ball::ceiling_contact`], two courts only): near the basket on its side (`FUN_00150cc8`) the
    /// backboard ([`Ball::board`]), the collar ([`Ball::collar`]) or else
    /// the rim ([`Ball::rim`]), and the basket ([`Ball::basket`]); then
    /// the net ([`Ball::net`]) the update after a ball went through, if it
    /// touched nothing else. A rolling ball that touched nothing falls
    /// after all: it drops half a gravity step, every contact is tried
    /// again, and if still nothing holds it up it is airborne
    /// (`0x0014d3ec..0x0014d508`). Last, the court keeps it within ±42 by
    /// ±26.5, stopping it against the edge.
    ///
    /// The sim's velocity is per second; the contacts here work on it per
    /// update, as the game's do, so that their thresholds are the game's.
    pub(crate) fn step(&mut self, rims: Rims, how: Step) -> Contact {
        let dt = TICK_SECONDS;
        let g = GRAVITY * dt * dt;
        let before = self.position;
        if self.airborne {
            self.integrate();
        } else {
            for k in 0..3 {
                self.position[k] += self.velocity[k] * dt;
            }
        }
        let hoop = rim_on_side(rims, self.position[0]);
        let side = if self.position[0] > 0.0 { 1.0 } else { -1.0 };
        let per_second = self.velocity;
        let per_update = per_second.map(|v| v * dt);
        self.velocity = per_update;
        let mut contact = Contact::default();
        let mut again = false;
        loop {
            contact.floor = self.floor(g) || self.ceiling_contact();
            if !contact.floor {
                let mut scored = false;
                let off = !how.watch && how.board_off.is_some_and(|s| s * side > 0.0);
                if self.near_hoop() && !off {
                    contact.board = self.board(g, side, hoop);
                    let rim =
                        !how.staged && (self.collar(g, hoop) || self.rim(g, hoop, before, again));
                    contact.touched = rim || contact.board;
                    contact.rim |= rim;
                    if !self.through {
                        scored = self.basket(before, hoop, how.watch);
                        contact.scored |= scored;
                    }
                }
                // The first pass skips the net in the update a ball goes
                // through (`0x0014d380`); the second does not (`0x0014d4a0`).
                if (again || !scored)
                    && self.through
                    && !how.no_net
                    && !contact.touched
                    && self.net(hoop)
                {
                    self.airborne = true;
                }
            }
            if self.airborne || contact.touched || contact.floor {
                break;
            }
            if again {
                self.airborne = true;
                break;
            }
            again = true;
            self.position[1] -= g * 0.5;
            self.velocity[1] -= g;
        }
        // Speeds the contacts left alone keep their per-second value.
        let mut velocity = per_second;
        for k in 0..3 {
            if self.velocity[k] != per_update[k] {
                velocity[k] = self.velocity[k] / dt;
            }
        }
        self.velocity = velocity;
        self.contain();
        contact
    }

    /// `FUN_001507c8`: the velocity of the ball's surface where it touches
    /// along `n`, the centre's plus the spin's `r·ω·(n × axis)`. The game
    /// takes the sine of the angle between the axis and `n` from its own
    /// polynomials (`acos` of `|axis·n|` clamped to 0.9999, then `sin`);
    /// `libm`'s are used here.
    fn surface_velocity(&self, n: V3) -> V3 {
        let v = self.velocity;
        if self.spin == 0.0 {
            return v;
        }
        let c = cross(n, self.spin_axis);
        let l = len(c);
        if l <= 0.0001 {
            return v;
        }
        let c = scale(c, 1.0 / l);
        let along = dot(self.spin_axis, n).abs().min(0.9999);
        let speed = libm::sinf(libm::acosf(along)) * self.radius * self.spin;
        add(v, scale(c, speed))
    }

    /// `FUN_00150d88`: a bounce off a surface facing `n` with restitution
    /// `e` and friction `mu`. The surface's speed into it is reversed and
    /// kept at `e`; the speed along it is slowed by `mu` times the bounce's
    /// change, at most 0.4 of it (none for a bank's first touch,
    /// `ball+0x198`). A ball still sliding takes spin from the friction
    /// ([`Ball::torque`]); otherwise it leaves rolling, spinning
    /// `|v_t| / r` about `n × v_t`.
    fn bounce(&mut self, e: f32, mu: f32, n: V3) {
        let vc = self.surface_velocity(n);
        let vn = scale(n, dot(n, vc));
        let vt = sub(vc, vn);
        let impulse = scale(vn, -(e + 1.0));
        let mut friction = [0.0; 3];
        let mut slides = false;
        if self.bank {
            self.bank = false;
        } else {
            let along = len(vt);
            if along > 0.0001 {
                let cap = along * 0.4;
                let mut f = mu * len(impulse);
                if f < cap {
                    slides = true;
                } else {
                    f = cap;
                }
                friction = scale(vt, -(f / along));
            }
        }
        self.velocity = add(self.velocity, add(impulse, friction));
        if slides {
            self.torque(friction, n);
            return;
        }
        let vt = sub(self.velocity, scale(n, dot(n, self.velocity)));
        let along = len(vt);
        let c = cross(n, vt);
        let l = len(c);
        if along <= 0.0001 || l <= 0.0001 {
            self.spin = 0.0;
            return;
        }
        self.spin_axis = scale(c, 1.0 / l);
        self.spin = along / self.radius;
    }

    /// The spin a push `friction` along the surface facing `n` adds to a
    /// hollow ball: `1.5/r · (-n × F)` (`FUN_00150d88` `0x00150f34..`,
    /// `FUN_001511f8` `0x001513c4..`).
    fn torque(&mut self, friction: V3, n: V3) {
        if len(friction) <= 0.0001 {
            return;
        }
        let c = cross(scale(n, -1.0), friction);
        if len(c) <= 0.0001 {
            return;
        }
        let w = add(
            scale(c, 1.5 / self.radius),
            scale(self.spin_axis, self.spin),
        );
        let rate = len(w);
        if rate > 0.0001 {
            self.spin = rate;
            self.spin_axis = scale(w, 1.0 / rate);
        } else {
            self.spin = 0.0;
        }
    }

    /// `FUN_001511f8`: a rolling ball held up by a surface facing `n`.
    /// Gravity pulls it down and the surface pushes back `g·n_y` along `n`;
    /// friction `mu` times that push (plus `push` along `n`, the rim's
    /// correction) slows the surface's speed, at most 0.4 of it, and spins
    /// the ball.
    fn rest(&mut self, g: f32, mu: f32, push: f32, n: V3) {
        let vc = self.surface_velocity(n);
        let support = if n[1] > 0.0 {
            scale(n, g * n[1])
        } else {
            [0.0; 3]
        };
        let mut change = support;
        change[1] -= g;
        let mut friction = [0.0; 3];
        let along = len(vc);
        if along > 0.0001 {
            let pressed = if push > 0.0 {
                len(add(scale(n, push), support))
            } else {
                len(support)
            };
            let f = mu * pressed;
            let cap = along * 0.4;
            let f = if f < cap { f } else { cap };
            friction = scale(vc, -(f / along));
            change = add(change, friction);
        }
        self.velocity = add(self.velocity, change);
        self.torque(friction, n);
    }

    /// `FUN_00151550`: the ball settles onto the surface facing `n` and
    /// rolls on it: no longer airborne, its speed into it gone.
    fn settle(&mut self, n: V3) {
        self.airborne = false;
        self.velocity = sub(self.velocity, scale(n, dot(self.velocity, n)));
    }

    /// Turn the velocity along the surface facing `n`, keeping its speed
    /// (a rolling ball on the glass, `0x00151f5c..0x00152034`, or on the
    /// rim, `0x00152ad4..0x00152bac`).
    fn slide_along(&mut self, n: V3) {
        let into = dot(n, self.velocity);
        if into == 0.0 {
            return;
        }
        let speed = len(self.velocity);
        self.velocity = sub(self.velocity, scale(n, into));
        let kept = len(self.velocity);
        if kept > 0.0001 {
            self.velocity = scale(self.velocity, speed / kept);
        }
    }

    /// `FUN_00153278`: the floor, under the ball's centre at its radius
    /// (`ball+0x11c`). A rolling ball rolls on with the floor's friction
    /// 0.9 ([`Ball::rest`]); a falling one bounces ([`Ball::bounce`],
    /// restitution 0.8, friction 0.9) and rolls once it comes off slower
    /// than five gravity steps.
    fn floor(&mut self, g: f32) -> bool {
        let r = self.radius;
        if self.position[1] > r {
            return false;
        }
        self.position[1] = r;
        if !self.airborne {
            self.rest(g, FLOOR_FRICTION, 0.0, UP);
            self.velocity[1] = 0.0;
            return true;
        }
        if self.velocity[1] >= 0.0 {
            return false;
        }
        self.bounce(FLOOR_RESTITUTION, FLOOR_FRICTION, UP);
        if self.velocity[1] <= g * 5.0 {
            self.airborne = false;
            self.velocity[1] = 0.0;
        }
        true
    }

    /// `FUN_00153428`: a court with a ceiling (`ball+0x140` at or above
    /// 0) turns a rising ball at `ceiling - r` back down, restitution and
    /// friction 0.5 (the game also clears the gamebreaker flag `+0x178`,
    /// which the sim keeps elsewhere).
    fn ceiling_contact(&mut self) -> bool {
        let r = self.radius;
        if self.ceiling < 0.0 || self.position[1] < self.ceiling - r {
            return false;
        }
        self.position[1] = self.ceiling - r;
        if self.velocity[1] <= 0.0 {
            return false;
        }
        self.bounce(0.5, 0.5, [0.0, -1.0, 0.0]);
        true
    }

    /// `FUN_00150cc8`: around a basket: `11.5 − r ..= r + 15` up, within
    /// `r + 3` of the middle across, `35.91355 − r ..= r + 38.25` out.
    fn near_hoop(&self) -> bool {
        let r = self.radius;
        let [x, y, z] = self.position;
        11.5 - r <= y
            && y <= r + 15.0
            && z.abs() <= r + 3.0
            && 35.91355 - r <= x.abs()
            && x.abs() <= r + 38.25
    }

    /// `FUN_00151928`: the backboard, the box 38.0..38.25 out by
    /// 11.5..15 up by ±3, faces, edges and corners.
    ///
    /// A bank shot's first touch (`ball+0x198`, within 26.5 of the basket)
    /// is put back to where its centre crossed the plane `38 − r` and meets
    /// the face head on. Otherwise a ball within `r` of the box is pushed
    /// out to touch it along the line from its nearest point; a falling
    /// ball that went in deeper is first taken back along its move to where
    /// it crossed the box grown by `r`. The ball bounces off (restitution
    /// 0.75, friction 0.8) and settles onto a surface facing up if it comes
    /// off slowly; a rolling one turns along the surface and rolls on the
    /// top, or bounces off the face when it rolls into it along the collar.
    /// The game measures the bank's 26.5 to the basket of the side with
    /// the ball (`FUN_00153790`); the sim to the basket on the ball's side,
    /// and skips the game's board-disable check (`FUN_00148760`, a record
    /// of `game+0x134` at least 13.25 high), which it does not model.
    fn board(&mut self, g: f32, side: f32, hoop: [f32; 3]) -> bool {
        let r = self.radius;
        if self.position[0].abs() < BOARD_FACE - r {
            return false;
        }
        let mut back = -1.0;
        let mut n = [0.0; 3];
        let (dx, dz) = (self.position[0] - hoop[0], self.position[2] - hoop[2]);
        if self.bank && libm::sqrtf(dx * dx + dz * dz) < 26.5 {
            let vx = self.velocity[0];
            if vx.abs() > 0.0001 {
                let plane = side * (BOARD_FACE - r);
                back = (plane - self.position[0]) / -vx;
                if back >= 0.0 {
                    self.position = add(self.position, scale(self.velocity, -back));
                    n = [-side, 0.0, 0.0];
                }
            } else {
                self.position[1] = NATIVE_BACKBOARD_BOUNDS_Y[0] - r;
                n = DOWN;
                back = 0.0;
            }
        }
        if back < 0.0 {
            let x = if side < 0.0 {
                [-38.25, -BOARD_FACE]
            } else {
                [BOARD_FACE, 38.25]
            };
            let faces = [x, NATIVE_BACKBOARD_BOUNDS_Y, NATIVE_BACKBOARD_BOUNDS_Z];
            let outside = |p: V3| {
                let mut d = [0.0; 3];
                for k in 0..3 {
                    if p[k] < faces[k][0] {
                        d[k] = p[k] - faces[k][0];
                    } else if faces[k][1] < p[k] {
                        d[k] = p[k] - faces[k][1];
                    }
                }
                d
            };
            let mut d = outside(self.position);
            let mut dist = len(d);
            if r < dist {
                return false;
            }
            let still = self.velocity.iter().all(|v| v.abs() <= 0.0001);
            if dist <= 0.0001 && still {
                self.position[1] = NATIVE_BACKBOARD_BOUNDS_Y[0] - r;
                d = DOWN;
            } else if dist < r {
                if dist <= 0.0001 || (self.airborne && !still) {
                    let mut best = -1.0_f32;
                    for (k, [low, high]) in faces.into_iter().enumerate() {
                        if self.velocity[k].abs() <= 0.0001 {
                            continue;
                        }
                        for (face, pad) in [(low, -r), (high, r)] {
                            let t = (face + pad - self.position[k]) / -self.velocity[k];
                            if 0.0 <= t && (best == -1.0 || t < best) {
                                best = t;
                            }
                        }
                    }
                    self.position = add(self.position, scale(self.velocity, -best));
                    d = outside(self.position);
                    dist = len(d);
                }
                // The sim's guard: the game divides by this distance.
                if dist <= 0.0 {
                    return false;
                }
                self.position = add(self.position, scale(d, r / dist - 1.0));
            }
            n = scale(d, 1.0 / len(d));
        }
        if self.airborne {
            if dot(n, self.velocity) >= 0.0 {
                return false;
            }
            self.bounce(BOARD_RESTITUTION, BOARD_FRICTION, n);
            if n[1] > 0.0 && dot(n, self.velocity) <= n[1] * (g * 5.0) {
                self.settle(n);
            }
            return true;
        }
        if n[1] > 0.0 {
            self.slide_along(n);
            self.rest(g, BOARD_FRICTION, 0.0, n);
            return true;
        }
        if self.velocity[1] == 0.0
            && self.position[1] == r + 12.0
            && self.velocity[0].abs() > 0.0001
        {
            if dot(n, self.velocity) >= 0.0 {
                return false;
            }
            self.bounce(BOARD_RESTITUTION, BOARD_FRICTION, n);
            return true;
        }
        self.slide_along(n);
        false
    }

    /// `FUN_00152418`: the collar, the flat top (12 up) of the bracket
    /// from the rim to the board: 36.75..38 out, within 0.25 of the
    /// middle, outside the ring's opening. A ball no more than `r` above
    /// or below the top, and not rising, is put on it: a rolling one rolls
    /// on (friction 0.6), a falling one bounces (restitution 0.7, friction
    /// 0.6) and rolls once it comes off slower than five gravity steps.
    /// The opening is measured from the rim's centre (36.75 in the game).
    fn collar(&mut self, g: f32, hoop: [f32; 3]) -> bool {
        if self.airborne && self.velocity[1] >= 0.0 {
            return false;
        }
        let r = self.radius;
        let top = r + 12.0;
        let [x, y, z] = self.position;
        if top < y || y < 12.0 - r || NATIVE_RIM_COLLAR_Z_MAX < z.abs() {
            return false;
        }
        let x = x.abs();
        if BOARD_FACE < x || x < NATIVE_RIM_COLLAR_X[0] {
            return false;
        }
        let (dx, dz) = (x - hoop[0].abs(), z - hoop[2]);
        if libm::sqrtf(dx * dx + dz * dz) <= RIM_ENTRY {
            return false;
        }
        self.position[1] = top;
        if !self.airborne {
            self.velocity[1] = 0.0;
            self.rest(g, COLLAR_FRICTION, 0.0, UP);
            self.velocity[1] = 0.0;
            self.position[1] = r + 12.0;
            return true;
        }
        self.bounce(RIM_RESTITUTION_BACK, COLLAR_FRICTION, UP);
        if self.velocity[1] <= g * 5.0 {
            self.airborne = false;
            self.velocity[1] = 0.0;
        }
        true
    }

    /// `FUN_001526d0`: the rim, a torus of centre radius [`RIM_ENTRY`] and
    /// tube [`RIM_TUBE`] about the rim's centre, its tube's centre
    /// 11.9567747 up. It is tried only for a ball `11.91355 − r ..= r + 12`
    /// up and `0.75 − r ..= r + 0.83645` from the middle.
    ///
    /// A falling ball touches at the first of a quarter, half and three
    /// quarters of the way along its move, or its end (`before` is where it
    /// started); a rolling one where it is. It is put there and pushed out
    /// to `r + 0.043225` from the tube. A rolling ball turns along the tube
    /// and rolls on (friction 0.6, pressed by the push when it is inside the
    /// ring on the first pass). A falling ball moving into the tube bounces
    /// off (friction 0.6) with a restitution blended by how upward the
    /// contact faces from 0.7 to, on top, 0.7 at the back of the ring and
    /// 0.65 at its front; it settles onto the rim if it comes off slower
    /// than five gravity steps, or, inside the ring, when it moves mostly
    /// across the ring's tangent (`asin` of its speed along the tangent no
    /// more than `0.86393803·√(1 − n_y²)`). The game's `acos` polynomial is
    /// replaced by `libm`'s.
    fn rim(&mut self, g: f32, hoop: [f32; 3], before: V3, again: bool) -> bool {
        let r = self.radius;
        let y = self.position[1];
        if y < NATIVE_RIM_HEIGHT - r || r + 12.0 < y {
            return false;
        }
        let across = |p: V3| {
            let (dx, dz) = (p[0] - hoop[0], p[2] - hoop[2]);
            len([dx, 0.0, dz])
        };
        let middle = across(self.position);
        if r + NATIVE_RIM_OUTER_RADIUS < middle || middle < NATIVE_RIM_RADIUS - r {
            return false;
        }
        let inside = middle < RIM_ENTRY;
        let reach = r + RIM_TUBE;
        let p = self.position;
        let (at, (off, dist)) = if self.airborne {
            let samples = [
                add(scale(p, 0.25), scale(before, 0.75)),
                scale(add(p, before), 0.5),
                add(scale(p, 0.75), scale(before, 0.25)),
                p,
            ];
            let Some(found) = samples
                .into_iter()
                .map(|q| (q, ring_offset(q, hoop)))
                .find(|(_, (_, d))| *d <= reach)
            else {
                return false;
            };
            found
        } else {
            let found = ring_offset(p, hoop);
            if reach < found.1 {
                return false;
            }
            (p, found)
        };
        self.position = at;
        let n = if dist <= 0.0001 {
            UP
        } else {
            self.position = sub(self.position, off);
            scale(off, 1.0 / dist)
        };
        self.position = add(self.position, scale(n, reach));
        if !self.airborne {
            self.slide_along(n);
            let push = if !again && inside {
                len(sub(self.position, at))
            } else {
                0.0
            };
            self.rest(g, RIM_FRICTION, push, n);
            return true;
        }
        let into = dot(n, self.velocity);
        if 0.0 < into {
            return false;
        }
        let mut after = into;
        if into < 0.0 {
            let e = if 0.0 < n[1] {
                let mut front = self.position[0] - hoop[0];
                if 0.0 < hoop[0] {
                    front = -front;
                }
                let f = ((front + RIM_ENTRY) * f32::from_bits(0x3f21_5dd8)).clamp(0.0, 1.0);
                (1.0 - n[1]) * RIM_RESTITUTION_BACK
                    + n[1] * ((1.0 - f) * RIM_RESTITUTION_BACK + f * RIM_RESTITUTION_FRONT)
            } else {
                RIM_RESTITUTION_BACK
            };
            self.bounce(e, RIM_FRICTION, n);
            after = dot(n, self.velocity);
        }
        if 0.0 <= n[1] && after <= n[1] * (g * 5.0) {
            self.settle(n);
            return true;
        }
        if n[1].abs() < 0.9999 && across(self.position) < RIM_ENTRY {
            let c = cross(UP, n);
            if len(c) > 0.0001 {
                let c = if dot(c, self.velocity) < 0.0 {
                    scale(c, -1.0)
                } else {
                    c
                };
                let c = scale(c, 1.0 / len(c));
                let speed = len(self.velocity);
                if speed <= 0.0001 {
                    self.settle(n);
                    return true;
                }
                let cos = (dot(self.velocity, c) / speed).clamp(-0.9999, 0.9999);
                let lean = std::f32::consts::FRAC_PI_2 - libm::acosf(cos);
                if lean <= libm::sqrtf(1.0 - n[1] * n[1]) * NATIVE_RIM_ENTRY_CONE_ANGLE {
                    self.settle(n);
                }
            }
        }
        true
    }

    /// `FUN_001515a8`: the basket. A ball that went down through the rim
    /// plane within [`RIM_ENTRY`] of the middle ([`crossed_rim`]) is
    /// through (`ball+0x18c`) and, outside the look-ahead, may not score
    /// again until it is held. An armed ball coming up through the ring is
    /// stopped just under it (11.870325, no speed up or down).
    fn basket(&mut self, before: V3, hoop: [f32; 3], watch: bool) -> bool {
        let rim = NATIVE_RIM_HEIGHT;
        if self.armed && rim < self.position[1] && before[1] <= rim {
            if crossing(before, self.position, hoop) <= RIM_ENTRY {
                self.velocity[1] = 0.0;
                self.position[1] = f32::from_bits(0x413d_ecda);
            }
            return false;
        }
        if !crossed_rim(before, self.position, hoop) {
            return false;
        }
        self.through = true;
        if !watch {
            self.armed = false;
        }
        true
    }

    /// The net (`FUN_00153550`), the update after the ball went through:
    /// its centre may be `(y − 10.5)(0.75 − r)/1.4567747` from the middle
    /// across (none below 10.5). A ball further out is pulled back onto that
    /// cone (to the middle at 10.5 if it is lower), slowed to a tenth and
    /// its spin to a quarter, and is no longer through. The game keeps half
    /// for a dunk's ball (`ball+0x184` 3 or 4); the sim's dunks place the
    /// ball themselves and are not marked, so every ball keeps a tenth.
    pub(crate) fn net(&mut self, hoop_rim: [f32; 3]) -> bool {
        let r = self.radius;
        let cone =
            (self.position[1] - NATIVE_NET_BOTTOM_HEIGHT) * (0.75 - r) / NATIVE_NET_CONE_SCALE;
        let off = [
            self.position[0] - hoop_rim[0],
            0.0,
            self.position[2] - hoop_rim[2],
        ];
        let d = len(off);
        if d < cone {
            return false;
        }
        if 0.0001 < d {
            let mut cone = cone;
            if cone < 0.0 {
                self.position[1] = NATIVE_NET_BOTTOM_HEIGHT;
                cone = 0.0;
            }
            self.position = add(self.position, scale(off, (cone - d) / d));
        }
        self.velocity = scale(self.velocity, if self.dunked { 0.5 } else { 0.1 });
        self.through = false;
        self.spin *= 0.25;
        true
    }

    /// One update of a shot ([`Ball::step`] at the basket of `rims`). A made
    /// shot scores the moment it goes down through the ring (the basket,
    /// `FUN_001515a8`); the flag gates it so that a caller who sets it (a
    /// staged finish, the game's own tests) gets the outcome it set, and
    /// the launcher chooses aims whose flights end as the release callback
    /// rolled. Otherwise the shot's timer runs down (`ball+0x158`, the
    /// launcher's: [`crate::World::launch_shot`]) as `FUN_0014d1f8`'s state
    /// machine runs it (raw `0x14d630..0x14d6e8`): a shot (5) that touches
    /// the backboard, the collar or the rim is 6 ([`Ball::touched`]); a
    /// shot still 5 whose timer runs out, or that meets the floor or the
    /// ceiling, is loose (1); a 6 whose timer runs out is a rebound (0,
    /// [`Ball::rebound`]), whatever it meets meanwhile. The ball goes on
    /// with the speed and spin it has. A timer that runs out never scores
    /// (the original counts only the ring), except for a staged Level 2
    /// finish, the rewrite's own: its script's make, flown on the script's
    /// timing past the rim untouched, counts when its flight ends. Also
    /// whether the ball met the rim or the collar (slot 30).
    pub(crate) fn fly(&mut self, rims: Rims, staged: bool) -> (Option<Shot>, bool) {
        let contact = self.step(
            rims,
            Step {
                staged,
                ..Step::default()
            },
        );
        (self.fly_on(contact), contact.rim)
    }

    fn fly_on(&mut self, contact: Contact) -> Option<Shot> {
        if contact.scored && self.shot.is_some_and(|s| s.made) {
            let shot = self.shot.take()?;
            self.state = BallState::Loose;
            self.touched = false;
            return Some(shot);
        }
        self.flight_left -= TICK_SECONDS;
        if self.flight_left > 0.0 && (self.touched || !contact.floor) {
            self.touched |= contact.touched;
            return None;
        }
        let mut shot = self.shot.take()?;
        shot.made &= shot.gamebreaker >= 2;
        self.state = BallState::Loose;
        self.rebound = self.touched && !shot.made;
        self.touched = false;
        if shot.made {
            self.velocity = [0.0, -8.0, 0.0];
        }
        Some(shot)
    }

    /// What the shot launcher's look-ahead (`0014b510..0014b988`) makes of
    /// this ball's flight at `hoop_rim`, played through [`Ball::step`] on a
    /// copy (no net, `ball+0x194`; the look-ahead's `ball+0x190`). It first
    /// lets the ball fly to just short of the circle `√9.0625 + r` around
    /// the backboard's middle (the frames until the earlier root of the
    /// circle's quadratic, rounded up, less one), then watches update by
    /// update: it stops once the ball is below the rim plane and not
    /// rising, or more than `r + 3.0103986` off the board's middle across,
    /// or nearer the court than `34.9896 − r`; it rejects a ball that
    /// passes behind the board (`r + 38.25`) or is still going after 600
    /// updates. The ranking adds one per update watched, or a quarter while
    /// the ball rolls with no speed across or up.
    pub(crate) fn forecast(&self, rims: Rims, hoop_rim: [f32; 3]) -> Forecast {
        let mut ball = self.clone();
        let how = Step {
            watch: true,
            no_net: true,
            ..Step::default()
        };
        let r = ball.radius;
        let side = if hoop_rim[0] >= 0.0 { 1.0 } else { -1.0 };
        let zone = libm::sqrtf(9.0625);
        let (dx, dz) = (
            ball.position[0] - side * BOARD_FACE,
            ball.position[2] - hoop_rim[2],
        );
        let (vx, vz) = (
            ball.velocity[0] * TICK_SECONDS,
            ball.velocity[2] * TICK_SECONDS,
        );
        let a = vx * vx + vz * vz;
        let b = 2.0 * (dx * vx + dz * vz);
        let c = dx * dx + dz * dz - (zone + r) * (zone + r);
        let disc = b * b - 4.0 * a * c;
        let mut ahead = 0;
        if disc >= 0.0 && a > 0.0001 {
            let root = libm::sqrtf(disc);
            let t = ((-b + root) * 0.5 / a).min((-b - root) * 0.5 / a);
            if t > 0.0 {
                ahead = libm::ceilf(t - 1.0) as u32;
            }
        }
        for _ in 0..ahead {
            ball.step(rims, how);
        }
        let mut scored = false;
        let mut watched = 0;
        let mut score = 0.0;
        let mut last = 0;
        loop {
            let before = ball.position;
            let contact = ball.step(rims, how);
            scored |= contact.scored;
            watched += 1;
            if contact.touched || contact.floor {
                last = ahead + watched;
            }
            let rolling_still =
                !ball.airborne && (ball.velocity[1] == 0.0 || ball.velocity[0] == 0.0);
            score += if rolling_still { 0.25 } else { 1.0 };
            let y = ball.position[1];
            let result = |score: f32, last: u32| Forecast {
                scored,
                score,
                ticks: ahead + watched,
                last,
            };
            if y < NATIVE_RIM_HEIGHT && y <= before[1] {
                // Through the ring as it stops watching: that update is
                // the last (`0x14b8e0`, `0x14b97c`).
                if ball.through {
                    last = ahead + watched;
                }
                return result(score, last);
            }
            if watched == 1 {
                continue;
            }
            if watched > 600 {
                return result(-1.0, last);
            }
            let x = ball.position[0].abs();
            if x > r + 38.25 && before[0].abs() <= r + 38.25 {
                return result(-1.0, last);
            }
            if x < BOARD_FACE - zone - r || (ball.position[2] - hoop_rim[2]).abs() > zone + r {
                return result(score, last);
            }
        }
    }

    /// One update of a pass ([`Ball::step`] at the original's hoops,
    /// [`native_rims`], which are the court's in every game the sim sets
    /// up: the pass step has no court to ask); true when it reaches its
    /// target point. A pass that meets the rim or the collar is knocked
    /// loose (the game's state 4 goes to 1, `0x0014d74c..0x0014d784`); one
    /// that meets the glass or the floor flies on. A Back 2 Papa leg off the
    /// glass aims at a point on the glass above 13.25 (the rewrite's leg,
    /// 2 over the rim), so the board-disable record lets it through that
    /// side's hoop to its point ([`Step::board_off`]); it ends there, or
    /// when it touches the glass (the sim's own guard).
    pub(crate) fn fly_pass(&mut self) -> bool {
        let board_off = match self.pass {
            Some(Pass {
                via: Via::Glass { hoop },
                ..
            }) => native_rims()
                .get(hoop)
                .map(|r| if r[0] >= 0.0 { 1.0 } else { -1.0 }),
            _ => None,
        };
        let contact = self.step(
            native_rims(),
            Step {
                board_off,
                ..Step::default()
            },
        );
        if contact.touched && !contact.board {
            self.drop_loose();
            return false;
        }
        self.flight_left -= TICK_SECONDS;
        let glass = board_off.is_some();
        (contact.board && glass) || self.flight_left <= 0.0
    }

    /// One update of a loose ball at the original's hoops.
    #[cfg(test)]
    pub(crate) fn roll(&mut self) {
        self.step(native_rims(), Step::default());
    }

    fn integrate(&mut self) {
        let dt = TICK_SECONDS;
        // `FUN_0014d1f8` (`0014d288`, `0014d3ec..0014d414`): position +=
        // velocity (VU0 vector op), then subtract half an acceleration
        // from Y and a full acceleration from VY. This agrees with the
        // parabola used by launch, including at integer-frame
        // destinations; semi-implicit Euler landed low.
        for k in 0..3 {
            self.position[k] += self.velocity[k] * dt;
        }
        self.position[1] -= 0.5 * GRAVITY * dt * dt;
        self.velocity[1] -= GRAVITY * dt;
    }

    /// Native FUN_0014d1f8 perimeter containment: clamps position to ±42.0
    /// (X) and ±26.5 (Z) and zeroes outward velocity.
    fn contain(&mut self) {
        for (k, limit) in [(0, tuning::BALL_HALF_LENGTH), (2, tuning::BALL_HALF_WIDTH)] {
            if self.position[k] > limit && self.velocity[k] > 0.0 {
                self.position[k] = limit;
                self.velocity[k] = 0.0;
            } else if self.position[k] < -limit && self.velocity[k] < 0.0 {
                self.position[k] = -limit;
                self.velocity[k] = 0.0;
            }
        }
    }
}

#[cfg(test)]
#[path = "ball_capture_parity.rs"]
mod capture_parity;

#[cfg(test)]
mod radius_tests {
    use super::*;

    #[test]
    fn native_ball_steps_reach_short_and_long_flight_destinations() {
        for (from, to, frames) in [
            ([36.0, 10.0, 0.0], [38.0, 11.75, 0.0], 30),
            ([10.0, 9.0, -12.0], [38.0, 11.75, 0.0], 60),
            ([-15.0, 7.0, 20.0], [38.0, 11.75, 0.0], 90),
        ] {
            let mut ball = Ball::loose_at(from);
            ball.launch(from, to, frames as f32 * TICK_SECONDS);
            for _ in 0..frames {
                ball.integrate();
            }
            for (axis, &got) in ball.position.iter().enumerate() {
                assert!(
                    (got - to[axis]).abs() < 0.0002,
                    "{frames} frames, axis {axis}: {:?}",
                    ball.position,
                );
            }
            assert!(ball.velocity[1] < 0.0);
        }
    }

    #[test]
    fn native_low_release_arc_rises_to_fourteen_then_falls_to_the_rim() {
        let target = [38.0, 11.75, 0.0];
        for height in [4.0, 8.0, 10.0, 13.0] {
            let from = [38.0, height, 0.0];
            let duration = shot_flight_time(from, target, 3.0 * std::f32::consts::PI / 8.0);
            let mut ball = Ball::loose_at(from);
            ball.launch(from, target, duration);
            let apex = height + ball.velocity[1] * ball.velocity[1] / (2.0 * GRAVITY);
            assert!((apex - 14.0).abs() < 0.00001, "release {height}");
            let arrival_velocity = ball.velocity[1] - GRAVITY * duration;
            assert!(arrival_velocity < 0.0);
            assert!(duration >= 0.5);
        }
        let from = [37.9, 13.9, 0.0];
        assert_eq!(
            shot_flight_time(from, target, std::f32::consts::FRAC_PI_4),
            0.5
        );
        let from = [0.0, 8.0, 0.0];
        let duration = shot_flight_time(from, target, std::f32::consts::FRAC_PI_4);
        let reference = libm::sqrtf(2.0 * (38.0 - (11.75 - 8.0)) / GRAVITY);
        assert!((duration - reference).abs() < 0.000001);
    }

    #[test]
    fn selected_native_size_controls_floor_contact() {
        for radius in [RADIUS, WOMENS_RADIUS] {
            let mut ball = Ball::loose_at([0.0, 0.0, 0.0]);
            ball.radius = radius;
            ball.roll();
            assert_eq!(ball.position[1], radius);
            assert_eq!(ball.velocity[1], 0.0);
        }
        const { assert!(WOMENS_RADIUS < RADIUS) };
        assert!(!valid_radius(f32::NAN));
        assert!(!valid_radius(0.38));
    }

    #[test]
    fn native_ball_bounce_restitution_and_court_containment() {
        // `FUN_00153278`: a falling ball meets the floor at its radius and
        // bounces with restitution 0.8 (`FUN_00150d88`, nothing across, so
        // no friction or spin).
        let mut ball = Ball::loose_at([0.0, RADIUS + 0.1, 0.0]);
        ball.airborne = true;
        ball.velocity = [0.0, -10.0, 0.0];
        let contact = ball.step(native_rims(), Step::default());
        assert!(contact.floor);
        assert_eq!(ball.position[1], RADIUS);
        let into = -10.0 - GRAVITY * TICK_SECONDS;
        assert!((ball.velocity[1] + 0.8 * into).abs() < 1e-3, "{ball:?}");
        assert!(ball.airborne);

        // Native court containment clamps at ±42.0 (X) and ±26.5 (Z) and
        // zeroes outward velocity.
        let mut ball_x = Ball::loose_at([43.0, RADIUS, 0.0]);
        ball_x.velocity = [5.0, 0.0, 0.0];
        ball_x.roll();
        assert_eq!(ball_x.position[0], 42.0);
        assert_eq!(ball_x.velocity[0], 0.0);

        let mut ball_z = Ball::loose_at([0.0, RADIUS, -28.0]);
        ball_z.velocity = [0.0, 0.0, -4.0];
        ball_z.roll();
        assert_eq!(ball_z.position[2], -26.5);
        assert_eq!(ball_z.velocity[2], 0.0);
    }
}
