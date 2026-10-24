//! Gamebreaker fill tuning (`FUN_00143248`, `FUN_00143458`).
//! These scalars affect meter gains, never player ratings. The normal
//! singleton mode is zero (`FUN_00131260`), hence its multiplier is 1.
use crate::Difficulty;

/// `FUN_00143d38`'s Easy comeback cap, the raw word `0x47260400` (42,500)
/// built at `0x143e54..0x143e58`, compared at `0x143e64` and stored at
/// `0x143f14`.
pub const EASY_CPU_CAP: f32 = 42_500.0;

/// The executable's final blend, including the outer scenario weight.
pub fn blend(difficulty: Difficulty, first: f32, second: f32, scenario: Option<f32>) -> f32 {
    let (human, cpu, default_weight) = match difficulty {
        Difficulty::Easy => (first, first * 0.75 + second * 0.25, 0.0),
        Difficulty::Medium => (
            first * 0.6875 + second * 0.3125,
            first * 0.25 + second * 0.75,
            0.5,
        ),
        Difficulty::Hard => (first * 0.375 + second * 0.625, second, 0.5),
    };
    let weight = scenario.unwrap_or(default_weight).clamp(0.0, 1.0);
    (1.0 - weight) * human + weight * cpu
}

/// `.rodata 0x43e298`: two endpoints for a controlled crew, then the CPU.
/// With no controlled crew the constructor averages the pair directly.
pub fn fill(difficulty: Difficulty, controlled: [bool; 2], scenario: Option<f32>) -> [f32; 2] {
    controlled.map(|human| {
        let (first, second) = if human { (0.9, 0.7) } else { (0.7, 0.95) };
        if controlled.iter().any(|&c| c) {
            blend(difficulty, first, second, scenario)
        } else {
            (first + second) * 0.5
        }
    })
}

/// Sequence-specific row at `0x43e2a8`, stored at meter object +0x48.
pub fn sequence_fill(
    difficulty: Difficulty,
    controlled: [bool; 2],
    scenario: Option<f32>,
) -> [f32; 2] {
    controlled.map(|human| {
        let (first, second) = if human { (0.8, 0.7) } else { (0.7, 0.8) };
        if controlled.iter().any(|&c| c) {
            blend(difficulty, first, second, scenario)
        } else {
            (first + second) * 0.5
        }
    })
}

/// Region progress table at `0x43df08`. The current opponent counts even
/// before its first win. Completed regions use 1, or .71 on Medium.
pub fn league_weight(difficulty: Difficulty, completed: bool, opponents: usize) -> f32 {
    if completed {
        if difficulty == Difficulty::Medium {
            0.71
        } else {
            1.0
        }
    } else {
        [0.0, 0.25, 0.5, 0.6, 0.65, 0.7][opponents.min(5)]
    }
}

/// Be A Legend outer blend in `FUN_00143458`; the first component is
/// the created star's packed field 15 divided by 11 (`FUN_001d83c8`).
pub fn legend_weight(
    star_tier: u8,
    event_level: u8,
    round: usize,
    rounds: usize,
    special_team: bool,
) -> f32 {
    (star_tier as f32 / 6.0 * 0.25
        + event_level as f32 / 6.0 * 0.5
        + if rounds == 0 {
            0.0
        } else {
            round as f32 / rounds as f32 * 0.25
        }
        + if special_team { 0.25 } else { 0.0 })
    .min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_outer_mix_is_included_and_cpu_matches_average() {
        let pairs = [
            (Difficulty::Easy, [0.9, 0.7]),
            (Difficulty::Medium, [0.79375, 0.8328125]),
            (Difficulty::Hard, [0.7375, 0.903125]),
        ];
        for (d, expected) in pairs {
            for (got, wanted) in fill(d, [true, false], None).into_iter().zip(expected) {
                assert!((got - wanted).abs() < 0.000001);
            }
            assert!((fill(d, [false, false], None)[0] - 0.825).abs() < 0.000001);
        }
        assert!((blend(Difficulty::Medium, 0.9, 0.7, Some(1.0)) - 0.75).abs() < 0.000001);
    }

    #[test]
    fn scaled_gains_and_drains_preserve_fractional_meter_points() {
        let mut w = crate::World::new(crate::tests::hoops(), vec![], crate::Rules::default(), 1);
        w.configure_meter_fill([true, false], None);
        w.award(0, 1, false);
        w.award(0, 1, false);
        assert_eq!(w.teams[0].meter, 1);
        assert!(w.teams[0].meter_fraction > 0.0);
        w.drain(0, 1);
        assert_eq!(w.teams[0].meter, 0);
        assert!((w.teams[0].meter_fraction - w.teams[0].meter_fill).abs() < 0.000001);
        w.drain(0, 2);
        assert_eq!(w.teams[0].meter_fraction, 0.0);
    }

    #[test]
    fn easy_cpu_cap_depends_on_opponent_and_score_difference() {
        let rules = crate::Rules {
            difficulty: Difficulty::Easy,
            ..Default::default()
        };
        let mut w = crate::World::new(crate::tests::hoops(), vec![], rules, 1);
        w.configure_meter_fill([true, false], None);
        w.teams[1].meter = 42_000;
        w.award(1, 10_000, false);
        assert_eq!(w.teams[1].meter, 42_500);
        w.award(1, 10_000, false);
        assert_eq!(w.teams[1].meter, 42_500);
        w.score[0] = 3;
        w.award(1, 10_000, false);
        assert_eq!(w.teams[1].meter, 49_500);
        w.award(1, 10_000, false);
        assert!(w.teams[1].meter_full());
        w.configure_meter_fill([false, false], None);
        assert!(!w.teams[1].meter_cpu_limit);
    }

    #[test]
    fn scenario_weights_follow_original_progress_branches() {
        assert_eq!(league_weight(Difficulty::Easy, false, 1), 0.25);
        assert_eq!(league_weight(Difficulty::Hard, false, 5), 0.7);
        assert_eq!(league_weight(Difficulty::Medium, true, 5), 0.71);
        assert_eq!(league_weight(Difficulty::Easy, true, 5), 1.0);
        assert_eq!(legend_weight(0, 0, 0, 0, false), 0.0);
        assert_eq!(legend_weight(6, 6, 2, 4, true), 1.0);
    }
}
