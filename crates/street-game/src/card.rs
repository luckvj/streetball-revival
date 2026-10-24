//! The memory card's messages and dialogs (the card widget 0x3c8c,
//! `FUN_002294e0` over `FUN_00226ae0`; its dialog `FUN_00259380`).
//!
//! The rewrite keeps its game data in a file, and the file stands where
//! the card stood: it is written only where the original writes the card
//! (`FUN_002d8460(.., 0x3f)` and Game Data's Save, `FUN_0022c9d0`), and the
//! card's own messages and dialogs show there, worded as for the card, with
//! the original's boxes, times and inputs. Between those points a change
//! stays in memory, as the original's stays in its game data in RAM
//! (`0x4bbf10`) until the card's next save.
//!
//! The card's state is read from the file in the nearest honest way:
//! - the file there reads as a formatted card holding the original game
//!   data (`+0x4c4` set, the game's file found);
//! - no file reads as a formatted card without the game's data, with the
//!   room of a fresh card: 8,000 KB, the 7,897 KB the runtime's card had
//!   left beside a 103 KB save;
//! - a file that does not read back reads as a card whose data fails to
//!   load (`FUN_0022b360` case 3's failure: "Load failed. Retry?");
//! - a file that cannot be written fails the save ("Save failed. Retry?").
//!
//! The rewrite's file is always there to write, so the card is never
//! missing, unformatted or full: the messages for those (`memcard_ps2.txt`
//! 32..38, 41, 53, 55..58: no card, "is not formatted", the format
//! question, too little room) have no state of the file to stand for and do
//! not show.
//!
//! Every flow, step and time below was measured in the project's PCSX2
//! runtime (PINE on the card widget's dialog, `+0xc090` `+0xee`, against the
//! 100 Hz timer `0x4b32e4`) with its own card files: unformatted, holding
//! the game's data, and formatted without it (the game's directory entry
//! renamed, its ECC recomputed). The answers' paths are `FUN_0022c150`'s.

use bevy::prelude::*;

use crate::look::{self, Align, Look, Piece, RowSpot};

/// What the file reads as, as a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holds {
    /// The game's data (the file is there).
    Data,
    /// A formatted card without it (no file).
    Nothing,
    /// The game's data, which does not read back (a damaged file).
    Damaged,
}

impl Holds {
    /// The file's state as the card's.
    pub fn of(path: &std::path::Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) if street_sim::story::Story::read(&bytes).is_some() => Holds::Data,
            Ok(_) => Holds::Damaged,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Holds::Nothing,
            Err(_) => Holds::Damaged,
        }
    }
}

/// The card widget's work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// The boot's load (`FUN_0029dc18`: `+0x174` with 0x7f, its `+0xd4`
    /// set): every section, nothing asked.
    Boot,
    /// The game's own save (`FUN_002d8460(.., 0x3f)`): Checking first with
    /// Options → Autosave on (`+0xb4`, the settings' bit 3), "Save league
    /// STREET Vol. 2 data ...?" first with it off.
    Autosave,
    /// Game Data's Save (`FUN_00299110` → `+0x16c` with 0x13f: its bit 8,
    /// `+0xdc`, clears `+0xb4`): always asks first.
    Save,
    /// Game Data's Load (`+0x174`, `FUN_0022cbd8`): Checking, then asks.
    Load,
    /// A delete done in Game Data: the widget's "Delete successful."
    /// (`FUN_002278c0` case 10), a message alone; the card is written when
    /// Game Data is left (`FUN_00299110`'s `+0xf0`).
    Deleted,
}

/// A dialog the card shows: `FUN_002298d0`'s state (its id at the
/// dialog's `+0xee`), the message (`memcard_ps2.txt`, ids 0x4900 on), its
/// caution line and its rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shows {
    /// 1: "Checking for memory card ...", timed.
    Checking,
    /// 2: "Saving to memory card ...", while the card is written.
    Saving,
    /// 3: "Loading from memory card ...", while it is read.
    Loading,
    /// 9: "No game data found ...", timed.
    NoData,
    /// 10: "Save successful.", timed.
    Saved,
    /// 11: "Load successful.", timed.
    Loaded,
    /// 14: "Create a new file for saved game data ...? File size: %2dKB
    /// (%3dKB available)", YES and NO, the cursor on NO.
    CreateFile,
    /// 15: "Save game data ...?" asked again (after Checking
    /// on the autosave's way, or a NO to the file's question), the cursor
    /// on YES.
    SaveAgain,
    /// 16: "Load game data ...? All unsaved data will be
    /// overwritten.", the cursor on NO.
    LoadAsk,
    /// 19: "Overwrite existing game data ...?", the cursor on
    /// NO.
    Overwrite,
    /// 21: "Save game data ...?" first, the cursor on YES.
    SaveAsk,
    /// 22: "Save failed.\n\nRetry?", the cursor on NO.
    SaveFailed,
    /// 23: "Load failed.\n\nRetry?", the cursor on NO.
    LoadFailed,
    /// 0xd3: "Delete successful." (`delete.txt` 6), timed.
    Deleted,
}

/// The size `FUN_0022cf58` gives the game's file: its six sections (0x60,
/// 0x18, 0xa64, 0x1780, 0x7d50, 0x460 bytes and 0x18 more: 42 KB) with the
/// icon's files, as the original's question says it.
pub const FILE_KB: u32 = 103;
/// A fresh card's room: 8,000 KB (the runtime's card had 7,897 left beside
/// a 103 KB save).
pub const ROOM_KB: u32 = 8_000;
/// The slot the game uses (`FUN_00339e78`: "1", not "1-A", without a
/// multitap).
pub const SLOT: &str = "1";

impl Shows {
    /// The original's dialog id.
    pub fn id(self) -> u16 {
        match self {
            Shows::Checking => 1,
            Shows::Saving => 2,
            Shows::Loading => 3,
            Shows::NoData => 9,
            Shows::Saved => 10,
            Shows::Loaded => 11,
            Shows::CreateFile => 14,
            Shows::SaveAgain => 15,
            Shows::LoadAsk => 16,
            Shows::Overwrite => 19,
            Shows::SaveAsk => 21,
            Shows::SaveFailed => 22,
            Shows::LoadFailed => 23,
            Shows::Deleted => 0xd3,
        }
    }

    /// The message's line in `memcard_ps2.txt` (`delete.txt` for the
    /// delete's) and its English.
    fn message_text(self) -> (usize, &'static str) {
        match self {
            Shows::Deleted => (6, "Delete successful."),
            Shows::Checking => (19, "Checking for saved data\\non this PC..."),
            Shows::Saving => (20, "Saving to PC..."),
            Shows::Loading => (23, "Loading from PC..."),
            Shows::NoData => (35, "No game data\\nfound on this PC."),
            Shows::Saved => (21, "Save successful."),
            Shows::Loaded => (22, "Load successful."),
            Shows::CreateFile => (
                42,
                "Create a new file for\\nsaved game data\\non this PC?\\n\\nFile size: %2dKB",
            ),
            Shows::SaveAgain | Shows::SaveAsk => (39, "Save game data\\nto this PC?"),
            Shows::LoadAsk => (
                40,
                "Load game data\\nfrom this PC?\\nAll unsaved data will\\nbe overwritten.",
            ),
            Shows::Overwrite => (43, "Overwrite the existing\\ngame data\\non this PC?"),
            Shows::SaveFailed => (44, "Save failed.\\n\\nRetry?"),
            Shows::LoadFailed => (45, "Load failed.\\n\\nRetry?"),
        }
    }

    /// The message, its `%1s` the slot and its numbers as the card's
    /// (`FUN_001d60e0`: thousands set off by commas, "7,897KB").
    pub fn message(self, look: Option<&Look>) -> String {
        let (k, or) = self.message_text();
        let file = match self {
            Shows::Deleted => "delete.txt",
            _ => "memcard_ps2.txt",
        };
        // The card's own messages say the PC (Vj, 2026-10-21: the save is a
        // file in Documents, not a memory card); the plain ones ("Save
        // successful.", "Delete successful.") keep the disc's words.
        let text = if self.says_the_card() {
            or
        } else {
            look.and_then(|l| l.text(file, k)).unwrap_or(or)
        };
        fill(text, SLOT, FILE_KB, ROOM_KB)
    }

    /// Whether the disc's words for it name the memory card.
    fn says_the_card(self) -> bool {
        !matches!(
            self,
            Shows::Saved | Shows::Loaded | Shows::Deleted | Shows::SaveFailed | Shows::LoadFailed
        )
    }

    /// The caution under the message while the card is busy (`0x4900`,
    /// the dialog's element 3: "Do not remove memory card ... Do not turn
    /// the power off or reset the console.").
    pub fn caution(self, look: Option<&Look>) -> Option<String> {
        matches!(self, Shows::Checking | Shows::Saving | Shows::Loading).then(|| {
            let _ = look;
            fill(
                "Do not close the game\\nwhile it saves.",
                SLOT,
                FILE_KB,
                ROOM_KB,
            )
        })
    }

    /// Its rows' words in `memcard_ps2.txt` (5 YES, 6 NO), top first; none
    /// for the messages that stand alone.
    pub fn rows(self) -> &'static [(usize, &'static str)] {
        match self {
            Shows::CreateFile
            | Shows::SaveAgain
            | Shows::LoadAsk
            | Shows::Overwrite
            | Shows::SaveAsk
            | Shows::SaveFailed
            | Shows::LoadFailed => &[(5, "YES"), (6, "NO")],
            _ => &[],
        }
    }

    /// Where the cursor starts (`FUN_0025a880`: 0 on YES for the save's
    /// questions, 1 on NO for the rest).
    pub fn first_row(self) -> usize {
        match self {
            Shows::SaveAgain | Shows::SaveAsk => 0,
            _ => 1,
        }
    }

    /// Whether it waits for an answer.
    pub fn asks(self) -> bool {
        !self.rows().is_empty()
    }
}

/// `%1s`, `%2d` and `%3d` filled (the numbers with thousands' commas).
fn fill(text: &str, slot: &str, size: u32, room: u32) -> String {
    text.replace("%1s", slot)
        .replace("%2s", slot)
        .replace("%2d", &thousands(size))
        .replace("%3d", &thousands(room))
}

/// The PC's words for `memcard_ps2.txt`'s lines that name the memory card
/// (Vj, 2026-10-21: the save is a file on the PC, in Documents, so no
/// screen speaks of a memory card or a console). Applied once to the disc's
/// texts as they load, so every screen that shows them says the PC.
pub fn pc_words(index: usize) -> Option<&'static str> {
    Some(match index {
        0 => "Do not close the game\\nwhile it saves.",
        19 => "Checking for saved data\\non this PC...",
        20 => "Saving to PC...",
        23 => "Loading from PC...",
        24 => "Deleting from PC...",
        25 => "Preparing saved data\\non this PC...",
        32 => {
            "No saved game data\\nfound on this PC.\\n\\nAt least %2d KB is needed\\nto save game data.\\nDo you want to start?"
        }
        33 => "Saved game data could not\\nbe read on this PC.\\n\\nDo you want to retry?",
        34 => "The saved game data\\non this PC is not ready.",
        35 => "No game data\\nfound on this PC.",
        36 => {
            "There is not enough\\nfree space on this PC.\\n\\nRequired: %2dKB\\n\\nDo you want to start?"
        }
        37 => {
            "There is not enough\\nfree space on this PC.\\n\\nRequired: %2dKB\\n\\nDo you want to try again?"
        }
        38 => "Saved game data could not\\nbe read on this PC.\\n\\nDo you want to retry?",
        39 => "Save game data\\nto this PC?",
        40 => "Load game data\\nfrom this PC?\\nAll unsaved data will\\nbe overwritten.",
        41 => "The saved game data\\non this PC is not ready.\\n\\nWould you like to prepare it?",
        42 => "Create a new file for\\nsaved game data\\non this PC?\\n\\nFile size: %2dKB",
        43 => "Overwrite the existing\\ngame data\\non this PC?",
        53 => {
            "This PC cannot save\\nany more user names.\\n(Maximum allowed: %2d)\\n\\nWould you like to try again?"
        }
        54 => "There are no created ballers\\nsaved on this PC.",
        55 => {
            "game data\\ncannot be saved on this PC.\\n\\nDo you want to continue\\nthe game without saving?"
        }
        56 => {
            "There is not enough\\nfree space on this PC.\\n\\nRequired: %2dKB\\n\\nDo you want to start?"
        }
        57 => {
            "There is not enough\\nfree space on this PC.\\n\\nRequired: %2dKB\\n\\nDo you want to try again?"
        }
        58 => "Create a new file for\\nsaved game data\\non this PC?\\n\\nFile size: %2dKB",
        _ => return None,
    })
}

/// `memcard_ps2.txt`'s lines with the PC's words in place ([`pc_words`]).
pub fn to_pc(texts: &mut [String]) {
    for (k, t) in texts.iter_mut().enumerate() {
        if let Some(words) = pc_words(k) {
            *t = words.to_owned();
        }
    }
}

/// `n` with a comma between each three digits.
pub fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// How long each timed step stands, in seconds, as measured (hundredths of
/// the 100 Hz timer from one dialog's id to the next).
pub mod times {
    /// A message alone (`FUN_0025a400`'s timer of 2, `FUN_002da198`): 1.99
    /// to 2.03, 2.02 in most of twelve.
    pub const MESSAGE: f64 = 2.02;
    /// Checking, the card's information read: 2.08 on the boot and the
    /// autosave, 2.07 and 2.09 for Game Data's load.
    pub const CHECK: f64 = 2.08;
    /// Checking before a save asked for, with the game's data on the card
    /// (the file read before the overwrite question): 3.42 and 3.43.
    pub const CHECK_DATA: f64 = 3.43;
    /// Saving over the game's file after the overwrite question: 3.12 (two
    /// runs).
    pub const SAVE_OVER: f64 = 3.12;
    /// Saving a new file after the file's question: 4.42.
    pub const SAVE_NEW: f64 = 4.42;
    /// Saving on the autosave's way with the game's data on the card, its
    /// check's rest under it: 4.41.
    pub const SAVE_AUTO: f64 = 4.41;
    /// Loading: 3.10 on the boot, 3.12 from Game Data.
    pub const LOAD_BOOT: f64 = 3.10;
    pub const LOAD: f64 = 3.12;
}

/// What the caller does for the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    /// Write the file now; answer with [`Flow::wrote`].
    Write,
    /// Read the file back now; answer with [`Flow::read`].
    Read,
    /// The load failed: the game data goes back to its defaults
    /// (`FUN_00227208`, on closing "Load failed.").
    Reset,
}

/// One run of the card's work, from its first dialog to its last.
#[derive(Clone, Debug)]
pub struct Flow {
    pub op: Op,
    pub holds: Holds,
    autosave: bool,
    /// The dialog up.
    pub shows: Shows,
    /// Its lit row.
    pub cursor: usize,
    /// How long it has stood (seconds).
    pub stood: f64,
    /// How long it stands, when timed.
    stands: f64,
    /// The file written (the save's work done and kept).
    pub saved: bool,
    /// The file read back.
    pub loaded: bool,
    pub done: bool,
    work: Option<Work>,
}

impl Flow {
    /// The card's work begun on a card that `holds` this, Options →
    /// Autosave `autosave`.
    pub fn new(op: Op, holds: Holds, autosave: bool) -> Self {
        let mut flow = Self {
            op,
            holds,
            autosave,
            shows: Shows::Checking,
            cursor: 0,
            stood: 0.0,
            stands: 0.0,
            saved: false,
            loaded: false,
            done: false,
            work: None,
        };
        // `FUN_0022ca50`: Checking first on the autosave's way, the
        // question first otherwise (`+0xb4` clear); a load checks first
        // (`FUN_0022cbd8`).
        let first = match op {
            Op::Boot | Op::Load => Shows::Checking,
            Op::Autosave if autosave => Shows::Checking,
            Op::Autosave | Op::Save => Shows::SaveAsk,
            Op::Deleted => Shows::Deleted,
        };
        flow.show(first);
        flow
    }

    fn show(&mut self, shows: Shows) {
        let measured = match shows {
            Shows::Checking => match (self.op, self.holds) {
                // The asked save reads the game's file before its
                // question.
                (Op::Save, Holds::Data | Holds::Damaged) => times::CHECK_DATA,
                (Op::Autosave, Holds::Data | Holds::Damaged) if !self.autosave => times::CHECK_DATA,
                _ => times::CHECK,
            },
            Shows::Saving => match (self.holds, self.shows) {
                (Holds::Nothing, _) => times::SAVE_NEW,
                (_, Shows::Overwrite) => times::SAVE_OVER,
                _ => times::SAVE_AUTO,
            },
            Shows::Loading if self.op == Op::Boot => times::LOAD_BOOT,
            Shows::Loading => times::LOAD,
            _ => times::MESSAGE,
        };
        // Fast unless the original's holds are asked for ([`crate::pace`]).
        self.stands = crate::pace::hold(
            measured,
            match shows {
                Shows::Checking | Shows::Saving | Shows::Loading => crate::pace::WAIT,
                _ => crate::pace::NOTICE,
            },
        );
        self.shows = shows;
        self.cursor = shows.first_row();
        self.stood = 0.0;
        debug!("memory card: dialog {} ({shows:?})", shows.id());
    }

    /// What the caller must do now, once.
    pub fn work(&mut self) -> Option<Work> {
        self.work.take()
    }

    /// Time passes: a timed dialog moves on at its end.
    pub fn tick(&mut self, dt: f64) {
        if self.done || self.shows.asks() || self.work.is_some() {
            return;
        }
        self.stood += dt;
        if self.stood < self.stands {
            return;
        }
        match self.shows {
            Shows::Checking => self.checked(),
            Shows::Saving => self.work = Some(Work::Write),
            Shows::Loading => self.work = Some(Work::Read),
            _ => self.done = true,
        }
    }

    /// After Checking, by the card's state (`FUN_0022a9b8`'s steps and
    /// `FUN_0022c150` case 0).
    fn checked(&mut self) {
        let data = self.holds != Holds::Nothing;
        let next = match self.op {
            // The boot loads without asking.
            Op::Boot if data => Shows::Loading,
            Op::Load if data => Shows::LoadAsk,
            Op::Boot | Op::Load => Shows::NoData,
            // On the autosave's way a card with the game's file is saved
            // at once; without it the question comes back (`+0xb4`
            // cleared): "Save ...?" again.
            Op::Autosave if self.autosave && data => Shows::Saving,
            Op::Autosave if self.autosave => Shows::SaveAgain,
            _ if data => Shows::Overwrite,
            _ => Shows::CreateFile,
        };
        self.show(next);
    }

    /// The lit row moves (up -1, down +1; the rows do not wrap).
    pub fn step(&mut self, by: i32) {
        let rows = self.shows.rows().len();
        if rows == 0 || self.work.is_some() {
            return;
        }
        let at = (self.cursor as i32 + by).clamp(0, rows as i32 - 1);
        self.cursor = at as usize;
    }

    /// Cross on the lit row (`FUN_0022c150`).
    pub fn choose(&mut self) {
        if !self.shows.asks() || self.done || self.work.is_some() {
            return;
        }
        let yes = self.cursor == 0;
        let data = self.holds != Holds::Nothing;
        match (self.shows, yes) {
            // Case 0x14: YES checks the card.
            (Shows::SaveAsk, true) => self.show(Shows::Checking),
            // Case 0xe: YES goes straight to the file's question.
            (Shows::SaveAgain, true) if data => self.show(Shows::Overwrite),
            (Shows::SaveAgain, true) => self.show(Shows::CreateFile),
            // Cases 0x12, 0xd: the file's questions write on YES and ask
            // "Save ...?" again on NO.
            (Shows::Overwrite | Shows::CreateFile, true) => self.show(Shows::Saving),
            (Shows::Overwrite | Shows::CreateFile, false) => self.show(Shows::SaveAgain),
            // Case 0xf.
            (Shows::LoadAsk, true) => self.show(Shows::Loading),
            // Case 0x15: Retry checks again (`FUN_0022ce30` clears
            // `+0xb4`, so the questions come).
            (Shows::SaveFailed, true) => {
                self.autosave = false;
                self.show(Shows::Checking);
            }
            // Case 0x16: the data goes back to its defaults either way
            // (`FUN_00227208`), and Retry checks again.
            (Shows::LoadFailed, retry) => {
                self.work = Some(Work::Reset);
                if retry {
                    self.show(Shows::Checking);
                } else {
                    self.done = true;
                }
            }
            // NO ends the work, nothing saved or loaded.
            (Shows::SaveAsk | Shows::SaveAgain | Shows::LoadAsk | Shows::SaveFailed, false) => {
                self.done = true;
            }
            _ => self.done = true,
        }
    }

    /// The caller wrote the file (or failed to).
    pub fn wrote(&mut self, ok: bool) {
        self.work = None;
        if ok {
            self.saved = true;
            // The card holds the game's file now.
            self.holds = Holds::Data;
            self.show(Shows::Saved);
        } else {
            self.show(Shows::SaveFailed);
        }
    }

    /// The caller read the file back (or failed to).
    pub fn read(&mut self, ok: bool) {
        self.work = None;
        if ok {
            self.loaded = true;
            self.show(Shows::Loaded);
        } else {
            self.holds = Holds::Damaged;
            self.show(Shows::LoadFailed);
        }
    }
}

/// Whether the load back saves to the card before the menus come
/// (`FUN_0029d5a8`): after every Street School exit (mode 1, `DAT_00400054`
/// bit 2 always set by `FUN_002f3918`); after a Pick Up, League Challenge or
/// Be A Legend game that was finished (bit 2: the game's phase 9) or whose
/// options changed in it (bit 4, `FUN_002f2e50` against the start's copy),
/// unless another screen follows that saves itself (`other`: Update Team,
/// a completion board, a finished tournament's bracket); never after the
/// attract's game.
pub fn saves_after_game(
    mode: crate::loading::Mode,
    finished: bool,
    options_changed: bool,
    other: bool,
) -> bool {
    use crate::loading::Mode;
    match mode {
        Mode::School => true,
        Mode::Attract => false,
        Mode::PickUp | Mode::League | Mode::Legend => !other && (finished || options_changed),
    }
}

/// The card's work in hand, for every screen that shows it.
#[derive(Resource, Default)]
pub struct Card {
    pub flow: Option<Flow>,
    /// The work ended this update: the press that ended it is the card's,
    /// not the screen's under it.
    pub closed: bool,
}

/// One update of the card's dialogs: up and down between a question's rows
/// and Cross on one (`FUN_00259380`'s list), the messages' time otherwise.
pub fn keys(flow: &mut Flow, input: &crate::menu::MenuInput, dt: f64) {
    if flow.shows.asks() {
        let dy = i32::from(input.down()) - i32::from(input.up());
        if dy != 0 {
            flow.step(dy);
        } else if input.confirm() {
            flow.choose();
        }
    } else {
        flow.tick(dt);
    }
}

/// Saving's end: the game data into the file.
pub fn write(flow: &mut Flow, save: &mut crate::story::Save) {
    save.store_now();
    flow.wrote(save.last_error.is_none());
}

/// How much of the light behind the card's dialog it takes: measured, the
/// Game Data screen and the load back keep 0.39 of theirs under it (0.393,
/// 0.390 and 0.392 on the original's grabs).
pub const DIM: f32 = 0.61;

/// The dialog's place and size (`FUN_0025a400`): its words' box and 100
/// more each way, and 30 for each row, at least 4:3 (taller boxes widen
/// to 1.33 of their height), its middle at (320, 240) (`FUN_00259380`'s
/// `+0x124`).
pub fn dialog_rect(widest: f32, words_h: f32, rows: usize) -> Rect {
    let w = widest + 100.0;
    let h = words_h + 30.0 * rows as f32 + 100.0;
    let size = if h < w * 0.75 {
        Vec2::new(w, w * 0.75)
    } else {
        Vec2::new(h * 1.33, h)
    };
    Rect::from_center_size(Vec2::new(320.0, 240.0), size)
}

/// The words' font (`FUN_002e5578(2)` on elements 2, 3 and the rows'
/// texts).
pub const FONT: usize = 2;
/// The message's top under the box's (`FUN_00259e38`: 35).
const MESSAGE_TOP: f32 = 35.0;
/// The caution's gap under the message (25).
const CAUTION_GAP: f32 = 25.0;
/// The rows over the box's bottom: NO (or OK) 65 up, YES 95.
const ROW_UP: [f32; 2] = [95.0, 65.0];

/// The flow's dialog over whatever stands under it, `dim` darkening that
/// (0 for none).
pub fn pieces(
    look: &Look,
    gm: &crate::boards::Gm,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    panels: &mut crate::boards::Panels,
    flow: &Flow,
    dim: f32,
    ticks: u64,
) -> Vec<Piece> {
    let rows: Vec<String> = flow
        .shows
        .rows()
        .iter()
        .map(|&(k, or)| look.text("memcard_ps2.txt", k).unwrap_or(or).to_owned())
        .collect();
    box_pieces(
        look,
        gm,
        (labels, images, panels),
        &flow.shows.message(Some(look)),
        flow.shows.caution(Some(look)).as_deref(),
        &rows,
        (flow.cursor, dim, ticks),
    )
}

/// The card widget's dialog (`FUN_00259380`, `FUN_0025a400`) with these
/// words, caution and rows, `lit` lit; also the widget's own questions
/// (Game Data's deletes, `FUN_002278c0`).
pub fn box_pieces(
    look: &Look,
    gm: &crate::boards::Gm,
    (labels, images, panels): (
        &mut look::Labels,
        &mut Assets<Image>,
        &mut crate::boards::Panels,
    ),
    message: &str,
    caution: Option<&str>,
    rows: &[String],
    (lit, dim, ticks): (usize, f32, u64),
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(font) = look.fonts.get(FONT) else {
        return out;
    };
    let (th, lh) = (font.text_height as f32, font.line_height as f32);
    let block = |text: &str| -> (Vec<String>, f32, f32) {
        let lines: Vec<String> = text
            .replace("\\n", "\n")
            .split('\n')
            .map(str::to_owned)
            .collect();
        let h = th + (lines.len().saturating_sub(1)) as f32 * lh;
        let w = lines
            .iter()
            .map(|l| font.measure(l) as f32)
            .fold(0.0, f32::max);
        (lines, w, h)
    };
    let (message, mw, mh) = block(message);
    let caution = caution.map(block);
    let widest = caution.as_ref().map_or(mw, |c| mw.max(c.1));
    let words_h = mh + caution.as_ref().map_or(0.0, |c| c.2);
    let rect = dialog_rect(widest, words_h, rows.len());
    if dim > 0.0 {
        let mut p = Piece::new(gm.white.clone(), Rect::new(0.0, 0.0, 640.0, 480.0));
        p.colour = Color::srgba(0.0, 0.0, 0.0, look::black_alpha(dim));
        out.push(p);
    }
    out.push(panels.get(
        [rect.min.x, rect.min.y, rect.width(), rect.height()],
        0xff,
        gm,
        images,
    ));
    let cx = rect.center().x;
    let mut put = |out: &mut Vec<Piece>, lines: &[String], top: f32| {
        for (k, l) in lines.iter().enumerate() {
            if let Some(p) = labels.text(
                look,
                images,
                FONT,
                l,
                [0xff; 4],
                None,
                Vec2::new(cx, top + lh * k as f32),
                Align::Centre,
            ) {
                out.push(p);
            }
        }
    };
    let message_top = rect.min.y + MESSAGE_TOP;
    put(&mut out, &message, message_top);
    if let Some((lines, _, _)) = &caution {
        put(&mut out, lines, message_top + mh + CAUTION_GAP);
    }
    let mut stars = Vec::new();
    let first = ROW_UP.len() - rows.len().min(ROW_UP.len());
    for (k, row) in rows.iter().enumerate() {
        let spot = RowSpot {
            at: Vec2::new(cx, rect.max.y - ROW_UP[first + k]),
            font: FONT,
            align: Align::Centre,
            valign: 0,
        };
        look::screen_row(
            &mut out,
            &mut stars,
            labels,
            look,
            images,
            spot,
            row,
            k == lit,
            true,
            ticks,
        );
    }
    out.extend(stars);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `flow` with the caller's work answered `ok`, choosing the
    /// rows in `answers` at each question in turn, and lists the dialogs
    /// it showed with how long each stood.
    fn run(mut flow: Flow, ok: bool, answers: &[usize]) -> (Vec<(u16, f64)>, Flow) {
        let mut seen = vec![(flow.shows.id(), 0.0)];
        let mut answers = answers.iter();
        let note = |seen: &mut Vec<(u16, f64)>, flow: &Flow| {
            if !flow.done && seen.last().is_some_and(|s| s.0 != flow.shows.id()) {
                seen.push((flow.shows.id(), 0.0));
            }
        };
        for _ in 0..100_000 {
            if flow.done {
                break;
            }
            if let Some(work) = flow.work() {
                match work {
                    Work::Write => flow.wrote(ok),
                    Work::Read => flow.read(ok),
                    Work::Reset => {}
                }
            } else if flow.shows.asks() {
                let Some(&row) = answers.next() else {
                    break;
                };
                flow.cursor = row;
                flow.choose();
            } else {
                flow.tick(0.01);
                if let Some(last) = seen.last_mut() {
                    last.1 += 0.01;
                }
            }
            note(&mut seen, &flow);
        }
        (seen, flow)
    }

    fn ids(seen: &[(u16, f64)]) -> Vec<u16> {
        seen.iter().map(|s| s.0).collect()
    }

    #[test]
    fn the_boot_loads_a_card_with_data_and_says_so() {
        let (seen, flow) = run(Flow::new(Op::Boot, Holds::Data, true), true, &[]);
        assert_eq!(ids(&seen), [1, 3, 11]);
        assert!(flow.loaded && flow.done);
        assert!((seen[0].1 - times::CHECK).abs() < 0.02);
        assert!((seen[1].1 - times::LOAD_BOOT).abs() < 0.02);
        assert!((seen[2].1 - times::MESSAGE).abs() < 0.02);
    }

    #[test]
    fn the_boot_finds_no_data_without_a_file() {
        let (seen, flow) = run(Flow::new(Op::Boot, Holds::Nothing, true), true, &[]);
        assert_eq!(ids(&seen), [1, 9]);
        assert!(!flow.loaded && flow.done);
    }

    #[test]
    fn the_autosave_saves_over_its_file_without_asking() {
        let (seen, flow) = run(Flow::new(Op::Autosave, Holds::Data, true), true, &[]);
        assert_eq!(ids(&seen), [1, 2, 10]);
        assert!(flow.saved);
        assert!((seen[1].1 - times::SAVE_AUTO).abs() < 0.02);
    }

    #[test]
    fn the_autosave_asks_before_making_its_file() {
        // Checking, "Save ...?" YES, "Create a new file ...?" YES.
        let (seen, flow) = run(Flow::new(Op::Autosave, Holds::Nothing, true), true, &[0, 0]);
        assert_eq!(ids(&seen), [1, 15, 14, 2, 10]);
        assert!(flow.saved);
        // NO to the file comes back to "Save ...?", and NO there ends it.
        let (seen, flow) = run(
            Flow::new(Op::Autosave, Holds::Nothing, true),
            true,
            &[0, 1, 1],
        );
        assert_eq!(ids(&seen), [1, 15, 14, 15]);
        assert!(!flow.saved && flow.done);
    }

    #[test]
    fn with_autosave_off_the_save_asks_first() {
        let (seen, flow) = run(Flow::new(Op::Autosave, Holds::Data, false), true, &[0, 0]);
        assert_eq!(ids(&seen), [21, 1, 19, 2, 10]);
        assert!(flow.saved);
        assert!((seen[1].1 - times::CHECK_DATA).abs() < 0.02);
        assert!((seen[3].1 - times::SAVE_OVER).abs() < 0.02);
        let (seen, flow) = run(Flow::new(Op::Autosave, Holds::Data, false), true, &[1]);
        assert_eq!(ids(&seen), [21]);
        assert!(!flow.saved && flow.done);
    }

    #[test]
    fn game_datas_save_and_load() {
        let (seen, _) = run(Flow::new(Op::Save, Holds::Nothing, true), true, &[0, 0]);
        assert_eq!(ids(&seen), [21, 1, 14, 2, 10]);
        assert!((seen[3].1 - times::SAVE_NEW).abs() < 0.02);
        let (seen, flow) = run(Flow::new(Op::Load, Holds::Data, true), true, &[0]);
        assert_eq!(ids(&seen), [1, 16, 3, 11]);
        assert!(flow.loaded);
        let (seen, _) = run(Flow::new(Op::Load, Holds::Nothing, true), true, &[]);
        assert_eq!(ids(&seen), [1, 9]);
    }

    #[test]
    fn a_delete_says_so_for_a_message_alone() {
        let (seen, flow) = run(Flow::new(Op::Deleted, Holds::Data, true), true, &[]);
        assert_eq!(ids(&seen), [0xd3]);
        assert!(flow.done && !flow.saved);
        assert!((seen[0].1 - times::MESSAGE).abs() < 0.02);
    }

    #[test]
    fn a_failed_write_asks_to_retry() {
        let (seen, flow) = run(Flow::new(Op::Save, Holds::Data, true), false, &[0, 0, 1]);
        assert_eq!(ids(&seen), [21, 1, 19, 2, 22]);
        assert!(!flow.saved && flow.done);
    }

    #[test]
    fn the_cursor_starts_where_the_original_puts_it() {
        assert_eq!(Shows::SaveAsk.first_row(), 0);
        assert_eq!(Shows::SaveAgain.first_row(), 0);
        for s in [
            Shows::Overwrite,
            Shows::CreateFile,
            Shows::LoadAsk,
            Shows::SaveFailed,
        ] {
            assert_eq!(s.first_row(), 1);
        }
    }

    #[test]
    fn numbers_take_commas() {
        assert_eq!(thousands(103), "103");
        assert_eq!(thousands(7897), "7,897");
        assert_eq!(thousands(8000), "8,000");
        // The PC's question names the file's size and no card's room.
        let ask = Shows::CreateFile.message(None);
        assert!(ask.contains("on this PC?") && ask.contains("File size: 103KB"));
        assert!(!ask.to_lowercase().contains("memory card"));
    }

    #[test]
    fn no_message_speaks_of_a_memory_card_or_console() {
        for k in 0..64 {
            if let Some(words) = pc_words(k) {
                let w = words.to_lowercase();
                assert!(
                    !w.contains("memory card") && !w.contains("playstation"),
                    "{k}: {words}"
                );
                assert!(!w.contains("console"), "{k}: {words}");
            }
        }
        for s in [
            Shows::Checking,
            Shows::Saving,
            Shows::Loading,
            Shows::SaveAsk,
            Shows::Overwrite,
        ] {
            assert!(!s.message(None).to_lowercase().contains("memory card"));
        }
    }

    #[test]
    fn the_load_back_saves_as_the_original_does() {
        use crate::loading::Mode;
        // Every School exit, finished or not.
        assert!(saves_after_game(Mode::School, false, false, false));
        // A Pick Up game finished, or quit with its options changed.
        assert!(saves_after_game(Mode::PickUp, true, false, false));
        assert!(saves_after_game(Mode::PickUp, false, true, false));
        assert!(!saves_after_game(Mode::PickUp, false, false, false));
        // Update Team, a board or a finished tournament saves itself.
        assert!(!saves_after_game(Mode::Legend, true, false, true));
        assert!(!saves_after_game(Mode::Attract, true, true, false));
    }

    #[test]
    fn the_box_is_at_least_four_by_three() {
        let r = dialog_rect(200.0, 50.0, 0);
        assert_eq!(r.size(), Vec2::new(300.0, 225.0));
        let r = dialog_rect(200.0, 300.0, 2);
        assert_eq!(r.height(), 460.0);
        assert!((r.width() - 460.0 * 1.33).abs() < 1e-3);
        assert_eq!(r.center(), Vec2::new(320.0, 240.0));
    }
}
