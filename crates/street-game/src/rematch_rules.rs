//! The rematch rules (`FUN_0025dc88`, dialog 0x5df over the base dialog
//! `FUN_0025beb0`): what a Pick Up REMATCH shows over the court before the
//! game starts again, from the pause's Quit Game (`FUN_002bd3f8`) and the
//! post-game dialog. Three rows: Play To (0x570c) with the mode's record,
//! Spot Points (0x570d) with each side's names over its spot, and Continue
//! (0x5709), lit at first.
//!
//! The keys (`FUN_0025f3f8`, slot `+0xa4`): up and down step the rows and
//! wrap (`+0x180`, 0..2); left and right on Play To step the mode's
//! record (`FUN_00260400`: its step, trick points 100,000 above a
//! million; Gamebreaker Wins turns between levels 1 and 2) within its
//! bounds (`FUN_002607b8`), and on Spot Points move the spot toward a side
//! (`FUN_00260368`: 1 point, 10 turbo, trick points by 25,000 under
//! 100,000, then 50,000, and 100,000 from a million); Cross on Continue stores the
//! mode, the record and the spot (`FUN_001dbd48`, `FUN_001dbd60`,
//! `FUN_001dbd78`, `FUN_001dbd80`) and the game starts again. Triangle
//! does nothing. The records and their steps are My Rules'
//! ([`crate::myrules::Edit`]).
//!
//! The look is `street-ingame-ui-original/post_4.png`: the dialog's panel
//! with its stars, the rows parted by 0xff424a9c lines (`FUN_0021e3f0`),
//! the spot's names and points either side of a cross of the same lines.

use bevy::prelude::*;
use street_sim::Rules;

use crate::{
    boards::{Gm, Panels},
    look::{self, Align, Labels, Look, Piece, RowSpot},
    myrules::{self, Edit, Record},
};

/// The rows.
pub const PLAY_TO: usize = 0;
pub const SPOT: usize = 1;
pub const CONTINUE: usize = 2;

/// The dialog under way.
#[derive(Clone, Debug, PartialEq)]
pub struct RulesDialog {
    pub edit: Edit,
    pub row: usize,
    pub ticks: u64,
}

impl RulesDialog {
    /// The dialog for the game just played, Continue lit.
    pub fn new(rules: &Rules) -> Self {
        Self {
            edit: Edit {
                fine_spot: true,
                ..Edit::from_rules(rules)
            },
            row: CONTINUE,
            ticks: 0,
        }
    }

    /// A press (`FUN_0025f3f8`); true once Continue is pressed.
    pub fn press(&mut self, up: bool, down: bool, left: bool, right: bool, cross: bool) -> bool {
        if up {
            self.row = (self.row + 2) % 3;
        } else if down {
            self.row = (self.row + 1) % 3;
        } else if left || right {
            let row = match self.row {
                PLAY_TO => myrules::PLAY_TO,
                SPOT => myrules::SPOT,
                _ => return false,
            };
            if left {
                self.edit.left(row);
            } else {
                self.edit.right(row);
            }
        } else if cross {
            return self.row == CONTINUE;
        }
        false
    }
}

/// The rematch rules, while they are up.
#[derive(Resource, Default)]
pub struct RematchRules(pub Option<RulesDialog>);

/// The panel (left, top, width, height): the constructor's 490 by 220
/// (`+0xf0`, `+0xf4`) on the dialogs' point (320, 240); its frame's inner
/// lines lie at x 83 and 557 and y 118 and 340 in `post_4.png`.
const PANEL: [f32; 4] = [75.0, 130.0, 490.0, 220.0];
/// The lines: the rows' dividers across the panel, and the spot's cross.
const ROW_LINES: [f32; 2] = [188.0, 275.0];
const LINE_X: (f32, f32) = (85.0, 556.0);
const SPOT_LINE: (f32, f32, f32) = (262.0, 531.0, 238.0);
const SPOT_BAR: (f32, f32, f32) = (395.0, 200.0, 265.0);
/// The line widgets' colour (0xff424a9c).
const LINE: [u8; 4] = [0x42, 0x4a, 0x9c, 0xff];
/// The words: the labels at x 110, the value centred at x 365, the spot's
/// names and points ending 7 left of the bar and starting 9 right of it;
/// the rows' middles at y 163, 240 (names 226, points 253) and 305.
const LABEL_X: f32 = 110.0;
const VALUE_X: f32 = 365.0;
const NAMES_Y: f32 = 226.0;
const POINTS_Y: f32 = 253.0;
const ROW_Y: [f32; 3] = [163.0, 240.0, 305.0];
const AWAY_END: f32 = 388.0;
const HOME_START: f32 = 406.0;
/// The words' colour (the dialog's text, `FUN_0025d1e8`).
const WORDS: [u8; 4] = [0xd4, 0xd8, 0xf0, 0xff];

/// The value of the mode's record ("21 Points", "Level 1 Gamebreaker",
/// `0x5715` "%1N %2s" and `0x5712`/`0x5713`).
pub fn value_text(gm: &Gm, edit: &Edit) -> String {
    match edit.record() {
        Record::Gamebreaker => gm
            .text(if edit.level2 { 0x5713 } else { 0x5712 })
            .to_owned(),
        Record::Points => format!("{} {}", edit.points, gm.text(0x570f)),
        Record::TrickPoints => format!(
            "{} {}",
            crate::users::count(edit.trick_points),
            gm.text(0x570f)
        ),
    }
}

/// A side's spot ("0 pts": `0x5715` with `0x570e`).
pub fn spot_text(gm: &Gm, edit: &Edit, home: bool) -> String {
    let amount = if edit.spot_home == home {
        match edit.record() {
            Record::Points => edit.spot_points,
            Record::TrickPoints => edit.spot_trick_points,
            Record::Gamebreaker => edit.spot_turbo,
        }
    } else {
        0
    };
    format!("{} {}", crate::users::count(amount), gm.text(0x570e))
}

/// The dialog's pieces over the court, dimming it as the pause's dialogs
/// do; `names` the sides' (away, home).
pub fn pieces(
    look: &Look,
    gm: &Gm,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    panels: &mut Panels,
    d: &RulesDialog,
    names: [&str; 2],
) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut dim = Piece::new(gm.white.clone(), Rect::new(0.0, 0.0, 640.0, 480.0));
    dim.colour = Color::srgba(0.0, 0.0, 0.0, look::black_alpha(crate::pause::PAUSE_DIM));
    out.push(dim);
    out.push(panels.get(PANEL, 0xff, gm, images));
    let line = |out: &mut Vec<Piece>, rect: Rect| {
        let mut p = Piece::new(gm.white.clone(), rect);
        p.colour = Color::srgba_u8(LINE[0], LINE[1], LINE[2], LINE[3]);
        out.push(p);
    };
    for y in ROW_LINES {
        line(&mut out, Rect::new(LINE_X.0, y, LINE_X.1, y + 2.0));
    }
    line(
        &mut out,
        Rect::new(SPOT_LINE.0, SPOT_LINE.2, SPOT_LINE.1, SPOT_LINE.2 + 2.0),
    );
    line(
        &mut out,
        Rect::new(SPOT_BAR.0, SPOT_BAR.1, SPOT_BAR.0 + 3.0, SPOT_BAR.2),
    );
    let font = crate::pause::FONT;
    let line_h = look.fonts.get(font).map_or(20.0, |f| f.text_height as f32);
    let mut text = |out: &mut Vec<Piece>, s: &str, colour: [u8; 4], at: Vec2, align| {
        if let Some(p) = labels.text(look, images, font, s, colour, None, at, align) {
            out.push(p);
        }
    };
    let lit = |row: usize| {
        if d.row == row {
            look::colours::ROW_LIT
        } else {
            WORDS
        }
    };
    let middle = |y: f32| y - line_h / 2.0;
    text(
        &mut out,
        gm.text(0x570c),
        lit(PLAY_TO),
        Vec2::new(LABEL_X, middle(ROW_Y[PLAY_TO])),
        Align::Left,
    );
    text(
        &mut out,
        &value_text(gm, &d.edit),
        lit(PLAY_TO),
        Vec2::new(VALUE_X, middle(ROW_Y[PLAY_TO])),
        Align::Centre,
    );
    text(
        &mut out,
        gm.text(0x570d),
        lit(SPOT),
        Vec2::new(LABEL_X, middle(ROW_Y[SPOT])),
        Align::Left,
    );
    for (home, x, align) in [
        (false, AWAY_END, Align::Right),
        (true, HOME_START, Align::Left),
    ] {
        let name = names[usize::from(home)];
        text(&mut out, name, WORDS, Vec2::new(x, middle(NAMES_Y)), align);
        let spot = spot_text(gm, &d.edit, home);
        text(
            &mut out,
            &spot,
            lit(SPOT),
            Vec2::new(x, middle(POINTS_Y)),
            align,
        );
    }
    // Continue: a row widget, lit with its stars.
    let mut stars = Vec::new();
    look::screen_row(
        &mut out,
        &mut stars,
        labels,
        look,
        images,
        RowSpot {
            at: Vec2::new(320.0, middle(ROW_Y[CONTINUE])),
            font,
            align: Align::Centre,
            valign: 0,
        },
        gm.text(0x5709),
        d.row == CONTINUE,
        true,
        d.ticks,
    );
    out.extend(stars);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_sim::{Goal, Mode};

    /// The rows wrap; Play To steps the record within its bounds; the
    /// spot moves to a side; Cross acts on Continue only.
    #[test]
    fn the_rows_wrap_and_step_the_records() {
        let mut rules = Rules::mode(Mode::Normal);
        rules.goal = Goal::Points(21);
        let mut d = RulesDialog::new(&rules);
        assert_eq!(d.row, CONTINUE);
        assert!(!d.press(false, true, false, false, false));
        assert_eq!(d.row, PLAY_TO, "down from Continue wraps");
        assert!(!d.press(false, false, false, true, false));
        assert_eq!(d.edit.points, 22);
        for _ in 0..40 {
            d.press(false, false, false, true, false);
        }
        assert_eq!(d.edit.points, myrules::POINTS.2, "held at the top");
        assert!(
            !d.press(false, false, false, false, true),
            "Cross on Play To"
        );
        d.press(true, false, false, false, false);
        assert_eq!(d.row, CONTINUE, "up from Play To wraps");
        d.press(true, false, false, false, false);
        assert_eq!(d.row, SPOT);
        d.press(false, false, false, true, false);
        assert!(d.edit.spot_home);
        assert_eq!(d.edit.spot_points, 1);
        d.press(false, true, false, false, false);
        assert!(d.press(false, false, false, false, true), "Continue");
        let mut after = rules;
        d.edit.apply(&mut after);
        assert_eq!(after.goal, Goal::Points(50));
        assert_eq!(after.spot, [1, 0]);
    }

    /// The trick points' spot steps 25,000 under 100,000 in the rematch
    /// (`FUN_00260368`), 50,000 in My Rules (`FUN_002cb620`); both step
    /// 50,000 shrinking from a million.
    #[test]
    fn the_rematch_spots_trick_points_finer() {
        let rules = Rules::mode(Mode::TrickPointChallenge);
        let mut d = RulesDialog::new(&rules);
        d.edit.trick_points = 2_000_000;
        d.row = SPOT;
        d.press(false, false, false, true, false);
        assert_eq!(d.edit.spot_trick_points, 25_000);
        let mut mine = Edit::from_rules(&rules);
        mine.trick_points = 2_000_000;
        mine.right(myrules::SPOT);
        assert_eq!(mine.spot_trick_points, 50_000);
        mine.spot_trick_points = 1_000_000;
        mine.left(myrules::SPOT);
        assert_eq!(mine.spot_trick_points, 950_000);
    }
}
