//! The pause screen (screen class 0 of `XLAY_GM`, `pause.txt`) and the
//! dialog widget its Quit Game and the post-game choices open.
//!
//! - **Build** (`FUN_002bc3f8`): the panel's body pieces take alpha 0xc0
//!   (`FUN_00211428`); the rows are registered in the order of the table
//!   at `0x48e250` (node id, label), each value widget then its label row
//!   (`FUN_0021f508`), so the list's nodes (kind 10, list 3, by prop 34)
//!   put the values on x 442, centred (`FUN_0024a4a0(2)`), and the labels
//!   on x 125, left, both on their node's middle (`FUN_0024a4e8(0x20)`),
//!   in font 2 (`FUN_002e5578(2)`). The labels are row widgets with stars
//!   (`FUN_0021c628(1)`). Camera Type, Onscreen Help, Visual Effects and
//!   Replays are choice lists (`0x48e210`); the three volumes are sliders
//!   (`FUN_0021d148`, `0x48e240`); Controller Layout, Trax and Quit
//!   Game have empty values (`0x48e248`).
//! - **Disabled rows** (`FUN_002bcd18`): in Street School (the front
//!   end's mode, game record +0x78, 1) Replays, Trax and Onscreen
//!   Help; Camera Type in mode 3, in mode 2 with more than two users and
//!   in mode 4 with more than one; Quit Game once the game's phase
//!   (+0xf4) is 9. Each label and value widget is disabled (vtable
//!   +0x114), which dims its text to half brightness
//!   ([`DISABLED_BRIGHTNESS`]).
//! - **Input** (`FUN_002bd038`): Start closes an open page, else the pause
//!   (`FUN_002bd660(0)`, the settings kept); confirm on Controller Layout
//!   opens the controller panel (`FUN_002144c8` at (70, 105)), on Trax
//!   the song list (`FUN_00218e68` at (10, 10)), on Quit Game the dialog
//!   (`FUN_002bd540`). The title follows the page (`FUN_002bca30`):
//!   "Pause", "Controller Layout" (0x5823), "Trax" (0x5821); the
//!   Continue legend hides on the controller panel, Preview shows with
//!   the song list.
//! - **Quit Game** (`FUN_002bd540`): "Are you sure you\n want to quit?"
//!   (0x580d) over Yes, No and, in Pick Up (modes 2 and 3), REMATCH
//!   (0x580e), No lit. Yes quits (`FUN_002bd660(2)`); REMATCH opens the
//!   rematch rules (`FUN_0025dc88`, 0x5df), then plays again.
//! - **The dialog** (`FUN_0025c860`; layout `FUN_0025cda0`): its words
//!   (font 2) then its rows, 20 below the words and 6 between rows, all
//!   centred on the dialog's point; the panel (`FUN_00211278`, centred)
//!   grows to take each 40 wider all round (`FUN_00212008` via
//!   `FUN_0025d000`), then to 4:3 (`fit`: the width to 4/3 of the
//!   height, or the height to 3/4 of the width). The dialog disables its
//!   parent screen (vtable +0x114), which then shows dimmed.

use bevy::prelude::*;
use street_sim::records::Settings;

use crate::{
    audio::AudioSettings,
    boards::{Gm, Panels},
    look::{self, Align, Labels, Look, Piece, RowSpot},
};

/// The rows, in the list's order (`0x48e250`).
pub mod row {
    pub const LAYOUT: usize = 0;
    pub const TRAX: usize = 1;
    pub const CAMERA: usize = 2;
    pub const SPEECH: usize = 3;
    pub const MUSIC: usize = 4;
    pub const SFX: usize = 5;
    pub const REPLAYS: usize = 6;
    pub const HELP: usize = 7;
    pub const EFFECTS: usize = 8;
    pub const QUIT: usize = 9;
    pub const COUNT: usize = 10;
}

/// Each row's label (`pause.txt`, by `0x48e250`).
const LABELS: [u16; row::COUNT] = [
    0x5809, 0x5821, 0x5806, 0x5802, 0x5804, 0x5803, 0x580b, 0x5807, 0x5808, 0x580c,
];
/// The camera choices (`0x48e210`), in `options::CAMERA_TYPES`' order:
/// Street, Court, Slide, Solid, Wide (0x5813 Behind is left out).
const CAMERA_WORDS: [u16; 5] = [0x580f, 0x5810, 0x5811, 0x5812, 0x5814];
/// Off, On.
const OFF_ON: [u16; 2] = [0x5816, 0x5815];
/// Replays' choices in the list's order and the setting each saves
/// (`FUN_002bdc60`: Always 0, Automatic 1, Manual 2).
pub const REPLAYS: [(u8, u16); 3] = [(1, 0x5817), (0, 0x5818), (2, 0x5819)];
/// The titles: "Pause", "Controller Layout", "Trax".
const TITLES: [u16; 3] = [0x5800, 0x5823, 0x5821];
/// The volumes' sliders run 0..10.
pub const VOLUME_MAX: u8 = 10;

/// The class 0 layout (kind 10 nodes, list 3): the labels' and values'
/// x and the rows' y (the eleventh node pair, y 395, has no row).
pub const LABEL_X: f32 = 125.0;
pub const VALUE_X: f32 = 442.0;
pub const ROW_Y: [f32; row::COUNT] = [
    126.0, 152.0, 179.0, 206.0, 233.0, 260.0, 287.0, 314.0, 341.0, 368.0,
];
/// The rows' font and the dialogs' (`FUN_002e5578(2)`, `FUN_002bd540`).
pub const FONT: usize = 2;
/// The panel's body alpha (`FUN_00211428(panel, 0xc0)`).
const PANEL_BODY: u8 = 0xc0;

/// The page the pause screen shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    List,
    /// The controller panel (`+0x108`).
    Layout,
    /// The song list (`+0x10c`).
    Trax,
}

/// What decides the disabled rows: the front end's mode (game record
/// +0x78: 1 Street School, 2 and 3 Pick Up, 4 the League Challenge, 5 Be A
/// Legend), the users playing (`FUN_001db248`) and whether the game is
/// over (phase 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    pub mode: u32,
    pub users: usize,
    pub over: bool,
}

/// `FUN_002bcd18`'s disabled rows.
pub fn disabled(c: Context) -> [bool; row::COUNT] {
    let mut out = [false; row::COUNT];
    if c.mode == 1 {
        out[row::REPLAYS] = true;
        out[row::TRAX] = true;
        out[row::HELP] = true;
    }
    if c.mode == 3 || (c.mode == 2 && c.users > 2) || (c.mode == 4 && c.users > 1) {
        out[row::CAMERA] = true;
    }
    if c.over {
        out[row::QUIT] = true;
    }
    out
}

/// The next row up or down that is not disabled (the list walks its
/// nodes' links, `FUN_0021f230` leaving the disabled out); stays put
/// when there is none.
pub fn next_row(from: usize, step: i32, disabled: &[bool; row::COUNT]) -> usize {
    let n = row::COUNT as i32;
    let mut k = from as i32;
    for _ in 0..row::COUNT {
        k = (k + step).rem_euclid(n);
        if !disabled[k as usize] {
            return k as usize;
        }
    }
    from
}

/// Left or right on a row with a value (`FUN_002bdc60` saves the choice):
/// the volumes clamp at 0 and 10, the choice lists wrap.
pub fn step(row: usize, step: i32, o: &mut Settings, sound: &mut AudioSettings) {
    let volume = |v: &mut u8| *v = (i32::from(*v) + step).clamp(0, i32::from(VOLUME_MAX)) as u8;
    match row {
        row::SPEECH => volume(&mut sound.speech),
        row::MUSIC => volume(&mut sound.music),
        row::SFX => volume(&mut sound.effects),
        row::HELP => o.help = !o.help,
        row::EFFECTS => o.visual_effects = !o.visual_effects,
        row::CAMERA => o.camera = crate::options::step_camera(o.camera, step),
        row::REPLAYS => {
            let at = REPLAYS
                .iter()
                .position(|&(v, _)| v == o.replays)
                .unwrap_or(0);
            o.replays = REPLAYS[(at as i32 + step).rem_euclid(REPLAYS.len() as i32) as usize].0;
        }
        _ => {}
    }
}

/// A row's value: a choice's word or a slider's level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    None,
    Word(u16),
    Slider(u8),
}

/// `FUN_002bd8f8`: what each row shows for these settings.
pub fn value(row: usize, o: &Settings, sound: &AudioSettings) -> Value {
    match row {
        row::SPEECH => Value::Slider(sound.speech.min(VOLUME_MAX)),
        row::MUSIC => Value::Slider(sound.music.min(VOLUME_MAX)),
        row::SFX => Value::Slider(sound.effects.min(VOLUME_MAX)),
        row::CAMERA => {
            let at = crate::options::CAMERA_TYPES
                .iter()
                .position(|&(v, _)| v == o.camera)
                .unwrap_or(0);
            Value::Word(CAMERA_WORDS[at])
        }
        row::HELP => Value::Word(OFF_ON[usize::from(o.help)]),
        row::EFFECTS => Value::Word(OFF_ON[usize::from(o.visual_effects)]),
        row::REPLAYS => Value::Word(
            REPLAYS
                .iter()
                .find(|&&(v, _)| v == o.replays)
                .map_or(REPLAYS[0].1, |&(_, w)| w),
        ),
        _ => Value::None,
    }
}

/// The controller panel's state: the pausing player's layout and what
/// the panel's cursor is on.
pub struct Controller {
    pub layout: u8,
    pub player: usize,
    pub view: u8,
    pub cursor: usize,
    pub vibration: bool,
}

/// What the pause screen shows this frame.
pub struct View<'a> {
    pub page: Page,
    pub cursor: usize,
    pub settings: &'a Settings,
    pub sound: &'a AudioSettings,
    /// Updates since the screen opened: the lit row pulses by them.
    pub ticks: u64,
    pub controller: Controller,
    /// The song list's rows and its cursor.
    pub trax: &'a Trax,
    pub trax_cursor: usize,
    /// `FUN_002bcd18`'s disabled rows ([`disabled`]).
    pub disabled: [bool; row::COUNT],
}

/// A disabled text's brightness (`+0x5a`): the text widget's disable
/// hook (vtable `+0x8c`, `FUN_00239990`) sets 0x7f when its flag 0x10
/// (`+0x5b`, set by the widget constructor `FUN_00248a90`) is on, and the
/// text draw (`FUN_00238d20`) scales red, green and blue by it over 255.
/// Disabling a row (vtable `+0x114`, `FUN_002491d8`) disables its
/// children, so its label and its value both dim.
pub const DISABLED_BRIGHTNESS: u8 = 0x7f;

/// Dims the last piece as a disabled text.
fn dim_last(out: &mut [Piece]) {
    if let Some(p) = out.last_mut() {
        let k = f32::from(DISABLED_BRIGHTNESS) / 255.0;
        let c = p.colour.to_srgba();
        p.colour = Color::srgba(c.red * k, c.green * k, c.blue * k, c.alpha);
    }
}

/// The song list: Music Experience, then each in-game song.
#[derive(Default)]
pub struct Trax {
    pub label: String,
    pub value: String,
    /// Title, artist, on.
    pub songs: Vec<(String, String, bool)>,
}

/// A dialog's words and rows.
pub struct Dialog<'a> {
    pub text: &'a str,
    pub rows: &'a [String],
    /// The rows' font (`+0x104`, the constructor's last argument: 2 for
    /// the pause's and the post-game's, 3 for Choose Baller's new-baller
    /// dialog, `FUN_002cdf10`).
    pub row_font: usize,
    pub lit: usize,
    pub ticks: u64,
    /// How dark the screen it opens over goes (0 for none): [`PAUSE_DIM`]
    /// in a game, [`FRONT_END_DIM`] in the menus.
    pub dim: f32,
    /// The width the words wrap to, when the dialog takes one
    /// (`FUN_0025d5c8`, `FUN_0025d660`, `FUN_0025d6d8`): the panel is at
    /// least as wide; 0 for the dialogs sized by their words alone.
    pub width: f32,
}

/// Where the dialogs stand (`DAT_003f7170`, set at run time): measured,
/// the box's middle in the original's Quit Game and post-game captures
/// is (320, 237), 3 above the dialog's point (the box's own middle is
/// (0, -3) of it, see [`dialog_box`]).
pub const DIALOG_AT: Vec2 = Vec2::new(320.0, 240.0);
/// `FUN_0025d000`: the panel takes each child 40 wider all round.
const DIALOG_MARGIN: f32 = 40.0;
/// `FUN_0025cda0`: 20 under the words, 6 between rows.
const DIALOG_TEXT_GAP: f32 = 20.0;
const DIALOG_ROW_GAP: f32 = 6.0;
/// How dark a screen a dialog disables shows: measured, the pause screen
/// and the court behind it keep 0.34 of their light under the Quit Game
/// dialog (the disabled draw is not traced).
pub const PAUSE_DIM: f32 = 0.66;
/// The front end's dialogs keep more: 0.40..0.48 of the screen's light on
/// the original's buy questions, the baller's dialog, the wizard's
/// confirmation and Rewards' "cannot" (0.45 on the whole).
pub const FRONT_END_DIM: f32 = 0.55;

/// The dialog's panel (left, top, width, height, as a kind 28 element
/// places it) and its words' and rows' tops, for words `text_h` tall
/// (0 for none) and rows `row_h` tall each, the widest `widest`.
pub fn dialog_box(widest: f32, text_h: f32, row_h: f32, rows: usize) -> ([f32; 4], f32, Vec<f32>) {
    let mut total = if text_h > 0.0 {
        text_h + DIALOG_TEXT_GAP
    } else {
        0.0
    };
    total += rows as f32 * (row_h + DIALOG_ROW_GAP);
    let mut y = -total / 2.0;
    let text_top = y;
    let mut bottom = y;
    if text_h > 0.0 {
        bottom = y + text_h;
        y += text_h + DIALOG_TEXT_GAP;
    }
    let mut tops = Vec::with_capacity(rows);
    for _ in 0..rows {
        tops.push(y);
        bottom = y + row_h;
        y += row_h + DIALOG_ROW_GAP;
    }
    // The panel starts empty on the dialog's point and grows to each
    // child's box and 40 more.
    let top = (-total / 2.0 - DIALOG_MARGIN).min(0.0);
    let bottom = (bottom + DIALOG_MARGIN).max(0.0);
    let (w0, h0) = (widest + 2.0 * DIALOG_MARGIN, bottom - top);
    let (w, h) = if w0 < h0 * 4.0 / 3.0 {
        (h0 * 4.0 / 3.0, h0)
    } else {
        (w0, w0 * 0.25 * 3.0)
    };
    let middle = DIALOG_AT + Vec2::new(0.0, (top + bottom) / 2.0);
    (
        [middle.x - w / 2.0, middle.y - h / 2.0, w, h],
        DIALOG_AT.y + text_top,
        tops.into_iter().map(|t| DIALOG_AT.y + t).collect(),
    )
}

/// A dialog's words broken into lines of at most `width` pixels in its
/// font (the dialogs that take a width, `FUN_0025d5c8`, `FUN_0025d660` and
/// `FUN_0025d6d8`: 200, 250 or 350; `FUN_0025d1e8` gives the text widget
/// its font before its words), whole words a line, a line measured to its
/// last glyph's ink (Rewards' "cannot" fits "You do not possess enough" on
/// its first line, `rewards_cannot.png`), the text's own breaks kept; a
/// width of 0 keeps the text as written.
pub fn wrap(look: &Look, text: &str, width: f32) -> String {
    let Some(font) = look.fonts.get(FONT).filter(|_| width > 0.0) else {
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
            if !line.is_empty() && font.ink_width(&tried) as f32 > width {
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

/// A dialog's pieces (`FUN_0025c860`), dimming what is under it when it
/// was opened over a screen.
pub fn dialog(
    look: &Look,
    gm: &Gm,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    panels: &mut Panels,
    d: &Dialog,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(font) = look.fonts.get(FONT) else {
        return out;
    };
    let (th, lh) = (font.text_height as f32, font.line_height as f32);
    let row_font = look.fonts.get(d.row_font).unwrap_or(font);
    let row_h = row_font.text_height as f32;
    let text = d.text.replace("\\n", "\n");
    let lines: Vec<&str> = if text.is_empty() {
        Vec::new()
    } else {
        text.split('\n').collect()
    };
    let text_h = if lines.is_empty() {
        0.0
    } else {
        th + (lines.len() - 1) as f32 * lh
    };
    let widest = lines
        .iter()
        .map(|l| font.measure(l) as f32)
        .chain(d.rows.iter().map(|r| row_font.measure(r) as f32))
        .fold(d.width, f32::max);
    let (rect, text_top, tops) = dialog_box(widest, text_h, row_h, d.rows.len());
    if d.dim > 0.0 {
        let mut p = Piece::new(gm.white.clone(), Rect::new(0.0, 0.0, 640.0, 480.0));
        p.colour = Color::srgba(0.0, 0.0, 0.0, look::black_alpha(d.dim));
        out.push(p);
    }
    out.push(panels.get(rect, 0xff, gm, images));
    for (k, line) in lines.iter().enumerate() {
        let at = Vec2::new(DIALOG_AT.x, text_top + lh * k as f32);
        if let Some(p) = labels.text(look, images, FONT, line, [0xff; 4], None, at, Align::Centre) {
            out.push(p);
        }
    }
    let mut stars = Vec::new();
    for (k, (row, top)) in d.rows.iter().zip(tops).enumerate() {
        let spot = RowSpot {
            at: Vec2::new(DIALOG_AT.x, top),
            font: d.row_font,
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
            k == d.lit,
            true,
            d.ticks,
        );
    }
    out.extend(stars);
    out
}

/// The pause screen's pieces, back to front.
pub fn pieces(
    look: &Look,
    gm: &Gm,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    panels: &mut Panels,
    v: &View,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(layout) = gm.layout(0) else {
        return out;
    };
    // The layout's elements, last first (`FUN_002e6338`).
    for e in layout.elements.iter().rev() {
        let Some((x, y)) = e.position() else {
            continue;
        };
        let at = Vec2::new(f32::from(x), f32::from(y));
        match e.kind {
            28 => {
                let (w, h) = e.size().unwrap_or((518, 349));
                out.push(panels.get(
                    [at.x, at.y, f32::from(w), f32::from(h)],
                    PANEL_BODY,
                    gm,
                    images,
                ));
            }
            35 => {
                let title = match v.page {
                    Page::List => TITLES[0],
                    Page::Layout => TITLES[1],
                    Page::Trax => TITLES[2],
                };
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
                    gm.text(title),
                    colour,
                    Some(shadow),
                    at,
                    Align::Left,
                ) {
                    out.push(p);
                }
            }
            34 => {
                let shown = match e.id {
                    1 => v.page != Page::Layout,
                    2 => v.page == Page::Trax,
                    _ => false,
                };
                if shown {
                    look::legend(&mut out, labels, look, images, e, &|id| {
                        Some(gm.text(id).to_owned()).filter(|s| !s.is_empty())
                    });
                }
            }
            _ => {}
        }
    }
    let mut stars = Vec::new();
    match v.page {
        Page::List => {
            for (k, &label) in LABELS.iter().enumerate() {
                let lit = k == v.cursor;
                let off = v.disabled[k];
                let y = ROW_Y[k];
                match value(k, v.settings, v.sound) {
                    Value::Word(word) => {
                        let spot = RowSpot {
                            at: Vec2::new(VALUE_X, y),
                            font: FONT,
                            align: Align::Centre,
                            valign: 1,
                        };
                        look::screen_row(
                            &mut out,
                            &mut stars,
                            labels,
                            look,
                            images,
                            spot,
                            gm.text(word),
                            lit,
                            false,
                            v.ticks,
                        );
                        if off {
                            dim_last(&mut out);
                        }
                    }
                    Value::Slider(level) => {
                        look.slider_at(&mut out, Vec2::new(VALUE_X, y), level, VOLUME_MAX, lit);
                    }
                    Value::None => {}
                }
                let spot = RowSpot {
                    at: Vec2::new(LABEL_X, y),
                    font: FONT,
                    align: Align::Left,
                    valign: 1,
                };
                look::screen_row(
                    &mut out,
                    &mut stars,
                    labels,
                    look,
                    images,
                    spot,
                    gm.text(label),
                    lit,
                    true,
                    v.ticks,
                );
                if off {
                    dim_last(&mut out);
                }
            }
        }
        Page::Layout => controller(&mut out, look, labels, images, gm, &v.controller, v.ticks),
        Page::Trax => trax(&mut out, look, labels, images, v),
    }
    out.extend(stars);
    out
}

/// The song list's rows: Music Experience (`bigradio.txt` 2: Playlist
/// 8, Interactive 9), then each in-game song, its title over its artist,
/// on or off.
pub fn trax_rows(o: &Settings, songs: &[street_audio::music::Song], radio: &[String]) -> Trax {
    let word = |k: usize, fallback: &str| radio.get(k).cloned().unwrap_or_else(|| fallback.into());
    Trax {
        label: word(2, "Music Experience"),
        value: if o.interactive {
            word(9, "Interactive")
        } else {
            word(8, "Playlist")
        },
        songs: songs
            .iter()
            .enumerate()
            .take(street_sim::records::FRONT_END_BIT as usize)
            .map(|(k, s)| {
                (
                    s.title.clone(),
                    s.artist.clone(),
                    o.songs_off & (1 << k) == 0,
                )
            })
            .collect(),
    }
}

/// The panel's band blue (`0x424a9c`, the panel's own inner line).
const BAND: [u8; 4] = [0x42, 0x4a, 0x9c, 0xff];
/// The legends' amber (the layout's kind 34 colour, `0xffebb423`).
const LEGEND_AMBER: [u8; 4] = [0xeb, 0xb4, 0x23, 0xff];
/// More icons on `XACOMMON` sheet 1, image 0: the left stick, R3 and the
/// plain Start arrow.
const STICK: Rect = Rect {
    min: Vec2::new(168.0, 51.0),
    max: Vec2::new(192.0, 77.0),
};
const R3: Rect = Rect {
    min: Vec2::new(57.0, 169.0),
    max: Vec2::new(73.0, 191.0),
};
const ARROW: Rect = Rect {
    min: Vec2::new(99.0, 54.0),
    max: Vec2::new(125.0, 73.0),
};

/// A text's piece with its middle on `at.y`; returns its width.
#[allow(clippy::too_many_arguments)]
fn line(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    font: usize,
    text: &str,
    colour: [u8; 4],
    at: Vec2,
    align: Align,
) -> f32 {
    let Some(f) = look.fonts.get(font) else {
        return 0.0;
    };
    let top = at.y - f.text_height as f32 / 2.0;
    if let Some(p) = labels.text(
        look,
        images,
        font,
        text,
        colour,
        None,
        Vec2::new(at.x, top),
        align,
    ) {
        out.push(p);
    }
    f.measure(text) as f32
}

/// Pulses the last piece as a lit row's text ([`look::row_pulse`]).
fn pulse(out: &mut [Piece], ticks: u64) {
    if let Some(p) = out.last_mut() {
        p.colour = Color::srgba(1.0, 1.0, 1.0, f32::from(look::row_pulse(ticks)) / 255.0);
    }
}

/// An icon of the pad's atlas, `height` tall, its middle on `centre`;
/// returns its width.
fn icon(out: &mut Vec<Piece>, look: &Look, source: Rect, centre: Vec2, height: f32) -> f32 {
    let Some([atlas, _]) = look.pad.clone() else {
        return 0.0;
    };
    let w = height * source.width() / source.height();
    out.push(Piece::part(
        atlas,
        source,
        Rect::from_center_size(centre, Vec2::new(w, height)),
    ));
    w
}

/// A lit value between the side arrows (the row widget's arrow children,
/// `FUN_0021c628`), 14 out from the text.
fn arrows(out: &mut Vec<Piece>, look: &Look, centre: Vec2, width: f32) {
    let Some(art) = &look.prematch else {
        return;
    };
    let size = art.arrow_side_size;
    let gap = 14.0;
    let left = Vec2::new(
        centre.x - width / 2.0 - gap - size.x,
        centre.y - size.y / 2.0,
    );
    let right = Vec2::new(centre.x + width / 2.0 + gap, centre.y - size.y / 2.0);
    out.push(Piece::new(
        art.arrow_left.clone(),
        Rect::from_corners(left, left + size),
    ));
    out.push(Piece::new(
        art.arrow_right.clone(),
        Rect::from_corners(right, right + size),
    ));
}

/// Where the pause screen puts the controller widget (`FUN_002bc3f8`:
/// vtable `+0x11c` with (70, 105)).
pub const CONTROLLER_AT: Vec2 = Vec2::new(70.0, 105.0);

/// The controller widget's button rows (`FUN_002144c8`'s children, the
/// same on the pause screen and the front end's Controller Layout page):
/// `FUN_00216dd8`'s icons at their own size (`FUN_00212748`, middle
/// anchored) with their left edges at x 259 and 294 of the widget, and
/// `FUN_002164a0`'s words in font 0, 0xffdcdcbb, from x 299 (the turbo
/// rows' from 329), each row on its middle: the stick and Move Player at
/// y 25, Cross 60, Circle 85, Triangle 115, Square 145, R1 R2 168 (their
/// icons 170), L1 L2 193 (195), R3 and Strategy 225, Pause 258. On
/// offence the face rows read the layout's words ([`diagram::offence_face`]),
/// on defence what the layout makes each button do.
///
/// [`diagram::offence_face`]: crate::diagram::offence_face
pub(crate) fn controller_rows(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    at: Vec2,
    layout: u8,
    view: u8,
) {
    use crate::diagram::Icon;
    use street_sim::button;
    let words = |k: usize, fallback: &str| {
        look.text("contconfig.txt", k)
            .unwrap_or(fallback)
            .to_owned()
    };
    let offence = view == 0;
    let offence_words = crate::diagram::offence_face(layout);
    let face = |row: usize, physical: u16| {
        if offence {
            let (_, k, fallback) = offence_words[row];
            (k, fallback)
        } else {
            crate::diagram::face_action(layout, physical, false)
        }
    };
    let strategy = if offence { 26 } else { 25 };
    // (icons with their left edges and middle y, words, the words' x and y).
    let rows: [(&[(Rect, f32)], (usize, &str), f32, f32, f32); 9] = [
        (&[(STICK, 262.0)], (6, "Move Player"), 299.0, 25.0, 25.0),
        (
            &[(Icon::Cross.source(), 259.0)],
            face(0, button::PASS),
            299.0,
            60.0,
            60.0,
        ),
        (
            &[(Icon::Circle.source(), 259.0)],
            face(1, button::SHOOT),
            299.0,
            85.0,
            85.0,
        ),
        (
            &[(Icon::Triangle.source(), 259.0)],
            face(2, button::JUMP),
            299.0,
            115.0,
            115.0,
        ),
        (
            &[(Icon::Square.source(), 259.0)],
            face(3, button::TRICK),
            299.0,
            145.0,
            145.0,
        ),
        (
            &[(Icon::R1.source(), 259.0), (Icon::R2.source(), 294.0)],
            (5, "Turbo"),
            329.0,
            168.0,
            170.0,
        ),
        (
            &[(Icon::L1.source(), 259.0), (Icon::L2.source(), 294.0)],
            (5, "Turbo"),
            329.0,
            193.0,
            195.0,
        ),
        (&[(R3, 265.0)], (strategy, "Strategy"), 299.0, 225.0, 225.0),
        (&[(ARROW, 259.0)], (24, "Pause"), 299.0, 258.0, 255.0),
    ];
    for (icons, (k, fallback), x, y, icon_y) in rows {
        for &(source, left) in icons {
            icon(
                out,
                look,
                source,
                at + Vec2::new(left + source.width() / 2.0, icon_y),
                source.height(),
            );
        }
        line(
            out,
            look,
            labels,
            images,
            look::fonts::SMALL,
            &words(k, fallback),
            look::colours::CREAM,
            at + Vec2::new(x, y),
            Align::Left,
        );
    }
}

/// The controller panel (`FUN_002144c8`, at (70, 105) on the pause
/// screen): its button rows from the widget's code ([`controller_rows`]);
/// the rest of its drawing is not traced, so those places are measured
/// from the original's capture: the band blue over the pause panel's
/// inside but two windows, a divider at x 319 and a band along the bottom
/// (y 388..425) holding Continue and Vibration; on the left the player,
/// the layout and the view (the lit one amber between arrows) over the
/// pad's picture.
fn controller(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    gm: &Gm,
    c: &Controller,
    ticks: u64,
) {
    for r in [
        Rect::new(319.0, 92.0, 322.0, 388.0),
        Rect::new(70.0, 388.0, 570.0, 425.0),
    ] {
        let mut p = Piece::new(gm.white.clone(), r);
        p.colour = Color::srgba_u8(BAND[0], BAND[1], BAND[2], BAND[3]);
        out.push(p);
    }
    let words = |k: usize, fallback: &str| {
        look.text("contconfig.txt", k)
            .unwrap_or(fallback)
            .to_owned()
    };
    let headers = [
        words(8, "Player %1N").replace("%1N", &(c.player + 1).to_string()),
        words(9 + usize::from(c.layout.min(2)), "Layout A"),
        words([12, 13][usize::from(c.view.min(1))], "Offense"),
    ];
    for (k, text) in headers.iter().enumerate() {
        let at = Vec2::new(190.0, 120.0 + 25.0 * k as f32);
        let lit = k == c.cursor;
        let colour = if lit {
            look::colours::ROW_LIT
        } else {
            look::colours::ROW
        };
        let w = line(
            out,
            look,
            labels,
            images,
            FONT,
            text,
            colour,
            at,
            Align::Centre,
        );
        if lit {
            pulse(out, ticks);
            arrows(out, look, at, w);
        }
    }
    if let Some([_, pad]) = look.pad.clone() {
        // Measured: the picture's pad spans x 83..307, y 194..357 in the
        // capture, the 256x256 image drawn 256x298 from (68, 136).
        out.push(Piece::new(pad, Rect::new(68.0, 136.0, 324.0, 434.0)));
    }
    controller_rows(out, look, labels, images, CONTROLLER_AT, c.layout, c.view);
    // The band: Continue (the Start arrow) and Vibration (Circle).
    let band = 413.0;
    icon(out, look, ARROW, Vec2::new(267.0, band), ARROW.height());
    line(
        out,
        look,
        labels,
        images,
        1,
        gm.text(0x0014),
        LEGEND_AMBER,
        Vec2::new(285.0, band),
        Align::Left,
    );
    let circle = crate::diagram::Icon::Circle.source();
    icon(out, look, circle, Vec2::new(436.0, band), circle.height());
    let w = line(
        out,
        look,
        labels,
        images,
        1,
        &words(37, "Vibration"),
        look::colours::PALE,
        Vec2::new(449.0, band),
        Align::Left,
    );
    let on = if c.vibration {
        words(38, "ON")
    } else {
        words(39, "OFF")
    };
    line(
        out,
        look,
        labels,
        images,
        1,
        &on,
        LEGEND_AMBER,
        Vec2::new(449.0 + w + 6.0, band),
        Align::Left,
    );
}

/// The song list (`FUN_00218e68`; its drawing is not traced, places
/// measured from the original's capture): Music Experience at y 138, then
/// four songs a page, 52 apart from y 178, each its title over its artist
/// (24 lower) with On or Off between them at x 505; the lit row amber, its
/// value between arrows; a down arrow at (88, 377) while more follow.
fn trax(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    v: &View,
) {
    const PAGE: usize = 4;
    let colour = |lit: bool| {
        if lit {
            look::colours::ROW_LIT
        } else {
            look::colours::ROW
        }
    };
    let t = v.trax;
    let lit = v.trax_cursor == 0;
    line(
        out,
        look,
        labels,
        images,
        FONT,
        &t.label,
        colour(lit),
        Vec2::new(104.0, 138.0),
        Align::Left,
    );
    if lit {
        pulse(out, v.ticks);
    }
    let at = Vec2::new(470.0, 138.0);
    let w = line(
        out,
        look,
        labels,
        images,
        FONT,
        &t.value,
        colour(lit),
        at,
        Align::Centre,
    );
    if lit {
        pulse(out, v.ticks);
        arrows(out, look, at, w);
    }
    let song = v.trax_cursor.saturating_sub(1);
    let first = song
        .saturating_sub(PAGE - 1)
        .min(t.songs.len().saturating_sub(PAGE));
    for (k, (title, artist, on)) in t.songs.iter().enumerate().skip(first).take(PAGE) {
        let y = 178.0 + 52.0 * (k - first) as f32;
        let lit = v.trax_cursor == k + 1;
        for (text, y) in [(title, y), (artist, y + 24.0)] {
            line(
                out,
                look,
                labels,
                images,
                FONT,
                text,
                colour(lit),
                Vec2::new(104.0, y),
                Align::Left,
            );
            if lit {
                pulse(out, v.ticks);
            }
        }
        let at = Vec2::new(505.0, y + 12.0);
        let word = if *on { "On" } else { "Off" };
        let w = line(
            out,
            look,
            labels,
            images,
            FONT,
            word,
            colour(lit),
            at,
            Align::Centre,
        );
        if lit {
            pulse(out, v.ticks);
            arrows(out, look, at, w);
        }
    }
    if let (true, Some(art)) = (first + PAGE < t.songs.len(), &look.prematch) {
        let min = Vec2::new(88.0, 377.0) - art.arrow_size / 2.0;
        out.push(Piece::new(
            art.arrow_down.clone(),
            Rect::from_corners(min, min + art.arrow_size),
        ));
    }
}

/// One UI node showing a pause piece.
#[derive(Component)]
pub struct PauseSlot(usize);

/// Points the pause screen's slots at this frame's pieces, above the
/// match, the HUD and the boards.
pub fn show(
    commands: &mut Commands,
    slots: &mut Query<(&PauseSlot, &mut ImageNode, &mut Node, &mut Visibility)>,
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
            GlobalZIndex(400 + k as i32),
            PauseSlot(k),
            DespawnOnExit(crate::Screen::Playing),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn school_disables_replays_trax_and_help_and_crowds_lock_the_camera() {
        let school = disabled(Context {
            mode: 1,
            users: 1,
            over: false,
        });
        assert!(school[row::REPLAYS] && school[row::TRAX] && school[row::HELP]);
        assert!(!school[row::CAMERA] && !school[row::QUIT]);
        let pickup = |users| {
            disabled(Context {
                mode: 2,
                users,
                over: false,
            })[row::CAMERA]
        };
        assert!(!pickup(2));
        assert!(pickup(3));
        let league = |users| {
            disabled(Context {
                mode: 4,
                users,
                over: false,
            })[row::CAMERA]
        };
        assert!(!league(1));
        assert!(league(2));
        assert!(
            disabled(Context {
                mode: 3,
                users: 1,
                over: true,
            })[row::QUIT]
        );
    }

    #[test]
    fn the_cursor_steps_over_disabled_rows_and_wraps() {
        let mut off = [false; row::COUNT];
        off[row::TRAX] = true;
        assert_eq!(next_row(row::LAYOUT, 1, &off), row::CAMERA);
        assert_eq!(next_row(row::CAMERA, -1, &off), row::LAYOUT);
        assert_eq!(next_row(row::LAYOUT, -1, &off), row::QUIT);
        assert_eq!(next_row(row::QUIT, 1, &off), row::LAYOUT);
    }

    #[test]
    fn replays_step_through_automatic_always_manual() {
        let mut o = Settings::default();
        let mut sound = AudioSettings::default();
        o.replays = 1;
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.push(value(row::REPLAYS, &o, &sound));
            step(row::REPLAYS, 1, &mut o, &mut sound);
        }
        assert_eq!(
            seen,
            [
                Value::Word(0x5817),
                Value::Word(0x5818),
                Value::Word(0x5819)
            ]
        );
        assert_eq!(o.replays, 1);
        step(row::REPLAYS, -1, &mut o, &mut sound);
        assert_eq!(o.replays, 2);
    }

    #[test]
    fn volumes_clamp_and_choices_name_their_words() {
        let mut o = Settings::default();
        let mut sound = AudioSettings {
            music: 10,
            ..Default::default()
        };
        step(row::MUSIC, 1, &mut o, &mut sound);
        assert_eq!(value(row::MUSIC, &o, &sound), Value::Slider(10));
        sound.speech = 0;
        step(row::SPEECH, -1, &mut o, &mut sound);
        assert_eq!(sound.speech, 0);
        o.camera = 5;
        assert_eq!(value(row::CAMERA, &o, &sound), Value::Word(0x5814));
        o.help = true;
        assert_eq!(value(row::HELP, &o, &sound), Value::Word(0x5815));
        assert_eq!(value(row::QUIT, &o, &sound), Value::None);
    }

    /// Two lines of words and three rows (text height 21 and line
    /// spacing 27 stand in for a font's): a wide box is 4:3 from its
    /// width, its middle 3 above the dialog's point, the words 20 over
    /// the rows, the rows 27 apart.
    #[test]
    fn the_dialog_box_grows_40_round_its_widest_line_then_to_four_by_three() {
        let (rect, text_top, tops) = dialog_box(260.0, 21.0 + 27.0, 21.0, 3);
        let [x, y, w, h] = rect;
        assert_eq!(w, 340.0);
        assert_eq!(h, 255.0);
        assert_eq!(x, 320.0 - 170.0);
        assert!((y + h / 2.0 - 237.0).abs() < 1e-3);
        let total = 48.0 + 20.0 + 3.0 * 27.0;
        assert_eq!(text_top, 240.0 - total / 2.0);
        assert_eq!(tops[0], text_top + 68.0);
        assert_eq!(tops[1] - tops[0], 27.0);
        // A tall dialog widens instead.
        let ([_, _, w, h], _, _) = dialog_box(10.0, 21.0, 21.0, 8);
        assert!((w - h * 4.0 / 3.0).abs() < 1e-3);
    }
}
