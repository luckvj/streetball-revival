//! The game object's class and the virtual methods the rules and the
//! computer ask of it.
//!
//! The game's factory (`FUN_002f3000`) builds the lesson game (vtable
//! `0x441680`, game+0x9c 4) for Street School, the trick-point game
//! (`0x440ce8`) for game type 1, Gamebreaker Wins (`0x43e9d0`) for type 6
//! and the points game (`0x43b490`) for the rest; their virtuals differ at
//! +0x44 (the catch-up cell, [`crate::shooting::catch_up_cell`]), +0x7c
//! (would a basket win, [`World::basket_would_win`]), +0x84 (a basket's
//! points, [`POINTS`]), +0x9c (a side's score, [`World::class_score`]),
//! +0xbc (the game is won), +0xc4 (pocket now, [`World::pocket_now`]) and
//! +0xcc (cancel now, [`World::cancel_now`]).

use crate::{Goal, World};

/// Which class the game object is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Points,
    TrickPoints,
    GamebreakerWins,
    /// Street School's (`0x441680`): its +0x7c and +0xbc say no.
    Lesson,
}

/// `0x43b618`: a basket's points by game type, gamebreaker level (0, 1, 2:
/// the ball's +0x178) and kind (0 a dunk or alley-oop, 1 any other inside
/// basket, 2 from beyond the arc): the scorer's, then the other side's
/// (`FUN_0012e998`, the points game's vt+0x84; the trick-point and
/// Gamebreaker Wins games' own leave both 0). The stats' basket listener
/// (`FUN_00187c10`) adds them, the other side's floored at 0.
pub const POINTS: [[[[i32; 2]; 3]; 3]; 8] = [
    [
        [[1, 0], [1, 0], [2, 0]],
        [[1, -1], [1, -1], [2, -1]],
        [[2, -3], [2, -3], [2, -4]],
    ],
    [
        [[0, 0], [0, 0], [0, 0]],
        [[0, 0], [0, 0], [0, 0]],
        [[0, 0], [0, 0], [0, 0]],
    ],
    [
        [[1, 0], [0, 0], [0, 0]],
        [[1, -1], [0, 0], [0, 0]],
        [[2, -3], [0, 0], [0, 0]],
    ],
    [
        [[1, 0], [1, 0], [2, 0]],
        [[0, 0], [0, 0], [0, 0]],
        [[0, 0], [0, 0], [0, 0]],
    ],
    [
        [[2, 0], [2, 0], [3, 0]],
        [[2, -2], [2, -2], [3, -2]],
        [[3, -4], [3, -4], [3, -7]],
    ],
    [
        [[1, 0], [1, 0], [1, 0]],
        [[1, -1], [1, -1], [1, -1]],
        [[2, -3], [2, -3], [2, -3]],
    ],
    [
        [[0, 0], [0, 0], [0, 0]],
        [[0, 0], [0, 0], [0, 0]],
        [[0, 0], [0, 0], [0, 0]],
    ],
    [
        [[1, 0], [1, 0], [2, 0]],
        [[1, -1], [1, -1], [2, -1]],
        [[2, -3], [2, -3], [2, -5]],
    ],
];

/// `FUN_0012e998` for a game type: the scorer's and the other side's
/// points for a basket of `kind` (0..2) at gamebreaker `level` (0..2).
/// The singleton's +0x70 (a long basket counted as an inside one) is
/// never written, so kind 2 stays 2.
pub fn points(game_type: u32, kind: usize, level: u8) -> [i32; 2] {
    POINTS[(game_type as usize).min(7)][usize::from(level.min(2))][kind.min(2)]
}

impl World {
    /// The game object's class (`FUN_002f3000`).
    pub fn class(&self) -> Class {
        if self.lesson.is_some() {
            return Class::Lesson;
        }
        match self.rules.goal {
            Goal::TrickPoints(_) => Class::TrickPoints,
            Goal::Gamebreaker(_) => Class::GamebreakerWins,
            Goal::Points(_) => Class::Points,
        }
    }

    /// The ball's gamebreaker (ball+0x178, `FUN_00141028`; `FUN_00141018`
    /// asks whether it is set): set when a gamebreaker goes up
    /// (`FUN_001422f8`), cleared at the basket or the floor. The rewrite's
    /// gamebreakers are shots: its level while a gamebreaker shot is in
    /// the air or on the rim, or its shooter holds the ball in it.
    pub fn gamebreaker_ball(&self) -> u8 {
        if let Some(shot) = self.ball.shot.filter(|s| s.gamebreaker > 0) {
            return shot.gamebreaker;
        }
        match self.handler().map(|h| &self.players[h].action) {
            Some(crate::Action::Shooting { gamebreaker, .. }) => *gamebreaker,
            _ => 0,
        }
    }

    /// The game's vt+0x9c: a side's score as its class keeps it — the
    /// points (`FUN_00140fd0`, stat 0) in the points and lesson games, the
    /// trick points (stat 0xb: `FUN_0018c760`, `FUN_00145a20`) in the
    /// trick-point and Gamebreaker Wins games.
    pub fn class_score(&self, team: usize) -> i32 {
        match self.class() {
            Class::TrickPoints | Class::GamebreakerWins => {
                self.stats.team(team, crate::stats::TRICK_POINTS)
            }
            Class::Points | Class::Lesson => self.score[team.min(1)] as i32,
        }
    }

    /// The game's vt+0x7c: whether a basket by `team` — `base` 3 from
    /// beyond the arc, else inside — at gamebreaker `level` would win.
    ///
    /// - the points game (`FUN_0012e890`): with [`points`]'s values for the
    ///   basket (kind 2 for base 3, else 1), either side at the target
    ///   (game+0xf8) and two or more apart;
    /// - the trick-point game (`FUN_0018c698`, its arguments unread): the
    ///   side's trick points past the target and past the other side's;
    /// - Gamebreaker Wins (`FUN_00145990`): won already (its vt+0xbc,
    ///   never in play), or a Level 1 to a Level 1 game (game+0xf8 0) or a
    ///   Level 2 to either;
    /// - the lesson game (`0x190b88`): never.
    pub fn basket_would_win(&self, team: usize, base: i32, level: u8) -> bool {
        let team = team.min(1);
        match (self.class(), self.rules.goal) {
            (Class::Lesson, _) => false,
            (Class::TrickPoints, Goal::TrickPoints(target)) => {
                let own = self.stats.team(team, crate::stats::TRICK_POINTS);
                let other = self.stats.team(1 - team, crate::stats::TRICK_POINTS);
                own > target as i32 && own > other
            }
            (Class::GamebreakerWins, Goal::Gamebreaker(goal)) => match level {
                1 => goal <= 1,
                2 => true,
                _ => false,
            },
            (_, Goal::Points(target)) => {
                let kind = if base == 3 { 2 } else { 1 };
                let [ours, theirs] = points(self.rules.game_type(), kind, level);
                let us = self.score[team] as i32 + ours;
                let them = self.score[1 - team] as i32 + theirs;
                let target = target as i32;
                (target <= us || target <= them) && 1 < (us - them).abs()
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(rules: crate::Rules) -> World {
        World::new(crate::tests::hoops(), vec![], rules, 1)
    }

    #[test]
    fn the_points_table_is_by_type_level_and_kind() {
        // Normal: dunk 1, inside 1, long 2; a Level 1 takes 1, a Level 2
        // scores 2 and takes 3 (4 from beyond the arc).
        assert_eq!(points(0, 0, 0), [1, 0]);
        assert_eq!(points(0, 2, 1), [2, -1]);
        assert_eq!(points(0, 1, 2), [2, -3]);
        assert_eq!(points(0, 2, 2), [2, -4]);
        // league scoring: 2 and 3; Old School 1 everywhere; Dunks Only only
        // a dunk; the trick-point and Gamebreaker Wins games nothing.
        assert_eq!(points(4, 2, 0), [3, 0]);
        assert_eq!(points(4, 2, 2), [3, -7]);
        assert_eq!(points(5, 2, 0), [1, 0]);
        assert_eq!(points(2, 1, 0), [0, 0]);
        assert_eq!(points(2, 0, 0), [1, 0]);
        assert_eq!(points(1, 2, 2), [0, 0]);
        assert_eq!(points(6, 0, 1), [0, 0]);
    }

    /// `POINTS` is the executable's `0x43b618` (STREET_ISO).
    #[test]
    fn the_points_table_is_the_executables() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let elf = street_assets::iso::Iso::open(path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let words = street_assets::scripts::executable_words(&elf, 0x43b618, 8 * 18).unwrap();
        for (k, w) in words.iter().enumerate() {
            let (game_type, level, kind, side) = (k / 18, k % 18 / 6, k % 6 / 2, k % 2);
            assert_eq!(POINTS[game_type][level][kind][side], *w as i32, "{k}");
        }
    }

    #[test]
    fn a_basket_wins_by_the_game_objects_class() {
        // Points: 20 to 18 at 21 wins with an inside basket; 20 to 19 does
        // not (21 to 19 is two apart; 21 to 20 is not).
        let mut w = world(crate::Rules::default());
        w.score = [20, 18];
        assert!(w.basket_would_win(0, 2, 0));
        w.score = [20, 20];
        assert!(!w.basket_would_win(0, 2, 0));
        // A Level 1 inside takes one: 20-20 becomes 21-19.
        assert!(w.basket_would_win(0, 2, 1));
        // league scoring: an inside basket is two.
        let mut w = world(crate::Rules::mode(crate::Mode::LeagueScoring));
        w.score = [19, 17];
        assert!(w.basket_would_win(0, 2, 0));
        // Dunks Only: a jumper scores nothing.
        let mut w = world(crate::Rules::mode(crate::Mode::DunksOnly));
        w.score = [20, 18];
        assert!(!w.basket_would_win(0, 2, 0));
        // The trick-point game: past the target and the other side.
        let mut w = world(crate::Rules::mode(crate::Mode::TrickPointChallenge));
        w.stats.add_team(0, crate::stats::TRICK_POINTS, 100_000);
        assert!(!w.basket_would_win(0, 0, 0));
        w.stats.add_team(0, crate::stats::TRICK_POINTS, 1);
        assert!(w.basket_would_win(0, 0, 0));
        w.stats.add_team(1, crate::stats::TRICK_POINTS, 100_001);
        assert!(!w.basket_would_win(0, 0, 0));
        // Gamebreaker Wins to a Level 2: a Level 1 does not win it.
        let mut rules = crate::Rules::mode(crate::Mode::GamebreakerWins);
        rules.goal = Goal::Gamebreaker(2);
        let w = world(rules);
        assert!(!w.basket_would_win(0, 2, 1));
        assert!(w.basket_would_win(0, 2, 2));
        assert!(!w.basket_would_win(0, 2, 0));
        // A lesson never.
        let mut w = world(crate::Rules::default());
        w.lesson = Some(3);
        w.score = [20, 0];
        assert!(!w.basket_would_win(0, 2, 0));
    }
}
