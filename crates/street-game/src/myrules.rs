//! My Rules, the game's Set Challenge screen (class 0x27 = 39, blob 11):
//! constructor `FUN_002c8ad0` (`FUN_002d7720(screen, 0x27, 0x19)`, backdrop
//! 25), build `FUN_002c8d08`, input `FUN_002c9740`, update `FUN_002ca060`,
//! the arrows and limits `FUN_002cb6d0`, the clamp `FUN_002cba40`, the
//! steps `FUN_002cb620`/`FUN_002cb6a0`, the cancel dialog `FUN_002cb548`
//! and its answer `FUN_002cb490`. The trace is in
//! `docs/FRONTEND_RECOVERY.md`, "Set Challenge".
//!
//! The build registers the rows Game Mode (0x25), Play To (0x27), Spot
//! Points (0x29), Shot Clock (0x28) and Play Game (0x24) on list 0x26, so
//! they sit on its nodes (151, 104), (151, 169), (151, 243), (151, 315)
//! and (395, 374); the values are the screen's own text widgets at x 408
//! (font 2, centred, 0xff88a0be, gold on the focused row), the Spot
//! Points row two text groups (the sides' names, then their points), and
//! the arrows a pair of pictures moved to the focused row. Each mode keeps
//! its own "Play To" record (points, trick points, a Gamebreaker level);
//! Play Game stores them (`FUN_001dbd48`, `FUN_001dbd60`, `FUN_001dbd78`,
//! `FUN_001dbd80`) and goes to the loading class 0x2c; Back asks "Cancel
//! Changes and Return to Choose Court?" and, on Yes, returns to class 0x21
//! with the records as they were.

use bevy::prelude::*;
use street_sim::{Goal, Mode, Rules};

use crate::{
    look::{self, Align, Look, Piece, colours, fonts},
    users::count,
};

/// The screen's backdrop (`FUN_002d7720(screen, 0x27, 0x19)`).
pub const BACKDROP: usize = 25;

/// The rows, in registration order.
pub const GAME_MODE: usize = 0;
pub const PLAY_TO: usize = 1;
pub const SPOT: usize = 2;
pub const CLOCK: usize = 3;
pub const PLAY_GAME: usize = 4;

/// The modes in the screen's order (text ids 0xe06..0xe0d, which left and
/// right cycle through), the same as [`Mode::ALL`].
pub const MODES: [Mode; 8] = Mode::ALL;

/// The values' places: the mode, Play To and the clock centred at x 408
/// (`FUN_002c8d08`: (408, 110), (408, 174), (408, 320)), the Spot Points
/// groups at (285, 244) and (285, 268).
pub const VALUE_X: f32 = 408.0;
pub const VALUE_Y: [f32; 4] = [110.0, 174.0, 244.0, 320.0];
pub const SPOT_NAMES_AT: Vec2 = Vec2::new(285.0, 244.0);
pub const SPOT_VALUES_AT: Vec2 = Vec2::new(285.0, 268.0);
/// In a group, the away side's text ends 118 (names) or 113 (values)
/// right of the group's point, the home side's starts 133 right; the
/// values sit 3 lower; a side with two names moves its text up 18.
pub const SPOT_AWAY_END: [f32; 2] = [118.0, 113.0];
pub const SPOT_HOME_START: f32 = 133.0;
pub const SPOT_VALUES_DOWN: f32 = 3.0;
pub const SPOT_TWO_NAMES_UP: f32 = 18.0;
/// The arrows (elements 0x45f and 0x460) by row: the left one's point and
/// the right one's (`DAT_004d5408..`).
pub const ARROWS: [(Vec2, Vec2); 4] = [
    (Vec2::new(254.0, 110.0), Vec2::new(554.0, 113.0)),
    (Vec2::new(254.0, 174.0), Vec2::new(554.0, 177.0)),
    (Vec2::new(254.0, 255.0), Vec2::new(554.0, 258.0)),
    (Vec2::new(254.0, 320.0), Vec2::new(554.0, 323.0)),
];
/// A pressed arrow dims for ten updates (`+0x100`/`+0x102` set to 10;
/// the alpha `~(n * 10) & 0xff` each update).
pub const FLASH: i8 = 10;

/// The points record (`+0x108`): 21 of 1..50 (League Scoring from 2) by 1;
/// the trick points record (`+0x1e4`): 100,000 of 100,000..10,000,000 by
/// 50,000, 100,000 from a million; the Gamebreaker record (`+0x2c0`):
/// level 1 or 2, its turbo spot by 10 up to 10 under 100 or 200.
pub const POINTS: (u32, u32, u32) = (21, 1, 50);
pub const POINTS_LEAGUE_MIN: u32 = 2;
pub const TRICK: (u32, u32, u32) = (100_000, 100_000, 10_000_000);
pub const TRICK_STEP: u32 = 50_000;
pub const TRICK_STEP_HIGH: u32 = 100_000;
pub const TRICK_HIGH: u32 = 1_000_000;
/// The rematch rules' trick-points spot under 100,000 (`FUN_00260368`).
pub const TRICK_SPOT_FINE: u32 = 25_000;
pub const TURBO_STEP: u32 = 10;
/// The shot clocks: Off, 12, 24 and 48 seconds (text ids 0xe0e..0xe11),
/// 24 to start.
pub const CLOCKS: [Option<f32>; 4] = [None, Some(12.0), Some(24.0), Some(48.0)];

/// What the screen edits: the records behind the rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edit {
    /// Index into [`MODES`].
    pub mode: usize,
    pub points: u32,
    pub trick_points: u32,
    /// Gamebreaker Wins' level 2 (`+0x104` cleared).
    pub level2: bool,
    /// The side spotted: true home (`+0x10` 1), false away.
    pub spot_home: bool,
    /// The points, trick points or turbo spotted (`+0x14`), per record.
    pub spot_points: u32,
    pub spot_trick_points: u32,
    pub spot_turbo: u32,
    /// Index into [`CLOCKS`].
    pub clock: usize,
    /// The arrows' flash countdowns (left, right).
    pub flash: [i8; 2],
    /// The rematch rules' finer trick-points spot (`FUN_00260368`: 25,000
    /// under 100,000, and from it going down), where My Rules' steps
    /// 50,000 (`FUN_002cb620`).
    pub fine_spot: bool,
}

impl Default for Edit {
    fn default() -> Self {
        Self {
            mode: 0,
            points: POINTS.0,
            trick_points: TRICK.0,
            level2: false,
            spot_home: true,
            spot_points: 0,
            spot_trick_points: 0,
            spot_turbo: 0,
            clock: 2,
            flash: [-1, -1],
            fine_spot: false,
        }
    }
}

/// Which record a mode plays to (`FUN_002c9740`'s switch on the mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record {
    Points,
    TrickPoints,
    Gamebreaker,
}

pub fn record_of(mode: Mode) -> Record {
    match mode {
        Mode::TrickPointChallenge => Record::TrickPoints,
        Mode::GamebreakerWins => Record::Gamebreaker,
        _ => Record::Points,
    }
}

impl Edit {
    /// The screen's state for the match's rules (the rewrite's side 0 is
    /// home).
    pub fn from_rules(rules: &Rules) -> Self {
        let mode_value = crate::menu::mode_of(rules);
        let mut edit = Edit {
            mode: MODES.iter().position(|&m| m == mode_value).unwrap_or(0),
            clock: CLOCKS
                .iter()
                .position(|&c| c == rules.shot_clock)
                .unwrap_or(2),
            ..Edit::default()
        };
        match rules.goal {
            Goal::Points(n) => edit.points = n,
            Goal::TrickPoints(n) => edit.trick_points = n,
            Goal::Gamebreaker(l) => edit.level2 = l >= 2,
        }
        let [home, away] = rules.spot;
        let [home_t, away_t] = rules.spot_trick_points;
        if away > 0 || away_t > 0 {
            edit.spot_home = false;
        }
        if edit.record() == Record::Gamebreaker {
            edit.spot_turbo = home.max(away);
        } else {
            edit.spot_points = home.max(away);
        }
        edit.spot_trick_points = home_t.max(away_t);
        edit
    }

    /// The rules Play Game hands the match.
    pub fn apply(&self, rules: &mut Rules) {
        let keep = (
            rules.difficulty,
            rules.unlimited_turbo,
            rules.no_counters,
            rules.easy_2_pointers,
            rules.hard_2_pointers,
        );
        *rules = Rules::mode(self.mode_value());
        rules.goal = match self.record() {
            Record::Points => Goal::Points(self.points),
            Record::TrickPoints => Goal::TrickPoints(self.trick_points),
            Record::Gamebreaker => Goal::Gamebreaker(if self.level2 { 2 } else { 1 }),
        };
        rules.shot_clock = CLOCKS[self.clock.min(3)];
        let side = if self.spot_home { 0 } else { 1 };
        rules.spot = [0; 2];
        rules.spot_trick_points = [0; 2];
        match self.record() {
            Record::Points => rules.spot[side] = self.spot_points,
            Record::TrickPoints => rules.spot_trick_points[side] = self.spot_trick_points,
            // The turbo spot (`FUN_001dbd80`'s amount in the Gamebreaker
            // mode): the side's meter at the start (`Rules::meter_spot`).
            Record::Gamebreaker => rules.spot[side] = self.spot_turbo,
        }
        (
            rules.difficulty,
            rules.unlimited_turbo,
            rules.no_counters,
            rules.easy_2_pointers,
            rules.hard_2_pointers,
        ) = keep;
    }

    pub fn mode_value(&self) -> Mode {
        MODES[self.mode.min(7)]
    }

    pub fn record(&self) -> Record {
        record_of(self.mode_value())
    }

    /// The record's value, minimum and maximum (`FUN_002cba40` clamps
    /// between them).
    fn value(&self) -> (u32, u32, u32) {
        match self.record() {
            Record::Points => (
                self.points,
                if self.mode_value() == Mode::LeagueScoring {
                    POINTS_LEAGUE_MIN
                } else {
                    POINTS.1
                },
                POINTS.2,
            ),
            Record::TrickPoints => (self.trick_points, TRICK.1, TRICK.2),
            Record::Gamebreaker => {
                let v = if self.level2 { 200 } else { 100 };
                (v, v, v)
            }
        }
    }

    /// The spot amount of the record.
    fn spot(&self) -> u32 {
        match self.record() {
            Record::Points => self.spot_points,
            Record::TrickPoints => self.spot_trick_points,
            Record::Gamebreaker => self.spot_turbo,
        }
    }

    fn set_spot(&mut self, amount: u32) {
        match self.record() {
            Record::Points => self.spot_points = amount,
            Record::TrickPoints => self.spot_trick_points = amount,
            Record::Gamebreaker => self.spot_turbo = amount,
        }
    }

    /// The spot's step (`FUN_002cb620`): 1 point, 10 turbo, or 50,000
    /// trick points (100,000 from a million, but 50,000 shrinking from it);
    /// the rematch's (`FUN_00260368`) 25,000 under 100,000 and shrinking
    /// from it.
    fn spot_step(&self, shrinking: bool) -> u32 {
        match self.record() {
            Record::Points => 1,
            Record::Gamebreaker => TURBO_STEP,
            Record::TrickPoints => {
                let s = self.spot_trick_points;
                if self.fine_spot && (s < TRICK.1 || (s == TRICK.1 && shrinking)) {
                    TRICK_SPOT_FINE
                } else if s >= TRICK_HIGH && !(s == TRICK_HIGH && shrinking) {
                    TRICK_STEP_HIGH
                } else {
                    TRICK_STEP
                }
            }
        }
    }

    /// Play To's step (`FUN_002cb6a0`): the record's own, trick points
    /// taking 100,000 over a million going up, and from above it going
    /// down.
    fn value_step(&self, up: bool) -> u32 {
        match self.record() {
            Record::Points => 1,
            Record::Gamebreaker => 0,
            Record::TrickPoints => {
                let high = if up {
                    self.trick_points >= TRICK_HIGH
                } else {
                    self.trick_points > TRICK_HIGH
                };
                if high { TRICK_STEP_HIGH } else { TRICK_STEP }
            }
        }
    }

    /// The clamp (`FUN_002cba40`) and the spot kept under the value
    /// (`FUN_002cb6d0`'s tail: a spot at or past the value drops to the
    /// value less a step). True when something was clamped (the original
    /// plays sound 2).
    fn settle(&mut self) -> bool {
        let (value, min, max) = self.value();
        let mut clamped = false;
        if self.record() != Record::Gamebreaker {
            let fixed = value.clamp(min, max);
            if fixed != value {
                clamped = true;
                match self.record() {
                    Record::Points => self.points = fixed,
                    Record::TrickPoints => self.trick_points = fixed,
                    Record::Gamebreaker => {}
                }
            }
        }
        let (value, _, _) = self.value();
        if value <= self.spot() {
            let step = self.spot_step(false);
            self.set_spot(value.saturating_sub(step));
            clamped = true;
        }
        clamped
    }

    /// Left on a row (`FUN_002c9740` case 2).
    pub fn left(&mut self, row: usize) {
        match row {
            GAME_MODE => {
                self.mode = (self.mode + MODES.len() - 1) % MODES.len();
                self.settle();
            }
            PLAY_TO => {
                match self.record() {
                    Record::Gamebreaker => self.level2 = !self.level2,
                    Record::Points => self.points = self.points.saturating_sub(1),
                    Record::TrickPoints => {
                        let step = self.value_step(false);
                        self.trick_points = self.trick_points.saturating_sub(step);
                    }
                }
                self.settle();
            }
            SPOT => {
                // Toward the away side: a home spot of nothing switches
                // sides, a home spot shrinks, an away spot grows.
                if self.spot() == 0 && self.spot_home {
                    self.spot_home = false;
                }
                let step = self.spot_step(self.spot_home);
                let amount = if self.spot_home {
                    self.spot().saturating_sub(step)
                } else {
                    self.spot() + step
                };
                self.set_spot(amount);
                self.settle();
            }
            CLOCK => self.clock = (self.clock + CLOCKS.len() - 1) % CLOCKS.len(),
            _ => return,
        }
        self.flash[0] = FLASH;
    }

    /// Right on a row (`FUN_002c9740` case 3).
    pub fn right(&mut self, row: usize) {
        match row {
            GAME_MODE => {
                self.mode = (self.mode + 1) % MODES.len();
                self.settle();
            }
            PLAY_TO => {
                match self.record() {
                    Record::Gamebreaker => self.level2 = !self.level2,
                    Record::Points => self.points += 1,
                    Record::TrickPoints => self.trick_points += self.value_step(true),
                }
                self.settle();
            }
            SPOT => {
                if self.spot() == 0 && !self.spot_home {
                    self.spot_home = true;
                }
                let step = self.spot_step(!self.spot_home);
                let amount = if self.spot_home {
                    self.spot() + step
                } else {
                    self.spot().saturating_sub(step)
                };
                self.set_spot(amount);
                self.settle();
            }
            CLOCK => self.clock = (self.clock + 1) % CLOCKS.len(),
            _ => return,
        }
        self.flash[1] = FLASH;
    }

    /// One update of the arrows' flash.
    pub fn tick(&mut self) {
        for f in &mut self.flash {
            if *f >= 0 {
                *f -= 1;
            }
        }
    }

    /// Which arrows the focused row shows (`FUN_002ca060` for Play To,
    /// `FUN_002cb6d0` for the spot): left, right.
    pub fn arrows(&self, row: usize) -> (bool, bool) {
        match row {
            GAME_MODE | CLOCK => (true, true),
            PLAY_TO => {
                let (value, min, max) = self.value();
                if self.record() == Record::Gamebreaker {
                    (true, true)
                } else if value > min {
                    (true, value < max)
                } else {
                    (false, true)
                }
            }
            SPOT => {
                let (value, _, _) = self.value();
                let step = self.spot_step(false);
                let at_limit = self.spot() + step >= value;
                if value == 1 && self.spot() == 0 {
                    return (false, false);
                }
                if self.spot_home {
                    (true, !at_limit)
                } else {
                    (!at_limit, true)
                }
            }
            _ => (false, false),
        }
    }

    /// The Play To value's text: "%1N %2s" of the points and "Points"
    /// (0xe2b, 0xe31), or the Gamebreaker level (0xe27, 0xe28).
    pub fn value_text(&self, look: &Look) -> String {
        match self.record() {
            Record::Gamebreaker => look
                .text_id(if self.level2 { 0x0e28 } else { 0x0e27 })
                .unwrap_or(if self.level2 {
                    "Level 2 Gamebreaker"
                } else {
                    "Level 1 Gamebreaker"
                })
                .to_owned(),
            Record::Points => format!(
                "{} {}",
                count(self.points),
                look.text_id(0x0e31).unwrap_or("Points")
            ),
            Record::TrickPoints => format!(
                "{} {}",
                count(self.trick_points),
                look.text_id(0x0e31).unwrap_or("Points")
            ),
        }
    }

    /// The mode's name (0xe06..0xe0d).
    pub fn mode_text<'a>(&self, look: &'a Look) -> &'a str {
        look.text_id(0x0e06 + self.mode as u16)
            .unwrap_or(self.mode_value().name())
    }

    /// The clock's text (0xe0e..0xe11).
    pub fn clock_text<'a>(&self, look: &'a Look) -> &'a str {
        look.text_id(0x0e0e + self.clock as u16)
            .unwrap_or(["Off", "12 seconds", "24 seconds", "48 seconds"][self.clock.min(3)])
    }

    /// A side's spot text: "%1N %2s" with "pts" (0xe2b, 0xe30); in the
    /// Gamebreaker mode "%1N %%" under 100 (0xe35), "GB1" at 100 (0xe2f)
    /// and "GB1 + %1N %%" past it (0xe36).
    pub fn spot_text(&self, look: &Look, home: bool) -> String {
        let amount = if home == self.spot_home {
            self.spot()
        } else {
            0
        };
        match self.record() {
            Record::Gamebreaker => {
                if amount < 100 {
                    format!("{amount} %")
                } else if amount == 100 {
                    look.text_id(0x0e2f).unwrap_or("GB1").to_owned()
                } else {
                    format!("GB1 + {} %", amount - 100)
                }
            }
            _ => format!(
                "{} {}",
                count(amount),
                look.text_id(0x0e30).unwrap_or("pts")
            ),
        }
    }

    /// The help line (`FUN_002ca060`): the row's, per mode for Game Mode,
    /// Play To and Spot Points.
    pub fn help_id(&self, row: usize) -> u16 {
        match row {
            GAME_MODE => [
                0x0e15, 0x0e16, 0x0e17, 0x0e18, 0x0e19, 0x0e1a, 0x0e1b, 0x0e3c,
            ][self.mode.min(7)],
            PLAY_TO => match self.record() {
                Record::Points => 0x0e1d,
                Record::TrickPoints => 0x0e1f,
                Record::Gamebreaker => 0x0e1e,
            },
            SPOT => match self.record() {
                Record::Points => 0x0e20,
                Record::TrickPoints => 0x0e21,
                Record::Gamebreaker => 0x0e22,
            },
            CLOCK => 0x0e23,
            _ => 0x0e25,
        }
    }
}

/// The Spot Points row's names: a side's people (their names, or
/// "Player%1N") one a line, "CPU" for a side with nobody
/// (`FUN_002c8d08`).
pub fn side_names(
    look: &Look,
    people: &[(usize, Option<String>)],
    home: bool,
    side_of: impl Fn(usize) -> bool,
) -> String {
    let names: Vec<String> = people
        .iter()
        .filter(|&&(seat, _)| side_of(seat) == home)
        .map(|(seat, name)| match name {
            Some(n) if !n.trim().is_empty() => n.clone(),
            _ => look
                .text_id(0x0e33)
                .unwrap_or("Player%1N")
                .replace("%1N", &(seat + 1).to_string()),
        })
        .collect();
    if names.is_empty() {
        look.text_id(0x0e34).unwrap_or("CPU").to_owned()
    } else {
        names.join("\n")
    }
}

/// The screen's own pieces over the registered rows: the values, the spot
/// names and amounts, and the arrows on the focused row.
#[allow(clippy::too_many_arguments)]
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    edit: &Edit,
    names: [&str; 2],
    cursor: usize,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(pm) = look.prematch.as_ref() else {
        return out;
    };
    let mut text = |out: &mut Vec<Piece>, font: usize, s: &str, colour, at: Vec2, align| {
        if let Some(p) = labels.text(look, images, font, s, colour, None, at, align) {
            out.push(p);
        }
    };
    let lit = |row: usize| {
        if row == cursor {
            colours::ROW_LIT
        } else {
            colours::ROW
        }
    };
    text(
        &mut out,
        fonts::LIST,
        edit.mode_text(look),
        lit(GAME_MODE),
        Vec2::new(VALUE_X, VALUE_Y[GAME_MODE]),
        Align::Centre,
    );
    text(
        &mut out,
        fonts::LIST,
        &edit.value_text(look),
        lit(PLAY_TO),
        Vec2::new(VALUE_X, VALUE_Y[PLAY_TO]),
        Align::Centre,
    );
    text(
        &mut out,
        fonts::LIST,
        edit.clock_text(look),
        lit(CLOCK),
        Vec2::new(VALUE_X, VALUE_Y[CLOCK]),
        Align::Centre,
    );
    // The spot groups: away on the left (child 0, right-aligned), home on
    // the right (child 1); two names lift a side's text 18.
    let line = look
        .fonts
        .get(fonts::LIST_SMALL)
        .map_or(24.0, |f| f.line_height as f32);
    let [away, home] = names;
    for (k, (name, end_x, align)) in [
        (away, SPOT_NAMES_AT.x + SPOT_AWAY_END[0], Align::Right),
        (home, SPOT_NAMES_AT.x + SPOT_HOME_START, Align::Left),
    ]
    .into_iter()
    .enumerate()
    {
        let lines: Vec<&str> = name.lines().collect();
        let up = if lines.len() >= 2 {
            SPOT_TWO_NAMES_UP
        } else {
            0.0
        };
        for (n, l) in lines.iter().enumerate() {
            text(
                &mut out,
                fonts::LIST_SMALL,
                l,
                lit(SPOT),
                Vec2::new(end_x, SPOT_NAMES_AT.y - up + line * n as f32),
                align,
            );
        }
        let value = edit.spot_text(look, k == 1);
        let x = if k == 0 {
            SPOT_VALUES_AT.x + SPOT_AWAY_END[1]
        } else {
            SPOT_VALUES_AT.x + SPOT_HOME_START
        };
        text(
            &mut out,
            fonts::LIST_SMALL,
            &value,
            lit(SPOT),
            Vec2::new(x, SPOT_VALUES_AT.y + SPOT_VALUES_DOWN),
            align,
        );
    }
    // The arrows on the focused row.
    if let Some(&(left_at, right_at)) = ARROWS.get(cursor) {
        let (left, right) = edit.arrows(cursor);
        let alpha = |f: i8| {
            if f >= 0 {
                f32::from(!(f as u8).wrapping_mul(10)) / 255.0
            } else {
                1.0
            }
        };
        if left {
            let mut p = Piece::new(
                pm.arrow_left.clone(),
                Rect::from_corners(left_at, left_at + pm.arrow_side_size),
            );
            p.colour = Color::WHITE.with_alpha(alpha(edit.flash[0]));
            out.push(p);
        }
        if right {
            let mut p = Piece::new(
                pm.arrow_right.clone(),
                Rect::from_corners(right_at, right_at + pm.arrow_side_size),
            );
            p.colour = Color::WHITE.with_alpha(alpha(edit.flash[1]));
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_modes_cycle_and_each_keeps_its_own_play_to_record() {
        let mut e = Edit::default();
        assert_eq!(e.mode_value(), Mode::Normal);
        e.left(GAME_MODE);
        assert_eq!(e.mode_value(), Mode::BackToBasics);
        e.right(GAME_MODE);
        e.right(GAME_MODE);
        assert_eq!(
            (e.mode_value(), e.record()),
            (Mode::TrickPointChallenge, Record::TrickPoints)
        );
        e.right(PLAY_TO);
        assert_eq!(e.trick_points, 150_000);
        e.left(GAME_MODE);
        assert_eq!(e.points, 21, "the points record was not touched");
        assert_eq!(e.flash, [FLASH, FLASH]);
    }

    #[test]
    fn play_to_clamps_to_the_records_limits() {
        let mut e = Edit::default();
        for _ in 0..40 {
            e.right(PLAY_TO);
        }
        assert_eq!(e.points, 50);
        assert_eq!(e.arrows(PLAY_TO), (true, false));
        for _ in 0..60 {
            e.left(PLAY_TO);
        }
        assert_eq!(e.points, 1);
        assert_eq!(e.arrows(PLAY_TO), (false, true));
        // League Scoring plays to 2 at least (`+0x10c` 2 for mode 4).
        e.mode = MODES
            .iter()
            .position(|&m| m == Mode::LeagueScoring)
            .unwrap();
        e.settle();
        assert_eq!(e.points, 2);
        // Trick points step by 50,000 under a million and 100,000 from it.
        e.mode = 1;
        e.trick_points = 950_000;
        e.right(PLAY_TO);
        assert_eq!(e.trick_points, 1_000_000);
        e.right(PLAY_TO);
        assert_eq!(e.trick_points, 1_100_000);
        e.left(PLAY_TO);
        assert_eq!(e.trick_points, 1_000_000);
        e.left(PLAY_TO);
        assert_eq!(e.trick_points, 950_000);
        // The Gamebreaker level toggles.
        e.mode = 6;
        assert!(!e.level2);
        e.left(PLAY_TO);
        assert!(e.level2);
        assert_eq!(e.value(), (200, 200, 200));
    }

    #[test]
    fn the_spot_walks_from_one_side_through_nothing_to_the_other() {
        let mut e = Edit::default();
        assert!(e.spot_home && e.spot_points == 0);
        e.right(SPOT);
        assert_eq!((e.spot_home, e.spot_points), (true, 1));
        e.left(SPOT);
        assert_eq!((e.spot_home, e.spot_points), (true, 0));
        e.left(SPOT);
        assert_eq!((e.spot_home, e.spot_points), (false, 1));
        e.left(SPOT);
        assert_eq!((e.spot_home, e.spot_points), (false, 2));
        e.right(SPOT);
        e.right(SPOT);
        assert_eq!((e.spot_home, e.spot_points), (false, 0));
        e.right(SPOT);
        assert_eq!((e.spot_home, e.spot_points), (true, 1));
        // The spot stays under the value: at 21 points it stops at 20, and
        // the arrow toward more hides.
        for _ in 0..30 {
            e.right(SPOT);
        }
        assert_eq!(e.spot_points, 20);
        assert_eq!(e.arrows(SPOT), (true, false));
        // Lowering Play To drags the spot under it.
        e.points = 5;
        e.settle();
        assert_eq!(e.spot_points, 4);
        // At 1 point with no spot both arrows hide.
        e.points = 1;
        e.spot_points = 0;
        e.settle();
        assert_eq!(e.arrows(SPOT), (false, false));
    }

    #[test]
    fn the_gamebreaker_spot_is_turbo_in_tens() {
        let mut e = Edit {
            mode: 6,
            ..Edit::default()
        };
        let look = Look::empty();
        assert_eq!(e.spot_text(&look, true), "0 %");
        for _ in 0..12 {
            e.right(SPOT);
        }
        assert_eq!(e.spot_turbo, 90, "ten under the level's 100");
        assert_eq!(e.arrows(SPOT), (true, false));
        e.level2 = true;
        for _ in 0..12 {
            e.right(SPOT);
        }
        assert_eq!(e.spot_turbo, 190);
        e.spot_turbo = 100;
        assert_eq!(e.spot_text(&look, true), "GB1");
        e.spot_turbo = 130;
        assert_eq!(e.spot_text(&look, true), "GB1 + 30 %");
        assert_eq!(e.spot_text(&look, false), "0 %");
    }

    #[test]
    fn the_rules_round_trip_through_the_records() {
        let mut rules = Rules::mode(Mode::Normal);
        rules.spot = [0, 3];
        rules.shot_clock = None;
        let e = Edit::from_rules(&rules);
        assert_eq!((e.spot_home, e.spot_points, e.clock), (false, 3, 0));
        let mut back = Rules::mode(Mode::DunksOnly);
        back.difficulty = street_sim::Difficulty::Hard;
        e.apply(&mut back);
        assert_eq!(back.goal, Goal::Points(21));
        assert_eq!(back.spot, [0, 3]);
        assert_eq!(back.shot_clock, None);
        assert_eq!(back.difficulty, street_sim::Difficulty::Hard, "kept");
        assert!(!back.dunks_only);
        let gb = Edit {
            mode: 6,
            level2: true,
            ..Edit::default()
        };
        gb.apply(&mut back);
        assert_eq!(back.goal, Goal::Gamebreaker(2));
        assert!(Edit::from_rules(&back).level2);
        // Gamebreaker Wins' turbo spot is the spotted side's meter
        // (`FUN_001457f8`): GB1 + 30 % for the away side.
        let spotted = Edit {
            mode: 6,
            level2: true,
            spot_home: false,
            spot_turbo: 130,
            ..Edit::default()
        };
        spotted.apply(&mut back);
        assert_eq!(back.spot, [0, 130]);
        assert_eq!(back.meter_spot(1), (true, 15_000));
        assert_eq!(back.start_score(), [0, 0]);
        let again = Edit::from_rules(&back);
        assert_eq!(
            (again.spot_home, again.spot_turbo, again.spot_points),
            (false, 130, 0)
        );
    }

    #[test]
    fn the_help_follows_the_row_and_the_mode() {
        let mut e = Edit::default();
        assert_eq!(e.help_id(GAME_MODE), 0x0e15);
        assert_eq!(e.help_id(PLAY_GAME), 0x0e25);
        e.mode = 7;
        assert_eq!(e.help_id(GAME_MODE), 0x0e3c);
        e.mode = 6;
        assert_eq!((e.help_id(PLAY_TO), e.help_id(SPOT)), (0x0e1e, 0x0e22));
        e.mode = 1;
        assert_eq!((e.help_id(PLAY_TO), e.help_id(SPOT)), (0x0e1f, 0x0e21));
        assert_eq!(e.help_id(CLOCK), 0x0e23);
    }

    #[test]
    fn the_side_names_list_the_people_or_the_computer() {
        let look = Look::empty();
        let people = vec![(0, Some("VJ".to_owned())), (2, None)];
        assert_eq!(side_names(&look, &people, true, |s| s == 0), "VJ");
        assert_eq!(side_names(&look, &people, false, |s| s == 0), "Player3");
        assert_eq!(side_names(&look, &[], true, |_| true), "CPU");
        let both = vec![(0, None), (1, None)];
        assert_eq!(side_names(&look, &both, true, |_| true), "Player1\nPlayer2");
    }

    #[test]
    fn a_pressed_arrow_fades_for_ten_updates() {
        let mut e = Edit::default();
        e.right(CLOCK);
        assert_eq!(e.flash[1], FLASH);
        for _ in 0..FLASH {
            e.tick();
        }
        assert_eq!(e.flash[1], 0);
        e.tick();
        assert_eq!(e.flash[1], -1);
        e.tick();
        assert_eq!(e.flash[1], -1);
    }
}
