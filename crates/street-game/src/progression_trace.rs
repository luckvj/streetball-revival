//! Pure translations of the original career star/nickname algorithms.
//!
//! Evidence: `FUN_001e70e0`, `FUN_001d83c8`, `FUN_001d8440`,
//! `FUN_001d7f48` and `FUN_001d2070` in SLUS-20651. Random values are
//! supplied by the caller; this does not invent an original RNG seed.

use street_assets::db::{Nickname, Skill};

/// `FUN_003b42a0`: the 32-bit LCG stores its full next state and returns
/// its lower 31 bits. The original global seed/other consumers are not
/// reconstructed by this local generator.
pub struct NativeRandom(u32);

impl NativeRandom {
    pub fn new(state: u32) -> Self {
        Self(state)
    }

    pub fn draw(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.0 & 0x7fff_ffff
    }
}

/// Field 15 stores `tier * 11 + progress`. Tier 5 is the terminal tier;
/// imported terminal records may contain a remainder and are left intact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stars {
    packed: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Announcement {
    Tier { tier: u8, overall_points: u16 },
    Progress { gained: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StarChange {
    pub stars: Stars,
    pub announcement: Option<Announcement>,
    pub nickname_due: bool,
}

impl Stars {
    pub fn from_packed(packed: i16) -> Option<Self> {
        (0..=65).contains(&packed).then_some(Self { packed })
    }

    pub const fn packed(self) -> i16 {
        self.packed
    }

    pub const fn tier(self) -> u8 {
        (self.packed / 11) as u8
    }

    pub const fn progress(self) -> u8 {
        (self.packed % 11) as u8
    }

    /// `FUN_001e70e0`: loss drains the current tier's progress without
    /// demotion; gain crosses at most one tier and discards overflow.
    /// Nickname assignment is checked whenever the resulting tier is 3,
    /// including an update that did not change the progress.
    pub fn advance(self, delta: i16, nickname_unset: bool) -> StarChange {
        let tier = self.tier();
        if tier >= 5 {
            return StarChange {
                stars: self,
                announcement: None,
                nickname_due: false,
            };
        }
        let sum = i16::from(self.progress()).wrapping_add(delta);
        let base = i16::from(tier) * 11;
        let packed = if sum < 0 {
            base
        } else if sum < 12 {
            self.packed.wrapping_add(delta)
        } else {
            base + 11
        };
        let stars = Self { packed };
        let announcement = if stars.tier() > tier {
            // .rodata 0x460a78 = {0,10,10,10,10,10}.
            Some(Announcement::Tier {
                tier: stars.tier(),
                overall_points: 10,
            })
        } else if stars.progress() != self.progress() {
            Some(Announcement::Progress { gained: delta > 0 })
        } else {
            None
        };
        StarChange {
            stars,
            announcement,
            nickname_due: stars.tier() == 3 && nickname_unset,
        }
    }
}

/// `FUN_001d7f48` compares the seven skill bars, rather than raw ratings.
pub fn skill_bar(skill: Skill, rating: i16) -> u8 {
    const SHOTS: [i16; 7] = [9, 30, 50, 70, 80, 90, 98];
    const OTHERS: [i16; 7] = [0, 20, 40, 60, 80, 90, 98];
    let levels = if skill == Skill::Shots { SHOTS } else { OTHERS };
    let rating = rating.clamp(0, 99);
    levels
        .iter()
        .position(|&threshold| rating <= threshold)
        .unwrap_or(6) as u8
}

/// Strict comparison preserves the first skill on equal bars. These are
/// the stack's seven values at `FUN_001e70e0` after the tier-3 gate.
pub fn nickname_skill(ratings: &[i16; 11]) -> Skill {
    let priority = [
        Skill::Dunks,
        Skill::Handles,
        Skill::Shots,
        Skill::Steals,
        Skill::Blocks,
        Skill::Rebounds,
        Skill::Power,
    ];
    let mut best = priority[0];
    for skill in priority.into_iter().skip(1) {
        if skill_bar(skill, ratings[skill.rating()]) > skill_bar(best, ratings[best.rating()]) {
            best = skill;
        }
    }
    best
}

/// `FUN_001d2070` gathers the first contiguous run for the chosen skill,
/// forward-shuffles it, then draws one row. The returned value is the
/// database **row index**, as `FUN_001dac10` reads table object +0x1c.
/// The separate nickname id field is not stored in player field 6.
pub fn nickname_row(
    names: &[Nickname],
    skill: Skill,
    mut random: impl FnMut() -> u32,
) -> Option<usize> {
    let first = names.iter().position(|name| name.skill == Some(skill))?;
    let mut rows: Vec<usize> = (first..names.len())
        .take_while(|&row| names[row].skill == Some(skill))
        .collect();
    for index in 1..rows.len() {
        let other = random() as usize % (index + 1);
        rows.swap(index, other);
    }
    Some(rows[random() as usize % rows.len()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_random_keeps_full_state_and_masks_only_the_return_value() {
        let mut random = NativeRandom::new(1);
        assert_eq!(random.draw(), 1_103_527_590);
        assert_eq!(random.draw(), 377_401_575);
        assert_eq!(random.0, 2_524_885_223);
    }

    #[test]
    fn losing_cannot_demote_and_gaining_can_advance_only_one_tier() {
        let stars = Stars::from_packed(25).unwrap();
        assert_eq!(stars.tier(), 2);
        assert_eq!(stars.progress(), 3);
        let loss = stars.advance(-5, true);
        assert_eq!(loss.stars.packed(), 22);
        assert_eq!(
            loss.announcement,
            Some(Announcement::Progress { gained: false })
        );
        let gain = stars.advance(40, true);
        assert_eq!(gain.stars.packed(), 33);
        assert_eq!(
            gain.announcement,
            Some(Announcement::Tier {
                tier: 3,
                overall_points: 10
            })
        );
        assert!(gain.nickname_due);
        assert!(!gain.stars.advance(0, false).nickname_due);
        assert!(gain.stars.advance(0, true).nickname_due);
    }

    #[test]
    fn terminal_stars_do_not_change_and_invalid_packed_values_are_rejected() {
        for packed in 55..=65 {
            let stars = Stars::from_packed(packed).unwrap();
            assert_eq!(stars.advance(-5, true).stars, stars);
            assert_eq!(stars.advance(5, true).announcement, None);
        }
        assert!(Stars::from_packed(-1).is_none());
        assert!(Stars::from_packed(66).is_none());
    }

    #[test]
    fn nickname_ties_compare_bars_and_preserve_original_priority() {
        let mut ratings = [0; 11];
        ratings[2] = 91; // Dunks, bar 6.
        ratings[7] = 99; // Handles, still bar 6.
        assert_eq!(nickname_skill(&ratings), Skill::Dunks);
        ratings[2] = 90;
        assert_eq!(nickname_skill(&ratings), Skill::Handles);
        assert_eq!(skill_bar(Skill::Shots, 9), 0);
        assert_eq!(skill_bar(Skill::Dunks, 9), 1);
    }

    #[test]
    fn nickname_draw_uses_the_contiguous_run_row_numbers_and_full_rng_sequence() {
        let names: Vec<_> = [
            Skill::Shots,
            Skill::Dunks,
            Skill::Dunks,
            Skill::Dunks,
            Skill::Shots,
            Skill::Dunks,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, skill)| Nickname {
            id: 100 + index as i16,
            name: String::new(),
            skill: Some(skill),
        })
        .collect();
        let mut draws = [0, 1, 1].into_iter();
        // [1,2,3] -> [2,1,3] -> [2,3,1], final draw selects row 3.
        assert_eq!(
            nickname_row(&names, Skill::Dunks, || draws.next().unwrap()),
            Some(3)
        );
        assert_eq!(draws.next(), None);
        assert_eq!(
            nickname_row(&names, Skill::Power, || panic!("no draw for absent skill")),
            None
        );
    }
}
