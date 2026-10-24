//! Be A Legend's Choose Baller on the game's own layout (class 0x36 = 54,
//! blob 20): the constructor `FUN_002cd930(screen, mode)` (`FUN_002d7720(
//! screen, 0x36, 2)`, backdrop 2; class 0x3d is the same screen with mode
//! 1, My Team's Create A Baller list), build `FUN_002ce270`, the rows'
//! refresh `FUN_002ce618`, update `FUN_002cdeb8`, input `FUN_002cdce8` and
//! the dialogs `FUN_002cdf10` / `FUN_002cd9c8`. The trace is in
//! `docs/FRONTEND_RECOVERY.md`, "Choose Baller".
//!
//! The build registers thirty row widgets on list 0x46 in groups of three
//! (`FUN_0021f508`: widget `k`, then `k + 10`, then `k + 20`), so career
//! `k`'s number, name and progress sit on the nodes of prop 34 `3k`,
//! `3k + 1` and `3k + 2`; the list moves three nodes at a time (`+0xfa`
//! 3), lighting the three together, and only the name has the row
//! widget's stars (`FUN_0021c628(name, 1)`). An empty slot reads
//! "<empty>" with a blue star in the progress column (elements 90..99,
//! `XACOMMON` sheet 0, image 5, frame 0), and when it is highlighted
//! "CREATE NEW BALLER" and "NEW" in place of the star. A career reads its
//! baller's name and "%1d/%2d" (its progress and the total, `FUN_001eb6e0`,
//! `FUN_001eb798`) or "Complete" once every event is done (`FUN_001eb5f8`).
//! The help line follows the highlighted slot: "Create New Baller.",
//! "Enter Be A Legend Mode with %1s." or, in mode 1, "Edit Baller.".

use bevy::prelude::*;
use street_assets::font::Layout;
use street_sim::story::{self as sim, Story};

use crate::{
    look::{self, Align, Look, Piece, RowSpot},
    setup::Disc,
};

/// The screen's layout class and backdrop (`FUN_002d7720(.., 0x36, 2)`).
pub const CLASS: u32 = 54;
pub const BACKDROP: usize = 2;
/// The list the rows are registered on (element 0x46).
pub const LIST: u32 = 70;
/// The slots (`FUN_002ce270`'s loop to 10).
pub const SLOTS: usize = 10;
/// The first progress star's element (`0x5a + k`).
const STAR_FIRST: u32 = 90;

/// Choose Baller's texts (`storymainmenu.txt`; the title, element 100, is
/// 0x1900, "Create A Baller" 0x1901 in mode 1).
pub mod text {
    pub const COMPLETE: u16 = 0x1902;
    pub const PROGRESS: u16 = 0x1903;
    pub const CREATE_NEW: u16 = 0x1904;
    pub const EMPTY: u16 = 0x1905;
    pub const NEW: u16 = 0x1906;
    pub const HELP_ENTER: u16 = 0x1907;
    pub const HELP_EDIT: u16 = 0x1908;
    pub const HELP_CREATE: u16 = 0x1909;
    pub const PREMADE: u16 = 0x190a;
    pub const MALE: u16 = 0x190c;
    pub const FEMALE: u16 = 0x190d;
    pub const EDIT_BALLER: u16 = 0x190e;
    pub const EDIT_MOVES: u16 = 0x190f;
    pub const CONTINUE: u16 = 0x1910;
    /// `common.txt` 5.
    pub const CANCEL: u16 = 0x0005;
}

/// One slot as the rows show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    Empty,
    Career {
        name: String,
        points: u32,
        complete: bool,
    },
}

/// The baller's name as `FUN_001eb4d8` builds it: the record's first name
/// (field 2), a space and its last name (field 1) when there is one; the
/// last name alone when the first is empty.
pub fn baller_name(first: &str, last: &str) -> String {
    if first.is_empty() {
        last.to_owned()
    } else if last.is_empty() || last.starts_with(' ') {
        first.to_owned()
    } else {
        format!("{first} {last}")
    }
}

/// The ten slots of the save's careers.
pub fn slots(story: &Story, disc: &Disc) -> Vec<Slot> {
    (0..SLOTS)
        .map(|k| match story.career_at(k) {
            None => Slot::Empty,
            Some(c) => {
                let name = c
                    .crew
                    .first()
                    .and_then(|&r| disc.players.get(usize::from(r)))
                    .map_or_else(String::new, |p| baller_name(&p.first_name, &p.last_name));
                let (points, complete) = story.career_standing(k).unwrap_or_default();
                Slot::Career {
                    name,
                    points,
                    complete,
                }
            }
        })
        .collect()
}

/// My Team's Create A Baller (class 0x3d, `FUN_002cd930(screen, 1)` over
/// the same layout): the ten slots, a created baller's with its name. The
/// original's slots are the careers' ballers, each with its progress; the
/// rewrite keeps its six created ballers apart from the careers, so a
/// created one reads "0/500" and the last four stay empty.
pub fn baller_slots(story: &Story, disc: &Disc) -> Vec<Slot> {
    (0..SLOTS)
        .map(|k| {
            let made = k < street_sim::story::BALLER_SLOTS
                && story.ballers.iter().any(|b| usize::from(b.slot) == k);
            match disc
                .players
                .get(usize::from(street_sim::story::BALLER_RECORDS) + k)
            {
                Some(p) if made => Slot::Career {
                    name: baller_name(&p.first_name, &p.last_name),
                    points: 0,
                    complete: false,
                },
                _ => Slot::Empty,
            }
        })
        .collect()
}

/// The words of one slot's three widgets: its number ("%d.", the format at
/// `0x491778`), name and progress (`None`: hidden, the star shows), as
/// `FUN_002ce270` and `FUN_002ce618` set them.
pub fn row_texts(
    look: &Look,
    k: usize,
    slot: &Slot,
    lit: bool,
) -> (String, String, Option<String>) {
    let word = |id: u16, plain: &str| look.text_id(id).unwrap_or(plain).to_owned();
    let number = format!("{}.", k + 1);
    match slot {
        Slot::Empty if lit => (
            number,
            word(text::CREATE_NEW, "CREATE NEW BALLER"),
            Some(word(text::NEW, "NEW")),
        ),
        Slot::Empty => (number, word(text::EMPTY, "<empty>"), None),
        Slot::Career {
            name,
            points,
            complete,
        } => {
            let progress = if *complete {
                word(text::COMPLETE, "Complete")
            } else {
                word(text::PROGRESS, "%1d/%2d")
                    .replace("%1d", &points.to_string())
                    .replace("%2d", &sim::overall_total().to_string())
            };
            (number, name.clone(), Some(progress))
        }
    }
}

/// The help line (element 85, `FUN_002ce618`): "Create New Baller." on an
/// empty slot; on a career "Enter Be A Legend Mode with %1s." (mode 0) or
/// "Edit Baller." (mode 1, My Team's list).
pub fn help(look: &Look, slot: &Slot, create_mode: bool) -> String {
    match slot {
        Slot::Empty => look
            .text_id(text::HELP_CREATE)
            .unwrap_or("Create New Baller.")
            .to_owned(),
        Slot::Career { .. } if create_mode => look
            .text_id(text::HELP_EDIT)
            .unwrap_or("Edit Baller.")
            .to_owned(),
        Slot::Career { name, .. } => look
            .text_id(text::HELP_ENTER)
            .unwrap_or("Enter Be A Legend Mode with %1s.")
            .replace("%1s", name),
    }
}

/// The dialog a confirm opens (`FUN_002cdf10`): on an empty slot its
/// words are none and its rows Male, Female, Premade, Cancel (dialog 0x56,
/// Male lit); on a career its words are the baller's name and its rows
/// Continue Career, Edit Baller, Edit Moves, Cancel (dialog 0x52), or in
/// mode 1 Edit Baller, Edit Moves, Cancel (0x58).
pub fn dialog_rows(look: &Look, slot: &Slot, create_mode: bool) -> (String, Vec<String>) {
    let word = |id: u16, plain: &str| look.text_id(id).unwrap_or(plain).to_owned();
    let cancel = word(text::CANCEL, "Cancel");
    match slot {
        Slot::Empty => (
            String::new(),
            vec![
                word(text::MALE, "Male"),
                word(text::FEMALE, "Female"),
                word(text::PREMADE, "Premade"),
                cancel,
            ],
        ),
        Slot::Career { name, .. } if create_mode => (
            name.clone(),
            vec![
                word(text::EDIT_BALLER, "Edit Baller"),
                word(text::EDIT_MOVES, "Edit Moves"),
                cancel,
            ],
        ),
        Slot::Career { name, .. } => (
            name.clone(),
            vec![
                word(text::CONTINUE, "Continue Career"),
                word(text::EDIT_BALLER, "Edit Baller"),
                word(text::EDIT_MOVES, "Edit Moves"),
                cancel,
            ],
        ),
    }
}

/// List 0x46's nodes in prop-34 order.
fn nodes(layout: &Layout) -> Vec<Vec2> {
    let mut n: Vec<(u32, Vec2)> = layout
        .elements
        .iter()
        .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&LIST))
        .filter_map(|e| {
            let (x, y) = e.position()?;
            Some((*e.props.get(&34)?, Vec2::new(f32::from(x), f32::from(y))))
        })
        .collect();
    n.sort_by_key(|n| n.0);
    n.into_iter().map(|n| n.1).collect()
}

/// Where widget `id` draws: on its node (`FUN_00222420`), in its template's
/// font and anchor.
fn spot(layout: &Layout, nodes: &[Vec2], node: usize, id: u32) -> Option<RowSpot> {
    let at = *nodes.get(node)?;
    let e = layout
        .elements
        .iter()
        .find(|e| e.kind == 19 && e.id == id)?;
    Some(RowSpot {
        at,
        font: e.props.get(&13).map_or(look::fonts::LIST, |&f| f as usize),
        align: match e.align & 3 {
            1 => Align::Centre,
            2 => Align::Right,
            _ => Align::Left,
        },
        valign: (e.align >> 2) & 3,
    })
}

/// The rows' pieces: `cursor` the highlighted slot.
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    slots: &[Slot],
    cursor: usize,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(layout) = look.layouts.iter().find(|l| l.class == CLASS) else {
        return out;
    };
    let nodes = nodes(layout);
    let mut stars = Vec::new();
    let (star, star_size) = look.bullet_star();
    for (k, slot) in slots.iter().enumerate().take(SLOTS) {
        let lit = k == cursor;
        let (number, name, progress) = row_texts(look, k, slot, lit);
        let widgets = [
            (3 * k, k as u32, Some(number), false),
            (3 * k + 1, k as u32 + 10, Some(name), true),
            (3 * k + 2, k as u32 + 20, progress.clone(), false),
        ];
        for (node, id, words, with_stars) in widgets {
            let (Some(words), Some(spot)) = (words, spot(layout, &nodes, node, id)) else {
                continue;
            };
            look::screen_row(
                &mut out, &mut stars, labels, look, images, spot, &words, lit, with_stars, ticks,
            );
        }
        // The progress column's star while the slot shows no progress.
        if progress.is_none()
            && let Some((x, y)) = layout
                .elements
                .iter()
                .find(|e| e.kind == 3 && e.id == STAR_FIRST + k as u32)
                .and_then(|e| e.position())
        {
            let middle = Vec2::new(f32::from(x), f32::from(y));
            out.push(Piece::new(
                star.clone(),
                Rect::from_center_size(middle, star_size),
            ));
        }
    }
    out.extend(stars);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_is_the_first_name_then_the_last() {
        assert_eq!(baller_name("Anthony", ""), "Anthony");
        assert_eq!(baller_name("Biggie", "Little"), "Biggie Little");
        assert_eq!(baller_name("", "Stretch"), "Stretch");
        // `FUN_001eb4d8` skips a last name starting with a space.
        assert_eq!(baller_name("Bo", " Jr"), "Bo");
    }

    #[test]
    fn an_empty_slot_shows_its_star_until_it_is_highlighted() {
        let look = Look::empty();
        let (n, name, progress) = row_texts(&look, 0, &Slot::Empty, false);
        assert_eq!(
            (n.as_str(), name.as_str(), progress),
            ("1.", "<empty>", None)
        );
        let (n, name, progress) = row_texts(&look, 9, &Slot::Empty, true);
        assert_eq!(n, "10.");
        assert_eq!(name, "CREATE NEW BALLER");
        assert_eq!(progress.as_deref(), Some("NEW"));
    }

    #[test]
    fn a_career_reads_its_progress_out_of_five_hundred_or_complete() {
        let look = Look::empty();
        let career = |points, complete| Slot::Career {
            name: "Anthony".into(),
            points,
            complete,
        };
        let (_, name, progress) = row_texts(&look, 0, &career(17, false), false);
        assert_eq!(name, "Anthony");
        assert_eq!(progress.as_deref(), Some("17/500"));
        let (_, _, progress) = row_texts(&look, 0, &career(500, true), true);
        assert_eq!(progress.as_deref(), Some("Complete"));
        assert_eq!(
            help(&look, &career(0, false), false),
            "Enter Be A Legend Mode with Anthony."
        );
        assert_eq!(help(&look, &career(0, false), true), "Edit Baller.");
        assert_eq!(help(&look, &Slot::Empty, false), "Create New Baller.");
    }

    #[test]
    fn the_dialogs_rows_follow_the_slot() {
        let look = Look::empty();
        let (words, rows) = dialog_rows(&look, &Slot::Empty, false);
        assert!(words.is_empty());
        assert_eq!(rows, ["Male", "Female", "Premade", "Cancel"]);
        let career = Slot::Career {
            name: "Anthony".into(),
            points: 0,
            complete: false,
        };
        let (words, rows) = dialog_rows(&look, &career, false);
        assert_eq!(words, "Anthony");
        assert_eq!(
            rows,
            ["Continue Career", "Edit Baller", "Edit Moves", "Cancel"]
        );
        let (_, rows) = dialog_rows(&look, &career, true);
        assert_eq!(rows, ["Edit Baller", "Edit Moves", "Cancel"]);
    }
}
