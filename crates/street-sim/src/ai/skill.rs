//! How good the computer is at a thing: four numbers blended by the
//! difficulty and by how far behind the computer's side is
//! (`FUN_00140a30`, rounded by `FUN_001409c8`).
//!
//! The four numbers are an Easy pair `(a, b)` and a Hard pair `(c, d)`;
//! the first of each is used level or ahead, the second when far behind.
//! The difficulty picks the mix of the two pairs (`FUN_00143458`'s
//! weights): Easy `a`, Medium `0.46875 a + 0.53125 c`, Hard
//! `0.1875 a + 0.8125 c` (and the same for `b`, `d`). The "behind" weight
//! `k / 6` is the game mode's catch-up table 0 (the game's virtual +0x44
//! called as `(0, side)`), 0..6. The difficulty only applies when the
//! other side has a person playing (`FUN_00140998`); otherwise the mix is
//! Medium's. In the story modes (the game record's mode 4, the league
//! Challenge, or 5, Be A Legend) the final weight is the run's own
//! ([`crate::World::scenario`]), the inner mixes still the difficulty's
//! (`0x140b90..`).

use super::Ctx;
use crate::{Difficulty, Goal};

/// Table 0 of the points game's catch-up group (`0x3dc0e0`, group 1 of
/// `FUN_00186638`): 31 rows (the lead −15..15) × 3 stages.
const POINTS: [u8; 93] = [
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 5, 6,
    6, 5, 6, 6, 4, 5, 6, 4, 5, 6, 3, 4, 5, 3, 4, 4, 3, 3, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 1, 2, 2, 1, 2, 2, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Table 0 of the trick-point and Gamebreaker Wins group (`0x3dc310`,
/// group 2; `FUN_0018c788`, `FUN_00145b48`).
const MODES: [u8; 93] = [
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 5, 6, 6, 5, 5, 6, 5, 5, 6, 5, 5, 6, 4, 5, 6, 4, 5,
    6, 4, 4, 6, 4, 4, 6, 4, 4, 6, 3, 4, 5, 3, 3, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 1, 2, 2, 1, 2, 2, 1, 2, 2, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Table 1 of the points game's group (`0x3dc13d`), read by the
/// playbook's start: the percent chance of playing for the meter. The
/// trick-point and Gamebreaker Wins group's table 1 (`0x3dc36d`) is all
/// zeros.
const POINTS_1: [u8; 93] = [
    95, 99, 99, 95, 95, 99, 95, 95, 99, 95, 95, 99, 90, 95, 99, 80, 95, 99, 70, 95, 99, 60, 90, 99,
    50, 80, 99, 40, 70, 99, 30, 60, 99, 20, 50, 90, 0, 40, 80, 0, 10, 70, 0, 0, 60, 0, 0, 50, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// The game's vt+0x44 called as `(1, side)` (`FUN_0012eac0`): table 1's
/// cell for `team`.
pub(super) fn table1(ctx: &Ctx, team: usize) -> i32 {
    let Some((lead, stage)) = crate::shooting::catch_up_cell(ctx.w, team) else {
        return 0;
    };
    let row = (lead.clamp(-15, 15) + 15) as usize;
    match ctx.w.rules.goal {
        Goal::Points(_) => i32::from(POINTS_1[row * 3 + stage.min(2)]),
        _ => 0,
    }
}

/// How far behind `team` is, 0..6 (table 0's cell).
pub(super) fn behind(ctx: &Ctx, team: usize) -> i32 {
    let Some((lead, stage)) = crate::shooting::catch_up_cell(ctx.w, team) else {
        return 0;
    };
    let row = (lead.clamp(-15, 15) + 15) as usize;
    let table = match ctx.w.rules.goal {
        Goal::Points(_) => &POINTS,
        _ => &MODES,
    };
    i32::from(table[row * 3 + stage.min(2)])
}

/// `FUN_00140998`: the difficulty that applies to `team` (0 Easy, 1
/// Medium, 2 Hard), Medium unless the other side has a person.
pub(super) fn difficulty(ctx: &Ctx, team: usize) -> u8 {
    if ctx.human[1 - team.min(1)] {
        match ctx.w.rules.difficulty {
            Difficulty::Easy => 0,
            Difficulty::Medium => 1,
            Difficulty::Hard => 2,
        }
    } else {
        1
    }
}

/// `FUN_00140a30`'s blend for a difficulty and a behind step `k`, in the
/// original's order of operations; `scenario` the story modes' weight.
pub(super) fn blend(
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    difficulty: u8,
    k: i32,
    scenario: Option<f32>,
) -> f32 {
    // (w, f25, f26, f27, f28): w mixes the Easy pair (f26 level, f25
    // behind) with the Hard pair (f28, f27).
    let (w, f25, f26, f27, f28) = match difficulty {
        0 => (0.0, b, a, b * 0.75 + d * 0.25, a * 0.75 + c * 0.25),
        2 => (0.5, b * 0.375 + d * 0.625, a * 0.375 + c * 0.625, d, c),
        _ => (
            0.5,
            b * 0.6875 + d * 0.3125,
            a * 0.6875 + c * 0.3125,
            b * 0.25 + d * 0.75,
            a * 0.25 + c * 0.75,
        ),
    };
    let w = scenario.unwrap_or(w);
    let behind = (1.0 - w) * f25 + w * f27;
    let level = (1.0 - w) * f26 + w * f28;
    let k = k as f32 / 6.0;
    level * (1.0 - k) + behind * k
}

/// `FUN_0017dda8`: the blended value for `team`.
pub(super) fn value(ctx: &Ctx, team: usize, a: f32, b: f32, c: f32, d: f32) -> f32 {
    blend(
        a,
        b,
        c,
        d,
        difficulty(ctx, team),
        behind(ctx, team),
        ctx.w.scenario,
    )
}

/// `FUN_0017dd68` / `FUN_001409c8`: the blended value of four integers,
/// rounded half up (`cvt.w.s` of `x + 0.5`, which truncates).
pub(super) fn percent(ctx: &Ctx, team: usize, a: i32, b: i32, c: i32, d: i32) -> i32 {
    (value(ctx, team, a as f32, b as f32, c as f32, d as f32) + 0.5) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_difficulty_mixes_the_easy_and_hard_pairs() {
        // Level (k = 0): Easy is a, Medium 0.46875a + 0.53125c, Hard
        // 0.1875a + 0.8125c.
        assert_eq!(blend(20.0, 10.0, 0.0, 0.0, 0, 0, None), 20.0);
        assert!((blend(20.0, 10.0, 0.0, 0.0, 1, 0, None) - 9.375).abs() < 1e-5);
        assert!((blend(20.0, 10.0, 0.0, 0.0, 2, 0, None) - 3.75).abs() < 1e-5);
        // Far behind (k = 6) uses the second of each pair.
        assert_eq!(blend(20.0, 10.0, 0.0, 0.0, 0, 6, None), 10.0);
        assert!((blend(0.0, 50.0, 100.0, 100.0, 1, 6, None) - 76.5625).abs() < 1e-4);
    }

    /// In the story modes the weight is the run's (`0x140b90..`): Easy's
    /// inner pairs (the first values, and three quarters of them with a
    /// quarter of the Hard ones) mixed by it; Medium's and Hard's likewise.
    #[test]
    fn the_story_modes_mix_by_the_runs_weight() {
        // Easy level: (1 - w) a + w (0.75 a + 0.25 c).
        let got = blend(20.0, 10.0, 60.0, 30.0, 0, 0, Some(0.4));
        assert!(
            (got - (0.6 * 20.0 + 0.4 * (15.0 + 15.0))).abs() < 1e-4,
            "{got}"
        );
        // Hard far behind: (1 - w)(0.375 b + 0.625 d) + w d.
        let got = blend(20.0, 10.0, 60.0, 30.0, 2, 6, Some(1.0));
        assert!((got - 30.0).abs() < 1e-4, "{got}");
        // No story: Medium's 0.5.
        assert_eq!(
            blend(20.0, 10.0, 60.0, 30.0, 1, 0, None),
            blend(20.0, 10.0, 60.0, 30.0, 1, 0, Some(0.5))
        );
    }

    #[test]
    fn the_catch_up_tables_are_the_discs() {
        // Fifteen behind at any stage is the most help; level is 3 early.
        assert_eq!(POINTS[0], 6);
        assert_eq!(POINTS[15 * 3], 3);
        assert_eq!(POINTS[30 * 3 + 2], 0);
        assert_eq!(MODES[15 * 3 + 2], 4);
    }
}
