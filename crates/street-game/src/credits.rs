//! The credits: Options' Circle opens their screen (class 9, layout 9 on
//! backdrop 31, `FUN_00283bd8`: "Credits", NuFX's and the publisher's logos
//! in the backdrop, the legends "NuFX" on Cross and "THE PUBLISHER CANADA"
//! on Circle, Back on Triangle, "Select and view game credits."), whose
//! keys (`FUN_00283d08`) go forward to a roll: Cross to NuFX's (built as
//! class 8: its transition plays NuFX's logo, `FUN_00283b40`, then
//! `xmtfblur`), Circle to the developer's (class 7: `xmtfblur` alone), Triangle
//! back to Options.
//!
//! A roll (`FUN_00281a08`: reported class 7, layout 7 on backdrop 8) reads
//! its team's text from `/CREDITS/XCREDITS.BIN` (`FUN_00281cb0`, a `C0FB`
//! archive of five UTF-16 texts: the developer's in English, again for PAL
//! consoles, NuFX's in English and in French, the developer's in French; a US
//! console reads entries 0 and 2). Each line is a row, its tabs part its
//! cells, a `/` and a letter before a cell set the row's style (`T` a
//! team, `H` a heading, `L` a name and its part; none the plain).
//!
//! As the roll arrives its picture (`/FE/XAFEBGEX.BIN` entry 3, the
//! skyline, streamed by `FUN_002e4560(.., 4, 3, -1)`) shows at (0, 224) as
//! 640 by 256 over the rows, fading in by 6 an update. Then "Credits"
//! (`credits.txt` 0, the screen's `+0x120` 0x600) starts at y 530 in the
//! title's style, and every update (`FUN_00282398`) each line moves up 0.8
//! pixels, its alpha `(min(y, 480 - y) - 50) / 50` of 256 (so they fade in
//! between y 430 and 380 and out between 100 and 50), and goes once its
//! bottom is above 50; while the newest line's bottom with 0.6 of its
//! height to spare is above 430, the next row starts there. A row's cells
//! stand at `640 * k / (n + 1)` across (`FUN_00282b08`), centred; an `L`
//! row's first 120 to the left, left-aligned, and its second 120 to the
//! right, right-aligned. With no line and no row left the roll goes back to
//! Options (`FUN_002d7d40(.., 0x28)`); so do Cross and Triangle
//! (`FUN_00282350`) once its picture is in.
//!
//! Measured in the project's PCSX2 runtime (2026-10-20, boot 4): Options
//! to the credits' screen `xmtfblur`; Circle to the developer's roll
//! `xmtfblur`; Cross to NuFX's `xnufx` (181 frames; Start and Cross during
//! it did nothing) then `xmtfblur`; NuFX's roll stood 55.4 s untouched and
//! went back to Options through `xmtblur`.

use bevy::prelude::*;

use crate::look::{self, Align, Look, Piece};

/// The credits' screen (`FUN_00283bd8`: `FUN_002d7720(screen, 9, 0x1f)`).
pub const INTRO_CLASS: u16 = 0x09;
/// The rolls' layout (their screen reports class 7) and backdrop
/// (`FUN_002d7720(screen, 7, 8)`).
pub const ROLL_CLASS: u32 = 7;
pub const ROLL_BACKDROP: usize = 8;
/// The texts' archive.
pub const ARCHIVE: &str = "/CREDITS/XCREDITS.BIN";
/// The roll's picture: `XAFEBGEX` entry 3, its painted 320 columns (of
/// 512) at (0, 224), 640 by 256.
pub const PICTURE_ENTRY: usize = 3;
pub const PICTURE_COLUMNS: u32 = 320;
pub const PICTURE_AT: Rect = Rect {
    min: Vec2::new(0.0, 224.0),
    max: Vec2::new(640.0, 480.0),
};

/// The front end's updates a second (the roll steps by them).
pub const UPDATES: f64 = 60.0;
/// A line's rise an update, px.
pub const RISE: f32 = 0.8;
/// Where "Credits" starts.
pub const TITLE_Y: f32 = 530.0;
/// The next row starts while the newest line's bottom plus this much of
/// its height is above [`SPAWN_BELOW`].
pub const GAP: f32 = 0.6;
pub const SPAWN_BELOW: f32 = 430.0;
/// The fades' band at either edge, and the top past which a line goes.
pub const EDGE: f32 = 50.0;
/// An `L` row's cells' push away from their places.
pub const NAME_PUSH: f32 = 120.0;
/// The title's style (the "Credits" line, `FUN_00282b08(.., 1, ..)`).
pub const TITLE_STYLE: u16 = 1;
/// Updates the picture's stream takes from the disc before its fade
/// (`FUN_002e4708` until it is in): fitted, with the rest of the model,
/// to the runtime's 55.44 s for NuFX's roll (55.13 s without it).
pub const PICTURE_LOAD: u64 = 18;

/// Whose credits a roll shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Team {
    /// NuFX's (Cross; the screen built as class 8, `+0x134`).
    NuFx,
    /// the developer's (Circle; class 7).
    CoreTeam,
}

impl Team {
    /// The factory class its screen is built as.
    pub fn class(self) -> u16 {
        match self {
            Team::NuFx => 0x08,
            Team::CoreTeam => 0x07,
        }
    }

    /// The archive's entry a US console reads (`FUN_00281cb0`: English,
    /// NTSC; a PAL console's the developer's is entry 1, French consoles' 3 and
    /// 4).
    pub fn entry(self) -> usize {
        match self {
            Team::NuFx => 2,
            Team::CoreTeam => 0,
        }
    }
}

/// A row: its style (0 the plain) and its cells.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub style: u16,
    pub cells: Vec<String>,
}

/// The `C0FB` archive's entries: a big-endian count at 4, then per entry a
/// 3-byte offset and a 3-byte size and its name (none here, a 0).
pub fn entries(data: &[u8]) -> Option<Vec<&[u8]>> {
    if data.get(..2)? != [0xc0, 0xfb] {
        return None;
    }
    let count = usize::from(u16::from_be_bytes([*data.get(4)?, *data.get(5)?]));
    let mut at = 6;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let e = data.get(at..at + 6)?;
        let off = usize::from(e[0]) << 16 | usize::from(e[1]) << 8 | usize::from(e[2]);
        let size = usize::from(e[3]) << 16 | usize::from(e[4]) << 8 | usize::from(e[5]);
        out.push(data.get(off..off + size)?);
        at += 6;
        while *data.get(at)? != 0 {
            at += 1;
        }
        at += 1;
    }
    Some(out)
}

/// A text's rows as `FUN_00281cb0` cuts them: past its byte-order mark,
/// lines at a newline, cells at a tab, each trimmed of spaces and carriage
/// returns; a cell starting with `/` gives the row its style (the letter
/// after it) and loses both.
pub fn rows(text: &[u8]) -> Vec<Row> {
    let units: Vec<u16> = text
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .collect();
    let units = units.strip_prefix(&[0xfeff]).unwrap_or(&units);
    let text = String::from_utf16_lossy(units);
    let mut out = Vec::new();
    let mut lines: Vec<&str> = text.split('\n').collect();
    // A last newline ends the text, not a row.
    if lines
        .last()
        .is_some_and(|l| l.trim_matches([' ', '\r']).is_empty())
    {
        lines.pop();
    }
    for line in lines {
        let mut style = 0;
        let mut cells = Vec::new();
        for cell in line.split('\t') {
            let mut cell = cell.trim_matches([' ', '\r']);
            if let Some(rest) = cell.strip_prefix('/')
                && let Some(c) = rest.chars().next()
            {
                style = c as u16;
                cell = rest[c.len_utf8()..].trim_matches([' ', '\r']);
            }
            cells.push(cell.to_owned());
        }
        out.push(Row { style, cells });
    }
    out
}

/// A style's font and colour (`FUN_00282b08`'s table at `0x47dd20`): the
/// title font 5, a team font 2 and a heading font 1 in gold, the names and
/// the plain font 1 in cream.
pub fn style(style: u16) -> (usize, [u8; 4]) {
    const GOLD: [u8; 4] = [0xcb, 0xac, 0x00, 0xff];
    const CREAM: [u8; 4] = [0xdc, 0xdc, 0xbb, 0xff];
    match style {
        TITLE_STYLE => (5, CREAM),
        s if s == u16::from(b'H') => (1, GOLD),
        s if s == u16::from(b'T') => (2, GOLD),
        _ => (1, CREAM),
    }
}

/// Where a row's cell `k` (1-based) of `n` stands across, and how it is
/// aligned there.
pub fn across(style: u16, k: usize, n: usize) -> (f32, Align) {
    let x = 640.0 * k as f32 / (n + 1) as f32;
    if style == u16::from(b'L') {
        if k < 2 {
            (x - NAME_PUSH, Align::Left)
        } else {
            (x + NAME_PUSH, Align::Right)
        }
    } else {
        (x, Align::Centre)
    }
}

/// A line of the roll: a row's cell (or the title), its top and height.
#[derive(Clone, Debug, PartialEq)]
struct Line {
    /// The row, `None` for the title.
    row: Option<usize>,
    cell: usize,
    style: u16,
    y: f32,
    height: f32,
    alpha: u8,
}

/// A roll under way.
#[derive(Clone, Debug)]
pub struct Roll {
    pub team: Team,
    rows: Vec<Row>,
    /// The next row to start (`+0x128`).
    next: usize,
    /// The lines on the screen, oldest first.
    lines: Vec<Line>,
    /// The picture's alpha (`+0x130`).
    pub picture_alpha: u8,
    /// "Credits" has started (`+0x11c`).
    titled: bool,
    /// Front-end updates taken, and the real time they stand for.
    pub updates: u64,
    clock: f64,
    /// Nothing left: the roll goes back to Options.
    pub done: bool,
}

impl Roll {
    pub fn new(team: Team, rows: Vec<Row>) -> Self {
        Self {
            team,
            rows,
            next: 0,
            lines: Vec::new(),
            picture_alpha: 0,
            titled: false,
            updates: 0,
            clock: 0.0,
            done: false,
        }
    }

    /// One front-end update (`FUN_00282398`), a line of style `s` being
    /// `height(s)` tall (its font's text height: one line's box).
    pub fn step(&mut self, height: &impl Fn(u16) -> f32) {
        if self.done {
            return;
        }
        self.updates += 1;
        if self.updates <= PICTURE_LOAD {
            return;
        }
        if self.picture_alpha != 0xff {
            self.picture_alpha = self.picture_alpha.saturating_add(6);
            return;
        }
        if !self.titled {
            self.titled = true;
            self.lines.push(Line {
                row: None,
                cell: 1,
                style: TITLE_STYLE,
                y: TITLE_Y,
                height: height(TITLE_STYLE),
                alpha: 0,
            });
            return;
        }
        for l in &mut self.lines {
            l.y -= RISE;
            let edge = l.y.min(480.0 - l.y);
            let a = ((edge - EDGE) / EDGE).clamp(0.0, 0.9999);
            l.alpha = (a * 256.0).round() as u8;
        }
        self.lines.retain(|l| l.y + l.height >= EDGE);
        if self.lines.is_empty() && self.next >= self.rows.len() {
            self.done = true;
            return;
        }
        while let Some(newest) = self.lines.last()
            && newest.y + newest.height + newest.height * GAP < SPAWN_BELOW
            && self.next < self.rows.len()
        {
            let y = newest.y + newest.height + newest.height * GAP;
            let row = &self.rows[self.next];
            for k in 1..=row.cells.len() {
                self.lines.push(Line {
                    row: Some(self.next),
                    cell: k,
                    style: row.style,
                    y,
                    height: height(row.style),
                    alpha: 0,
                });
            }
            self.next += 1;
        }
    }

    /// Steps the roll to `elapsed` seconds of its own (an update each 1/60
    /// s).
    pub fn run_to(&mut self, elapsed: f64, height: &impl Fn(u16) -> f32) {
        self.clock = self.clock.max(0.0);
        while self.clock + 1.0 / UPDATES <= elapsed && !self.done {
            self.clock += 1.0 / UPDATES;
            self.step(height);
        }
    }

    /// Cross and Triangle leave once the picture is in (`+0x12c`).
    pub fn started(&self) -> bool {
        self.updates > PICTURE_LOAD
    }

    /// The roll's pieces: its lines, then its picture over them.
    pub fn pieces(
        &self,
        look: &Look,
        labels: &mut look::Labels,
        images: &mut Assets<Image>,
        picture: Option<&Handle<Image>>,
    ) -> Vec<Piece> {
        let mut out = Vec::new();
        for l in &self.lines {
            if l.alpha == 0 {
                continue;
            }
            let words = match l.row {
                None => look.text("credits.txt", 0).unwrap_or("Credits"),
                Some(r) => self.rows[r].cells[l.cell - 1].as_str(),
            };
            let n = l.row.map_or(1, |r| self.rows[r].cells.len());
            let (font, mut colour) = style(l.style);
            colour[3] = l.alpha;
            let (x, align) = across(l.style, l.cell, n);
            if let Some(p) = labels.text(
                look,
                images,
                font,
                words,
                colour,
                None,
                Vec2::new(x, l.y),
                align,
            ) {
                out.push(p);
            }
        }
        if let Some(picture) = picture {
            let mut p = Piece::new(picture.clone(), PICTURE_AT);
            p.colour = Color::srgba(1.0, 1.0, 1.0, f32::from(self.picture_alpha) / 255.0);
            out.push(p);
        }
        out
    }
}

/// Steps the roll on show by the real time since it stood (an update each
/// 1/60 s), its rows read from the disc as it starts; at its end it goes
/// back to Options.
pub fn run(
    mut menus: ResMut<crate::menu::Menus>,
    t: Res<crate::transition::Transition>,
    mut disc: ResMut<crate::setup::Disc>,
    look: Option<Res<Look>>,
    time: Res<Time<Real>>,
    mut since: Local<Option<f64>>,
) {
    let crate::menu::Page::CreditsRoll(team) = menus.top().0 else {
        menus.credits = None;
        *since = None;
        return;
    };
    if !t.standing() {
        return;
    }
    let now = time.elapsed_secs_f64();
    if menus.credits.as_ref().is_none_or(|r| r.team != team) {
        let rows = disc
            .iso
            .read(ARCHIVE)
            .map_err(|e| warn!("{ARCHIVE}: {e}"))
            .ok()
            .and_then(|data| entries(&data)?.get(team.entry()).map(|t| rows(t)))
            .unwrap_or_default();
        menus.credits = Some(Roll::new(team, rows));
        *since = Some(now);
    }
    let Some(look) = look else {
        return;
    };
    let height = |s: u16| {
        look.fonts
            .get(style(s).0)
            .map_or(20.0, |f| f.text_height as f32)
    };
    let start = *since.get_or_insert(now);
    if let Some(roll) = menus.credits.as_mut() {
        roll.run_to(now - start, &height);
        if roll.done {
            menus.leave_credits();
        }
    }
}

pub struct CreditsPlugin;

impl Plugin for CreditsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, run.run_if(in_state(crate::Screen::Menu)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The texts as `FUN_00281cb0` cuts them: styles, tabs, blank rows.
    #[test]
    fn rows_take_their_styles_and_cells() {
        let text: Vec<u8> = "\u{feff}/TTEAM \r\n\r\n/HART TEAM:\r\n/LKirk Gibbons\tART DIRECTOR\r\nKeith Morton \r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let r = rows(&text);
        assert_eq!(r.len(), 5);
        assert_eq!(
            r[0],
            Row {
                style: u16::from(b'T'),
                cells: vec!["TEAM".into()]
            }
        );
        assert_eq!(
            r[1],
            Row {
                style: 0,
                cells: vec![String::new()]
            }
        );
        assert_eq!(r[2].style, u16::from(b'H'));
        assert_eq!(r[3].cells, vec!["Kirk Gibbons", "ART DIRECTOR"]);
        assert_eq!(
            r[4],
            Row {
                style: 0,
                cells: vec!["Keith Morton".into()]
            }
        );
        // An L row's two cells pushed apart, the rest centred.
        assert_eq!(
            across(u16::from(b'L'), 1, 2),
            (640.0 / 3.0 - 120.0, Align::Left)
        );
        assert_eq!(
            across(u16::from(b'L'), 2, 2),
            (1280.0 / 3.0 + 120.0, Align::Right)
        );
        assert_eq!(across(0, 1, 1), (320.0, Align::Centre));
    }

    /// The roll's steps: the picture's fade, "Credits" at 530, the rise and
    /// the fades by the edges, the next row's start under the newest, and
    /// the end once all are gone.
    #[test]
    fn the_roll_rises_and_ends() {
        let rows = vec![
            Row {
                style: u16::from(b'H'),
                cells: vec!["A".into()],
            },
            Row {
                style: u16::from(b'L'),
                cells: vec!["B".into(), "C".into()],
            },
        ];
        let mut roll = Roll::new(Team::NuFx, rows);
        let h = |_s: u16| 20.0;
        for _ in 0..PICTURE_LOAD {
            roll.step(&h);
        }
        assert!(!roll.started());
        assert_eq!(roll.picture_alpha, 0);
        for _ in 0..42 {
            roll.step(&h);
        }
        assert!(roll.started());
        assert_eq!(roll.picture_alpha, 252);
        roll.step(&h);
        assert_eq!(roll.picture_alpha, 255);
        roll.step(&h);
        assert_eq!(roll.lines.len(), 1);
        assert_eq!(roll.lines[0].y, TITLE_Y);
        // The first row starts once the title's bottom + 12 is above 430:
        // the title at 530 needs (530 + 32 - 430) / 0.8 = 165 updates, one
        // more as the rises add up in floats.
        let mut n = 0;
        while roll.lines.len() == 1 {
            roll.step(&h);
            n += 1;
        }
        assert!((165..=166).contains(&n), "{n}");
        assert_eq!(roll.lines.len(), 2);
        let (title, a) = (&roll.lines[0], &roll.lines[1]);
        assert!((a.y - (title.y + 32.0)).abs() < 1e-3);
        // The title's top at 398: (480 - 398 - 50) / 50 of 256.
        assert!((title.y - 398.0).abs() < 1.0);
        assert!((160..176).contains(&title.alpha), "{}", title.alpha);
        assert_eq!(a.alpha, 0, "a new line shows from its first rise");
        // Both cells of the L row at once.
        for _ in 0..40 {
            roll.step(&h);
        }
        assert_eq!(roll.lines.len(), 4);
        assert_eq!(roll.lines[2].y, roll.lines[3].y);
        // A line halfway through the bottom fade.
        let mut l = roll.clone();
        l.lines[0].y = 405.8;
        l.step(&h);
        assert_eq!(l.lines[0].alpha, 128);
        // All gone: done.
        let mut n = 0;
        while !roll.done {
            roll.step(&h);
            n += 1;
            assert!(n < 10_000);
        }
        assert!(roll.lines.is_empty());
    }

    /// NuFX's roll in the runtime stood 55.44 s (counts 8,463 to 14,007)
    /// from its arrival to its way back to Options: the model, on the
    /// disc's text and fonts.
    #[test]
    fn nufx_roll_takes_as_long_as_the_runtimes() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let data = iso.read(ARCHIVE).unwrap();
        let texts = entries(&data).unwrap();
        assert_eq!(texts.len(), 5);
        let nufx = rows(texts[Team::NuFx.entry()]);
        assert_eq!(nufx[0].style, u16::from(b'T'));
        assert!(nufx[0].cells[0].contains("DEVELOPMENT TEAM"));
        let core_team = rows(texts[Team::CoreTeam.entry()]);
        assert!(core_team[0].cells[0].contains("CORE TEAM"));
        let fonts = street_assets::font::fonts(&mut iso).unwrap();
        let height = |s: u16| fonts[style(s).0].text_height as f32;
        let mut roll = Roll::new(Team::NuFx, nufx);
        while !roll.done {
            roll.step(&height);
        }
        let seconds = roll.updates as f64 / UPDATES;
        assert!((seconds - 55.44).abs() < 0.05, "{seconds}");
    }
}
