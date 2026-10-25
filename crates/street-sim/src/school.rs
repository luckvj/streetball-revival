//! Street School's lesson manager: the School game's (`FUN_0018e9f0`,
//! vtable `0x441680`) lesson listener (`FUN_0018df00`, vtable `0x4413d8`,
//! game+0x4c0) and its update (`FUN_0018eec0`, vt+0xd8). The listener's
//! slots read the match's posts — here the [`World`]'s events — and each
//! decides the lesson's result (`FUN_0018f6b0`); the manager waits
//! `fps × 0.9` (Cancel: `× 1.5`) updates and commits it, unless a staged
//! sequence plays. Who is a person is the controller words
//! ([`World::people`]). docs/SCHOOL_PROFILE_RECOVERY.md has the slots;
//! docs/TRACE.md "Street School as a capture suite" the runs of the
//! original they were checked against.
//!
//! The manager also prompts the person (`FUN_0018eec0`'s second half):
//! once the lesson has started, its readiness check ([`ready`],
//! `FUN_0018f740`'s 23 checks) shows the move's prompt (the message
//! `0x3c6970` to the overlay, `FUN_002ba3b0`) and hides it again when the
//! check fails (`0x3c6980`, `FUN_002bab48`), each change standing
//! [`prompt_hold`] updates; and it opens the School's tutorial pages
//! (`0x3c6990`, `FUN_002bb338`) the first time a lesson needs one
//! (`FUN_0018f540`). What it posts is [`Run::posts`].

use crate::{
    Action, Basket, Event, PassStyle, Phase, World,
    ball::Pass,
    ball::Via,
    game_phase,
    timing::{angle_diff, angle_units},
};

/// State sampled before the authoritative world step: what the game's
/// update (the lesson manager, `FUN_0018eec0`, runs in it before the actors
/// and the ball) and the step's posts read of the last update.
#[derive(Clone, Copy, Debug)]
pub struct Before {
    live: bool,
    pass: Option<Pass>,
    sequence: bool,
    /// The game's phase was the restart (game+0xf4 6): a pass launched or
    /// caught in this step is the inbound's (`FUN_0018e190`'s +8,
    /// `FUN_0018e210`).
    restart: bool,
}

impl Before {
    pub fn new(world: &World) -> Self {
        Self {
            live: world.phase == Phase::Live,
            pass: world.ball.pass,
            sequence: world.sequence.is_some(),
            restart: world.game_phase.phase == game_phase::phase::RESTART,
        }
    }
}

/// The lesson listener's words (`FUN_0018df00`, 0x2c bytes at
/// game+0x4c0; slot 79 clears them at each result, `FUN_0018dfc8`) and the
/// manager's (game+0x4c4..+0x500).
#[derive(Clone, Debug)]
struct Active {
    lesson: usize,
    /// +0x4e4 set: the restart that launched the lesson has ended (slot 28,
    /// `FUN_0018e458`); results before it are dropped (`FUN_0018f6b0`).
    armed: bool,
    /// +0x4e8: the person's actor then (Switch Players compares it).
    baseline: Option<usize>,
    /// The last pass launched's receiver (slot 18, `FUN_0018e190`).
    pass: Option<usize>,
    /// +8: it was launched in the restart (the inbound pass).
    inbound: bool,
    /// +0xc: to the passer himself.
    self_pass: bool,
    /// +0x10: the last bounce leg's selector (slot 39, `FUN_0018e038`):
    /// 0 off a head, 1 off the glass; never cleared but at a result.
    bounce: Option<u8>,
    /// +0x14: Give 'n Go's pass caught by the computer.
    cpu_caught: bool,
    /// +0x1c: Level 2's staged basket, passed when its script ends.
    staged: bool,
    /// +0x20: a pass to oneself caught (Advanced Dunks).
    caught_self: bool,
    /// +0x24: a kick pass launched; kept until the result.
    kick: bool,
    /// +0x28: a dunk or a leap took off (slot 4, the jump opcodes'
    /// `0x199868`/`0x19a5b4`) since the last pass launched.
    took_off: bool,
    /// game+0x4f8..: the Dive lesson's update each team slot's player was
    /// last diving with the ball loose (the manager's, by actor+0x280).
    dives: [Option<u64>; 3],
    /// Actor+0x1e4 by player: his last shot a fadeaway (8).
    fades: Vec<bool>,
    /// +0x4d0/+0x4d4: later callbacks may replace a pending outcome.
    pending: Option<(u32, bool)>,
    /// +0x4e4: the update the lesson started in (game+0x128 at the
    /// restart's end, slot 28), from which the checks count.
    start: Option<i64>,
    /// +0x4d8: the prompt is down and may be shown (set at the launch).
    may_show: bool,
    /// +0x4dc: Dunk to Lay-up's or Give 'n Go's first prompt is up; the
    /// second follows the take-off or the computer's catch.
    latched: bool,
    /// +0x4e0: updates before the manager looks at the prompt again.
    cooldown: u32,
    /// +0x4f0: updates before a tutorial page may open (2 at the launch).
    page_wait: u32,
    /// +0x4ec: a page opened before this lesson's start.
    page_shown: bool,
    /// +0x4f4: the Shot Clock page opened in this lesson (the clock is
    /// left to run).
    clock_page: bool,
    /// The manager returned at a prompt's post in the last update: the
    /// Switch Players check at its end was not reached.
    posted: bool,
}

/// One match's lessons: the run under way.
#[derive(Clone, Debug, Default)]
pub struct Run {
    active: Option<Active>,
    /// The tutorial pages the user has seen ([`page::bit`]; the game keeps
    /// it from the user's nibbles).
    pub seen: u8,
    /// What the manager posted in the last update.
    posts: Vec<Post>,
}

/// The side a lesson starts with the ball (`FUN_0018ebe0`, table
/// `0x3de308`): the person's, the other for 7..11 and 16.
pub fn expected_side(lesson: usize, person: usize) -> usize {
    if matches!(lesson, 7..=11 | 16) {
        1 - person
    } else {
        person
    }
}

/// The updates a result waits before its commit (`FUN_0018f6b0`, raw
/// `0x18f6dc..0x18f728`): `cvt.w.s(fps × 0.9)`, or `× 1.5` for Cancel
/// Gamebreaker (0x18).
pub fn delay(lesson: usize) -> u32 {
    let fps = crate::TICK_HZ as f32;
    (fps * if lesson == 0x18 { 1.5 } else { 0.9 }) as u32
}

/// The updates a lesson must have run before the Pass, Kick Pass, Trick
/// Move, Advanced Trick Move and Give 'n Go prompts (`DAT_004baa70`, set
/// by the static initialiser at `0x190af0` to `cvt.w.s(fps × 0.5)`).
pub fn ready_wait() -> i64 {
    (crate::TICK_HZ as f32 * 0.5) as i64
}

/// The updates a prompt shown or hidden stands before the manager looks
/// again (`DAT_004baa74`, `cvt.w.s(fps × 0.75)` at `0x190b20`).
pub fn prompt_hold() -> u32 {
    (crate::TICK_HZ as f32 * 0.75) as u32
}

/// The School's tutorial pages (the user's lesson nibbles 26..32,
/// `FUN_001eebb8`/`FUN_001eedb0`; `FUN_002bb338` builds each).
pub mod page {
    /// "Turbo", before the Dunk lesson (3).
    pub const TURBO: u8 = 0x1a;
    /// "Player Indicator", before the shot lessons (1, 2).
    pub const PLAYER_INDICATOR: u8 = 0x1b;
    /// "Shot Clock", when the clock reads 9 in lessons 1..3, 13 and 21.
    pub const SHOT_CLOCK: u8 = 0x1c;
    /// "Trick Points", before the trick lessons (6, 20).
    pub const TRICK_POINTS: u8 = 0x1d;
    /// "Gamebreaker", before the Gamebreaker lesson (14).
    pub const GAMEBREAKER: u8 = 0x1e;

    /// The page's bit in [`super::Run::seen`] (nibble 26 is bit 0).
    pub const fn bit(page: u8) -> u8 {
        1 << (page - TURBO)
    }
}

/// `FUN_0018f540` before a lesson's start: its first tutorial page the
/// user has not seen (the launch, `FUN_0018ec28`, asks it too: a page due
/// shortens the inbound's wait, `DAT_003c7af8`).
pub fn page_due(lesson: usize, seen: u8) -> Option<u8> {
    [
        (page::TURBO, lesson == 3),
        (page::PLAYER_INDICATOR, (1..=2).contains(&lesson)),
        (page::TRICK_POINTS, lesson == 6 || lesson == 0x14),
        (page::GAMEBREAKER, lesson == 0xe),
    ]
    .into_iter()
    .find(|&(p, due)| due && seen & page::bit(p) == 0)
    .map(|(p, _)| p)
}

/// What the manager posts in an update ([`Run::posts`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Post {
    /// `0x3c6970`: show the prompt of this lesson's record
    /// (`FUN_002ba3b0`). Dunk to Lay-up's and Give 'n Go's second prompt
    /// is record 1's (the lay-up's Shoot) and record 0's (the Pass).
    Prompt(usize),
    /// `0x3c6980`: hide it (`FUN_002bab48`).
    Hide,
    /// `0x3c6990`: a tutorial page ([`page`]); the user has seen it from
    /// here (`FUN_001eedb0` writes its nibble 8).
    Page(u8),
    /// vt+0xfc (`FUN_0013f188`): the shot clock full again.
    RefillClock,
    /// `FUN_001448e0(0, meter)`: this side's meter emptied at a commit
    /// (it was over 45,000).
    EmptyMeter(usize),
}

/// The person's actor as the manager reads it (`FUN_0018f4a0`: the first
/// whose controller is not the computer's).
pub fn persons_actor(world: &World) -> Option<usize> {
    world.people.iter().position(|&p| p)
}

/// The actor state bits the readiness checks read (actor+0xa4).
pub mod bits {
    /// `FUN_0016e990`: a jump shot, or a lay-up (with [`LAYUP`]).
    pub const SHOT: u32 = 1 << 3;
    /// `FUN_00154680`: throwing a pass.
    pub const THROW: u32 = 1 << 4;
    /// `FUN_0016e9a0`: a swipe.
    pub const SWIPE: u32 = 1 << 6;
    /// `FUN_0016e9d0`: a dunk.
    pub const DUNK: u32 = 1 << 7;
    /// `FUN_0016e9f0`: a rebound jump.
    pub const REBOUND: u32 = 1 << 8;
    /// `FUN_0016ea10`: a trick move.
    pub const TRICK: u32 = 1 << 10;
    /// `FUN_0016e9c0`: a block jump.
    pub const BLOCK: u32 = 1 << 12;
    /// `FUN_0016ea40`: a lay-up.
    pub const LAYUP: u32 = 1 << 13;
    /// `FUN_0016e9e0`: an alley-oop's leap.
    pub const LEAP: u32 = 1 << 15;
}

/// The pad word's pocket and cancel bits (`FUN_0012fca0`'s record, the
/// person's controller).
pub const POCKET: u32 = 0x800_0000;
pub const CANCEL: u32 = 0x1000_0000;

/// What the readiness checks read of the game (`FUN_0018f740` and its
/// checks): the world's, or a capture's sample in the tests.
pub trait Court {
    /// Ball+0xf4: who holds the ball.
    fn holder(&self) -> Option<usize>;
    /// `FUN_0018f4a0`: the person's actor.
    fn person(&self) -> Option<usize>;
    /// Actor+0x284.
    fn team(&self, i: usize) -> usize;
    /// Actor+0x280: his slot in his team.
    fn slot(&self, i: usize) -> usize;
    /// `FUN_00140920(game, team, k)`: the team's player in slot `k`.
    fn in_slot(&self, team: usize, k: usize) -> Option<usize>;
    /// Actor+0x30, +0x38.
    fn position(&self, i: usize) -> [f32; 2];
    /// `FUN_0016e930` (game+0x374): his distance to the rim the offence
    /// attacks.
    fn hoop_dist(&self, i: usize) -> f32;
    /// `FUN_0018d0c0`: the distance table's.
    fn between(&self, a: usize, b: usize) -> f32;
    /// `FUN_0017da80`: the stick angle from him toward the offence's rim.
    fn angle_to_hoop(&self, i: usize) -> i32;
    /// Actor+0xa4 ([`bits`]).
    fn bits(&self, i: usize) -> u32;
    /// Actor+0x1a0 is 1: dribbling.
    fn dribbling(&self, i: usize) -> bool;
    /// `FUN_0016ea70`: off balance or on the floor.
    fn down(&self, i: usize) -> bool;
    /// `FUN_0016c500(actor, 1)`.
    fn dunk_on(&self, i: usize) -> i32;
    /// `FUN_001332e0` of his AI's queue: the front behaviour's type.
    fn front_kind(&self, i: usize) -> i32;
    /// Game+0xb0 at `0x441808[team]`: the side's first turbo bar.
    fn turbo(&self, team: usize) -> f32;
    /// The team's meter (+0xc).
    fn meter(&self, team: usize) -> f32;
    /// The person's controller's pad word ([`POCKET`], [`CANCEL`]).
    fn pad(&self) -> u32;
    /// Game+0xf4.
    fn phase(&self) -> i32;
    /// Ball+0x180.
    fn ball_state(&self) -> i32;
}

impl Court for World {
    fn holder(&self) -> Option<usize> {
        self.handler()
    }
    fn person(&self) -> Option<usize> {
        persons_actor(self)
    }
    fn team(&self, i: usize) -> usize {
        self.team_of(i)
    }
    fn slot(&self, i: usize) -> usize {
        team_slot(self, i)
    }
    fn in_slot(&self, team: usize, k: usize) -> Option<usize> {
        (0..self.players.len())
            .filter(|&j| self.team_of(j) == team)
            .nth(k)
    }
    fn position(&self, i: usize) -> [f32; 2] {
        self.players[i].position
    }
    fn hoop_dist(&self, i: usize) -> f32 {
        let rim = self.rim_spot(crate::target_hoop(self.possession));
        let p = self.players[i].position;
        crate::math::len(p[0] - rim[0], p[1] - rim[1])
    }
    fn between(&self, a: usize, b: usize) -> f32 {
        let (p, q) = (self.players[a].position, self.players[b].position);
        crate::math::len(p[0] - q[0], p[1] - q[1])
    }
    fn angle_to_hoop(&self, i: usize) -> i32 {
        let rim = self.rim_spot(crate::target_hoop(self.possession));
        let p = self.players[i].position;
        angle_units(crate::math::atan2(p[0] - rim[0], p[1] - rim[1]))
    }
    /// As the AI's court reads them (`ai::court`): the bits from the
    /// player's action.
    fn bits(&self, i: usize) -> u32 {
        let p = &self.players[i];
        let mut out = 0;
        let mut set = |bit: u32, on: bool| out |= if on { bit } else { 0 };
        set(
            bits::SHOT,
            matches!(
                p.action,
                Action::Shooting { .. }
                    | Action::Layup { .. }
                    | Action::Dunking { layup: true, .. }
            ),
        );
        set(bits::THROW, matches!(p.action, Action::Passing { .. }));
        set(
            bits::SWIPE,
            match p.action {
                Action::Stealing { counter: false, .. } => true,
                Action::Stealing {
                    stance: Some(s), ..
                } => s.take(),
                _ => false,
            },
        );
        set(
            bits::DUNK,
            matches!(p.action, Action::Dunking { layup: false, .. }),
        );
        set(
            bits::REBOUND,
            matches!(p.action, Action::ReboundJump { .. }),
        );
        set(bits::TRICK, p.action.juking());
        set(bits::BLOCK, matches!(p.action, Action::Jumping { .. }));
        set(
            bits::LAYUP,
            matches!(
                p.action,
                Action::Layup { .. } | Action::Dunking { layup: true, .. }
            ),
        );
        set(bits::LEAP, p.leaping());
        out
    }
    fn dribbling(&self, i: usize) -> bool {
        self.players[i].dribble == 1
    }
    fn down(&self, i: usize) -> bool {
        World::down(self, i)
    }
    fn dunk_on(&self, i: usize) -> i32 {
        World::dunk_on(self, i, true)
    }
    fn front_kind(&self, i: usize) -> i32 {
        self.ai.brains.get(i).map_or(0, |b| b.front_kind())
    }
    fn turbo(&self, team: usize) -> f32 {
        self.turbo[crate::turbo::COMPUTER_SLOT[team.min(1)]]
    }
    fn meter(&self, team: usize) -> f32 {
        let t = &self.teams[team.min(1)];
        t.meter as f32 + t.meter_fraction
    }
    /// The person's pad as the step took it: Select alone (or with no
    /// turbo left) the pocket, with a turbo the cancel.
    fn pad(&self) -> u32 {
        let Some(input) = persons_actor(self).and_then(|k| self.previous_inputs.get(k)) else {
            return 0;
        };
        if input.buttons & crate::button::SELECT == 0 {
            0
        } else if input.turbos() == 0 {
            POCKET
        } else {
            CANCEL
        }
    }
    fn phase(&self) -> i32 {
        self.game_phase.phase
    }
    fn ball_state(&self) -> i32 {
        self.native_ball_state()
    }
}

/// `FUN_0018f928`: the holder can make his move: dribbling (actor+0x1a0
/// 1), not in a shot, dunk, leap or lay-up, not throwing a pass.
fn free_handler(c: &impl Court, h: usize) -> bool {
    c.dribbling(h)
        && c.bits(h) & (bits::SHOT | bits::DUNK | bits::LEAP | bits::LAYUP | bits::THROW) == 0
}

/// `FUN_0016c908`: out of the dunking area (22.5 across, 23.75 from the
/// rim; `DAT_003d9050`, `DAT_003d904c`).
fn far(c: &impl Court, h: usize) -> bool {
    !(c.position(h)[1].abs() < 22.5 && c.hoop_dist(h) < 23.75)
}

/// The short and long range lessons' 33.75 (the arc as the checks read it).
const RANGE: f32 = 33.75;

/// The meter's full value as the checks compare it (`50000.0`).
const FULL: f32 = crate::street::METER_FULL as f32;

/// `FUN_0018fa70`: Long Range Shot's (and Pick's first) check.
fn long_range(c: &impl Court, h: usize) -> bool {
    free_handler(c, h) && far(c, h) && c.hoop_dist(h) < RANGE
}

/// `FUN_0018fae8`: Dunk's and Dunk to Lay-up's (and Advanced Dunk's
/// first): within 13 of the rim.
fn close(c: &impl Court, h: usize) -> bool {
    free_handler(c, h) && c.hoop_dist(h) < 13.0
}

/// `FUN_0018fc90` and `FUN_0018fdd0`: the person within 10 of the holder
/// and their angles to the rim within 85 units (`0x42abaa8b`, 85.83
/// truncated by `cvt.w.s`), not in a rebound jump.
fn near_holder(c: &impl Court, h: usize, p: usize) -> bool {
    c.bits(p) & bits::REBOUND == 0
        && c.between(h, p) < 10.0
        && angle_diff(c.angle_to_hoop(p), c.angle_to_hoop(h)).abs() <= 85
}

/// The opponents of the holder's side, by slot.
fn opponents(c: &impl Court, h: usize) -> impl Iterator<Item = usize> + '_ {
    let other = 1 - c.team(h).min(1);
    (0..3).filter_map(move |k| c.in_slot(other, k))
}

/// `FUN_0018f740`: lesson `lesson`'s readiness check, `elapsed` updates
/// after its start (game+0x128 − +0x4e4). The checks by the lesson
/// (`0x18f7a0..`): the holder is ball+0xf4's, the person `FUN_0018f4a0`'s.
pub fn ready(lesson: usize, c: &impl Court, elapsed: i64) -> bool {
    let holder = c.holder();
    let person = c.person();
    let team = person.map_or(1, |p| c.team(p));
    let waited = elapsed >= ready_wait();
    let turbo = || c.turbo(team) > 0.3;
    let tricking = |h: usize| c.bits(h) & bits::TRICK != 0;
    match lesson {
        // Pass, Kick Pass (`FUN_0018f9b8`): a holder not in a trick, half
        // a second in.
        0 | 0x11 => holder.is_some_and(|h| !tricking(h) && waited),
        // Short Range Shot (`FUN_0018fa20`): in the dunking area.
        1 => holder.is_some_and(|h| free_handler(c, h) && !far(c, h)),
        // Long Range Shot (`FUN_0018fa70`).
        2 => holder.is_some_and(|h| long_range(c, h)),
        // Dunk, Dunk to Lay-up (`FUN_0018fae8`).
        3 | 4 => holder.is_some_and(|h| close(c, h)),
        // Alley-Oop (`FUN_0018fb50`): a teammate of the holder leaping.
        5 => holder.is_some_and(|h| {
            (0..3)
                .filter(|&k| k != c.slot(h))
                .filter_map(|k| c.in_slot(c.team(h), k))
                .any(|j| c.bits(j) & bits::LEAP != 0)
        }),
        // Trick Move (`FUN_0018fbc8`).
        6 => holder.is_some_and(|h| !tricking(h) && waited && turbo()),
        // Switch Players (`FUN_0018fc80`): out of the restart.
        7 => c.phase() != game_phase::phase::RESTART,
        // Steal (`FUN_0018fc90`).
        8 => match (holder, person) {
            (Some(h), Some(p)) => free_handler(c, h) && !tricking(h) && near_holder(c, h, p),
            _ => false,
        },
        // Block, Turbo Block (`FUN_0018ff00`): the holder shooting,
        // dunking or laying up, the person free in front of him.
        9 | 0xb => match (holder, person) {
            (Some(h), Some(p)) => {
                c.bits(h) & (bits::SHOT | bits::DUNK | bits::LAYUP) != 0
                    && c.bits(p) & (bits::SWIPE | bits::BLOCK | bits::REBOUND) == 0
                    && !c.down(p)
                    && c.between(h, p) < 10.0
                    && c.hoop_dist(p) - 3.0 < c.hoop_dist(h)
            }
            _ => false,
        },
        // Trick Counter (`FUN_0018fdd0`).
        10 => match (holder, person) {
            (Some(h), Some(p)) => free_handler(c, h) && near_holder(c, h, p),
            _ => false,
        },
        // Pump Fake (`FUN_00190028`): no dunk on, an opponent within 8
        // between him and the rim.
        0xc => holder.is_some_and(|h| {
            c.dunk_on(h) != 2
                && opponents(c, h)
                    .any(|o| c.between(h, o) < 8.0 && c.hoop_dist(o) < c.hoop_dist(h) && !c.down(o))
        }),
        // Fadeaway (`FUN_00190118`).
        0xd => holder.is_some_and(|h| c.hoop_dist(h) < RANGE),
        // Gamebreaker (`FUN_00190168`), Level 2 (`FUN_001901d8`): the
        // second meter full (`DAT_003d30bc + 0xcc`).
        0xe | 0x19 => holder.is_some_and(|h| c.meter(1) == FULL && c.hoop_dist(h) < RANGE),
        // Pick (`FUN_00190610`): Long Range Shot's, and no teammate of
        // the person's side already on a pick (types 1 and 2).
        0xf => holder.is_some_and(|h| {
            long_range(c, h)
                && (0..3)
                    .filter(|&k| k != c.slot(h))
                    .filter_map(|k| c.in_slot(team, k))
                    .all(|j| !matches!(c.front_kind(j), 1 | 2))
        }),
        // Dive (`FUN_001906e0`): nobody holds it and it is loose.
        0x10 => holder.is_none() && c.ball_state() == game_phase::ball::LOOSE,
        // Off the Heezay (`FUN_001904b8`): an opponent within 6, nearer
        // the rim, and a turbo.
        0x12 => holder.is_some_and(|h| {
            free_handler(c, h)
                && opponents(c, h)
                    .any(|o| c.between(h, o) < 6.0 && !c.down(o) && c.hoop_dist(o) < c.hoop_dist(h))
                && turbo()
        }),
        // Back 2 Papa (`FUN_001903b0`): an opponent within 6 and a turbo.
        0x13 => holder.is_some_and(|h| {
            free_handler(c, h) && opponents(c, h).any(|o| c.between(h, o) < 6.0) && turbo()
        }),
        // Advanced Trick Move (`FUN_00190700`).
        0x14 => holder.is_some() && waited && turbo(),
        // Advanced Dunk (`FUN_001907a8`): Dunk's, and 23..29 across the
        // court's length, within 8 of its middle.
        0x15 => holder.is_some_and(|h| {
            let [x, z] = c.position(h);
            close(c, h) && 23.0 < x.abs() && x.abs() < 29.0 && z.abs() < 8.0
        }),
        // Give 'n Go (`FUN_00190348`).
        0x16 => holder.is_some_and(|h| !tricking(h) && waited),
        // Pocket, Cancel (`FUN_001902c8`, `FUN_00190248`): the meter full
        // and the press not down already.
        0x17 => holder.is_some() && c.meter(1) == FULL && c.pad() & POCKET == 0,
        0x18 => holder.is_some() && c.meter(1) == FULL && c.pad() & CANCEL == 0,
        _ => false,
    }
}

/// A player's slot in his team (actor+0x280).
fn team_slot(world: &World, player: usize) -> usize {
    let team = world.team_of(player);
    (0..player).filter(|&j| world.team_of(j) == team).count()
}

/// `FUN_0016eab0`: diving.
fn diving(world: &World, player: usize) -> bool {
    matches!(
        world.players.get(player).map(|p| &p.action),
        Some(Action::Dive { .. })
    )
}

/// The blocker's clip is the turbo block's, record 0x1b5
/// (`FUN_0018e858`).
fn turbo_block(world: &World, player: usize) -> bool {
    matches!(
        world.players.get(player).map(|p| &p.action),
        Some(Action::Jumping { block: Some(jump), .. })
            if crate::block::SCRIPTS
                .get(usize::from(jump.script))
                .is_some_and(|s| s.records.contains(&0x1b5))
    )
}

impl Active {
    fn new(lesson: usize, armed: bool, players: usize) -> Self {
        Self {
            lesson,
            armed,
            baseline: None,
            pass: None,
            inbound: false,
            self_pass: false,
            bounce: None,
            cpu_caught: false,
            staged: false,
            caught_self: false,
            kick: false,
            took_off: false,
            dives: [None; 3],
            fades: vec![false; players],
            pending: None,
            start: None,
            may_show: true,
            latched: false,
            cooldown: 0,
            page_wait: 2,
            page_shown: false,
            clock_page: false,
            posted: false,
        }
    }

    /// `FUN_0018f540`: the tutorial page due, if any. Before the start
    /// (once a lesson) the lesson's first page not yet seen; after it the
    /// Shot Clock's, when the clock reads 9 in the shot lessons and the
    /// ball is not in a shot.
    fn page(&mut self, world: &World, seen: u8) -> Option<u8> {
        let unseen = |p: u8| seen & page::bit(p) == 0;
        let lesson = self.lesson;
        if self.start.is_none() {
            if self.page_shown {
                return None;
            }
            return page_due(lesson, seen);
        }
        let clock = match world.rules.shot_clock {
            Some(_) => world.shot_clock,
            None => 24.0,
        };
        let due = unseen(page::SHOT_CLOCK)
            && world.native_ball_state() != game_phase::ball::SHOT
            && (clock as i32).rem_euclid(60) == 9
            && matches!(lesson, 1..=3 | 0xd | 0x15);
        // `FUN_0018f540` notes it itself (+0x4f4).
        self.clock_page |= due;
        due.then_some(page::SHOT_CLOCK)
    }

    /// `FUN_0018f6b0`: ignored before the start; else the result and the
    /// delay ([`delay`]).
    fn result(&mut self, passed: bool) {
        if self.armed {
            self.pending = Some((delay(self.lesson), passed));
        }
    }
}

impl Run {
    /// The pending result's updates left and outcome (+0x4d0, +0x4d4).
    pub fn pending(&self) -> Option<(u32, bool)> {
        self.active.as_ref().and_then(|run| run.pending)
    }

    /// Whether the lesson has started (+0x4e4 set at the restart's end).
    pub fn started(&self) -> bool {
        self.active.as_ref().is_some_and(|run| run.armed)
    }

    /// What the manager posted in the last update, in order.
    pub fn posts(&self) -> &[Post] {
        &self.posts
    }

    /// One update of lesson `lesson` (the game's vt+0xb4): the listener on
    /// this step's events, then the manager on the step's state.
    /// The result once its delay has run (the caller commits it and stops
    /// advancing this run).
    pub fn advance(&mut self, lesson: usize, before: Before, world: &World) -> Option<bool> {
        let Run {
            active,
            seen,
            posts,
        } = self;
        posts.clear();
        let selected = persons_actor(world)?;
        let side = world.team_of(selected);
        let expected = expected_side(lesson, side);
        // The game's counter as this update's manager reads it (game+0x128:
        // the step that ends here is the update before it, its count
        // already raised).
        let now = world.tick as i64;
        if active.as_ref().is_none_or(|run| run.lesson != lesson) {
            let armed = before.live || world.phase == Phase::Live;
            let mut run = Active::new(lesson, armed, world.players.len());
            run.baseline = armed.then_some(selected);
            run.start = armed.then_some(now - 1);
            *active = Some(run);
        }
        let run = active.as_mut().expect("bound above");

        // Slot 28 (`FUN_0013e4c8`, raw `0x13e6a0`; `FUN_0018e458`): the
        // restart's end, its update the start (+0x4e4).
        if world
            .events
            .iter()
            .any(|event| matches!(event, Event::Inbound { .. }))
        {
            run.armed = true;
            run.baseline = Some(selected);
            run.start = Some(now - 1);
        }
        // Slot 39 (`FUN_00148378`): a bounce leg ends.
        if let Some(pass) = before.pass
            && !matches!(pass.via, Via::Direct)
            && world.ball.pass.is_some_and(|after| {
                after.from == pass.from && after.to == pass.to && after.via == Via::Direct
            })
        {
            run.bounce = Some(match pass.via {
                Via::Head { .. } => 0,
                Via::Glass { .. } => 1,
                Via::Direct | Via::Floor => unreachable!(),
            });
        }
        // Slot 62 (`FUN_00337250`): Level 2's staged sequence ends.
        if run.staged && before.sequence && world.sequence.is_none() {
            run.result(true);
        }
        for event in &world.events {
            match *event {
                // Slot 18 (`FUN_0018e190`): a pass launched.
                Event::Pass { from, to, style } => {
                    run.pass = Some(to);
                    run.inbound = before.restart;
                    run.self_pass = from == to;
                    run.caught_self = false;
                    run.kick |= style == PassStyle::Kick;
                    run.took_off = false;
                }
                // Slot 19 (`FUN_0018e210`): the pass caught by the side.
                Event::Catch { player } => {
                    if run.pass == Some(player)
                        && !before.restart
                        && !run.inbound
                        && world.game_phase.possession == side as i32
                    {
                        if !run.self_pass && lesson == 0 {
                            run.result(true);
                        } else {
                            match lesson {
                                17 if run.kick => run.result(true),
                                18 if run.bounce == Some(0) => run.result(true),
                                19 if run.bounce == Some(1) => run.result(true),
                                22 => {
                                    if !world.people.get(player).copied().unwrap_or(false) {
                                        run.cpu_caught = true;
                                    } else if run.cpu_caught {
                                        run.result(true);
                                    }
                                }
                                _ => (),
                            }
                        }
                        if run.self_pass {
                            run.caught_self = true;
                        }
                    }
                }
                // Slot 4: a dunk takes off.
                Event::Dunk { .. } => run.took_off = true,
                // Slot 13 (`FUN_0018e6f8`): a trick lands; Advanced Trick
                // Move checks actor+0x264, not the value class.
                Event::Trick { script, .. } if world.game_phase.possession == side as i32 => {
                    if lesson == 6 || (lesson == 20 && matches!(script, 5 | 20 | 24 | 21)) {
                        run.result(true);
                    }
                }
                // Slot 15 (`FUN_0018e7b0`): a steal, or a trick counter by
                // a person in the stance.
                Event::Steal { .. } => match lesson {
                    8 => run.result(true),
                    25 => run.result(false),
                    _ => (),
                },
                Event::Counter { player, .. } => match lesson {
                    8 => run.result(true),
                    10 if world.people.get(player).copied().unwrap_or(false) => run.result(true),
                    25 => run.result(false),
                    _ => (),
                },
                // Slot 16 (`FUN_0018e858`): a person's block; Turbo Block's
                // on the turbo block's clip.
                Event::Block { player, .. }
                    if world.people.get(player).copied().unwrap_or(false) =>
                {
                    if lesson == 9 || (lesson == 11 && turbo_block(world, player)) {
                        run.result(true);
                    }
                }
                // Slot 26 (`FUN_0018e498`): a basket by the person's side.
                // Raw 16b158..174 selects native base2/3 from shot range;
                // 1734f4 -> 14aaf8 -> 1515a8 carries it to packet+c.
                // Display Score.points may instead be 1/2.
                Event::Score {
                    team,
                    player,
                    basket,
                    ..
                } if team == side => match (lesson, basket) {
                    (1, Basket::Gamebreaker { long: false, .. })
                        if !before.sequence && world.sequence.is_none() =>
                    {
                        run.result(true)
                    }
                    (1, Basket::Jumper { long: false })
                    | (2, Basket::Jumper { long: true })
                    | (2, Basket::Gamebreaker { long: true, .. })
                    | (3, Basket::Dunk { .. } | Basket::AlleyOop { .. })
                    | (14, Basket::Gamebreaker { .. }) => run.result(true),
                    // The lay-up after a dunk's take-off, or a gamebreaker.
                    (4, Basket::Layup) if run.took_off => run.result(true),
                    (4, Basket::Gamebreaker { .. }) => run.result(true),
                    (5, Basket::AlleyOop { .. }) if !run.self_pass => run.result(true),
                    // The shooter's kind word is a fadeaway's.
                    (13, _) if run.fades.get(player).copied().unwrap_or(false) => run.result(true),
                    // A pass to oneself caught, and the move's 0x800.
                    (21, _) if run.caught_self && world.advanced_move(player) => run.result(true),
                    (25, Basket::Gamebreaker { level: 2, .. }) => {
                        if world.sequence.is_some() {
                            run.staged = true;
                        } else {
                            run.result(true);
                        }
                    }
                    (25, Basket::Gamebreaker { .. }) => (),
                    (25, _) => run.result(false),
                    _ => (),
                },
                // Slot 27 (the ball's update, raw `0x14d66c`): a shot done
                // without a basket.
                Event::RimOut if matches!(lesson, 8..=11 | 14 | 25) => run.result(false),
                // Slot 52: the shot clock runs out.
                Event::ShotClockViolation { .. } => run.result(false),
                // Slot 67 (`FUN_00143c20`): a full meter's hold runs out.
                Event::MeterDropped { .. } if matches!(lesson, 14 | 23..=25) => run.result(false),
                // Slots 70 and 71: a pocket, a cancel.
                Event::Pocket { .. } => run.result(lesson == 23),
                Event::Cancel { .. } => run.result(lesson == 24),
                // Slot 73 (`FUN_0013f950`): a defender bit on a pump fake.
                Event::PumpFakeBite { .. }
                    if lesson == 12 && world.game_phase.possession == side as i32 =>
                {
                    run.result(true)
                }
                // Slot 74: a contact of kind 3 (a pick).
                Event::Pick { .. }
                    if lesson == 15 && world.game_phase.possession == side as i32 =>
                {
                    run.result(true)
                }
                _ => (),
            }
            // Slot 24 (`FUN_0018e350`): the take, for Dive: the person's
            // side's taker diving, or within `fps × 0.12` (7) updates of
            // his dive at the loose ball.
            if lesson == 16
                && let Event::Pickup { player } | Event::Catch { player } = *event
                && world.team_of(player) == side
                && (diving(world, player)
                    || run.dives[team_slot(world, player).min(2)]
                        .is_some_and(|at| world.tick.saturating_sub(at) < 7))
            {
                run.result(true);
            }
        }

        // What the next update's manager reads: the shot kind words and
        // the Dive lesson's dives at the loose ball (`0x18ef70..`).
        for (k, p) in world.players.iter().enumerate() {
            match p.action {
                Action::Shooting { fade, script, .. } => {
                    let name = crate::timing::SHOT_SCRIPTS
                        .get(usize::from(script))
                        .map_or("", |s| s.name);
                    if let Some(word) = run.fades.get_mut(k) {
                        *word = fade || name.contains("fadeaway");
                    }
                }
                Action::Dunking { .. } => {
                    if let Some(word) = run.fades.get_mut(k) {
                        *word = false;
                    }
                }
                _ => (),
            }
        }
        // The manager's update (`FUN_0018eec0`, the game's vt+0xd8): the
        // game's update (`FUN_0013d868`) raises its counter, runs the phase
        // machine, the shot clock and the meters and then this, before the
        // actors and the ball update; so it reads the whole step's state,
        // and a result their posts set this step is counted down once
        // already.
        //
        // First (`0x18eee0..`): with the Shot Clock page seen and not
        // opened in this lesson, the clock is refilled every update while
        // the lesson is the person's side's (`FUN_0018ebe0` 1, vt+0xfc).
        if !run.clock_page && *seen & page::bit(page::SHOT_CLOCK) != 0 && expected == 1 {
            posts.push(Post::RefillClock);
        }
        run.page_wait = run.page_wait.saturating_sub(1);
        run.cooldown = run.cooldown.saturating_sub(1);
        let posted = std::mem::take(&mut run.posted);
        // The Dive lesson's dives at the loose ball (`0x18ef70..`).
        if lesson == 16 {
            match world.handler() {
                None => {
                    if let Some(k) = persons_actor(world)
                        && diving(world, k)
                    {
                        run.dives[team_slot(world, k).min(2)] = Some(world.tick);
                    }
                }
                Some(h) => run.dives[team_slot(world, h).min(2)] = None,
            }
        }
        // Switch Players: the person drives another actor than at the start.
        // The action tap switches before the game's update (its task comes
        // first), so the manager that sees it is the one that ran in the
        // step that switched: here, counted down at once. It sits at the
        // manager's end (`0x18f22c..`), which an update that posted a
        // prompt did not reach.
        if run.pending.is_none()
            && !posted
            && lesson == 7
            && world.game_phase.possession == expected as i32
            && run.baseline.is_some()
            && persons_actor(world).is_some_and(|k| Some(k) != run.baseline)
        {
            run.result(true);
        }
        if let Some((left, passed)) = run.pending.as_mut() {
            *left = left.saturating_sub(1);
            if *left == 0 {
                // The commit waits while a script plays (states 3..7).
                if world.sequence.is_none() {
                    // A meter over 45,000 is emptied with the commit.
                    if Court::meter(world, expected) > 45_000.0 {
                        posts.push(Post::EmptyMeter(expected));
                    }
                    return Some(*passed);
                }
                *left = 1;
            }
            return None;
        }
        if world.game_phase.possession != -1 && world.game_phase.possession != expected as i32 {
            // The expected side lost the ball (all but Level 2).
            if lesson != 25 {
                run.result(false);
            }
            return None;
        }

        // The prompt (`0x18f13c..0x18f22c`): while the cooldown runs it
        // stands; else, once started, a check passing shows it and one
        // failing hides it, each standing `prompt_hold` updates. Dunk to
        // Lay-up's and Give 'n Go's first prompt latches: a failing check
        // hides it but it does not come back until the second.
        let elapsed = now - run.start.unwrap_or(-1);
        if run.cooldown == 0 {
            if run.may_show {
                if run.start.is_some() && ready(lesson, world, elapsed) {
                    run.may_show = false;
                    posts.push(Post::Prompt(lesson));
                    run.cooldown = prompt_hold();
                    run.latched = matches!(lesson, 4 | 0x16);
                    run.posted = true;
                    return None;
                }
            } else if !ready(lesson, world, elapsed) {
                run.may_show = !run.latched;
                posts.push(Post::Hide);
                run.cooldown = prompt_hold();
                run.posted = true;
                return None;
            }
        }
        if !run.latched {
            // A tutorial page (`FUN_0018f540`) two updates after the
            // launch, or the Shot Clock's at 9.
            if run.page_wait == 0
                && let Some(p) = run.page(world, *seen)
            {
                run.page_shown = true;
                *seen |= page::bit(p);
                posts.push(Post::Page(p));
            }
        } else if lesson == 0x16 && run.cpu_caught {
            // Give 'n Go's second prompt: Pass, once the computer caught
            // the pass.
            posts.push(Post::Prompt(0));
            run.latched = false;
        } else if lesson == 4 && run.took_off {
            // Dunk to Lay-up's: Shoot, once the dunk took off.
            posts.push(Post::Prompt(1));
            run.latched = false;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Hoop, Player, Rules};

    /// Six on the court, live, the person on 0 (team 0) and the computer
    /// on the rest.
    fn world() -> World {
        let players = (0..6)
            .map(|k| Player::new(k / 3, [0.0, k as f32], 0.0))
            .collect();
        let hoop = |x: f32| Hoop {
            rim: [x, 11.75, 0.0],
            rim_radius: 0.75,
        };
        let mut world = World::new([hoop(37.0), hoop(-37.0)], players, Rules::default(), 1);
        world.phase = Phase::Live;
        world.people = (0..6).map(|k| k == 0).collect();
        world
    }

    /// `events` in one update of `lesson` with the ball side `side`'s:
    /// the pending result after it.
    fn after(lesson: usize, side: i32, events: Vec<Event>) -> Option<(u32, bool)> {
        let mut w = world();
        w.game_phase.possession = side;
        w.game_phase.phase = game_phase::phase::FRONT_COURT;
        let mut run = Run::default();
        let before = Before::new(&w);
        run.advance(lesson, before, &w);
        w.events = events;
        run.advance(lesson, before, &w);
        run.pending()
    }

    fn score(basket: Basket) -> Event {
        Event::Score {
            team: 0,
            player: 0,
            points: 1,
            basket,
        }
    }

    fn pass(from: usize, to: usize, style: PassStyle) -> Event {
        Event::Pass { from, to, style }
    }

    /// A post's result, counted down once by the same update's manager.
    const PASSED: Option<(u32, bool)> = Some((53, true));
    const FAILED: Option<(u32, bool)> = Some((53, false));

    #[test]
    fn each_lessons_listener_slot_decides_as_the_originals() {
        let normal = PassStyle::Normal;
        // Pass (slot 19): a pass to a teammate caught, not to oneself.
        let caught = |k| Event::Catch { player: k };
        assert_eq!(after(0, 0, vec![pass(0, 1, normal), caught(1)]), PASSED);
        assert_eq!(after(0, 0, vec![pass(0, 0, normal), caught(0)]), None);
        // Kick Pass needs the kick (+0x24).
        assert_eq!(
            after(17, 0, vec![pass(0, 1, PassStyle::Kick), caught(1)]),
            PASSED
        );
        assert_eq!(after(17, 0, vec![pass(0, 1, normal), caught(1)]), None);
        // Dunk to Lay-up: the lay-up after a dunk's take-off (slot 4), or a
        // gamebreaker's basket.
        let dunk = Event::Dunk { player: 0, hoop: 0 };
        assert_eq!(after(4, 0, vec![dunk, score(Basket::Layup)]), PASSED);
        assert_eq!(after(4, 0, vec![score(Basket::Layup)]), None);
        let gamebreaker = |level| Basket::Gamebreaker {
            level,
            long: false,
            dunk: false,
        };
        assert_eq!(after(4, 0, vec![score(gamebreaker(1))]), PASSED);
        // Alley-Oop: an oop, not off a pass to oneself.
        let oop = score(Basket::AlleyOop { advanced: false });
        assert_eq!(after(5, 0, vec![pass(1, 0, normal), oop]), PASSED);
        assert_eq!(after(5, 0, vec![pass(0, 0, normal), oop]), None);
        // Fadeaway: the shooter's kind word a fadeaway's, not any jumper.
        assert_eq!(
            after(13, 0, vec![score(Basket::Jumper { long: false })]),
            None
        );
        // Pump Fake: a defender's bite (slot 73), not the fake.
        assert_eq!(after(12, 0, vec![Event::PumpFake { player: 0 }]), None);
        assert_eq!(
            after(12, 0, vec![Event::PumpFakeBite { player: 3 }]),
            PASSED
        );
        // Pick (slot 74), with the ball the side's.
        let pick = Event::Pick {
            screener: 1,
            defender: 3,
        };
        assert_eq!(after(15, 0, vec![pick]), PASSED);
        // Steal (slot 15): anyone's take; Trick Counter: a person's counter.
        let steal = Event::Steal {
            player: 4,
            from: 1,
            intercepted_pass: false,
        };
        assert_eq!(after(8, 1, vec![steal]), PASSED);
        assert_eq!(
            after(10, 1, vec![Event::Counter { player: 0, from: 3 }]),
            PASSED
        );
        assert_eq!(
            after(10, 1, vec![Event::Counter { player: 1, from: 3 }]),
            None
        );
        // Block (slot 16): a person's; Turbo Block's on its clip only.
        let block = Event::Block {
            player: 0,
            shooter: 3,
        };
        assert_eq!(after(9, 1, vec![block]), PASSED);
        assert_eq!(after(11, 1, vec![block]), None);
        // A shot done without a basket (slot 27) fails the defence and the
        // gamebreaker lessons.
        assert_eq!(after(9, 1, vec![Event::RimOut]), FAILED);
        assert_eq!(after(14, 0, vec![Event::RimOut]), FAILED);
        assert_eq!(after(1, 0, vec![Event::RimOut]), None);
        // The meter's hold running out fails the gamebreaker lessons (67).
        let dropped = Event::MeterDropped { team: 0 };
        assert_eq!(after(23, 0, vec![dropped]), FAILED);
        assert_eq!(after(24, 0, vec![dropped]), Some((89, false)));
        assert_eq!(after(3, 0, vec![dropped]), None);
        // Pocket, Cancel (slots 70, 71) and their delays.
        assert_eq!(after(23, 0, vec![Event::Pocket { team: 0 }]), PASSED);
        assert_eq!(
            after(24, 0, vec![Event::Cancel { team: 0 }]),
            Some((89, true))
        );
        // Level 2: a Level 1 basket does nothing, an ordinary one fails.
        assert_eq!(after(25, 0, vec![score(gamebreaker(1))]), None);
        assert_eq!(
            after(25, 0, vec![score(Basket::Jumper { long: false })]),
            FAILED
        );
        assert_eq!(after(25, 0, vec![score(gamebreaker(2))]), PASSED);
        // Range by the basket's base: short and long.
        assert_eq!(
            after(1, 0, vec![score(Basket::Jumper { long: false })]),
            PASSED
        );
        assert_eq!(
            after(1, 0, vec![score(Basket::Jumper { long: true })]),
            None
        );
        assert_eq!(
            after(2, 0, vec![score(Basket::Jumper { long: true })]),
            PASSED
        );
    }

    #[test]
    fn the_inbound_pass_and_results_before_the_start_do_not_count() {
        // A pass launched in the restart (game phase 6) is the inbound's.
        let mut w = world();
        w.game_phase.possession = 0;
        w.game_phase.phase = game_phase::phase::RESTART;
        let mut run = Run::default();
        let before = Before::new(&w);
        run.advance(0, before, &w);
        w.events = vec![pass(0, 1, PassStyle::Normal)];
        run.advance(0, before, &w);
        w.game_phase.phase = game_phase::phase::FRONT_COURT;
        w.events = vec![Event::Catch { player: 1 }];
        run.advance(0, Before::new(&w), &w);
        assert_eq!(run.pending(), None);
        // A lesson launched into a restart starts at its end: a result
        // before it is dropped (`FUN_0018f6b0`).
        let mut w = world();
        w.phase = Phase::DeadBall { left: 1.0, team: 0 };
        let mut run = Run::default();
        let before = Before::new(&w);
        w.events = vec![Event::Pocket { team: 0 }];
        run.advance(23, before, &w);
        assert!(!run.started());
        assert_eq!(run.pending(), None);
        w.events = vec![Event::Inbound { player: 0 }];
        run.advance(23, before, &w);
        assert!(run.started());
    }

    #[test]
    fn a_result_commits_after_its_delay_and_a_later_callback_replaces_it() {
        let mut w = world();
        w.game_phase.possession = 0;
        w.game_phase.phase = game_phase::phase::FRONT_COURT;
        let mut run = Run::default();
        let before = Before::new(&w);
        w.events = vec![Event::Pocket { team: 0 }];
        assert_eq!(run.advance(23, before, &w), None);
        w.events.clear();
        for _ in 0..10 {
            assert_eq!(run.advance(23, before, &w), None);
        }
        // A shot clock violation replaces the success and restarts the
        // delay; the update it is posted in counts once (the shot clock runs
        // before the manager in the game's update) and the commit comes 53
        // updates later, 54 after the post.
        w.events = vec![Event::ShotClockViolation { team: 0 }];
        assert_eq!(run.advance(23, before, &w), None);
        assert_eq!(run.pending(), Some((53, false)));
        w.events.clear();
        let mut updates = 0;
        loop {
            updates += 1;
            if let Some(passed) = run.advance(23, before, &w) {
                assert!(!passed);
                break;
            }
        }
        assert_eq!(updates, 53);
        // The expected side losing the ball fails all but Level 2: the
        // manager's own result, counted from the next update.
        let mut w = world();
        w.game_phase.possession = 1;
        let mut run = Run::default();
        run.advance(3, Before::new(&w), &w);
        assert_eq!(run.pending(), Some((54, false)));
        let mut run = Run::default();
        run.advance(25, Before::new(&w), &w);
        assert_eq!(run.pending(), None);
    }

    /// The person (0) holding the ball for team 0, dribbling at `at`.
    fn holding(at: [f32; 2]) -> World {
        let mut w = world();
        w.possession = 0;
        w.game_phase.possession = 0;
        w.game_phase.phase = game_phase::phase::FRONT_COURT;
        w.players[0].position = at;
        w.players[0].dribble = 1;
        w.ball.hold(0);
        w
    }

    #[test]
    fn the_readiness_checks_read_the_court_as_fun_0018f740() {
        let wait = ready_wait();
        assert_eq!((wait, prompt_hold()), (30, 45));
        // Pass waits half a second from the start.
        let w = holding([20.0, 0.0]);
        assert!(!ready(0, &w, wait - 1) && ready(0, &w, wait));
        // Short Range: in the dunking area (23.75 from the rim, 22.5
        // across); Long Range: out of it and within 33.75.
        assert!(ready(1, &holding([20.0, 0.0]), 0));
        assert!(!ready(2, &holding([20.0, 0.0]), 0));
        assert!(ready(2, &holding([10.0, 0.0]), 0));
        assert!(!ready(2, &holding([0.0, 0.0]), 0));
        // Not before he dribbles (actor+0x1a0 1 from his first step).
        let mut w = holding([20.0, 0.0]);
        w.players[0].dribble = 0;
        assert!(!ready(1, &w, 0));
        // Dunk: within 13; Advanced Dunk: 23..29 across and 8 from the
        // middle (the rim at 37: 8..14 from it, so 13 limits it too).
        assert!(ready(3, &holding([25.0, 0.0]), 0) && !ready(3, &holding([23.0, 0.0]), 0));
        assert!(ready(0x15, &holding([27.0, 0.0]), 0));
        assert!(!ready(0x15, &holding([30.0, 0.0]), 0));
        // Fadeaway and the gamebreakers within 33.75; the gamebreakers on
        // the second side's full meter.
        assert!(ready(0xd, &holding([10.0, 0.0]), 0));
        let mut w = holding([10.0, 0.0]);
        assert!(!ready(0xe, &w, 0));
        w.teams[1].meter = crate::street::METER_FULL;
        assert!(ready(0xe, &w, 0) && ready(0x17, &w, 0) && ready(0x18, &w, 0));
        // Switch Players: any time out of the restart.
        let mut w = world();
        w.game_phase.phase = game_phase::phase::BACK_COURT;
        assert!(ready(7, &w, 0));
        w.game_phase.phase = game_phase::phase::RESTART;
        assert!(!ready(7, &w, 0));
        // Dive: nobody holds it and it is loose.
        let mut w = world();
        w.ball.state = crate::BallState::Loose;
        assert!(ready(0x10, &w, 0));
        // Steal: the person within 10 of a dribbling holder, their angles
        // to the rim within 85 units.
        let mut w = holding([10.0, 0.0]);
        w.ball.hold(3);
        w.players[3].position = [10.0, 0.0];
        w.players[3].dribble = 1;
        w.players[0].position = [14.0, 0.0];
        assert!(ready(8, &w, 0) && ready(10, &w, 0));
        w.players[0].position = [10.0, 11.0];
        assert!(!ready(8, &w, 0));
        // Pump Fake: an opponent within 8 between him and the rim.
        let mut w = holding([10.0, 0.0]);
        for k in 3..6 {
            w.players[k].position = [-30.0, 10.0 * k as f32];
        }
        assert!(!ready(0xc, &w, 0));
        w.players[3].position = [15.0, 0.0];
        assert!(ready(0xc, &w, 0));
        // Off the Heezay and Back 2 Papa want a turbo over 0.3.
        w.players[3].position = [14.0, 0.0];
        assert!(ready(0x12, &w, 0) && ready(0x13, &w, 0));
        w.turbo[crate::turbo::COMPUTER_SLOT[0]] = 0.3;
        assert!(!ready(0x12, &w, 0) && !ready(0x13, &w, 0));
    }

    #[test]
    fn the_prompt_shows_once_ready_hides_when_not_and_stands_its_hold() {
        let mut w = holding([20.0, 0.0]);
        w.phase = Phase::DeadBall { left: 1.0, team: 0 };
        let mut run = Run::default();
        let before = Before::new(&w);
        // Not before the start.
        run.advance(0, before, &w);
        assert_eq!(run.posts(), &[]);
        w.events = vec![Event::Inbound { player: 0 }];
        run.advance(0, before, &w);
        w.events.clear();
        // The start is the update the restart ended in: the prompt comes
        // `ready_wait` updates on (the manager reads the raised counter).
        let mut shown = None;
        for k in 1..60 {
            w.tick += 1;
            run.advance(0, before, &w);
            if run.posts() == [Post::Prompt(0)] {
                shown = Some(k);
                break;
            }
            assert_eq!(run.posts(), &[]);
        }
        assert_eq!(shown, Some(ready_wait() - 1));
        // The ball let go: the prompt stands its hold, then hides.
        w.ball.state = crate::BallState::Loose;
        let mut hidden = None;
        for k in 1..60 {
            w.tick += 1;
            run.advance(0, before, &w);
            if run.posts() == [Post::Hide] {
                hidden = Some(k);
                break;
            }
        }
        assert_eq!(hidden, Some(prompt_hold() as i64));
    }

    #[test]
    fn dunk_to_lay_up_latches_its_prompt_until_the_take_off() {
        let mut w = holding([28.0, 0.0]);
        let mut run = Run::default();
        let before = Before::new(&w);
        run.advance(4, before, &w);
        assert_eq!(run.posts(), &[Post::Prompt(4)]);
        // The check failing (here no dribble): hidden, but not shown again.
        w.players[0].dribble = 0;
        for _ in 0..prompt_hold() {
            w.tick += 1;
            run.advance(4, before, &w);
        }
        assert_eq!(run.posts(), &[Post::Hide]);
        w.tick += 1;
        run.advance(4, before, &w);
        assert_eq!(run.posts(), &[]);
        // The take-off (slot 4): the lay-up's Shoot.
        w.events = vec![Event::Dunk { player: 0, hoop: 0 }];
        w.tick += 1;
        run.advance(4, before, &w);
        assert_eq!(run.posts(), &[Post::Prompt(1)]);
    }

    #[test]
    fn a_tutorial_page_opens_two_updates_after_the_launch_once() {
        let mut w = holding([20.0, 0.0]);
        w.phase = Phase::DeadBall { left: 1.0, team: 0 };
        let before = Before::new(&w);
        let mut run = Run::default();
        run.advance(3, before, &w);
        assert_eq!(run.posts(), &[]);
        run.advance(3, before, &w);
        assert_eq!(run.posts(), &[Post::Page(page::TURBO)]);
        assert_eq!(run.seen, page::bit(page::TURBO));
        run.advance(3, before, &w);
        assert_eq!(run.posts(), &[]);
        // Seen: not again.
        let mut run = Run {
            seen: page::bit(page::TURBO),
            ..Run::default()
        };
        run.advance(3, before, &w);
        run.advance(3, before, &w);
        assert_eq!(run.posts(), &[]);
        // The shot lessons' page; none for Pass.
        let mut run = Run::default();
        run.advance(1, before, &w);
        run.advance(1, before, &w);
        assert_eq!(run.posts(), &[Post::Page(page::PLAYER_INDICATOR)]);
        let mut run = Run::default();
        run.advance(0, before, &w);
        run.advance(0, before, &w);
        assert_eq!(run.posts(), &[]);
    }

    #[test]
    fn the_shot_clock_page_opens_at_nine_and_refills_the_clock_after() {
        // The person on side 1 (the School's Lakers), started, the clock
        // at 9.5 in Short Range Shot.
        let mut w = world();
        w.people = (0..6).map(|k| k == 3).collect();
        w.possession = 1;
        w.game_phase.possession = 1;
        w.game_phase.phase = game_phase::phase::FRONT_COURT;
        w.rules.shot_clock = Some(24.0);
        w.shot_clock = 9.5;
        let before = Before::new(&w);
        let seen = page::bit(page::PLAYER_INDICATOR);
        let mut run = Run {
            seen,
            ..Run::default()
        };
        run.advance(1, before, &w);
        run.advance(1, before, &w);
        assert!(run.posts().contains(&Post::Page(page::SHOT_CLOCK)));
        // Left to run in this lesson.
        run.advance(1, before, &w);
        assert!(!run.posts().contains(&Post::RefillClock));
        // The next lesson of the person's side refills it every update; a
        // defensive one (the Nets' ball) does not.
        let mut run = Run {
            seen: seen | page::bit(page::SHOT_CLOCK),
            ..Run::default()
        };
        run.advance(2, before, &w);
        assert!(run.posts().contains(&Post::RefillClock));
        run.advance(8, before, &w);
        assert!(!run.posts().contains(&Post::RefillClock));
    }

    /// Updates from a lesson's launch to its restart's end (slot 28).
    fn start_after(page: bool) -> Option<usize> {
        let players = (0..6)
            .map(|k| Player::new(k / 3, [0.0, k as f32 * 3.0], 0.0))
            .collect();
        let mut w = World::new(Hoop::native(), players, Rules::default(), 1);
        w.people = (0..6).map(|k| k == 3).collect();
        w.launch_lesson(0, 1, page, &[]);
        let mut inputs: Vec<crate::Input> = (0..6).map(|_| crate::Input::computer()).collect();
        inputs[3] = crate::Input::default();
        (1..400).find(|_| {
            w.step(&inputs);
            w.events.iter().any(|e| matches!(e, Event::Inbound { .. }))
        })
    }

    #[test]
    fn replaying_lessons_repeatedly_leaves_the_restart_and_keeps_control() {
        let players = (0..6)
            .map(|k| Player::new(k / 3, [0.0, k as f32 * 3.0], 0.0))
            .collect();
        let mut w = World::new(Hoop::native(), players, Rules::default(), 1);
        w.poses = crate::pose::disc_bank();
        let mut inputs = vec![crate::Input::computer(); 6];
        inputs[3] = crate::Input::default();
        for lesson in 0..26 {
            for attempt in 0..3 {
                let side = expected_side(lesson, 1);
                w.launch_lesson(lesson as u8, side, false, &inputs);
                w.settle_presses(&inputs);
                // Short of the rewrite's limit (which would end it anyway):
                // a relaunch's inbound can go uncaught and roll away, and the
                // restarting side's scoop (callback 0, no phase test) takes
                // it in phase 6 (lesson 10's third relaunch: 850 updates).
                let limit = (crate::tuning::RESTART_LIMIT * crate::TICK_HZ as f32) as usize - 1;
                let end = (1..limit).find(|_| {
                    for (i, input) in inputs.iter_mut().enumerate() {
                        *input = if w.people[i] {
                            crate::Input::default()
                        } else {
                            crate::Input::computer()
                        };
                    }
                    w.step(&inputs);
                    !w.restarting()
                });
                assert!(
                    end.is_some(),
                    "lesson {lesson}, attempt {attempt}: {:?}, windup {:?}",
                    w.phase,
                    w.windup
                );
                assert_eq!(w.phase, crate::Phase::Live);
                // Relaunch both immediately after the inbound's release and
                // after it has had time to reach the receiver.
                for _ in 0..(attempt * 30) {
                    w.step(&inputs);
                }
            }
        }
    }

    #[test]
    fn a_lessons_restart_ends_as_the_schools_captures_start_them() {
        // The launch leaves the computer's slots as the AI's last frame
        // would (team+0x88), so the person's teammates walk and ready; the
        // inbound waits `fps × 2` (`fps × 0.5` with a page due): the
        // captures start their lessons 143..144 updates after the launch,
        // 55 after the Gamebreaker lesson's page.
        let plain = start_after(false).expect("the restart ends");
        assert!((140..=146).contains(&plain), "{plain}");
        let paged = start_after(true).expect("the restart ends");
        assert!((50..=60).contains(&paged), "{paged}");
    }
}
