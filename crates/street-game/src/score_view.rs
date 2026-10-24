//! High Scores (class 10, layout 10, backdrop 0x17; `FUN_002d1e80`).
//!
//! The build (`FUN_002d1f08`) puts ten tabs on list 1 (the kind-9 strip at
//! (190, 106), its one node 2 at (320, 106)): row widgets 3000 and
//! 3002..3010 with `score.txt`'s names 0x700 and 0x702..0x70a in font 1,
//! centred on the node's middle; Fastest Rise To Fame (3001) and Pass Outs
//! are never made. The screen opens on Win Percentage (`FUN_00220af8(list,
//! 0xbba)`). L1 and R1 (and left and right) step the tabs round
//! (`FUN_002d3278`), the L1 and R1 icons (kind 29, elements 4 and 6, icons
//! 5 and 6) and the list's arrows (kind 37, 3 and 5) beside them; Cross and
//! Triangle go back to the Main Menu (`FUN_002d30f0`, actions 0x23 and
//! 0x25), Square is Help.
//!
//! Each tab shows its table (`FUN_002d3438` maps the tab to the table,
//! `FUN_002d2398` fills ten row groups 2000..2009 of three font-1 texts: the
//! first 0xff88a0be, the others 0xffdcdcbb):
//! - Street Legends: each row two lines, the names centred at (320, 127.5 +
//!   25.5 * 2k) ("%s \"%s\" %s" with a nickname), then the record
//!   (`score.txt` 24, "%1dW-%2dL") and the share (25, "%1S W%") at 230 and
//!   350, 124.5 + 25.5 * (2k + 1) down;
//! - Win Percentage: the name right-anchored at 280, the record at 300 and
//!   the share at 420, 133 + 24k down;
//! - the single-game tables: the name right-anchored at 310 and the figure
//!   at 330, 133 + 24k down.
//!
//! The help line is the tab's (`score.txt` 13..22).

use bevy::prelude::*;
use street_assets::font::Layout;
use street_sim::records::{GameRecord, Records};

use crate::{
    choose::place,
    look::{self, Look, Piece},
};

/// The screen's layout and backdrop (`FUN_002d7720(.., 10, 0x17)`).
pub const CLASS: u32 = 10;
pub const BACKDROP: usize = 0x17;

/// A tab's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    Legends,
    Win,
    Game(GameRecord),
}

/// A tab: its name and help line in `score.txt`, and its table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tab {
    pub name: usize,
    pub help: usize,
    pub table: Table,
}

/// The tabs in the list's order (3000, 3002..3010; `FUN_002d3438` gives the
/// table, `FUN_002d2398` its name and help).
pub const TABS: [Tab; 10] = [
    Tab {
        name: 0,
        help: 13,
        table: Table::Legends,
    },
    Tab {
        name: 2,
        help: 14,
        table: Table::Win,
    },
    Tab {
        name: 3,
        help: 15,
        table: Table::Game(GameRecord::TrickPoints),
    },
    Tab {
        name: 4,
        help: 16,
        table: Table::Game(GameRecord::Gamebreakers1),
    },
    Tab {
        name: 5,
        help: 17,
        table: Table::Game(GameRecord::Gamebreakers2),
    },
    Tab {
        name: 6,
        help: 18,
        table: Table::Game(GameRecord::Steals),
    },
    Tab {
        name: 7,
        help: 19,
        table: Table::Game(GameRecord::Blocks),
    },
    Tab {
        name: 8,
        help: 20,
        table: Table::Game(GameRecord::AlleyOops),
    },
    Tab {
        name: 9,
        help: 21,
        table: Table::Game(GameRecord::LongRange),
    },
    Tab {
        name: 10,
        help: 22,
        table: Table::Game(GameRecord::Dunks),
    },
];

/// The tab the screen opens on: Win Percentage.
pub const FIRST: usize = 1;

/// The tab `dir` steps from `tab`, round the ring (`FUN_002d3278`).
pub fn step(tab: usize, dir: i32) -> usize {
    (tab as i32 + dir).rem_euclid(TABS.len() as i32) as usize
}

/// The rows' first line (`DAT_003fd638`, `DAT_003fd63c`, set by
/// `FUN_002d35b8`) and their spacing.
const ROW_X: f32 = 310.0;
const ROW_Y: f32 = 133.0;
const ROW_STEP: f32 = 24.0;
/// The row group's second and third texts, right of its point
/// (`FUN_002d1f08`: 20 and 140).
const SECOND: f32 = 20.0;
const THIRD: f32 = 140.0;
/// Street Legends' two lines a row.
const LEGEND_STEP: f32 = 25.5;
/// The texts' colours (`FUN_002d1f08`).
const NAME_COLOUR: [u8; 4] = [0x88, 0xa0, 0xbe, 0xff];
const FIGURE_COLOUR: [u8; 4] = [0xdc, 0xdc, 0xbb, 0xff];
/// The texts' font.
const FONT: usize = 1;
/// A row's texts' vertical anchor: the text widget's top
/// (`FUN_00238a78`'s default).
const TOP: u8 = 0;

/// The share as the table writes it (`FUN_002d2f08`, English): ".950", or
/// "1.000" with no losses; ".000" with no games.
pub fn share_text(wins: u32, losses: u32) -> String {
    let games = wins as f32 + losses as f32;
    if games <= 0.0 {
        return ".000".into();
    }
    if losses == 0 {
        return "1.000".into();
    }
    let thousandths = (wins as f32 / games * 1000.0 + 0.5).floor() as i32;
    format!(".{thousandths:03}")
}

/// A figure as the table shows it, its thousands grouped ("500,000",
/// measured on `high_scores_table2.png`).
pub fn figure_text(value: u32) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// One text of a row: the words, its point and its layout-style anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub words: String,
    pub at: Vec2,
    pub align: u8,
    pub name: bool,
}

/// A tab's texts, as `FUN_002d2398` places them.
pub fn cells(look: &Look, records: &Records, tab: usize) -> Vec<Cell> {
    let mut out = Vec::new();
    let record = |w: u32, l: u32| {
        look.text("score.txt", 24)
            .unwrap_or("%1dW-%2dL")
            .replace("%1d", &w.to_string())
            .replace("%2d", &l.to_string())
    };
    let share = |w: u32, l: u32| {
        look.text("score.txt", 25)
            .unwrap_or("%1S W%%%% ")
            .replace("%1S", &share_text(w, l))
            .replace("%%%%", "%")
            .trim_end()
            .to_owned()
    };
    let Some(t) = TABS.get(tab) else {
        return out;
    };
    match t.table {
        Table::Legends => {
            for (k, l) in records.first_legends().iter().enumerate() {
                let line = 2.0 * k as f32;
                let mut words = l.name.clone();
                if !l.nickname.is_empty() {
                    let (first, last) = l.name.split_once(' ').unwrap_or((&l.name, ""));
                    words = format!("{first} \"{}\" {last}", l.nickname)
                        .trim_end()
                        .to_owned();
                }
                out.push(Cell {
                    words,
                    at: Vec2::new(320.0, ROW_Y + line * LEGEND_STEP + 1.5 - 7.0),
                    align: 1 | TOP,
                    name: true,
                });
                let y = ROW_Y + (line + 1.0) * LEGEND_STEP - 1.5 - 7.0;
                let x = ROW_X - 100.0;
                out.push(Cell {
                    words: record(l.wins, l.losses),
                    at: Vec2::new(x + SECOND, y),
                    align: TOP,
                    name: false,
                });
                out.push(Cell {
                    words: share(l.wins, l.losses),
                    at: Vec2::new(x + THIRD, y),
                    align: TOP,
                    name: false,
                });
            }
        }
        Table::Win => {
            for (k, r) in records.win_rows.iter().enumerate() {
                let at = Vec2::new(ROW_X - 30.0, ROW_Y + k as f32 * ROW_STEP);
                out.push(Cell {
                    words: r.name.clone(),
                    at,
                    align: 2 | TOP,
                    name: true,
                });
                out.push(Cell {
                    words: record(r.wins, r.losses),
                    at: at + Vec2::new(SECOND, 0.0),
                    align: TOP,
                    name: false,
                });
                out.push(Cell {
                    words: share(r.wins, r.losses),
                    at: at + Vec2::new(THIRD, 0.0),
                    align: TOP,
                    name: false,
                });
            }
        }
        Table::Game(g) => {
            for (k, r) in records.games[g.index()].iter().enumerate() {
                let at = Vec2::new(ROW_X, ROW_Y + k as f32 * ROW_STEP);
                out.push(Cell {
                    words: r.name.clone(),
                    at,
                    align: 2 | TOP,
                    name: true,
                });
                out.push(Cell {
                    words: figure_text(r.value),
                    at: at + Vec2::new(SECOND, 0.0),
                    align: TOP,
                    name: false,
                });
            }
        }
    }
    out
}

/// The screen's pieces over its layout: the tab's name on the strip, the
/// shoulder icons and arrows, the table.
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    layout: &Layout,
    records: &Records,
    tab: usize,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    // The tab: the list's lit row on its node (centred, middle).
    if let (Some(t), Some(node)) = (
        TABS.get(tab),
        layout
            .elements
            .iter()
            .find(|e| e.kind == 10 && e.props.get(&33) == Some(&1))
            .and_then(crate::choose::spot),
    ) {
        let words = look.text("score.txt", t.name).unwrap_or("High Scores");
        let mut colour = look::colours::ROW_LIT;
        colour[3] = look::row_pulse(ticks);
        place(
            &mut out,
            labels,
            look,
            images,
            FONT,
            words,
            colour,
            node,
            1 | 4,
        );
    }
    look::shoulder_tabs(&mut out, look, layout, (3, 5), (4, 6));
    for c in cells(look, records, tab) {
        let colour = if c.name { NAME_COLOUR } else { FIGURE_COLOUR };
        place(
            &mut out, labels, look, images, FONT, &c.words, colour, c.at, c.align,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tabs_go_round_from_win_percentage() {
        assert_eq!(TABS[FIRST].table, Table::Win);
        assert_eq!(step(FIRST, -1), 0);
        assert_eq!(step(0, -1), TABS.len() - 1);
        assert_eq!(step(TABS.len() - 1, 1), 0);
        // Steals before Blocks before Alley-oops (`FUN_002d3438`: 3006 is
        // table 7, 3007 table 6, 3008 table 5).
        assert_eq!(TABS[5].table, Table::Game(GameRecord::Steals));
        assert_eq!(TABS[7].table, Table::Game(GameRecord::AlleyOops));
    }

    #[test]
    fn shares_and_figures_read_as_the_tables_write_them() {
        assert_eq!(share_text(95, 5), ".950");
        assert_eq!(share_text(25, 75), ".250");
        assert_eq!(share_text(3, 0), "1.000");
        assert_eq!(share_text(0, 0), ".000");
        assert_eq!(share_text(2, 1), ".667");
        assert_eq!(figure_text(500_000), "500,000");
        assert_eq!(figure_text(4), "4");
        assert_eq!(figure_text(1_234_567), "1,234,567");
    }
}
