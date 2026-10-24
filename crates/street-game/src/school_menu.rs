//! Street School's lesson menu: the overlay the School's one game shows
//! between lessons (the in-game layout, `XLAY_GM` class 2, elements 69..88,
//! with the panel 1000 and the fourteen check rows 0x59..0x66 the HUD's
//! constructor adds).
//!
//! - `FUN_002b9988` opens it when a lesson commits (the manager's message
//!   `0x3c6960`, the listener's slot `+0x27c`): the gameplay's task groups
//!   suspended (`FUN_0033d380(0x19d2)`), the lesson ticked when it passed
//!   (the record's `+0x10`), the list back on the lesson just played, the
//!   HUD's elements 0..62 hidden (`FUN_002ba010`), and the menu's shown.
//!   With every lesson done and the user not yet paid (`+0x54` bit 0) it
//!   hides itself and ends the game on the completion (`FUN_002ba0a0`,
//!   `FUN_001ded40`).
//! - `FUN_002babf0` fills the category's rows (`PTR_DAT_003fb450`, fourteen
//!   per category, a row whose record has no title hidden) and focuses the
//!   first; `FUN_002ba1d8` refills the lesson's words when the focus moves.
//! - `FUN_002b3cf0` sets the state: 0 a lesson not done (Play, its
//!   description `+4`), 2 one done (Replay, its tip `+8`), 1 Replay pressed
//!   (Play again, the description, Back shown, the other rows dimmed to
//!   0x7f).
//! - `FUN_002b3ad0` takes the keys while no lesson runs (game vt+0xb4 is
//!   -1): Cross (0x23) plays the lesson (`FUN_002b4400`) or, on a done one,
//!   asks first (state 1); Triangle (0x25) backs out of state 1; L1 and R1
//!   (0x31, 0x33) step the category (`FUN_0021ee10`, `FUN_0021fa48`), not in
//!   state 1; Start (0x3d) pauses; up and down move the list
//!   (`FUN_002d7db0`).

use bevy::prelude::*;

use crate::boards::{Gm, Panels};
use crate::look::{self, Align, Labels, Look, Piece};

/// A lesson's record (`PTR_DAT_003fb3e8`, by its task `+0xc`): its title,
/// description and tip, and the lesson prompt's word (`+0x14`), button
/// (`+0x18`, `FUN_002b4288`'s action, -1 none) and move name (`+0x1c`;
/// `FUN_002b4400`, `FUN_002ba3b0`, [`crate::school_prompt`]).
#[derive(Clone, Copy, Debug)]
pub struct Record {
    pub title: u16,
    pub description: u16,
    pub tip: u16,
    pub word: u16,
    pub button: i8,
    pub name: u16,
}

const fn record(title: u16, description: u16, tip: u16, prompt: (u16, i8, u16)) -> Record {
    Record {
        title,
        description,
        tip,
        word: prompt.0,
        button: prompt.1,
        name: prompt.2,
    }
}

/// The 26 records by task (`0x3fb088..`: the pointers at `0x3fb3e8` put
/// 10 and 11 the other way round in memory; 0x20 bytes each, the fifth
/// word the run's done flag). The texts are `school.txt`'s
/// (file 0x5a); the descriptions and tips the PS2 build reads are the
/// "analog stick" ones (Give 'n Go's 0x5a2c, the pass tip 0x5a59).
pub const RECORDS: [Record; 26] = [
    record(0x5a02, 0x5a2d, 0x5a59, (0x5a6e, 0, 0x5a79)),
    record(0x5a03, 0x5a2e, 0x5a5a, (0x5a6e, 1, 0x5a7a)),
    record(0x5a04, 0x5a2f, 0x5a5b, (0x5a6e, 1, 0x5a7b)),
    record(0x5a05, 0x5a30, 0x5a5c, (0x5a6f, 1, 0x5a7c)),
    record(0x5a0d, 0x5a49, 0x5a4e, (0x5a6f, 1, 0x5a84)),
    record(0x5a06, 0x5a31, 0x5a5d, (0x5a6e, 0, 0x5a7d)),
    record(0x5a07, 0x5a32, 0x5a5e, (0x5a6e, 2, 0x5a7e)),
    record(0x5a09, 0x5a33, 0x5a5f, (0x5a6e, 0, 0x5a80)),
    record(0x5a0a, 0x5a34, 0x5a60, (0x5a6e, 2, 0x5a81)),
    record(0x5a0b, 0x5a35, 0x5a61, (0x5a6e, 3, 0x5a82)),
    record(0x5a23, 0x5a24, 0x5a50, (0x5a71, 2, 0x5a9a)),
    record(0x5a0c, 0x5a48, 0x5a4d, (0x5a71, 3, 0x5a83)),
    record(0x5a12, 0x5a36, 0x5a63, (0x5a70, 1, 0x5a89)),
    record(0x5a13, 0x5a37, 0x5a64, (0x5a72, 1, 0x5a8a)),
    record(0x5a14, 0x5a38, 0x5a65, (0x5a71, 1, 0x5a8b)),
    record(0x5a16, 0x5a39, 0x5a67, (0x5a6e, 6, 0x5a8d)),
    record(0x5a17, 0x5a3a, 0x5a68, (0x5a73, -1, 0x5a8e)),
    record(0x5a0e, 0x5a4b, 0x5a4f, (0x5a6e, 0, 0x5a85)),
    record(0x5a25, 0x5a3d, 0x5a52, (0x5a71, 0, 0x5a9b)),
    record(0x5a27, 0x5a3e, 0x5a53, (0x5a75, 0, 0x5a9c)),
    record(0x5a18, 0x5a3b, 0x5a69, (0x5a74, 2, 0x5a8f)),
    record(0x5a19, 0x5a3c, 0x5a6a, (0x5a74, 1, 0x5a90)),
    record(0x5a29, 0x5a2c, 0x5a56, (0x5a9e, 7, 0x5a9d)),
    record(0x5a1c, 0x5a42, 0x5a6d, (0x5a6e, 4, 0x5a1c)),
    record(0x5a1a, 0x5a40, 0x5a6b, (0x5a6f, 5, 0x5a1a)),
    record(0x5a1b, 0x5a41, 0x5a6c, (0x5a71, 1, 0x5a1b)),
];

/// The categories' lessons by task (`PTR_DAT_003fb450`: Offense, Defense,
/// Advanced; list 73's nodes 0x4b..0x4d).
pub const CATEGORIES: [&[usize]; 3] = [
    &[0, 1, 2, 3, 4, 5, 6, 14],
    &[7, 8, 9, 10, 11],
    &[12, 13, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25],
];

/// The category tab's words (`overlays.txt` 39..41, list 73's templates
/// 75..77).
const CATEGORY_WORDS: [u16; 3] = [0x5927, 0x5928, 0x5929];

/// List 73's height, which the category's word fills.
const CATEGORY_HEIGHT: f32 = 17.0;

/// The rows list 78 shows at once (its nodes 79..81).
pub const SHOWN: usize = 3;

/// The lessons a profile has done (bit per task).
pub fn all_done(done: u32) -> bool {
    done & ((1 << RECORDS.len()) - 1) == (1 << RECORDS.len()) - 1
}

/// `FUN_002b3cf0`'s states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// A lesson not done: Cross plays it.
    #[default]
    Play,
    /// Replay pressed on a done lesson: Cross plays it, Triangle backs out.
    Again,
    /// A done lesson: Cross asks first.
    Replay,
}

/// The menu's keys (`FUN_002b3ad0`'s actions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Cross,
    Triangle,
    /// L1 (-1) or R1 (+1).
    Shoulder(i32),
    /// Up (-1) or down (+1).
    Step(i32),
}

/// The menu's state.
#[derive(Resource, Debug, Default)]
pub struct LessonMenu {
    pub open: bool,
    pub category: usize,
    /// The lit row in the category, and the first row on screen.
    pub row: usize,
    pub top: usize,
    pub state: State,
    /// Updates the menu has been up: the lit row pulses by them.
    pub ticks: u64,
}

impl LessonMenu {
    /// The lit row's lesson (its task).
    pub fn lesson(&self) -> usize {
        CATEGORIES[self.category][self.row.min(CATEGORIES[self.category].len() - 1)]
    }

    /// Opens on the lesson's row (`FUN_002b9988`: list 78 refocused on the
    /// lesson played, `*(hud + 0x244) + 0x40`; the category stays).
    pub fn open_on(&mut self, lesson: usize, done: u32) {
        if let Some((category, row)) = CATEGORIES
            .iter()
            .enumerate()
            .find_map(|(c, list)| list.iter().position(|&t| t == lesson).map(|r| (c, r)))
        {
            self.category = category;
            self.row = row;
        }
        self.open = true;
        self.ticks = 0;
        self.settle(done);
    }

    /// The state by whether the lit lesson is done, and the list scrolled
    /// to keep it on screen.
    fn settle(&mut self, done: u32) {
        self.state = if done & (1 << self.lesson()) != 0 {
            State::Replay
        } else {
            State::Play
        };
        if self.row < self.top {
            self.top = self.row;
        }
        if self.row >= self.top + SHOWN {
            self.top = self.row + 1 - SHOWN;
        }
    }

    /// A key; returns the lesson to play when Cross starts one.
    pub fn key(&mut self, key: Key, done: u32) -> Option<usize> {
        match (key, self.state) {
            (Key::Cross, State::Replay) => self.state = State::Again,
            (Key::Cross, _) => return Some(self.lesson()),
            (Key::Triangle, State::Again) => self.state = State::Replay,
            (_, State::Again) | (Key::Triangle, _) => {}
            (Key::Shoulder(step), _) => {
                // `FUN_002babf0`: the new category's first row.
                self.category = (self.category as i32 + step).rem_euclid(3) as usize;
                self.row = 0;
                self.top = 0;
                self.settle(done);
            }
            (Key::Step(step), _) => {
                let last = CATEGORIES[self.category].len() - 1;
                self.row = (self.row as i32 + step).clamp(0, last as i32) as usize;
                self.settle(done);
            }
        }
        None
    }
}

/// The check row's art (`FUN_0021d9d0`: `XACOMMON` sheet 0, image 6,
/// frame 4 the box and frame 5 the tick).
pub struct SchoolArt {
    pub check: (Handle<Image>, Vec2),
    pub tick: (Handle<Image>, Vec2),
}

/// The panel (element 1000, code-made: measured on `school_lessons.png`,
/// its frame's lines at x 59.5/76.5 and 553.5/570, y 412/427.5, the band
/// 82..104), as [`crate::boards::panel_inked`] places one, and its body's alpha.
const PANEL: [f32; 4] = [67.0, 96.0, 494.0, 326.0];
const PANEL_BODY: u8 = 0xff;
/// Its lines, measured: the navy edge the description's own navy, the
/// blue and the blue-grey darker than the boards' panels'.
const PANEL_INK: crate::boards::PanelInk = crate::boards::PanelInk {
    blue: 0x30398d,
    navy: 0x00002e,
    grey: 0x808e9c,
};
/// The panel's inner pieces (measured): the lines in the frame's blue, 3
/// wide, the category and title row and the description's box in navy.
const LINES_Y: [f32; 3] = [142.5, 174.0, 373.0];
const LINES_X: (f32, f32) = (75.5, 552.5);
const DIVIDER_X: f32 = 266.0;
const BLUE: [u8; 3] = [0x30, 0x39, 0x8d];
const TAB_ROW: (Rect, [u8; 3]) = (
    Rect {
        min: Vec2::new(77.0, 144.0),
        max: Vec2::new(551.0, 172.5),
    },
    [0x00, 0x03, 0x4e],
);
const TEXT_BOX: (Rect, [u8; 3]) = (
    Rect {
        min: Vec2::new(267.5, 175.5),
        max: Vec2::new(551.0, 371.5),
    },
    [0x00, 0x00, 0x2e],
);
/// The L1 and R1 icons beside the category (measured): the elements 87
/// and 88 the HUD makes in code (`FUN_002b3cf0` shows them, and hides
/// them in state 1).
const SHOULDERS: [Vec2; 2] = [Vec2::new(110.0, 157.0), Vec2::new(232.0, 157.0)];
/// The legend's point once the menu shows (measured: its last pair ends at
/// 539, its middle at 395).
const LEGEND_AT: (u16, u16) = (539, 407);
/// The check row's words (`FUN_0021d9d0`): font 1 at 0.9, 5 right of the
/// box and on its middle, wrapped at 115, 0xff91adc0 idle.
const ROW_FONT: usize = 1;
const ROW_SCALE: f32 = 0.9;
const ROW_WRAP: f32 = 115.0;
const STEEL: [u8; 4] = [0x91, 0xad, 0xc0, 0xff];
/// The lesson's title over the description (element 71's row widget text,
/// measured cream).
const TITLE: [u8; 4] = look::colours::CREAM;

/// `text` wrapped to `width` in `font` at `scale`.
fn wrap(look: &Look, font: usize, scale: f32, text: &str, width: f32) -> String {
    let Some(f) = look.fonts.get(font) else {
        return text.to_owned();
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
            if !line.is_empty() && f.measure(&tried) as f32 * scale > width {
                out.push(std::mem::take(&mut line));
                line = word.to_owned();
            } else {
                line = tried;
            }
        }
        out.push(line);
    }
    out.join("\n")
}

/// The "%1d/%2d Lessons Complete" line (`school.txt` 0, element 70,
/// `FUN_002bb518`).
pub fn count_line(gm: &Gm, done: u32) -> String {
    let n = (done & ((1 << RECORDS.len()) - 1)).count_ones();
    let words = gm.text(0x5a00);
    let words = if words.is_empty() {
        "%1d/%2d Lessons Complete"
    } else {
        words
    };
    words
        .replacen("%1d", &n.to_string(), 1)
        .replacen("%2d", &RECORDS.len().to_string(), 1)
}

/// The menu's pieces, back to front.
#[allow(clippy::too_many_arguments)]
pub fn pieces(
    look: &Look,
    gm: &Gm,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    panels: &mut Panels,
    art: Option<&SchoolArt>,
    m: &LessonMenu,
    done: u32,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(layout) = gm.layout(2) else {
        return out;
    };
    let element = |id: u32| layout.elements.iter().find(|e| e.id == id && e.kind != 0);
    let spot = |id: u32| {
        element(id)
            .and_then(|e| e.position())
            .map(|(x, y)| Vec2::new(f32::from(x), f32::from(y)))
    };
    let rgb = |c: [u8; 3], a: f32| {
        Color::srgba(
            f32::from(c[0]) / 255.0,
            f32::from(c[1]) / 255.0,
            f32::from(c[2]) / 255.0,
            a,
        )
    };
    let fill = |out: &mut Vec<Piece>, r: Rect, c: [u8; 3]| {
        let mut p = Piece::new(gm.white.clone(), r);
        p.colour = rgb(c, 1.0);
        out.push(p);
    };
    out.push(panels.get_inked(PANEL, PANEL_BODY, PANEL_INK, gm, images));
    fill(&mut out, TAB_ROW.0, TAB_ROW.1);
    fill(&mut out, TEXT_BOX.0, TEXT_BOX.1);
    for y in LINES_Y {
        fill(
            &mut out,
            Rect::new(LINES_X.0 - 1.5, y - 1.5, LINES_X.1 + 1.5, y + 1.5),
            BLUE,
        );
    }
    fill(
        &mut out,
        Rect::new(DIVIDER_X - 1.5, LINES_Y[0], DIVIDER_X + 1.5, LINES_Y[2]),
        BLUE,
    );
    let text = |out: &mut Vec<Piece>,
                labels: &mut Labels,
                images: &mut Assets<Image>,
                font: usize,
                words: &str,
                colour: [u8; 4],
                at: Vec2,
                align: u8,
                alpha: f32| {
        let mut w = crate::hud::Writer {
            labels,
            look,
            images,
        };
        w.text(
            out,
            words,
            crate::hud::TextStyle::plain(font, colour),
            at,
            align,
            alpha,
        );
    };
    // The title (element 72, kind 35) in its font and colours.
    if let (Some(e), Some(at)) = (element(72), spot(72)) {
        let font = e.props.get(&13).map_or(6, |&f| f as usize);
        let colour = e.colour().unwrap_or(look::colours::PALE);
        let shadow = (
            e.props
                .get(&23)
                .map_or(look::colours::COOL.0, |&c| street_assets::font::argb(c)),
            e.props
                .get(&24)
                .map_or(look::colours::COOL.1, |&c| street_assets::font::argb(c)),
        );
        if let Some(p) = labels.text(
            look,
            images,
            font,
            gm.text(0x592a),
            colour,
            Some(shadow),
            at,
            Align::Left,
        ) {
            out.push(p);
        }
    }
    // The count (element 70).
    if let (Some(e), Some(at)) = (element(70), spot(70)) {
        let font = e.props.get(&13).map_or(2, |&f| f as usize);
        let colour = e.colour().unwrap_or(look::colours::PALE);
        text(
            &mut out,
            labels,
            images,
            font,
            &count_line(gm, done),
            colour,
            at,
            e.align,
            1.0,
        );
    }
    // The category on node 74 between its arrows (83, 82) and L1 and R1
    // (the code-made elements 87 and 88). State 1 hides the four
    // (`FUN_002b3cf0`: 87 and 88 hidden, and lists 73 and 78 lose their
    // visibility bits, `FUN_00249368(list, 0xff, 0)`, so their arrows go;
    // `advdunk3-after.png`).
    let asking = m.state == State::Again;
    if let Some(pm) = look.prematch.as_ref().filter(|_| !asking) {
        for (id, image) in [(83, &pm.arrow_left), (82, &pm.arrow_right)] {
            if let Some(at) = spot(id) {
                out.push(Piece::new(
                    image.clone(),
                    Rect::from_center_size(at, pm.arrow_side_size),
                ));
            }
        }
    }
    if let Some((atlas, frames)) = look.icons.as_ref().filter(|_| !asking) {
        for (at, frame) in SHOULDERS.iter().zip([5, 6]) {
            if let Some(&f) = frames.get(frame) {
                out.push(Piece::part(
                    atlas.clone(),
                    f,
                    Rect::from_center_size(*at, f.size()),
                ));
            }
        }
    }
    // The category's template (75..77, font 2) fills list 73's 17-pixel
    // height (measured: 0.83 of the font's size).
    if let (Some(at), Some(f)) = (spot(74), look.fonts.get(2)) {
        let mut w = crate::hud::Writer {
            labels,
            look,
            images,
        };
        let style = crate::hud::TextStyle {
            scale: CATEGORY_HEIGHT / f.text_height as f32,
            ..crate::hud::TextStyle::plain(2, look::colours::ROW_LIT)
        };
        w.text(
            &mut out,
            gm.text(CATEGORY_WORDS[m.category]),
            style,
            at,
            1,
            1.0,
        );
    }
    // The rows on nodes 79..81: the box, the tick when done, the words.
    let list = CATEGORIES[m.category];
    let pulse = f32::from(look::row_pulse(m.ticks)) / 255.0;
    for (k, id) in [79u32, 80, 81].into_iter().enumerate() {
        let index = m.top + k;
        let (Some(&lesson), Some(at)) = (list.get(index), spot(id)) else {
            continue;
        };
        let lit = index == m.row;
        let dim = m.state == State::Again && !lit;
        let bright = if dim {
            f32::from(crate::pause::DISABLED_BRIGHTNESS) / 255.0
        } else {
            1.0
        };
        let mut size = Vec2::new(24.0, 24.0);
        if let Some(art) = art {
            size = art.check.1;
            let mut p = Piece::new(art.check.0.clone(), Rect::from_corners(at, at + size));
            p.colour = Color::srgba(bright, bright, bright, 1.0);
            out.push(p);
            if done & (1 << lesson) != 0 {
                let min = at + Vec2::new(3.0, -4.0);
                let mut p = Piece::new(
                    art.tick.0.clone(),
                    Rect::from_corners(min, min + art.tick.1),
                );
                p.colour = Color::srgba(bright, bright, bright, 1.0);
                out.push(p);
            }
        }
        let words = wrap(
            look,
            ROW_FONT,
            ROW_SCALE,
            gm.text(RECORDS[lesson].title),
            ROW_WRAP,
        );
        let colour = if lit { look::colours::ROW_LIT } else { STEEL };
        let colour = colour.map(|c| (f32::from(c) * bright) as u8);
        let mut w = crate::hud::Writer {
            labels,
            look,
            images,
        };
        let style = crate::hud::TextStyle {
            scale: ROW_SCALE,
            ..crate::hud::TextStyle::plain(ROW_FONT, colour)
        };
        let at = at + Vec2::new(size.x + 5.0, size.y / 2.0);
        w.text(
            &mut out,
            &words,
            style,
            at,
            4,
            if lit && !asking { pulse } else { 1.0 },
        );
    }
    // The list's arrows: up while rows hide above, down while below (not
    // in state 1).
    if let Some(pm) = look.prematch.as_ref().filter(|_| !asking) {
        for (id, image, shown) in [
            (84, &pm.arrow_up, m.top > 0),
            (85, &pm.arrow_down, m.top + SHOWN < list.len()),
        ] {
            if let (true, Some(at)) = (shown, spot(id)) {
                out.push(Piece::new(
                    image.clone(),
                    Rect::from_center_size(at, pm.arrow_size),
                ));
            }
        }
    }
    // The lesson's title (71) and its description or tip (86).
    let record = &RECORDS[m.lesson()];
    if let (Some(e), Some(at)) = (element(71), spot(71)) {
        let font = e.props.get(&13).map_or(2, |&f| f as usize);
        text(
            &mut out,
            labels,
            images,
            font,
            gm.text(record.title),
            TITLE,
            at,
            e.align,
            1.0,
        );
    }
    if let (Some(e), Some(at)) = (element(86), spot(86)) {
        let font = e.props.get(&13).map_or(1, |&f| f as usize);
        let width = e.size().map_or(253.0, |(w, _)| f32::from(w));
        let id = if m.state == State::Replay {
            record.tip
        } else {
            record.description
        };
        // The text box's scale (prop 0x2c: whole part, then thousandths;
        // 0.88 here).
        let scale = e
            .props
            .get(&0x2c)
            .map_or(1.0, |&v| (v & 0xffff) as f32 + (v >> 16) as f32 / 1000.0);
        let words = wrap(look, font, scale, gm.text(id), width);
        let mut w = crate::hud::Writer {
            labels,
            look,
            images,
        };
        let style = crate::hud::TextStyle {
            scale,
            ..crate::hud::TextStyle::plain(font, STEEL)
        };
        w.text(&mut out, &words, style, at, 0, 1.0);
    }
    // The legend (69): Pause; Play, or Replay on a done lesson; Back in
    // state 1 (`FUN_002b3cf0`: label 1 0x590a or 0x5908, pair 2 shown only
    // then).
    if let Some(e) = element(69) {
        let mut e = e.clone();
        e.props
            .insert(3, (u32::from(LEGEND_AT.1) << 16) | u32::from(LEGEND_AT.0));
        let state = m.state;
        look::legend(&mut out, labels, look, images, &e, &|id| {
            let id = match (id, state) {
                (0x5908, State::Replay) => 0x5908,
                (0x5908, _) => 0x590a,
                (0x5907, State::Again) => 0x5907,
                (0x5907, _) => return None,
                (id, _) => id,
            };
            Some(gm.text(id).to_owned()).filter(|s| !s.is_empty())
        });
    }
    out
}

/// One UI node showing a piece of the menu.
#[derive(Component)]
pub struct MenuSlot(usize);

/// Points the menu's slots at this frame's pieces: over the match and the
/// HUD, under the boards' dialogs and the pause screen.
pub fn show(
    commands: &mut Commands,
    slots: &mut Query<(&MenuSlot, &mut ImageNode, &mut Node, &mut Visibility)>,
    pieces: &[Piece],
) {
    let pct = |v: f32, of: f32| Val::Percent(v / of * 100.0);
    let place = |node: &mut Node, r: &Rect| {
        node.left = pct(r.min.x, 640.0);
        node.top = pct(r.min.y, 480.0);
        node.width = pct(r.width(), 640.0);
        node.height = pct(r.height(), 480.0);
    };
    let mut have = 0;
    for (slot, mut image, mut node, mut vis) in slots.iter_mut() {
        have = have.max(slot.0 + 1);
        match pieces.get(slot.0) {
            Some(p) => {
                if image.image != p.image {
                    image.image = p.image.clone();
                }
                image.rect = p.source;
                image.flip_x = p.flip;
                image.flip_y = p.flip_y;
                image.color = p.colour;
                place(&mut node, &p.rect);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    for (k, p) in pieces.iter().enumerate().skip(have) {
        let mut node = Node {
            position_type: PositionType::Absolute,
            ..default()
        };
        place(&mut node, &p.rect);
        let mut image = ImageNode::new(p.image.clone()).with_mode(NodeImageMode::Stretch);
        image.rect = p.source;
        image.flip_x = p.flip;
        image.flip_y = p.flip_y;
        image.color = p.colour;
        commands.spawn((
            image,
            node,
            Visibility::Inherited,
            GlobalZIndex(300 + k as i32),
            MenuSlot(k),
            DespawnOnExit(crate::Screen::Playing),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_categories_hold_every_lesson_once() {
        let mut seen: Vec<usize> = CATEGORIES.iter().flat_map(|c| c.iter().copied()).collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..26).collect::<Vec<_>>());
        assert_eq!(RECORDS[22].title, 0x5a29);
    }

    #[test]
    fn the_keys_move_the_list_and_ask_before_a_replay() {
        let mut m = LessonMenu::default();
        m.open_on(0, 0);
        assert_eq!((m.category, m.row, m.state), (0, 0, State::Play));
        // Down to the last offence row: the list scrolls one at a time.
        for _ in 0..10 {
            m.key(Key::Step(1), 0);
        }
        assert_eq!((m.row, m.top, m.lesson()), (7, 5, 14));
        m.key(Key::Step(-1), 0);
        assert_eq!((m.row, m.top), (6, 5));
        // L1 wraps to Advanced, its first row.
        m.key(Key::Shoulder(-1), 0);
        assert_eq!((m.category, m.row, m.top, m.lesson()), (2, 0, 0, 12));
        assert_eq!(m.key(Key::Cross, 0), Some(12));
        // A done lesson asks first; Triangle backs out; L1 waits.
        let done = 1 << 12;
        m.open_on(12, done);
        assert_eq!(m.state, State::Replay);
        assert_eq!(m.key(Key::Cross, done), None);
        assert_eq!(m.state, State::Again);
        m.key(Key::Shoulder(1), done);
        assert_eq!((m.category, m.state), (2, State::Again));
        m.key(Key::Triangle, done);
        assert_eq!(m.state, State::Replay);
        m.key(Key::Cross, done);
        assert_eq!(m.key(Key::Cross, done), Some(12));
    }

    #[test]
    fn the_menu_reopens_on_the_lesson_played() {
        let mut m = LessonMenu::default();
        m.open_on(10, 1 << 10);
        assert_eq!((m.category, m.row, m.state), (1, 3, State::Replay));
        assert_eq!(m.top, 1);
        assert!(all_done((1 << 26) - 1) && !all_done((1 << 26) - 2));
    }
}
