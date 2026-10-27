//! The rims' give: each hoop's pole sways and its rim bends (the game's
//! `AIPhysBackboard`, vtable `0x43ecb0`, game+0x178 for the hoop at -x and
//! +0x17c for the one at +x; built by `FUN_00148eb0`, reset by
//! `FUN_001490a0`, updated by `FUN_001496c8` after the players each update).
//!
//! The hoop model's skeleton (`<court>bs.o`, `<court>ds.o`) chains bones
//! 1..4 from the root down to the rim. The update turns bones 2, 3 and 4
//! about their z: their local rotations (bone record +0x20, kept at the
//! hoop's +0xd0..+0xf0, `0x148f7c..0x148fe4`) times the quaternions
//! `FUN_002f6fb0` builds of the sway (bones 2 and 3, the second at half)
//! and of the bend (bone 4), `FUN_002f74b0`'s product, `0x14a028..0x14a0e4`.
//! Everything skinned below a bone turns with it ([`Rim::moves`]); a
//! hanging dunker's hands follow bone 4 (`FUN_00149150`).
//!
//! What starts them (the game's listeners, `0x141d48..0x141f14`):
//! - a ball off the rim or the collar (slot 30, `FUN_001526d0`,
//!   `FUN_00152418`) bends the rim on the ball's side ([`Rim::bend`]);
//! - a dunker's hands on the rim (slot 42, actor+0x2b0 set, `0x15734c`)
//!   sways the pole by 3.75 and holds it at the swing's quarter while he
//!   hangs; letting go (slot 43, `0x1573a8`) lets it swing on;
//! - a slam's callback 27 (slot 44, `FUN_001720a8`) sways it by 3 when the
//!   dunker is the ball's last shooter;
//! - a player's head up under the board from behind (slot 75,
//!   `FUN_001389a8`, [`crate::World`]'s hoop contacts) sways it by 1.5;
//! - a game's winning dunk or alley-oop (slot 26, `FUN_001f6d98`, in
//!   every game type but 1) tears it off ([`Rim::tear`]): the bend falls
//!   to −90 and dangles.
//!
//! Not traced: what the render hoop shows after a tear beyond bone 4's
//! bend (the game's reset shows its groups again, `FUN_00317448`). Not
//! ported: the users' counter 7 the torn rim adds to.

/// The hoop model's bones 1..4 (the root, bone 0, sits at the model's
/// origin unturned).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rig {
    /// Bones 1..4's translations in their parents' frames.
    pub t: [[f32; 3]; 4],
    /// Bones 1..4's rotations in their parents' frames (x, y, z, w).
    pub r: [[f32; 4]; 4],
}

impl Default for Rig {
    fn default() -> Self {
        Self::rucker()
    }
}

impl Rig {
    /// Rucker's hoops (`ruckerbs.o`, `ruckerds.o`), for a world without the
    /// disc's courts.
    /// Forest's hoops (`forestbs.o`, `forestds.o`): a tall frame the rim
    /// hangs from, the court of save states 1, 2, 12 and 13 (their RAM's
    /// bone records hold these).
    pub fn forest() -> Self {
        Self {
            t: [
                [2.2324, 19.1065, 0.0],
                [6.1077, 0.0, 0.0],
                [10.4130, 0.0, 0.0],
                [1.0619, 0.0, 0.0],
            ],
            r: [
                [0.0, 0.0, -0.6933, 0.7207],
                [0.0, 0.0, -0.0245, 0.9997],
                [0.0, 0.0, 0.7340, 0.6792],
                [0.0, 0.0, -0.0379, 0.9993],
            ],
        }
    }

    pub fn rucker() -> Self {
        Self {
            t: [
                [-2.8864, -9.5973, 0.0],
                [9.4829, 0.0, 0.0],
                [2.7912, 0.0, 0.0],
                [8.1960, 0.0, 0.0],
            ],
            r: [
                [0.0, 0.0, 0.7059, 0.7083],
                [0.0, 0.0, 0.3649, 0.9310],
                [0.0, 0.0, -0.8971, 0.4418],
                [0.0, 0.0, -0.0476, 0.9989],
            ],
        }
    }

    /// Bones 0..4 in the model posed with bones 1..4's rotations `r`:
    /// each one's rotation and place.
    fn pose(&self, r: [[f32; 4]; 4]) -> [([f32; 4], [f32; 3]); 5] {
        let mut out = [([0.0, 0.0, 0.0, 1.0], [0.0; 3]); 5];
        for k in 0..4 {
            let (q, at) = out[k];
            let d = rotate(q, self.t[k]);
            out[k + 1] = (qmul(q, r[k]), std::array::from_fn(|n| at[n] + d[n]));
        }
        out
    }
}

/// How a bone moved from the rest pose: a point `p` that rode it goes to
/// `to + rot (p − from)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub rot: [f32; 4],
    pub from: [f32; 3],
    pub to: [f32; 3],
}

impl Move {
    pub const STILL: Move = Move {
        rot: [0.0, 0.0, 0.0, 1.0],
        from: [0.0; 3],
        to: [0.0; 3],
    };

    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        let d = rotate(self.rot, std::array::from_fn(|k| p[k] - self.from[k]));
        std::array::from_fn(|k| self.to[k] + d[k])
    }

    /// Between this move and `next` by `t` (the turn slerped, the places
    /// lerped), as a replay shows the hoop between two recorded frames.
    pub fn blend(&self, next: &Move, t: f32) -> Move {
        let lerp = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
        Move {
            rot: crate::head::slerp(self.rot, next.rot, t),
            from: lerp(self.from, next.from),
            to: lerp(self.to, next.to),
        }
    }

    /// [`Move::apply`] backwards: where a point now at `p` sat at rest.
    pub fn undo(&self, p: [f32; 3]) -> [f32; 3] {
        let d = rotate(conj(self.rot), std::array::from_fn(|k| p[k] - self.to[k]));
        std::array::from_fn(|k| self.from[k] + d[k])
    }

    /// The same move in the court's frame for hoop `side`, which the court
    /// places at [`PLACES`].
    pub fn to_court(&self, side: usize) -> Move {
        let (at, turn) = PLACES[side];
        let place = |v: [f32; 3]| {
            let v = rotate(turn, v);
            std::array::from_fn(|k| at[k] + v[k])
        };
        Move {
            rot: qmul(qmul(turn, self.rot), conj(turn)),
            from: place(self.from),
            to: place(self.to),
        }
    }
}

/// Where the game puts the two hoop models (the scene's setup, as
/// `street_assets::court`'s placements): the -x hoop unturned, the +x one
/// half round about y.
pub const PLACES: [([f32; 3], [f32; 4]); 2] = {
    let (y, z) = (f32::from_bits(0x4115_e7d5), f32::from_bits(0xbb2a_64c3));
    [
        ([f32::from_bits(0xc224_0c5d), y, z], [0.0, 0.0, 0.0, 1.0]),
        ([f32::from_bits(0x4224_17ae), y, z], [0.0, 1.0, 0.0, 0.0]),
    ]
};

/// One hoop's state words (+0x70..+0xc0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rim {
    /// +0x70: the rim bends.
    pub bend: bool,
    /// +0x74: the pole sways.
    pub sway: bool,
    /// +0x78: a hanging dunker holds the sway at its quarter.
    pub held: bool,
    /// +0x80: the rim is torn off (never set here).
    pub torn: bool,
    /// +0x84: a sway is under way (a new one does not restart it).
    pub swaying: bool,
    /// +0x88: a torn rim dangles (never set here).
    pub dangles: bool,
    /// +0x8c: a bend is under way.
    pub bending: bool,
    /// +0x90: the bend is on its way back.
    pub returning: bool,
    /// +0x94: the bend's phase, degrees (6 an update to 60 and back).
    pub bend_phase: f32,
    /// +0x98: the bend, degrees (`−6 sin phase`).
    pub bend_angle: f32,
    /// +0x9c: the sway's phase, degrees.
    pub sway_phase: f32,
    /// +0xa0: the sway, degrees (`−amplitude sin phase`).
    pub sway_angle: f32,
    /// +0xa4: a torn rim's dangle step, degrees an update.
    pub dangle_step: f32,
    /// +0xa8: the sway's step, degrees an update (`120 / amplitude`).
    pub sway_step: f32,
    /// +0xac: the sway's amplitude, degrees.
    pub amplitude: f32,
    /// +0xb0: a torn rim's dangle amplitude.
    pub dangle_amplitude: f32,
    /// +0xb4: the amplitude the next sway starts at.
    pub kick: f32,
    /// +0xb8: what each swing leaves of the amplitude.
    pub decay: f32,
    /// +0xbc: a torn rim's dangle, degrees.
    pub dangle: f32,
    /// +0xc0: a torn rim has dropped.
    pub dropped: bool,
}

impl Default for Rim {
    /// `FUN_001490a0`.
    fn default() -> Self {
        Self {
            bend: false,
            sway: false,
            held: false,
            torn: false,
            swaying: false,
            dangles: false,
            bending: false,
            returning: false,
            bend_phase: 0.0,
            bend_angle: 0.0,
            sway_phase: 0.0,
            sway_angle: 0.0,
            dangle_step: 0.0,
            sway_step: 0.0,
            amplitude: 3.0,
            dangle_amplitude: 0.0,
            kick: 3.0,
            decay: f32::from_bits(0x3f19_999a),
            dangle: 0.0,
            dropped: false,
        }
    }
}

/// 359, where a swing's phase wraps (`0x43b38000`).
const WRAP: f32 = 359.0;

impl Rim {
    /// A sway of `kick` degrees (`FUN_0013e3b8`, `FUN_0013e470`: 3).
    pub fn sway(&mut self, kick: f32) {
        self.sway = true;
        self.kick = kick;
    }

    /// A dunker's hands on the rim (`held`, slot 42) or off it (slot 43,
    /// only while held): a sway of 3.75 either way (`FUN_0013e3e0`,
    /// `FUN_0013e428`).
    pub fn hang(&mut self, held: bool) {
        if held || self.held {
            self.sway = true;
            self.held = held;
            self.kick = 3.75;
        }
    }

    /// The rim torn off by a game's winning dunk (`0x1f6e58..0x1f6ea0`:
    /// +0x88, +0x80 and +0x70 set, the global `0x400054` bit 3 the users'
    /// counter 7 reads after the game): it bends down 4 an update to −90
    /// and dangles there.
    pub fn tear(&mut self) {
        self.dangles = true;
        self.torn = true;
        self.bend = true;
    }

    /// A ball off the rim (`FUN_0013e498`, `FUN_0013e4b0`).
    pub fn bend(&mut self) {
        self.bend = true;
    }

    /// One update (`FUN_001496c8`).
    pub fn update(&mut self) {
        if !self.swaying && self.sway {
            self.amplitude = self.kick;
            self.sway_angle = 0.0;
            self.sway_phase = 0.0;
            self.sway_step = 120.0 / self.kick;
            if 120.0 <= self.sway_step {
                self.sway = false;
            }
            // +0x7c, set at the hoop's construction (`0x148f00`) and never
            // cleared: the sway runs.
            self.swaying = true;
        }
        if !self.bending && self.bend {
            self.dangle_amplitude = 40.0;
            self.dangle_step = 3.0;
            self.bending = true;
            self.bend_phase = 0.0;
            self.returning = false;
        }
        if self.sway {
            let phase = self.sway_phase + self.sway_step;
            self.sway_phase = phase;
            if phase > 90.0 && self.held {
                self.sway_phase = 90.0;
            } else if self.sway_phase > WRAP {
                while self.sway_phase > WRAP {
                    self.sway_phase -= 360.0;
                    self.amplitude *= self.decay;
                    if self.amplitude <= f32::from_bits(0x3a83_126f) {
                        self.sway = false;
                        self.swaying = false;
                    }
                }
                self.sway_step = 120.0 / self.amplitude;
            }
            self.sway_angle = -self.amplitude * sin_degrees(self.sway_phase);
        }
        if self.bend {
            if self.bend_phase < 60.0 && !self.returning && !self.torn {
                self.bend_phase += 6.0;
                self.bend_angle = sin_degrees(self.bend_phase) * -6.0;
            } else if self.held {
                // Held where it is while a dunker hangs.
            } else if !self.torn {
                self.bend_phase -= 6.0;
                self.returning = true;
                self.bend_angle = sin_degrees(self.bend_phase) * -6.0;
            } else {
                self.bend_angle -= 4.0;
                if self.bend_angle <= -90.0 && self.dangles {
                    self.bend_phase += self.dangle_step;
                    self.bend_angle = -90.0;
                    self.dropped = true;
                    if self.bend_phase > WRAP {
                        while self.bend_phase > WRAP {
                            self.bend_phase -= 360.0;
                            self.dangle_amplitude =
                                if self.dangle_amplitude > f32::from_bits(0x38d1_b717) {
                                    self.dangle_amplitude * 0.5
                                } else {
                                    0.0
                                };
                        }
                        self.dangle_step = if self.dangle_amplitude < f32::from_bits(0x38d1_b717) {
                            120.0
                        } else {
                            120.0 / self.dangle_amplitude
                        };
                    }
                    self.dangle = -self.dangle_amplitude * sin_degrees(self.bend_phase);
                }
            }
            if self.returning && self.bend_phase <= 0.0 {
                self.bend_phase = 0.0;
                self.bend = false;
                self.bending = false;
                self.returning = false;
            }
        }
    }

    /// How bones 0..4 moved from the rest pose, in the hoop model's space:
    /// bones 2 and 3 turned by the sway and half of it while it runs, bone
    /// 4 by the bend and dangle while the rim bends (`0x14a028..0x14a0e4`).
    pub fn moves(&self, rig: &Rig) -> [Move; 5] {
        if !self.sway && !self.bend {
            return [Move::STILL; 5];
        }
        let turn = |r: [f32; 4], degrees: Option<f32>| match degrees {
            Some(d) => qmul(r, quat_z(d)),
            None => r,
        };
        let r = [
            rig.r[0],
            turn(rig.r[1], self.sway.then_some(self.sway_angle)),
            turn(rig.r[2], self.sway.then_some(self.sway_angle * 0.5)),
            turn(rig.r[3], self.bend.then_some(self.bend_angle + self.dangle)),
        ];
        let rest = rig.pose(rig.r);
        let now = rig.pose(r);
        std::array::from_fn(|k| Move {
            rot: qmul(now[k].0, conj(rest[k].0)),
            from: rest[k].1,
            to: now[k].1,
        })
    }

    /// Bone 4's move in the court's frame for hoop `side` (what a hanging
    /// dunker's hands and the net's top ring follow).
    pub fn rim_move(&self, rig: &Rig, side: usize) -> Move {
        self.moves(rig)[4].to_court(side)
    }
}

/// The world's two rims: index 0 the hoop at -x (game+0x178, the court's
/// `<court>d` with no turn), 1 the one at +x (+0x17c, `<court>b` turned
/// half round about y); `FUN_00157f20` takes the -x one for `x < 0`.
pub fn side(x: f32) -> usize {
    usize::from(x >= 0.0)
}

/// The rim the listeners pick for a post at `x`: +x for `0 < x`
/// (`0x141de4`, `0x141ed4`, the hang's message word `0x157328`).
pub fn post_side(x: f32) -> usize {
    usize::from(x > 0.0)
}

/// `sin` of `degrees` as the hoop's update takes it (`0x149878..0x1499c0`:
/// `2 degrees π / 360`, the quadrant `cvt.w.s` truncates, the EE's
/// polynomials).
fn sin_degrees(degrees: f32) -> f32 {
    sin_reduced((degrees + degrees) * f32::from_bits(0x4049_0fdb) / 360.0)
}

/// The reduction both sines share: the quadrant `q` (rounded half away by
/// ±0.5 and truncated), the remainder's polynomial, then `q & 1` takes the
/// cosine polynomial and `q & 2` the sign.
fn sin_reduced(x: f32) -> f32 {
    let k = x * f32::from_bits(0x3f22_f983);
    let k = if x < 0.0 { k - 0.5 } else { k + 0.5 };
    let q = k as i32;
    let r = x - q as f32 * f32::from_bits(0x3fc9_0fdb);
    let r2 = r * r;
    let s = sin_poly(r, r2);
    let c = ((((r2 * f32::from_bits(0x37d0_0d01) + f32::from_bits(0xbab6_0b61)) * r2
        + f32::from_bits(0x3d2a_aaab))
        * r2
        - 0.5)
        * r2)
        + 1.0;
    let v = if q & 1 != 0 { c } else { s };
    if q & 2 != 0 { -v } else { v }
}

fn sin_poly(r: f32, r2: f32) -> f32 {
    ((((r2 * f32::from_bits(0x3638_ef1d) + f32::from_bits(0xb950_0d01)) * r2
        + f32::from_bits(0x3c08_8889))
        * r2
        + f32::from_bits(0xbe2a_aaab))
        * r2)
        * r
        + r
}

/// `FUN_002f6fb0(0, 0, degrees)`: the quaternion of a turn about z.
fn quat_z(degrees: f32) -> [f32; 4] {
    quat_axis(2, degrees)
}

/// The quaternion of a turn about axis `axis` (x 0, y 1, z 2) as
/// `FUN_002f6fb0` and `FUN_002f6a28` build one: its half angle's sine of
/// the EE's polynomial (`π / 360`) and the cosine `√(1 − s²)`, by quadrant.
fn quat_axis(axis: usize, degrees: f32) -> [f32; 4] {
    let x = degrees * f32::from_bits(0x3c0e_fa35);
    let k = x * f32::from_bits(0x3f22_f983);
    let k = if x < 0.0 { k - 0.5 } else { k + 0.5 };
    let q = k as i32;
    let r = x - q as f32 * f32::from_bits(0x3fc9_0fdb);
    let s = sin_poly(r, r * r);
    let c = libm::sqrtf(1.0 - s * s);
    let (s, c) = match q & 3 {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    };
    let mut q = [0.0, 0.0, 0.0, c];
    q[axis] = s;
    q
}

/// `FUN_00157fe0` for a hanging dunker (state bit 14, `0x157e78..0x157eb0`):
/// each hand posed on the rim ([`crate::hands::RIM_HANG`]) turns to grip
/// the rim of the side his team attacks, where it gives: actor+0xb8's pose
/// (4, [`crate::World::hand_shapes`]' first) turns bone 15 (`LHand`, under
/// 14), +0xbc's (10) bone 19 (`RHand`, under 18).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grip {
    /// The rim's give ([`Rim::rim_move`], `FUN_00157f20`'s
    /// `FUN_00149150`; back the other way `FUN_00157f80`'s `FUN_00149410`).
    pub rim: Move,
    /// The centre's x (`FUN_001404e0`: side × 36.75).
    pub centre: f32,
    /// Which of [`GRIP_BONES`] grip.
    pub hands: [bool; 2],
}

/// The hand bones the grip turns and their parents, by actor+0xb8 then
/// +0xbc.
pub const GRIP_BONES: [(usize, usize); 2] = [(15, 14), (19, 18)];

impl crate::World {
    /// Player `i`'s grip on the rim this update, if he hangs with a hand
    /// posed on it (actor+0xb8 = 4, +0xbc = 10).
    pub fn rim_grip(&self, i: usize) -> Option<Grip> {
        let p = self.players.get(i)?;
        if !p.flight.hung {
            return None;
        }
        let (first, second) = self.hand_shapes(i);
        let hands = [
            first == crate::hands::RIM_HANG,
            second == crate::hands::RIM_HANG + crate::hands::LEFT,
        ];
        if hands == [false; 2] {
            return None;
        }
        let side = side(p.position[0]);
        Some(Grip {
            rim: self.rims[side].rim_move(&self.rim_rigs[side], side),
            centre: self.rim_spot(crate::target_hoop(p.team))[0],
            hands,
        })
    }
}

impl Grip {
    /// `0x1580a0..0x158798`: the hand at `hand` (its world position) under
    /// a parent whose world rotation is `parent`: its local rotation, facing
    /// round the rim from its centre and tipped toward the nearest point of
    /// its ring.
    ///
    /// The turn about y is the hand's angle about the centre (`x` from it,
    /// `z` from the court's line), in 1024ths of a turn (`× 162.97`, the
    /// `cvt.w.s` truncating `+ 0.5`). The tip about x is the angle of the
    /// ring's nearest point (0.7932 out from the centre at height 12, moved
    /// with the give) from the hand: across, the flat distance, negative
    /// with the hand outside the ring; down, the height it sits above the
    /// hand (at least 0), held to ±128. Bone 19 (`second`) turns half round
    /// more and tips the other way, both words masked to 0x3ff. Then the
    /// hand's world rotation is y · x · z(90°) and its local that over the
    /// parent's (`FUN_002f6a28`'s quaternions, `FUN_002f74b0`'s products).
    pub fn hand(&self, hand: [f32; 3], parent: [f32; 4], second: bool) -> [f32; 4] {
        let centre = self.rim.apply([self.centre, 12.0, 0.0]);
        let yaw = turn_word(grip_angle(hand[0] - centre[0], hand[2]));
        let local = self.rim.undo(hand);
        let (dx, dz) = (local[0] - self.centre, local[2]);
        let inv = 1.0 / libm::sqrtf((dx * dx + dz * dz).max(f32::MIN_POSITIVE));
        let radius = f32::from_bits(0x3f4b_10cb);
        let ring = self
            .rim
            .apply([dx * inv * radius + self.centre, 12.0, dz * inv * radius]);
        let flat = |p: [f32; 3], x: f32, z: f32| (p[0] - x) * (p[0] - x) + (p[2] - z) * (p[2] - z);
        let mut across = libm::sqrtf(flat(ring, hand[0], hand[2]));
        if flat(ring, centre[0], 0.0) < flat(hand, centre[0], 0.0) {
            across = -across;
        }
        let down = (ring[1] - hand[1]).max(0.0);
        let tip = turn_word(grip_angle(across, down)).clamp(-128, 128);
        let (yaw, tip) = if second {
            ((yaw + 512) & 0x3ff, -tip & 0x3ff)
        } else {
            (yaw, tip)
        };
        let degrees = |w: i32| w as f32 * 360.0 * f32::from_bits(0x3a80_0000);
        let world = qmul(
            quat_axis(1, degrees(yaw)),
            qmul(quat_axis(0, degrees(tip)), quat_axis(2, 90.0)),
        );
        qmul(conj(parent), world)
    }
}

/// An angle in 1024ths of a turn, rounded half up and truncated.
fn turn_word(angle: f32) -> i32 {
    (angle * f32::from_bits(0x4322_f983) + 0.5) as i32
}

/// The grip's angle of `n` across `d` (`0x1580a8..0x1582b0`): `atan2(n,
/// d)` by the arcsine's series in `1 / (1 + r²)` (`|r| > 1`) or `r² / (1 +
/// r²)` (`0x4b3120..0x4b312c`); `d` 0 gives ±π/2 by `n`'s sign.
fn grip_angle(n: f32, d: f32) -> f32 {
    let half = f32::from_bits(0x3fc9_0fdb);
    if d == 0.0 {
        return if n == 0.0 {
            0.0
        } else if n >= 0.0 {
            half
        } else {
            -half
        };
    }
    let series = |u: f32| {
        libm::sqrtf(u)
            * ((((u * f32::from_bits(0x3d61_37ab) + f32::from_bits(0x3d55_033a)) * u
                + f32::from_bits(0x3d8a_908b))
                * u
                + f32::from_bits(0x3e2b_ba25))
                * u
                + 1.0)
    };
    let r = n / d;
    let a = if r < -1.0 {
        series(1.0 / (r * r + 1.0)) - half
    } else if 1.0 < r {
        half - series(1.0 / (r * r + 1.0))
    } else {
        let r2 = r * r;
        let v = series(r2 / (r2 + 1.0));
        if r < 0.0 { -v } else { v }
    };
    if d < 0.0 {
        let pi = f32::from_bits(0x4049_0fdb);
        if 0.0 < n { a + pi } else { a - pi }
    } else {
        a
    }
}

fn conj(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

fn qmul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [x, y, z, w] = q;
    let t = qmul(qmul(q, [v[0], v[1], v[2], 0.0]), [-x, -y, -z, w]);
    [t[0], t[1], t[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grips_angle_is_the_arctangent() {
        for n in [-3.0f32, -1.0, -0.4, 0.0, 0.3, 1.0, 2.5] {
            for d in [-2.0f32, -0.5, 0.5, 1.0, 4.0] {
                let got = grip_angle(n, d);
                // Straight back (`n` 0, `d` below 0) is -π, not atan2's π.
                let want = if n == 0.0 && d < 0.0 {
                    -f32::from_bits(0x4049_0fdb)
                } else {
                    libm::atan2f(n, d)
                };
                assert!((got - want).abs() < 2e-4, "{n} {d}: {got} vs {want}");
            }
        }
        assert_eq!(grip_angle(1.0, 0.0), f32::from_bits(0x3fc9_0fdb));
        assert_eq!(grip_angle(0.0, 0.0), 0.0);
    }

    #[test]
    fn a_hand_under_the_rims_front_grips_it_facing_round() {
        // The +x rim, a hand right under its front (toward the court's
        // middle): turned a quarter round less the truncation's word (-255
        // of 1024), not tipped; the other hand half round more.
        let grip = Grip {
            rim: Move::STILL,
            centre: 36.75,
            hands: [true; 2],
        };
        let hand = [36.75 - f32::from_bits(0x3f4b_10cb), 11.0, 0.0];
        let one = |w: i32| w as f32 * 360.0 / 1024.0;
        let q = grip.hand(hand, [0.0, 0.0, 0.0, 1.0], false);
        let want = qmul(
            quat_axis(1, one(-255)),
            qmul(quat_axis(0, 0.0), quat_axis(2, 90.0)),
        );
        for k in 0..4 {
            assert!((q[k] - want[k]).abs() < 1e-6, "{q:?} vs {want:?}");
        }
        let q = grip.hand(hand, [0.0, 0.0, 0.0, 1.0], true);
        let want = qmul(
            quat_axis(1, one(257)),
            qmul(quat_axis(0, 0.0), quat_axis(2, 90.0)),
        );
        for k in 0..4 {
            assert!((q[k] - want[k]).abs() < 1e-6, "{q:?} vs {want:?}");
        }
        // Under a parent turned, the local rotation undoes its turn.
        let parent = quat_axis(1, 30.0);
        let local = grip.hand(hand, parent, false);
        let world = qmul(parent, local);
        let free = grip.hand(hand, [0.0, 0.0, 0.0, 1.0], false);
        for k in 0..4 {
            assert!((world[k] - free[k]).abs() < 1e-5);
        }
    }

    #[test]
    fn a_move_undone_puts_the_point_back() {
        let m = Move {
            rot: quat_axis(2, 7.0),
            from: [1.0, 2.0, 0.0],
            to: [1.1, 1.9, 0.0],
        };
        let p = [3.0, 12.0, 0.5];
        let back = m.undo(m.apply(p));
        for k in 0..3 {
            assert!((back[k] - p[k]).abs() < 1e-5);
        }
    }

    #[test]
    fn the_sines_are_the_ees() {
        for d in [
            -270.0f32, -95.0, -3.75, 0.0, 6.0, 30.0, 60.0, 90.0, 133.0, 250.0, 359.0,
        ] {
            let want = (d.to_radians()).sin();
            assert!((sin_degrees(d) - want).abs() < 2e-6, "{d}");
            let q = quat_z(d);
            assert!((q[2] - (d / 2.0).to_radians().sin()).abs() < 2e-6, "{d}");
            assert!((q[3] - (d / 2.0).to_radians().cos()).abs() < 2e-6, "{d}");
        }
    }

    #[test]
    fn a_torn_rim_falls_and_dangles() {
        let mut rim = Rim::default();
        rim.tear();
        // −4 an update from 0: past −90 by the 23rd, then held there.
        for _ in 0..23 {
            rim.update();
        }
        assert_eq!(rim.bend_angle, -90.0);
        assert!(rim.dropped);
        // The dangle swings 40 either way, halving each turn.
        let mut most: f32 = 0.0;
        for _ in 0..200 {
            rim.update();
            most = most.max(rim.dangle.abs());
        }
        assert!(most > 30.0 && most <= 40.0, "{most}");
        assert!(rim.dangle_amplitude < 40.0);
        // It never comes back up.
        assert_eq!(rim.bend_angle, -90.0);
        assert!(rim.bend);
    }

    #[test]
    fn the_rest_pose_is_the_rams() {
        // Save state 1's -x hoop (Forest's): bone 4 at (-37.592, 12.031) in
        // the court, as its RAM has it.
        let rig = Rig::forest();
        let pose = rig.pose(rig.r);
        let (at, _) = PLACES[0];
        let p = pose[4].1;
        assert!((at[0] + p[0] + 37.592).abs() < 0.03, "{p:?}");
        assert!((at[1] + p[1] - 12.031).abs() < 0.01, "{p:?}");
    }

    #[test]
    fn a_hang_holds_the_frame_bent_toward_the_baseline() {
        let rig = Rig::forest();
        let mut rim = Rim::default();
        rim.hang(true);
        for _ in 0..30 {
            rim.update();
        }
        // 120 / 3.75 = 32 degrees an update: at 90 by the third.
        assert_eq!(rim.sway_phase, 90.0);
        assert!((rim.sway_angle + 3.75).abs() < 1e-5);
        // The hands at the rim's front, 0.75 past its middle: the played
        // captures' hangs sit 0.69 further toward the baseline and 0.19
        // lower than the rim at rest gives.
        let hands = [-36.0, 12.0, 0.0];
        let to = rim.rim_move(&rig, 0).apply(hands);
        assert!((to[0] - hands[0] + 0.69).abs() < 0.05, "{to:?}");
        assert!((to[1] - hands[1] + 0.19).abs() < 0.05, "{to:?}");
        // The +x hoop mirrors it.
        let mirrored = rim.rim_move(&rig, 1).apply([36.0, 12.0, 0.0]);
        assert!((mirrored[0] - 36.0 - 0.69).abs() < 0.05, "{mirrored:?}");
        // Let go: it swings on and dies away.
        rim.hang(false);
        let mut n = 0;
        while rim.sway && n < 2000 {
            rim.update();
            n += 1;
        }
        assert!(!rim.sway && !rim.swaying && n < 2000);
        let back = rim.rim_move(&rig, 0).apply(hands);
        // Back to rest (the raw quaternions are a hair off unit, as the
        // game's are, so not to the last bit).
        assert!(
            (0..3).all(|k| (back[k] - hands[k]).abs() < 1e-3),
            "{back:?}"
        );
    }

    #[test]
    fn a_ball_off_the_rim_bends_it_ten_updates_and_ten_back() {
        let rig = Rig::rucker();
        let mut rim = Rim::default();
        rim.bend();
        let front = rig.pose(rig.r)[4].1;
        let front = [front[0] + 1.5, front[1], front[2]];
        let mut low: f32 = 0.0;
        let mut n = 0;
        loop {
            rim.update();
            n += 1;
            let m = rim.moves(&rig)[4];
            low = low.min(m.apply(front)[1] - front[1]);
            assert_eq!(m.to, m.from);
            if !rim.bend {
                break;
            }
        }
        assert_eq!(n, 20);
        // −6 sin 60 = −5.2 degrees about the rim's back: its front 1.5 out
        // drops 0.14.
        assert!(low < -0.12 && low > -0.16, "{low}");
    }
}
