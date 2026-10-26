//! The head turned to its look (`FUN_00154690`, called at the end of
//! `FUN_00159380`, the actor's update; docs/TRACE.md "The head's turn").
//!
//! The look ([`crate::look`]) picks a target; `FUN_00155d90` records it
//! (actor+0x130..+0x154) with the head's last rotation as where a blend
//! starts (+0x110 to +0x120, weight +0x180 = 1, +0x160 set) and
//! `FUN_00155dd8` lets it go the same way. Each update the head (bone 21)
//! takes, in place of its clip's rotation, the turn toward the target in
//! the neck's frame (bone 20's world rotation): a yaw about x and a pitch
//! about z from the direction (each folded back past a quarter turn, the
//! yaw held to ±π/4, the pitch to −π/4 below and an ellipse above), then a
//! roll about y that levels it (at most 0.47 and less as it looks up or
//! down). While the weight runs down (0.1 an update, `0x4b8668`) the head
//! slerps from where it was to that turn, or back to the clip's.
//!
//! The functions keep the original's arithmetic: `FUN_002f64b0`'s
//! half-angle sine and cosine, and the arcsine the inlined arctangents and
//! the slerp share (`0x4b3110..0x4b311c`).

/// `0x4b8668`: the blend's step an update.
pub const BLEND_STEP: f32 = 0.1;

const PI: f32 = f32::from_bits(0x4049_0fdb);
const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);
const QUARTER_PI: f32 = f32::from_bits(0x3f49_0fdb);

/// `0x4b8580`, `0x4b8590`, `0x4b85a0`.
const X: [f32; 3] = [1.0, 0.0, 0.0];
const Y: [f32; 3] = [0.0, 1.0, 0.0];
const Z: [f32; 3] = [0.0, 0.0, 1.0];

/// One actor's head (actor+0x110..+0x180).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Head {
    /// +0x160: a target is held.
    pub active: bool,
    /// +0x180: the blend still to run, 1 down to 0.
    pub weight: f32,
    /// +0x120: where the blend starts.
    pub from: [f32; 4],
    /// +0x110: the head's rotation as the last update left it.
    pub last: [f32; 4],
}

impl Default for Head {
    fn default() -> Self {
        Self {
            active: false,
            weight: 0.0,
            from: [0.0, 0.0, 0.0, 1.0],
            last: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

impl Head {
    /// `FUN_00155d90`: a new target.
    pub fn aim(&mut self) {
        self.weight = 1.0;
        self.from = self.last;
        self.active = true;
    }

    /// `FUN_00155dd8`: the target let go.
    pub fn release(&mut self) {
        if self.active {
            self.weight = 1.0;
            self.from = self.last;
            self.active = false;
        }
    }

    /// One update: the head's local rotation given its clip's (`clip`),
    /// the neck's world rotation and the direction to the target (world,
    /// from the head; `None` without a target).
    pub fn step(&mut self, clip: [f32; 4], neck: [f32; 4], to: Option<[f32; 3]>) -> [f32; 4] {
        let turn = |to: Option<[f32; 3]>| look_rotation(neck, to.unwrap_or(Y));
        let q = if self.weight == 0.0 {
            if !self.active {
                self.last = clip;
                return clip;
            }
            turn(to)
        } else {
            let end = if self.active { turn(to) } else { clip };
            self.weight = (self.weight - BLEND_STEP).max(0.0);
            slerp(self.from, end, 1.0 - self.weight)
        };
        self.last = q;
        q
    }
}

/// The direction to look along from `head` to `target` (`0x154818..
/// 0x1548d0`): normalised, straight up (`0x4b8590`) when they meet.
pub fn direction(head: [f32; 3], target: [f32; 3]) -> [f32; 3] {
    let d: [f32; 3] = std::array::from_fn(|k| target[k] - head[k]);
    let n = len(d);
    if n <= f32::from_bits(0x38d1_b717) {
        Y
    } else {
        d.map(|v| v / n)
    }
}

/// The head's rotation in the neck's frame looking along world direction
/// `dir` (`0x154a94..0x155630`).
pub fn look_rotation(neck: [f32; 4], dir: [f32; 3]) -> [f32; 4] {
    let local = normalize(rotate(conj(neck), dir));
    let mut yaw = atan2(local[2], local[1]);
    let across = len([0.0, local[1], local[2]]);
    let mut pitch = -atan2(local[0], across);
    let wrap = |a: f32| {
        if PI < a {
            a - 2.0 * PI
        } else if a < -PI {
            a + 2.0 * PI
        } else {
            a
        }
    };
    yaw = wrap(yaw);
    pitch = wrap(pitch);
    // Past a quarter turn either way the yaw folds back, and then looks
    // no higher than level.
    if HALF_PI < yaw {
        yaw -= (yaw - HALF_PI) + (yaw - HALF_PI);
        if 0.0 < pitch {
            pitch = 0.0;
        }
    } else if yaw < -HALF_PI {
        let over = -yaw - HALF_PI;
        yaw += over + over;
        if 0.0 < pitch {
            pitch = 0.0;
        }
    }
    if HALF_PI < pitch {
        pitch -= (pitch - HALF_PI) + (pitch - HALF_PI);
    } else if pitch < -HALF_PI {
        let over = -pitch - HALF_PI;
        pitch += over + over;
    }
    yaw = yaw.clamp(-QUARTER_PI, QUARTER_PI);
    pitch = pitch.max(-QUARTER_PI);
    if 0.0 < pitch {
        let top = libm::sqrtf(1.0 - yaw * yaw / f32::from_bits(0x3f1d_e9e7))
            * f32::from_bits(0x3ea0_d97c);
        if top < pitch {
            pitch = top;
        }
    }
    let turned = qmul(axis_angle(X, yaw), axis_angle(Z, pitch));
    let world = qmul(neck, turned);
    // The roll that levels it (`0x155198..0x1555f0`).
    let up = normalize(rotate(world, Y));
    let mut roll = 0.0;
    if up[1].abs() < f32::from_bits(0x3f7f_f972) {
        let level = normalize(std::array::from_fn(|k| Y[k] - up[k] * up[1]));
        let side = normalize(rotate(world, X));
        if f32::from_bits(0x38d1_b717) < side[1] {
            let c = cross(side, level);
            let s = len(c).clamp(f32::from_bits(0xbf7f_be77), f32::from_bits(0x3f7f_be77));
            roll = asin(s);
            if dot(c, up) < 0.0 {
                roll = -roll;
            }
            let most = ((1.0 - up[1].abs()) * HALF_PI).min(f32::from_bits(0x3ef1_463b));
            if most < roll {
                roll = most;
            } else if roll < -most {
                roll = -most;
            }
        }
    }
    qmul(turned, axis_angle(Y, roll))
}

/// The blend from `a` to `b` at `t` (`0x155650..0x155c70`): the nearer
/// way round, a straight mix when the angle is under 0.0001.
pub fn slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    if t < f32::from_bits(0x38d1_b717) {
        return a;
    }
    if f32::from_bits(0x3f7f_f972) < t {
        return b;
    }
    let mut c = dot4(a, b);
    let mut sign = 1.0;
    if c < 0.0 {
        sign = -1.0;
        c = -c;
    }
    let c = c.min(1.0);
    let angle = HALF_PI - asin(c);
    let (ka, kb) = if f32::from_bits(0x38d1_b717) < angle.abs() {
        let inv = 1.0 / sin(angle);
        (sin((1.0 - t) * angle) * inv, sin(t * angle) * inv)
    } else {
        (1.0 - t, t)
    };
    std::array::from_fn(|k| a[k] * ka + b[k] * (sign * kb))
}

/// The arcsine's series (`0x4b3110..0x4b311c`, the first also at gp−32480).
fn series(s: f32) -> f32 {
    let c = [0x3e2b_ba25, 0x3d8a_908b, 0x3d55_033a, 0x3d61_37ab].map(f32::from_bits);
    (((s * c[3] + c[2]) * s + c[1]) * s + c[0]) * s + 1.0
}

/// The inlined arcsine (`0x155444..0x155564`): beyond ±0.7071 through the
/// cosine's root.
fn asin(x: f32) -> f32 {
    let edge = f32::from_bits(0x3f35_04f3);
    if x < -edge {
        let s = 1.0 - x * x;
        libm::sqrtf(s) * series(s) + -HALF_PI
    } else if edge < x {
        let s = 1.0 - x * x;
        HALF_PI - libm::sqrtf(s) * series(s)
    } else {
        x * series(x * x)
    }
}

/// The inlined arctangent of `y` over `x` (`0x154b90..0x154d50`).
fn atan2(y: f32, x: f32) -> f32 {
    if x == 0.0 {
        return if y == 0.0 {
            y
        } else if 0.0 <= y {
            HALF_PI
        } else {
            -HALF_PI
        };
    }
    let r = y / x;
    let mut a = if r < -1.0 {
        let s = 1.0 / (r * r + 1.0);
        libm::sqrtf(s) * series(s) + -HALF_PI
    } else if 1.0 < r {
        let s = 1.0 / (r * r + 1.0);
        HALF_PI - libm::sqrtf(s) * series(s)
    } else {
        let r2 = r * r;
        let s = r2 / (r2 + 1.0);
        let v = libm::sqrtf(s) * series(s);
        if r < 0.0 { -v } else { v }
    };
    if x < 0.0 {
        a = if 0.0 < y { a + PI } else { a - PI };
    }
    a
}

/// The EE's sine (`0x1558a8..0x1559b8`): the quadrant `cvt.w.s` truncates,
/// the polynomials.
fn sin(x: f32) -> f32 {
    let k = x * f32::from_bits(0x3f22_f983);
    let k = if x < 0.0 { k - 0.5 } else { k + 0.5 };
    let q = k as i32;
    let r = x - q as f32 * HALF_PI;
    let r2 = r * r;
    let v = if q & 1 != 0 {
        ((((r2 * f32::from_bits(0x37d0_0d01) + f32::from_bits(0xbab6_0b61)) * r2
            + f32::from_bits(0x3d2a_aaab))
            * r2
            - 0.5)
            * r2)
            + 1.0
    } else {
        ((((r2 * f32::from_bits(0x3638_ef1d) + f32::from_bits(0xb950_0d01)) * r2
            + f32::from_bits(0x3c08_8889))
            * r2
            + f32::from_bits(0xbe2a_aaab))
            * r2)
            * r
            + r
    };
    if q & 2 != 0 { -v } else { v }
}

/// `FUN_002f64b0`: the turn of `angle` about `axis` (the half angle's
/// sine of the polynomial, the cosine `√(1 − s²)`, by quadrant).
fn axis_angle(axis: [f32; 3], angle: f32) -> [f32; 4] {
    let x = angle * 0.5;
    let k = x * f32::from_bits(0x3f22_f983);
    let k = if x < 0.0 { k - 0.5 } else { k + 0.5 };
    let q = k as i32;
    let r = x - q as f32 * HALF_PI;
    let r2 = r * r;
    let s = ((((r2 * f32::from_bits(0x3638_ef1d) + f32::from_bits(0xb950_0d01)) * r2
        + f32::from_bits(0x3c08_8889))
        * r2
        + f32::from_bits(0xbe2a_aaab))
        * r2)
        * r
        + r;
    let c = libm::sqrtf(1.0 - s * s);
    let (s, c) = match q & 3 {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    };
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}

pub fn qmul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

pub fn conj(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

/// `q v q*`.
pub fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let t = qmul(qmul(q, [v[0], v[1], v[2], 0.0]), conj(q));
    [t[0], t[1], t[2]]
}

fn len(v: [f32; 3]) -> f32 {
    libm::sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let n = len(v);
    v.map(|c| c / n)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn dot4(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_series_are_the_ees_arcsine_and_arctangent() {
        for x in [-0.99f32, -0.8, -0.5, 0.0, 0.3, 0.7, 0.75, 0.99] {
            assert!((asin(x) - x.asin()).abs() < 3e-4, "{x}");
        }
        for (y, x) in [
            (1.0f32, 2.0f32),
            (2.0, 1.0),
            (-3.0, 1.0),
            (1.0, -1.0),
            (-1.0, -0.5),
            (0.0, -1.0),
        ] {
            // Along -x the original's comes out at -π (the same way).
            let d = (atan2(y, x) - y.atan2(x)).rem_euclid(2.0 * PI);
            assert!(d.min(2.0 * PI - d) < 3e-4, "{y} {x}");
        }
        for x in [-3.0f32, -1.0, 0.2, 1.5, 3.0] {
            assert!((sin(x) - x.sin()).abs() < 1e-5, "{x}");
        }
    }

    #[test]
    fn straight_along_the_neck_the_head_does_not_turn() {
        // The neck's frame unturned: along its y the yaw and pitch are 0.
        let q = look_rotation([0.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0]);
        assert!(
            (q[3] - 1.0).abs() < 1e-5
                && q[0].abs() < 1e-5
                && q[1].abs() < 1e-5
                && q[2].abs() < 1e-5,
            "{q:?}"
        );
    }

    #[test]
    fn the_yaw_stops_at_a_quarter_turn_of_pi() {
        // Toward z, a quarter turn about x from y: held at π/4.
        let q = look_rotation([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0]);
        let angle = 2.0 * q[0].atan2(q[3]);
        assert!((angle - std::f32::consts::FRAC_PI_4).abs() < 1e-3, "{q:?}");
    }

    #[test]
    fn a_new_target_blends_in_over_ten_updates() {
        let mut h = Head::default();
        let clip = [0.0, 0.0, 0.0, 1.0];
        assert_eq!(h.step(clip, clip, None), clip);
        h.aim();
        let mut n = 0;
        while h.weight > 0.0 {
            h.step(clip, clip, Some([0.0, 0.3, 1.0]));
            n += 1;
        }
        assert_eq!(n, 10);
        let held = h.step(clip, clip, Some([0.0, 0.3, 1.0]));
        assert_eq!(held, look_rotation(clip, [0.0, 0.3, 1.0]));
        // Let go: back to the clip over ten more.
        h.release();
        for _ in 0..10 {
            h.step(clip, clip, None);
        }
        assert_eq!(h.step(clip, clip, None), clip);
    }
}

#[cfg(test)]
mod disc_tests {
    use super::*;
    use crate::pose::{Body, Frame, Place};

    /// On the disc's skeleton the head's y is its facing and its x the
    /// neck's line: looking straight ahead points the head along the
    /// player's facing (record 65's stance looks 38 degrees to his left,
    /// its neck 23), and a target to his left turns it left.
    #[test]
    fn the_head_looks_along_its_y() {
        let crate::pose::Bank::Disc(poses) = crate::pose::disc_bank() else {
            return;
        };
        let place = Place {
            position: [0.0, 0.0, 0.0],
            word: 0,
        };
        let frame = Frame {
            record: 65,
            time: 0.0,
            looping: true,
        };
        let pose = poses.pose(frame, Body::uniform(1.2), place).unwrap();
        let neck = pose.joints[20].rotation;
        let head = pose.joints[21];
        let face = |target: [f32; 3]| {
            let q = look_rotation(neck, direction(head.position, target));
            rotate(qmul(neck, q), Y)
        };
        // Word 0 faces -z.
        let p = head.position;
        let ahead = face([p[0], p[1], p[2] - 10.0]);
        assert!(ahead[2] < -0.95 && ahead[0].abs() < 0.2, "{ahead:?}");
        // To his left (-x), as far as the quarter-π yaw takes it.
        let left = face([p[0] - 10.0, p[1], p[2] - 4.0]);
        assert!(left[0] < -0.6, "{left:?}");
        // Up as far as π/4 from the neck (the pitch's lower bound); down
        // no more than the ellipse's 0.31.
        let up = face([p[0], p[1] + 10.0, p[2] - 3.0]);
        assert!(up[1] > 0.3 && up[1] < 0.75, "{up:?}");
        let down = face([p[0], p[1] - 10.0, p[2] - 3.0]);
        assert!(down[1] > -0.45, "{down:?}");
    }
}
