//! The swipe: a steal press without the two turbos (`FUN_0015da28`'s
//! press, raw `0x15e0e4..0x15e360`) plays one of five scripts, and while
//! the script's swipe bit is set (`actor+0xa4` bit 6) the actor's update
//! tests the swiping hand against the ball every update (`FUN_0016cf30`)
//! until it has once been in reach.
//!
//! The script: with nobody holding the ball, a low swipe by `r & 1`
//! (`0x3d7220`); with a holder, the post-up steal when the swiper stands
//! within 5 of him (the game's distance table), in one of the three eighths
//! behind him (`FUN_0018d9f0`) and the two face each other's way
//! (`FUN_0018d1f8`: the holder within `0xc8` of the swiper's facing, the
//! swiper `400..0x270` off the holder's), or when the swiper plays the post
//! defence's stance (records 718/719); otherwise by the ball's height
//! against the swiper's pelvis (bone 0): within 1.5 the middle swipe, above
//! it the high one, below it a low one by `r & 1`. Each blends five
//! updates to its clip (`8e f5 05 0`: the left-handed alternate when the
//! left hand, bone 15, is nearer the ball than the right, bone 19), sets
//! the swipe bit from its first update (`9b 40`), waits to frame 23 (30 for
//! the post-up steal) over its updates (`8d`) and clears the bit (`9c 40`):
//! 27, 32, 30, 32 and 40 updates, as the first attract capture's swipes
//! have them.
//!
//! The test (`FUN_0016cf30`): the hand is `FUN_0014da20`'s point for
//! channel 0's clip and frame (during the blend the clip the swiper was
//! playing, held), the right hand when that clip's state has bit 8, else
//! the left; it must be within 1.5 of the ball, 0.75 when the swiper is
//! moving, a twentieth of that for the post-up steal's clips (398, 399).
//! The roll `r % 100` is drawn on every test; the first update in reach
//! uses the swipe up (`actor+0x23c`), landing or not ([`crate::World`]'s
//! strip).
//!
//! The simulation poses the swiper's held clip from his gait channel, and
//! takes the ball where the last ball update left it (ball+0x30): for a
//! holder in his gait the dribble's place ([`crate::dribble`],
//! `FUN_0014e060`), for one in a shot, dunk, lay-up, trick or pass the
//! rewrite's carried point. Without the disc's animations
//! ([`crate::pose::Poses`]) the rewrite's stand-in tests once
//! ([`crate::tuning::HAND_REACH`]).

use crate::pose::{self, Body, Frame, Place, Pose, Poses};

/// One of the swipe's scripts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Script {
    pub address: u32,
    /// The blend's primary record (mirrored) and its alternate.
    pub records: [u16; 2],
    /// The frame the wait reaches and over how many updates.
    pub frame: f32,
    pub updates: u16,
}

/// The five scripts: low out, low in, high out, middle in, post-up.
pub const SCRIPTS: [Script; 5] = [
    Script {
        address: 0x457524,
        records: [389, 388],
        frame: 23.0,
        updates: 22,
    },
    Script {
        address: 0x457577,
        records: [393, 392],
        frame: 23.0,
        updates: 27,
    },
    Script {
        address: 0x4575c9,
        records: [387, 386],
        frame: 23.0,
        updates: 25,
    },
    Script {
        address: 0x45761b,
        records: [391, 390],
        frame: 23.0,
        updates: 27,
    },
    Script {
        address: 0x45766d,
        records: [399, 398],
        frame: 30.0,
        updates: 35,
    },
];

/// `0x3d7220`: the low swipes by `r & 1` (in, out).
pub const LOW: [u8; 2] = [1, 0];
pub const POST_UP: u8 = 4;
/// The blend's updates (`8e .. 05`).
pub const BLEND: u16 = 5;
/// The post defence's stance on the ball (`0x43f070`).
pub const POST_STANCE: [u16; 2] = [719, 718];
/// The swipe's reach (`0x16d1a4..0x16d1bc`): standing, moving.
pub const REACH: [f32; 2] = [1.5, 0.75];

/// A swipe under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swipe {
    /// Into [`SCRIPTS`].
    pub script: u8,
    /// The record its blend plays.
    pub record: u16,
    /// The clip the swiper played when he pressed (record and frame),
    /// held through the blend.
    pub from: Option<(u16, f32)>,
    /// Updates since the press.
    pub tick: u16,
    /// The hand has been in reach once (`actor+0x23c`).
    pub tested: bool,
}

/// What channel 0 shows on a swipe's tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    /// Channel 0's record (`+0x50`) and integer frame (`+0x5c`): the held
    /// clip's through the blend.
    pub record: Option<u16>,
    pub frame: i32,
}

impl Swipe {
    /// Updates the swipe bit is set: the blend and the wait.
    pub fn span(&self) -> u16 {
        BLEND + SCRIPTS[usize::from(self.script)].updates
    }

    /// The incoming clip's weight through the blend (the counter over the
    /// length, the opcode's own update first), then None.
    pub fn blend(&self) -> Option<f32> {
        (self.tick < BLEND).then(|| f32::from(self.tick + 1) / f32::from(BLEND))
    }

    /// The swipe clip's frame: its start through the blend, then the wait's
    /// even steps to the script's frame.
    pub fn frame(&self) -> f32 {
        let s = SCRIPTS[usize::from(self.script)];
        if self.tick < BLEND {
            return 0.0;
        }
        let k = f32::from(self.tick - BLEND + 1).min(f32::from(s.updates));
        k * s.frame / f32::from(s.updates)
    }

    /// Channel 0 on this tick.
    pub fn shown(&self) -> Shown {
        match (self.blend(), self.from) {
            (Some(_), Some((record, time))) => Shown {
                record: Some(record),
                frame: time as i32,
            },
            (Some(_), None) => Shown {
                record: None,
                frame: 0,
            },
            (None, _) => Shown {
                record: Some(self.record),
                frame: self.frame() as i32,
            },
        }
    }

    /// The swiper's pose on this tick.
    pub fn pose(&self, poses: &Poses, body: Body, place: Place) -> Option<Pose> {
        let to = Frame {
            record: self.record,
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
}

/// The hand `FUN_0016cf30` swipes with: the right (1) when channel 0's
/// state has bit 8, else the left (2).
pub fn hand(state: u32) -> u8 {
    if state & 8 != 0 { 1 } else { 2 }
}

/// The swipe's reach: 1.5 standing, 0.75 moving, a twentieth of that on
/// the post-up steal's clips.
pub fn reach(moving: bool, record: Option<u16>) -> f32 {
    let r = REACH[usize::from(moving)];
    if record.is_some_and(|r| r == 398 || r == 399) {
        r * 0.05
    } else {
        r
    }
}

/// The ball's distance from the swiping hand.
pub fn distance(pose: &Pose, shown: Shown, state: u32, body: Body, ball: [f32; 3]) -> f32 {
    let clip = shown.record.map_or(-1, i32::from);
    let at = pose::hand_point(pose, clip, shown.frame, hand(state), body, None);
    pose::length([at[0] - ball[0], at[1] - ball[1], at[2] - ball[2]])
}

/// The script by the ball's height against the pelvis (bone 0): within
/// 1.5 the middle swipe; above, the high one; below, a low one by `draw()`
/// (`r & 1`).
pub fn by_height(pelvis_y: f32, ball_y: f32, draw: impl FnOnce() -> u32) -> u8 {
    if ball_y <= pelvis_y + 1.5 && pelvis_y - 1.5 <= ball_y {
        3
    } else if pelvis_y <= ball_y {
        2
    } else {
        LOW[(draw() & 1) as usize]
    }
}

/// The blend's pick (`0xf5`, `FUN_001978b0`): the alternate (left-handed)
/// record when the left hand's bone is nearer the ball than the right's.
/// The post-up steal picks by the stance instead (`99`).
pub fn record(script: u8, pose: Option<&Pose>, ball: [f32; 3], stance: bool) -> u16 {
    let s = SCRIPTS[usize::from(script)];
    if script == POST_UP {
        return s.records[usize::from(stance)];
    }
    let Some(pose) = pose else {
        return s.records[0];
    };
    let d = |b: usize| {
        let p = pose.joints[b].position;
        pose::length([p[0] - ball[0], p[1] - ball[1], p[2] - ball[2]])
    };
    let left_nearer = d(pose::LEFT_HAND) < d(pose::RIGHT_HAND);
    s.records[usize::from(left_nearer)]
}

impl crate::World {
    /// Where `i` stands for posing: his feet and height, his facing word.
    pub fn place(&self, i: usize) -> Place {
        let p = &self.players[i];
        Place {
            position: [p.position[0], p.height, p.position[1]],
            word: crate::timing::facing_word(p.heading),
        }
    }

    /// The clip `i`'s gait channel plays and its frame.
    pub fn gait_frame(&self, i: usize) -> Frame {
        if let Some(s) = self.players[i].stand
            && let Some(record) = s.run.shown().record
        {
            return Frame {
                record,
                time: s.run.shown().frame,
                looping: true,
            };
        }
        let c = &self.players[i].gait_channel;
        Frame {
            record: c.row,
            time: c.phase,
            looping: true,
        }
    }

    /// The swipe a steal press by `i` starts (`0x15e0e4..0x15e360`), with
    /// the disc's animations; None without them.
    pub(crate) fn start_swipe(&mut self, i: usize) -> Option<Swipe> {
        let poses = self.poses.arc()?;
        let from = self.gait_frame(i);
        let body = self.players[i].body();
        let pose = poses.pose(from, body, self.place(i));
        let stance = POST_STANCE.contains(&from.record);
        let holder = match self.ball.state {
            crate::BallState::Held(h) => Some(h),
            _ => None,
        };
        let (script, ball) = match holder {
            None => (LOW[(self.rng.next_u32() & 1) as usize], self.ball.position),
            Some(h) => {
                // The ball where the last ball update left it (ball+0x30).
                let ball = self.ball.position;
                let script = if stance || self.behind_on_the_post(i, h) {
                    POST_UP
                } else {
                    let pelvis = pose
                        .as_ref()
                        .map_or(ball[1], |p| p.joints[pose::PELVIS].position[1]);
                    by_height(pelvis, ball[1], || self.rng.next_u32())
                };
                (script, ball)
            }
        };
        // The gait's cycle held at the frame it had reached.
        let held = poses
            .frames(from.record)
            .map_or(from.time, |n| from.time.rem_euclid(n as f32));
        Some(Swipe {
            script,
            record: record(script, pose.as_ref(), ball, stance),
            from: Some((from.record, held)),
            tick: 0,
            tested: false,
        })
    }

    /// The post-up steal's place (`0x15e124..0x15e208`): `i` within 5 of
    /// holder `h`, in the eighths behind him (`FUN_0018d9f0(h, i)` 0, 1 or
    /// 7), facing him within `0xc8` (`FUN_0018d1f8(i, h)`) while `h` faces
    /// `400..0x270` off the line to `i` (`FUN_0018d1f8(h, i)`).
    fn behind_on_the_post(&self, i: usize, h: usize) -> bool {
        let (pi, ph) = (self.players[i].position, self.players[h].position);
        if crate::math::len(pi[0] - ph[0], pi[1] - ph[1]) > 5.0 {
            return false;
        }
        let octant = self.octant(h, i);
        let off = |a: usize, b: usize| {
            (self.facing_units(a)
                - crate::contact::angle(self.players[a].position, self.players[b].position))
                & 0x3ff
        };
        let (toward, back) = (off(i, h) as u32, off(h, i) as u32);
        (octant < 2 || octant == 7)
            && (toward < 0xc9 || toward.wrapping_sub(0x338) < 0xc9)
            && back.wrapping_sub(400) < 0xe1
    }

    /// `FUN_0015da28`'s swipe test for `i` this update, after its presses
    /// and before his channel steps: while the script's bit 6 is set, from
    /// the update after the press (the script's first step sets it) to
    /// the one after its last (that update's step clears it), on the clock
    /// the last update left, until the hand has been in reach once.
    pub(crate) fn swipe_due(&mut self, i: usize) {
        let crate::Action::Stealing {
            counter: false,
            swipe: Some(s),
            ..
        } = self.players[i].action
        else {
            return;
        };
        if s.tick == 0 || s.tested {
            return;
        }
        // `0x15e4f8..0x15e564`: no test while actor+0x1a8 is set: a bar
        // from touching the ball, or this update's own (off balance;
        // behind his own board within 4.5 of the offence's rim).
        let p = &self.players[i];
        let side = self.sides()[p.team.min(1)];
        let rim = self.rim_spot(crate::target_hoop(self.possession));
        if p.barred > 0
            || self.down(i)
            || (crate::block::behind_own_board(p.position[0], p.position[1], side)
                && crate::distance(p.position, rim) <= 4.5)
        {
            return;
        }
        let last = Swipe {
            tick: s.tick - 1,
            ..s
        };
        if self.swipe_test(i, last)
            && let crate::Action::Stealing {
                swipe: Some(swipe), ..
            } = &mut self.players[i].action
        {
            swipe.tested = true;
        }
    }

    /// One update of `i`'s swipe script, after the side's test: the clock
    /// steps; his facing turns 46 an update toward the press's (at the
    /// holder: `FUN_00168ea8`); through the blend he stands where the press
    /// left him, after it the clip's root moves him (`FUN_0019a658`, as a
    /// trick's: the played captures' swipers keep actor+0x40 at 0 and move
    /// by the clip, up to 0.2 an update). The update after the script's
    /// last is its end (`9c 40`), which the step makes
    /// ([`crate::Player::step`]).
    pub(crate) fn swipe_tick(&mut self, i: usize) {
        let crate::Action::Stealing {
            counter: false,
            swipe: Some(s),
            ..
        } = &mut self.players[i].action
        else {
            return;
        };
        let s = *s;
        let p = &mut self.players[i];
        let now = crate::timing::facing_word(p.heading);
        let d = crate::guard::turn(crate::timing::facing_word(p.wanted), now)
            .clamp(-crate::guard::TURN, crate::guard::TURN);
        if d != 0 {
            p.heading = crate::restart::heading_radians((now + d) & 0x3ff);
        }
        if let Some(poses) = self.poses.arc()
            && s.tick > 0
        {
            let shown = |s: Swipe| crate::trick::Shown {
                record: s.shown().record,
                frame: if s.blend().is_some() { 0.0 } else { s.frame() },
                blend: s.blend(),
            };
            let before = Swipe {
                tick: s.tick - 1,
                ..s
            };
            let before = crate::trick::Shown {
                record: Some(s.record),
                ..shown(before)
            };
            let step = crate::trick::root_step(&poses, before, shown(s));
            let p = &mut self.players[i];
            let m =
                crate::trick::root_motion(step, p.body(), crate::timing::facing_word(p.heading));
            p.position[0] += m[0];
            p.position[1] += m[1];
            p.clamp_to_court();
        }
        if let crate::Action::Stealing {
            swipe: Some(swipe), ..
        } = &mut self.players[i].action
        {
            swipe.tick += 1;
        }
    }

    /// One update of a swipe's test (`FUN_0016cf30`) on clock `s`, the
    /// swiper's place and speed as the last update left them: nothing
    /// without a holder; the roll drawn; the hand in reach (the answer)
    /// uses the swipe up and tries the strip.
    fn swipe_test(&mut self, i: usize, s: Swipe) -> bool {
        let crate::BallState::Held(h) = self.ball.state else {
            return false;
        };
        // The rewrite's own: a swipe never takes the ball from a teammate.
        if self.players[h].team == self.players[i].team {
            return false;
        }
        let Some(poses) = self.poses.arc() else {
            return false;
        };
        let body = self.players[i].body();
        let (place, moving) = (self.place(i), self.players[i].moving());
        let Some(pose) = s.pose(&poses, body, place) else {
            return false;
        };
        let shown = s.shown();
        let state = shown.record.map_or(0, |r| poses.state(r, shown.frame));
        // The ball where the last ball update left it (ball+0x30).
        let ball = self.ball.position;
        let d = distance(&pose, shown, state, body, ball);
        let roll = (self.rng.next_u32() % 100) as i32;
        if d > reach(moving, shown.record) {
            return false;
        }
        self.swipe_lands(i, h, roll, moving);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_swipe_bit_spans_the_captured_updates() {
        // The first attract capture's whole swipes: 27 (low out), 32 (low
        // in, middle), 30 (high), 40 (post-up).
        let spans: Vec<u16> = (0..5)
            .map(|script| {
                Swipe {
                    script,
                    record: 0,
                    from: None,
                    tick: 0,
                    tested: false,
                }
                .span()
            })
            .collect();
        assert_eq!(spans, [27, 32, 30, 32, 40]);
    }

    #[test]
    fn the_clip_waits_to_its_frame_after_the_blend() {
        let mut s = Swipe {
            script: 0,
            record: 389,
            from: Some((65, 7.5)),
            tick: 0,
            tested: false,
        };
        assert_eq!(s.blend(), Some(0.2));
        assert_eq!(s.shown().record, Some(65));
        assert_eq!(s.shown().frame, 7);
        s.tick = 4;
        assert_eq!(s.blend(), Some(1.0));
        s.tick = 5;
        assert_eq!(s.blend(), None);
        // The capture's frame words 1..8 on updates 5..12 (23 over 22).
        let frames: Vec<i32> = (5..13)
            .map(|tick| Swipe { tick, ..s }.shown().frame)
            .collect();
        assert_eq!(frames, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(Swipe { tick: 26, ..s }.frame(), 23.0);
    }

    #[test]
    fn the_height_picks_the_script() {
        assert_eq!(by_height(4.5, 5.9, || unreachable!()), 3);
        assert_eq!(by_height(4.5, 6.1, || unreachable!()), 2);
        assert_eq!(by_height(4.5, 2.0, || 0), 1);
        assert_eq!(by_height(4.5, 2.0, || 1), 0);
        assert_eq!(reach(false, Some(398)), 1.5 * 0.05);
        assert_eq!(reach(true, Some(390)), 0.75);
    }
}
