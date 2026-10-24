//! Select's question: "Exit to Main Menu?" (`SELECT_PRESS`, event 0x3e).
//!
//! On 21 of the original's screens Select opens one question
//! (`FUN_0024bfa0`): the base Yes/No dialog `FUN_0025d328` with the words
//! of text id 0x110 (`common_fe.txt` 16, "Exit\nto Main Menu?"), rows
//! "Yes" and "No" (`common.txt` 2, 3) in font 2, and its Triangle switched
//! off (`+0xe0` 1, which the dialogs' keys `FUN_0025c100` test before they
//! close on 0x25). The screen plays sound 3 as it opens it. Each screen
//! gives the dialog its own id (`+0x40`), which only its answer handler
//! (the screen's slot `+0xe4`) tests: Yes (`FUN_0024c028`: the lit row is
//! the Yes widget `+0x110`) goes back to Main (`FUN_0029e138`'s class 3,
//! `FUN_002d7d40`, a back change), No plays sound 9 and leaves the screen
//! as it was. Measured in the project's PCSX2 runtime on 18 screens: the
//! question opens on No, up and down step between the rows without
//! wrapping, Triangle and Start do nothing, Cross answers; Yes stood Main
//! 0.7 s after the press (Play It, state 2 then the change).
//!
//! The screens and their ids ([`asked`]), read from each screen's keys
//! (its slot `+0xa4`) and answer (`+0xe4`) through the factory
//! `FUN_002e55c0`:
//!
//! | Class | Screen | Keys, answer | Id |
//! |---|---|---|---|
//! | 5 | Play It | `FUN_002bf428`, `FUN_002bf670` | 0x55 |
//! | 6 | My Team | `FUN_002a3fe0`, `FUN_002a41a0` | 0x55 |
//! | 9 | the credits' screen | `FUN_00283d08`, `FUN_00283e30` | 1000 |
//! | 10 | High Scores | `FUN_002d30f0`, `FUN_002d3540` | 0x3e9 |
//! | 0xc..0x13 | User Setup | `FUN_00297c40`, `FUN_00298168` | 0x55 |
//! | 0x14 | Rewards | `FUN_002c1370`, `FUN_002c3518` | 0x44e |
//! | 0x19, 0x1b, 0x1c | Choose Team (not 0x4d, join a team) | `FUN_0026ed50`, `FUN_0026ef00` | 200 |
//! | 0x1d, 0x2b | Choose Players | `FUN_0026a860`, `FUN_0026aeb0` | 0x55 |
//! | 0x21 | Choose Court | `FUN_002686e0`, `FUN_002696a8` | 0xd06 |
//! | 0x27 | My Rules | `FUN_002c9740`, `FUN_002cb490` | 0x457 |
//! | 0x28 | Options | `FUN_002a4d18`, `FUN_002a5cb8` | 0x85 |
//! | 0x29 | Choose Region | `FUN_0026bdc8`, `FUN_0026ddb0` | 0x457 |
//! | 0x30 | Trax | `FUN_00265978`, `FUN_00265c10` | 10 |
//! | 0x36, 0x3d | Choose Baller, Create A Baller | `FUN_002cdce8`, `FUN_002cd9c8` | 535 |
//! | 0x37 | Controller Layout | `FUN_00273388`, `FUN_002736e0` | 0x55 |
//! | 0x38 | Be A Legend's map | `FUN_0029f4b0`, `FUN_0029fae8` | 0x55 |
//! | 0x4a..0x4c | Create Baller | `FUN_00274530`, `FUN_002743d0` | 535 |
//! | 0x4f..0x55, 0x65 | the editor: Edit Baller, Edit Moves, The Moves | `FUN_0028be88`, `FUN_0028bd60` | 535 |
//! | 0x58, 0x59, 0x5e | a tournament's bracket, not yet decided | `FUN_002cd0f8`, `FUN_00288290`, `FUN_00266e70` | 0xd05 |
//! | 0x61 | Game Data | `FUN_00299110`, `FUN_002993f8` | 0x55 |
//! | 0x66..0x68 | Inside Vol. 2's pages | `FUN_00289f98`, `FUN_0028aee0`, `FUN_00289100` | 0x55 |
//! | 0x69 | Difficulty | `FUN_00284848`, `FUN_00284af8` | 0x55 |
//!
//! (535 is `DAT_003f814c`.) Main (`FUN_0029e700`), the title, the
//! loading screens, the results, the event intros, Create Team and the
//! crew, the completion boards, join a team, Update Team and the credits'
//! rolls take no Select. A decided bracket (its `+0x10c`, set as the
//! champion shows) takes none either.

use crate::{
    boards::{Gm, Panels},
    look::{Labels, Look, Piece},
};

/// The question's words: text id 0x110.
pub const WORDS: (&str, usize) = ("common_fe.txt", 16);
/// The words without the disc.
pub const WORDS_PLAIN: &str = "Exit\\nto Main Menu?";
/// The rows.
pub const YES: usize = 0;
pub const NO: usize = 1;
/// The row lit as it opens (measured: No, as every `FUN_0025d328`
/// dialog).
pub const OPENS_ON: usize = NO;
/// The sound the screens play as they open it (`FUN_001a1908(.., 3, 0)`).
pub const OPEN_SOUND: usize = 3;

/// What a screen does on Yes besides going back to Main.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Yes {
    /// `FUN_002d7d40(screen, 3)` alone.
    Main,
    /// Trax (`FUN_00265c10`): the radio's rows applied to the settings
    /// (`FUN_00219f58`), without the card's save its Exit makes.
    KeepTrax,
    /// Options (`FUN_002a5cb8`): the settings as the game data holds them
    /// put back (`FUN_002a6d40`, as its cancel question's Yes).
    CancelOptions,
    /// User Setup (`FUN_00298168`): the codes entered cleared
    /// (`FUN_00298ae8` on the cheats' settings `FUN_00131208`).
    ClearCodes,
    /// The editor (`FUN_0028bd60`): the baller as edited kept in the game
    /// data (`FUN_001e6888`, as its card save does) without the save; the
    /// film room's clip stopped (`FUN_002e4690`).
    KeepBaller,
}

/// A screen's question: the screen's class, its dialog id and what Yes
/// does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Asked {
    pub class: u16,
    pub dialog: u16,
    pub yes: Yes,
}

/// The question a screen asks on Select, by its factory class; `None`
/// where Select does nothing.
pub fn asked(class: u16) -> Option<Asked> {
    let (dialog, yes) = match class {
        0x05 | 0x06 | 0x37 | 0x38 | 0x61 | 0x66..=0x69 | 0x1d | 0x2b => (0x55, Yes::Main),
        0x09 => (1000, Yes::Main),
        0x0a => (0x3e9, Yes::Main),
        0x0c..=0x13 => (0x55, Yes::ClearCodes),
        0x14 => (0x44e, Yes::Main),
        0x19 | 0x1b | 0x1c => (200, Yes::Main),
        0x21 => (0xd06, Yes::Main),
        0x27 | 0x29 => (0x457, Yes::Main),
        0x28 => (0x85, Yes::CancelOptions),
        0x30 => (10, Yes::KeepTrax),
        0x36 | 0x3d | 0x4a..=0x4c => (535, Yes::Main),
        0x4f..=0x55 | 0x65 => (535, Yes::KeepBaller),
        0x58 | 0x59 | 0x5e => (0xd05, Yes::Main),
        _ => return None,
    };
    Some(Asked { class, dialog, yes })
}

/// Whether the screen's answer plays the transition sound itself on Yes
/// (`FUN_001a1b88(.., 0)`): Inside Vol. 2's (`FUN_0028a208`) alone; Play
/// It's and My Team's, back to Main with no movie as their Triangle is,
/// do not, where their Triangle does ([`crate::transition::hand_whoosh`]).
pub fn whooshes(class: u16) -> bool {
    class == 0x66
}

/// What leaving the screen on Yes adds to the front end's hold before the
/// back change's movie, ms (measured, the 100 Hz timer: the change's
/// request to its end 1.48 s from Trax and 1.59 s from Create Baller's
/// wizard against 1.18 s elsewhere): Trax's answer restarts the front
/// end's music (`FUN_00219f58`'s `FUN_001a0208`); the wizard's leaving
/// frees its baller (not traced).
pub fn hold_ms(class: u16) -> f32 {
    match class {
        0x30 => 300.0,
        0x4a..=0x4c => 410.0,
        _ => 0.0,
    }
}

/// Up (-1) or down (+1) from `lit`: the two rows, no wrap.
pub fn step(lit: usize, dy: i32) -> usize {
    (lit as i32 + dy).clamp(YES as i32, NO as i32) as usize
}

/// The question over the screen, as the front end's dialogs draw
/// ([`crate::pause::dialog`]).
pub fn pieces(
    look: &Look,
    gm: &Gm,
    labels: &mut Labels,
    images: &mut bevy::prelude::Assets<bevy::prelude::Image>,
    panels: &mut Panels,
    lit: usize,
    ticks: u64,
) -> Vec<Piece> {
    let words = look
        .text(WORDS.0, WORDS.1)
        .unwrap_or(WORDS_PLAIN)
        .to_owned();
    let rows: Vec<String> = [(2, "Yes"), (3, "No")]
        .iter()
        .map(|&(k, or)| look.text("common.txt", k).unwrap_or(or).to_owned())
        .collect();
    crate::pause::dialog(
        look,
        gm,
        labels,
        images,
        panels,
        &crate::pause::Dialog {
            text: &words,
            rows: &rows,
            row_font: 2,
            lit,
            ticks,
            dim: crate::pause::FRONT_END_DIM,
            width: 0.0,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ids the answer handlers test, and the screens that take no
    /// Select.
    #[test]
    fn the_screens_ask_with_their_ids() {
        let ids: Vec<(u16, u16)> = [
            5, 6, 9, 10, 0xd, 0x14, 0x19, 0x1b, 0x1d, 0x21, 0x27, 0x28, 0x29, 0x2b, 0x30, 0x36,
            0x37, 0x38, 0x3d, 0x4a, 0x4f, 0x55, 0x58, 0x59, 0x5e, 0x61, 0x65, 0x66, 0x67, 0x68,
            0x69,
        ]
        .iter()
        .map(|&c| (c, asked(c).unwrap().dialog))
        .collect();
        assert_eq!(
            ids,
            [
                (5, 0x55),
                (6, 0x55),
                (9, 1000),
                (10, 0x3e9),
                (0xd, 0x55),
                (0x14, 0x44e),
                (0x19, 200),
                (0x1b, 200),
                (0x1d, 0x55),
                (0x21, 0xd06),
                (0x27, 0x457),
                (0x28, 0x85),
                (0x29, 0x457),
                (0x2b, 0x55),
                (0x30, 10),
                (0x36, 535),
                (0x37, 0x55),
                (0x38, 0x55),
                (0x3d, 535),
                (0x4a, 535),
                (0x4f, 535),
                (0x55, 535),
                (0x58, 0xd05),
                (0x59, 0xd05),
                (0x5e, 0xd05),
                (0x61, 0x55),
                (0x65, 535),
                (0x66, 0x55),
                (0x67, 0x55),
                (0x68, 0x55),
                (0x69, 0x55),
            ]
        );
        // Main, the title, the boot, the rolls, the results, the loading
        // screens, the intros, Create Team and the crew, the boards, join a
        // team and Update Team.
        for class in [
            0, 2, 3, 4, 7, 8, 0x22, 0x23, 0x2c, 0x2f, 0x31, 0x35, 0x3b, 0x3c, 0x4d, 0x60, 0x62,
            0x63,
        ] {
            assert_eq!(asked(class), None, "class {class:#x}");
        }
    }

    /// Yes does more than go back on four screens.
    #[test]
    fn yes_keeps_or_puts_back_what_the_screen_holds() {
        assert_eq!(asked(0x30).unwrap().yes, Yes::KeepTrax);
        assert_eq!(asked(0x28).unwrap().yes, Yes::CancelOptions);
        assert_eq!(asked(0x0e).unwrap().yes, Yes::ClearCodes);
        assert_eq!(asked(0x52).unwrap().yes, Yes::KeepBaller);
        assert_eq!(asked(0x55).unwrap().yes, Yes::KeepBaller);
        assert_eq!(asked(0x05).unwrap().yes, Yes::Main);
        assert_eq!(asked(0x4b).unwrap().yes, Yes::Main);
    }

    /// It opens on No; up and down stop at the rows' ends.
    #[test]
    fn the_rows_step_without_wrapping() {
        assert_eq!(OPENS_ON, NO);
        assert_eq!(step(NO, 1), NO);
        assert_eq!(step(NO, -1), YES);
        assert_eq!(step(YES, -1), YES);
        assert_eq!(step(YES, 1), NO);
    }

    /// The words are text id 0x110 of the front end's table.
    #[test]
    fn the_words_are_text_0x110() {
        assert_eq!(
            street_assets::locale::FE_TEXT_FILES_ALL[0x110 >> 8],
            WORDS.0
        );
        assert_eq!(0x110 & 0xff, WORDS.1);
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let fe = street_assets::locale::read(&mut iso, "/LOCALE/XLOC_FE.EUS").unwrap();
        let file = fe.iter().find(|f| f.name == WORDS.0).unwrap();
        assert_eq!(file.texts[WORDS.1], WORDS_PLAIN);
    }
}
