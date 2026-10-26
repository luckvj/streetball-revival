//! The held ball as the original's ball update places it (`FUN_0014e060`,
//! from `FUN_0014a968` while the ball is held; traced raw 2026-10-16,
//! docs/TRACE.md "Blocks and the dribble on the original's bones").
//!
//! The holder's clip marks the frames the ball leaves the hand (state bit
//! 0x10) and comes back to it (0x20). On a frame with neither hand bit
//! (0xc), or between frames from a release frame, the ball is in flight
//! between them: the frame `r` at or before the clip's frame with 0x10
//! (searching back, round the cycle) and the frame `c` after it with 0x20;
//! the hand points of both (posed at those frames where the holder stands
//! now, the hand by each frame's bit 8) are its ends, and `t` the clip's
//! way from `r` to `c`. It moves straight across between them, and its
//! height falls straight from the release hand to the floor (the ball's
//! radius) and rises straight to the catching hand, the floor met at the
//! fraction `dA / (dA + dB / 0.8)` of the way (`dA`, `dB` the hands'
//! heights over the radius, 0.8 the floor's restitution, ball+0x138). Some
//! clips carry the ball other ways: 302 and 303 straight from hand to hand,
//! 222, 223, 300, 301 (and 270, 271 to frame 23, 252, 253 to frame 44) on
//! the floor. Otherwise the ball is in the hand: both hands (0xc) the point
//! between them, else the right on bit 8 and the left on 4
//! (`FUN_0014da20`, flag 0: the ball's radius off the palm).
//!
//! The ends are posed at their own frames where the holder stands now,
//! each moved by its root's way across from that frame to now (the
//! original keeps each end's root, ball+0xb0 and +0xd0, and lays the posed
//! hand on it). The rewrite's own: the hands are posed as [`crate::pose`]
//! poses a clip; the channel blends (flag 0x200000, `DAT_004b8380`) and the
//! cached ends (ball+0xc0, +0xe0, reused while the clip and both frames
//! stay) are not modelled, nor the ball's spin. The bounce's sound (event
//! 0x3c6670) is [`World::dribble_bounce`]'s.

use crate::pose::{self, Body, Frame, Place, Pose, Poses};

/// `FUN_0014da20` (flag 0, the ball's radius off the palm) on `pose` for
/// the hands a frame of `record` holds out (`state`, the frame `frame`'s):
/// both (0xc) the point between them, else the right on 8 and the left
/// (`0x14f194..0x14f2bc`).
pub fn in_hand(
    pose: &Pose,
    record: u16,
    frame: i32,
    state: u32,
    body: Body,
    radius: f32,
) -> [f32; 3] {
    let at = |hand| pose::hand_point(pose, i32::from(record), frame, hand, body, Some(radius));
    if state & 0xc == 0xc {
        let (a, b) = (at(1), at(2));
        std::array::from_fn(|k| (a[k] + b[k]) * 0.5)
    } else {
        at(if state & 8 != 0 { 1 } else { 2 })
    }
}

/// Whether the ball is in flight on `record` at `time`: a dribbling clip
/// (a frame with 0x10) on a frame with no hand bit, or between frames from
/// a release frame (`0x14e45c..0x14e4b4`).
fn flying(poses: &Poses, record: u16, time: f32) -> bool {
    let Some(n) = poses.frames(record).map(|n| n as i32).filter(|&n| n > 0) else {
        return false;
    };
    let cur = time.rem_euclid(n as f32);
    let int = (cur as i32).min(n - 1);
    let s = poses.state(record, int);
    dribbles(poses, record) && (s & 0xc == 0 || (s & 0x10 != 0 && cur != int as f32))
}

/// The release frame at or before `cur` (searched back round the cycle)
/// and the catch frame after it, and the clip's way from one to the other.
fn ends(poses: &Poses, record: u16, cur: f32) -> Option<(i32, i32, f32)> {
    let n = poses.frames(record)? as i32;
    let state = |f: i32| poses.state(record, f.rem_euclid(n));
    let int = (cur as i32).min(n - 1);
    let mut release = int;
    while state(release) & 0x10 == 0 {
        release = if release == 0 { n - 1 } else { release - 1 };
        if release == int {
            break;
        }
    }
    let mut catch = int;
    while state(catch) & 0x20 == 0 {
        catch = if catch == n - 1 { 0 } else { catch + 1 };
        if catch == int {
            break;
        }
    }
    let (r, c, nf) = (release as f32, catch as f32, n as f32);
    let (along, length) = if c < r {
        if r <= cur {
            (cur - r, nf - r + c)
        } else {
            (nf - r + cur, nf - r + c)
        }
    } else {
        (cur - r, c - r)
    };
    let t = if length > 0.0 {
        (along / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some((release, catch, t))
}

/// The live hand (actor+0x25c, 1 the right, 2 the left) after
/// `FUN_0014e060` holds the ball on `record` at `time`: on a dribbling
/// clip in the hand, 2 on a frame with 4 and else 1 (`0x14e490..0x14e4b4`);
/// in flight, the nearer end's frame's hand, 1 on 8 and else 2, if it has
/// one (`0x14e6f4..0x14e718`); on any other clip 1 on 8 alone, 2 on 4
/// alone, unchanged on both or neither (`0x14f194..0x14f1f0`).
pub fn live_hand(poses: &Poses, record: u16, time: f32, hand: u8) -> u8 {
    let Some(n) = poses.frames(record).map(|n| n as i32).filter(|&n| n > 0) else {
        return hand;
    };
    let cur = time.rem_euclid(n as f32);
    let int = (cur as i32).min(n - 1);
    let s = poses.state(record, int);
    if !dribbles(poses, record) {
        return match s & 0xc {
            8 => 1,
            4 => 2,
            _ => hand,
        };
    }
    if !flying(poses, record, time) {
        return if s & 4 != 0 { 2 } else { 1 };
    }
    let Some((release, catch, t)) = ends(poses, record, cur) else {
        return hand;
    };
    let near = poses.state(record, if t < 0.5 { release } else { catch });
    if near & 0xc == 0 {
        hand
    } else if near & 8 != 0 {
        1
    } else {
        2
    }
}

/// `FUN_0014e060`'s ball for a holder playing `record` at `time` (the
/// channel's float frame, +0x64), standing at `place`, with `body`.
pub fn ball_point(
    poses: &Poses,
    record: u16,
    time: f32,
    body: Body,
    place: Place,
    radius: f32,
) -> Option<[f32; 3]> {
    ball_place(poses, record, time, body, place, radius).map(|(at, _)| at)
}

/// Where a dribbling clip has the ball for the bounce's sound
/// (`0x14f410..0x14f4a4`: actor+0x388 set, the clip lets the ball go and
/// neither carries it on the floor nor hand to hand).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dribbled {
    /// In the hand.
    Hand,
    /// In flight: the clip's way `t` from the release to the catch, and
    /// the share of it the fall to the floor takes (`0x14f0e0`).
    Flight { t: f32, fall: f32 },
}

/// [`ball_point`] with where the dribble has the ball ([`Dribbled`]), none
/// off a dribbling clip or on one that carries it on the floor or hand to
/// hand.
pub fn ball_place(
    poses: &Poses,
    record: u16,
    time: f32,
    body: Body,
    place: Place,
    radius: f32,
) -> Option<([f32; 3], Option<Dribbled>)> {
    let n = poses.frames(record)? as i32;
    if n == 0 {
        return None;
    }
    let state = |f: i32| poses.state(record, f.rem_euclid(n));
    let cur = time.rem_euclid(n as f32);
    let int = (cur as i32).min(n - 1);
    let hand_at = |frame: i32| -> Option<[f32; 3]> {
        let pose = poses.pose(
            Frame {
                record,
                time: frame as f32,
                looping: false,
            },
            body,
            place,
        )?;
        Some(in_hand(&pose, record, frame, state(frame), body, radius))
    };
    if !flying(poses, record, time) {
        // In the hand (`0x14f128..0x14f2c4`).
        let pose = poses.pose(
            Frame {
                record,
                time: cur,
                looping: true,
            },
            body,
            place,
        )?;
        let dribbled = dribbles(poses, record).then_some(Dribbled::Hand);
        return Some((
            in_hand(&pose, record, int, state(int), body, radius),
            dribbled,
        ));
    }
    let (release, catch, t) = ends(poses, record, cur)?;
    let c = catch as f32;
    // The ends where the clip's root had them (ball+0xb0, +0xd0: each
    // end's root, posed with it and laid where the holder stands now): the
    // root's way across from that frame to now, at the height scale, turned
    // by the facing. The played captures' dribbles in flight come within
    // 0.012 (median) of the original's ball with it, 0.034 without.
    let shift = |frame: i32| -> Option<[f32; 3]> {
        let at = |time: f32| {
            poses.root(Frame {
                record,
                time,
                looping: false,
            })
        };
        let (then, now) = (at(frame as f32)?, at(cur)?);
        let theta = place.word as f32 * std::f32::consts::TAU / 1024.0;
        let yaw = [
            0.0,
            crate::math::sin(theta * 0.5),
            0.0,
            crate::math::cos(theta * 0.5),
        ];
        let d = [
            (then[0] - now[0]) * body.height,
            0.0,
            (then[2] - now[2]) * body.height,
        ];
        Some(pose::rotate(yaw, d))
    };
    let (sa, sb) = (shift(release)?, shift(catch)?);
    let a = hand_at(release)?;
    let b = hand_at(catch)?;
    let a: [f32; 3] = std::array::from_fn(|k| a[k] + sa[k]);
    let b: [f32; 3] = std::array::from_fn(|k| b[k] + sb[k]);
    let mut at: [f32; 3] = std::array::from_fn(|k| a[k] * (1.0 - t) + b[k] * t);
    let on_floor = matches!(record, 222 | 223 | 300 | 301)
        || (matches!(record, 270 | 271) && c <= 23.0)
        || (matches!(record, 252 | 253) && c <= 44.0);
    let mut dribbled = None;
    if matches!(record, 302 | 303) {
        // Hand to hand.
    } else if on_floor {
        at[1] = radius;
    } else {
        // The bounce (`0x14efd4..0x14f128`).
        let (da, db) = (a[1] - radius, b[1] - radius);
        let whole = da + db / crate::ball::FLOOR_RESTITUTION;
        if whole > 0.0 {
            let fall = da / whole;
            at[1] = if t <= fall {
                a[1] - (t / fall) * da
            } else {
                radius + ((t - fall) / (1.0 - fall)) * db
            };
            dribbled = Some(Dribbled::Flight { t, fall });
        }
    }
    Some((at, dribbled))
}

/// Whether `record` dribbles: a frame of its clip lets the ball go (0x10).
pub fn dribbles(poses: &Poses, record: u16) -> bool {
    poses
        .frames(record)
        .is_some_and(|n| (0..n as i32).any(|f| poses.state(record, f) & 0x10 != 0))
}
