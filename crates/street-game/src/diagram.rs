//! Controller Layout's diagram (screen class 0x37, `FUN_002730a8`; its
//! controller widget `FUN_002144c8` at (70, 90)): the pad in the game's
//! own picture (`XACOMMON` sheet 1, image 1) and what each button does
//! in the chosen layout, on offence and on defence, beside the game's
//! button icons (sheet 1, image 0), in `contconfig.txt`'s words. The
//! constructed coordinates come from 00216dd8/002164a0/00217398.

use bevy::prelude::*;
use street_sim::button;

use crate::look::{Align, Look, Piece, colours, fonts};

/// The icons on sheet 1, image 0 (x, y, w, h).
#[derive(Clone, Copy)]
pub enum Icon {
    Triangle,
    Circle,
    Cross,
    Square,
    L1,
    R1,
    L2,
    R2,
    Select,
}

impl Icon {
    pub fn source(self) -> Rect {
        let (x, y, w, h) = match self {
            Icon::Triangle => (4, 52, 24, 24),
            Icon::Cross => (36, 52, 24, 24),
            Icon::Square => (4, 84, 24, 24),
            Icon::Circle => (36, 84, 24, 24),
            Icon::L2 => (129, 52, 31, 24),
            Icon::R2 => (65, 84, 31, 24),
            Icon::R1 => (97, 85, 31, 21),
            Icon::L1 => (129, 85, 31, 21),
            Icon::Select => (171, 121, 27, 18),
        };
        Rect::new(x as f32, y as f32, (x + w) as f32, (y + h) as f32)
    }
}

/// The face buttons as layout A names them (Circle shoot, Cross pass,
/// Square trick, Triangle block), and their icons.
const FACE: [(Icon, u16); 4] = [
    (Icon::Triangle, button::JUMP),
    (Icon::Circle, button::SHOOT),
    (Icon::Cross, button::PASS),
    (Icon::Square, button::TRICK),
];

/// The controller widget's offence rows (`FUN_002164a0`, cases 1..4, at
/// y 60, 85, 115 and 145 of the panel beside `FUN_00216dd8`'s Cross,
/// Circle, Triangle and Square): the `contconfig.txt` line by the player's
/// layout (`+0x100`): B (1) Shoot, Pass, Trick Move, N/A; A and C Pass,
/// Shoot, N/A, Trick Move. Block and rebound are defence's.
pub(crate) fn offence_face(layout: u8) -> [(Icon, usize, &'static str); 4] {
    if layout == 1 {
        [
            (Icon::Cross, 2, "Shoot"),
            (Icon::Circle, 3, "Pass"),
            (Icon::Triangle, 4, "Trick Move"),
            (Icon::Square, 27, "N/A"),
        ]
    } else {
        [
            (Icon::Cross, 3, "Pass"),
            (Icon::Circle, 2, "Shoot"),
            (Icon::Triangle, 27, "N/A"),
            (Icon::Square, 4, "Trick Move"),
        ]
    }
}

/// What a face button does through `layout` (`options::remap`): the
/// `contconfig.txt` line and its words.
pub(crate) fn face_action(layout: u8, physical: u16, offence: bool) -> (usize, &'static str) {
    let logical = crate::options::remap(layout, physical, offence);
    match (offence, logical) {
        (true, b) if b & button::SHOOT != 0 => (2, "Shoot"),
        (true, b) if b & button::PASS != 0 => (3, "Pass"),
        (true, b) if b & button::TRICK != 0 => (4, "Trick Move"),
        (true, b) if b & button::JUMP != 0 => (7, "Block / Rebound"),
        (false, b) if b & button::PASS != 0 => (22, "Change Players"),
        (false, b) if b & button::TRICK != 0 => (23, "Steal"),
        (false, b) if b & button::JUMP != 0 => (7, "Block / Rebound"),
        _ => (27, "N/A"),
    }
}

/// The diagram's pieces for `layout` (0..2).
pub fn pieces(
    look: &Look,
    labels: &mut crate::look::Labels,
    images: &mut Assets<Image>,
    layout: u8,
    player: usize,
    view: u8,
    cursor: usize,
    vibration: bool,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some([icons, pad]) = look.pad.clone() else {
        return out;
    };
    out.push(Piece::new(pad, Rect::new(80.0, 180.0, 336.0, 436.0)));
    let strings = |index, fallback: &str| {
        look.text("contconfig.txt", index)
            .unwrap_or(fallback)
            .to_owned()
    };
    let headers = [
        strings(8, "Player %1N").replace("%1N", &(player + 1).to_string()),
        strings(9 + usize::from(layout), "Layout A"),
        strings([12, 13, 0][usize::from(view.min(2))], "Offense"),
        format!(
            "{}  {}",
            strings(37, "Vibration"),
            strings(
                if vibration { 38 } else { 39 },
                if vibration { "ON" } else { "OFF" }
            )
        ),
        strings(28, "Continue"),
    ];
    for (row, text) in headers.iter().enumerate() {
        let at = match row {
            0..=2 => Vec2::new(190.0, 95.0 + 25.0 * row as f32),
            3 => Vec2::new(450.0, 390.0),
            _ => Vec2::new(520.0, 412.0),
        };
        if let Some(piece) = labels.text(
            look,
            images,
            fonts::LIST,
            text,
            if cursor == row {
                colours::SELECTED
            } else {
                [0x88, 0xa0, 0xbe, 0xff]
            },
            None,
            at,
            Align::Centre,
        ) {
            out.push(piece);
        }
    }
    let mut row = |out: &mut Vec<Piece>,
                   row_icons: &[Icon],
                   index,
                   fallback: &str,
                   y: f32,
                   advanced: bool| {
        let mut x = 328.0;
        for icon in row_icons {
            let source = icon.source();
            let width = 16.0 * source.width() / source.height();
            out.push(Piece::part(
                icons.clone(),
                source,
                Rect::new(x, y, x + width, y + 16.0),
            ));
            x += width + 3.0;
        }
        let text = strings(index, fallback);
        if let Some(piece) = labels.text(
            look,
            images,
            fonts::SMALL,
            &text,
            colours::CREAM,
            None,
            Vec2::new(if advanced { x + 5.0 } else { x.max(369.0) }, y),
            Align::Left,
        ) {
            out.push(piece);
        }
    };
    if view < 2 {
        // The controller widget at (70, 90) (`FUN_0029c578`): its button
        // rows as the pause screen's widget draws them.
        crate::pause::controller_rows(
            &mut out,
            look,
            labels,
            images,
            Vec2::new(70.0, 90.0),
            layout,
            view,
        );
    } else {
        let face = |logical, offense| {
            FACE.iter()
                .find(|&&(_, physical)| {
                    crate::options::remap(layout, physical, offense) & logical != 0
                })
                .map(|&(icon, _)| icon)
                .unwrap_or(Icon::Square)
        };
        let pass = face(button::PASS, true);
        let trick = face(button::TRICK, false);
        let shoot = face(button::SHOOT, true);
        let actions = [
            (vec![trick, Icon::L1, Icon::R1], 15, "Trick Counter"),
            (vec![pass, Icon::L1, Icon::R1], 16, "Off the Heezay"),
            (vec![pass, Icon::L1, Icon::R1], 17, "Back 2 Papa"),
            (vec![pass], 21, "Give 'n Go"),
            (vec![Icon::Select], 19, "Pocket Gamebreaker"),
            (vec![Icon::Select], 20, "Cancel Gamebreaker"),
            (
                vec![shoot, Icon::L1, Icon::L2, Icon::R1, Icon::R2],
                29,
                "Gamebreaker",
            ),
        ];
        for (index, (row_icons, line, fallback)) in actions.iter().enumerate() {
            row(
                &mut out,
                row_icons,
                *line,
                fallback,
                185.0 + 27.0 * index as f32,
                true,
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `FUN_002164a0`: on offence layout A and C read Pass, Shoot, N/A,
    /// Trick Move down Cross, Circle, Triangle, Square, and B Shoot, Pass,
    /// Trick Move, N/A, as the pause screen's capture shows for A.
    #[test]
    fn the_offence_rows_follow_fun_002164a0() {
        let lines = |layout| offence_face(layout).map(|(_, k, _)| k);
        assert_eq!(lines(0), [3, 2, 27, 4]);
        assert_eq!(lines(2), [3, 2, 27, 4]);
        assert_eq!(lines(1), [2, 3, 4, 27]);
    }

    #[test]
    fn each_layout_puts_its_own_buttons() {
        // A: Circle shoots, Cross passes, Square tricks, Triangle blocks.
        let a: Vec<usize> = FACE
            .iter()
            .map(|&(_, b)| face_action(0, b, true).0)
            .collect();
        assert_eq!(a, [7, 2, 3, 4]);
        // B: Circle passes, Cross shoots (blocks on defence), Square
        // blocks, Triangle tricks.
        let b: Vec<usize> = FACE
            .iter()
            .map(|&(_, b)| face_action(1, b, true).0)
            .collect();
        assert_eq!(b, [4, 3, 2, 7]);
        assert_eq!(face_action(1, button::PASS, false).0, 7);
        // Defence: the trick button steals, the pass button switches.
        assert_eq!(face_action(0, button::TRICK, false).0, 23);
        assert_eq!(face_action(0, button::PASS, false).0, 22);
        assert_eq!(face_action(0, button::SHOOT, false).0, 27);
    }
}
