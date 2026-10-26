//! The half-court players without the ball: "perim player"
//! (`FUN_001121e0`, update `FUN_00112288`), "post player" (`FUN_00114910`,
//! update `FUN_00114968`), the cuts they send each other on (the base
//! `FUN_0010fae8`: "cut to" `FUN_00111858`, "net cut in" `FUN_00110868`,
//! "circle cut in" `FUN_00110c48`, "cut out" `FUN_0010fd78`), "post cut"
//! (`FUN_00112b80`) and "post pause" (`FUN_001135a8`).
//!
//! Each "perim player" or "post player" makes one choice and is done; with
//! nothing queued the player's next frame picks another (`FUN_0017e5c8`).

use super::court::{angle_toward, dist, turn};
use super::offball::{Pause, PerimPause};
use super::pad::bits;
use super::words::words;
use super::{Act, Brain, Ctx};
use crate::input::{native_cos_1024, native_sin_1024};

/// `FUN_0013c098`: a zone mirrored across the court (0↔4, 1↔3, 5↔6).
fn mirror(zone: i32) -> i32 {
    if zone < 5 {
        4 - zone
    } else if zone == 5 {
        6
    } else {
        5
    }
}

words! {
    /// "perim player": by the zones the player, the ball's owner (where he
    /// is headed, for the play's version) and the third man are in (`0x3c94a0`,
    /// or `0x3c96f0` for a play's), mirrored so the player is on the
    /// near side, one of up to three moves (`0x3c97b8`'s weights, one
    /// draw), each a zone and a class whose kind is one of four (`0x3c97e8`,
    /// a second draw): a perim pause, a cut to the zone, a cut in to it
    /// (around the rim when one zone away), or a cut out to it. Two in one
    /// zone: the one nearer the baseline, or the rim, gives way
    /// (`FUN_00112770`).
    pub(crate) struct PerimPlayer {
        play: bool,
        done: bool,
    }
}

impl PerimPlayer {
    pub fn new(play: bool) -> Self {
        Self { play, done: false }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// `FUN_00112248`.
    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let Some(owner) = ctx.ball_owner() else {
            self.done = true;
            return;
        };
        let mut me = ctx.zone(i);
        let ahead = self.play && ctx.has_ball(owner);
        let mut own = if ahead {
            ctx.zone_at(owner, ctx.heading_for(owner))
        } else {
            ctx.zone(owner)
        };
        let Some(third) = ctx.third_man(i) else {
            return;
        };
        let mut third = ctx.zone_at(third, ctx.heading_for(third));
        if me == own {
            give_way(ctx, i, owner, &mut me, &mut own);
        }
        let flipped = matches!(me, 3 | 4 | 6);
        if flipped {
            me = mirror(me);
            own = mirror(own);
            third = mirror(third);
        }
        if me == 5 {
            me = 3;
        }
        let (me, own, third) = (me as usize, own as usize, third as usize);
        let row: &[u8] = if self.play {
            let k = me * 49 + own * 7 + third;
            &MOVES1[k.min(MOVES1.len() - 1)..(k + 1).min(MOVES1.len())]
        } else {
            let k = me * 147 + own * 21 + third * 3;
            &MOVES3[k.min(MOVES3.len() - 3)..(k + 3).min(MOVES3.len())]
        };
        let count = row.iter().take_while(|&&v| v != 0xff).count();
        let r = ctx.rand();
        let pick = PICKS[count * 16 + (r & 0xf) as usize] as usize;
        let entry = row.get(pick).copied().unwrap_or(0xff);
        let mut zone = i32::from(entry & 0xf);
        if flipped {
            zone = mirror(zone);
        }
        let r = ctx.rand();
        let mut kind = KINDS[usize::from(entry >> 4) * 4 + (r & 3) as usize];
        if ctx.zone(i) == zone {
            kind = 0x10;
        }
        match kind {
            0x20 => {
                if 33.75 < ctx.hoop_dist(i) {
                    let z = ctx.zone(i);
                    let c = CutMove::new(ctx, i, CutKind::To, z, false, false);
                    b.push(Act::CutMove(c));
                } else {
                    let r = ctx.rand();
                    let frames = if self.play {
                        (r % 24) as i32 + 60
                    } else {
                        (r % 18) as i32 + 24
                    };
                    b.push(Act::PerimPause(PerimPause::new(frames)));
                }
            }
            0x10 => {
                let c = CutMove::new(ctx, i, CutKind::To, zone, false, false);
                b.push(Act::CutMove(c));
            }
            0x40 => {
                let kind = if 1 < (ctx.zone(i) - zone).abs() {
                    CutKind::NetIn
                } else {
                    CutKind::CircleIn
                };
                let c = CutMove::new(ctx, i, kind, zone, false, false);
                b.push(Act::CutMove(c));
            }
            0x80 => {
                let c = CutMove::new(ctx, i, CutKind::Out, zone, false, false);
                b.push(Act::CutMove(c));
            }
            _ => {}
        }
        self.done = true;
    }
}

/// `FUN_00112770`: two in one zone. On the perimeter, the one nearer the
/// baseline side (by the angle about the rim) moves a zone up; in the
/// paint, the one farther from the rim takes his sector.
fn give_way(ctx: &Ctx, i: usize, owner: usize, me: &mut i32, own: &mut i32) {
    if *me < 5 {
        let d = turn(ctx.hoop_angle(i), ctx.hoop_angle(owner));
        if *me == 4 {
            if d < 0 {
                *me = 3;
            } else {
                *own = 3;
            }
        } else if 0 <= d {
            *me += 1;
        } else {
            *own += 1;
        }
    } else if ctx.hoop_dist(owner) < ctx.hoop_dist(i) {
        *me = ctx.sector(i);
    } else {
        *own = ctx.sector(owner);
    }
}

words! {
    /// "post player": within 10 of the ball's owner or in his zone, a
    /// spot by the angle to him (`FUN_00114e60`, `0x3c9d48`/`0x3c9d68`);
    /// else, by the zones and sides of the owner and the third man, a post
    /// cut to a block (deep or not, one draw where the side is open) or a
    /// perimeter player's move.
    pub(crate) struct PostPlayer {
        done: bool,
    }
}

impl PostPlayer {
    pub fn done(&self) -> bool {
        self.done
    }

    /// `FUN_00114968`.
    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        b.stop(ctx, i);
        let Some(owner) = ctx.ball_owner() else {
            self.done = true;
            return;
        };
        let open = super::offence::path_open(ctx, i);
        let Some(third) = ctx.third_man(i) else {
            return;
        };
        let me = ctx.zone(i);
        let own = ctx.zone_at(owner, ctx.heading_for(owner));
        let other = ctx.zone_at(third, ctx.heading_for(third));
        if ctx.between(i, owner) <= 10.0 || me == own {
            self.done = self.near(ctx, b, i);
            return;
        }
        let side = |p: [f32; 2]| if 0.0 <= p[1] { 1 } else { -1 };
        let attack = ctx.side(ctx.team(i)) as f32;
        let act = if me < 5 {
            let mut perim = false;
            if own == me {
                perim = true;
            } else {
                let third_out = other < 5;
                let go = |perim: &mut bool| {
                    // `0x114a74`.
                    if open {
                        let sm = side(ctx.pos(i));
                        if side(ctx.heading_for(owner)) != sm && sm != side(ctx.heading_for(third))
                        {
                            return;
                        }
                    }
                    if third_out {
                        let s = if own == 5 { -1.0 } else { 1.0 };
                        if attack * s * ctx.pos(i)[1] <= 0.0 {
                            return;
                        }
                    }
                    *perim = true;
                };
                if own < 5 {
                    if !third_out {
                        go(&mut perim);
                    }
                } else if third_out {
                    go(&mut perim);
                } else {
                    perim = true;
                }
            }
            if perim {
                Act::PerimPlayer(PerimPlayer::new(false))
            } else {
                let mut deep = if own < 5 {
                    own == 1 || own == 3
                } else {
                    36.75 - ctx.pos(owner)[0].abs() >= 8.0
                };
                if open {
                    deep = true;
                }
                let block = if attack * ctx.pos(i)[1] <= 0.0 { 5 } else { 6 };
                Act::PostCut(PostCut::new(ctx, i, block, deep, false))
            }
        } else {
            let mine = side(ctx.pos(i));
            if side(ctx.heading_for(owner)) == mine {
                // `0x114db0`.
                let (block, deep) = if !open || me == other {
                    let r = ctx.rand();
                    (if me == 5 { 6 } else { 5 }, r & 1 != 0)
                } else {
                    (me, true)
                };
                if block != other {
                    b.push(Act::PostCut(PostCut::new(ctx, i, block, deep, false)));
                    self.done = true;
                } else {
                    self.done = self.near(ctx, b, i);
                }
                return;
            }
            if side(ctx.heading_for(third)) != side(ctx.heading_for(owner)) && own < 5 {
                let deep = own == 1 || own == 3;
                let block = if me != 5 { 5 } else { 6 };
                Act::PostCut(PostCut::new(ctx, i, block, deep, false))
            } else if ctx.rand() & 1 == 0 {
                Act::PerimPlayer(PerimPlayer::new(false))
            } else {
                let deep = 36.75 - ctx.pos(i)[0].abs() >= 8.0;
                Act::PostCut(PostCut::new(ctx, i, me, deep, false))
            }
        };
        b.push(act);
        self.done = true;
    }

    /// `FUN_00114e60`: from the paint, the spot for the angle to the
    /// owner in eighths (`0x3c9d68` by depth; on the other block mirrored
    /// by `0x3c9d48`), the second choice when the third man holds the
    /// first; a perimeter spot is a cut to it, a paint spot a post cut.
    /// False (not done) with no owner or third man.
    fn near(&self, ctx: &mut Ctx, b: &mut Brain, i: usize) -> bool {
        let me = ctx.zone(i);
        if me < 5 {
            b.push(Act::PerimPlayer(PerimPlayer::new(false)));
            return true;
        }
        let (Some(owner), Some(third)) = (ctx.ball_owner(), ctx.third_man(i)) else {
            return false;
        };
        let a = ctx.angle_between(i, owner);
        let mut eighth = (((a + 0x40) & 0x3ff) >> 7) as usize;
        if ctx.side(ctx.team(i)) < 0 {
            eighth = (eighth + 4) & 7;
        }
        let near = me == 6;
        if near {
            eighth = NEAR_EIGHTH[eighth];
        }
        let deep = usize::from(36.75 - ctx.pos(i)[0].abs() < 8.0);
        let mut k = deep * 32 + eighth * 4;
        let zone = |v: i32| if near { mirror(v) } else { v };
        if zone(NEAR_SPOTS[k]) == ctx.zone_at(third, ctx.heading_for(third)) {
            k += 2;
        }
        let z = zone(NEAR_SPOTS[k]);
        if NEAR_SPOTS[k] < 5 {
            b.push(Act::CutMove(CutMove::new(
                ctx,
                i,
                CutKind::To,
                z,
                false,
                false,
            )));
        } else {
            let deep = NEAR_SPOTS[k + 1] != 0;
            b.push(Act::PostCut(PostCut::new(ctx, i, z, deep, false)));
        }
        true
    }
}

/// `0x3c9d48`: the eighths mirrored for the far block.
const NEAR_EIGHTH: [usize; 8] = [4, 3, 2, 1, 0, 7, 6, 5];
/// `0x3c9d68`: by depth and eighth, two `{zone, deep}` choices.
const NEAR_SPOTS: [i32; 64] = [
    6, 1, 2, 0, 6, 1, 0, 0, 6, 1, 0, 0, 0, 0, 5, 1, 1, 0, 0, 0, 1, 0, 2, 0, 2, 0, 1, 0, 2, 0, 1, 0,
    6, 1, 2, 0, 6, 1, 0, 0, 6, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 2, 0, 2, 0, 1, 0, 2, 0, 1, 0,
];

/// The cuts (`FUN_0010fae8`'s subclasses).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum CutKind {
    /// "cut to" (`FUN_00111858`, update `FUN_00111970`): straight there.
    #[default]
    To,
    /// "net cut in" (`FUN_00110868`, update `FUN_001109a8`): from outside
    /// the paint, first to within 5 of the rim, then out to the spot.
    NetIn,
    /// "circle cut in" (`FUN_00110c48`, update `FUN_00111068`): round the
    /// middle of the way there.
    CircleIn,
    /// "cut out" (`FUN_0010fd78`, update `FUN_0010fee0`): round the rim
    /// at the spot's distance.
    Out,
}

impl super::words::Word for CutKind {
    fn put(&self, out: &mut Vec<u32>) {
        out.push(*self as u32);
    }
    fn get(r: &mut super::words::Reader) -> Result<Self, String> {
        Ok(match r.word()? {
            0 => CutKind::To,
            1 => CutKind::NetIn,
            2 => CutKind::CircleIn,
            3 => CutKind::Out,
            k => return Err(format!("bad AI cut {k}")),
        })
    }
}

words! {
    /// A cut (`FUN_0010fae8(ai, zone, long, again)`): the spot from
    /// `FUN_00180f80` in the zone. Its done test (`FUN_0010fb78`): once
    /// over `fps × 0.7` frames old and closing on the ball's holder
    /// (`FUN_0017eed8`), a pause of `r % (fps × 0.2) + fps × 0.4` frames
    /// (and, when `again`, the cut once more); once there, a perim pause
    /// of `r % fps + fps × 1.9` frames; and done with the ball.
    pub(crate) struct CutMove {
        kind: CutKind,
        zone: i32,
        long: bool,
        again: bool,
        spot: [f32; 2],
        there: bool,
        /// The net cut's first leg is done; the circle cut's arc is.
        stage: bool,
        /// The cut out's way round (±1) and radius; the circle cut's
        /// middle, start angle, way round, radius and last angle.
        dir: i32,
        radius: f32,
        mid: [f32; 2],
        start: i32,
        last: i32,
    }
}

/// The AI's `+0xb4` (7, set by the AI player's constructor at
/// `0x17da14`): how far from the rim the near-rim pass targets stand.
pub(crate) const NEAR_RIM: f32 = 7.0;

/// `FUN_0010aea8` (the fast break's pass target, and the roll's
/// `FUN_001180d8`): the point `min(7, his distance)` from the rim on its
/// line to him (game+0x35c's angle).
pub(crate) fn near_rim(pos: [f32; 2], rim: [f32; 2]) -> [f32; 2] {
    let d = dist(pos, rim).min(NEAR_RIM);
    let a = angle_toward(rim, pos);
    [
        rim[0] - native_sin_1024(a) * d,
        rim[1] - native_cos_1024(a) * d,
    ]
}

impl CutMove {
    /// The pass target (vt+0x24): the spot, but `FUN_00110ae8` for the net
    /// cut (on its way in the point near the rim on his line, [`near_rim`];
    /// on its way out the spot, or 10 from the rim toward it once he is
    /// within 10) and `FUN_00111720` for the circle cut (on its arc while
    /// its last heading stays within 0x100 of the line to the rim: the
    /// point near the rim within 0x80, else his place).
    pub fn pass_target(&self, pos: [f32; 2], rim: [f32; 2]) -> [f32; 2] {
        match self.kind {
            CutKind::NetIn if !self.stage => near_rim(pos, rim),
            CutKind::NetIn => {
                if 10.0 < dist(pos, rim) {
                    self.spot
                } else {
                    let (dx, dz) = (self.spot[0] - rim[0], self.spot[1] - rim[1]);
                    let n = crate::math::len(dx, dz);
                    if n == 0.0 {
                        rim
                    } else {
                        [rim[0] + dx / n * 10.0, rim[1] + dz / n * 10.0]
                    }
                }
            }
            CutKind::CircleIn => {
                let off = turn(self.last, angle_toward(pos, rim)).abs();
                if !self.stage && off <= 0x100 {
                    if off < 0x80 { near_rim(pos, rim) } else { pos }
                } else {
                    self.spot
                }
            }
            _ => self.spot,
        }
    }

    pub fn new(ctx: &mut Ctx, i: usize, kind: CutKind, zone: i32, long: bool, again: bool) -> Self {
        let spot = super::offence::spot_in(ctx, i, zone, long);
        let mut c = Self {
            kind,
            zone,
            long,
            again,
            spot,
            ..Default::default()
        };
        match kind {
            CutKind::To => {}
            // `0x1108a0`: inside the paint already, straight out.
            CutKind::NetIn => c.stage = 5 <= ctx.zone(i),
            CutKind::Out => {
                c.dir = round_way(ctx, i, spot).1;
                c.radius = crate::math::len(spot[0].abs() - 36.75, spot[1]);
            }
            CutKind::CircleIn => {
                let p = ctx.pos(i);
                c.mid = [(spot[0] + p[0]) * 0.5, (spot[1] + p[1]) * 0.5];
                c.start = angle_toward(c.mid, p);
                let rim = ctx.goal(ctx.team(i));
                let a = angle_toward(rim, c.mid);
                c.dir = if 0 <= turn(ctx.hoop_angle(i), a) {
                    1
                } else {
                    -1
                };
                c.radius = dist(c.mid, p);
                c.last = ctx.angle_to_hoop(i);
            }
        }
        c
    }

    pub fn spot(&self) -> [f32; 2] {
        self.spot
    }

    /// `FUN_0010fb78`.
    pub fn done(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize, age: u16) -> bool {
        if !self.there {
            if 42 < i32::from(age) && ctx.closing_on_ball(i) {
                let r = ctx.rand();
                b.push(Act::Pause(Pause::new((r % 12) as i32 + 24)));
                if self.again {
                    let c = CutMove::new(ctx, i, CutKind::To, self.zone, self.long, self.again);
                    b.push(Act::CutMove(c));
                }
                self.there = true;
            }
        } else {
            let r = ctx.rand();
            b.push(Act::PerimPause(PerimPause::new((r % 60) as i32 + 114)));
        }
        if ctx.has_ball(i) {
            self.there = true;
        }
        self.there
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        match self.kind {
            CutKind::To => self.straight(ctx, b, i),
            CutKind::NetIn => {
                if !self.stage {
                    let side = ctx.side(ctx.team(i)) as f32;
                    let p = ctx.pos(i);
                    let x = 36.75 - side * p[0];
                    if 25.0 < x * x + p[1] * p[1] {
                        b.move_to(ctx, i, ctx.goal(ctx.team(i)), 3, 1);
                        b.sprint(ctx, i);
                    } else {
                        self.stage = true;
                    }
                } else {
                    self.straight(ctx, b, i);
                }
            }
            CutKind::CircleIn => self.circle(ctx, b, i),
            CutKind::Out => self.round(ctx, b, i),
        }
    }

    /// `FUN_00111970`: to the spot, sprinting beyond 5; there within 1.
    fn straight(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) {
        let d = dist(ctx.pos(i), self.spot);
        if d < 1.0 {
            self.there = true;
            return;
        }
        let level = if 5.0 < d {
            b.sprint(ctx, i);
            3
        } else {
            2
        };
        b.move_to(ctx, i, self.spot, level, 1);
    }

    /// `FUN_00111068`: while the angle round the middle has turned less
    /// than 299, run 28 units ahead on the circle (a little wider at the
    /// half way), sprinting; then straight to the spot.
    fn circle(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) {
        let a = angle_toward(self.mid, ctx.pos(i));
        let d = turn(self.start, a).abs();
        if !self.stage && d < 299 {
            let e = (0x100 - d).abs();
            let r = self.radius + (0x100 - e) as f32 * 5.0 / 256.0;
            let ang = self.dir * 28 + a;
            let to = [
                self.mid[0] - r * native_sin_1024(ang),
                self.mid[1] - r * native_cos_1024(ang),
            ];
            let heading = angle_toward(ctx.pos(i), to);
            b.steer(ctx, i, heading, 3, 1, None);
            b.sprint(ctx, i);
            self.last = heading;
            return;
        }
        self.stage = true;
        let d = dist(ctx.pos(i), self.spot);
        if 1.0 < d {
            let level = if 5.0 < d {
                b.sprint(ctx, i);
                3
            } else {
                2
            };
            b.move_to(ctx, i, self.spot, level, 1);
        } else {
            self.there = true;
        }
    }

    /// `FUN_0010fee0`: round the rim, 14 units ahead of the player's
    /// angle at the spot's distance (kept inside 25.5 of the sideline from
    /// 22 out); past the spot's angle he is there. At the sideline with
    /// the arc heading out, he turns for the middle.
    fn round(&mut self, ctx: &Ctx, b: &mut Brain, i: usize) {
        let (a, dir) = round_way(ctx, i, self.spot);
        if dir != self.dir {
            self.there = true;
            return;
        }
        let ang = a + self.dir * 14;
        let mut r = self.radius;
        if 22.0 <= r {
            let rr = r.min(25.5);
            if 0x1b4 < (ang - 0x126) as u32 {
                r = rr / native_sin_1024(ang).abs();
            }
        }
        let mut t = [r * native_cos_1024(ang) + 36.75, r * native_sin_1024(ang)];
        let p = ctx.pos(i);
        if 25.5 <= p[1].abs() {
            let out = if 0.0 <= p[1] { t[1] } else { -t[1] };
            if 0.0 < out {
                t = [if 0.0 <= t[0] { 1.0 } else { -1.0 }, 0.0];
            }
        }
        let side = ctx.side(ctx.team(i)) as f32;
        let t = [t[0] * side, t[1] * side];
        let heading = angle_toward(p, t);
        let level = if 5.0 < dist(p, self.spot) {
            b.sprint(ctx, i);
            3
        } else {
            2
        };
        b.steer(ctx, i, heading, level, 1, None);
    }
}

/// `FUN_00110350`: the player's angle round the rim (from +x toward +z,
/// in the court turned to the +x rim) and which way round the spot lies
/// (+1 when the spot's angle is the larger).
fn round_way(ctx: &Ctx, i: usize, spot: [f32; 2]) -> (i32, i32) {
    let side = ctx.side(ctx.team(i)) as f32;
    let angle = |p: [f32; 2]| {
        let (x, z) = (p[0] * side - 36.75, p[1] * side);
        super::court::units(crate::math::atan2(z, x)) & 0x3ff
    };
    let at = angle(ctx.pos(i));
    let to = angle(spot);
    (at, if at < to { 1 } else { -1 })
}

words! {
    /// "post cut" (`FUN_00112b80(ai, zone, deep, again)`, update
    /// `FUN_00112ca0`, done `FUN_00112c50`): to a low-post spot
    /// (`FUN_001812a8`), at speed level 3 (1 within 3), then "post
    /// pause". Over `fps × 0.7` frames old and closing on the ball, a
    /// pause and (with `again`) the cut again, or from the perimeter a
    /// pause, from the paint a post pause.
    pub(crate) struct PostCut {
        zone: i32,
        deep: bool,
        again: bool,
        spot: [f32; 2],
        level: i32,
        done: bool,
    }
}

impl PostCut {
    pub fn new(ctx: &mut Ctx, i: usize, zone: i32, deep: bool, again: bool) -> Self {
        let spot = super::offence::paint_spot(ctx, i, zone, deep);
        let level = if dist(ctx.pos(i), spot) < 3.0 { 1 } else { 3 };
        Self {
            zone,
            deep,
            again,
            spot,
            level,
            done: false,
        }
    }

    pub fn spot(&self) -> [f32; 2] {
        self.spot
    }

    pub fn done(&self, ctx: &Ctx, i: usize) -> bool {
        self.done || ctx.has_ball(i)
    }

    /// `FUN_00112f48`, the pass target: from 7 out (`+0xb4`) with the spot
    /// within 200 units of his line to the rim, the point 7 from the rim
    /// on its line to him; else his place.
    pub fn pass_target(&self, pos: [f32; 2], rim: [f32; 2]) -> [f32; 2] {
        if NEAR_RIM <= dist(pos, rim)
            && turn(angle_toward(pos, self.spot), angle_toward(pos, rim)).abs() < 200
        {
            let a = angle_toward(rim, pos);
            return [
                rim[0] - native_sin_1024(a) * NEAR_RIM,
                rim[1] - native_cos_1024(a) * NEAR_RIM,
            ];
        }
        pos
    }

    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        if dist(ctx.pos(i), self.spot) < 1.5 {
            self.done = true;
            let p = PostPause::new(ctx);
            b.push(Act::PostPause(p));
        } else {
            b.move_to(ctx, i, self.spot, self.level as usize, 1);
            if self.level == 3 {
                b.sprint(ctx, i);
            }
        }
        if i32::from(b.age) <= 42 || !ctx.closing_on_ball(i) {
            return;
        }
        if !self.again {
            if ctx.zone(i) < 5 {
                let r = ctx.rand();
                b.push(Act::Pause(Pause::new((r % 12) as i32 + 24)));
            } else {
                let p = PostPause::new(ctx);
                b.push(Act::PostPause(p));
            }
        } else {
            let r = ctx.rand();
            b.push(Act::Pause(Pause::new((r % 12) as i32 + 24)));
            let c = PostCut::new(ctx, i, self.zone, self.deep, self.again);
            b.push(Act::PostCut(c));
        }
        self.done = true;
    }
}

words! {
    /// "post pause" (`FUN_001135a8`, update `FUN_00113768`, done
    /// `FUN_00113688`): stand on the block facing the ball for `r % (fps ×
    /// 2.2) + fps × 3.7` frames, asking for a lob (0x800000, the post call
    /// [`crate::World::post_call`]) while one is on (`FUN_00113b38`). When
    /// the owner has kept to the other side's wing or corner for `fps ×
    /// 1.1` frames he moves over to that side's block (a post cut, unless
    /// the third man is headed there). The pick calls (`FUN_00114050`,
    /// `FUN_00114480`: "run pick play" and "set pick") need the sim's
    /// screens, which it does not have; they are not ported. Done after
    /// the frames, or with the third man in the paint (for a man not the
    /// post man), the owner within 10 or gone, or in another zone than his.
    pub(crate) struct PostPause {
        frames: i32,
        done: bool,
        /// `[9]`: the set-pick window is still open (`FUN_00113f38`).
        early: bool,
        /// `[0x10]`, `[0xc]`: frames the owner has stood on the far side,
        /// and where.
        same: i32,
        at: [f32; 2],
        /// `[0x11]`: frames the lob call holds.
        hold: i32,
    }
}

impl PostPause {
    pub fn new(ctx: &mut Ctx) -> Self {
        let r = ctx.rand();
        Self {
            frames: (r % 132) as i32 + 222,
            early: true,
            ..Default::default()
        }
    }

    /// `FUN_00113688`.
    pub fn done(&self, ctx: &Ctx, b: &Brain, i: usize, age: u16) -> bool {
        if self.done || self.frames <= i32::from(age) {
            return true;
        }
        if b.role != 2
            && let Some(t) = ctx.third_man(i)
            && 4 < ctx.zone_at(t, ctx.heading_for(t))
        {
            return true;
        }
        let Some(o) = ctx.ball_owner() else {
            return true;
        };
        ctx.between(i, o) <= 10.0 || ctx.zone(o) == ctx.zone(i)
    }

    /// `FUN_00113768`.
    pub fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        let owner = ctx.ball_owner();
        let facing = match owner {
            Some(o) => ctx.angle_between(i, o),
            None => ctx.facing(i),
        };
        if !b.face(ctx, i, facing, 1, 0x80) {
            b.stop(ctx, i);
            if lob_ready(ctx, i, self.hold) {
                b.press(bits::POST_CALL);
                // Not posting yet (`FUN_001816f0`, actor bit 11) the call
                // holds `DAT_004b4920` (fps) frames; posting, it runs down.
                if !ctx.posting(i) {
                    self.hold = 60;
                } else if 0 < self.hold {
                    self.hold -= 1;
                }
            } else {
                self.hold = 0;
            }
        }
        // `FUN_00113f38`.
        if self.early && 132 <= i32::from(b.age) {
            match ctx.w.handler() {
                None => self.early = false,
                Some(h) => {
                    let same = (0.0 <= ctx.pos(h)[1]) == (0.0 <= ctx.pos(i)[1]);
                    let z = ctx.zone(h);
                    if !same || 4 < z || z == 2 {
                        self.early = false;
                    }
                }
            }
        }
        match owner {
            None => self.same = 0,
            Some(o) => {
                let same_side = (0.0 <= ctx.pos(o)[1]) == (0.0 <= ctx.pos(i)[1]);
                let z = ctx.zone(o);
                if same_side || z == 2 || 4 < z {
                    self.same = 0;
                } else if self.same == 0 {
                    self.same = 1;
                    self.at = ctx.pos(o);
                } else if dist(ctx.pos(o), self.at) < 5.0 {
                    self.same += 1;
                } else {
                    self.same = 0;
                }
            }
        }
        if 66 <= self.same
            && let Some(o) = owner
            && ctx.zone(o) < 5
        {
            let z = ctx.zone(o);
            let attack = ctx.side(ctx.team(i)) as f32;
            let block = if z < 2 || (z == 2 && attack * ctx.pos(o)[1] < 0.0) {
                5
            } else {
                6
            };
            let held = ctx
                .third_man(i)
                .is_some_and(|t| ctx.zone_at(t, ctx.heading_for(t)) == block);
            if !held {
                let deep = z == 1 || z == 3;
                let c = PostCut::new(ctx, i, block, deep, false);
                b.push(Act::PostCut(c));
            }
            self.done = true;
            return;
        }
        // `0x113ac8`: on a person's side an open lane shortens the wait.
        let deep = 36.75 - ctx.pos(i)[0].abs() < 8.0;
        if !deep && ctx.human[ctx.team(i)] && super::offence::path_open(ctx, i) {
            self.frames -= 1;
        }
    }
}

/// `FUN_00113b38`: a lob is on. While the call holds, an opponent within
/// 6.5 (no more than 1 farther from the rim) keeps it on, as does a call
/// under `fps × 0.8` frames old. Else: no teammate within 12 (nor one
/// but the holder posting, actor bit 11), the holder 18.75..35.75 out
/// and on the same side or within 3 of the middle, the post man facing
/// him (within 0x72) with no lane to the rim and a defender within 5.5 no
/// farther from the rim; off the baseline the holder within 27.75 of
/// the middle in x, on it the holder's angle about the rim over 341 or
/// under 0xab.
fn lob_ready(ctx: &Ctx, i: usize, hold: i32) -> bool {
    let Some(h) = ctx.w.handler() else {
        return false;
    };
    if 0 < hold && (defended(ctx, i, 6.5, 1.0) || 48 < hold) {
        return true;
    }
    let team = ctx.team(i);
    if ctx
        .squad(team)
        .into_iter()
        .any(|j| j != i && (ctx.between(i, j) < 12.0 || (j != h && ctx.posting(j))))
    {
        return false;
    }
    let d = ctx.hoop_dist(h);
    if !(18.75 < d && d < 35.75) {
        return false;
    }
    let same = (0.0 <= ctx.pos(h)[1]) == (0.0 <= ctx.pos(i)[1]);
    if !same && 3.0 < ctx.pos(h)[1].abs() {
        return false;
    }
    if 0x72 <= turn(ctx.angle_between(i, h), ctx.facing(i)).abs() {
        return false;
    }
    if super::offence::path_open(ctx, i) || !defended(ctx, i, 5.5, 0.0) {
        return false;
    }
    if 36.75 - ctx.pos(i)[0].abs() >= 8.0 {
        ctx.pos(h)[0].abs() <= 27.75
    } else {
        let mut a = ctx.hoop_angle(h);
        if ctx.side(ctx.team(h)) < 0 {
            a = 0x400 - a;
        }
        !(0xab..=341).contains(&a)
    }
}

/// `FUN_00113e60(r, slack)`: an opponent within `r` no more than `slack`
/// farther from the rim.
fn defended(ctx: &Ctx, i: usize, r: f32, slack: f32) -> bool {
    let d = ctx.hoop_dist(i);
    ctx.squad(1 - ctx.team(i))
        .into_iter()
        .any(|o| ctx.between(i, o) < r && ctx.hoop_dist(o) < d + slack)
}

const MOVES3: [u8; 588] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x43, 0x44, 0xff, 0x63, 0x64, 0xff, 0x64, 0x63, 0xff, 0x64, 0xff,
    0xff, 0x63, 0xff, 0xff, 0x63, 0x64, 0xff, 0x63, 0x64, 0xff, 0x81, 0x61, 0x44, 0x64, 0xff, 0xff,
    0x64, 0xe1, 0xff, 0xc1, 0x64, 0xff, 0xe1, 0x63, 0xff, 0xe1, 0x64, 0x63, 0xe1, 0x64, 0x63, 0x81,
    0x82, 0xff, 0x62, 0xff, 0xff, 0x81, 0x61, 0xff, 0x81, 0x41, 0xa2, 0x81, 0x41, 0xa2, 0xc1, 0xc2,
    0xff, 0xc1, 0x82, 0xff, 0xc2, 0x81, 0xff, 0x62, 0xff, 0xff, 0xc1, 0xff, 0xff, 0xc1, 0xc2, 0xff,
    0xc1, 0xc2, 0xff, 0xc2, 0x81, 0xff, 0xc2, 0x81, 0xff, 0x82, 0xff, 0xff, 0x82, 0xff, 0xff, 0x81,
    0xff, 0xff, 0x82, 0xff, 0xff, 0x82, 0xff, 0xff, 0x82, 0xff, 0xff, 0x82, 0xff, 0xff, 0x81, 0x82,
    0xff, 0xa2, 0xff, 0xff, 0xa1, 0xff, 0xff, 0x81, 0xa2, 0xff, 0x81, 0xa2, 0xff, 0x81, 0x82, 0xff,
    0x81, 0xa2, 0xff, 0x82, 0x83, 0xff, 0x82, 0x83, 0xff, 0xc3, 0xff, 0xff, 0xc2, 0xff, 0xff, 0xc2,
    0x83, 0xff, 0x82, 0x83, 0xff, 0x82, 0x83, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x64, 0xff, 0xff,
    0x80, 0xff, 0xff, 0x80, 0xff, 0xff, 0x80, 0xff, 0xff, 0x80, 0xff, 0xff, 0xa0, 0x44, 0x63, 0x80,
    0x63, 0xff, 0xe2, 0x64, 0xff, 0x82, 0xa0, 0xff, 0xe0, 0x20, 0xff, 0xe0, 0x20, 0xff, 0xe0, 0xa2,
    0x20, 0xe2, 0x64, 0x20, 0xa2, 0x20, 0xff, 0xc2, 0xe2, 0xe3, 0x82, 0xa0, 0xa3, 0xa0, 0xa3, 0x20,
    0xa0, 0xa2, 0x20, 0xe2, 0x80, 0x20, 0xe2, 0xa3, 0xff, 0xe2, 0x20, 0xff, 0x83, 0x82, 0xff, 0x83,
    0x82, 0xff, 0x83, 0xff, 0xff, 0x82, 0xff, 0xff, 0x83, 0x82, 0xff, 0x83, 0x82, 0xff, 0x83, 0x82,
    0xff, 0x82, 0xff, 0xff, 0x82, 0xff, 0xff, 0xa0, 0x20, 0xff, 0xa0, 0x20, 0xff, 0xa2, 0x20, 0xff,
    0xa2, 0x20, 0xff, 0xa2, 0x20, 0xff, 0xe3, 0xff, 0xff, 0xe3, 0xff, 0xff, 0xc3, 0x81, 0xff, 0xa1,
    0x20, 0xff, 0xa3, 0xe1, 0xff, 0xa3, 0x20, 0xff, 0xa3, 0xe1, 0xff, 0x83, 0x84, 0xff, 0x83, 0x84,
    0xff, 0x83, 0x84, 0xff, 0x64, 0xff, 0xff, 0xa3, 0x60, 0xff, 0x83, 0xff, 0xff, 0x83, 0x60, 0xff,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0xe1, 0xff, 0xff, 0x60, 0x20, 0xff, 0x81, 0xa0, 0xff, 0x81, 0xa0,
    0xff, 0xc1, 0xe0, 0xff, 0xe1, 0xff, 0xff, 0xc1, 0xff, 0xff, 0xe1, 0xe3, 0xff, 0xe3, 0x20, 0xff,
    0xa1, 0xff, 0xff, 0xe1, 0xff, 0xff, 0xa1, 0xff, 0xff, 0xa3, 0xa1, 0xff, 0xa1, 0xff, 0xff, 0x83,
    0xff, 0xff, 0x83, 0xff, 0xff, 0x83, 0xff, 0xff, 0xa4, 0xff, 0xff, 0xa3, 0xff, 0xff, 0x83, 0xff,
    0xff, 0xa3, 0xff, 0xff, 0xa1, 0xff, 0xff, 0xa0, 0xff, 0xff, 0x81, 0xff, 0xff, 0x81, 0xff, 0xff,
    0x81, 0xff, 0xff, 0xa1, 0xff, 0xff, 0x81, 0xff, 0xff, 0x12, 0x13, 0xff, 0x13, 0xff, 0xff, 0x13,
    0xff, 0xff, 0x12, 0xff, 0xff, 0x12, 0xff, 0xff, 0x13, 0x12, 0xff, 0x12, 0x13, 0xff, 0x13, 0x14,
    0xff, 0x13, 0x14, 0xff, 0x14, 0x13, 0xff, 0x14, 0xff, 0xff, 0x13, 0xff, 0xff, 0x13, 0xff, 0xff,
    0x13, 0x12, 0xff, 0x14, 0xff, 0xff, 0x14, 0xff, 0xff, 0x14, 0x10, 0xff, 0x10, 0xff, 0xff, 0x10,
    0xff, 0xff, 0x14, 0x10, 0xff, 0x10, 0x14, 0xff, 0x11, 0xff, 0xff, 0x10, 0xff, 0xff, 0x11, 0x10,
    0xff, 0x11, 0x10, 0xff, 0x11, 0x10, 0xff, 0x11, 0x12, 0xff, 0x11, 0xff, 0xff, 0x12, 0x11, 0xff,
    0x12, 0xff, 0xff, 0x11, 0xff, 0xff, 0x11, 0x12, 0xff, 0x11, 0x12, 0xff, 0x12, 0x11, 0xff, 0x11,
    0x12, 0xff, 0x13, 0x12, 0xff, 0x13, 0x12, 0xff, 0x13, 0xff, 0xff, 0x12, 0xff, 0xff, 0x12, 0x13,
    0xff, 0x13, 0x12, 0xff, 0x12, 0x13, 0xff, 0x12, 0xff, 0xff, 0x12, 0xff, 0xff, 0x11, 0xff, 0xff,
    0x11, 0xff, 0xff, 0x11, 0x12, 0xff, 0x11, 0x12, 0xff, 0x11, 0x12, 0xff,
];

const MOVES1: [u8; 196] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43, 0x43, 0x44, 0x44, 0x43, 0x43, 0x43, 0x44, 0x64,
    0x20, 0xa1, 0xa1, 0x44, 0x20, 0x81, 0x20, 0xa1, 0xa1, 0xa1, 0xa1, 0xa1, 0x81, 0xa2, 0xa1, 0x81,
    0x81, 0x81, 0x81, 0x82, 0x82, 0x81, 0x82, 0x82, 0x82, 0x82, 0x81, 0xa2, 0xa1, 0x81, 0x81, 0x82,
    0x81, 0x83, 0x83, 0x83, 0x82, 0x82, 0x82, 0x82, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20,
    0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x20, 0x80, 0x80, 0x20, 0x20, 0x20, 0x20, 0x82, 0x82, 0xa0,
    0x20, 0x20, 0xa2, 0x20, 0x83, 0x83, 0x83, 0x82, 0x82, 0x83, 0x82, 0xa2, 0x82, 0xa0, 0x20, 0x20,
    0xa2, 0x20, 0x83, 0x83, 0x83, 0xa4, 0xa3, 0x83, 0xa3, 0x83, 0x83, 0x83, 0x84, 0x83, 0x83, 0x83,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x81, 0x80, 0x81, 0x81, 0x81, 0x81, 0x81, 0xa1, 0xa0,
    0x81, 0x81, 0xa1, 0xa1, 0x81, 0x83, 0x83, 0x83, 0xa4, 0x83, 0x83, 0xa3, 0xa1, 0xa0, 0x81, 0x81,
    0x81, 0xa1, 0x81, 0x13, 0x13, 0x13, 0x12, 0x12, 0x13, 0x12, 0x13, 0x13, 0x14, 0x14, 0x13, 0x13,
    0x13, 0x11, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x11, 0x10, 0x11, 0x11, 0x11, 0x11, 0x11, 0x12,
    0x12, 0x11, 0x11, 0x11, 0x12, 0x11, 0x13, 0x13, 0x13, 0x12, 0x12, 0x13, 0x12, 0x12, 0x12, 0x11,
    0x11, 0x11, 0x11, 0x11,
];

/// `0x3c97a8..0x3c97f7`: the weights by how many moves the row offers
/// (the row for none is the end of `0x3c96f0`, which the original reads
/// there too).
const PICKS: [u8; 64] = [
    0x13, 0x12, 0x12, 0x13, 0x12, 0x12, 0x12, 0x11, 0x11, 0x11, 0x11, 0x11, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x02, 0x02, 0x02,
];

/// `0x3c97e8`: the kinds by a move's class, four to a row.
const KINDS: [u8; 64] = [
    0x00, 0x00, 0x00, 0x00, 0x10, 0x10, 0x10, 0x10, 0x20, 0x20, 0x20, 0x20, 0x20, 0x10, 0x20, 0x10,
    0x40, 0x40, 0x40, 0x40, 0x40, 0x10, 0x40, 0x10, 0x40, 0x20, 0x40, 0x20, 0x40, 0x20, 0x10, 0x10,
    0x80, 0x80, 0x80, 0x80, 0x80, 0x10, 0x80, 0x10, 0x80, 0x20, 0x80, 0x20, 0x80, 0x20, 0x10, 0x10,
    0x80, 0x40, 0x80, 0x40, 0x80, 0x40, 0x10, 0x10, 0x80, 0x40, 0x20, 0x80, 0x80, 0x40, 0x20, 0x10,
];
