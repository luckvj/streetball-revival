//! Original user-setup cheat entry (`FUN_002453e8`, PS2 SLUS-20651).
//!
//! `FUN_00245b88` enables entry while L1 is held in widget states 11/12.
//! The checker packs four face-button values into a big-endian u32; failed
//! attempts retain the last three presses, while successful attempts clear
//! the history. This is separate from the reward card's field-5 reference.
//! Actions' names/ids are registered at `0x2e8fec..0x2e90f8` in the ELF.
//! Gameplay writes target the singleton returned by `FUN_00131208`.

use bevy::prelude::Resource;
use street_sim::{Ratings, Rules};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Circle,
    Square,
    Triangle,
    Cross,
}

impl Button {
    pub const fn value(self) -> u8 {
        match self {
            Self::Circle => 1,
            Self::Square => 2,
            Self::Triangle => 4,
            Self::Cross => 8,
        }
    }

    pub const fn action(self) -> u16 {
        match self {
            Self::Triangle => 0x29,
            Self::Circle => 0x2a,
            Self::Square => 0x2b,
            Self::Cross => 0x2c,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Circle => "Circle",
            Self::Square => "Square",
            Self::Triangle => "Triangle",
            Self::Cross => "Cross",
        }
    }

    pub const fn from_action(action: u16) -> Option<Self> {
        match action {
            0x29 => Some(Self::Triangle),
            0x2a => Some(Self::Circle),
            0x2b => Some(Self::Square),
            0x2c => Some(Self::Cross),
            _ => None,
        }
    }
}

/// The twelve pictured code cards and five hidden unlock codes, in the
/// original checker/card order (card ids 96..112).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Cheat {
    StreetKids,
    NoCounters,
    Gb2TrailsAlwaysOn,
    AllQuicks,
    UnlimitedTurbo,
    Easy2Pointers,
    Hard2Pointers,
    BigHeads,
    WomensBall,
    AlternateBall,
    NoHud,
    ExplosiveRims,
    UnlockCourts,
    UnlockStreetLegends,
    UnlockLeagueLegends,
    UnlockJerseys,
    UnlockMichaelJordan,
}

pub const ALL: [Cheat; 17] = [
    Cheat::StreetKids,
    Cheat::NoCounters,
    Cheat::Gb2TrailsAlwaysOn,
    Cheat::AllQuicks,
    Cheat::UnlimitedTurbo,
    Cheat::Easy2Pointers,
    Cheat::Hard2Pointers,
    Cheat::BigHeads,
    Cheat::WomensBall,
    Cheat::AlternateBall,
    Cheat::NoHud,
    Cheat::ExplosiveRims,
    Cheat::UnlockCourts,
    Cheat::UnlockStreetLegends,
    Cheat::UnlockLeagueLegends,
    Cheat::UnlockJerseys,
    Cheat::UnlockMichaelJordan,
];

impl Cheat {
    pub const fn card_id(self) -> u16 {
        96 + self as u16
    }

    pub const fn common_index(self) -> Option<usize> {
        if (self as u8) < 12 {
            Some(27 + self as usize)
        } else {
            None
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::StreetKids => "Street Kids",
            Self::NoCounters => "No Counters",
            Self::Gb2TrailsAlwaysOn => "GB2 Trails Always On",
            Self::AllQuicks => "All Quicks",
            Self::UnlimitedTurbo => "Unlimited Turbo",
            Self::Easy2Pointers => "Easy 2 Pointers",
            Self::Hard2Pointers => "Hard 2 Pointers",
            Self::BigHeads => "Big Heads",
            Self::WomensBall => "Women's Ball",
            Self::AlternateBall => "Alternate Ball",
            Self::NoHud => "No Hud",
            Self::ExplosiveRims => "Explosive Rims",
            Self::UnlockCourts => "Unlock Courts",
            Self::UnlockStreetLegends => "Unlock Street Legends",
            Self::UnlockLeagueLegends => "Unlock Legends",
            Self::UnlockJerseys => "Unlock Jerseys",
            Self::UnlockMichaelJordan => "Unlock Michael Jordan",
        }
    }

    pub const fn packed_code(self) -> u32 {
        match self {
            Self::StreetKids => 0x0404_0102,
            Self::NoCounters => 0x0404_0101,
            Self::Gb2TrailsAlwaysOn => 0x0404_0402,
            Self::AllQuicks => 0x0401_0402,
            Self::UnlimitedTurbo => 0x0202_0404,
            Self::Easy2Pointers => 0x0401_0204,
            Self::Hard2Pointers => 0x0402_0104,
            Self::BigHeads => 0x0102_0201,
            Self::WomensBall => 0x0104_0401,
            Self::AlternateBall => 0x0102_0102,
            Self::NoHud => 0x0201_0101,
            Self::ExplosiveRims => 0x0101_0104,
            Self::UnlockCourts => 0x0204_0402,
            Self::UnlockStreetLegends => 0x0204_0104,
            Self::UnlockLeagueLegends => 0x0204_0401,
            Self::UnlockJerseys => 0x0204_0101,
            Self::UnlockMichaelJordan => 0x0204_0202,
        }
    }

    pub fn button_sequence(self) -> [Button; 4] {
        self.packed_code().to_be_bytes().map(|value| match value {
            1 => Button::Circle,
            2 => Button::Square,
            4 => Button::Triangle,
            8 => Button::Cross,
            _ => unreachable!("the traced codes contain only face-button values"),
        })
    }

    pub fn from_packed(code: u32) -> Option<Self> {
        ALL.into_iter().find(|cheat| cheat.packed_code() == code)
    }

    /// Exact singleton offset/value writes made by the checker. These
    /// integers are modes; they do not establish physical scale factors.
    pub const fn tuning_writes(self) -> &'static [(usize, u32)] {
        match self {
            Self::StreetKids => &[(0x30, 1)],
            Self::NoCounters => &[(0x78, 1)],
            Self::Gb2TrailsAlwaysOn => &[(0x18, 1)],
            Self::AllQuicks => &[(0x40, 2)],
            Self::UnlimitedTurbo => &[(0x6c, 1)],
            Self::Easy2Pointers => &[(0x4c, 2)],
            Self::Hard2Pointers => &[(0x4c, 1)],
            Self::BigHeads => &[(0x34, 2)],
            Self::WomensBall => &[(0x00, 1), (0x10, 3)],
            Self::AlternateBall => &[(0x10, 2)],
            Self::NoHud => &[(0x28, 1)],
            Self::ExplosiveRims => &[(0x14, 1)],
            Self::UnlockCourts
            | Self::UnlockStreetLegends
            | Self::UnlockLeagueLegends
            | Self::UnlockJerseys
            | Self::UnlockMichaelJordan => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
    Accepted(Cheat),
    Rejected,
}

impl Feedback {
    /// FE event supplied to `FUN_001a1908` (success 9, failure 6).
    pub const fn sound_event(self) -> u16 {
        match self {
            Self::Accepted(_) => 9,
            Self::Rejected => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntryResult {
    pub consumed: bool,
    pub feedback: Option<Feedback>,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct EntryState {
    held: bool,
    packed: u32,
    length: u8,
    since_feedback: u8,
}

impl EntryState {
    pub fn is_held(&self) -> bool {
        self.held
    }

    pub fn reset(&mut self) {
        self.packed = 0;
        self.length = 0;
        self.since_feedback = 0;
    }

    /// Called only for the original widget's ready/waiting states 11/12.
    /// `mode` is game-record +0x78; mode 1 rejects all cheat input.
    pub fn handle_action(&mut self, mode: u32, action: u16) -> EntryResult {
        if action == 0x31 {
            self.held = true;
            self.reset();
        } else if action == 0x35 {
            self.held = false;
            self.reset();
            return EntryResult {
                consumed: true,
                feedback: None,
            };
        }
        if !self.held || mode == 1 {
            return EntryResult::default();
        }
        let Some(button) = Button::from_action(action) else {
            return EntryResult {
                consumed: matches!(action, 0x23 | 0x25 | 0x27),
                feedback: None,
            };
        };
        self.packed = (self.packed << 8) | u32::from(button.value());
        self.length += 1;
        self.since_feedback += 1;
        if self.length < 4 {
            return EntryResult {
                consumed: true,
                feedback: None,
            };
        }
        self.length = 3;
        if let Some(cheat) = Cheat::from_packed(self.packed) {
            self.reset();
            return EntryResult {
                consumed: true,
                feedback: Some(Feedback::Accepted(cheat)),
            };
        }
        let feedback = if self.since_feedback == 4 {
            self.since_feedback = 0;
            Some(Feedback::Rejected)
        } else {
            None
        };
        EntryResult {
            consumed: true,
            feedback,
        }
    }
}

/// Original cheat selection modes, retained until game setup resets them.
/// Only selectors are represented here; their tuning/rendering consumers
/// must supply the independently traced effects.
#[derive(Clone, Debug, Resource)]
pub struct Settings {
    modes: [u32; 0x8c / 4],
}

impl Default for Settings {
    fn default() -> Self {
        // FUN_00131260 clears all 35 words, including offset 0x88.
        Self { modes: [0; 35] }
    }
}

impl Settings {
    pub const BYTE_LEN: usize = 0x8c;

    /// Little-endian integer selectors for lobby agreement. This is a Rust
    /// network format, not an original memory-card representation.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.modes
            .iter()
            .flat_map(|mode| mode.to_le_bytes())
            .collect()
    }

    /// Accept only selectors supported by the traced cheat entry path.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::BYTE_LEN {
            return None;
        }
        let mut settings = Self::default();
        for (index, chunk) in bytes.as_chunks::<4>().0.iter().enumerate() {
            let value = u32::from_le_bytes(*chunk);
            let allowed = match index * 4 {
                0x00 | 0x14 | 0x18 | 0x28 | 0x30 | 0x6c | 0x78 => value <= 1,
                0x10 => value <= 3,
                0x34 | 0x4c => value <= 2,
                0x40 => matches!(value, 0 | 2),
                _ => value == 0,
            };
            if !allowed {
                return None;
            }
            settings.modes[index] = value;
        }
        Some(settings)
    }

    pub fn reset(&mut self) {
        self.modes.fill(0);
    }

    pub fn mode(&self, byte_offset: usize) -> Option<u32> {
        if byte_offset.is_multiple_of(4) {
            self.modes.get(byte_offset / 4).copied()
        } else {
            None
        }
    }

    pub fn apply(&mut self, cheat: Cheat) {
        for &(offset, value) in cheat.tuning_writes() {
            self.modes[offset / 4] = value;
        }
    }

    /// `FUN_003153d0`: Street Kids halves the model's height and width.
    pub fn body_scale(&self) -> f32 {
        if self.modes[0x30 / 4] == 1 { 0.5 } else { 1.0 }
    }

    /// `FUN_003153d0`: mode 2 multiplies the head's existing scale by 1.5.
    pub fn head_scale(&self) -> f32 {
        match self.modes[0x34 / 4] {
            1 => 0.8,
            2 => 1.5,
            _ => 1.0,
        }
    }

    /// `FUN_00316a28` chooses `bal{suffix}.o/.ssh`: the normal 'a',
    /// Alternate Ball 'e' and women's Ball 'f'. Story's special court/team
    /// override must still be applied by the caller.
    pub fn ball_model_suffix(&self) -> char {
        const MODELS: &[u8] = b"aefgkhmija";
        let selector = self.modes[0x10 / 4];
        selector
            .checked_sub(1)
            .and_then(|i| MODELS.get(i as usize))
            .map(|&value| char::from(value))
            .unwrap_or('a')
    }

    /// `FUN_0014a300`, table 0x43edc8: the model scale is 1.25 normally
    /// and 1.2 for women's Ball. The same selector persists if Alternate
    /// Ball is subsequently entered, exactly as the checker writes it.
    pub fn ball_scale(&self) -> f32 {
        if self.modes[0] == 1 { 1.2 } else { 1.25 }
    }

    /// The native physics ball radius is selected scale * 0.5 * 0.75.
    pub fn ball_radius(&self) -> f32 {
        self.ball_scale() * 0.5 * 0.75
    }

    pub fn no_hud(&self) -> bool {
        self.modes[0x28 / 4] == 1
    }

    pub fn gb2_trails_always_on(&self) -> bool {
        self.modes[0x18 / 4] == 1
    }

    pub fn explosive_rims(&self) -> bool {
        self.modes[0x14 / 4] == 1
    }

    /// `FUN_001d7d00`, tables 0x45fe20/0x45fe50: All Quicks changes
    /// Speed to 99; Easy/Hard 2 Pointers change Threes to 99/0. Ratings
    /// here normalize the original 0..99 range to 0..1.
    pub fn apply_ratings(&self, ratings: &mut Ratings) {
        if self.modes[0x40 / 4] == 2 {
            ratings.speed = 1.0;
        }
        match self.modes[0x4c / 4] {
            1 => ratings.threes = 0.0,
            2 => ratings.threes = 1.0,
            _ => {}
        }
    }

    /// Copy the gameplay selectors supported by the simulation.
    pub fn apply_rules(&self, rules: &mut Rules) {
        rules.unlimited_turbo |= self.modes[0x6c / 4] == 1;
        rules.no_counters |= self.modes[0x78 / 4] == 1;
        if self.modes[0x4c / 4] != 0 {
            rules.easy_2_pointers = self.modes[0x4c / 4] == 2;
            rules.hard_2_pointers = self.modes[0x4c / 4] == 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_seventeen_traced_codes_recognize_in_card_order() {
        let mut entry = EntryState::default();
        for (index, cheat) in ALL.into_iter().enumerate() {
            assert_eq!(cheat.card_id(), 96 + index as u16);
            entry.handle_action(2, 0x31);
            let mut result = EntryResult::default();
            for button in cheat.button_sequence() {
                result = entry.handle_action(2, button.action());
            }
            assert_eq!(result.feedback, Some(Feedback::Accepted(cheat)));
            assert!(result.consumed);
            assert_eq!(entry.length, 0);
            assert_eq!(entry.packed, 0);
        }
    }

    #[test]
    fn failed_input_keeps_a_rolling_window_but_success_clears_it() {
        let mut entry = EntryState::default();
        entry.handle_action(2, 0x31);
        for button in [Button::Cross, Button::Triangle, Button::Triangle] {
            assert_eq!(entry.handle_action(2, button.action()).feedback, None);
        }
        assert_eq!(
            entry.handle_action(2, Button::Circle.action()).feedback,
            Some(Feedback::Rejected)
        );
        assert_eq!(
            entry.handle_action(2, Button::Circle.action()).feedback,
            Some(Feedback::Accepted(Cheat::NoCounters))
        );
        assert_eq!(entry.length, 0);
        assert_eq!(entry.since_feedback, 0);
    }

    #[test]
    fn mode_one_and_released_l1_prevent_entry() {
        let mut entry = EntryState::default();
        entry.handle_action(1, 0x31);
        for button in Cheat::NoCounters.button_sequence() {
            assert!(!entry.handle_action(1, button.action()).consumed);
        }
        assert_eq!(entry.length, 0);
        entry.handle_action(2, 0x35);
        assert!(!entry.is_held());
        assert!(!entry.handle_action(2, Button::Circle.action()).consumed);
    }

    #[test]
    fn ignored_actions_do_not_advance_history_and_bad_groups_sound_every_four() {
        let mut entry = EntryState::default();
        entry.handle_action(2, 0x31);
        for index in 1..=12 {
            assert!(!entry.handle_action(2, 0x30).consumed);
            let result = entry.handle_action(2, Button::Cross.action());
            assert_eq!(
                result.feedback,
                (index % 4 == 0).then_some(Feedback::Rejected)
            );
        }
    }

    #[test]
    fn shared_modes_replace_one_another_without_changing_unrelated_settings() {
        let mut settings = Settings::default();
        settings.apply(Cheat::NoCounters);
        settings.apply(Cheat::Easy2Pointers);
        settings.apply(Cheat::Hard2Pointers);
        settings.apply(Cheat::WomensBall);
        settings.apply(Cheat::AlternateBall);
        assert_eq!(settings.mode(0x78), Some(1));
        assert_eq!(settings.mode(0x4c), Some(1));
        assert_eq!(settings.mode(0), Some(1));
        assert_eq!(settings.mode(0x10), Some(2));
        assert_eq!(settings.mode(1), None);
        assert_eq!(settings.mode(0x8c), None);
        let mut rules = Rules::default();
        settings.apply_rules(&mut rules);
        assert!(rules.no_counters);
        assert!(!rules.easy_2_pointers);
        assert!(rules.hard_2_pointers);
    }

    #[test]
    fn network_selectors_round_trip_and_reject_unsupported_values() {
        let mut settings = Settings::default();
        for cheat in ALL {
            settings.apply(cheat);
        }
        let bytes = settings.to_bytes();
        assert_eq!(bytes.len(), Settings::BYTE_LEN);
        let restored = Settings::from_bytes(&bytes).expect("valid selectors");
        assert_eq!(restored.to_bytes(), bytes);
        assert_eq!(restored.body_scale(), 0.5);
        assert_eq!(restored.head_scale(), 1.5);
        assert_eq!(restored.ball_model_suffix(), 'e');
        assert!(restored.no_hud());
        let mut ratings = Ratings::default();
        restored.apply_ratings(&mut ratings);
        assert_eq!(ratings.speed, 1.0);
        assert_eq!(ratings.threes, 0.0);
        assert_eq!(ratings.shots, Ratings::default().shots);
        for (offset, value) in [(4, 1u32), (0x10, 4), (0x30, 2), (0x40, 1)] {
            let mut invalid = bytes.clone();
            invalid[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(Settings::from_bytes(&invalid).is_none());
        }
        assert!(Settings::from_bytes(&bytes[..bytes.len() - 1]).is_none());
        settings.reset();
        assert_eq!(settings.to_bytes(), vec![0; Settings::BYTE_LEN]);
    }

    #[test]
    fn ball_model_switch_keeps_the_native_physics_size_selector() {
        let mut settings = Settings::default();
        assert_eq!(settings.ball_radius(), street_sim::ball::RADIUS);
        settings.apply(Cheat::WomensBall);
        assert_eq!(settings.ball_model_suffix(), 'f');
        assert_eq!(settings.ball_radius(), street_sim::ball::WOMENS_RADIUS);
        settings.apply(Cheat::AlternateBall);
        assert_eq!(settings.ball_model_suffix(), 'e');
        assert_eq!(settings.ball_radius(), street_sim::ball::WOMENS_RADIUS);
        settings.reset();
        assert_eq!(settings.ball_radius(), street_sim::ball::RADIUS);
    }
}
