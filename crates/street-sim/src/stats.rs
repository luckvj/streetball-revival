//! The match's statistics: the stats object behind the game's stats
//! listener (vtable `0x440578`, built per match; the object is the
//! listener's +8). `FUN_00186ef8(stats, who, scope, stat, amount)` adds,
//! `FUN_00186e50` sets and `FUN_001871e0`/`FUN_00186df8` read. Stats 0..14
//! are kept for each actor (scope 0, six), each side (scope 1, two) and
//! each controller (scope 2, four; not modelled: the sim has no
//! controllers); 0x10..0x27 for each side only (+0xb8 + side × 0x60).
//! `FUN_00186fa8` lets none through before the game starts or after it
//! (phases 0 and 9), and during the script manager's states 3..7 only
//! some (the sim records them all, as its staged sequences score as play).
//!
//! The listeners, by slot (the post offset game+0x3b + slot):
//!
//! | Slot | Listener | Moment | Stats |
//! |---|---|---|---|
//! | 11 | `FUN_001877b0` | a shot leaves the hand (`FUN_0014aaf8`) | 9 a three tried, 10 a shot tried (the event's base 2 or 3) |
//! | 13 | `FUN_001884e0` | a trick move lands | 11 the move's value; 0x16/0x17 by the side |
//! | 15 | `FUN_00188640` | a steal or trick counter | 5; 11 + 1,000 or 3,000 |
//! | 16 | `FUN_00188868` | a block (`FUN_0016d888`: the ball knocked loose; never caught in the captures) | 6; 11 + 2,500 |
//! | 17 | `FUN_00187a08` | a rebound | 4; 11 + 500 |
//! | 19 | `FUN_00187480` | a passout special | 11 + team meter+0x5c |
//! | 24 | `FUN_00188b20` | every catch | 11 + 500 for a dive, unless 15/16/17 just paid |
//! | 26 | `FUN_00187c10` | a basket | 0, 1 dunks, 2 alley-oops, 3 assists, 7, 8 threes, 11, 12 and 13 the gamebreakers |
//! | 39 | `FUN_00187930` | a dunk's slam | 14 |
//! | 66, 67 | `FUN_001883f0`, `FUN_00188368` | the meter fills, drops | 0x11/0x13, 0x12/0x14 (with a pocket) |
//! | 71 | `FUN_00187ba0` | a cancel | 11 + 5,000 |
//! | 73, 74 | `FUN_00187338`, `FUN_001875e0` | a pass special; a contact | 11 + 500 (a pick: contact kind 3) |
//!
//! The post-game screens read 0, 1, 4, 5, 6, 11, 12 and 13
//! (`docs/TRACE.md`), the computer players' star 0 and 10
//! (`FUN_0012a7d0`, [`Stats::star`]).
//!
//! Ported: stats 0..14 for actors and sides, 0x10..0x27 for sides — the
//! assists (3, the ball's assist window `ball+0x10c`/`+0x158`), the
//! trick points by source and `FUN_00187200`'s walk of a paid combo, the
//! passout specials (19), the pump fake's and the pick's 500 (73, 74), the
//! blocker's attempt credit (`FUN_00188868`) — the gate `FUN_00186fa8`
//! (phases 0 and 9, the staged states' list) and the type-7 gate on trick
//! points; the listeners themselves are [`crate::scoring`]'s. Not ported:
//! controller scope (2; the sim has no controllers) and the score snapshot
//! at a fill (`+0x9e4`).

/// Points (the scoring function's, vt+0x84).
pub const POINTS: usize = 0;
pub const DUNKS: usize = 1;
pub const ALLEY_OOPS: usize = 2;
pub const ASSISTS: usize = 3;
pub const REBOUNDS: usize = 4;
pub const STEALS: usize = 5;
pub const BLOCKS: usize = 6;
/// Baskets made, and those from beyond the arc (the event's base 3).
pub const FIELD_GOALS: usize = 7;
pub const THREES: usize = 8;
/// Shots tried from beyond the arc, and all shots tried.
pub const THREE_ATTEMPTS: usize = 9;
pub const ATTEMPTS: usize = 10;
pub const TRICK_POINTS: usize = 11;
/// Level 1 and Level 2 gamebreaker baskets.
pub const LEVEL1: usize = 12;
pub const LEVEL2: usize = 13;
/// Dunks slammed (`FUN_00187930`, slot 39, the ball's dunker).
pub const SLAMS: usize = 14;
/// Stats an actor and a side both keep.
pub const ACTOR_STATS: usize = 15;

/// Side-only stats, from 0x10: the meter filling without and with a
/// pocketed gamebreaker (0x11, 0x13), dropping (0x12, 0x14), and the
/// biggest lead (0x15, `FUN_00187c10`).
pub const METER_FILLS: usize = 0x11;
pub const METER_DROPS: usize = 0x12;
pub const METER_FILLS_POCKETED: usize = 0x13;
pub const METER_DROPS_POCKETED: usize = 0x14;
pub const BIGGEST_LEAD: usize = 0x15;
/// 0x10..0x27.
pub const SIDE_STATS: usize = 24;

/// The statistics of one match.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    /// Stats 0..14 by actor and by side.
    pub players: Vec<[i32; ACTOR_STATS]>,
    pub teams: [[i32; ACTOR_STATS]; 2],
    /// Stats 0x10..0x27 by side.
    pub sides: [[i32; SIDE_STATS]; 2],
    /// Actor+0x2a4: updates each actor has played, counted in live play
    /// (`FUN_00156f48` adds 1 where `FUN_00140558` lets play run;
    /// `FUN_00156870` clears it for the match).
    pub on_court: Vec<u32>,
    /// The loose ball came off a shot, a slam or a block (the rewrite's
    /// own flag; the take reads the ball's states 0 and 6 for slot 17).
    pub rebound: bool,
    /// The listener's +0xa10: a steal, block or rebound paid this take, so
    /// the catch's dive pays nothing (`FUN_00188b20`).
    pub paid: bool,
    /// The listener's +0xa0c: a pump fake (slot 5, `FUN_00187328`) arms
    /// its points (slot 73, `FUN_00187338`).
    pub pump_fake: bool,
    /// +0x9e4/+0x9e8 by side: the side's score at the meter's fill before
    /// last and at its last (slot 66, `FUN_001883f0`: the trick points in
    /// the trick-point game, else the game's vt+0x9c), which the pocket
    /// checks read ([`crate::World::class_score`]).
    pub fills: [[i32; 2]; 2],
}

impl Stats {
    pub fn new(players: usize) -> Self {
        Self {
            players: vec![[0; ACTOR_STATS]; players],
            on_court: vec![0; players],
            ..Default::default()
        }
    }

    /// `FUN_00186ef8` for the actor and his side.
    pub fn add(&mut self, player: usize, team: usize, stat: usize, amount: i32) {
        if let Some(p) = self.players.get_mut(player) {
            p[stat] += amount;
        }
        self.teams[team.min(1)][stat] += amount;
    }

    /// `FUN_00186ef8` for a side alone.
    pub fn add_team(&mut self, team: usize, stat: usize, amount: i32) {
        let team = team.min(1);
        if stat < ACTOR_STATS {
            self.teams[team][stat] += amount;
        } else {
            self.sides[team][stat - 0x10] += amount;
        }
    }

    /// `FUN_001871e0` / `FUN_00186df8`: an actor's stat (scope 0).
    pub fn player(&self, player: usize, stat: usize) -> i32 {
        self.players.get(player).map_or(0, |p| p[stat])
    }

    /// A side's stat (scope 1; 0x10.. read the side-only block).
    pub fn team(&self, team: usize, stat: usize) -> i32 {
        let team = team.min(1);
        if stat < ACTOR_STATS {
            self.teams[team][stat]
        } else {
            self.sides[team][stat - 0x10]
        }
    }

    /// `FUN_00181ba8`: whole seconds an actor has played, actor+0x2a4 /
    /// fps (integer division).
    pub fn seconds(&self, player: usize) -> i32 {
        (self.on_court.get(player).copied().unwrap_or(0) / crate::TICK_HZ) as i32
    }

    /// Slot 66 (`FUN_001883f0`): the count, and the side's score now
    /// after its score at the last fill ([`Stats::fills`]).
    pub fn meter_filled(&mut self, team: usize, pocketed: bool, score: i32) {
        let stat = if pocketed {
            METER_FILLS_POCKETED
        } else {
            METER_FILLS
        };
        self.add_team(team, stat, 1);
        let fills = &mut self.fills[team.min(1)];
        *fills = [fills[1], score];
    }

    /// Slot 67 (`FUN_00188368`).
    pub fn meter_dropped(&mut self, team: usize, pocketed: bool) {
        let stat = if pocketed {
            METER_DROPS_POCKETED
        } else {
            METER_DROPS
        };
        self.add_team(team, stat, 1);
    }

    /// Slot 11 (`FUN_001877b0`): a shot left `player`'s hand, its base 3
    /// from beyond the arc, else 2.
    pub fn shot(&mut self, player: usize, team: usize, long: bool) {
        if long {
            self.add(player, team, THREE_ATTEMPTS, 1);
        }
        self.add(player, team, ATTEMPTS, 1);
    }

    /// Slot 26 (`FUN_00187c10`): `player` scored `points` (the other side
    /// loses `taken`), the ball's kind (2 a jump shot, 3 an alley-oop, 4 a
    /// dunk, 5 a lay-up), the base (3 beyond the arc), the gamebreaker
    /// level, and the trick points the basket paid (the chain's total and
    /// the class's value).
    #[allow(clippy::too_many_arguments)]
    pub fn basket(
        &mut self,
        player: usize,
        team: usize,
        points: u32,
        taken: u32,
        kind: u8,
        long: bool,
        gamebreaker: u8,
        trick_points: u32,
    ) {
        let team = team.min(1);
        let other = 1 - team;
        if long {
            self.add(player, team, THREES, 1);
        }
        self.add(player, team, POINTS, points as i32);
        self.teams[other][POINTS] -= taken as i32;
        if self.teams[other][POINTS] < 0 {
            self.teams[other][POINTS] = 0;
        }
        // 0x15: the biggest lead each side has had.
        let lead = self.teams[0][POINTS] - self.teams[1][POINTS];
        if self.sides[0][BIGGEST_LEAD - 0x10] < lead {
            self.sides[0][BIGGEST_LEAD - 0x10] = lead;
        }
        if self.sides[1][BIGGEST_LEAD - 0x10] < -lead {
            self.sides[1][BIGGEST_LEAD - 0x10] = -lead;
        }
        self.add(player, team, FIELD_GOALS, 1);
        match kind {
            4 => self.add(player, team, DUNKS, 1),
            3 => self.add(player, team, ALLEY_OOPS, 1),
            _ => {}
        }
        match gamebreaker {
            0 => {}
            1 => self.add(player, team, LEVEL1, 1),
            _ => self.add(player, team, LEVEL2, 1),
        }
        if trick_points != 0 {
            self.add(player, team, TRICK_POINTS, trick_points as i32);
        }
    }

    /// `FUN_0012a7d0`'s weights for one side's three (`squad`, in slot
    /// order) by their overall ratings (rating 9, `ratings`): each one's
    /// share of the side's total, and once both are known, the share times
    /// the side's (points × 2 + shots tried) per second played over his
    /// own. The best weight is the star (the first of equals); the weights
    /// as percentages of their sum are the playbook's +0xcc..+0xd4.
    pub fn star_weights(&self, team: usize, squad: &[usize], ratings: &[i32]) -> Vec<f32> {
        let total: i32 = ratings.iter().sum();
        let time: i32 = squad.iter().map(|&j| self.seconds(j)).sum();
        let side_rate = if time == 0 {
            0.0
        } else {
            let tried = self.team(team, ATTEMPTS) as f32;
            let points = self.team(team, POINTS) as f32;
            (tried + points + points) / time as f32
        };
        squad
            .iter()
            .zip(ratings)
            .map(|(&j, &r)| {
                let seconds = self.seconds(j);
                let own = if seconds == 0 {
                    0.0
                } else {
                    let tried = self.player(j, ATTEMPTS) as f32;
                    let points = self.player(j, POINTS) as f32;
                    (tried + points + points) / seconds as f32
                };
                let share = r as f32 / total as f32;
                if own != 0.0 && side_rate != 0.0 {
                    side_rate * share / own
                } else {
                    share
                }
            })
            .collect()
    }
}
