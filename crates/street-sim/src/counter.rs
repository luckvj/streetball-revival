//! The trick counter on its scripts. Traced raw 2026-10-17 (docs/TRACE.md
//! "The turbo bars, the juke press and the counter stance").
//!
//! The press (`FUN_0015da28`, raw `0x15df60..0x15e0e4`): steal (pad
//! +0x7c & 0x200) with two turbos held (+0x78 & 0x40000), on the floor,
//! not swiping nor already in the stance, without the No Counters cheat
//! (`FUN_00131208()+0x78`), and with [`crate::turbo::MOVE`] of the bar,
//! which it spends. One of the two stances of `0x3d7228` by `r & 1`
//! ([`SCRIPTS`] 0 and 1), then the face (`FUN_00159190(actor, 1 or 2 by
//! r & 1, fps × 2)`, which draws once more).
//!
//! The stance's script: the velocity zeroed (`97`), the state bit 16
//! (actor+0xa6 bit 0, `FUN_0016e9b0`), a blend of five updates to
//! `STEALCOUNTER_1_2` (402) or `STEALCOUNTER_2_5` (403) from frame 0, the
//! rest of the clip at 1.25 updates a frame (`8c`: 72 and 62 updates), the
//! bit cleared and the gait back (callback 1): 77 and 67 updates. Its
//! first `91` (`02 00 82 00`) leaves the channel's 0x1000 clear, so the
//! clip's root carries him (a lunge of about 2 forward and back); the
//! stick no longer steers (channel flag 2 keeps `FUN_00163d40` off), and
//! his facing turns to the one he pressed with (actor+0xb0), 46 an update.
//! His bones are the clip's ([`World::scripted_pose`]), so his head
//! reaches toward the handler in the contact's radii
//! (`FUN_001337e0`).
//!
//! The contact (`FUN_001343e8`, raw `0x134ac8..0x134f48`, the plain
//! contact of a pair within their radii): a defender in the stance
//! against a handler in a trick (bit 10) whose channel 0 is not blending
//! (0x200000: the trick's opening `8e`), holding the ball, in the
//! defender's front sectors 3..5. The odds: Steals less the handler's
//! Handles, kept to ±50, plus 50, times 0.8 (truncated), plus 20; in the
//! Trick Counter lesson (vt+0xb4 10) it always lands, without the draw.
//! `r % 100` over the odds misses: the stance ends (bit 16 cleared), the
//! defender plays [`crate::trick::REACTIONS`]' miss (`0x44bf94`,
//! `standstumbleback`, 27 updates) and the contact's message
//! (`0x3c6910`) tells his AI the handler screened him (AI+0xb0). Else the
//! handler plays the countered stumble (`0x44bf61`, 68 updates) and his
//! AI hears the defender screened him; the handler's face (`r & 1`, then
//! the face's own draw), the take-or-knock draw (`r % 100` over 100: never
//! the knock), the take (`FUN_0014cb28`), the defender's dribble at 0, his
//! face (two draws), and his take script (`0x3d0fa0`, after one more
//! unused draw; `0x45795d` for a trick on the records that never fake):
//! the stance's bits and the swipe's (0x10040), a blend of five to
//! `STEALCOUNTER_4_4_L` (405, a left-hander's 404) from frame 17 and the
//! rest at 2 a frame (41 updates; he runs about 8 on with the ball), the
//! facing from the pelvis (callback 7) and the gait back. Either way the
//! handler's fake window closes (+0x198).

use crate::trick::{
    self, REACTIONS, Run, Shown, Step, moving_updates, pose, root_motion, root_step, stand_in_step,
};
use crate::{Action, BallState, Event, World, timing};

/// One of the counter's own scripts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Script {
    pub address: u32,
    /// The clip's root change over the script, as
    /// [`trick::Script::root`] (the primary record's, then the
    /// alternate's).
    pub root: [[f32; 2]; 2],
    pub steps: &'static [Step],
}

/// The stances (`0x3d7228`'s two) and the takes (`0x3d0fa0`'s, and
/// `0x45795d` for the records that never fake).
pub const SCRIPTS: [Script; 4] = [
    // STEALCOUNTER_1_2
    Script {
        address: 0x457803,
        root: [
            [f32::from_bits(0xbe10_78ec), f32::from_bits(0x3d4f_ac7b)],
            [f32::from_bits(0xbe10_78ec), f32::from_bits(0x3d4f_ac7b)],
        ],
        steps: &[
            Step::Stop,
            Step::Bits(0x10000),
            Step::Blend {
                n: 5,
                start: 0,
                records: [402, 402],
            },
            Step::PlayOut {
                per_frame: f32::from_bits(0x3fa0_0000),
                to: 57,
                n: 72,
            },
            Step::Clear(0x10000),
            Step::Call(1),
            Step::End,
        ],
    },
    // STEALCOUNTER_2_5
    Script {
        address: 0x45783b,
        root: [
            [f32::from_bits(0x3ca7_c407), f32::from_bits(0xbe07_9ee8)],
            [f32::from_bits(0x3ca7_c407), f32::from_bits(0xbe07_9ee8)],
        ],
        steps: &[
            Step::Stop,
            Step::Bits(0x10000),
            Step::Blend {
                n: 5,
                start: 0,
                records: [403, 403],
            },
            Step::PlayOut {
                per_frame: f32::from_bits(0x3fa0_0000),
                to: 49,
                n: 62,
            },
            Step::Clear(0x10000),
            Step::Call(1),
            Step::End,
        ],
    },
    // STEALCOUNTER_4_4_L (mirrored), a left-hander's twin 404
    Script {
        address: 0x457873,
        root: [
            [f32::from_bits(0x3e66_e32c), f32::from_bits(0xc103_51e3)],
            [f32::from_bits(0xbe66_e32c), f32::from_bits(0xc103_51e3)],
        ],
        steps: &[
            Step::Stop,
            Step::Bits(0x10040),
            Step::Pick {
                n: 5,
                start: 17,
                records: [405, 404],
                by: trick::Select::Handed,
            },
            Step::PlayOut {
                per_frame: f32::from_bits(0x4000_0000),
                to: 35,
                n: 36,
            },
            Step::Clear(0x10040),
            Step::Call(7),
            Step::Call(1),
            Step::End,
        ],
    },
    // TWOPERSON_FREESTYLE_05_1_SHANE_L (mirrored; its `f5` selector plays
    // the primary)
    Script {
        address: 0x45795d,
        root: [
            [f32::from_bits(0x3c1d_7f98), f32::from_bits(0x3fb1_803d)],
            [f32::from_bits(0x3c1d_7f98), f32::from_bits(0x3fb1_803d)],
        ],
        steps: &[
            Step::Stop,
            Step::Bits(0x10040),
            Step::Blend {
                n: 5,
                start: 5,
                records: [401, 401],
            },
            Step::PlayOut {
                per_frame: f32::from_bits(0x3f80_0000),
                to: 25,
                n: 20,
            },
            Step::Clear(0x10040),
            Step::Call(1),
            Step::End,
        ],
    },
];

/// `0x3d7228`: the stance by `r & 1`.
pub const STANCE_TABLE: u32 = 0x3d7228;
/// `0x3d0fa0`: the take's script.
pub const TAKE_TABLE: u32 = 0x3d0fa0;
/// [`SCRIPTS`]' take, and the take after a trick that never fakes.
pub const TAKE: u8 = 2;
pub const TAKE_NO_FAKE: u8 = 3;
/// [`REACTIONS`]' countered handler and missed counter.
pub const COUNTERED: u8 = 26;
pub const MISSED: u8 = 27;
/// The Trick Counter lesson (the game's vt+0xb4, [`World::lesson`]): its
/// counters always land.
pub const LESSON: u8 = 10;

/// The counter stance (or the take after it) under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stance {
    /// Into [`SCRIPTS`].
    pub script: u8,
    pub run: Run,
    /// The clip he played when it started (record and frame), held
    /// through the blend.
    pub from: Option<(u16, f32)>,
    /// The facing he turns to (actor+0xb0).
    pub word: i32,
}

impl Stance {
    pub fn steps(&self) -> &'static [Step] {
        SCRIPTS[usize::from(self.script).min(SCRIPTS.len() - 1)].steps
    }

    /// The take after a counter landed (still bit 16, with the swipe's).
    pub fn take(&self) -> bool {
        self.script >= TAKE
    }
}

impl World {
    /// `FUN_0015da28`'s stance for `i` (`0x15e01c..0x15e09c`): the script
    /// by `r & 1`, then the face's two draws; he turns to the facing he
    /// pressed with (`word`).
    pub(crate) fn start_counter(&mut self, i: usize, word: i32) {
        let script = (self.rng.next_u32() & 1) as u8;
        // `FUN_00159190(actor, 1 or 2, fps × 2)`: the kind by `r & 1`, and
        // the face's own draw.
        self.rng.next_u32();
        self.face(i, 2 * crate::TICK_HZ as i32);
        let from = self.poses.get().map(|_| {
            let f = self.gait_frame(i);
            (f.record, f.time)
        });
        self.players[i].action = Action::Stealing {
            elapsed: 0.0,
            counter: true,
            swipe: None,
            stance: Some(Stance {
                script,
                run: Run::new(false, 0),
                from,
                word,
            }),
        };
    }

    /// One update of `i`'s stance or take (as [`World::reaction_tick`]):
    /// the channel's update, the velocity zeroed, the clip's root, the
    /// turn to the stance's facing, the facing from the pelvis at the
    /// take's callback 7; at the end the gait back.
    pub(crate) fn counter_tick(&mut self, i: usize) {
        let Action::Stealing {
            elapsed,
            counter: true,
            swipe,
            stance: Some(mut s),
        } = self.players[i].action
        else {
            return;
        };
        let steps = s.steps();
        let before = s.run.shown();
        let ran = s.run.step(steps, self.players[i].height);
        let after = s.run.shown();
        let p = &mut self.players[i];
        p.speed = 0.0;
        let now = timing::facing_word(p.heading);
        let d = turn(s.word, now).clamp(-trick::TURN, trick::TURN);
        if d != 0 {
            p.heading = crate::restart::heading_radians((now + d) & 0x3ff);
        }
        let script = &SCRIPTS[usize::from(s.script).min(SCRIPTS.len() - 1)];
        let step = match self.poses.get() {
            Some(poses) => root_step(poses, before, after),
            None if s.run.done => [0.0; 3],
            None => {
                let moving = moving_updates(steps, s.run.left, 0, 0.0);
                let left = usize::from(s.run.handed);
                stand_in_step(script.root[left], moving, after)
            }
        };
        let p = &mut self.players[i];
        let m = root_motion(step, p.body(), timing::facing_word(p.heading));
        p.position[0] += m[0];
        p.position[1] += m[1];
        self.players[i].clamp_to_court();
        if ran.face
            && let Some(poses) = self.poses.get()
            && let Some(pose) = pose(poses, after, s.from, self.players[i].body(), self.place(i))
        {
            self.players[i].heading = crate::restart::heading_radians(trick::pelvis_word(&pose));
        }
        self.players[i].action = if s.run.done {
            Action::None
        } else {
            Action::Stealing {
                elapsed,
                counter: true,
                swipe,
                stance: Some(s),
            }
        };
    }

    /// The contact of defender `i` in the counter stance with handler `h`
    /// (`FUN_001343e8`, raw `0x134ac8..0x134f48`). Returns whether it was
    /// resolved (the push is not made).
    pub(crate) fn counter(&mut self, i: usize, h: usize) -> bool {
        let Action::Stealing {
            stance: Some(s), ..
        } = self.players[i].action
        else {
            return false;
        };
        if s.take() || self.players[h].team == self.players[i].team {
            return false;
        }
        // The handler in a trick (bit 10), his channel 0 not blending, the
        // ball his, in the defender's front sectors.
        let Action::Trick { play, .. } = self.players[h].action else {
            return false;
        };
        if !self.players[h].action.juking()
            || play.run.blend_counter().is_some()
            || self.ball.state != BallState::Held(h)
            || !(3..=5).contains(&self.octant(i, h))
        {
            return false;
        }
        let edge =
            (self.players[i].ratings.native(6) - self.players[h].ratings.native(7)).clamp(-50, 50);
        let odds = ((edge + 50) as f32 * f32::from_bits(0x3f4c_cccd)) as i32 + 20;
        if self.lesson != Some(LESSON) && (self.rng.next_u32() % 100) as i32 > odds {
            // The miss (`0x134ed0`): the stance ends, he stumbles, and his
            // AI hears the handler screened him.
            self.react(i, MISSED);
            self.screened(i, h);
            self.close_window(h);
            return true;
        }
        // The handler's stumble and the message his AI hears.
        let record = play.run.shown().record;
        self.react(h, COUNTERED);
        self.screened(h, i);
        // His face (`r & 1`, then `FUN_00159190(.., fps × 2)`, `0x134c58..
        // 0x134c7c`) and the take-or-knock draw (`r % 100` over 100: the
        // take).
        self.rng.next_u32();
        self.face(h, 2 * crate::TICK_HZ as i32);
        self.rng.next_u32();
        // The take (`FUN_0014cb28`, slot 15's trick counter,
        // [`World::post_steal`]), his dribble at 0, his face (`0x134cc0..
        // 0x134ce4`).
        self.take_ball(i);
        self.rng.next_u32();
        self.face(i, 2 * crate::TICK_HZ as i32);
        let script = if record.is_some_and(|r| trick::NO_FAKE.contains(&r)) {
            TAKE_NO_FAKE
        } else {
            // `0x134e78`: a draw the script does not use.
            self.rng.next_u32();
            TAKE
        };
        let from = self.counter_pose_frame(i, s);
        let handed = self.players[i].left_handed;
        let take = Stance {
            script,
            run: Run::new(false, 0).picking(handed, false),
            from,
            word: timing::facing_word(self.players[i].heading),
        };
        self.players[i].action = Action::Stealing {
            elapsed: 0.0,
            counter: true,
            swipe: None,
            stance: Some(take),
        };
        // `FUN_0019c2f8` runs its first update at once.
        self.counter_tick(i);
        self.events.push(Event::Counter { player: i, from: h });
        true
    }

    /// The clip `i`'s stance shows now (record and frame), held through
    /// the take's blend: the gait's before the stance's blend ends.
    fn counter_pose_frame(&self, i: usize, s: Stance) -> Option<(u16, f32)> {
        self.poses.get()?;
        let shown: Shown = s.run.shown();
        match (shown.record, shown.blend) {
            (Some(record), None) => Some((record, shown.frame)),
            _ => s.from.or_else(|| {
                let f = self.gait_frame(i);
                Some((f.record, f.time))
            }),
        }
    }

    /// The contact's message (`0x3c6910`, `FUN_00141448`): `who`'s AI+0xb0
    /// is `by`.
    fn screened(&mut self, who: usize, by: usize) {
        if let Some(brain) = self.ai.brains.get_mut(who) {
            brain.screened_by = by as i32;
        }
    }

    /// The handler's scripted contact cleared (+0x198 = 0): his trick's
    /// fake window closes.
    fn close_window(&mut self, h: usize) {
        if let Action::Trick { id, mut play } = self.players[h].action {
            play.window = 0;
            self.players[h].action = Action::Trick { id, play };
        }
    }
}

/// `FUN_0018cd78`: `a - b` wrapped to -512..=512.
fn turn(a: i32, b: i32) -> i32 {
    let d = a - b;
    if d.abs() > 0x200 {
        if d < 0 { d + 0x400 } else { d - 0x400 }
    } else {
        d
    }
}

/// [`REACTIONS`]' counter scripts' addresses, for the disc test.
pub fn reaction_addresses() -> [u32; 2] {
    [
        REACTIONS[usize::from(COUNTERED)].address,
        REACTIONS[usize::from(MISSED)].address,
    ]
}
