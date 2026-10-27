//! The litter blowing across a court (`DRIFT`): the manager the court's
//! script builds (`FUN_001f4248`) and its update (`FUN_001f4950`, once a
//! frame), ported from the executable. Its tuning is the constructor's
//! (`FUN_001f5cc8`, `0x1f5ec0..0x1f6040`), all in court units (feet) a
//! frame at the game's 60: 3 mph up for a leaf's gust, 0.1 mph the calm
//! below which nothing comes back in, a fifth of gravity (32.152 ft/s²) for
//! a leaf's fall, a turn a second, eight seconds of rest at most, and the
//! wind itself, 15 mph toward (-1, 0, 1). The court's own `WIND` line goes
//! to the steam and the breath (`0x3f4fec`); the litter never reads it.
//!
//! A leaf (the line's first kind) lies still, sliding to a stop, until its
//! rest (`rand() % 480` frames) runs out; then a gust lifts it at 3 mph
//! with the wind turned up to 45° either way, and it tumbles over a whole
//! number of half turns as it falls back. Trash (the second kind) rolls
//! along with the wind, turned as a leaf's, about the axis across its way,
//! and keeps rolling while it rests; each gust turns it again. A piece
//! that leaves the box stops there and, unless the camera sees it or the
//! spot it would come back at, comes back in somewhere in the box (a leaf
//! up to 11.5 high, already blown along).
//!
//! The draws are the rewrite's own generator: the game takes them from its
//! shared one (`FUN_003a16b8`) and the C library's `rand()`.

use street_sim::{head::qmul, rim::sin_cos};

/// The frames a second the game's tuning is made for.
const FPS: f32 = 60.0;

fn mph(n: f32) -> f32 {
    5280.0 / (FPS * 3600.0) * n
}

/// `0x4c81dc`: a leaf's lift in a gust, 3 mph.
fn lift() -> f32 {
    mph(1.0) * 3.0
}

/// `0x4c81e4`: the calm, 0.1 mph: no wind, nothing comes back in.
fn calm() -> f32 {
    mph(1.0) * f32::from_bits(0x3dcc_cccd)
}

/// `0x4c81e8`: a leaf's fall a frame, a fifth of gravity.
fn fall() -> f32 {
    f32::from_bits(0x4200_9be3) / (FPS * FPS) * f32::from_bits(0x3e4c_cccd)
}

/// `0x4c81f4`: a turn a second, in radians a frame.
fn turn() -> f32 {
    1.0 / FPS * f32::from_bits(0x40c9_0fdb)
}

/// `0x4c81f8`: the wind's speed, 15 mph.
fn wind_speed() -> f32 {
    mph(1.0) * 15.0
}

/// `0x4c8200`: the wind's way, (-1, 0, 1) made unit.
const WIND_WAY: [f32; 3] = [
    -std::f32::consts::FRAC_1_SQRT_2,
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
];

/// The wind (the update's sp+0x50: the way times the speed).
fn wind() -> [f32; 3] {
    WIND_WAY.map(|c| c * wind_speed())
}

/// `0x4c81d8`: the longest rest, eight seconds.
const REST: i32 = 8 * FPS as i32;
const PI: f32 = f32::from_bits(0x4049_0fdb);
const UP: [f32; 3] = [0.0, 1.0, 0.0];

/// What the camera sees (the camera record at `*0x402020`: its place,
/// forward, right and up, and the tangents of its half views, +0x80 and
/// +0x98), and the clip's near and far (`FUN_0030fa50`'s +8, +0xc).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub at: [f32; 3],
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub tan_across: f32,
    pub tan_up: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    /// `0x1f54b8..0x1f55b4`: a piece `size` across at `p` in view.
    fn sees(&self, p: [f32; 3], size: f32) -> bool {
        let rel = sub(p, self.at);
        let depth = dot(rel, self.forward);
        if !(self.near < depth && depth < self.far) {
            return false;
        }
        let k = 1.0 / depth;
        (dot(rel, self.up).abs() - size) * k < self.tan_up
            && (dot(rel, self.right).abs() - size) * k < self.tan_across
    }
}

/// One piece (`0x60` bytes a piece in the game).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    /// +0x00: how it lies.
    pub rotation: [f32; 4],
    /// +0x10.
    pub position: [f32; 3],
    /// +0x20, a frame.
    velocity: [f32; 3],
    /// +0x30, +0x40: what it turns about, and how fast (radians a frame,
    /// the quaternion's angle: twice that on screen).
    axis: [f32; 3],
    spin: f32,
    /// +0x44: the height it rests at (0.25 a leaf, 0.375 trash).
    pub height: f32,
    /// +0x48: frames left resting; 0 in the air or rolling.
    rest: i32,
    /// +0x4c: inside the box.
    inside: bool,
    /// +0x50: a leaf (false) or trash.
    pub trash: bool,
}

/// The manager: its pieces, the box they blow in (±`half`) and the draws.
#[derive(Clone, Debug)]
pub struct Drift {
    pub pieces: Vec<Piece>,
    half: [f32; 2],
    rng: u32,
}

impl Drift {
    /// `FUN_001f4248`: `leaves` of the first kind then `trash` of the
    /// second, each at rest at its height somewhere in the box, facing up.
    pub fn new(leaves: u32, trash: u32, half: [f32; 2], seed: u32) -> Self {
        let mut d = Drift {
            pieces: Vec::new(),
            half,
            rng: seed | 1,
        };
        for k in 0..leaves + trash {
            let trash = k >= leaves;
            let height = if trash { 0.375 } else { 0.25 };
            let x = (d.unit() - 0.5) * (half[0] + half[0]);
            let z = (d.unit() - 0.5) * (half[1] + half[1]);
            d.pieces.push(Piece {
                rotation: [0.0, 0.0, 0.0, 1.0],
                position: [x, height, z],
                velocity: [0.0; 3],
                axis: UP,
                spin: 0.0,
                height,
                rest: 0,
                inside: true,
                trash,
            });
        }
        d
    }

    fn next(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// A draw of the shared generator as the update takes it: its low 31
    /// bits over 2³¹.
    fn unit(&mut self) -> f32 {
        (self.next() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000)
    }

    /// `rand()`.
    fn rand(&mut self) -> i32 {
        (self.next() >> 1) as i32
    }

    /// One frame of `FUN_001f4950`. Gusts come but in a court with
    /// precipitation and no cold (+0x3f4fe0, +0x3f4fe4; no court has
    /// any).
    pub fn step(&mut self, camera: Option<&Camera>) {
        for i in 0..self.pieces.len() {
            let mut p = self.pieces[i];
            if p.rest > 0 {
                p.rest -= 1;
                if p.rest == 0 {
                    self.gust(&mut p);
                }
            }
            // `0x1f50c4..0x1f521c`: a leaf in the air falls, at rest it
            // slows; trash rolls on its way.
            if p.trash {
                roll(&mut p);
            } else if p.rest == 0 {
                p.velocity[1] -= fall();
            } else {
                let k = f32::from_bits(0x3f73_3333);
                p.velocity = p.velocity.map(|c| c * k);
                p.spin *= k;
            }
            // `0x1f5220..0x1f5268`: out of the box it stops there.
            p.inside = p.position[0].abs() <= self.half[0] && p.position[2].abs() <= self.half[1];
            if !p.inside {
                p.velocity[0] = 0.0;
                p.velocity[2] = 0.0;
            }
            p.position = add(p.position, p.velocity);
            // `0x1f527c..0x1f5410`: down, it rests a while.
            if p.rest == 0 && p.position[1] <= p.height {
                p.rest = self.rand() % REST;
                p.position[1] = p.height;
                p.velocity[1] = 0.0;
                if p.trash {
                    roll(&mut p);
                } else {
                    p.axis = self.up_or_down();
                }
            }
            // `0x1f5414..0x1f5490`: it turns by its spin (`FUN_002f6828`
            // takes degrees and builds `(axis sin, cos)` of their radians).
            let degrees = p.spin * f32::from_bits(0x3ea2_f983) * 180.0;
            let (s, c) = sin_cos(degrees * f32::from_bits(0x3c0e_fa35));
            let q = normalize4([p.axis[0] * s, p.axis[1] * s, p.axis[2] * s, c]);
            p.rotation = normalize4(qmul(q, p.rotation));
            if !p.inside && calm() < wind_speed() {
                self.respawn(&mut p, camera);
            }
            self.pieces[i] = p;
        }
    }

    /// The axis a leaf spins flat about: up or down, even odds.
    fn up_or_down(&mut self) -> [f32; 3] {
        if self.unit() < 0.5 {
            UP
        } else {
            [0.0, -1.0, 0.0]
        }
    }

    /// `0x1f4ac0..0x1f50c0`: a gust, the wind turned up to 45° either way
    /// (`FUN_002f7700` about up).
    fn gust(&mut self, p: &mut Piece) {
        let a = (self.unit() - 0.5) * f32::from_bits(0x3fc9_0fdb);
        let way = turn_up(WIND_WAY, a);
        if p.trash {
            let k = wind_speed() * f32::from_bits(0x3f4c_cccd);
            p.velocity = way.map(|c| c * k);
            roll(p);
            return;
        }
        let k = wind_speed() * f32::from_bits(0x3f66_6666);
        p.velocity = way.map(|c| c * k);
        p.velocity[1] = lift();
        // Up and down again at a fifth of gravity: its frames.
        let up = lift() * (1.0 / fall());
        self.tumble(p, ceil(up + up));
    }

    /// `0x1f4be8..0x1f4f9c`: over `n` frames in the air, a leaf turns a
    /// whole number of half turns: one about a level axis, or an even
    /// number about any; a short flight just spins it flat.
    fn tumble(&mut self, p: &mut Piece, n: f32) {
        let span = n * turn();
        if PI < span {
            let most = (span * f32::from_bits(0x3ea2_f983)) as i32;
            let r = self.rand() % most.max(1);
            let mut k = r + 1;
            if k % 2 == 1 && k >= 2 {
                k = r;
            }
            p.spin = k as f32 * PI / n;
            if k & 1 != 0 {
                let a = (self.unit() - 0.5) * f32::from_bits(0x40c9_0fdb);
                p.axis = normalize3(turn_up([0.0, 0.0, 1.0], a));
            } else {
                let mut v = [self.unit() - 0.5, self.unit() - 0.5, self.unit() - 0.5];
                if v[1].abs() <= f32::from_bits(0x38d1_b717) {
                    v[1] = f32::from_bits(0x3dcc_cccd);
                }
                p.axis = normalize3(v);
            }
        } else {
            p.axis = self.up_or_down();
            p.spin = self.unit() * turn();
        }
    }

    /// `0x1f5494..0x1f5c48`: a piece outside the box comes back in where
    /// the camera sees neither it nor that spot.
    fn respawn(&mut self, p: &mut Piece, camera: Option<&Camera>) {
        if camera.is_some_and(|c| c.sees(p.position, p.height)) {
            return;
        }
        let x = (self.unit() - 0.5) * (self.half[0] + self.half[0]);
        let z = (self.unit() - 0.5) * (self.half[1] + self.half[1]);
        let mut at = [x, p.height, z];
        if !p.trash {
            at[1] += self.unit() * 11.5;
        }
        if camera.is_some_and(|c| c.sees(at, p.height)) {
            return;
        }
        p.rotation = [0.0, 0.0, 0.0, 1.0];
        p.position = at;
        p.rest = 0;
        p.inside = true;
        if p.trash {
            p.velocity = [0.0; 3];
            return;
        }
        p.velocity = wind().map(|c| c * f32::from_bits(0x3f66_6666));
        // Down from where it came back in: its frames.
        let drop = (at[1] - p.height) * (1.0 / fall());
        self.tumble(p, ceil((drop + drop).sqrt()));
    }
}

/// `0x1f50ec..0x1f5218`: trash rolls about the axis across its way, its
/// speed over its height a half turn; still, it stops turning.
fn roll(p: &mut Piece) {
    let speed = len3(p.velocity);
    if f32::from_bits(0x38d1_b717) < speed {
        p.axis = normalize3(cross(UP, p.velocity));
        p.spin = speed / p.height * 0.5;
    } else {
        p.spin = 0.0;
    }
}

/// `FUN_002f7700` about up, applied: `v` turned `a` radians.
fn turn_up(v: [f32; 3], a: f32) -> [f32; 3] {
    let (s, c) = sin_cos(a);
    [c * v[0] + s * v[2], v[1], c * v[2] - s * v[0]]
}

/// Truncated, and one more when that fell short (`cvt.w.s` then a step).
fn ceil(x: f32) -> f32 {
    let t = x as i32 as f32;
    if t < x { t + 1.0 } else { t }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn len3(v: [f32; 3]) -> f32 {
    dot(v, v).sqrt()
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let k = 1.0 / len3(v);
    v.map(|c| c * k)
}

fn normalize4(q: [f32; 4]) -> [f32; 4] {
    let k = 1.0 / (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    q.map(|c| c * k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tuning_is_the_constructors() {
        // The bss words save state 9 holds (`0x4c81dc..0x4c8200`).
        assert!((lift() - 0.073_333).abs() < 1e-6);
        assert!((calm() - 0.002_444).abs() < 1e-6);
        assert!((fall() - 0.001_786).abs() < 1e-6);
        assert!((1.0 / fall() - 559.8).abs() < 0.1);
        assert!((turn() - 0.104_72).abs() < 1e-5);
        let w = wind();
        assert!((len3(w) - 0.366_67).abs() < 1e-5);
        assert!(w[0] < 0.0 && w[2] > 0.0 && w[1] == 0.0);
    }

    #[test]
    fn leaves_rest_then_gusts_lift_and_tumble_them_back_down() {
        let mut d = Drift::new(5, 0, [55.0, 32.8], 7);
        let mut lifted = 0;
        let mut highest: f32 = 0.0;
        for _ in 0..3000 {
            d.step(None);
            for p in &d.pieces {
                if p.position[1] > p.height + 0.01 {
                    lifted += 1;
                }
                highest = highest.max(p.position[1]);
                // Never below its rest.
                assert!(p.position[1] >= p.height - 1e-4);
            }
        }
        assert!(lifted > 0, "no gust lifted a leaf");
        // 3 mph up at a fifth of gravity tops out near 1.5 (v² / 2a) over
        // its rest; one back in comes up to 11.5 high.
        let apex = lift() * lift() / (2.0 * fall());
        assert!(highest <= 11.5 + 0.25 + apex, "{highest} vs {apex}");
    }

    #[test]
    fn trash_rolls_downwind_and_comes_back_in() {
        let mut d = Drift::new(0, 5, [55.0, 32.8], 11);
        let start: Vec<[f32; 3]> = d.pieces.iter().map(|p| p.position).collect();
        let mut came_back = 0;
        for _ in 0..6000 {
            let before: Vec<[f32; 3]> = d.pieces.iter().map(|p| p.position).collect();
            d.step(None);
            for (p, was) in d.pieces.iter().zip(&before) {
                assert_eq!(p.position[1], p.height);
                // Out of the box it comes back in at once, elsewhere.
                if len3(sub(p.position, *was)) > 1.0 {
                    came_back += 1;
                }
                assert!(p.position[0].abs() <= 55.0 + 1.0 && p.position[2].abs() <= 32.8 + 1.0);
            }
        }
        assert!(came_back > 0, "nothing came back in");
        // Downwind: -x, +z on the whole.
        let moved = d.pieces.iter().zip(&start).any(|(p, s)| p.position != *s);
        assert!(moved);
    }

    #[test]
    fn nothing_comes_back_in_where_the_camera_sees_it() {
        // A camera over the whole court sees everything: what leaves the
        // box stays out.
        let camera = Camera {
            at: [0.0, 200.0, 0.0],
            forward: [0.0, -1.0, 0.0],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            tan_across: 10.0,
            tan_up: 10.0,
            near: 0.5,
            far: 4000.0,
        };
        let mut d = Drift::new(0, 5, [10.0, 10.0], 3);
        for _ in 0..6000 {
            let before: Vec<bool> = d.pieces.iter().map(|p| p.inside).collect();
            d.step(Some(&camera));
            for (p, &was) in d.pieces.iter().zip(&before) {
                assert!(was || !p.inside, "came back in in view");
            }
        }
    }

    #[test]
    fn a_turn_about_up_is_the_matrixs() {
        // `FUN_002f7700`'s rows for up: (c, 0, -s), (0, 1, 0), (s, 0, c).
        let v = turn_up([0.0, 0.0, 1.0], PI / 2.0);
        assert!((v[0] - 1.0).abs() < 1e-5 && v[2].abs() < 1e-5);
    }
}
