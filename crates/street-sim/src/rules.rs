//! Game settings, as the game's "My Rules" screen offers them (`XLOC_FE`):
//! a game mode, "Play To", "Spot Points" and the shot clock (off, 12, 24 or
//! 48 seconds). The modes:
//!
//! - Normal / Pick Up: "Play a game up to a set number of points."
//! - Trick Point Challenge: "Play a game up to a set number of trick points."
//! - Dunks Only: "Play a game where only dunks count for points."
//! - No Gamebreakers: "Gamebreakers do not count towards your score."
//! - League Scoring: "Play a game with 2 point shots and 3 point shots."
//! - Old School Scoring: "Play a game with no long range shots."
//! - Gamebreaker Wins: "The first player to score the Gamebreaker wins the
//!   game", at a set level.
//! - Back To Basics: "No gamebreakers or trick points. Just ball."

/// What wins the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    /// First to this many points, winning by [`Rules::win_by`].
    Points(u32),
    /// First to this many trick points.
    TrickPoints(u32),
    /// The first to score a gamebreaker of at least this level.
    Gamebreaker(u8),
}

/// What baskets are worth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scoring {
    /// 1 inside the long-range arc, 2 beyond it.
    Street,
    /// 2 and 3.
    League,
    /// Everything 1: "When Stretch started running at this court, there was
    /// no long range arc. All shots were worth one point."
    OldSchool,
}

/// The game modes of "My Rules".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    TrickPointChallenge,
    DunksOnly,
    NoGamebreakers,
    LeagueScoring,
    OldSchool,
    GamebreakerWins,
    BackToBasics,
}

impl Mode {
    pub const ALL: [Mode; 8] = [
        Mode::Normal,
        Mode::TrickPointChallenge,
        Mode::DunksOnly,
        Mode::NoGamebreakers,
        Mode::LeagueScoring,
        Mode::OldSchool,
        Mode::GamebreakerWins,
        Mode::BackToBasics,
    ];

    /// The name the game's menu uses.
    pub fn name(self) -> &'static str {
        match self {
            Mode::Normal => "Normal",
            Mode::TrickPointChallenge => "Trick Point Challenge",
            Mode::DunksOnly => "Dunks Only",
            Mode::NoGamebreakers => "No Gamebreakers",
            Mode::LeagueScoring => "League Scoring",
            Mode::OldSchool => "Old School Scoring",
            Mode::GamebreakerWins => "Gamebreaker Wins",
            Mode::BackToBasics => "Back To Basics",
        }
    }
}

/// How hard the computer plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    /// How much more (or less) often the computer goes for steals, blocks,
    /// counters and tricks, and how choosy it is about shots.
    pub fn factor(self) -> f32 {
        match self {
            Difficulty::Easy => 0.6,
            Difficulty::Medium => 1.0,
            Difficulty::Hard => 1.35,
        }
    }

    /// The sim's own rubber-band pair for the one-player match (the
    /// game's `FUN_00143458` blends meter tuning scalars with these as
    /// its inner mixes — see `docs/TRACE.md` "Difficulty"; the
    /// crew-rating blend is the sim's model, not that function's).
    pub fn ratings_blend(self) -> (f32, f32) {
        match self {
            Difficulty::Easy => (1.0, 0.0),
            Difficulty::Medium => (0.6875, 0.3125),
            Difficulty::Hard => (0.375, 0.625),
        }
    }

    /// The saved option's level (`FUN_001d74a8`'s bits 0..1): 0 Got Game,
    /// 1 Mad Game, 2 Legendary.
    pub fn from_level(level: u8) -> Self {
        match level {
            0 => Difficulty::Easy,
            1 => Difficulty::Medium,
            _ => Difficulty::Hard,
        }
    }

    /// The level the option saves (the inverse of [`Difficulty::from_level`]).
    pub fn level(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rules {
    pub goal: Goal,
    /// A points game must be won by this much.
    pub win_by: u32,
    /// Seconds a team has to shoot (12, 24 or 48), or `None` for off.
    pub shot_clock: Option<f32>,
    pub scoring: Scoring,
    /// Only dunks (and alley-oops) score.
    pub dunks_only: bool,
    /// Trick points are counted.
    pub tricks: bool,
    /// The gamebreaker meter fills and gamebreakers can be used.
    pub gamebreakers: bool,
    /// Gamebreakers score their extra points and take points away; in "No
    /// Gamebreakers" they count as ordinary baskets.
    pub gamebreakers_score: bool,
    /// Points each team starts with ("Spot Points"); in Gamebreaker Wins
    /// the meter each starts with, in percent, 100 and over a pocketed
    /// Level 1 and the rest (`FUN_001457f8`, [`Rules::meter_spot`]).
    pub spot: [u32; 2],
    /// Trick points each team starts with.
    pub spot_trick_points: [u32; 2],
    /// The "Unlimited Turbo" cheat.
    pub unlimited_turbo: bool,
    /// The "No Counters" cheat: trick counters never take the ball.
    pub no_counters: bool,
    /// The "Easy 2 Pointers" cheat: the long-range rating is forced to 99
    /// (1.0 here), via FUN_001d7d00's selector at singleton+0x4c.
    pub easy_2_pointers: bool,
    /// The "Hard 2 Pointers" cheat: the long-range rating is forced to 0.
    pub hard_2_pointers: bool,
    pub difficulty: Difficulty,
}

impl Default for Rules {
    fn default() -> Self {
        Self::mode(Mode::Normal)
    }
}

/// The shot clock's rows (`FUN_001dbd60`'s word, setup `+0xec`): off, 12,
/// 24 and 48 seconds (My Rules' Shot Clock row, 0xe0e..0xe11).
pub const CLOCKS: [Option<f32>; 4] = [None, Some(12.0), Some(24.0), Some(48.0)];

impl Rules {
    /// The rules the game setup's words make: the game type (`+0xe8`,
    /// `FUN_001dbd48`, 0..7 by [`Mode::ALL`]), the shot clock's row
    /// (`+0xec`, `FUN_001dbd60`, [`CLOCKS`]), the target (`+0xf0`,
    /// `FUN_001dbd78`: the points, the trick points, or in Gamebreaker Wins
    /// the level less one, which `FUN_001457f8` copies to game+0xf8) and
    /// the spots by `FUN_001dbd80`'s index (`+0xf4` then `+0xf8`: points,
    /// trick points or Gamebreaker Wins' meters), on `base`'s difficulty
    /// and cheats. The statistics' constructor (`FUN_00186b10`) starts side
    /// 0's score from `+0xf8` and side 1's from `+0xf4` (stat 0, or 0xb in
    /// the trick-point game; none in Gamebreaker Wins), and Gamebreaker
    /// Wins' init (`FUN_001457f8`) side 0's meter (game+0xc8) from `+0xf8`
    /// and side 1's (+0xcc) from `+0xf4`: index 0 spots side 1. My Rules'
    /// home side is index 1 (`+0x10` 1).
    pub fn from_setup(
        game_type: u32,
        clock: u32,
        value: u32,
        spots: [u32; 2],
        base: &Rules,
    ) -> Rules {
        let mode = Mode::ALL[(game_type as usize).min(7)];
        let mut r = Rules {
            difficulty: base.difficulty,
            unlimited_turbo: base.unlimited_turbo,
            no_counters: base.no_counters,
            easy_2_pointers: base.easy_2_pointers,
            hard_2_pointers: base.hard_2_pointers,
            ..Rules::mode(mode)
        };
        r.shot_clock = CLOCKS[(clock as usize).min(3)];
        let by_side = [spots[1], spots[0]];
        r.goal = match r.goal {
            Goal::Points(_) => {
                r.spot = by_side;
                Goal::Points(value)
            }
            Goal::TrickPoints(_) => {
                r.spot_trick_points = by_side;
                Goal::TrickPoints(value)
            }
            Goal::Gamebreaker(_) => {
                r.spot = by_side;
                Goal::Gamebreaker(if value == 1 { 2 } else { 1 })
            }
        };
        r
    }

    /// Gamebreaker Wins' start for `side`'s meter (`FUN_001457f8`): a spot
    /// of 100 and over is a pocketed Level 1 (meter+0x20) and the rest,
    /// each percent 500 trick points (`FUN_001448e0`, which keeps them
    /// under a full meter's 50,000 here: My Rules' spot stops 10 under the
    /// level's 100 or 200). Every other game: none.
    pub fn meter_spot(&self, side: usize) -> (bool, u32) {
        let Goal::Gamebreaker(_) = self.goal else {
            return (false, 0);
        };
        let spot = self.spot[side.min(1)];
        let (pocketed, rest) = if spot > 99 {
            (true, spot - 100)
        } else {
            (false, spot)
        };
        (pocketed, rest.saturating_mul(500).min(50_000))
    }

    /// The points each side starts with: the spots, but in Gamebreaker
    /// Wins, whose spots are the meters' and whose statistics take none
    /// (`FUN_00186b10` skips type 6), none.
    pub fn start_score(&self) -> [u32; 2] {
        match self.goal {
            Goal::Gamebreaker(_) => [0; 2],
            _ => self.spot,
        }
    }

    /// The settings a mode starts from.
    pub fn mode(mode: Mode) -> Self {
        let normal = Self {
            goal: Goal::Points(21),
            win_by: 2,
            shot_clock: Some(24.0),
            scoring: Scoring::Street,
            dunks_only: false,
            tricks: true,
            gamebreakers: true,
            gamebreakers_score: true,
            spot: [0; 2],
            spot_trick_points: [0; 2],
            unlimited_turbo: false,
            no_counters: false,
            easy_2_pointers: false,
            hard_2_pointers: false,
            difficulty: Difficulty::Medium,
        };
        match mode {
            Mode::Normal => normal,
            Mode::TrickPointChallenge => Self {
                goal: Goal::TrickPoints(100_000),
                ..normal
            },
            Mode::DunksOnly => Self {
                dunks_only: true,
                ..normal
            },
            Mode::NoGamebreakers => Self {
                gamebreakers_score: false,
                ..normal
            },
            Mode::LeagueScoring => Self {
                scoring: Scoring::League,
                ..normal
            },
            Mode::OldSchool => Self {
                scoring: Scoring::OldSchool,
                ..normal
            },
            Mode::GamebreakerWins => Self {
                goal: Goal::Gamebreaker(1),
                ..normal
            },
            Mode::BackToBasics => Self {
                tricks: false,
                gamebreakers: false,
                ..normal
            },
        }
    }

    /// The shot clock's full time, or 0 when it is off.
    pub fn clock(&self) -> f32 {
        self.shot_clock.unwrap_or(0.0)
    }

    /// The game type as the original keeps it (config+0xe8,
    /// `FUN_001dbd48`): the mode's place in [`Mode::ALL`], read back from
    /// the rules a mode starts from (0 for any mix the modes do not make).
    /// The meter's fill (`FUN_00143d38`) skips types 3 and 7, its decay
    /// (`FUN_00143c20`) type 7, the statistics' trick points type 7; the
    /// basket's listeners count only dunks and alley-oops in type 2; the
    /// game-winning dunk is no class 6 in type 1.
    pub fn game_type(&self) -> u32 {
        if let Goal::TrickPoints(_) = self.goal {
            1
        } else if let Goal::Gamebreaker(_) = self.goal {
            6
        } else if self.dunks_only {
            2
        } else if !self.tricks && !self.gamebreakers {
            7
        } else if !self.gamebreakers_score {
            3
        } else {
            match self.scoring {
                Scoring::League => 4,
                Scoring::OldSchool => 5,
                Scoring::Street => 0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setups_spots_reach_the_sides_by_the_statistics_reading() {
        // `FUN_001dbd80(setup, 0, 3)` writes +0xf4, which the statistics'
        // constructor gives side 1; index 1 (+0xf8) is side 0's.
        let base = Rules::default();
        let r = Rules::from_setup(0, 2, 21, [3, 0], &base);
        assert_eq!((r.spot, r.start_score()), ([0, 3], [0, 3]));
        let t = Rules::from_setup(1, 2, 250_000, [0, 50_000], &base);
        assert_eq!((t.spot_trick_points, t.spot), ([50_000, 0], [0, 0]));
    }

    #[test]
    fn gamebreaker_wins_starts_the_meters_from_the_spots() {
        // `FUN_001457f8`: +0xf8 to side 0's meter, +0xf4 to side 1's; 100
        // and over a pocketed Level 1, the rest times 500; no points.
        let r = Rules::from_setup(6, 2, 1, [40, 150], &Rules::default());
        assert_eq!(r.goal, Goal::Gamebreaker(2));
        assert_eq!(r.meter_spot(0), (true, 25_000));
        assert_eq!(r.meter_spot(1), (false, 20_000));
        assert_eq!(r.start_score(), [0, 0]);
        let w = crate::World::new(crate::Hoop::native(), Vec::new(), r, 1);
        assert_eq!(w.score, [0, 0]);
        assert_eq!(
            w.teams.each_ref().map(|t| (t.pocketed, t.meter)),
            [(true, 25_000), (false, 20_000)]
        );
        // Any other game's meters start empty.
        let n = Rules::from_setup(0, 2, 21, [40, 150], &Rules::default());
        assert_eq!((n.meter_spot(0), n.start_score()), ((false, 0), [150, 40]));
    }
}
