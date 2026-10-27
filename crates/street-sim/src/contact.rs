//! Players bumping into each other: the original's contact manager
//! (`FUN_00133bb0`, once an update after the actors move). For every pair
//! of players closer than their two radii it runs the plain contact
//! (`FUN_00134250` handler 0, `FUN_001343e8`), which pushes them apart
//! along the line between them. Who gives way is what makes a screen: a
//! player running into an opponent who stands still takes the whole push
//! and cannot get through him.
//!
//! Ported: the radii (`FUN_00133b48`: 1.25 each), the pairs it skips
//! (`FUN_00136218`: teammates when either is in a shot or a dunk), which
//! of the two is moved (`FUN_001343e8`: the defender first; shares by
//! weight; the still opponent, the juker, the mover against a still man,
//! a fallen player, the pass's receiver, a rebound jumper against a man on
//! the floor, and the immovable states, in the original's order), the cap
//! of 0.4 a push an update, and the hold that keeps a jumper from being
//! pushed past |x| 38 under a basket.
//!
//! The screen reaction (raw `0x135400..0x135570`): an offensive player
//! without the ball who keeps running into the same defender (30
//! contacts, `0x3d0f68`, counted in `0x3d0f70`) knocks him off balance;
//! the contact's message (`0x3c6910`) tells the defender's AI who
//! screened him (AI+0xb0, `FUN_00141448` / `FUN_0018b308`), which the
//! defense's switches read (`FUN_00123510`).
//!
//! Each player's radius reaches toward the other with the limbs of his
//! posed clip (`FUN_001337e0`, `World::radius_toward`), the trick's and
//! the counter stance's included; the stance's contact with a juking
//! handler is the counter ([`crate::counter`]).
//!
//! The scripted contacts (`actor+0x198`, the handler table `0x3d0c30`)
//! have four kinds, all ported: 0 the plain one above, 1 the dunks' and
//! lay-ups' knock ([`World::knock_contacts`]), 2 the clearout
//! ([`World::clearout_contact`]) and 3 the trick's fake
//! ([`crate::trick`]). The screen's reactions are the original's scripts
//! ([`crate::trick::SCREENED_HEAVY`], [`crate::trick::SCREENER`]).
//! Each player's weight is the record's (attribute 9,
//! [`crate::Player::weight`], [`WEIGHT`] until a roster sets it).

use crate::{Action, BallState, OpeningToss, World, distance, math};

/// `0x3d0f68`: the contacts with one defender that make a screen.
pub const SCREEN_CONTACTS: i32 = 30;
/// The count after a screen (`0x135508`: -600, ten seconds of contacts).
pub const SCREEN_REST: i32 = -600;

/// The contact manager's memory of each player's pushing (`0x3d0f70`, 8
/// bytes a player, by actor index): the opponent he last pushed against
/// and how many contacts with him in a row (any contact with a teammate
/// or another opponent starts over).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contacts {
    pub with: [i32; 6],
    pub count: [i32; 6],
}

impl Default for Contacts {
    fn default() -> Self {
        Self {
            with: [-1; 6],
            count: [0; 6],
        }
    }
}

/// A stick angle (10 bits, moving along `(-sin a, -cos a)`) from `from`
/// toward `to`, as the original's angle tables hold it.
pub(crate) fn angle(from: [f32; 2], to: [f32; 2]) -> i32 {
    ((math::atan2(from[0] - to[0], from[1] - to[1]) * 162.974_66 + 0.5) as i32) & 0x3ff
}

/// Players `a` and `b`'s distance in the distance table (`places`).
fn apart(places: &[[f32; 2]], a: usize, b: usize) -> f32 {
    distance(places[a], places[b])
}

/// `FUN_00133b48`: each player's base radius (`0x3fa00000`).
pub const RADIUS: f32 = 1.25;

/// When a limb of `FUN_001337e0`'s table counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Counts {
    Always,
    /// In a swipe (state bit 6, `FUN_0016e9a0`).
    Swiping,
    /// On the post defence's rows (`FUN_0016ebe8`: 0x18e, 0x18f,
    /// 0x2cc..0x2cf), which the sim never plays.
    PostDefence,
}

/// `FUN_001337e0`'s limbs (`0x43d060`, the fifth built on the stack): the
/// bone, and how far past it along the line from the pelvis the limb
/// reaches (short of it when negative): the head, the hands, the forearms.
const LIMBS: [(usize, f32, Counts); 5] = [
    (21, 0.5, Counts::Always),
    (crate::pose::RIGHT_HAND, -0.5, Counts::Swiping),
    (crate::pose::LEFT_HAND, -0.5, Counts::Swiping),
    (18, 0.5, Counts::PostDefence),
    (14, 0.5, Counts::PostDefence),
];

/// `FUN_001337e0` only looks at an opponent within this many units of the
/// wanted facing (a quarter turn).
const LIMB_ARC: i32 = 0x101;

/// The rim plane the hoop tests take (`0x41400000`) and the body's room
/// about the ring's middle (`0x3fb8bb99`, the ring's 0.79 and the body's
/// 0.65), squared `0x40054e35`.
const RIM_PLANE: f32 = 12.0;
const RIM_ROOM: f32 = f32::from_bits(0x3fb8_bb99);

/// `FUN_001382b0`: a body from `feet` to `top` at `at` against the rim on
/// side `side` (its middle at 36.75 out): reaching the rim's plane (or
/// rising toward it), within the ring's room in front of its middle, out
/// to its edge; between the middle and the board (36.75..38), out across
/// to either side of the slab. Scaled by how near the top comes to the
/// plane when it is below.
fn rim_push(feet: f32, top: f32, at: [f32; 2], side: f32, rise: f32) -> Option<[f32; 2]> {
    if RIM_PLANE < feet || (top < RIM_PLANE && rise <= 0.0) {
        return None;
    }
    let x = side * 36.75;
    let dx = at[0] - x;
    let z = at[1];
    let push = if dx * dx + z * z < f32::from_bits(0x4005_4e35) && at[0].abs() <= 36.75 {
        let n = libm::sqrtf(dx * dx + z * z);
        [dx / n * RIM_ROOM - dx, z / n * RIM_ROOM - z]
    } else if (36.75..=38.0).contains(&at[0].abs()) && z.abs() <= RIM_ROOM {
        [
            0.0,
            if 0.0 <= z {
                RIM_ROOM - z
            } else {
                -RIM_ROOM - z
            },
        ]
    } else {
        return None;
    };
    let k = if top < RIM_PLANE {
        top / RIM_PLANE
    } else {
        1.0
    };
    Some(push.map(|v| v * k))
}

/// `FUN_001384e0`: a body against the board on side `side` (its face at
/// 38, 4.4 either side; the bottom 11.5): the nearest of its forward bones
/// (`0x3d10b8`, `0x3d10d0`: the hands, the shins, the feet; off balance,
/// the hands above 9.5 reaching furthest, `0x3d10e8`) within the slab's
/// width (1.4, narrowing past 3 across) is pushed out of it, toward the
/// court in front, away behind. And whether his head (bone 21) is up
/// under the board from behind it (`0x13877c..0x1388cc`): over 10.5
/// within 3 of the middle and his nearest bone within 1 of the face, or
/// within 1.5 of the middle (the timed second chance of AIDetect+0x28's
/// frames is never armed).
fn board_push(
    top: f32,
    at: [f32; 2],
    side: f32,
    rise: f32,
    pose: &crate::pose::Pose,
    down: bool,
) -> (Option<[f32; 2]>, bool) {
    const BOTTOM: f32 = 11.5;
    if top < BOTTOM && rise <= 0.0 {
        return (None, false);
    }
    let bone = |k: usize| pose.joints[k].position;
    let forward = [15, 19, 2, 6, 3, 7];
    let mut z = at[1];
    if down {
        for k in forward {
            if bone(k)[2].abs() < z.abs() {
                z = bone(k)[2];
            }
        }
    }
    if f32::from_bits(0x408c_cccd) <= z.abs() {
        return (None, false);
    }
    let mut width = f32::from_bits(0x3fb3_3333);
    if 3.0 <= z.abs() {
        width -= z.abs() - 3.0;
    }
    let face = side * 38.0;
    let mut x = at[0];
    let dir;
    let behind = face.abs() <= x.abs();
    if behind {
        dir = if 0.0 <= face { 1.0 } else { -1.0 };
        for k in forward {
            if bone(k)[0].abs() < x.abs() {
                x = bone(k)[0];
            }
        }
    } else {
        dir = if 0.0 <= face { -1.0 } else { 1.0 };
        if down {
            for k in [15, 19] {
                let b = bone(k);
                if x.abs() < b[0].abs() && 9.5 < b[1] {
                    x = b[0];
                }
            }
        }
    }
    let head = bone(21)[1];
    let bonk = behind
        && at[1].abs() < 3.0
        && 10.5 < head
        && (x.abs() < face.abs() + 1.0 || at[1].abs() < 1.5);
    if (x - face).abs() >= width {
        return (None, bonk);
    }
    let push = [(face - x) + dir * width, 0.0];
    let k = if top < BOTTOM { top / BOTTOM } else { 1.0 };
    (Some(push.map(|v| v * k)), bonk)
}

/// Past any limb's reach from the pelvis (a tall player's head is about
/// 3.5 from it): no pair further apart than twice this can touch.
const LIMB_MAX: f32 = 6.0;
/// The most a contact moves a player in an update (`0x3ecccccd`).
pub const MAX_PUSH: f32 = 0.4;
/// A player's weight until a roster gives the record's (attribute 9,
/// read by `FUN_001da4e0(record, 9)`): the same for everyone, so a
/// contact between two movers splits the push evenly.
pub const WEIGHT: f32 = 200.0;

impl World {
    /// Off balance or on the floor (animation flag 0x40, `FUN_0016ea70`).
    pub(crate) fn down(&self, i: usize) -> bool {
        matches!(
            self.players[i].action,
            Action::Stumbling { .. } | Action::Dive { .. }
        ) || self.bumped(i)
    }

    /// The states no contact moves (`0x8e088`: an alley-oop leap, a shot,
    /// a lay-up, a dunk).
    fn immovable(&self, i: usize) -> bool {
        matches!(
            self.players[i].action,
            Action::Shooting { .. } | Action::Dunking { .. } | Action::Layup { .. }
        )
    }

    /// A rebound jump (bit 8, `FUN_0016e9f0`; [`crate::rebound`]).
    fn rebound_jump(&self, i: usize) -> bool {
        matches!(self.players[i].action, Action::ReboundJump { .. })
    }

    /// A block jump (bit 12, `FUN_0016e9c0`).
    fn block_jump(&self, i: usize) -> bool {
        matches!(self.players[i].action, Action::Jumping { .. })
    }

    /// Moving (actor+0x40, the velocity, not the zero vector).
    fn moving(&self, i: usize) -> bool {
        let p = &self.players[i];
        p.moving() || (p.height > 0.0 && !matches!(p.action, Action::None))
    }

    /// `FUN_00133bb0`'s plain contacts: every pair, the lower index first,
    /// closer than the two radii. How close is the game's distance table
    /// (`FUN_0018d0c0`, game+0x1ac, built by `FUN_0013fbc8` before the
    /// actors update): `table`, every player's place as the update began;
    /// `FUN_001343e8` tests the radii against it and pushes by their
    /// overlap with it (raw `0x1344ac..0x1344cc`, `0x135c34..0x135c64`),
    /// along the line between where they stand now.
    pub(crate) fn step_contacts(&mut self, table: &[[f32; 2]]) {
        // The box-outs' pairs drawn together, then the scripted contacts
        // (each pair's before its push): the trick moves' fake windows
        // ([`crate::trick`]).
        self.pin_box_outs();
        self.trick_contacts();
        self.knock_contacts();
        let n = self.players.len();
        let places: Vec<[f32; 2]> = (0..n)
            .map(|i| table.get(i).copied().unwrap_or(self.players[i].position))
            .collect();
        let mut poses: Vec<Option<Option<crate::pose::Pose>>> = vec![None; n];
        for a in 0..n {
            for b in a + 1..n {
                let d = apart(&places, a, b);
                if d > LIMB_MAX + LIMB_MAX {
                    continue;
                }
                let reach = self.reach(a, b, d, &mut poses);
                if d < reach && self.may_touch(a, b) {
                    self.bump(a, b, reach, d);
                }
            }
        }
        self.hoop_contacts();
    }

    /// The hoops against the bodies (`0x134110..0x134210`, after the
    /// pairs): a block jumper (bit 12, not the glass pair's bit 19), a
    /// rebound jumper (bit 8, not a lay-up's bit 13), a passer in the air
    /// or a man off balance in the air (flag 0x40), never a dunker (bit 7),
    /// meets the rim and the board ([`World::hoop_contact`]).
    fn hoop_contacts(&mut self) {
        if self.poses.get().is_none() {
            return;
        }
        for i in 0..self.players.len() {
            let p = &self.players[i];
            let aloft = p.height > 0.0;
            let meets = match p.action {
                Action::Dunking { .. } => false,
                Action::Jumping { block: Some(j), .. } => {
                    !crate::block::SCRIPTS[usize::from(j.script)].glass
                }
                Action::ReboundJump { .. } => true,
                Action::Passing { .. } => aloft,
                _ => self.down(i) && aloft,
            };
            if meets {
                self.hoop_contact(i);
            }
        }
    }

    /// `FUN_001389a8`: `i`'s body against the hoop on his side, the rim
    /// (`FUN_001382b0`) then the board (`FUN_001384e0`), each push held to
    /// [`MAX_PUSH`]; he moves (not a passer, `0x138ca0`) and stays on the
    /// court (`FUN_0016f900`), his velocity across halved where it runs
    /// into the push (`0x138cd0..0x138dbc`). His head into the board while
    /// he rises, not passing nor off balance, plays reaction `0x44bd94`
    /// and sways the rim by 1.5 (slot 75, `0x3c6920`, `FUN_001414d8`).
    fn hoop_contact(&mut self, i: usize) {
        let Some(pose) = self.actor_pose(i) else {
            return;
        };
        let y = |k: usize| pose.joints[k].position[1];
        // The head and the hands' highest (`0x43d110`), the feet's lowest.
        let top = [21, 15, 19].map(y).into_iter().fold(-100.0f32, f32::max);
        let feet = y(3).min(y(7));
        let p = &self.players[i];
        let rise = p.rise;
        let before = p.position;
        let side = if before[0] >= 0.0 { 1.0 } else { -1.0 };
        let mut at = before;
        let mut moved = false;
        let push_by = |at: &mut [f32; 2], push: [f32; 2]| {
            let len = crate::math::len(push[0], push[1]);
            let k = if len > MAX_PUSH { MAX_PUSH / len } else { 1.0 };
            at[0] += push[0] * k;
            at[1] += push[1] * k;
        };
        if let Some(push) = rim_push(feet, top, at, side, rise) {
            push_by(&mut at, push);
            moved = true;
        }
        let (board, bonk) = board_push(top, at, side, rise, &pose, self.down(i));
        if let Some(push) = board {
            push_by(&mut at, push);
            moved = true;
        }
        if moved {
            if !matches!(self.players[i].action, Action::Passing { .. }) {
                self.players[i].position = at;
                self.contain(i, before);
                at = self.players[i].position;
            }
            let d = [at[0] - before[0], at[1] - before[1]];
            self.halve_drift(i, d);
        }
        if bonk
            && !matches!(self.players[i].action, Action::Passing { .. })
            && !self.down(i)
            && rise > 0.0
        {
            self.react(i, crate::trick::KNOCK_BEHIND);
            self.events.push(crate::Event::RimBump { player: i });
            let side = crate::rim::post_side(self.players[i].position[0]);
            let rim = &mut self.rims[side];
            if !(rim.torn && !rim.held) {
                rim.sway(1.5);
            }
        }
    }

    /// `0x138cd0..0x138dbc`: each part of `i`'s velocity across that runs
    /// against `push` is halved.
    fn halve_drift(&mut self, i: usize, push: [f32; 2]) {
        let halve = |v: &mut f32, d: f32| {
            if d != 0.0 && (*v >= 0.0) != (d >= 0.0) {
                *v *= 0.5;
            }
        };
        let p = &mut self.players[i];
        match &mut p.action {
            Action::Jumping { block: Some(j), .. } => {
                halve(&mut j.drift[0], push[0]);
                halve(&mut j.drift[1], push[1]);
            }
            Action::ReboundJump { drift, .. } => {
                halve(&mut drift[0], push[0]);
                halve(&mut drift[1], push[1]);
            }
            _ => {
                halve(&mut p.flight.velocity[0], push[0]);
                halve(&mut p.flight.velocity[2], push[1]);
            }
        }
    }

    /// `FUN_00133b48(a, b)`: the two radii, each reaching toward the
    /// other, `apart` in the distance table. `poses` keeps each player's
    /// bones once posed this update.
    fn reach(
        &self,
        a: usize,
        b: usize,
        apart: f32,
        poses: &mut [Option<Option<crate::pose::Pose>>],
    ) -> f32 {
        let mut toward = |me: usize, other: usize| {
            let pose = poses[me].get_or_insert_with(|| self.actor_pose(me));
            self.radius_toward(me, other, apart, pose.as_ref())
        };
        toward(a, b) + toward(b, a)
    }

    /// `FUN_001337e0(1.25, me, other)`: `me`'s radius toward `other`. Out
    /// of the tip and the post-up, with `other` within a quarter turn of `me`'s wanted
    /// facing, each limb that counts (`LIMBS`) and comes nearer `other`
    /// than the last one taken widens the radius to its reach from the
    /// pelvis on the floor (the bone pushed along the line from the pelvis,
    /// raw `0x133a50..0x133aa8`); the first to beat is `apart`, the distance
    /// table's (`FUN_0018d0c0`).
    fn radius_toward(
        &self,
        me: usize,
        other: usize,
        apart: f32,
        pose: Option<&crate::pose::Pose>,
    ) -> f32 {
        let mut radius = RADIUS;
        let Some(pose) = pose else {
            return radius;
        };
        if self.game_phase.phase == crate::game_phase::phase::TIP {
            return radius;
        }
        // The post-up (`0x133838..0x133880`): posting or on the post-up's
        // clips, no limbs; nor against a man posting off those clips from
        // the post defence's rows.
        if self.posting(me) || self.on_post_up_clip(me) {
            return radius;
        }
        if self.posting(other)
            && !self.on_post_up_clip(other)
            && matches!(self.channel_row(me), 718 | 719)
        {
            return radius;
        }
        let (pm, po) = (self.players[me].position, self.players[other].position);
        let wanted = crate::timing::facing_word(self.players[me].wanted);
        if crate::guard::turn(wanted, angle(pm, po)).abs() >= LIMB_ARC {
            return radius;
        }
        // The swipe's bit 6: a swipe, or the take after a counter
        // ([`crate::counter`], 0x10040).
        let swiping = match self.players[me].action {
            Action::Stealing { counter: false, .. } => true,
            Action::Stealing {
                stance: Some(s), ..
            } => s.take(),
            _ => false,
        };
        let root = pose.joints[crate::pose::PELVIS].position;
        let mut nearest = apart;
        for (bone, past, counts) in LIMBS {
            let counted = match counts {
                Counts::Always => true,
                Counts::Swiping => swiping,
                Counts::PostDefence => false,
            };
            if !counted {
                continue;
            }
            let at = pose.joints[bone].position;
            let d = [at[0] - root[0], at[1] - root[1], at[2] - root[2]];
            let length = crate::pose::length(d);
            if length <= 0.0 {
                continue;
            }
            let limb = [at[0] + d[0] / length * past, at[2] + d[2] / length * past];
            let reach = distance(limb, [root[0], root[2]]);
            let near = distance(po, limb);
            if radius < reach && near < nearest {
                radius = reach;
                nearest = near;
            }
        }
        radius
    }

    /// `FUN_00136218`: teammates do not touch while either shoots or
    /// dunks (`0x88080`), nor does a box-out's pair (`0x13624c..0x136278`:
    /// one's boxer, +0x230, the other, whose man, +0x234, he is).
    fn may_touch(&self, a: usize, b: usize) -> bool {
        let busy = |i: usize| {
            matches!(
                self.players[i].action,
                Action::Shooting { .. } | Action::Dunking { .. }
            )
        };
        let pair = |x: usize, y: usize| {
            self.players[x].boxer == Some(y as u8) && self.players[y].partner == Some(x as u8)
        };
        (self.players[a].team != self.players[b].team || !(busy(a) || busy(b)))
            && !pair(a, b)
            && !pair(b, a)
    }

    /// The contact manager's first pass (`FUN_00133bb0`, raw
    /// `0x133c08..0x133e40`): each boxed man (bit 18) whose boxer (+0x230,
    /// bit 17) holds him (+0x234), neither channel in a blend (0x200000),
    /// is drawn toward his place by the boxer: his model's scale (+0x40)
    /// along the boxer's wanted facing, turned by the clips they play (the
    /// boxer's 60/61: 320 either way at 2.5 scales; else behind him, the
    /// boxed man's 58/59 64 either way of it at 1.5, else 2.15), plus his
    /// model's root now (+0xe0, `0x133d74..0x133d80`: his clip's root at
    /// the frame shown, turned and scaled into the world), at most 0.4 an
    /// update.
    fn pin_box_outs(&mut self) {
        for i in 0..self.players.len() {
            let p = &self.players[i];
            let Some(b) = p.boxer.map(usize::from) else {
                continue;
            };
            if !p.boxed
                || b >= self.players.len()
                || !self.players[b].boxing
                || self.players[b].partner != Some(i as u8)
            {
                continue;
            }
            let blending = |k: usize| {
                self.players[k]
                    .stand
                    .is_some_and(|s| s.run.blend_counter().is_some())
            };
            if blending(i) || blending(b) {
                continue;
            }
            let (held, boxer) = (self.channel_row(i), self.channel_row(b));
            let mut word = crate::timing::facing_word(self.players[b].wanted);
            let mut scale = self.players[i].body().height;
            match (boxer, held) {
                (60, _) => {
                    word += 320;
                    scale *= 2.5;
                }
                (61, _) => {
                    word -= 320;
                    scale *= 2.5;
                }
                (_, 58) => {
                    word += 512 - 64;
                    scale *= 1.5;
                }
                (_, 59) => {
                    word += 512 + 64;
                    scale *= 1.5;
                }
                _ => {
                    word += 512;
                    scale *= f32::from_bits(0x4009_999a);
                }
            }
            let word = word & 0x3ff;
            let at = self.players[b].position;
            let root = self.model_root(i);
            let to = [
                at[0] - scale * crate::input::native_sin_1024(word) + root[0],
                at[1] - scale * crate::input::native_cos_1024(word) + root[1],
            ];
            let me = self.players[i].position;
            let mut d = [to[0] - me[0], to[1] - me[1]];
            let len = math::len(d[0], d[1]);
            let step = f32::from_bits(0x3ecc_cccd);
            if step < len {
                d = [d[0] / len * step, d[1] / len * step];
            }
            self.players[i].position = [me[0] + d[0], me[1] + d[1]];
        }
    }

    /// Actor+0xe0, the model's root now: the clip channel 0 shows, its
    /// root at the frame shown turned by his facing word and scaled by his
    /// body (x, z); nothing without the disc's animations or in a blend.
    pub(crate) fn model_root(&self, i: usize) -> [f32; 2] {
        let (Some(poses), Some(shown)) = (self.poses.get(), self.channel(i).map(|c| c.shown))
        else {
            return [0.0; 2];
        };
        let (Some(record), None) = (shown.record, shown.blend) else {
            return [0.0; 2];
        };
        let Some(root) = poses.root(crate::pose::Frame {
            record,
            time: shown.frame,
            looping: false,
        }) else {
            return [0.0; 2];
        };
        let p = &self.players[i];
        let theta = crate::timing::facing_word(p.heading) as f32 * std::f32::consts::TAU / 1024.0;
        let yaw = [0.0, math::sin(theta * 0.5), 0.0, math::cos(theta * 0.5)];
        let h = p.body().height;
        let off = crate::pose::rotate(yaw, [root[0] * h, 0.0, root[2] * h]);
        [off[0], off[2]]
    }

    /// `FUN_001343e8(p, q)`: push the two apart. `a` (the original's
    /// `iVar21`) is the defender of an opposing pair, `b` the other; the
    /// tip's winner is always `b`. `a` moves back along the line from `a`
    /// to `b` by his share of the overlap (the radii less `apart`, the
    /// distance table's), `b` forward by his.
    fn bump(&mut self, p: usize, q: usize, reach: f32, apart: f32) {
        let offence = self.possession.min(1);
        let (mut a, mut b) = (p, q);
        if self.players[p].team.min(1) == offence && self.players[p].team != self.players[q].team {
            (a, b) = (q, p);
        }
        let toss = self.opening_toss != OpeningToss::None;
        if toss && self.tip.winner >= 0 {
            let winner = (0..self.players.len())
                .filter(|&j| self.players[j].team.min(1) == (self.tip.winner / 3) as usize)
                .nth(0);
            if winner == Some(a) {
                (a, b) = (b, a);
            }
        }
        let (pa, pb) = (self.players[a].position, self.players[b].position);
        let gap = apart;
        if reach <= gap {
            return;
        }
        // `0x134680..0x1346c8`: the pushing count of `b` (the offence's man
        // of an opposing pair).
        if let Some(k) = (b < 6).then_some(b) {
            let c = &mut self.contacts;
            if self.players[a].team == self.players[b].team {
                c.with[k] = -1;
                c.count[k] = 0;
            } else if c.with[k] != a as i32 {
                c.with[k] = a as i32;
                c.count[k] = 0;
            } else {
                c.count[k] += 1;
            }
        }
        let (a_moving, b_moving) = (self.moving(a), self.moving(b));
        let (a_floor, b_floor) = (self.players[a].height == 0.0, self.players[b].height == 0.0);
        let (a_down, b_down) = (self.down(a), self.down(b));
        let opponents = self.players[a].team != self.players[b].team;
        // `0x134ac8..0x134f48`: a defender in the counter stance meeting
        // a juke resolves the counter ([`World::counter`]) instead of the
        // push.
        if opponents
            && matches!(
                self.players[a].action,
                Action::Stealing { counter: true, .. }
            )
            && self.counter(a, b)
        {
            return;
        }
        // `0x134f4c..0x135344`: a box-out takes the pair instead
        // ([`World::box_out_contact`]).
        if self.box_out_contact(a, b) {
            return;
        }
        // The line from a to b (raw 0x134760..0x1347d0): +x when they
        // stand on one spot.
        let (dx, dz) = (pb[0] - pa[0], pb[1] - pa[1]);
        let n = if dx * dx + dz * dz < f32::from_bits(0x3a83_126f) {
            [1.0, 0.0]
        } else {
            let k = 1.0 / math::len(dx, dz);
            [dx * k, dz * k]
        };
        // Shares by weight; two rebound jumpers in the air add their
        // Rebounds and the higher one 50 (the original compares the
        // animations' root heights; here the jumps').
        let (mut wa, mut wb) = (
            f32::from(self.players[a].weight),
            f32::from(self.players[b].weight),
        );
        if self.rebound_jump(b) && self.rebound_jump(a) && !a_floor && !b_floor {
            wa += (self.players[a].ratings.rebounds * 99.0).round();
            wb += (self.players[b].ratings.rebounds * 99.0).round();
            if 0.0 <= self.players[a].height - self.players[b].height {
                wa += 50.0;
            } else {
                wb += 50.0;
            }
        }
        // (a's share, b's share).
        let mut share = (wb / (wa + wb), wa / (wa + wb));
        let a_all = (1.0, 0.0);
        let b_all = (0.0, 1.0);
        if toss || (opponents && !b_moving) {
            share = a_all;
        }
        let juke = |i: usize| self.players[i].action.juking();
        if juke(b) && !juke(a) {
            share = a_all;
        } else if juke(a) && !juke(b) {
            share = b_all;
        }
        // Neither posting (`0x1349b0..0x1349c8`).
        if a_floor && b_floor && !self.posting(a) && !self.posting(b) {
            if !a_moving {
                if b_moving {
                    share = b_all;
                }
            } else if !b_moving {
                share = a_all;
            }
        }
        if !a_down {
            if b_down {
                share = a_all;
            }
        } else if !b_down {
            share = b_all;
        }
        if let (BallState::Pass, Some(pass)) = (self.ball.state, self.ball.pass) {
            if pass.to == a {
                share = b_all;
            } else if pass.to == b {
                share = a_all;
            }
        }
        // A posting man standing still is not moved (`0x134a94..
        // 0x134ab4`): his backdown pushes the defender.
        if self.posting(b) && !b_moving {
            share = a_all;
        }
        let a_offence = self.players[a].team.min(1) == offence;
        if !opponents {
            if self.rebound_jump(a) && !self.immovable(a) && !a_floor && b_floor {
                share = if a_offence && !b_moving { a_all } else { b_all };
            }
            if self.rebound_jump(b) && !self.immovable(b) && !b_floor && a_floor {
                share = if a_offence && !a_moving { b_all } else { a_all };
            }
        } else {
            if self.rebound_jump(a) && !self.immovable(a) && !a_floor && b_floor {
                share = b_all;
            }
            if self.rebound_jump(b) && !self.immovable(b) && !b_floor && a_floor {
                share = if a_moving { a_all } else { b_all };
            }
        }
        if self.block_jump(b) && !b_floor && a_floor {
            share = if a_moving { a_all } else { b_all };
        }
        if self.block_jump(a) && !a_floor && b_floor {
            share = if b_moving { b_all } else { a_all };
        }
        if self.immovable(a) {
            share = b_all;
        }
        if self.immovable(b) {
            share = a_all;
        }
        let overlap = reach - gap;
        let (mut sa, mut sb) = (share.0 * overlap, share.1 * overlap);
        // Both still sharing on the floor (raw 0x134ea0..): unless `a` is
        // the heavier teammate, `b`, if moving, gives way only as far as
        // he came toward `a` this update (`-(v_b . n)`), and none if he
        // moves away; else `a`, if moving, only as far as he came toward
        // `b`. The comparisons are the original's, on the scaled shares.
        if sa != 0.0 && sb != 0.0 && sa != 1.0 && sb != 1.0 && a_floor && b_floor {
            let along = |i: usize| {
                let v = self.players[i].velocity();
                (v[0] * n[0] + v[1] * n[1]) * crate::TICK_SECONDS
            };
            if wa <= wb || opponents {
                if b_moving {
                    let toward = -along(b);
                    if toward <= 0.0 {
                        (sa, sb) = (overlap, 0.0);
                    } else if toward < sb {
                        (sa, sb) = (overlap - toward, toward);
                    }
                }
            } else if a_moving {
                let toward = along(a);
                if toward <= 0.0 {
                    (sa, sb) = (0.0, overlap);
                } else if toward < sa {
                    (sa, sb) = (toward, overlap - toward);
                }
            }
        }
        if opponents {
            // With the ball `b` may bump a posting man's defender instead
            // (`0x135430`); the bump pushes neither.
            if self.ball.state == BallState::Held(b) {
                // On the backdown's walk the bump, else the turbo
                // collision (`0x135748`).
                let taken = if matches!(self.channel_row(b), 704 | 705) {
                    self.backdown_bump(a, b, [a_moving, b_moving])
                } else {
                    let turbo = self.turbo_held(b);
                    self.turbo_collision(a, b, [a_moving, b_moving], turbo)
                };
                if taken {
                    return;
                }
            } else {
                self.screen_reaction(a, b, a_down, a_floor, b_floor, b_moving);
            }
        }
        for (i, s, sign) in [(a, sa, -1.0), (b, sb, 1.0)] {
            if s == 0.0 {
                continue;
            }
            let step = s.min(MAX_PUSH) * sign;
            let jumper = self.rebound_jump(i) || self.block_jump(i) || self.players[i].height > 0.0;
            let p = &mut self.players[i];
            let before = p.position;
            let mut at = [before[0] + n[0] * step, before[1] + n[1] * step];
            // Under a basket a jumper is not pushed out past |x| 38.
            if jumper && 38.0 < at[0].abs() && before[0].abs() < 38.0 && at[1].abs() < 4.4 {
                at[0] = f32::from_bits(0x4217_999a).copysign(at[0]);
            }
            p.position = at;
        }
    }
}

impl World {
    /// A player's facing as a 10-bit stick angle (actor+0xb0).
    pub(crate) fn facing_units(&self, i: usize) -> i32 {
        ((self.players[i].heading + std::f32::consts::PI) * 162.974_66 + 0.5) as i32 & 0x3ff
    }

    /// `FUN_0018d9f0(a, b)`: the eighth of a turn about `a`'s facing that
    /// `b` stands in, 0..7 (4 dead ahead); its quarter is
    /// [`World::quarter`].
    pub(crate) fn octant(&self, a: usize, b: usize) -> i32 {
        let (pa, pb) = (self.players[a].position, self.players[b].position);
        let d = (self.facing_units(a) - angle(pa, pb) + 0x200) & 0x3ff;
        ((d + 0x40) & 0x3ff) >> 7
    }

    /// `FUN_0018d9f0(a, b)`'s quarter (its third argument), 0..3 (2 dead
    /// ahead).
    pub(crate) fn quarter(&self, a: usize, b: usize) -> i32 {
        let (pa, pb) = (self.players[a].position, self.players[b].position);
        let d = (self.facing_units(a) - angle(pa, pb) + 0x200) & 0x3ff;
        ((d + 0x80) & 0x3ff) >> 8
    }

    /// The screen reaction (raw `0x135400..0x135570`): `b`, the offence's
    /// man without the ball, moving, with someone holding the ball, has
    /// pushed against defender `a` (not off balance) `SCREEN_CONTACTS`
    /// times, facing within 160 units of him, both on the floor. Three
    /// draws: `b`'s reaction (`0x3d0fb0[r % 3]`); on equal power (rating
    /// 4) the reaction's weight (`r % 100 < 51` the light one, `0x3d0f38`,
    /// else the heavy one, `0x3d0f28`; the weaker defender always the
    /// heavy, the stronger the light); then `a`'s animation in the list for
    /// the side `b` hit him from (`FUN_0018d9f0`'s quarter; the lists hold
    /// 5/3/4/3 and 1/2/1/2). The count rests at -600. The message tells
    /// `a`'s AI who screened him.
    fn screen_reaction(
        &mut self,
        a: usize,
        b: usize,
        a_down: bool,
        a_floor: bool,
        b_floor: bool,
        b_moving: bool,
    ) {
        if b >= 6 || self.ball.state == BallState::Held(b) {
            return;
        }
        // Not in the Pick lesson (game vt+0xb4 0xf, `0x135444`).
        if self.lesson == Some(0xf) || self.handler().is_none() || !b_moving || a_down {
            return;
        }
        if self.contacts.count[b] < SCREEN_CONTACTS {
            return;
        }
        let (pa, pb) = (self.players[a].position, self.players[b].position);
        // `FUN_0018d1f8(b, a)`: how far `b` faces off the line to `a`.
        let off = (self.facing_units(b) - angle(pb, pa)) & 0x3ff;
        let off = if 0x200 < off { 0x400 - off } else { off };
        if 0xa0 < off || !a_floor || !b_floor {
            return;
        }
        // `b`'s reaction (`0x3d0fb0[r % 3]`, `FUN_0019c2f8` running its
        // first update at once): a setroughpick clip.
        let r = self.rng.next_u32();
        self.react(b, crate::trick::SCREENER[(r % 3) as usize]);
        self.contacts.count[b] = SCREEN_REST;
        let power = |i: usize| self.players[i].ratings.native(4);
        let (pa_power, pb_power) = (power(a), power(b));
        let heavy = if pa_power < pb_power {
            true
        } else if pa_power == pb_power {
            self.rng.next_u32() % 100 >= 0x33
        } else {
            false
        };
        // `FUN_0018d9f0(a, b)`: the quarter `b` hit `a` from.
        let quarter = self.quarter(a, b) as usize;
        let list = if heavy {
            crate::trick::SCREENED_HEAVY[quarter]
        } else {
            crate::trick::SCREENED_LIGHT[quarter]
        };
        let r = self.rng.next_u32();
        self.react(a, list[(r % list.len() as u32) as usize]);
        // `0x3c6910` (slot 74) with `FUN_001342d0(a)`'s kind: 3, a screen
        // on the holder's own defender by the arc, is a pick's 500
        // ([`World::post_contact`]).
        let kind = self.screen_kind(a);
        self.post_contact(b, kind);
        if kind == 3 {
            self.events.push(crate::Event::Pick {
                screener: b,
                defender: a,
            });
        }
        // `0x3c6910` → `FUN_00141448`: unless the script manager runs a
        // script (game+0x424 +0x50 in 3..7), the defender's AI+0xb0 is the
        // screener.
        if self.sequence.is_none()
            && let Some(brain) = self.ai.brains.get_mut(a)
        {
            brain.screened_by = b as i32;
        }
    }
}

#[cfg(test)]
mod hoop_tests {
    use super::*;

    #[test]
    fn a_body_at_the_rim_goes_out_to_its_edge() {
        // Under the +x rim's middle, 0.5 in front of it, head over the plane.
        let push = rim_push(9.0, 12.5, [36.25, 0.0], 1.0, 0.0).unwrap();
        assert!((push[0] + (RIM_ROOM - 0.5)).abs() < 1e-5, "{push:?}");
        // Between the middle and the board: out across.
        let push = rim_push(9.0, 12.5, [37.5, 0.4], 1.0, 0.0).unwrap();
        assert!((push[1] - (RIM_ROOM - 0.4)).abs() < 1e-5, "{push:?}");
        // Below the plane and falling: nothing; rising, scaled.
        assert!(rim_push(5.0, 11.0, [36.25, 0.0], 1.0, -0.1).is_none());
        let up = rim_push(5.0, 11.0, [36.25, 0.0], 1.0, 0.1).unwrap();
        assert!(
            (up[0] + (RIM_ROOM - 0.5) * 11.0 / 12.0).abs() < 1e-5,
            "{up:?}"
        );
        // Feet above the plane: clear of it.
        assert!(rim_push(12.5, 18.0, [36.25, 0.0], 1.0, 0.0).is_none());
    }
}
