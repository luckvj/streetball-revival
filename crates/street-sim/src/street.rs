//! What makes it the original game: trick moves and fakeouts, trick points and
//! combos, and gamebreakers. The rules are the game's own, as its tips put
//! them (`XLOC_FE`/`XLOC_GM`, see `docs/FORMATS.md`):
//!
//! - "Trick points are earned by performing trick moves, dunks, alley-oops
//!   and other moves. The better the move, the higher the trick point bonus."
//! - "Each time you earn trick points you are powering up your Gamebreaker
//!   Meter. Once your meter is full you can shoot a Gamebreaker, Pocket your
//!   Level 1 Gamebreaker, or Cancel your opponent's pocketed Gamebreaker."
//! - "Although trick moves extend a combo, a successful fakeout will add a
//!   multiplier to the combo." The biggest combo is five fakeouts followed by
//!   a Level 2 gamebreaker.
//! - "Level 1 Gamebreakers not only score points as normal but also take 1
//!   point from your opponent's score." Level 2s "are worth more than twice as
//!   many points as Level 1 Gamebreakers and as much as five times more trick
//!   points", "take away even more points" and cannot be blocked.
//! - Gamebreaker: hold any two turbos and shoot. Pocket: the pocket button
//!   with the ball. Cancel: one turbo and the pocket button.
//!
//! The point values and the meter's size come from the executable's own
//! tables (see docs/TRACE.md, "Gameplay values"): the trick ladder is the
//! pick/fakeout scorer's own five-entry table (500 to 5,000 by the move's
//! value class), a move repeated within the team's last four halves its
//! points, and the chain pays through the game's own pattern matcher
//! (`combo::Chain::evaluate`, `FUN_00131970`): the pattern table gates it,
//! and the weighted rows from `0x43be58` pay the slots the matched row
//! walked, inside its two-second window (`combo::WINDOW`). The messages
//! that feed the ring, the meters and the statistics are `crate::scoring`'s.

use crate::{
    Action, BallState, Event, Phase, Rules, World, distance, gamebreaker,
    player::{Player, TrickIdentity},
    sub, target_hoop,
};

/// Trick points per move.
pub mod points {
    /// A trick move by native value class (0..4); generic simulation levels
    /// 1..5 index it with level-1. FUN_001438b8 reads 0x43df20 directly by
    /// actor+0x248: 500, 1,000, 2,000, 3,000, 5,000.
    pub const TRICK: [u32; 5] = [500, 1_000, 2_000, 3_000, 5_000];
    /// Close shots (short range), and shots beyond the arc (long range).
    pub const JUMPER: u32 = 250;
    pub const LONG_JUMPER: u32 = 750;
    pub const DUNK: u32 = 1_500;
    pub const ADVANCED_DUNK: u32 = 5_000;
    /// Base for an alley-oop; the way it goes up adds up to +1,000 and
    /// +2,000 more (base 3,000, up to 6,000).
    pub const ALLEY_OOP: u32 = 3_000;
    /// A lay-up, by how it went in: swish to and-one style. The plainest
    /// one is 1,000; better finishes up to 7,500.
    pub const LAYUP: u32 = 1_000;
    pub const GOALTEND: u32 = 2_000;
    pub const BLOCK: u32 = 2_500;
    pub const STEAL: u32 = 1_000;
    pub const COUNTER: u32 = 3_000;
    /// A rebound or a loose-ball pickup (popup 500).
    pub const REBOUND: u32 = 500;
    /// Level 1 and 2 gamebreakers (2 is twice the level 1, and up to five
    /// times the level 1's trick points in the special cases).
    pub const GAMEBREAKER: [u32; 2] = [25_000, 50_000];
    /// Cancelling the other team's pocketed gamebreaker.
    pub const CANCEL: u32 = 5_000;
}

/// Trick points that fill the gamebreaker meter (exactly 50,000; the meter
/// is a float the game clamps to 0..=50,000). Blocks and steals meanwhile
/// drain the victim's meter by their own value.
pub const METER_FULL: u32 = 50_000;
/// The combo multiplier: 1, plus one per fakeout, up to five fakeouts.
pub const MAX_MULTIPLIER: u32 = 6;

/// `0x4b7b14`: how long a full meter lasts, `fps × 30` updates (the reset
/// at `0x144c30..0x144c40`, fps the timing object's word 60). It runs
/// down in live play (`FUN_00143c20`); at 0 the meter empties.
pub const METER_HOLD: i32 = 30 * crate::TICK_HZ as i32;

/// `0x4b7990`: what an unfilled meter loses each update of live play
/// before its side's fill scales it, `50000 / (fps × 600.0)` (the reset at
/// `0x142e04..0x142f04`, fps the timing object's float 60.0): a whole
/// meter in ten minutes. The f32 quotient is already the rounded-down one
/// the EE's division gives.
pub const METER_DECAY: f32 = 50_000.0 / (60.0 * 600.0);

/// Game+0x4a4: whether the meters run (`FUN_0013eff8` reads it). The
/// original clears it at a restart (`FUN_001406b8`) and sets it in game
/// phase 4, the ball in a team's back court, once someone holds the ball
/// or it crosses into the front court (`FUN_0013e4c8` case 4, which the
/// game's phase machine runs: [`crate::game_phase`]). Phase 4 follows a
/// restart, the tip, and a pickup or steal made in the taker's back court
/// (`FUN_0013ee10`; one in the front court is phase 5, which leaves the
/// flag alone).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeterClock {
    /// Not set: from the start of a match until a team has the ball in
    /// its back court.
    #[default]
    Off,
    /// Cleared by a restart: set once the inbound is caught in the back
    /// court or the ball crosses into the inbounding team's front court.
    Inbound,
    On,
}

/// `DAT_004b7d34` and `DAT_004b7d38` (written at `0x145574..0x1455c8`):
/// the gamebreaker task's wait before its first replay, 60 updates, or 40
/// when the ball's holder is dunking (state bit 7, `FUN_0016e9d0`).
pub const GAMEBREAKER_WAIT: [i32; 2] = [60, 40];

/// The gamebreaker task (game+0x434, `FUN_00144df0`; its process
/// `FUN_00144ef8`, raw `0x144ef8..0x1451c4`), created by the game
/// listener's slot +0x204 (`FUN_001422f8`, one at a time). Its process
/// runs before the game's update; the game's update deletes it once its
/// busy word (+0x2c) is clear (`0x13e0d8`).
///
/// - The shot's (kind 1): its first update sets the wait
///   ([`GAMEBREAKER_WAIT`]); it counts down, and at 0 asks the replay
///   manager for the last 90 frames at 0.8125 (state 2); the update the
///   manager is not playing (+0x28 not 2 or 3) it asks again at 0.53125
///   (state 3); the next such update it posts its end (`0x3c6880`: slot 65
///   on the meters, [`World::end_gamebreaker_meter`]; the camera's
///   +0x20c). The manager's replays hold the game's updates while they
///   play, so the end comes 62 of the simulation's updates after the
///   press's (`street-game`'s tape shows them, in a lesson too: the
///   School's Gamebreaker capture, the meter emptied 63 game updates after
///   the press, before the shot reached the rim, the two replays 1.85 s and
///   2.81 s of the wall clock between).
/// - A staged Level 2's (kind 0): it starts the script (after the HUD's
///   banner allows, `FUN_002bb818`), then posts its end the first update
///   the script manager's state (+0x50) is out of 3..7 (the School's Level
///   2 capture: the update after the sequence).
///
/// Before, the simulation ended the gamebreaker at the next take or
/// restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GamebreakerTask {
    /// +0x38: 0 a staged script's, 1 the shot's.
    pub kind: u8,
    /// +0x30: 0 new; the shot's 1 counting, 2 and 3 its replays; the
    /// staged one's 1 playing.
    pub state: u8,
    /// +0x34: the wait left.
    pub count: i32,
}

impl GamebreakerTask {
    /// One update of the process: `dunking` whether the ball's holder is
    /// dunking, `staged` whether the script manager's state is in 3..7 (a
    /// staged script plays). True when it posts its end.
    pub fn step(&mut self, dunking: bool, staged: bool) -> bool {
        match (self.kind, self.state) {
            (1, 0) => {
                self.state = 1;
                self.count = GAMEBREAKER_WAIT[usize::from(dunking)];
                false
            }
            (1, 1) => {
                if self.count > 0 {
                    self.count -= 1;
                }
                if self.count == 0 {
                    self.state = 2;
                }
                false
            }
            (1, 2) => {
                self.state = 3;
                false
            }
            (0, 0) => {
                self.state = 1;
                false
            }
            (0, _) => !staged,
            _ => true,
        }
    }
}

/// The tricks strung together in one possession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Combo {
    /// Trick points earned in it so far.
    pub points: u32,
    /// 1 plus one per fakeout.
    pub multiplier: u32,
    pub moves: u32,
}

impl Default for Combo {
    fn default() -> Self {
        Self {
            points: 0,
            multiplier: 1,
            moves: 0,
        }
    }
}

/// One team's trick points and gamebreaker state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeamState {
    /// Trick points this game.
    pub trick_points: u32,
    pub combo: Combo,
    /// Trick points toward the next gamebreaker, up to [`METER_FULL`].
    pub meter: u32,
    pub meter_fraction: f32,
    /// The meter object's fill tuning (team meter+0x40, the difficulty
    /// blend), its running scale (+0x44, 1.0 until a gamebreaker basket
    /// multiplies it by +0x48, [`TeamState::meter_sequence_fill`]) and the
    /// sequence factor itself.
    pub meter_fill: f32,
    pub meter_scale: f32,
    pub meter_sequence_fill: f32,
    /// Team meter+0x18: frames a full meter has left (`fps × 30`,
    /// `0x4b7b14`, from the moment it fills); 1 through a gamebreaker of
    /// this side's; 0 while the meter is not full. No fill or drain lands
    /// while it is set (`FUN_00143d38`).
    pub meter_hold: i32,
    /// Team meter+0x1c: this side's gamebreaker is under way (`0x144708`,
    /// slot 64, until slot 65 `0x144760`): its meter neither counts down
    /// nor decays.
    pub meter_gamebreaker: bool,
    /// Easy CPU crews cannot fill beyond 85% in a close game against a person.
    pub meter_cpu_limit: bool,
    /// A person plays this side (`game+0x9a/0x9b`): Easy and Hard change
    /// shot odds only when a side is human.
    pub human: bool,
    /// A Level 1 gamebreaker put away to build a Level 2.
    pub pocketed: bool,
    /// Team meter+0x2c..+0x38: the last four repeat keys, newest first, −1
    /// for none — a trick move's script (actor+0x264) for value classes
    /// 2..4 (`FUN_001438b8`), −3 (−2) for a passout special
    /// (`FUN_00143a88`). Each copy of a key halves the next one's value;
    /// a catch after the possession changed clears it (`FUN_001447d0`).
    pub recent_moves: [i32; 4],
    /// Team meter+0x50: a steal, block, rebound or passout paid this take,
    /// so the catch's dive pays nothing (`FUN_001447d0`).
    pub paid: bool,
    /// Team meter+0x4c: a pump fake (slot 5) arms the pump fake's 500
    /// (slot 73, `FUN_001440d0`).
    pub pump_fake: bool,
    /// Team meter+0x8: the possession the meter last saw at a catch.
    pub possession_seen: i32,
}

impl Default for TeamState {
    fn default() -> Self {
        Self {
            trick_points: 0,
            combo: Combo::default(),
            meter: 0,
            meter_fraction: 0.0,
            meter_fill: 1.0,
            meter_scale: 1.0,
            meter_sequence_fill: 1.0,
            meter_hold: 0,
            meter_gamebreaker: false,
            meter_cpu_limit: false,
            human: false,
            pocketed: false,
            recent_moves: [-1; 4],
            paid: false,
            pump_fake: false,
            possession_seen: -1,
        }
    }
}

impl TeamState {
    pub fn meter_full(&self) -> bool {
        self.meter >= METER_FULL
    }

    /// The gamebreaker a full meter gives: 2 with one in the pocket, else 1;
    /// 0 when the meter is not full.
    pub fn gamebreaker(&self) -> u8 {
        match (self.meter_full(), self.pocketed) {
            (false, _) => 0,
            (true, false) => 1,
            (true, true) => 2,
        }
    }
}

/// A basket, by how it was scored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basket {
    Jumper {
        long: bool,
    },
    Dunk {
        advanced: bool,
    },
    Layup,
    /// An alley-oop's finish; `advanced` when the finisher's move has
    /// flag 0x800 (`FUN_00171de8`), which makes the ball's kind a dunk's
    /// (4) where it is otherwise 3 (the launcher, raw `0x14ad14..0x14ad3c`).
    AlleyOop {
        advanced: bool,
    },
    /// A gamebreaker's basket: its level (the ball's +0x178), from beyond
    /// the arc, or a dunk (a staged Level 2 dunk's, the ball's kind 4).
    Gamebreaker {
        level: u8,
        long: bool,
        dunk: bool,
    },
}

impl Basket {
    /// The points table's kind (`FUN_00187c10`): 0 a dunk or an
    /// alley-oop (the ball's kind 3 or 4), 2 from beyond the arc (the
    /// event's base 3), else 1.
    pub fn table_kind(self) -> usize {
        match self {
            Basket::Dunk { .. } | Basket::AlleyOop { .. } => 0,
            Basket::Gamebreaker { dunk: true, .. } => 0,
            Basket::Jumper { long: true } | Basket::Gamebreaker { long: true, .. } => 2,
            _ => 1,
        }
    }

    /// The ball's gamebreaker at the basket (ball+0x178), 0 for none.
    pub fn level(self) -> u8 {
        match self {
            Basket::Gamebreaker { level, .. } => level,
            _ => 0,
        }
    }

    /// Points for the scoring team under `rules`: the game type's row of
    /// the points table (`0x43b618`, [`crate::game_class::points`]) by the
    /// basket's kind and gamebreaker. Normal: 1 inside, 2 from beyond the
    /// arc; league scoring 2 and 3; Old School 1; Dunks Only a dunk's 1; a
    /// Level 2's 2 (3 in league scoring); the trick-point and Gamebreaker Wins
    /// games none.
    pub fn points(self, rules: &Rules) -> u32 {
        let [ours, _] =
            crate::game_class::points(rules.game_type(), self.table_kind(), self.level());
        ours.max(0) as u32
    }

    /// Points taken from the other team: a Level 1's 1 (2 in league
    /// scoring), a Level 2's 3, or 4 from beyond the arc (4 and 7 in league
    /// scoring); the other side's score floors at 0 (`FUN_00187c10`).
    pub fn taken(self, rules: &Rules) -> u32 {
        let [_, theirs] =
            crate::game_class::points(rules.game_type(), self.table_kind(), self.level());
        (-theirs).max(0) as u32
    }

    pub fn trick_points(self) -> u32 {
        match self {
            Basket::Jumper { long: false } => points::JUMPER,
            Basket::Jumper { long: true } => points::LONG_JUMPER,
            Basket::Dunk { advanced: false } => points::DUNK,
            Basket::Dunk { advanced: true } => points::ADVANCED_DUNK,
            Basket::AlleyOop { .. } => points::ALLEY_OOP,
            Basket::Layup => points::LAYUP,
            Basket::Gamebreaker { level, .. } => {
                points::GAMEBREAKER[usize::from(level.clamp(1, 2)) - 1]
            }
        }
    }

    /// The ring class the game pushes for this basket, with its own extra
    /// index: long and short shots are classes 3 and 4 (the 750/250 base),
    /// dunks and lay-ups class 0 (the finish family), alley-oops class 1,
    /// and a gamebreaker its own 6 or 7 with the long-range bit in the extra.
    pub fn combo_kind(self) -> (i32, i32) {
        match self {
            Basket::Jumper { long } => (if long { 3 } else { 4 }, 0),
            Basket::Dunk { .. } | Basket::Layup => (0, 0),
            Basket::AlleyOop { .. } => (1, 0),
            Basket::Gamebreaker { level, long, .. } => {
                (i32::from(level.clamp(1, 2)) + 5, i32::from(long))
            }
        }
    }
}

impl World {
    pub fn configure_meter_fill(&mut self, controlled: [bool; 2], scenario: Option<f32>) {
        self.scenario = scenario;
        let normal = crate::meter::fill(self.rules.difficulty, controlled, scenario);
        let sequence = crate::meter::sequence_fill(self.rules.difficulty, controlled, scenario);
        for (side, team) in self.teams.iter_mut().enumerate() {
            team.meter_fill = normal[side];
            team.meter_sequence_fill = sequence[side];
            team.human = controlled[side];
            team.meter_cpu_limit = self.rules.difficulty == crate::Difficulty::Easy
                && !controlled[side]
                && controlled[1 - side];
        }
    }
    /// Trick points into `team`'s meter and its total, as a listener with
    /// its own value would add them (the tests' shorthand).
    #[cfg(test)]
    pub(crate) fn award(&mut self, team: usize, points: u32, _in_combo: bool) {
        if !self.rules.tricks {
            return;
        }
        match self.players.iter().position(|p| p.team == team) {
            Some(i) => self.trick_points(i, points as i32),
            None => self.teams[team].trick_points += points,
        }
        self.fill_meter(team, points as f32);
    }

    /// Take trick points out of `team`'s meter, as a block or steal against
    /// it does (the tests' shorthand).
    #[cfg(test)]
    pub(crate) fn drain(&mut self, team: usize, points: u32) {
        self.fill_meter(team, -(points as f32));
    }

    /// `FUN_00143d38`: `amount` trick points into `team`'s meter (a drain
    /// is a negative amount), scaled by the side's fill, team meter+0x40 ×
    /// +0x44 (the +0x10 meter-size path is never set in play). Nothing
    /// lands in game types 3 and 7 (No Gamebreakers and Back To Basics,
    /// [`Rules::game_type`]) or while a full meter's hold (+0x18) runs; the
    /// court intro's gate (`FUN_0013e358`, game+0x428 +0x24) is shut only
    /// while the intro plays, when nothing scores. Past the Easy cap an
    /// Easy computer crew against a person less than three ahead keeps the
    /// previous value when that was past the cap, else stops at the cap;
    /// the Trick Point and Gamebreaker Wins games (game+0x49c) and the
    /// lessons skip the cap. At 50,000 the meter is set to exactly that and
    /// its hold starts (`fps × 30`, [`METER_HOLD`]) — or, in a lesson
    /// (game+0x9c 4) other than the gamebreaker lessons (`vt+0xb4` 0xe,
    /// 0x17..0x19, [`World::lesson`]), it stops at 49,999 (the singleton's
    /// +0x64, the other way there, is never written); below 0 it is 0.
    pub(crate) fn fill_meter(&mut self, team: usize, amount: f32) {
        if matches!(self.rules.game_type(), 3 | 7) {
            return;
        }
        let chasing = matches!(
            self.rules.goal,
            crate::Goal::TrickPoints(_) | crate::Goal::Gamebreaker(_)
        ) || self.lesson.is_some();
        let behind = self.score[1 - team] as i64 - self.score[team] as i64;
        let short = self.lesson.is_some_and(|l| !matches!(l, 0xe | 0x17..=0x19));
        let t = &mut self.teams[team];
        if t.meter_hold != 0 {
            return;
        }
        let previous = t.meter as f32 + t.meter_fraction;
        let mut fill = previous + amount * t.meter_fill * t.meter_scale;
        let cap = crate::meter::EASY_CPU_CAP;
        if t.meter_cpu_limit && !chasing && cap < fill && behind < 3 {
            fill = if cap < previous { previous } else { cap };
        }
        let full = METER_FULL as f32;
        if full <= fill {
            if short {
                fill = 49_999.0;
            } else {
                fill = full;
                t.meter_hold = METER_HOLD;
            }
        } else if fill < 0.0 {
            fill = 0.0;
        }
        t.meter = fill as u32;
        t.meter_fraction = fill - t.meter as f32;
        if t.meter_hold != 0 {
            self.events.push(Event::GamebreakerReady { team });
            let score = self.fill_score(team);
            self.stats
                .meter_filled(team, self.teams[team].pocketed, score);
        }
    }

    /// The score slot 66 notes at a fill (`FUN_001883f0`): the trick
    /// points in game type 1, else the game's vt+0x9c.
    fn fill_score(&self, team: usize) -> i32 {
        if self.rules.game_type() == 1 {
            self.stats.team(team, crate::stats::TRICK_POINTS)
        } else {
            self.class_score(team)
        }
    }

    /// `FUN_001448e0`: set `team`'s meter outright (the match's start fill,
    /// `FUN_001457f8`, and the lessons). Below 0 it is 0; at 50,000 it is
    /// full, and its hold starts unless it was already full; any change
    /// that leaves it short of full clears the hold and the gamebreaker
    /// flag. Filling posts "meter full" (slot 66) and emptying a full one
    /// "meter drops" (slot 67), as the original's do.
    pub fn set_meter(&mut self, team: usize, value: f32) {
        let t = &mut self.teams[team];
        let previous = t.meter as f32 + t.meter_fraction;
        let full = METER_FULL as f32;
        let mut filled = false;
        let value = if value < 0.0 {
            0.0
        } else if full <= value {
            if previous != full {
                t.meter_hold = METER_HOLD;
                filled = true;
            }
            full
        } else {
            value
        };
        t.meter = value as u32;
        t.meter_fraction = value - t.meter as f32;
        if previous != value && value != full {
            t.meter_hold = 0;
            t.meter_gamebreaker = false;
        }
        let pocketed = t.pocketed;
        if filled {
            self.events.push(Event::GamebreakerReady { team });
            let score = self.fill_score(team);
            self.stats.meter_filled(team, pocketed, score);
        }
        if value == 0.0 && previous == full {
            self.events.push(Event::MeterDropped { team });
            self.stats.meter_dropped(team, pocketed);
        }
    }

    /// `FUN_0013eff8`, once each update of live play (the game's vt+0x16c,
    /// after its phase update) while game+0x4a4 is set
    /// ([`MeterClock::On`]): both meters run `FUN_00143c20` with
    /// [`METER_DECAY`]. A side whose gamebreaker is under way is skipped.
    /// A full meter counts its hold down and at 0 empties, taking the
    /// pocketed gamebreaker with it (+0x20), and posts "meter drops"
    /// (slot 67); a meter short of full loses the decay times its fill.
    /// The script manager's states 3..7 (a staged sequence) skip it, as
    /// [`World::step`] does.
    ///
    /// The rewrite's own: a meter written full directly (the debugging
    /// switches, the tests) starts its hold here, as `FUN_001448e0` would
    /// have.
    pub(crate) fn step_meters(&mut self) {
        let live = self.phase == Phase::Live && self.opening_toss == crate::OpeningToss::None;
        // `FUN_00143c20` skips game type 7 (Back To Basics).
        if !live || self.meter_clock != MeterClock::On || self.rules.game_type() == 7 {
            return;
        }
        for team in 0..2 {
            let t = &mut self.teams[team];
            if t.meter_gamebreaker {
                continue;
            }
            if t.meter_hold == 0 && t.meter_full() {
                t.meter_hold = METER_HOLD;
            }
            if t.meter_hold < 1 {
                let left =
                    t.meter as f32 + t.meter_fraction - METER_DECAY * t.meter_fill * t.meter_scale;
                let left = if left < 0.0 { 0.0 } else { left };
                t.meter = left as u32;
                t.meter_fraction = left - t.meter as f32;
            } else {
                t.meter_hold -= 1;
                if t.meter_hold == 0 {
                    let pocketed = t.pocketed;
                    t.meter = 0;
                    t.meter_fraction = 0.0;
                    t.pocketed = false;
                    self.events.push(Event::MeterDropped { team });
                    self.stats.meter_dropped(team, pocketed);
                }
            }
        }
    }

    /// Slot 64 (`0x144708`, a gamebreaker goes up): each meter's +0x1c
    /// says whether it is this side's; the shooting side loses its pocket,
    /// its hold is set to 1 (no fill lands), and a Level 2 empties the
    /// meter at once. A Level 1 meter stays full until the gamebreaker is
    /// over ([`World::end_gamebreaker_meter`]).
    pub(crate) fn start_gamebreaker_meter(&mut self, team: usize, level: u8) {
        for (side, t) in self.teams.iter_mut().enumerate() {
            t.meter_gamebreaker = side == team;
        }
        let t = &mut self.teams[team];
        t.pocketed = false;
        t.meter_hold = 1;
        if level == 2 {
            t.meter = 0;
            t.meter_fraction = 0.0;
        }
    }

    /// One update of the gamebreaker task's process ([`GamebreakerTask`]).
    pub(crate) fn step_gamebreaker_task(&mut self) {
        let Some(mut task) = self.gamebreaker_task else {
            return;
        };
        let dunking = self
            .handler()
            .is_some_and(|h| matches!(self.players[h].action, Action::Dunking { .. }));
        let staged = self.sequence.is_some() || self.staged_start.is_some();
        if task.step(dunking, staged) {
            self.end_gamebreaker_meter();
            self.gamebreaker_task = None;
        } else {
            self.gamebreaker_task = Some(task);
        }
    }

    /// Slot 65 (`0x144760`, posted when the gamebreaker task ends,
    /// [`World::step_gamebreaker_task`]): a side whose gamebreaker it was
    /// is emptied and its hold and flag cleared.
    pub(crate) fn end_gamebreaker_meter(&mut self) {
        for t in &mut self.teams {
            if t.meter_gamebreaker {
                t.meter_gamebreaker = false;
                t.meter_hold = 0;
                t.meter = 0;
                t.meter_fraction = 0.0;
            }
        }
    }

    /// Native FUN_0015eff0: arbitration among slots with matching value class / level.
    /// Traverses slots 0..11, collects matching indices, draws `uVar1 = rng.next_u32()`,
    /// and returns `matching[uVar1 % matching.len()]`.
    pub fn select_slot_by_level(
        rng: &mut crate::rng::Rng,
        level: u8,
        slots: &[crate::player::TrickIdentity; 11],
    ) -> Option<crate::player::TrickIdentity> {
        let mut matching = [0usize; 11];
        let mut count = 0;
        for (i, identity) in slots.iter().enumerate() {
            if identity.level == level {
                matching[count] = i;
                count += 1;
            }
        }
        if count == 0 {
            None
        } else {
            let draw = rng.next_u32();
            let idx = matching[(draw as usize) % count];
            Some(slots[idx])
        }
    }

    /// A trick move's points (the class-10 or -11 message `0x3c6550` that
    /// `0x157dd0` posts the update bit 10 drops; [`World::post_trick`]):
    /// the ladder by its value class, halved unless it faked `faked` out
    /// (actor+0x218), halved again per repeat in the team's last four for
    /// classes 2..4; a fakeout raises the HUD's multiplier (the rewrite's
    /// own bookkeeping, [`Combo`]).
    ///
    /// A trick alone never loses the ball: the rewrite used to fumble an
    /// advanced trick of a poor handler (its own rule, about one trick in
    /// twenty), where the two attract captures' 138 trick moves lost the
    /// ball only to a defender (`ai::capture`). [`Event::Fumble`] is no
    /// longer sent.
    pub(crate) fn score_trick(&mut self, i: usize, id: TrickIdentity, faked: Option<u8>) {
        let team = self.players[i].team;
        let value_class = usize::from(id.level).min(4);
        let faked = faked.map(usize::from);
        let trick = self.post_trick(i, value_class, i32::from(id.script), faked.is_some());
        let combo = &mut self.teams[team].combo;
        if faked.is_some() {
            combo.multiplier = (combo.multiplier + 1).min(MAX_MULTIPLIER);
        }
        combo.points += trick;
        combo.moves += 1;
        self.events.push(Event::Trick {
            player: i,
            level: id.level + 1,
            script: id.script,
            fakeout: faked,
            points: trick,
        });
    }

    /// The nearest opponent of `i` within `range` and in front of them.
    pub(crate) fn defender_in_front(&self, i: usize, range: f32) -> Option<usize> {
        let p = &self.players[i];
        let f = p.forward();
        self.players
            .iter()
            .enumerate()
            .filter(|(_, q)| q.team != p.team)
            .filter_map(|(j, q)| {
                let d = sub(q.position, p.position);
                let n = distance(q.position, p.position);
                let ahead = (d[0] * f[0] + d[1] * f[1]) / n.max(1e-3);
                (n < range && ahead > 0.0).then_some((j, n))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(j, _)| j)
    }

    /// The pocket button: put a full meter's Level 1 gamebreaker away, or
    /// with one turbo held, spend it to cancel the other team's pocketed one.
    pub(crate) fn pocket(&mut self, i: usize, turbos: u32) {
        let team = self.players[i].team;
        if !self.rules.gamebreakers || !self.teams[team].meter_full() {
            return;
        }
        let other = 1 - team;
        // In a lesson (game+0x9c 4) `FUN_00141190` allows the cancel only in
        // Cancel Gamebreaker (vt+0xb4 0x18) and `FUN_00141098` the pocket
        // only in Pocket Gamebreaker (0x17; raw `0x141248`, `0x141148`).
        let (cancels, pockets) = match self.lesson {
            Some(lesson) => (lesson == 0x18, lesson == 0x17),
            None => (true, true),
        };
        // `FUN_00141190`: a cancel needs the other side's pocket and none of
        // this side's own.
        if turbos == 1 && self.teams[other].pocketed && !self.teams[team].pocketed {
            if !cancels {
                return;
            }
            // Slot 71 (`0x1447a8`): both pockets go, and the cancelling
            // side's meter and hold. The 5,000 (class 18 in the ring, and
            // the stats' trick points, `FUN_00187ba0`) never reaches the
            // meter.
            self.teams[other].pocketed = false;
            let t = &mut self.teams[team];
            t.meter = 0;
            t.meter_fraction = 0.0;
            t.meter_hold = 0;
            self.post_cancel(team, i);
            self.events.push(Event::Cancel { team });
        } else if turbos == 0 && !self.teams[team].pocketed && pockets {
            // Slot 70 (`0x144780`): pocketed, the meter and its hold at 0.
            let t = &mut self.teams[team];
            t.pocketed = true;
            t.meter = 0;
            t.meter_fraction = 0.0;
            t.meter_hold = 0;
            self.events.push(Event::Pocket { team });
        }
    }

    /// Start a gamebreaker shot if the team has one: it uses up the meter
    /// (and the pocket, for a Level 2). `FUN_00159cf0` (raw
    /// `0x159cf0..0x159ea4`) asks nothing of the turbo bar: not while a
    /// gamebreaker task runs (`FUN_00140ff8`), not with the meter short of
    /// full, and not for a player the computer drives on a side a person
    /// plays (`FUN_00156d98` 4 with game+0x9a for his side set).
    pub(crate) fn start_gamebreaker(&mut self, i: usize, hoop: usize) -> bool {
        let team = self.players[i].team;
        let level = self.teams[team].gamebreaker();
        let person = |j: usize| self.people.get(j).copied().unwrap_or(false);
        let driven = !person(i)
            && (0..self.players.len()).any(|j| self.players[j].team == team && person(j));
        if !self.rules.gamebreakers || level == 0 || self.gamebreaker_task.is_some() || driven {
            return false;
        }
        self.start_gamebreaker_meter(team, level);
        self.events.push(Event::Gamebreaker { player: i, level });
        let staged = level == 2 && self.start_staged(i);
        // Slot +0x204 (`FUN_001422f8`): one task at a time, its kind the
        // message's (0 the staged script's, 1 the shot's).
        if self.gamebreaker_task.is_none() {
            self.gamebreaker_task = Some(GamebreakerTask {
                kind: u8::from(!staged),
                state: 0,
                count: 0,
            });
        }
        if staged {
            return true;
        }
        let rim = self.rim_spot(hoop);
        let long = self.is_long_range(self.players[i].position, hoop);
        let buttons = self.players[i].action_buttons;
        let script = crate::timing::shot_script(self, i, buttons, long, level, false);
        self.begin_script(i, crate::timing::SHOT_SCRIPTS[script].script);
        self.players[i].start_shot(hoop, rim, level, false, script);
        true
    }

    /// A Level 2 gamebreaker from a spot the disc stages: its script plays
    /// out (`FUN_00141298` picks the family by the spot, `FUN_001851f0` the
    /// variant). False where the spot has none, or the script needs a
    /// player this game doesn't have.
    fn start_staged(&mut self, i: usize) -> bool {
        let Some(config) = self.staged.clone() else {
            return false;
        };
        let random = self.rng.next_u32();
        let mut variety = self.variety;
        let Some(sequence) = config.start(&self.players, i, 2, &mut variety, random) else {
            return false;
        };
        let Some(plan) = config.plan(sequence.script) else {
            return false;
        };
        if plan.ball_roles().any(|role| {
            sequence
                .actors
                .get(usize::from(role))
                .copied()
                .flatten()
                .is_none()
        }) {
            return false;
        }
        self.variety = variety;
        self.staged_start = Some((sequence, 2));
        true
    }

    /// The staged script's first pose ([`World::staged_start`]): its actors
    /// leave what they were doing and stand where its frame 0 puts them.
    pub(crate) fn begin_staged(&mut self, sequence: gamebreaker::Sequence) {
        for &p in sequence.actors.iter().flatten() {
            let player = &mut self.players[p];
            player.action = Action::None;
            player.speed = 0.0;
        }
        self.give_and_go = None;
        self.pick = None;
        self.sequence = Some(sequence);
        self.step_staged_pose();
    }

    /// One frame of a staged gamebreaker: pose its actors, carry the ball
    /// and count the basket when it arrives; at the end, play resumes (the
    /// other team inbounds).
    pub(crate) fn step_staged(&mut self) {
        let (Some(config), Some(mut sequence)) = (self.staged.clone(), self.sequence) else {
            self.sequence = None;
            return;
        };
        let Some(plan) = config.plan(sequence.script) else {
            self.sequence = None;
            return;
        };
        let native = plan.actors.iter().any(|actor| actor.playback.is_some());
        if native
            && !sequence.resume_pending
            && let Some(hold) = plan.controls.hold
            && hold.frame == sequence.frame
        {
            if sequence.held < hold.duration {
                sequence.held += 1;
                self.sequence = Some(sequence);
                return;
            }
            sequence.resume_native_actors(plan);
            sequence.resume_pending = true;
            self.sequence = Some(sequence);
            self.step_staged_pose();
            return;
        }
        if native {
            sequence.update_native_clocks(plan, &config.player_scales);
            sequence.resume_pending = false;
            self.sequence = Some(sequence);
        }
        let previous_ball = self.ball.position;
        let held_before = self.held_offset();
        self.step_staged_pose();
        self.step_staged_finishers(plan, &mut sequence, held_before);
        if sequence.held == 0 || native {
            self.events.push(Event::GamebreakerFrame {
                script: sequence.script,
                frame: sequence.frame,
                mirrored: sequence.mirrored(),
            });
            self.step_staged_ball(&config, plan, &sequence, previous_ball);
            // The ball's own motion this update (ball+0x40's displacement,
            // which the next update's dunk path paces itself by), as the
            // game's update keeps it.
            self.ball.moved = std::array::from_fn(|k| self.ball.position[k] - previous_ball[k]);
        }
        // The freeze: the world stops at its frame while its camera moves.
        if !native
            && let Some(hold) = plan.controls.hold
            && hold.frame == sequence.frame
            && sequence.held < hold.duration
        {
            sequence.held += 1;
            self.sequence = Some(sequence);
            return;
        }
        sequence.held = 0;
        let last = sequence.frame;
        if sequence.advance(plan.length() as usize) {
            self.sequence = Some(sequence);
        } else {
            // The replay command whose span ends on the script's last frame
            // dispatches before its stop (`FUN_003355c8`, the records in
            // the disc's order): the span plays again as the game's replay
            // ([`crate::replay`]), the script manager waiting in its state 5
            // until the replay's end hands it back (`FUN_003362a0`) and its
            // stop ends it. No game update runs meanwhile, so the script's
            // end is this update's for the game (the School's Level 2
            // capture: the script's state 1 and the gamebreaker task's end
            // a game update apart, the replay between them).
            if let Some(r) = plan.controls.rerun.filter(|r| r.to == last) {
                self.events.push(Event::StagedRerun {
                    script: sequence.script,
                    mirrored: sequence.mirrored(),
                    from: r.from,
                    frames: r.to - r.from,
                    speed: r.speed,
                    camera: r.camera,
                });
            }
            self.sequence = None;
            for p in &mut self.players {
                p.height = 0.0;
                p.speed = 0.0;
            }
            if matches!(self.ball.state, BallState::Held(_)) && self.phase == Phase::Live {
                // A script that never scored: the holder plays on.
            } else if self.phase == Phase::Live
                && !matches!(self.ball.state, BallState::Shot | BallState::Pass)
            {
                // A shot or pass in flight keeps flying and lands as the
                // game's does; only a watched-over ball drops loose.
                self.ball.drop_loose();
            }
        }
    }

    /// Put the actors and the ball where the sequence's current frame has
    /// them.
    fn step_staged_pose(&mut self) {
        let (Some(config), Some(sequence)) = (self.staged.clone(), self.sequence) else {
            return;
        };
        let Some(plan) = config.plan(sequence.script) else {
            return;
        };
        let mut spatial = sequence;
        spatial.frame = sequence.pose_frame();
        let _ = plan.apply(&spatial, &mut self.players).count();
        if let BallState::Held(player) = self.ball.state
            && let Some(role) = sequence
                .actors
                .iter()
                .position(|&actor| actor == Some(player))
            && sequence.finishing & (1 << role) == 0
            && let Some(hand) =
                config.hand_position(plan, &sequence, role as u8, sequence.pose_frame() as f32)
        {
            self.ball.position = hand;
            self.ball.velocity = [0.0; 3];
        }
    }

    /// A bounce pass's launch (`FUN_0014c0c0` with its flag,
    /// `FUN_0014f728`): the floor met once on the way, `n` updates to
    /// `to`; units a second.
    fn bounce_velocity(&self, from: [f32; 3], to: [f32; 3], n: u32) -> Option<[f32; 3]> {
        let dt = crate::TICK_SECONDS;
        let total = n as f32;
        let floor = self.ball.radius;
        let t1 = crate::self_toss::solve_bounce_time(
            from[1],
            floor,
            to[1],
            total,
            crate::GRAVITY * dt * dt,
            crate::ball::FLOOR_RESTITUTION,
        )?;
        if t1 <= 0.0 || t1 >= total {
            return None;
        }
        let (t1, t) = (t1 * dt, total * dt);
        Some([
            (to[0] - from[0]) / t,
            0.5 * crate::GRAVITY * t1 - (from[1] - floor) / t1,
            (to[2] - from[2]) / t,
        ])
    }

    /// `FUN_00330f98` for a flying receiver `i`, `n` updates on.
    fn lob_endpoint(&self, i: usize, n: i32) -> Option<[f32; 3]> {
        let poses = self.poses.get()?;
        let shown = self.move_shown_ahead(i, n)?;
        let record = shown.record?;
        let p = &self.players[i];
        let mut place = self.place(i);
        place.word = crate::timing::facing_word(p.wanted);
        // cvt.w.s: the frame truncated.
        let frame = shown.frame.max(0.0) as u8;
        let pose = crate::jump::aim_pose(poses, record, frame, p.body(), place, None)?;
        let j = |b: usize| pose.joints[b].position;
        let hand = match poses.state(record, i32::from(frame)) & 0xc {
            8 => j(19),
            4 => j(15),
            _ => std::array::from_fn(|k| (j(19)[k] + j(15)[k]) * 0.5),
        };
        let v = p.flight.velocity;
        let n = n as f32;
        let fall = 0.5 * crate::jump::gravity() * n * n;
        Some([
            hand[0] + v[0] * n,
            (hand[1] + v[1] * n + fall).max(12.5),
            hand[2] + v[2] * n,
        ])
    }

    /// The finishing bodies (`FUN_00334178`, a plan's opcode-1 cue): from
    /// its start a role's actor runs the body on his own channel and
    /// physics as a dunk's script ([`gamebreaker::FinishTrack::row`]), so
    /// its `9e` aims fly him to the rim, gravity brings him down and the
    /// ball sits in the hand his clip shows; the script's poses leave him
    /// until the sequence ends. The script's shot command still releases
    /// the ball (the body's callback 21 falls on the same update), so his
    /// own release moment does nothing here.
    fn step_staged_finishers(
        &mut self,
        plan: &gamebreaker::SequencePlan,
        sequence: &mut gamebreaker::Sequence,
        held_before: Option<(usize, [f32; 3])>,
    ) {
        for finish in &plan.finishes {
            let bit = 1u8 << finish.slot;
            let (Some(row), Some(player)) = (
                finish.row,
                sequence
                    .actors
                    .get(usize::from(finish.slot))
                    .copied()
                    .flatten(),
            ) else {
                continue;
            };
            if sequence.finishing & bit != 0 || sequence.frame < finish.start_frame {
                continue;
            }
            let Some(r) = crate::timing::move_row(row) else {
                continue;
            };
            sequence.finishing |= bit;
            self.sequence = Some(*sequence);
            let hoop = target_hoop(sequence.team);
            let p = &mut self.players[player];
            // His facing stays the script's until the body's own turn.
            p.wanted = p.heading;
            let ahead = [
                p.position[0] + libm::sinf(p.heading),
                p.position[1] + libm::cosf(p.heading),
            ];
            self.begin_move(player, row);
            self.players[player].start_dunk(
                hoop,
                ahead,
                crate::player::DunkStyle::Normal,
                (row, r.release, r.end),
            );
            // His channel held the script's clip until now, at the frame
            // the staged channel shows: the body blends from it, and its
            // resync flag (0x10000, on from 0x44b62a's first step) lifts
            // him by that clip's root over the body's. Script 2's dunker
            // leaves at 4.03 in the School's capture, 982's root at frame
            // 21 two above 984's; from the gameplay clip he left at 1.85.
            // The update before: its record may change clip on the
            // body's first frame (984 starts at 98 too).
            let shown = (sequence.pose_frame() as f32 - 1.0).max(0.0);
            if let Some(pose) = plan.pose_at(shown)[usize::from(finish.slot)] {
                let frame = plan
                    .display_phase_at(usize::from(finish.slot), shown)
                    .map_or(pose.animation_frame, |phase| phase.display_frame);
                if let Some(start) = &mut self.players[player].channel_start {
                    start.from = Some((pose.animation, frame));
                }
            }
        }
        for slot in 0..7 {
            if sequence.finishing & (1 << slot) == 0 {
                continue;
            }
            let Some(player) = sequence.actors[slot] else {
                continue;
            };
            let at = self.players[player].position;
            let held = self.ball.state == BallState::Held(player);
            // His release moment is the script's shot command's.
            let _ = self.players[player].step(&crate::Input::default(), held, None);
            self.flight_tick(player, at);
            if self.ball.state == BallState::Held(player) && !self.place_held(player, held_before) {
                self.ball.carry(&self.players[player]);
            }
        }
    }

    /// Execute genuine ball instructions. Opcode 1 changes the player's
    /// finishing animation; it does not transfer ball ownership. Script
    /// durations time passes, while shot arrival comes from its trajectory.
    fn step_staged_ball(
        &mut self,
        config: &gamebreaker::Config,
        plan: &gamebreaker::SequencePlan,
        sequence: &gamebreaker::Sequence,
        previous: [f32; 3],
    ) {
        let cues: Vec<_> = plan
            .ball_cues
            .iter()
            .copied()
            .filter(|cue| cue.frame == sequence.frame)
            .collect();
        let active_pass = plan
            .ball_cues
            .iter()
            .filter(|cue| cue.opcode() == 0 && cue.frame < sequence.frame)
            .max_by_key(|cue| cue.frame)
            .copied();
        // The board-disable record (`FUN_00148760`): a leg to the backboard
        // (word 16 of 1, its side in the leg, aimed 15 less the radius,
        // 13.25 or more) turns the backboard, collar, rim and basket off on
        // that side for it and the leg after it.
        let passes: Vec<_> = plan
            .ball_cues
            .iter()
            .filter(|cue| cue.opcode() == 0 && cue.frame < sequence.frame)
            .collect();
        let board_off = passes
            .iter()
            .rev()
            .take(2)
            .any(|cue| cue.word(16) == Some(1))
            .then_some(if sequence.mirrored() { 1.0 } else { -1.0 });
        match self.ball.state {
            BallState::Pass => {
                let landed = active_pass
                    .is_some_and(|cue| sequence.frame >= cue.frame.saturating_add(cue.duration));
                let arrived =
                    (self.ball.flight_left > 0.0 && self.ball.fly_pass_with(board_off)) || landed;
                if arrived && let Some(cue) = active_pass {
                    self.ball.flight_left = 0.0;
                    if cue.word(16) == Some(0)
                        && let Ok(role) = u8::try_from(cue.target())
                        && let Some(player) =
                            sequence.actors.get(usize::from(role)).copied().flatten()
                    {
                        self.ball.hold(player);
                        if sequence.finishing & (1 << role) != 0 {
                            if !self.place_held(player, None) {
                                self.ball.carry(&self.players[player]);
                            }
                        } else if let Some(hand) =
                            config.hand_position(plan, sequence, role, sequence.frame as f32)
                        {
                            self.ball.position = hand;
                        }
                        self.events.push(Event::Catch { player });
                    } else {
                        self.ball.velocity = [0.0; 3];
                    }
                }
            }
            BallState::Shot => self.step_ball(None),
            BallState::Held(_) => (),
            // A ball nobody holds (through the net after the basket) falls
            // on its own update (`FUN_0014a968`), the script's or not.
            BallState::Loose => self.step_ball(None),
        }
        for cue in cues {
            if self.phase != Phase::Live || cue.source() == 6 || cue.target() == 6 {
                continue;
            }
            match cue.opcode() {
                0 => self.staged_pass(config, plan, sequence, cue),
                2 => self.staged_shot(config, plan, sequence, cue),
                _ => (),
            }
        }
        if let BallState::Held(player) = self.ball.state {
            let rim = self.hoops[target_hoop(sequence.team)].rim;
            let at = self.ball.position;
            let dx = at[0] - rim[0];
            let dz = at[2] - rim[2];
            // A carried dunk is a basket only when the actual animated ball
            // crosses the rim downward. Catching the alley-oop never scores.
            if self.phase == Phase::Live
                && self.players[player].team == sequence.team
                && previous[1] >= rim[1]
                && at[1] < rim[1]
                && dx * dx + dz * dz < self.hoops[target_hoop(sequence.team)].rim_radius.powi(2)
            {
                self.ball.drop_loose();
                self.ball.velocity = [0.0, -8.0, 0.0];
                self.score_basket(
                    player,
                    Basket::Gamebreaker {
                        level: 2,
                        long: false,
                        dunk: true,
                    },
                );
            }
        }
    }

    fn staged_pass(
        &mut self,
        config: &gamebreaker::Config,
        plan: &gamebreaker::SequencePlan,
        sequence: &gamebreaker::Sequence,
        cue: gamebreaker::BallCue,
    ) {
        let actor = |role: i16| {
            usize::try_from(role)
                .ok()
                .and_then(|r| sequence.actors.get(r).copied().flatten())
        };
        let from = actor(cue.source())
            .or_else(|| self.ball.pass.map(|pass| pass.from))
            .or_else(|| self.handler());
        let Some(from) = from else {
            return;
        };
        let end_frame = cue.frame.saturating_add(cue.duration);
        let target = actor(cue.target());
        let destination = match cue.word(16) {
            Some(0) => {
                let Ok(role) = u8::try_from(cue.target()) else {
                    return;
                };
                let Some(hand) = config.hand_position(plan, sequence, role, end_frame as f32)
                else {
                    return;
                };
                match actor(cue.target()) {
                    // A role on his finishing body flies on his own: the
                    // hand his clip shows then, on the root his flight
                    // reaches by the pass's end (his velocity and gravity,
                    // down to the floor).
                    // A dunker or a leaper (`FUN_0016e9d0`, `FUN_0016e9e0`:
                    // actor bits 7, 15), as a role on his finishing body
                    // is: `FUN_00330f98`, his model posed at the frame his
                    // channel shows by the pass's end, on his place and
                    // wanted facing, the hand by the clip's flags, moved
                    // on by his velocity and gravity over the updates,
                    // and never under 12.5.
                    Some(player) if sequence.finishing & (1 << role) != 0 => self
                        .lob_endpoint(player, cue.duration as i32)
                        .unwrap_or(hand),
                    _ => hand,
                }
            }
            Some(1) => {
                // FUN_0032fe28's high backboard waypoint, in court units.
                let side = if sequence.mirrored() { 1.0 } else { -1.0 };
                [
                    side * (38.0 - self.ball.radius),
                    15.0 - self.ball.radius,
                    0.0,
                ]
            }
            Some(2) => {
                // A point of the record's own (`0x330060..0x3300f8`): words
                // 20, 24 and 28, x turned for the mirrored side.
                let [x, y] = cue.destination();
                let z = cue.word(28).map_or(0.0, |w| f32::from_bits(w as u32));
                [if sequence.mirrored() { -x } else { x }, y, z]
            }
            _ => return,
        };
        let to = target
            .or_else(|| {
                plan.ball_cues
                    .iter()
                    .filter(|next| next.opcode() == 0 && next.frame >= end_frame)
                    .find_map(|next| actor(next.target()))
            })
            .unwrap_or(from);
        let duration = cue.duration.max(1) as f32 * crate::TICK_SECONDS;
        let pass = crate::Pass {
            from,
            to,
            tried: 0,
            via: crate::ball::Via::Direct,
            catch: crate::ball::NO_CATCH,
            since: self.tick,
        };
        // The record's word 12 goes with each leg to the ball's launch
        // (`FUN_001488e8`, `FUN_0014c0c0`'s flag): 1 bounces it off the
        // floor on the way (`FUN_0014f728`'s first contact), a bounce pass.
        let from_point = self.ball.position;
        match self.bounce_velocity(from_point, destination, cue.duration) {
            Some(velocity) if cue.word(12) == Some(1) => self.ball.throw_with_velocity(
                from_point,
                velocity,
                duration,
                crate::Pass {
                    via: crate::ball::Via::Floor,
                    ..pass
                },
            ),
            _ => {
                self.ball.launch(from_point, destination, duration);
                self.ball.state = BallState::Pass;
                self.ball.shot = None;
                self.ball.pass = Some(pass);
            }
        }
        self.events.push(Event::Pass {
            from,
            to,
            style: crate::PassStyle::Normal,
        });
    }

    fn staged_shot(
        &mut self,
        config: &gamebreaker::Config,
        plan: &gamebreaker::SequencePlan,
        sequence: &gamebreaker::Sequence,
        cue: gamebreaker::BallCue,
    ) {
        let Ok(role) = u8::try_from(cue.source()) else {
            return;
        };
        let Some(player) = sequence.actors.get(usize::from(role)).copied().flatten() else {
            return;
        };
        // The release happens at the scripted hand; a ball resting at a
        // hover point comes off it the same way it would from a hand. A
        // role on its finishing body holds it in his own hand.
        let finishing = sequence.finishing & (1 << role) != 0;
        let hand = if finishing && self.ball.state == BallState::Held(player) {
            self.ball.position
        } else {
            let Some(hand) = config.hand_position(plan, sequence, role, sequence.frame as f32)
            else {
                return;
            };
            hand
        };
        let hoop = target_hoop(sequence.team);
        let rim = self.hoops[hoop].rim;
        let long = config.families[1..=4]
            .iter()
            .flatten()
            .any(|&script| script == Some(sequence.script));
        let points = if long { 2 } else { 1 };
        // The actor's own shot (`FUN_00333ef8`, player vt+0x12c): the
        // launcher `FUN_0014aaf8` as a gameplay shot's release calls it,
        // made unless the record's word 8, aimed by its word 12 when that
        // is 5 or less (else straight at the basket), at 67.5° with its
        // word 16 of 1 and otherwise `π/4 + π/8 × max(0, (20 − d) / 20)` for
        // his distance `d` from the rim across the floor: a shot's own arc,
        // its flight from the angle with the launcher's half-second and
        // 14-unit floors. A shooter in a dunk or the alley-oop leap takes
        // the launcher's dunk path at the hand's pace.
        // The launcher's fourth argument (`FUN_00141038`'s script bands)
        // only records the basket for the side.
        let made = cue.word(8) == Some(0);
        let kind = cue
            .word(12)
            .and_then(|k| u8::try_from(k).ok())
            .filter(|&k| k <= 5)
            .unwrap_or(0);
        let pose = config
            .clip_pose(plan, sequence, role, sequence.frame as f32)
            .filter(|_| !finishing);
        let at = pose.map_or(self.players[player].position, |p| {
            let x = if sequence.mirrored() {
                -p.position[0]
            } else {
                p.position[0]
            };
            [x, p.position[2]]
        });
        let d = ((at[0] - rim[0]) * (at[0] - rim[0]) + at[1] * at[1]).sqrt();
        let angle = if cue.word(16) == Some(1) {
            f32::from_bits(0x3f96_cbe4)
        } else {
            ((20.0 - d) / 20.0).max(0.0) * f32::from_bits(0x3ec9_0fda) + f32::from_bits(0x3f49_0fdb)
        };
        // The launcher's dunk path (`0x14abf0..0x14ac18`): the shooter in a
        // dunk (bit 7) or the alley-oop leap (bit 15). A staged actor's
        // state word comes from his finishing body (its last `9b` before
        // the callback 21 that releases the ball); the clips' own words
        // hold only the hand flags.
        let dunking = plan
            .finishes
            .iter()
            .filter(|f| f.slot == role && f.start_frame <= sequence.frame)
            .max_by_key(|f| f.start_frame)
            .is_some_and(|f| f.release_bits & 0x8080 != 0);
        let shot = crate::Shot {
            shooter: player,
            hoop,
            made,
            points,
            gamebreaker: 2,
            layup: false,
            tried: 0,
            slam: None,
        };
        self.ball.position = hand;
        if dunking {
            let middle = [rim[0], crate::shooting::NATIVE_AIM_HEIGHT, rim[2]];
            let from = crate::shooting::dunk_place(hand, middle, self.ball.radius);
            let pace = crate::shooting::dunk_pace(crate::pose::length(self.ball.moved), false);
            let call = crate::shooting::ShotCall {
                percent: 0,
                made,
                kind: if made { 0 } else { 3 },
                angle,
            };
            let slam = Some(Basket::Gamebreaker {
                level: 2,
                long,
                dunk: true,
            });
            self.launch_shot(hoop, from, call, crate::Shot { slam, ..shot }, Some(pace));
        } else {
            let call = crate::shooting::ShotCall {
                percent: 100,
                made,
                kind,
                angle,
            };
            self.launch_shot(hoop, hand, call, shot, None);
        }
        self.events.push(Event::Shot {
            player,
            hoop,
            points: if long { 2 } else { 1 },
        });
    }
}

/// True when `p` is in the middle of a trick move.
pub fn tricking(p: &Player) -> bool {
    p.action.juking()
}
