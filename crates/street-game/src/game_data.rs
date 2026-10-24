//! Options → Game Data (class 97, layout 97, backdrop 0x1c; `FUN_00298eb0`).
//!
//! `FUN_00299600` registers six rows on list 9 (Save, Load, the three
//! deletes, Exit; no stars) and makes the memory card widget 0x3c8c
//! (`FUN_002294e0`) that runs every row's work: `FUN_00299110` hands it
//! Save and Load (`+0x16c`, `+0x174`, 0x13f) and the deletes (`+0x17c`
//! with 8 names, 0x20 teams, 0x10 ballers); Exit and Triangle go back to
//! Options (0x28). The help line follows the row (`FUN_00299480`:
//! `gamedata.txt` 6..11).
//!
//! Save and Load are the card widget's own work, its checks, questions and
//! messages as the original shows them, the rewrite's file standing for
//! the card ([`crate::card`]). A delete with nothing to
//! delete says so (`delete.txt` 15..17, OK, `FUN_002278c0`); otherwise
//! the card's list dialog (`FUN_00258a48`: its title 0x2e00 / 0x2e02 /
//! 0x2e01, a row a name in font 2, a baller's as "%d.  %s") picks one,
//! "Are you sure ...?" (12..14) asks on NO, and "Delete successful." (6,
//! a message alone for 2 s) ends it. A baller on a created team asks first
//! to delete the team (18). The widget's questions and notices stand in
//! the card's own box ([`crate::card::box_pieces`]); leaving Game Data
//! after a delete saves to the card (`+0xf0`).

use crate::look::Look;

/// The screen's layout.
pub const CLASS: u32 = 97;

/// The rows' words (`FUN_002997e0`: `gamedata.txt` 1..5, then
/// `common.txt`'s Exit).
pub fn rows(look: &Look) -> Vec<String> {
    let mut out: Vec<String> = [
        (1, "Save"),
        (2, "Load"),
        (3, "Delete Name"),
        (4, "Delete Created Team"),
        (5, "Delete Created Baller"),
    ]
    .iter()
    .map(|&(k, or)| look.text("gamedata.txt", k).unwrap_or(or).to_owned())
    .collect();
    out.push(look.text("common.txt", 19).unwrap_or("Exit").to_owned());
    out
}

/// The delete lists' titles in `delete.txt` (names, teams, ballers:
/// `FUN_00258a48`'s 0x2e00, 0x2e02, 0x2e01).
pub fn list_title(look: &Look, kind: usize) -> String {
    let (k, or) = match kind {
        0 => (0, "Delete Name"),
        1 => (2, "Delete Team"),
        _ => (1, "Delete Baller"),
    };
    look.text("delete.txt", k).unwrap_or(or).to_owned()
}

/// A question or notice over Game Data's deletes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Nothing of the kind to delete.
    Nothing(usize),
    /// "Are you sure ...?" for an entry of a kind.
    Sure {
        kind: usize,
        row: usize,
        name: String,
    },
    /// The baller is on created teams: delete them first?
    OnTeam {
        row: usize,
        name: String,
        teams: Vec<String>,
    },
}

impl Ask {
    /// Whether its rows are Yes and No (else one OK).
    pub fn asks(&self) -> bool {
        matches!(self, Ask::Sure { .. } | Ask::OnTeam { .. })
    }

    /// Its lit row as it opens: the questions on NO (`FUN_002278c0`'s
    /// `FUN_0025a880(.., 1)`), the notice on its OK.
    pub fn first_row(&self) -> usize {
        usize::from(self.asks())
    }

    /// Its words, with the game's line breaks.
    pub fn words(&self, look: &Look) -> String {
        let text = |file: &str, k: usize, or: &str| look.text(file, k).unwrap_or(or).to_owned();
        match self {
            Ask::Nothing(kind) => text(
                "delete.txt",
                15 + kind.min(&2),
                "You have to\\ncreate a new\\nuser name before\\none can be\\ndeleted.",
            ),
            Ask::Sure { kind, name, .. } => text(
                "delete.txt",
                match kind {
                    0 => 12,
                    1 => 14,
                    _ => 13,
                },
                "Are you sure?",
            )
            .replace("%1s", name),
            Ask::OnTeam { name, teams, .. } => {
                let teams = teams
                    .iter()
                    .map(|t| format!(" {t}"))
                    .collect::<Vec<_>>()
                    .join(",");
                text(
                    "delete.txt",
                    18,
                    "%1s\\nis on the Created Team%2s.\\nThe Created Team must be deleted first.\\n\\nDo you want to delete%3s?",
                )
                .replace("%1s", name)
                .replace("%2s", &teams)
                .replace("%3s", &teams)
            }
        }
    }

    /// Its rows: YES and NO, or OK (`memcard_ps2.txt` 5..7).
    pub fn rows(&self, look: &Look) -> Vec<String> {
        let word = |k: usize, or: &str| look.text("memcard_ps2.txt", k).unwrap_or(or).to_owned();
        if self.asks() {
            vec![word(5, "YES"), word(6, "NO")]
        } else {
            vec![word(7, "OK")]
        }
    }
}

/// A delete list's row for a created baller (`FUN_00258a48`'s "%d.  %s"
/// of the slot and the name).
pub fn baller_row(slot: usize, first: &str, last: &str) -> String {
    let name = if last.trim().is_empty() {
        first.to_owned()
    } else {
        format!("{first} {last}")
    };
    format!("{}.  {name}", slot + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_baller_row_counts_from_one() {
        assert_eq!(baller_row(0, "PLAYER", ""), "1.  PLAYER");
        assert_eq!(baller_row(4, "JAY", "Z"), "5.  JAY Z");
    }

    #[test]
    fn questions_ask_and_notices_end() {
        assert!(
            Ask::Sure {
                kind: 0,
                row: 0,
                name: "A".into()
            }
            .asks()
        );
        assert!(!Ask::Nothing(1).asks());
        assert_eq!(Ask::Nothing(0).first_row(), 0);
        assert_eq!(
            Ask::Sure {
                kind: 0,
                row: 0,
                name: "A".into()
            }
            .first_row(),
            1
        );
    }
}
