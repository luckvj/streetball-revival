//! The ball-handler's trick moves as the original plays them, and the
//! defender's reaction when one meets him. Traced raw 2026-10-16
//! (docs/TRACE.md "Trick moves on their scripts").
//!
//! The trick press (`FUN_0015f0f0`, raw `0x15f0f0..0x15f860`; the decomp
//! cuts it at `FUN_0030fa50`) takes the turbo buttons' direct slot (the
//! eleven masks of `0x43d408`, a slot's script `FUN_0013a1f8(record, 1,
//! +0x250, slot)` and its value class), or else draws among the 75 trick
//! scripts of `0x3d7238` (12 bytes a row: the script, a legend, the
//! selection [`flag`]s) those the player may play at value class 0 (no
//! turbo) or 1 (a turbo held), standing or moving, with the nearest
//! defender on the ball's side or the other, not the last trick drawn
//! (`0x3d75bc`). It faces the defender when he stands within ±45° of the
//! facing (actor+0xb0), and starts the script (`FUN_0019c2f8`).
//!
//! Every trick script is the same shape ([`SCRIPTS`]): the state word
//! 0x400 (bit 10), a blend of six or ten updates to the clip (the live
//! left hand's alternate, the `f8` selector), `97` (the velocity zeroed),
//! `a6 3 3 n` (the fake window: contact kind 3 for `n` updates, counted
//! down by `FUN_00156f48`), one wait (`8d ee F T`), the bit cleared, the
//! facing taken from the pelvis (callback 7, `FUN_00154230`) and the gait
//! back (callback 1). Through the blend the handler coasts on the velocity
//! he pressed with; after it the clip's root carries him (`FUN_0019a658`:
//! the change of the root translation, times the model's height scale,
//! turned by his facing, added to his x and z), the stick no longer
//! steering. The self-tosses add the launch (callback 53, [`crate::self_toss`])
//! and the ball's flight.
//!
//! The fake (`FUN_001372d8`, contact kind 3 of the manager `FUN_00133bb0`):
//! while the window is open, an opponent within 6 who is not reacting, not
//! in a counter stance facing the handler and not holding the ball, whose
//! body, legs or arms ([`DEFENDER`]) meet the handler's head, trunk or legs
//! ([`HANDLER`], the segment test `FUN_00139198`), and who stands in the
//! handler's front sectors 3..5, is tried: `r % 100 ≤ lo + trunc((hi − lo)
//! × Handles / 99)` ([`ODDS`] by the value class; 10 more for a computer's
//! trick on the first difficulty) fakes him out (actor+0x218, the full
//! points), else he holds. Either way he plays a reaction ([`HELD`],
//! [`FAKED`], by the quarter the handler stands in about him) and the window
//! closes. The points come at the trick's end (`0x157dd0`): the update bit
//! 10 drops, in live play, with the ball still his.

use crate::pose::{self, Body, Frame, Place, Pose, Poses};

/// One step of a move script as the channel's clock sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// `9b w`: the actor's state word (actor+0xa4) set to `w`.
    Bits(u32),
    /// `9c w`: those bits cleared.
    Clear(u32),
    /// `8e`: a blend of `n` updates to `records[0]` (`records[1]` on the
    /// live left hand, the `f8` selector; other selectors play the
    /// primary, or [`Step::Pick`]) at clip frame `start`. A record of
    /// [`CURRENT`] is the one already on the channel (`-1000`).
    Blend { n: u8, start: u8, records: [u16; 2] },
    /// `8e f2` or `8e ec`: a blend as [`Step::Blend`] whose alternate
    /// `records[1]` the actor's handedness or his side of the court picks
    /// (`FUN_001978b0`, [`Select`]).
    Pick {
        n: u8,
        start: u8,
        records: [u16; 2],
        by: Select,
    },
    /// `97`: the velocity zeroed (`FUN_001947a0`).
    Stop,
    /// `a6 3 row n`: the fake window, contact kind 3 with row `row` of
    /// `0x3d0d90`, for `n` updates.
    Window { row: u8, n: u8 },
    /// `a6 2 row n`: the clearout's window, contact kind 2 (`FUN_00136290`)
    /// with row `row` of `0x3d0d90`, for `n` updates.
    Shove { row: u8, n: u8 },
    /// `a6 1 row n`: a dunk's or lay-up's window, contact kind 1
    /// (`FUN_001366d8`) with row `row` of `0x3d0d90`, for `n` updates.
    Knock { row: u8, n: u8 },
    /// `8d ee F T`: the clip to frame `to` over `n` updates.
    Wait { to: u8, n: u8 },
    /// `8c x`: the rest of the clip (to frame `to`) at a frame each
    /// `per_frame` updates, `n` updates.
    PlayOut { per_frame: f32, to: u8, n: u16 },
    /// `8d ee F fe`: the clip to frame `to` by the landing.
    Land { to: u8 },
    /// `8d ee F ef 0`: the clip to frame `to` by the ball's flight (the
    /// self-toss's catch).
    Flight { to: u8 },
    /// `a1 81 a v w`: a hop (`FUN_00194d58`'s default): `speed` along the
    /// facing turned by `turn` units and `rise` up, units an update at 60
    /// Hz, landing as the jump opcode's (`FUN_0019bed8`).
    Hop { turn: i16, speed: f32, rise: f32 },
    /// `83 id`: a game callback (1 the gait back, 4 the post-up's, 7 the
    /// facing from the pelvis, 53 the self-toss launch).
    Call(u8),
    /// `84`/`85 id`: a callback that decides a jump; the plain path.
    Test(u8),
    /// `81`.
    End,
    /// The jumps' own opcodes, on the shot, dunk and lay-up scripts
    /// ([`crate::jump`]). `91 w` (`FUN_00195858`): channel 0's flags, its
    /// low half `w`'s and its high half or'ed in.
    Flags(u32),
    /// `9f v` (`FUN_00194c40`): up at `v` units an update (the velocity's
    /// y; its x and z kept).
    Rise(f32),
    /// `a1 f0 a v w` (`FUN_00194d58`): a hop toward the rim his team
    /// attacks (`FUN_0016f260`) turned by `turn` units, `speed` along it and
    /// `rise` up, units an update at 60 Hz.
    Toward { turn: i16, speed: f32, rise: f32 },
    /// `a1 85 a v w` (`FUN_00194d58`, raw `0x194f0c..0x195378`): the
    /// dive's hop, `rise` up and at most `speed` along the line to where
    /// the loose ball will be when he comes down ([`crate::dive`]), units
    /// an update at 60 Hz.
    Dive { speed: f32, rise: f32 },
    /// `a0 f` (`FUN_001957d8`): the slide's deceleration (actor+0x184, a
    /// negative `f` the default 0.5), which the motion step takes off his
    /// speed each update until he stops (`0x157530..0x1575d0`).
    Slide(f32),
    /// `9e` (`FUN_001988b0`): the dunk's aimed jump.
    Aim(crate::jump::Aim),
    /// `ac` (`FUN_001998f0`): the lay-up's reach.
    Reach(crate::jump::Reach),
    /// `aa k` (`FUN_00194848`): the velocity `k` times the last update's
    /// displacement (actor+0x220).
    Carry(f32),
    /// Callback 4 (`FUN_00175248`): the wanted facing (actor+0xb0) toward
    /// the rim his team attacks (`FUN_0016f260`) plus its word.
    Want(i16),
    /// `93` (`FUN_00193780`, the hands, channel flags 0xc0000: no gravity)
    /// or `95` (`FUN_00194228`, the feet, 0x80000): a planted point, the
    /// midpoint of `bones` or the one the `pick` selector takes
    /// ([`crate::jump::Plant`]).
    Plant(crate::jump::Plant),
    /// `96` (`FUN_00194780`, flag 0x2000000): the planted points let go at
    /// the actor's next motion step (`FUN_00156f48`, `0x157418`).
    Unplant,
    /// `98 r -> T` over a pair of branches (`FUN_00196068`): the channel
    /// playing `record` runs the `alternate` steps after the `plain` ones,
    /// else the `plain` ones and skips the others.
    Either {
        record: u16,
        plain: u8,
        alternate: u8,
    },
    /// Callback 28 (`FUN_00172400`) deciding a dunker's hang on the rim
    /// (`85 28` falls through into it, `84 28` jumps to it): when he hangs
    /// ([`Run::hang`]) the `hang` steps that follow run and the `other`
    /// ones after them are skipped; else the `hang` steps are skipped.
    /// Either path ends at its `End` or goes on with the steps after both.
    Hang { hang: u8, other: u8 },
    /// `ab x` (`FUN_00197234`): the clip from its frame on at a frame each
    /// `per_frame` updates, wrapping at its `frames`, for good.
    Loop { per_frame: f32, frames: u8 },
    /// `82 T`: the steps go on at step `to`.
    Jump(u8),
    /// `85 3 n -> T` (callback 3, `FUN_00172288`: the C library's
    /// `rand() % n`): on a draw of 0 the steps go on at step `to`.
    Roll { n: u8, to: u8 },
    /// `84 22 -> T` (callback 22, `FUN_00174e08`: the live hand is the
    /// right, actor+0x25c 1): with the right hand the steps go on at step
    /// `to`.
    RightHand(u8),
    /// `98 r.. -> T` read as a jump (`FUN_00196068`): with the channel
    /// playing one of `records` (0 for none; before the first blend the
    /// clip the script started on, [`Run::before`]) the steps go on at
    /// step `to`.
    OnRecord { records: [u16; 12], to: u8 },
    /// `84 10`, `84 45` or `84 49 -> T`: a test the script's starter
    /// answers as it starts ([`Run::asked`]): callback 10 (`FUN_001757e8`,
    /// the post call's side), callback 45 (`FUN_00177750`, the backdown's
    /// turn), callback 49 (`FUN_00177fa0`, the post defence's side).
    Asked(u8),
}

/// What a [`Step::Pick`] blend's alternate turns on (`FUN_001978b0`'s
/// selectors, raw `0x197974..0x197a58`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Select {
    /// `f2`: actor+0xc0 is 2, a player whose record's field 13 is set
    /// (`FUN_001563a8`, `0x156648`): a left-hander.
    Handed,
    /// `ec`: his z against the side his team attacks (`FUN_001404e0`'s
    /// sign): the alternate when they differ (z × side below 0).
    Side,
}

/// A blend's target record meaning the record already on the channel
/// (`-1000`, `0xfffffc18`).
pub const CURRENT: u16 = u16::MAX;

/// One trick script of `0x3d7238`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Script {
    pub address: u32,
    /// The row's selection flags (+8, [`flag`]).
    pub flags: u8,
    /// The ordinary players' availability (`FUN_00139850`'s row): 1 the
    /// plain draw, 2 the turbo draw, `1 << class` a direct slot's.
    pub mask: u8,
    /// The clip's root change over the script (x and z at scale 1, the
    /// clip's frame), for the right-hand clip and the left-hand one: the
    /// stand-in for the root's motion without the disc's animations.
    pub root: [[f32; 2]; 2],
    pub steps: &'static [Step],
}

/// A defender's reaction script (`0x3d0f18`, `0x3d0f48`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reaction {
    pub address: u32,
    /// The clips' root change over the script, as [`Script::root`].
    pub root: [f32; 2],
    pub steps: &'static [Step],
}

/// The selection flags of a trick row.
pub mod flag {
    /// Drawn with the defender on the ball's side.
    pub const BALL_SIDE: u8 = 1;
    /// Drawn with him on the other side.
    pub const OTHER_SIDE: u8 = 2;
    /// Standing (without 8: only standing).
    pub const STANDING: u8 = 4;
    /// Moving (without 4: only moving).
    pub const MOVING: u8 = 8;
    /// Only posting up (actor bit 11).
    pub const POST_UP: u8 = 0x10;
}

/// A [`crate::player::TrickIdentity`]'s slot for a drawn trick.
pub const DRAWN: u8 = u8::MAX;

/// The ordinary players' direct slots (`0x43d498`'s first eleven, the row
/// `FUN_00139850` builds; `FUN_0013a1f8` for a record that is neither a
/// created player nor the generic team's, nor a legend's slot 1).
pub const SLOTS: [u8; 11] = [20, 17, 19, 22, 12, 16, 56, 57, 63, 66, 69];

/// The fakeout's odds by value class: `lo + trunc((hi − lo) × Handles /
/// 99)` percent (`0x43d0e0`, `0x43d0f8`).
pub const ODDS: [(i32, i32); 5] = [(25, 45), (30, 55), (35, 65), (40, 75), (45, 85)];

/// What a person's trick adds to the odds on the first difficulty
/// (`0x137650..0x137674`: `FUN_00172220` false, so not the computer's
/// controller, and `FUN_001e0bc0()+0x12c & 3` zero, Got Game).
pub const PERSON_ODDS: i32 = 10;

/// The kind-3 rows of `0x3d0d90` (3 the trick moves', 4 the post-up's):
/// the handler's capsules, bones `a` to `b` pushed `reach` past `b`
/// (`FUN_00137ab0`), `radius` about them (`0x3d0cf0`, `0x3d0d30`).
pub const HANDLER: [(usize, usize, f32, f32); 4] = [
    (20, 21, 1.5, f32::from_bits(0x3f59_999a)),
    (20, 0, 1.5, 1.0),
    (0, 8, 1.0, 0.5),
    (0, 4, 1.0, 0.5),
];

/// The other player's capsules (`FUN_001392b8`, `0x3d1180`'s three
/// lists): the body, the legs, the arms.
pub const DEFENDER: [&[(usize, usize, f32, f32)]; 3] = [
    &[(0, 20, 0.7, 1.5)],
    &[
        (1, 2, 0.4, 0.0),
        (2, 3, 0.3, 0.0),
        (5, 6, 0.4, 0.0),
        (6, 7, 0.3, 0.0),
    ],
    &[
        (13, 14, 0.3, 0.0),
        (14, 15, 0.3, 0.0),
        (17, 18, 0.3, 0.0),
        (18, 19, 0.3, 0.0),
    ],
];

/// `FUN_00139408`: the capsules are tried only within this.
pub const REACH: f32 = 6.0;

/// Without the disc's animations, the rewrite's stand-in for the capsules:
/// the floor distance within which the handler's capsules meet a
/// defender's (the median of the attract captures' trick contacts, which
/// met at 1.5 to 5.2).
pub const STAND_IN_REACH: f32 = 3.5;

/// Records whose trick never fakes (`FUN_001372d8`: the window closes).
pub const NO_FAKE: [u16; 12] = [
    0xca, 0xcb, 0xde, 0xdf, 0xea, 0xeb, 0xec, 0xed, 0x10e, 0x10f, 0x2b0, 0x2b1,
];

/// Records whose fakeout from in front (quarter 2) plays [`STEP_THROUGH`]
/// on the defender.
pub const STEP_THROUGH_RECORDS: [u16; 6] = [0xbc, 0xbd, 0x106, 0x107, 0x108, 0x109];

/// The trick's turn toward its facing, units an update (as callback 5's).
pub const TURN: i32 = 46;

/// The channel's clock running a script: `playback::Channel`'s, over
/// [`Step`]s (`FUN_00193440`, `FUN_0019a658`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    /// The next step.
    pub pc: u8,
    /// Updates until the steps run again (`+0x4c`).
    pub(crate) count: i8,
    /// Updates since the script started (the first update is 0).
    pub tick: i16,
    /// `+0x50`: the record the channel plays (none before the first blend
    /// ends: the actor's own clip).
    pub record: Option<u16>,
    /// `+0x64` and `+0x5c`.
    pub frame: f32,
    pub(crate) int_frame: i16,
    pub(crate) span: Option<Span>,
    /// The tick the actor lands from a [`Step::Hop`] (`actor+0x64`).
    pub landing: i16,
    /// The self-toss's flight in updates ([`Step::Flight`]).
    pub flight: u8,
    /// The live left hand: blends take their `f8` alternate.
    pub left: bool,
    /// A left-hander: [`Select::Handed`] blends take their alternate.
    pub handed: bool,
    /// His z on the other side from the one his team attacks:
    /// [`Select::Side`] blends take their alternate.
    pub side: bool,
    pub done: bool,
    /// The tick the actor lands, as his flight knows it ([`crate::jump`]):
    /// a landing wait (`8d .. fe`) lasts until then; -1 to use the hop's.
    pub land_at: i16,
    /// An [`Step::Either`]'s plain branch under way: at the first step the
    /// clock goes on from the second.
    pub(crate) join: Option<(u8, u8)>,
    /// A [`Step::Hang`]'s hang under way, as [`Run::join`] (an
    /// [`Step::Either`] may run inside it).
    pub(crate) outer: Option<(u8, u8)>,
    /// Callback 28's answer for this move: the dunker hangs on the rim
    /// ([`Step::Hang`]); none until it is asked ([`Ran::hang_test`]).
    pub hang: Option<bool>,
    /// The answer to the script's [`Step::Asked`] test.
    pub asked: bool,
    /// The record the channel played as the script started, which
    /// [`Step::OnRecord`] reads before the first blend.
    pub before: Option<u16>,
}

/// A span the channel carries (a blend, a wait or a play-out).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Span {
    pub(crate) blend: bool,
    pub(crate) length: u16,
    pub(crate) counter: u16,
    pub(crate) rate: f32,
    pub(crate) to: u8,
    pub(crate) to_record: Option<u16>,
    /// A [`Step::Loop`]'s clip length (its frames wrap there), 0 for a
    /// span that ends.
    pub(crate) frames: u8,
}

/// What one update of a script ran.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ran {
    /// `97`.
    pub stop: bool,
    /// `a6`: the window's row and updates.
    pub window: Option<(u8, u8)>,
    /// `a6 2`: the clearout's window, its row and updates.
    pub shove: Option<(u8, u8)>,
    /// `a6 1`: a dunk's or lay-up's window, its row and updates.
    pub knock: Option<(u8, u8)>,
    /// `a1`: the hop's turn, speed and rise.
    pub hop: Option<(i16, f32, f32)>,
    /// Callback 53: the self-toss launch.
    pub launch: bool,
    /// Callback 7: the facing from the pelvis.
    pub face: bool,
    /// `9c 0x400`: bit 10 cleared.
    pub cleared: bool,
    /// `9b`/`9c`: the state bits (actor+0xa4) set and cleared.
    pub set_bits: u32,
    pub clear_bits: u32,
    /// Callback 5 (`FUN_00174eb0`): the facing (+0xb0) toward the ball.
    pub toward_ball: bool,
    /// Callback 0 (`FUN_001775d8`): the scoop's take.
    pub take: bool,
    /// The jumps' steps this update ran, in order ([`crate::jump`]).
    pub ops: crate::jump::Ops,
    /// What the channel showed after the span's step, before the steps
    /// ran (the root's motion splits there: the steps change the flags).
    pub mid: Option<Shown>,
    /// A landing wait started.
    pub land: bool,
    /// A [`Step::Hang`] met with [`Run::hang`] unanswered (its hang
    /// skipped): callback 28 is due this update.
    pub hang_test: bool,
}

/// What the channel shows: the record and frame (a blend's incoming clip
/// at its start) and the blend's weight while it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub record: Option<u16>,
    pub frame: f32,
    pub blend: Option<f32>,
}

impl Run {
    /// A script about to start: the steps due in its first update.
    pub fn new(left: bool, flight: u8) -> Self {
        Self {
            pc: 0,
            count: 1,
            tick: -1,
            record: None,
            frame: 0.0,
            int_frame: -1,
            span: None,
            landing: -1,
            flight,
            left,
            handed: false,
            side: false,
            done: false,
            land_at: -1,
            join: None,
            outer: None,
            hang: None,
            asked: false,
            before: None,
        }
    }

    /// The [`Step::Pick`] blends' choices: a left-hander, and his side.
    pub fn picking(self, handed: bool, side: bool) -> Self {
        Self {
            handed,
            side,
            ..self
        }
    }

    /// A blend under way: its counter (`+0x70`, 1 on the update it
    /// starts) and length (`+0x6c`).
    pub fn blend_counter(&self) -> Option<(u16, u16)> {
        self.span
            .filter(|s| s.blend)
            .map(|s| (s.counter, s.length.max(1)))
    }

    /// One update of `steps`; `height` the actor's, for a hop's landing.
    pub fn step(&mut self, steps: &[Step], height: f32) -> Ran {
        self.step_rolling(steps, height, &mut || 1)
    }

    /// [`Run::step`] with `roll` the C library's `rand()` for the
    /// [`Step::Roll`]s.
    pub fn step_rolling(
        &mut self,
        steps: &[Step],
        height: f32,
        roll: &mut dyn FnMut() -> u32,
    ) -> Ran {
        self.tick = self.tick.saturating_add(1);
        let mut ran = Ran::default();
        if self.done {
            return ran;
        }
        self.count -= 1;
        if self.count > 0 {
            return ran;
        }
        if self.span.is_some() && !self.advance() {
            return ran;
        }
        ran.mid = Some(self.shown());
        self.run(steps, height, &mut ran, roll);
        ran
    }

    /// The span's step (`FUN_0019a658(-1)`); true when the steps run on.
    fn advance(&mut self) -> bool {
        let Some(span) = &mut self.span else {
            return true;
        };
        if span.frames > 0 {
            // `ab`: on at its rate, wrapping at the clip's end.
            self.frame += span.rate;
            let n = f32::from(span.frames);
            if self.frame >= n {
                self.frame -= n;
            }
            self.int_frame = self.frame as i16;
            self.count = 1;
            return false;
        }
        span.counter += 1;
        if span.counter > span.length {
            if span.blend {
                self.record = span.to_record;
            }
            self.frame = f32::from(span.to);
            self.int_frame = i16::from(span.to);
            self.span = None;
            self.count = 0;
            return true;
        }
        if !span.blend {
            self.frame += span.rate;
            if span.counter == span.length {
                self.frame = f32::from(span.to);
            }
            self.int_frame = self.frame as i16;
        }
        self.count = 1;
        false
    }

    /// `8e` (`FUN_001978b0`): the span to `record` at `start` over `n`
    /// updates ([`CURRENT`] the record on the channel).
    fn blend(&mut self, n: u8, start: u8, record: u16) -> bool {
        let to_record = if record == CURRENT {
            self.record
        } else {
            Some(record)
        };
        self.span = Some(Span {
            blend: true,
            length: u16::from(n),
            counter: 0,
            rate: 0.0,
            to: start,
            to_record,
            frames: 0,
        });
        self.advance()
    }

    /// `ab` (`FUN_00197234`): the clip on from its frame for good.
    fn looping(&mut self, per_frame: f32, frames: u8) -> bool {
        self.frame = f32::from(self.int_frame.max(0));
        self.span = Some(Span {
            blend: false,
            length: u16::MAX,
            counter: 0,
            rate: 1.0 / per_frame.max(1e-3),
            to: 0,
            to_record: None,
            frames: frames.max(1),
        });
        self.advance()
    }

    fn wait(&mut self, to: u8, length: u16, rate: Option<f32>) -> bool {
        let delta = if self.int_frame < 0 {
            f32::from(to)
        } else {
            f32::from(to) - f32::from(self.int_frame)
        };
        let length = length.max(1);
        self.frame = f32::from(self.int_frame.max(0));
        self.span = Some(Span {
            blend: false,
            length,
            counter: 0,
            rate: rate.unwrap_or(delta / f32::from(length)),
            to,
            to_record: None,
            frames: 0,
        });
        self.advance()
    }

    fn run(&mut self, steps: &[Step], height: f32, ran: &mut Ran, roll: &mut dyn FnMut() -> u32) {
        loop {
            loop {
                if let Some((at, to)) = self.join
                    && self.pc == at
                {
                    self.pc = to;
                    self.join = None;
                } else if let Some((at, to)) = self.outer
                    && self.pc == at
                {
                    self.pc = to;
                    self.outer = None;
                } else {
                    break;
                }
            }
            let Some(&step) = steps.get(usize::from(self.pc)) else {
                self.done = true;
                return;
            };
            self.pc += 1;
            let on = match step {
                Step::End => {
                    self.done = true;
                    return;
                }
                Step::Bits(w) => {
                    ran.set_bits |= w;
                    true
                }
                Step::Test(_) => true,
                Step::Clear(w) => {
                    ran.cleared |= w & 0x400 != 0;
                    ran.clear_bits |= w;
                    true
                }
                Step::Call(5) => {
                    ran.toward_ball = true;
                    true
                }
                Step::Call(0) => {
                    ran.take = true;
                    true
                }
                Step::Call(7) => {
                    ran.face = true;
                    true
                }
                Step::Call(53) => {
                    ran.launch = true;
                    true
                }
                Step::Call(18) => {
                    // The side fades' jump (`FUN_00172788`).
                    ran.ops.push(step);
                    true
                }
                Step::Call(_) => true,
                Step::Stop => {
                    ran.stop = true;
                    ran.ops.push(step);
                    true
                }
                Step::Window { row, n } => {
                    ran.window = Some((row, n));
                    true
                }
                Step::Shove { row, n } => {
                    ran.shove = Some((row, n));
                    true
                }
                Step::Knock { row, n } => {
                    ran.knock = Some((row, n));
                    true
                }
                Step::Hop { turn, speed, rise } => {
                    ran.hop = Some((turn, speed, rise));
                    ran.ops.push(step);
                    self.landing = self.tick + i16::from(crate::timing::airtime(rise, height));
                    true
                }
                Step::Dive { rise, .. } => {
                    ran.ops.push(step);
                    self.landing = self.tick + i16::from(crate::timing::airtime(rise, height));
                    true
                }
                Step::Blend { n, start, records } => {
                    self.blend(n, start, records[usize::from(self.left)])
                }
                Step::Pick {
                    n,
                    start,
                    records,
                    by,
                } => {
                    let alternate = match by {
                        Select::Handed => self.handed,
                        Select::Side => self.side,
                    };
                    self.blend(n, start, records[usize::from(alternate)])
                }
                Step::Wait { to, n } => self.wait(to, u16::from(n), None),
                Step::PlayOut { per_frame, to, n } => {
                    self.wait(to, n, Some(1.0 / per_frame.max(1e-3)))
                }
                Step::Land { to } => {
                    ran.land = true;
                    let at = if self.land_at >= 0 {
                        self.land_at
                    } else {
                        self.landing
                    };
                    let n = if at >= 0 {
                        (at - self.tick).max(1) as u16
                    } else {
                        1
                    };
                    self.wait(to, n, None)
                }
                Step::Flight { to } => self.wait(to, u16::from(self.flight), None),
                Step::Loop { per_frame, frames } => self.looping(per_frame, frames),
                Step::Jump(to) => {
                    self.pc = to;
                    true
                }
                Step::RightHand(to) => {
                    if !self.left {
                        self.pc = to;
                    }
                    true
                }
                Step::Asked(to) => {
                    if self.asked {
                        self.pc = to;
                    }
                    true
                }
                Step::OnRecord { records, to } => {
                    let playing = self.record.or(self.before);
                    if playing.is_some_and(|r| r != 0 && records.contains(&r)) {
                        self.pc = to;
                    }
                    true
                }
                Step::Roll { n, to } => {
                    if roll().is_multiple_of(u32::from(n.max(1))) {
                        self.pc = to;
                    }
                    true
                }
                Step::Either {
                    record,
                    plain,
                    alternate,
                } => {
                    if self.record == Some(record) {
                        self.pc += plain;
                    } else {
                        self.join = Some((self.pc + plain, self.pc + plain + alternate));
                    }
                    true
                }
                Step::Hang { hang, other } => {
                    match self.hang {
                        Some(true) => self.outer = Some((self.pc + hang, self.pc + hang + other)),
                        answer => {
                            ran.hang_test |= answer.is_none();
                            self.pc += hang;
                        }
                    }
                    true
                }
                Step::Flags(_)
                | Step::Rise(_)
                | Step::Toward { .. }
                | Step::Aim(_)
                | Step::Reach(_)
                | Step::Carry(_)
                | Step::Slide(_)
                | Step::Want(_)
                | Step::Plant { .. }
                | Step::Unplant => {
                    ran.ops.push(step);
                    true
                }
            };
            if !on {
                return;
            }
        }
    }

    /// The channel after the last update.
    pub fn shown(&self) -> Shown {
        match self.span {
            Some(s) if s.blend => Shown {
                record: s.to_record,
                frame: f32::from(s.to),
                blend: Some(f32::from(s.counter) / f32::from(s.length.max(1))),
            },
            _ => Shown {
                record: self.record,
                frame: self.frame,
                blend: None,
            },
        }
    }

    /// The updates a script runs before its end with fixed waits (a hop's
    /// landing and the self-toss's flight as [`Run::new`] and `height` give
    /// them).
    pub fn length(steps: &[Step], left: bool, flight: u8, height: f32) -> i16 {
        let mut run = Run::new(left, flight);
        for _ in 0..4096 {
            run.step(steps, height);
            if run.done {
                return run.tick;
            }
        }
        run.tick
    }
}

/// The updates of a script outside its blends (from the first blend's end):
/// what [`Script::root`] is spread over without the disc's animations.
pub fn moving_updates(steps: &[Step], left: bool, flight: u8, height: f32) -> i16 {
    let mut run = Run::new(left, flight);
    let mut n = 0;
    for _ in 0..4096 {
        run.step(steps, height);
        if run.done {
            break;
        }
        let shown = run.shown();
        n += i16::from(shown.blend.is_none() && shown.record.is_some());
    }
    n.max(1)
}

/// The root's change on an update without the disc's animations: the
/// script's whole change ([`Script::root`]) spread evenly over its updates
/// outside the blends.
pub fn stand_in_step(root: [f32; 2], moving: i16, after: Shown) -> [f32; 3] {
    if after.blend.is_some() || after.record.is_none() {
        return [0.0; 3];
    }
    let k = 1.0 / f32::from(moving.max(1));
    [root[0] * k, 0.0, root[1] * k]
}

/// The updates trick `script` runs (its script's end, the update bit 10
/// drops), the self-tosses with their cue's flight.
pub fn length(script: u16) -> i16 {
    let steps = SCRIPTS[usize::from(script).min(SCRIPTS.len() - 1)].steps;
    Run::length(steps, false, flight(script), 0.0)
}

/// The self-toss's flight in updates (callback 53's first word), 0 for
/// any other trick.
pub fn flight(script: u16) -> u8 {
    crate::self_toss::cue_for_script(script)
        .map_or(0, |c| c.flight_frames_at_60_hz.clamp(0, 255) as u8)
}

/// A trick move under way (the original's channel and actor fields).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Play {
    pub run: Run,
    /// The velocity the blend coasts on, units an update.
    pub drift: [f32; 2],
    /// The trick's facing (actor+0xb0).
    pub word: i32,
    /// The fake window: its row of `0x3d0d90` and updates left
    /// (actor+0x19c, +0x1a4); 0 closed.
    pub row: u8,
    pub window: u8,
    /// The defender faked out (actor+0x218 set), if one was.
    pub faked: Option<u8>,
    /// The clip he played at the press (record and frame), held through
    /// the blend.
    pub from: Option<(u16, f32)>,
}

impl Play {
    /// Trick `script` about to start (the live left hand's clip with
    /// `left`), facing `word`.
    pub fn new(script: u16, left: bool, word: i32) -> Self {
        Self {
            run: Run::new(left, flight(script)),
            drift: [0.0; 2],
            word,
            row: 0,
            window: 0,
            faked: None,
            from: None,
        }
    }

    /// Updates since the press (the press update is 0).
    pub fn tick(&self) -> u16 {
        self.run.tick.max(0) as u16
    }
}

/// A drawn trick `script` of value class `class` just pressed, facing
/// word 0 (for tests that need a trick under way).
#[cfg(test)]
pub(crate) fn pressed(script: u16, class: u8) -> crate::Action {
    crate::Action::Trick {
        id: crate::TrickIdentity {
            slot: DRAWN,
            script,
            level: class,
            equipped_row: None,
        },
        play: Play::new(script, false, 0),
    }
}

/// A reaction under way: [`REACTIONS`]' `script`, its clock, the hop's
/// velocity (units an update: across, then up) while he is up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reacting {
    pub script: u8,
    pub run: Run,
    pub from: Option<(u16, f32)>,
    pub drift: [f32; 2],
    pub vy: f32,
    pub up: bool,
}

impl Reacting {
    pub fn steps(&self) -> &'static [Step] {
        REACTIONS[usize::from(self.script)].steps
    }
}

/// A script's pose at what its channel shows, `from` held through a blend
/// (the actor's own clip before the first).
pub fn pose(
    poses: &Poses,
    shown: Shown,
    from: Option<(u16, f32)>,
    body: Body,
    place: Place,
) -> Option<Pose> {
    let to = Frame {
        record: shown.record?,
        time: shown.frame,
        looping: false,
    };
    match (shown.blend, from) {
        (Some(w), Some((record, time))) => poses.blend(
            Frame {
                record,
                time,
                looping: true,
            },
            to,
            w,
            body,
            place,
        ),
        _ => poses.pose(to, body, place),
    }
}

/// The root's change from `before` to `after` (`FUN_0019a658`): none while
/// a blend runs or across a change of clip; the first update after a blend
/// measures from the blend's frame. In court units at scale 1, in the
/// clip's own frame.
pub fn root_step(poses: &Poses, before: Shown, after: Shown) -> [f32; 3] {
    let (Some(a), Some(b)) = (before.record, after.record) else {
        return [0.0; 3];
    };
    if after.blend.is_some() || a != b {
        return [0.0; 3];
    }
    let at = |time: f32| {
        poses.root(Frame {
            record: b,
            time,
            looping: false,
        })
    };
    match (at(before.frame), at(after.frame)) {
        (Some(p), Some(q)) => [q[0] - p[0], q[1] - p[1], q[2] - p[2]],
        _ => [0.0; 3],
    }
}

/// A root change in the world: times the height scale, turned by the facing
/// word (the pose's yaw, `pose::place`), on the floor.
pub fn root_motion(step: [f32; 3], body: Body, word: i32) -> [f32; 2] {
    let theta = word as f32 * std::f32::consts::TAU / 1024.0;
    let (s, c) = (crate::math::sin(theta), crate::math::cos(theta));
    let (x, z) = (step[0] * body.height, step[2] * body.height);
    [x * c + z * s, -x * s + z * c]
}

/// `FUN_00154230` (callback 7): the facing word of the pelvis's local +y
/// turned into the world, `(units(atan2(x, z)) + 0x200) & 0x3ff`.
pub fn pelvis_word(pose: &Pose) -> i32 {
    let q = pose.joints[0].rotation;
    let v = pose::rotate(q, [0.0, 1.0, 0.0]);
    let a = (crate::math::atan2(v[0], v[2]) * 162.974_66 + 0.5) as i32;
    let a = if a > 0 { a } else { a + 0x400 };
    (a + 0x200) & 0x3ff
}

/// `FUN_00137ab0`: `b` pushed `reach` past itself along `a` to `b`.
fn extend(a: [f32; 3], b: [f32; 3], reach: f32) -> [f32; 3] {
    if reach == 0.0 {
        return b;
    }
    let d = sub(b, a);
    let l = pose::length(d);
    if l <= 0.0 {
        return b;
    }
    let k = (reach + l) / l;
    std::array::from_fn(|i| a[i] + d[i] * k)
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

fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = pose::length(a);
    if l <= 0.0 { a } else { a.map(|c| c / l) }
}

/// `FUN_00138f10`: the point of the segment from `p` along `d` on the
/// plane (`n`, `w`); its start when the crossing is off the segment or the
/// segment lies along the plane.
fn on_plane(p: [f32; 3], d: [f32; 3], n: [f32; 3], w: f32) -> [f32; 3] {
    let a = dot(p, n) + w;
    let e = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
    let b = dot(e, n) + w;
    if ((a - b) / (a.abs() + b.abs())).abs() < f32::from_bits(0x38d1_b717) {
        return p;
    }
    let t = a / (a - b);
    if !(0.0..=1.0).contains(&t) {
        return p;
    }
    std::array::from_fn(|i| p[i] * (1.0 - t) + e[i] * t)
}

/// `FUN_00139038`: the points of two segments (`p` along `d`, `q` along
/// `e`) nearest each other through their common perpendicular's planes
/// (`FUN_00138ea0`); their starts when they run parallel.
fn nearest(p: [f32; 3], d: [f32; 3], q: [f32; 3], e: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let n = cross(d, e);
    if pose::length(n) < f32::from_bits(0x38d1_b717) {
        return (p, q);
    }
    let n = normalize(n);
    let plane = |dir: [f32; 3], at: [f32; 3]| {
        let m = normalize(cross(n, dir));
        (m, -dot(m, at))
    };
    let (mp, wp) = plane(d, p);
    let (mq, wq) = plane(e, q);
    (on_plane(p, d, mq, wq), on_plane(q, e, mp, wp))
}

/// `FUN_00139198`: two capsules meet (each segment pushed past its end by
/// its reach, the nearest points within the radii).
pub fn capsules_meet(a: ([f32; 3], [f32; 3], f32, f32), b: ([f32; 3], [f32; 3], f32, f32)) -> bool {
    let (a0, a1) = (a.0, extend(a.0, a.1, a.3));
    let (b0, b1) = (b.0, extend(b.0, b.1, b.3));
    let (p, q) = nearest(a0, sub(a1, a0), b0, sub(b1, b0));
    pose::length(sub(p, q)) <= a.2 + b.2
}

/// `FUN_00139408`'s capsule half for kind 3: any of the handler's capsules
/// against any of the other's.
pub fn touches(handler: &Pose, other: &Pose) -> bool {
    capsules_touch(&HANDLER, handler, other)
}

/// The clearout's arms ([`CLEAROUT_ARMS`]) against the other's capsules.
pub fn arms_touch(handler: &Pose, other: &Pose) -> bool {
    capsules_touch(&CLEAROUT_ARMS, handler, other)
}

/// The air duel's draws by the gap between the two men's scores
/// (`0x136b50..0x136e60`): the stronger's knock of the weaker, then the
/// weaker's of the stronger, each `r % 100` under a chance that slides
/// across the band: 10..20 and 10..15 up to 20, 20..30 and 15..10 to 50,
/// 30..50 and 10 to 100, 50..75 and 10..5 to 150, 75..90 and 5..2 to 200,
/// then 90 and 2; past 100 the second is drawn only after the first.
pub fn knock_odds(rng: &mut crate::rng::Rng, d: i32) -> (bool, bool) {
    let mut roll = || (rng.next_u32() % 100) as i32;
    let lerp = |from: i32, to: i32, at: i32, span: i32| (from * (span - at) + to * at) / span;
    match d {
        ..=20 => (roll() < lerp(10, 20, d, 20), roll() < lerp(10, 15, d, 20)),
        21..=50 => (
            roll() < lerp(20, 30, d - 20, 30),
            roll() < lerp(15, 10, d - 20, 30),
        ),
        51..=100 => (roll() < lerp(30, 50, d - 50, 50), roll() < 10),
        101..=150 => {
            let first = roll() < lerp(50, 75, d - 100, 50);
            (first, first && roll() < lerp(10, 5, d - 100, 50))
        }
        151..=200 => {
            let first = roll() < lerp(75, 90, d - 150, 50);
            (first, first && roll() < lerp(5, 2, d - 150, 50))
        }
        _ => {
            let first = roll() <= 89;
            (first, first && roll() <= 1)
        }
    }
}

/// `capsules`, bones of `handler`, against the other's [`DEFENDER`] lists.
fn capsules_touch(capsules: &[(usize, usize, f32, f32)], handler: &Pose, other: &Pose) -> bool {
    capsules.iter().any(|&(a, b, r, reach)| {
        let mine = (
            handler.joints[a].position,
            handler.joints[b].position,
            r,
            reach,
        );
        DEFENDER.iter().any(|list| {
            list.iter().any(|&(c, d, s, far)| {
                capsules_meet(
                    (other.joints[c].position, other.joints[d].position, s, far),
                    mine,
                )
            })
        })
    })
}

/// `FUN_0018d768`'s sector and half of `to` about `from` facing `word`.
fn sector_half(word: i32, from: [f32; 2], to: [f32; 2]) -> (i32, i32) {
    let a = crate::contact::angle(from, to);
    let u = (word - a + 0x200) & 0x3ff;
    (((u + 0x40) & 0x3ff) >> 7, ((u + 0x80) & 0x3ff) >> 8)
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

use crate::player::{NativeTricks, TrickIdentity};
use crate::{Action, BallState, Input, World, timing};

impl World {
    /// `FUN_00170dc0`: `i`'s nearest opponent on the game's distance table
    /// (the first of equals).
    pub(crate) fn nearest_opponent(&self, i: usize) -> Option<usize> {
        let p = &self.players[i];
        let mut best = (1000.0f32, None);
        for (j, q) in self.players.iter().enumerate() {
            if j == i || q.team == p.team {
                continue;
            }
            let d = crate::distance(p.position, q.position);
            if d < best.0 {
                best = (d, Some(j));
            }
        }
        best.1
    }

    /// The facing `i` wants (actor+0xb0): where the stick points while it
    /// is pushed, else where he faces.
    pub(crate) fn wanted_facing(&self, i: usize, input: &Input) -> i32 {
        let [sx, sz] = input.stick();
        if input.moves() {
            timing::facing_word(crate::math::atan2(sx, sz))
        } else {
            timing::facing_word(self.players[i].heading)
        }
    }

    /// The trick press (`FUN_0015f0f0`, raw `0x15f0f0..0x15f860`) of `i`,
    /// who holds the ball: the turbo buttons' direct slot, or the draw
    /// among the scripts ([`World::draw_trick`]); the facing toward the
    /// nearest defender when he stands within ±45° ahead (actor+0xb0); the
    /// script started.
    ///
    /// The original's draws are kept: the face (`FUN_00159190(actor, 2,
    /// fps × 2)`), the draw, and for a value class of 2 or more the
    /// fumble's (`r % 100` against a chance from Handles and the class,
    /// actor+0x1d4: the timed knock-loose `FUN_0015a060` plays is not
    /// modelled). The turbo is the juke press's ([`World::juke_press`]).
    pub(crate) fn start_trick(&mut self, i: usize, input: &Input, turbos: u32) {
        // The press clears game+0x94 (`0x15f1c8`, [`World::fresh_take`]).
        self.fresh_take = false;
        // The face (`FUN_00159190(actor, 2, fps × 2)`, `0x15f1e8`).
        self.face(i, 2 * crate::TICK_HZ as i32);
        let defender = self.nearest_opponent(i);
        let wanted = self.wanted_facing(i, input);
        let (position, heading) = (self.players[i].position, self.players[i].heading);
        let (sector, half) = defender.map_or((0, 0), |d| {
            sector_half(wanted, position, self.players[d].position)
        });
        let slots = self.players[i]
            .native_tricks
            .unwrap_or_else(NativeTricks::ordinary);
        let id = match NativeTricks::slot(self.players[i].action_buttons) {
            Some(k) => slots.slots[k],
            None => {
                // `0x15f358`: a turbo held (+0x78 & 0x2000) draws at value
                // class 1 from the turbo row, else at 0.
                let class = u8::from(turbos > 0);
                let Some(script) = self.draw_trick(i, defender, sector, wanted, class) else {
                    return;
                };
                TrickIdentity {
                    slot: DRAWN,
                    script,
                    level: class,
                    equipped_row: None,
                }
            }
        };
        // `0x15f598`: he faces the defender ahead of where he wants to go.
        let word = match defender {
            Some(d) if half == 2 => crate::contact::angle(position, self.players[d].position),
            _ => timing::facing_word(heading),
        };
        if id.level >= 2 {
            // The fumble's draw (`0x15f7f4`).
            self.rng.next_u32();
        }
        let from = self.poses.get().map(|_| {
            let f = self.gait_frame(i);
            (f.record, f.time)
        });
        let p = &mut self.players[i];
        let left = p.gait_channel.hand == 2;
        p.action = Action::Trick {
            id,
            play: Play {
                from,
                ..Play::new(id.script, left, word)
            },
        };
    }

    /// The holder's juke press (pad +0x7c & 0x10, `FUN_0015a060`
    /// `0x15ad10..0x15ad2c`), `FUN_0015c8e8`: on the floor and not out of
    /// a dribble he faked (`FUN_0016ed08`, actor+0x1a0 at least 0), with
    /// [`turbo::MOVE`] of his bar (short of it, the "no turbo" message
    /// `0x3c6520` and nothing), which it spends, the trick press
    /// ([`World::start_trick`]), unless the two-person freestyle plays
    /// ([`World::freestyle`]). First the clearout ([`World::clearout`]).
    pub(crate) fn juke_press(&mut self, i: usize, input: &Input, turbos: u32) {
        if self.clearout(i) {
            return;
        }
        let p = &self.players[i];
        if p.dribble < 0 || p.height != 0.0 {
            return;
        }
        if !self.spend_turbo(i, crate::turbo::MOVE) {
            return;
        }
        if !self.freestyle(i, input, turbos) {
            self.start_trick(i, input, turbos);
        }
    }

    /// `FUN_0015c8e8`'s two-person freestyles (`0x15c97c..0x15cdd4`):
    /// without both turbos held (+0x78 & 0x40000), the nearest opponent
    /// (`FUN_00170dc0`) within 64 of the holder's facing and the holder
    /// within 128 of his (`FUN_0018d1f8` both ways), the opponent standing
    /// exactly still on the floor and not reacting, the holder still or
    /// barely pushing (+0x90 under 16): by their distance (the distance
    /// table, `FUN_0016e948`) over the holder's scale, within 4.75 one of
    /// [`FREESTYLES`]' near three, within 8 one of the far three, by `r %
    /// 100` under 33 and 66, the two turned to face each other
    /// (`FUN_0016f508`), the script at value class 2 (+0x248) with the
    /// opponent faked (+0x218) and playing the row's reaction, unless he
    /// is in a counter stance (`FUN_0016e9b0`). False beyond 8 or when it
    /// does not apply (the trick press then).
    fn freestyle(&mut self, i: usize, input: &Input, turbos: u32) -> bool {
        let Some(d) = self.nearest_opponent(i) else {
            return false;
        };
        let (p, q) = (&self.players[i], &self.players[d]);
        let off = |a: &crate::Player, b: &crate::Player| {
            let u = (timing::facing_word(a.heading)
                - crate::contact::angle(a.position, b.position))
                & 0x3ff;
            if 0x200 < u { 0x400 - u } else { u }
        };
        if turbos >= 2
            || off(p, q) > 64
            || off(q, p) > 128
            || q.moving()
            || q.height != 0.0
            || matches!(q.action, Action::Stumbling { .. } | Action::Dive { .. })
            || (p.moving() && input.strength() >= 16)
        {
            return false;
        }
        let far = crate::distance(p.position, q.position) / p.body().width.max(1e-3);
        let row = match far {
            f if f <= 4.75 => 0,
            f if f <= 8.0 => 1,
            _ => return false,
        };
        let r = (self.rng.next_u32() % 100) as usize;
        let k = usize::from(r >= 33) + usize::from(r >= 66);
        let (script, reaction) = FREESTYLES[row][k];
        // Each faces the other (`FUN_0016f508`, +0xb0).
        let (pp, qp) = (p.position, q.position);
        let word = crate::contact::angle(pp, qp);
        self.players[d].heading = crate::restart::heading_radians(crate::contact::angle(qp, pp));
        let from = self.poses.get().map(|_| {
            let f = self.gait_frame(i);
            (f.record, f.time)
        });
        let left = self.players[i].gait_channel.hand == 2;
        self.players[i].gait_channel.trick_side = u8::from(left);
        let id = TrickIdentity {
            slot: DRAWN,
            script,
            level: 2,
            equipped_row: None,
        };
        let countering = matches!(
            self.players[d].action,
            Action::Stealing { counter: true, .. }
        );
        let mut play = Play {
            from,
            ..Play::new(script, left, word)
        };
        if !countering {
            self.react(d, reaction);
            play.faked = Some(d as u8);
        }
        self.players[i].action = Action::Trick { id, play };
        true
    }

    /// `FUN_0015c778`, the clearout, which the juke press tries first: the
    /// holder standing exactly still (velocity +0x40 zero), on the floor,
    /// outside a restart (game+0xf4 not 6), not dribbling (+0x1a0 not 1),
    /// not posting up (the sim never does), with [`turbo::MOVE`] of his
    /// bar, plays `clearout01` on his channel ([`CLEAROUT`], its kind-2
    /// window of 40 updates) and spends it. True when it did.
    fn clearout(&mut self, i: usize) -> bool {
        let p = &self.players[i];
        if p.moving()
            || p.height != 0.0
            || self.game_phase.phase == crate::game_phase::phase::RESTART
            || p.dribble == 1
            || self.turbo_of(i) + 1e-6 < crate::turbo::MOVE
        {
            return false;
        }
        self.start_clearout(i);
        self.spend_turbo(i, crate::turbo::MOVE);
        true
    }

    /// `clearout01` started on player `i`'s channel (`FUN_0019c2f8(actor
    /// + 0xc4, 0x456d02, ..)`): the juke press's clearout and callback
    /// 37's at a rebound's end ([`World::rebound_landed`]).
    pub(crate) fn start_clearout(&mut self, i: usize) {
        let p = &self.players[i];
        let word = timing::facing_word(p.heading);
        let left = p.gait_channel.hand == 2;
        let from = self.poses.get().map(|_| {
            let f = self.gait_frame(i);
            (f.record, f.time)
        });
        self.players[i].gait_channel.trick_side = u8::from(left);
        self.players[i].action = Action::Trick {
            id: crate::TrickIdentity {
                slot: DRAWN,
                script: CLEAROUT,
                level: 0,
                equipped_row: None,
            },
            play: Play {
                from,
                ..Play::new(CLEAROUT, left, word)
            },
        };
    }

    /// `0x15f374..0x15f590`: the draw among the 75 trick scripts. The
    /// defender dead ahead is put to one side (`FUN_0015f860`); with him on
    /// the ball's side only the rows that allow it qualify (flag 1), on the
    /// other side those with flag 2; standing or moving as the row allows;
    /// none for the post-up, and not the last script drawn (`0x3d75bc`).
    /// Posting (bit 11) off the backdown's walk and transition (rows
    /// 704/705, 712/713) only the post-up's rows qualify, the sides and the
    /// last drawn not asked (`0x15f40c..0x15f434`, `0x15f514..0x15f54c`).
    /// `r %` the count picks one. None when nothing qualifies (the original
    /// divides by zero).
    fn draw_trick(
        &mut self,
        i: usize,
        defender: Option<usize>,
        sector: i32,
        wanted: i32,
        class: u8,
    ) -> Option<u16> {
        let p = &self.players[i];
        let moving = p.moving();
        let mut s = sector;
        if let Some(d) = defender
            && s == 4
        {
            let a = crate::contact::angle(p.position, self.players[d].position);
            s = if (wanted - a) & 0x3ff < 0x200 { 3 } else { 5 };
        }
        let hand = p.gait_channel.hand;
        let mut side = 0u8;
        if (s == 5 && hand == 1) || (s == 3 && hand == 2) {
            side |= flag::BALL_SIDE;
        }
        if (s == 3 && hand == 1) || (s == 5 && hand == 2) {
            side |= flag::OTHER_SIDE;
        }
        let row = self.channel_row(i);
        let post = self.posting(i) && !matches!(row, 704 | 705 | 712 | 713);
        if post {
            side = 0;
        }
        let mask = 1u8 << class;
        let last = self.last_trick;
        let candidates: Vec<u16> = (0..CLEAROUT)
            .filter(|&k| {
                let s = &SCRIPTS[usize::from(k)];
                let f = s.flags;
                s.mask & mask != 0
                    && !(f & flag::MOVING == 0 && f & flag::STANDING != 0 && moving)
                    && !(f & flag::STANDING == 0 && f & flag::MOVING != 0 && !moving)
                    && (side & flag::BALL_SIDE == 0 || f & flag::BALL_SIDE != 0)
                    && (side & flag::OTHER_SIDE == 0 || f & flag::OTHER_SIDE != 0)
                    && if post {
                        f & flag::POST_UP != 0
                    } else {
                        f & flag::POST_UP == 0 && u16::from(last) != k
                    }
            })
            .collect();
        let r = self.rng.next_u32();
        let k = *candidates.get((r % candidates.len().max(1) as u32) as usize)?;
        self.last_trick = k as u8;
        Some(k)
    }

    /// One update of `i`'s trick script, before his step: the window's
    /// count down (`FUN_00156f48`), the channel's update (`FUN_0019a658`),
    /// then the motion: on the press update the gait moves him and he
    /// coasts on that through the blend; after it (`97`) the clip's root
    /// carries him; his facing turns 46 an update toward the trick's. At
    /// callback 53 the self-toss leaves; at the script's end (bit 10 drops,
    /// `0x157dd0`) the trick scores and he faces as his pelvis does
    /// (callback 7). Returns the clip's frame when the trick ends here, for
    /// the live hand.
    pub(crate) fn trick_tick(&mut self, i: usize, input: &Input, held: bool) -> Option<f32> {
        let Action::Trick { id, mut play } = self.players[i].action else {
            return None;
        };
        let steps = SCRIPTS[usize::from(id.script).min(SCRIPTS.len() - 1)].steps;
        play.window = play.window.saturating_sub(1);
        let before = play.run.shown();
        let ran = play.run.step(steps, self.players[i].height);
        let after = play.run.shown();
        // The fake's window, or the clearout's (kind 2).
        if let Some((row, n)) = ran.window.or(ran.shove) {
            play.row = row;
            play.window = n;
        }
        if ran.stop {
            play.drift = [0.0; 2];
        }
        let p = &mut self.players[i];
        if play.run.tick == 0 {
            // The press update: the gait still runs (it points his
            // velocity), and the blend coasts on what it left.
            let heading = p.heading;
            p.walk(input, 1.0, held, None);
            let v = p.velocity();
            play.drift = [v[0] * crate::TICK_SECONDS, v[1] * crate::TICK_SECONDS];
            p.heading = heading;
        } else {
            p.position[0] += play.drift[0];
            p.position[1] += play.drift[1];
        }
        p.speed = 0.0;
        p.height = 0.0;
        let now = timing::facing_word(p.heading);
        let d = turn(play.word, now).clamp(-TURN, TURN);
        if d != 0 {
            p.heading = crate::restart::heading_radians((now + d) & 0x3ff);
        }
        let script = &SCRIPTS[usize::from(id.script).min(SCRIPTS.len() - 1)];
        let step = match self.poses.get() {
            Some(poses) => root_step(poses, before, after),
            None if play.run.done => [0.0; 3],
            None => {
                let moving = moving_updates(script.steps, play.run.left, play.run.flight, 0.0);
                stand_in_step(script.root[usize::from(play.run.left)], moving, after)
            }
        };
        let p = &mut self.players[i];
        let m = root_motion(step, p.body(), timing::facing_word(p.heading));
        p.position[0] += m[0];
        p.position[1] += m[1];
        self.players[i].clamp_to_court();
        let frame = after.frame;
        if ran.face
            && let Some(poses) = self.poses.get()
            && let Some(pose) = pose(
                poses,
                after,
                play.from,
                self.players[i].body(),
                self.place(i),
            )
        {
            self.players[i].heading = crate::restart::heading_radians(pelvis_word(&pose));
        }
        self.players[i].action = if play.run.done {
            Action::None
        } else {
            Action::Trick { id, play }
        };
        if ran.launch {
            self.self_toss_launch(i, id.script);
        }
        if play.run.done {
            // `0x157dd0`: in live play, with the ball still his (the
            // clearout never sets bit 10).
            if id.script != CLEAROUT
                && self.game_phase.live()
                && self.ball.state == BallState::Held(i)
            {
                self.score_trick(i, id, play.faked);
            }
            return Some(frame);
        }
        None
    }

    /// The kind-3 contacts (`FUN_00133bb0`'s scripted pass, `FUN_00139408`
    /// with `FUN_00137868`, then `FUN_001372d8`): every trick handler whose
    /// window is open against each opponent.
    pub(crate) fn trick_contacts(&mut self) {
        for i in 0..self.players.len() {
            for j in 0..self.players.len() {
                let open = matches!(
                    self.players[i].action,
                    Action::Trick { play, .. } if play.window > 0
                );
                let clearout = matches!(
                    self.players[i].action,
                    Action::Trick { id, .. } if id.script == CLEAROUT
                );
                if open && i != j {
                    if clearout {
                        self.clearout_contact(i, j);
                    } else {
                        self.trick_contact(i, j);
                    }
                }
            }
        }
    }

    /// `FUN_00137868` and the capsules: may handler `a`'s trick meet `b`.
    fn trick_meets(&self, a: usize, b: usize) -> bool {
        let (p, q) = (&self.players[a], &self.players[b]);
        // `FUN_0018cdb0`: the actors' positions, heights and all.
        let (dx, dy, dz) = (
            q.position[0] - p.position[0],
            q.height - p.height,
            q.position[1] - p.position[1],
        );
        if p.team == q.team || (dx * dx + dy * dy + dz * dz).sqrt() > REACH {
            return false;
        }
        // Reacting or down (`FUN_0016ea70`, channel flag 0x40).
        if matches!(q.action, Action::Stumbling { .. } | Action::Dive { .. }) {
            return false;
        }
        // A counter stance (bit 16) facing the handler meets him itself.
        if matches!(q.action, Action::Stealing { counter: true, .. })
            && (3..=5).contains(&self.octant(b, a))
        {
            return false;
        }
        // Not him with the ball (+0x70); the handler in his trick (bit 10).
        if self.ball.state == BallState::Held(b) {
            return false;
        }
        match (self.poses.get(), self.actor_pose(a), self.actor_pose(b)) {
            (Some(_), Some(pa), Some(pb)) => touches(&pa, &pb),
            _ => crate::distance(p.position, q.position) <= STAND_IN_REACH,
        }
    }

    /// `FUN_001372d8`: handler `a`'s trick meets `b`.
    fn trick_contact(&mut self, a: usize, b: usize) {
        if !self.trick_meets(a, b) {
            return;
        }
        let Action::Trick { id, mut play } = self.players[a].action else {
            return;
        };
        let shown = play.run.shown();
        let record = shown.record.unwrap_or(u16::MAX);
        if NO_FAKE.contains(&record) {
            play.window = 0;
            self.players[a].action = Action::Trick { id, play };
            return;
        }
        // Only a defender in the handler's front sectors is tried; the
        // window stays open for the others.
        if !(3..=5).contains(&self.octant(a, b)) {
            return;
        }
        let (lo, hi) = ODDS[usize::from(id.level).min(4)];
        let handles = self.players[a].ratings.native(7);
        let mut chance = lo + ((hi - lo) as f32 * handles as f32 / 99.0) as i32;
        let computer = self.previous_inputs.get(a).is_some_and(|x| x.is_computer());
        if !computer && self.rules.difficulty == crate::Difficulty::Easy {
            chance += PERSON_ODDS;
        }
        let roll = (self.rng.next_u32() % 100) as i32;
        // The quarter the handler stands in about the defender (2 in
        // front), which picks the reaction's list.
        let quarter = self.quarter(b, a).clamp(0, 3) as usize;
        let faked = roll <= chance;
        let list = if faked { FAKED[quarter] } else { HELD[quarter] };
        let r = self.rng.next_u32();
        let mut script = list[(r % list.len().max(1) as u32) as usize];
        if faked && quarter == 2 && STEP_THROUGH_RECORDS.contains(&record) {
            script = STEP_THROUGH;
        }
        self.react(b, script);
        play.window = 0;
        if faked {
            play.faked = Some(b as u8);
            // The handler's face (`FUN_00159190(handler, 9, fps × 2)`).
            self.face(a, 2 * crate::TICK_HZ as i32);
        }
        self.players[a].action = Action::Trick { id, play };
    }

    /// The clearout's kind-2 contact (`FUN_00139408` with `FUN_001366a0`,
    /// then `FUN_00136290`): `a`'s arms ([`CLEAROUT_ARMS`]) meet opponent
    /// `b`, who is not reacting, within [`REACH`]. Only a man standing in
    /// front of `a` (`FUN_0018d9f0(a, b)`'s quarter 2) is knocked; the
    /// window stays open for the others. A roll `r % 100`, then: in one of
    /// [`CLEAROUT_STANCES`], a defender with less power (attribute 4) than
    /// `a`, else one with less power than the roll, stumbles back
    /// ([`CLEAROUT_STUMBLE`]); the rest take one of [`CLEAROUT_BRACED`]. His
    /// face (`FUN_00159190(b, 7, fps × 2)`), the window closed and the
    /// contact's message (`0x3c6910`): his AI's screener is `a`.
    fn clearout_contact(&mut self, a: usize, b: usize) {
        if !self.shove_contact(a, b, &CLEAROUT_ARMS) {
            return;
        }
        if let Action::Trick { id, mut play } = self.players[a].action {
            play.window = 0;
            self.players[a].action = Action::Trick { id, play };
        }
    }

    /// `FUN_00136290` for window owner `a` with `capsules` (its row of
    /// `0x3d0d90`) against `b`, as [`World::clearout_contact`] describes;
    /// true when `b` was knocked (the caller closes the window).
    fn shove_contact(&mut self, a: usize, b: usize, capsules: &[(usize, usize, f32, f32)]) -> bool {
        let (p, q) = (&self.players[a], &self.players[b]);
        let (dx, dy, dz) = (
            q.position[0] - p.position[0],
            q.height - p.height,
            q.position[1] - p.position[1],
        );
        if p.team == q.team || (dx * dx + dy * dy + dz * dz).sqrt() > REACH {
            return false;
        }
        if matches!(q.action, Action::Stumbling { .. } | Action::Dive { .. }) {
            return false;
        }
        let meets = match (self.poses.get(), self.actor_pose(a), self.actor_pose(b)) {
            (Some(_), Some(pa), Some(pb)) => capsules_touch(capsules, &pa, &pb),
            _ => crate::distance(p.position, q.position) <= STAND_IN_REACH,
        };
        if !meets || self.quarter(a, b) != 2 {
            return false;
        }
        let roll = (self.rng.next_u32() % 100) as i32;
        let theirs = self.players[b].ratings.native(4);
        let stance = CLEAROUT_STANCES.contains(&self.gait_frame(b).record);
        let weaker = if stance {
            theirs < self.players[a].ratings.native(4)
        } else {
            theirs < roll
        };
        let list: &[u8] = if weaker {
            &CLEAROUT_STUMBLE
        } else {
            &CLEAROUT_BRACED
        };
        let r = self.rng.next_u32();
        self.react(b, list[(r % list.len() as u32) as usize]);
        // His face (`FUN_00159190(b, 7, fps × 2)`).
        self.face(b, 2 * crate::TICK_HZ as i32);
        let kind = self.screen_kind(b);
        self.post_contact(a, kind);
        if let Some(brain) = self.ai.brains.get_mut(b) {
            brain.screened_by = a as i32;
        }
        true
    }

    /// The dunks' and lay-ups' kind-1 contacts (`FUN_00139408` with
    /// `FUN_001371d8`, then `FUN_001366d8`): every mover whose knock window
    /// is open against each man; and the held scripts' kind-2 windows (the
    /// heave shot's and a gamebreaker shot's, [`SHOVE_BODY`]) as the
    /// clearout's ([`World::clearout_contact`]), each closing once it
    /// knocks a man.
    pub(crate) fn knock_contacts(&mut self) {
        for i in 0..self.players.len() {
            for j in 0..self.players.len() {
                if i != j && self.players[i].flight.knock > 0 && self.knock_meets(i, j) {
                    self.knock_contact(i, j);
                }
                if i != j
                    && self.players[i].flight.shove > 0
                    && self.shove_contact(i, j, &SHOVE_BODY)
                {
                    self.players[i].flight.shove = 0;
                }
            }
        }
    }

    /// `FUN_001371d8` and the capsules: may `a`'s move meet `b`. Another
    /// team; `a` dunking or leaping (bits 7, 15) and in the air, not yet
    /// met with `b` this move; `b` not reacting nor in a rebound jump (bit
    /// 8); with either dunking, the ball held and no gamebreaker on it
    /// (ball+0x178). Then [`KNOCK_BODY`] against the other's capsules,
    /// within [`REACH`].
    fn knock_meets(&self, a: usize, b: usize) -> bool {
        let (p, q) = (&self.players[a], &self.players[b]);
        let dunking = |x: &crate::Player| matches!(x.action, Action::Dunking { layup: false, .. });
        if p.team == q.team
            || !(dunking(p) || p.leaping())
            || p.flight.met & (1 << b) != 0
            || p.height == 0.0
            || matches!(
                q.action,
                Action::Stumbling { .. } | Action::Dive { .. } | Action::ReboundJump { .. }
            )
        {
            return false;
        }
        if (dunking(p) || dunking(q))
            && (!matches!(self.ball.state, BallState::Held(_)) || self.gamebreaker_ball() != 0)
        {
            return false;
        }
        let (dx, dy, dz) = (
            q.position[0] - p.position[0],
            q.height - p.height,
            q.position[1] - p.position[1],
        );
        if (dx * dx + dy * dy + dz * dz).sqrt() > REACH {
            return false;
        }
        match (self.poses.get(), self.actor_pose(a), self.actor_pose(b)) {
            (Some(_), Some(pa), Some(pb)) => capsules_touch(&KNOCK_BODY, &pa, &pb),
            _ => crate::distance(p.position, q.position) <= STAND_IN_REACH,
        }
    }

    /// `FUN_001366d8`: `a`'s dunk or lay-up meets `b`.
    ///
    /// On the floor, `b` is knocked (one of [`KNOCK_FLOOR`] by the quarter
    /// `a` stands in about him) when `a` holds the ball, is coming down,
    /// stands within 200 of `b`'s facing and has more power (attribute 4).
    ///
    /// In the air, within 2 of `b`'s height and at least a tenth into his
    /// jump, each within 256 of the other's facing, it is a duel: `a`'s
    /// weight (field 9), twice his power and his dunks against `b`'s
    /// weight, twice his power and his blocks; `a`'s × 6/5 coming down,
    /// × 9/10 or × 4/5 after beating one or two men already; the gap's
    /// band draws the stronger's and the weaker's knocks
    /// ([`knock_odds`]). A knocked man plays [`KNOCK_BEHIND`] turned away
    /// from the other when he has him behind (quarter 0), else
    /// [`KNOCK_FRONT`] facing him, and his AI names the other his screener
    /// (`0x3c6910`); never `a` while the ball carries a gamebreaker. A
    /// knocked `a` with the ball lets it go (`FUN_0014c408`): along his
    /// facing turned by `r % 200 − 100`, back 0.35 an update across and
    /// 0.125 up, spinning about the vertical at `0.075 + u × 0.075`.
    fn knock_contact(&mut self, a: usize, b: usize) {
        let mark = |w: &mut World, beat: bool| {
            let f = &mut w.players[a].flight;
            f.met |= 1 << b;
            f.beat += u8::from(beat);
        };
        let holds = self.ball.state == BallState::Held(a);
        let off = |w: &World, x: usize, y: usize| {
            let (px, py) = (&w.players[x], &w.players[y]);
            let u = (timing::facing_word(px.heading)
                - crate::contact::angle(px.position, py.position))
                & 0x3ff;
            if 0x200 < u { 0x400 - u } else { u }
        };
        let power = |w: &World, x: usize| w.players[x].ratings.native(4);
        if self.players[b].height == 0.0 {
            if !holds
                || off(self, b, a) > 200
                || self.players[a].flight.velocity[1] > 0.0
                || power(self, b) >= power(self, a)
            {
                return;
            }
            let list = KNOCK_FLOOR[self.quarter(b, a).clamp(0, 3) as usize];
            let r = self.rng.next_u32();
            self.react(b, list[(r % list.len() as u32) as usize]);
            mark(self, false);
            return;
        }
        let f = self.players[a].flight;
        if (self.players[a].height - self.players[b].height).abs() > 2.0
            || f.air == 0
            || (f32::from(f.aloft) / f32::from(f.air)) < 0.1
            || off(self, b, a) > 256
            || off(self, a, b) > 256
        {
            return;
        }
        let score = |w: &World, x: usize, third: usize| {
            let p = &w.players[x];
            i32::from(p.weight) + 2 * p.ratings.native(4) + p.ratings.native(third)
        };
        let mut sa = score(self, a, 2);
        let sb = score(self, b, 5);
        if f.velocity[1] <= 0.0 {
            sa = sa * 6 / 5;
        }
        match f.beat {
            1 => sa = sa * 9 / 10,
            2 => sa = sa * 4 / 5,
            _ => {}
        }
        let (mut theirs, mut ours) = knock_odds(&mut self.rng, (sa - sb).abs());
        if sa < sb {
            std::mem::swap(&mut theirs, &mut ours);
        }
        if self.gamebreaker_ball() != 0 {
            ours = false;
        }
        mark(self, theirs);
        let knock = |w: &mut World, x: usize, y: usize| {
            let toward = crate::contact::angle(w.players[x].position, w.players[y].position);
            let (script, word) = if w.quarter(x, y) == 0 {
                (KNOCK_BEHIND, (toward + 0x200) & 0x3ff)
            } else {
                (KNOCK_FRONT, toward)
            };
            w.players[x].heading = crate::restart::heading_radians(word);
            w.react(x, script);
            if let Some(brain) = w.ai.brains.get_mut(x) {
                brain.screened_by = y as i32;
            }
        };
        if theirs {
            knock(self, b, a);
        }
        if ours {
            // His face (`FUN_00159190(a, 7, fps × 2)`).
            self.face(a, 2 * crate::TICK_HZ as i32);
            knock(self, a, b);
            if holds {
                let word = timing::facing_word(self.players[a].heading);
                let turn = (self.rng.next_u32() % 200) as i32 - 100;
                let dir = (word - turn) & 0x3ff;
                let v = [
                    -crate::input::native_sin_1024(dir) * 0.35,
                    0.125,
                    -crate::input::native_cos_1024(dir) * 0.35,
                ];
                let u = (self.rng.next_u32() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000);
                self.ball.drop_loose();
                self.ball.velocity = v.map(|c| c / crate::TICK_SECONDS);
                self.ball.spin_axis = [0.0, 1.0, 0.0];
                self.ball.spin = u * 0.075 + 0.075;
                self.ball.airborne = true;
                self.ball.flight_left = 0.0;
                self.ball.flight_time = 0.0;
            }
        }
    }

    /// `b` starts reaction `script` (`FUN_0019c2f8` runs its first update
    /// at once).
    pub(crate) fn react(&mut self, b: usize, script: u8) {
        self.begin_reaction(b, script);
        self.reaction_tick(b);
    }

    /// `b`'s reaction `script` set on his channel, its first update his
    /// channel's next step (a script his own side's or the game's update
    /// starts).
    pub(crate) fn begin_reaction(&mut self, b: usize, script: u8) {
        let steps = REACTIONS[usize::from(script)].steps;
        let height = self.players[b].height;
        let length = Run::length(steps, false, 0, height);
        let from = self.poses.get().map(|_| {
            let f = self.gait_frame(b);
            (f.record, f.time)
        });
        self.players[b].action = Action::Stumbling {
            elapsed: 0.0,
            duration: f32::from(length.max(0) as u16) * crate::TICK_SECONDS,
            reaction: Some(Reacting {
                script,
                run: Run::new(false, 0),
                from,
                drift: [0.0; 2],
                vy: 0.0,
                up: false,
            }),
        };
    }

    /// `FUN_00170e70`: `i` celebrates, `r % 17` of [`CELEBRATE_ROWS`]
    /// (`0x3d9078`) on script `0x44cbbf`, his wanted facing (actor+0xb0)
    /// to the ball (`FUN_0016f2b0`).
    pub(crate) fn celebrate(&mut self, i: usize) {
        let r = self.rng.next_u32();
        let k = (r % CELEBRATE_ROWS.len() as u32) as u8;
        let (p, b) = (&self.players[i], self.ball.position);
        let word = timing::angle_units(crate::math::atan2(
            p.position[0] - b[0],
            p.position[1] - b[2],
        ));
        self.players[i].wanted = crate::restart::heading_radians(word);
        self.begin_reaction(i, CELEBRATE_FIRST + k);
    }

    /// The win's celebration (`0x13dc34..0x13de78`, `0x13e8ec..0x13eb30`):
    /// [`World::celebrate`], then the wanted facing to the court's centre.
    fn celebrate_facing_centre(&mut self, i: usize) {
        self.celebrate(i);
        let [x, z] = self.players[i].position;
        let word = timing::angle_units(crate::math::atan2(x, z));
        self.players[i].wanted = crate::restart::heading_radians(word);
    }

    /// `FUN_00171160`: the loser's reaction at the win ([`DEJECT_ROWS`],
    /// `0x3d9138`, on script `0x44cc03`). At least 10 from the centre on
    /// either axis, a man facing within a quarter turn of it draws `r %` the
    /// rows not barred to that facing and wants to face the centre, else
    /// the other two and wants to face away; nearer, `r & 3` of the four
    /// with his wanted facing kept.
    pub(crate) fn deject(&mut self, i: usize) {
        let p = &self.players[i];
        let [x, z] = p.position;
        let k = if 10.0 <= x.abs() || 10.0 <= z.abs() {
            let toward = timing::angle_units(crate::math::atan2(x, z));
            let u = (toward - timing::facing_word(p.heading)) & 0x3ff;
            let facing = u <= 256 || 768 < u;
            let barred = if facing { 2 } else { 1 };
            let rows: Vec<u8> = (0..DEJECT_ROWS.len() as u8)
                .filter(|&k| DEJECT_ROWS[usize::from(k)].1 & barred == 0)
                .collect();
            let r = self.rng.next_u32();
            let word = if facing { toward } else { toward + 0x200 };
            self.players[i].wanted = crate::restart::heading_radians(word & 0x3ff);
            rows[(r % rows.len() as u32) as usize]
        } else {
            (self.rng.next_u32() & 3) as u8
        };
        self.begin_reaction(i, DEJECT_FIRST + k);
    }

    /// `FUN_0015da28`'s first test (`0x15dcf8..0x15dd80`): in the restart
    /// (game phase 6) a man the game told to celebrate ([`Player::celebrate`])
    /// does so once he stands on the floor and no celebration's or
    /// reaction's script (channel flag 0x40) plays; the word is cleared and
    /// the rest of his side's update skipped. (Its other word, +0x364, for
    /// [`World::deject`], only the game's teardown sets: slot 52 from
    /// `FUN_0013d388`.)
    ///
    /// [`Player::celebrate`]: crate::Player::celebrate
    pub(crate) fn celebrate_due(&mut self, i: usize) {
        let p = &self.players[i];
        if !p.celebrate
            || self.game_phase.phase != crate::game_phase::phase::RESTART
            || p.height != 0.0
            || matches!(
                p.action,
                Action::Stumbling {
                    reaction: Some(_),
                    ..
                }
            )
        {
            return;
        }
        self.players[i].celebrate = false;
        self.celebrate(i);
    }

    /// Bits 7 and 15 (`FUN_0016e9d0`, `FUN_0016e9e0`): in a dunk or a
    /// leap.
    fn celebration_busy(&self, i: usize) -> bool {
        let p = &self.players[i];
        matches!(p.action, Action::Dunking { layup: false, .. }) || p.leaping()
    }

    /// The win (`FUN_0013d868`, `0x13dbb8..0x13dee8`, as phase 9 begins):
    /// the last basket's scorer (the game listener's +4, [`World::scorer`])
    /// celebrates facing the court's centre; in a trick-point game
    /// (config+0xe8 1) a scorer still in a dunk or a leap waits for phase
    /// 9's countdown to find him out of it ([`World::celebrate_pending`],
    /// game+0x10c). Then the first of the losers by slot standing on the
    /// floor reacts ([`World::deject`]).
    pub(crate) fn won_celebration(&mut self, winner: usize) {
        if let Some(s) = self.scorer {
            if self.rules.game_type() == 1 && self.celebration_busy(s) {
                self.celebrate_pending = true;
            }
            if !self.celebrate_pending {
                self.celebrate_facing_centre(s);
            }
        }
        let loser = (0..self.players.len())
            .filter(|&j| self.players[j].team.min(1) != winner.min(1))
            .take(3)
            .find(|&j| self.players[j].height == 0.0);
        if let Some(j) = loser {
            self.deject(j);
        }
    }

    /// Phase 9's countdown (case 9, `0x13e890..0x13eb38`): a waiting
    /// celebration starts once the scorer is out of his dunk or leap.
    pub(crate) fn won_countdown(&mut self) {
        let Some(s) = self.scorer else {
            return;
        };
        if self.celebrate_pending && !self.celebration_busy(s) {
            self.celebrate_facing_centre(s);
            self.celebrate_pending = false;
        }
    }

    /// One update of `i`'s reaction: the channel's update, the hop and its
    /// flight (as the tip-off's jump flies), the clip's root, the end (the
    /// facing from the pelvis at callback 7).
    pub(crate) fn reaction_tick(&mut self, i: usize) {
        let Action::Stumbling {
            elapsed,
            duration,
            reaction: Some(mut r),
        } = self.players[i].action
        else {
            return;
        };
        let steps = r.steps();
        let before = r.run.shown();
        let ran = r.run.step(steps, self.players[i].height);
        let after = r.run.shown();
        let p = &mut self.players[i];
        p.speed = 0.0;
        if ran.stop {
            // `97`: the velocity zeroed.
            r.drift = [0.0; 2];
            r.vy = 0.0;
        }
        if let Some((turn_units, speed, rise)) = ran.hop {
            // `a1 81`: along the facing (+0xb0) turned by `turn`.
            let a = (timing::facing_word(p.heading) + i32::from(turn_units)) & 0x3ff;
            let theta = a as f32 * std::f32::consts::TAU / 1024.0;
            r.drift = [
                -speed * crate::math::sin(theta),
                -speed * crate::math::cos(theta),
            ];
            r.vy = rise;
            r.up = true;
        }
        if r.up {
            let g = crate::toss::JUMP_GRAVITY;
            p.height += r.vy - 0.5 * g;
            r.vy -= g;
            if p.height <= 0.0 {
                p.height = 0.0;
                r.vy = 0.0;
                r.up = false;
            }
        }
        p.position[0] += r.drift[0];
        p.position[1] += r.drift[1];
        let step = match self.poses.get() {
            Some(poses) => root_step(poses, before, after),
            None if r.run.done => [0.0; 3],
            None => {
                let moving = moving_updates(steps, false, 0, 0.0);
                stand_in_step(REACTIONS[usize::from(r.script)].root, moving, after)
            }
        };
        let p = &mut self.players[i];
        let m = root_motion(step, p.body(), timing::facing_word(p.heading));
        p.position[0] += m[0];
        p.position[1] += m[1];
        self.players[i].clamp_to_court();
        // `FUN_00168ea8` on a celebration or the win's reaction: the facing
        // turns toward the wanted one their start set, at most 46 units an
        // update.
        if CELEBRATE_FIRST <= r.script {
            let p = &mut self.players[i];
            let (now, to) = (
                timing::facing_word(p.heading),
                timing::facing_word(p.wanted),
            );
            let d = crate::guard::turn(to, now);
            if d.abs() > crate::guard::TURN {
                p.heading = crate::restart::heading_radians(now + crate::guard::TURN * d.signum());
            } else if d != 0 {
                p.heading = p.wanted;
            }
        }
        if ran.face
            && let Some(poses) = self.poses.get()
            && let Some(pose) = pose(poses, after, r.from, self.players[i].body(), self.place(i))
        {
            self.players[i].heading = crate::restart::heading_radians(pelvis_word(&pose));
        }
        self.players[i].action = if r.run.done {
            self.players[i].height = 0.0;
            Action::None
        } else {
            Action::Stumbling {
                elapsed,
                duration,
                reaction: Some(r),
            }
        };
    }

    /// `i`'s trick or reaction pose, if he is in one.
    pub(crate) fn scripted_pose(&self, i: usize) -> Option<Pose> {
        let poses = self.poses.get()?;
        let (shown, from) = match self.players[i].action {
            Action::Trick { play, .. } => (play.run.shown(), play.from),
            Action::Stumbling {
                reaction: Some(r), ..
            } => (r.run.shown(), r.from),
            // The counter stance and its take ([`crate::counter`]).
            Action::Stealing {
                stance: Some(s), ..
            } => (s.run.shown(), s.from),
            // A stand ([`crate::stand`]).
            Action::None => {
                let s = self.players[i].stand?;
                (s.run.shown(), s.from)
            }
            _ => return None,
        };
        pose(poses, shown, from, self.players[i].body(), self.place(i))
    }
}

include!("trick_tables.rs");
