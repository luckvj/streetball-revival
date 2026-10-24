//! The front end's Help overlay (`FUN_00250000`, its own screen class
//! 0x28's "overlay"): while Square is held on a screen with a Help legend,
//! a panel lists the screen's buttons and what each does.
//!
//! - The rows are the screen class's entries in the table at `0x46fbc8`
//!   (class, icon id, text id; `help.txt`, file 2), in order.
//! - Each row is an icon (the record at `0x4702a0 + id * 0x1c`: `XACOMMON`
//!   sheet 1, image 0, its frame; id 20 is L1 and R1 side by side, records
//!   8 and 6) and a text (`FUN_0023aa68`: font 1, 0xff98a8b9, wrapped at
//!   220) 40 right of the icon's middle, on the row's middle.
//! - A row is as tall as its icon or its text; rows stand 10 apart (the
//!   spare height `DAT_003f6b68` shared out, 0 on the disc, so the floor of
//!   10 holds); the rows are centred on the overlay's point, the icons'
//!   half-width, 40 and the widest text across (`0x250634`).
//! - The panel (`FUN_00211278(.., 0x15, 0x1c)`) is fitted 30 round the rows
//!   (`FUN_00212008(panel, 30.0, ..)`), its body opaque as the dialogs';
//!   "Help" (`help.txt` 171, font 5, `FUN_0020f6c0`) sits on its corner;
//!   the screen behind is dimmed.
//!
//! Measured on the original's Main Menu, Play It and Difficulty with Square
//! held (`help_main.png`, `help_play_it.png`, `help_difficulty.png`): the
//! overlay's point (its run-time `DAT_003f6b70`) and the title's place.

use bevy::prelude::*;

use crate::boards::{Gm, Panels};
use crate::look::{self, Align, Labels, Look, Piece};

/// The table at `0x46fbc8`: (screen class, icon id, text id), its 291 rows
/// to the class -1 that ends it (class 0, the pause, among them).
pub const TABLE: [(u32, u8, u16); 291] = [
    (3, 18, 613),
    (3, 2, 665),
    (5, 18, 613),
    (5, 2, 665),
    (5, 5, 545),
    (5, 10, 545),
    (105, 18, 613),
    (105, 2, 665),
    (105, 5, 561),
    (105, 10, 545),
    (102, 18, 613),
    (102, 2, 665),
    (102, 5, 545),
    (102, 10, 545),
    (103, 18, 613),
    (103, 2, 665),
    (103, 5, 562),
    (103, 10, 545),
    (104, 18, 613),
    (104, 2, 665),
    (104, 5, 562),
    (104, 10, 545),
    (6, 18, 613),
    (6, 2, 665),
    (6, 5, 545),
    (6, 10, 545),
    (11, 18, 613),
    (11, 2, 665),
    (11, 5, 567),
    (11, 10, 545),
    (12, 18, 613),
    (12, 2, 665),
    (12, 5, 566),
    (12, 10, 545),
    (26, 18, 622),
    (26, 2, 668),
    (26, 5, 543),
    (26, 10, 545),
    (29, 18, 616),
    (29, 2, 667),
    (29, 5, 552),
    (29, 10, 545),
    (33, 18, 597),
    (33, 2, 656),
    (33, 5, 537),
    (33, 10, 545),
    (39, 18, 625),
    (39, 2, 521),
    (39, 5, 530),
    (39, 10, 545),
    (43, 18, 616),
    (43, 2, 667),
    (43, 5, 552),
    (43, 10, 545),
    (25, 18, 622),
    (25, 2, 668),
    (25, 5, 554),
    (25, 10, 545),
    (41, 18, 612),
    (41, 2, 664),
    (41, 5, 539),
    (41, 10, 545),
    (41, 9, 690),
    (42, 18, 612),
    (42, 2, 664),
    (42, 5, 543),
    (42, 10, 545),
    (42, 9, 690),
    (54, 18, 593),
    (54, 2, 654),
    (54, 5, 552),
    (54, 10, 545),
    (61, 18, 613),
    (61, 2, 572),
    (61, 5, 555),
    (61, 10, 545),
    (56, 18, 613),
    (56, 2, 572),
    (56, 5, 556),
    (56, 10, 545),
    (56, 9, 690),
    (57, 18, 613),
    (57, 2, 572),
    (57, 5, 546),
    (57, 8, 669),
    (57, 6, 669),
    (57, 10, 545),
    (49, 2, 517),
    (49, 5, 548),
    (49, 10, 545),
    (50, 2, 526),
    (50, 5, 548),
    (50, 10, 545),
    (51, 2, 517),
    (51, 5, 548),
    (51, 10, 545),
    (53, 18, 616),
    (53, 2, 667),
    (53, 5, 560),
    (53, 8, 677),
    (53, 6, 680),
    (53, 10, 545),
    (98, 18, 616),
    (98, 2, 667),
    (98, 5, 542),
    (98, 10, 545),
    (24, 18, 623),
    (24, 11, 584),
    (24, 8, 679),
    (24, 6, 681),
    (24, 10, 545),
    (20, 18, 591),
    (20, 2, 634),
    (20, 11, 584),
    (20, 8, 679),
    (20, 6, 681),
    (20, 10, 545),
    (21, 18, 671),
    (21, 2, 553),
    (21, 5, 553),
    (21, 10, 545),
    (22, 18, 671),
    (22, 2, 553),
    (22, 5, 553),
    (22, 4, 586),
    (22, 10, 545),
    (23, 18, 671),
    (23, 2, 675),
    (23, 5, 553),
    (23, 10, 545),
    (40, 18, 588),
    (40, 2, 665),
    (40, 5, 545),
    (40, 11, 19),
    (40, 4, 578),
    (40, 10, 545),
    (97, 18, 613),
    (97, 2, 665),
    (97, 5, 551),
    (97, 10, 545),
    (55, 18, 592),
    (55, 5, 646),
    (55, 4, 19716),
    (55, 10, 545),
    (55, 11, 576),
    (48, 18, 613),
    (48, 2, 689),
    (48, 5, 551),
    (48, 11, 19),
    (48, 10, 545),
    (10, 18, 670),
    (10, 2, 545),
    (10, 5, 545),
    (10, 8, 678),
    (10, 6, 682),
    (10, 10, 545),
    (0, 18, 629),
    (0, 2, 630),
    (0, 11, 631),
    (77, 18, 622),
    (77, 2, 668),
    (63, 18, 589),
    (63, 2, 652),
    (63, 5, 552),
    (63, 20, 687),
    (63, 12, 648),
    (63, 10, 545),
    (65, 18, 590),
    (65, 2, 653),
    (65, 5, 552),
    (65, 6, 514),
    (65, 12, 648),
    (65, 10, 545),
    (66, 18, 603),
    (66, 2, 659),
    (66, 5, 552),
    (66, 20, 688),
    (66, 12, 648),
    (66, 10, 545),
    (67, 18, 609),
    (67, 2, 662),
    (67, 5, 552),
    (67, 20, 688),
    (67, 12, 648),
    (67, 10, 545),
    (68, 18, 601),
    (68, 2, 658),
    (68, 5, 552),
    (68, 20, 688),
    (68, 12, 648),
    (68, 10, 545),
    (69, 18, 599),
    (69, 2, 657),
    (69, 5, 552),
    (69, 20, 688),
    (69, 12, 648),
    (69, 10, 545),
    (70, 18, 594),
    (70, 2, 655),
    (70, 5, 552),
    (70, 20, 688),
    (70, 12, 648),
    (70, 10, 545),
    (71, 18, 619),
    (71, 2, 641),
    (71, 5, 552),
    (71, 20, 688),
    (71, 12, 648),
    (71, 10, 545),
    (72, 18, 587),
    (72, 2, 651),
    (72, 5, 552),
    (72, 8, 513),
    (72, 12, 648),
    (72, 10, 545),
    (73, 2, 512),
    (73, 5, 552),
    (73, 12, 648),
    (73, 4, 581),
    (73, 10, 545),
    (80, 18, 594),
    (80, 2, 635),
    (80, 11, 584),
    (80, 20, 687),
    (80, 12, 648),
    (80, 4, 582),
    (80, 10, 545),
    (81, 18, 618),
    (81, 2, 638),
    (81, 11, 584),
    (81, 20, 687),
    (81, 12, 647),
    (81, 4, 582),
    (81, 10, 545),
    (82, 18, 601),
    (82, 2, 658),
    (82, 11, 584),
    (82, 20, 687),
    (82, 12, 648),
    (82, 4, 582),
    (82, 10, 545),
    (83, 18, 599),
    (83, 2, 657),
    (83, 11, 584),
    (83, 20, 687),
    (83, 12, 648),
    (83, 4, 582),
    (83, 10, 545),
    (84, 18, 620),
    (84, 2, 641),
    (84, 11, 584),
    (84, 20, 687),
    (84, 12, 648),
    (84, 4, 582),
    (84, 10, 545),
    (85, 18, 611),
    (85, 2, 570),
    (85, 11, 584),
    (85, 20, 687),
    (85, 4, 633),
    (85, 10, 545),
    (101, 18, 611),
    (101, 2, 633),
    (101, 5, 552),
    (101, 20, 687),
    (101, 10, 545),
    (86, 18, 611),
    (86, 2, 570),
    (86, 5, 5),
    (86, 4, 633),
    (86, 10, 545),
    (96, 18, 616),
    (96, 2, 667),
    (96, 5, 674),
    (79, 18, 614),
    (79, 2, 665),
    (79, 5, 643),
    (79, 10, 643),
    (88, 2, 577),
    (88, 5, 558),
    (88, 10, 545),
    (89, 2, 577),
    (89, 5, 558),
    (89, 10, 545),
    (94, 2, 577),
    (94, 5, 558),
    (94, 10, 545),
    (90, 2, 577),
    (90, 5, 545),
    (90, 10, 545),
    (91, 2, 577),
];

/// The icon records' frames (`0x4702a0`, records 0..18: `+0x10`), in
/// `XACOMMON` sheet 1, image 0.
const FRAMES: [usize; 19] = [
    4, 4, 3, 1, 0, 2, 6, 7, 5, 8, 0x12, 9, 0x11, 0x11, 0x11, 0x10, 0x13, 0x14, 4,
];
/// Id 20's two icons (records 8 and 6: L1, R1).
const SHOULDERS: u8 = 20;
const TEXT_FONT: usize = 1;
const TEXT_COLOUR: [u8; 4] = [0x98, 0xa8, 0xb9, 0xff];
const TEXT_WRAP: f32 = 220.0;
/// The text's left from the icon's middle, the rows' gap, the panel's
/// margin round them.
const TEXT_AT: f32 = 40.0;
const ROW_GAP: f32 = 10.0;
const MARGIN: f32 = 30.0;
/// The overlay's point (measured: the rows' box is centred on it on all
/// three grabs).
const ORIGIN: Vec2 = Vec2::new(321.5, 239.5);
/// "Help" (font 5): its left and its foot from the panel's corner
/// (measured).
const TITLE_FONT: usize = 5;
const TITLE_AT: Vec2 = Vec2::new(-1.5, -13.5);
/// How dark the screen behind goes (measured: it keeps 0.33..0.45 of its
/// light).
const DIM: f32 = 0.6;

/// A class's rows: (icon id, text id).
pub fn rows(class: u32) -> Vec<(u8, u16)> {
    TABLE
        .iter()
        .filter(|r| r.0 == class)
        .map(|&(_, icon, text)| (icon, text))
        .collect()
}

/// The frames a row's icon id shows.
fn frames_of(id: u8) -> Vec<usize> {
    if id == SHOULDERS {
        vec![FRAMES[8], FRAMES[6]]
    } else {
        FRAMES
            .get(usize::from(id))
            .map(|&f| vec![f])
            .unwrap_or_default()
    }
}

/// `text` wrapped at `width`. The rows' text widget is given its text and
/// its 220 (`FUN_00239d80`, `FUN_0023b710` at `0x2503cc`) before its font
/// (`FUN_002399f8` at `0x2503e0`), so it breaks its lines measuring them in
/// the executable's own font ([`Look::system_font`], narrower than font 1):
/// "Back Up To Previous Screen" stays one line, 241 pixels wide in font 1.
fn wrap(look: &Look, font: usize, text: &str, width: f32) -> Vec<String> {
    let Some(f) = look.system_font.as_ref().or(look.fonts.get(font)) else {
        return vec![text.to_owned()];
    };
    let mut out = Vec::new();
    for paragraph in text.replace("\\n", "\n").split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let tried = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && f.measure(&tried) as f32 > width {
                out.push(std::mem::take(&mut line));
                line = word.to_owned();
            } else {
                line = tried;
            }
        }
        out.push(line);
    }
    out
}

/// The overlay's pieces over a screen of `class` (none when the class has
/// no rows).
pub fn pieces(
    look: &Look,
    gm: &Gm,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    panels: &mut Panels,
    class: u32,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let rows = rows(class);
    let (Some(font), Some((atlas, frames))) = (look.fonts.get(TEXT_FONT), look.icons.as_ref())
    else {
        return out;
    };
    if rows.is_empty() {
        return out;
    }
    // Each row's icons, words and height.
    struct Row {
        icons: Vec<Rect>,
        lines: Vec<String>,
        width: f32,
        height: f32,
    }
    let rows: Vec<Row> = rows
        .iter()
        .map(|&(icon, text)| {
            let icons: Vec<Rect> = frames_of(icon)
                .into_iter()
                .filter_map(|f| frames.get(f).copied())
                .collect();
            let lines = wrap(
                look,
                TEXT_FONT,
                look.text_full(text).unwrap_or(""),
                TEXT_WRAP,
            );
            // The row's width as the overlay lays it out: its text's, no
            // more than the 220 (`0x2504fc..0x250514`).
            let width = lines
                .iter()
                .map(|l| font.measure(l) as f32)
                .fold(0.0, f32::max)
                .min(TEXT_WRAP);
            let text_h =
                font.text_height as f32 + font.line_height as f32 * (lines.len() as f32 - 1.0);
            let icon_h = icons.iter().map(|r| r.height()).fold(0.0, f32::max);
            Row {
                icons,
                lines,
                width,
                height: icon_h.max(text_h),
            }
        })
        .collect();
    // The widest icon widget's width: L1 and R1's pair is one widget with
    // the second icon its child, so it counts as its first icon's
    // (`0x25052c..0x250540`).
    let icon_w = rows
        .iter()
        .filter_map(|r| r.icons.first().map(|i| i.width()))
        .fold(0.0, f32::max);
    let text_w = rows.iter().map(|r| r.width).fold(0.0, f32::max);
    let total = rows.iter().map(|r| r.height).sum::<f32>() + ROW_GAP * (rows.len() - 1) as f32;
    let across = icon_w / 2.0 + TEXT_AT + text_w;
    let x0 = ORIGIN.x - across / 2.0 + icon_w / 2.0;
    let top = ORIGIN.y - total / 2.0;
    // The screen behind, dimmed; the panel; its title.
    let mut dim = Piece::new(gm.white.clone(), Rect::new(0.0, 0.0, 640.0, 480.0));
    dim.colour = Color::srgba(0.0, 0.0, 0.0, look::black_alpha(DIM));
    out.push(dim);
    let panel = [
        ORIGIN.x - across / 2.0 - MARGIN,
        top - MARGIN,
        across + 2.0 * MARGIN,
        total + 2.0 * MARGIN,
    ];
    out.push(panels.get(panel, 0xff, gm, images));
    if let Some(f) = look.fonts.get(TITLE_FONT) {
        let foot = Vec2::new(panel[0], panel[1]) + TITLE_AT;
        let at = Vec2::new(foot.x, foot.y - f.text_height as f32);
        if let Some(p) = labels.text(
            look,
            images,
            TITLE_FONT,
            look.text("help.txt", 171).unwrap_or("Help"),
            look::colours::PALE,
            Some(look::colours::COOL),
            at,
            Align::Left,
        ) {
            out.push(p);
        }
    }
    // The rows.
    let mut y = top;
    for row in &rows {
        let middle = y + row.height / 2.0;
        // Two icons stand either side of the middle by a quarter of both
        // their widths (`0x250830`).
        let quarter = row.icons.iter().map(|i| i.width()).sum::<f32>() / 4.0;
        for (k, icon) in row.icons.iter().enumerate() {
            let dx = match (row.icons.len(), k) {
                (1, _) => 0.0,
                (_, 0) => -quarter,
                _ => quarter,
            };
            out.push(Piece::part(
                atlas.clone(),
                *icon,
                Rect::from_center_size(Vec2::new(x0 + dx, middle), icon.size()),
            ));
        }
        let text = row.lines.join("\n");
        let mut w = crate::hud::Writer {
            labels,
            look,
            images,
        };
        w.text(
            &mut out,
            &text,
            crate::hud::TextStyle::plain(TEXT_FONT, TEXT_COLOUR),
            Vec2::new(x0 + TEXT_AT, middle),
            4,
            1.0,
        );
        y += row.height + ROW_GAP;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_class_lists_its_buttons_in_the_tables_order() {
        assert_eq!(rows(3), [(18, 613), (2, 665)]);
        assert_eq!(rows(5).len(), 4);
        assert_eq!(rows(41).last(), Some(&(9, 690)));
        // The whole table, past the pause's class 0: Options (40), the
        // wizard's Build panel (65) and the editor's outfit panel (80), as
        // `help_wizard_build.png` and `help_editor_outfit.png` show them.
        assert_eq!(TABLE.len(), 291);
        assert_eq!(rows(40).len(), 6);
        assert_eq!(
            rows(65),
            [
                (18, 590),
                (2, 653),
                (5, 552),
                (6, 514),
                (12, 648),
                (10, 545)
            ]
        );
        assert_eq!(rows(80).len(), 7);
        assert!(rows(1).is_empty());
        assert_eq!(frames_of(2), [3]);
        assert_eq!(frames_of(18), [4]);
        assert_eq!(frames_of(SHOULDERS), [5, 6]);
    }
}
