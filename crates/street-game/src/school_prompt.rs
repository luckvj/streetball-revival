//! Street School's lesson prompt and tutorial pages: what the lesson
//! manager posts ([`street_sim::school::Post`]) as the School's HUD and
//! front end show it.
//!
//! - *The prompt* (`FUN_002ba3b0`, the overlay listener's answer to
//!   `0x3c6970`): the move's name (element 0x20, its record's `+0x1c`, set
//!   on the first show of a lesson only, `+0x228`) at GUI (55, 320), the
//!   record's word (0x21, `+0x14`: "press", "press 2 turbos +" ...) at
//!   (50, 346) and its button's icon (0x24, `FUN_002b4288` of `+0x18`) 5
//!   right of the word; Kick Pass (0x11) "press", L3, "+" (0x23) and Pass
//!   (0x25). `FUN_002b4400` fills them at the launch; `FUN_002bab48` hides
//!   them (`0x3c6980`). While shown the HUD's update (`FUN_002b4c38`)
//!   takes 4 off their alpha an update as a signed byte, so they fade and
//!   come back every 64 updates.
//! - *The pages* (`FUN_002bb338` for `0x3c6990`): a front-end screen over
//!   the held game (`FUN_0033d380(0x19d2)`), built on `FUN_00250eb8`: the
//!   screen dimmed, the panel (`FUN_00211278(.., -1, 0x1c)`, 475 by 325
//!   about (320, 240)), the title (kind 0x23 at (108, 33), font 6), the
//!   text (font 1 in a box 400 wide about (325, 180)) and Continue (0x5709
//!   with the Cross at (310, 404)); then each page's picture: the player
//!   indicator (`FUN_00251538`), the HUD's left panel (`FUN_00251f50`
//!   Turbo, `FUN_00253258` Trick Points, `FUN_00255050` Gamebreaker), or
//!   the shot clock's count and violation (`FUN_00252e28`, its update
//!   `FUN_00253088`). Cross closes it.

use bevy::prelude::*;

use crate::boards::{Gm, Panels};
use crate::hud::{TextStyle, Writer};
use crate::look::{self, Labels, Look, Piece};
use crate::school_menu::RECORDS;
use street_sim::school::page;

/// The region word (`FUN_0030fa50()+0x1c`): 0, the US disc.
const REGION: u32 = 0;

/// `FUN_002b4288`: the icon frame (`XACOMMON` sheet 1, image 0) for a
/// record's button, with the user's controller configuration 0 (the
/// rewrite has none other) and region 0: Pass the Cross, Shoot the
/// Circle, Trick and Steal the Square, Block the Triangle, the pocket and
/// cancel Select's, Pick R3's, Give 'n Go the right stick's, the kick L3's.
pub fn button_frame(button: i8) -> Option<usize> {
    Some(match button {
        0 => 3,
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 0x12,
        5 if REGION < 1 => 0x12,
        5 => 0x13,
        6 => 0xf,
        7 => 0x11,
        8 => 0xe,
        9 => 0x10,
        _ => return None,
    })
}

/// The kick pass's record (`PTR_DAT_003fb42c`) and its second button
/// (`FUN_002b4288(.., 8)`: L3).
const KICK: usize = 0x11;
const KICK_BUTTON: i8 = 8;

/// Where the prompt's pieces go (GUI points; the HUD adds its runtime
/// shift).
const NAME_AT: Vec2 = Vec2::new(55.0, 320.0);
const WORD_AT: Vec2 = Vec2::new(50.0, 346.0);
const KICK_WORD_AT: Vec2 = Vec2::new(55.0, 346.0);
/// The gaps: word to icon 5, icon to "+" 5, "+" to the second icon 7.
const ICON_GAP: f32 = 5.0;
const PLUS_GAP: f32 = 7.0;
/// Measured on the first lesson's prompt (`lesson0-2.png`): the pieces
/// sit (1, -2) from the GUI points the runtime shift takes them to, and
/// the word's widget is 5 narrower than the rewrite's measure of it (the
/// last glyph's spacing).
const MEASURED: Vec2 = Vec2::new(1.0, -2.0);
const WIDTH_SLACK: f32 = 5.0;
/// Elements 0x20, 0x21, 0x23 (kind 4: font 2, 0xfff6f4d7, prop 7's
/// shadow, left and middle anchored).
const FONT: usize = 2;
const COLOUR: [u8; 4] = [0xf6, 0xf4, 0xd7, 0xff];
const ALIGN: u8 = 4;

/// The lesson prompt's widgets.
#[derive(Resource, Debug, Default, Clone)]
pub struct Prompt {
    /// Elements 0x20 and 0x21 shown.
    pub shown: bool,
    /// 0x20's text and whether it shows.
    pub name: u16,
    pub name_shown: bool,
    /// 0x21's text and 0x24's icon frame.
    pub word: u16,
    pub button: Option<usize>,
    /// The kick pass's "+" and second icon (0x23, 0x25).
    pub kick: Option<usize>,
    /// Their alpha (`+0x59`).
    pub alpha: u8,
    /// `+0x228`: the shows since the launch.
    pub shows: u32,
}

impl Prompt {
    /// `FUN_002b4400` at a lesson's launch: the texts and icon of its
    /// record, hidden, the count back to 0.
    pub fn launch(&mut self, lesson: usize) {
        let r = &RECORDS[lesson.min(RECORDS.len() - 1)];
        *self = Self {
            name: r.name,
            word: r.word,
            button: button_frame(r.button),
            ..Self::default()
        };
    }

    /// `FUN_002ba3b0(record)` for the lesson `lesson` the menu launched.
    pub fn post(&mut self, record: usize, lesson: usize) {
        let Some(r) = RECORDS.get(record) else {
            return;
        };
        self.shows += 1;
        if self.shows == 1 {
            self.name = r.name;
        }
        if record == KICK {
            self.word = RECORDS[KICK].word;
            self.button = button_frame(KICK_BUTTON);
            self.kick = button_frame(RECORDS[KICK].button);
            self.name_shown = true;
        } else {
            self.word = r.word;
            self.button = button_frame(r.button);
            self.kick = None;
            // Give 'n Go's second prompt drops the name in region 2 only.
            self.name_shown = !(lesson == 0x16 && self.shows > 1 && REGION == 2);
        }
        self.shown = true;
        self.alpha = 0xff;
    }

    /// `FUN_002bab48`.
    pub fn hide(&mut self) {
        self.shown = false;
    }

    /// `FUN_002b4c38`'s fade: `*(char *)(w + 0x59) - 4` stored back as a
    /// byte, each update the prompt shows.
    pub fn tick(&mut self) {
        if self.shown {
            self.alpha = (self.alpha as i8).wrapping_sub(4) as u8;
        }
    }

    /// The prompt's pieces, `shift` the HUD's runtime one.
    pub(crate) fn draw(
        &self,
        out: &mut Vec<Piece>,
        ink: &mut Writer,
        word: &dyn Fn(u16) -> String,
        shift: Vec2,
    ) {
        if !self.shown {
            return;
        }
        let a = f32::from(self.alpha) / 255.0;
        let style = TextStyle::shadowed(FONT, COLOUR);
        let shift = shift + MEASURED;
        if self.name_shown {
            ink.text(out, &word(self.name), style, NAME_AT + shift, ALIGN, a);
        }
        let at = if self.kick.is_some() {
            KICK_WORD_AT
        } else {
            WORD_AT
        } + shift;
        let text = word(self.word);
        ink.text(out, &text, style, at, ALIGN, a);
        let measure = |s: &str| {
            ink.look
                .fonts
                .get(FONT)
                .map_or(0.0, |f| f.measure(s) as f32 - WIDTH_SLACK)
        };
        let mut x = at.x + measure(&text) + ICON_GAP;
        let icons = ink.look.icons.clone();
        let icon = |out: &mut Vec<Piece>, frame: usize, x: f32| -> f32 {
            let Some((atlas, frames)) = icons.as_ref() else {
                return 0.0;
            };
            let Some(&rect) = frames.get(frame) else {
                return 0.0;
            };
            let min = Vec2::new(x, at.y - rect.height() / 2.0);
            let mut p = Piece::part(
                atlas.clone(),
                rect,
                Rect::from_corners(min, min + rect.size()),
            );
            p.colour = Color::srgba(1.0, 1.0, 1.0, a);
            out.push(p);
            rect.width()
        };
        if let Some(frame) = self.button {
            x += icon(out, frame, x) + ICON_GAP;
        }
        if let Some(second) = self.kick {
            ink.text(out, "+", style, Vec2::new(x, at.y), ALIGN, a);
            x += measure("+") + PLUS_GAP;
            icon(out, second, x);
        }
    }
}

/// The page's dim: the screen behind keeps 0.3 of its light (measured on
/// the five pages' grabs, 0.27..0.34 left and right of the panel).
const DIM: f32 = 0.70;
/// The panel (`FUN_00211278(.., -1, 0x1c)`: 475 by 325, centred on (320,
/// 240)), in [`crate::boards::panel_inked`]'s terms.
const PANEL: [f32; 4] = [82.5, 77.5, 475.0, 325.0];
const PANEL_INK: crate::boards::PanelInk = crate::boards::PanelInk {
    blue: 0x30398d,
    navy: 0x00002e,
    grey: 0x808e9c,
};
/// The title (kind 0x23: font 6, text 0xfff6f4d7, outlines 0xff010635 and
/// 0xff3b45a2) at (108, 33), Gamebreaker's at (103, 33).
const TITLE_FONT: usize = 6;
const TITLE_INNER: [u8; 4] = [0x01, 0x06, 0x35, 0xff];
const TITLE_OUTER: [u8; 4] = [0x3b, 0x45, 0xa2, 0xff];
/// The text: font 1, wrapped at 400, its box centred on (325, 180).
const BODY_FONT: usize = 1;
const BODY_WRAP: f32 = 400.0;
const BODY_AT: Vec2 = Vec2::new(323.0, 179.0);
const BODY_COLOUR: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
/// Continue (0x5709, the Cross; set at (310, 404) with the legend's own
/// anchors): measured on the five grabs, its pair centred on (313, 373.5)
/// in font 1 at 0.9, the icon 6 left of the words.
const CONTINUE: u16 = 0x5709;
const CONTINUE_AT: Vec2 = Vec2::new(313.0, 377.5);
const CONTINUE_SCALE: f32 = 0.9;
const CONTINUE_GAP: f32 = 6.0;
/// The HUD panel's place on the Turbo page and on Trick Points' and
/// Gamebreaker's (measured against the School's own HUD: (129, 230) and
/// (129, 221) from it).
const PANEL_SHIFT: [Vec2; 2] = [Vec2::new(132.0, 244.0), Vec2::new(132.0, 235.0)];
/// Trick Points' two figures (measured: the points under the panel in the
/// HUD's trick points font, the combo's bonus right of them in the pop-up
/// font at 0.86).
const POINTS: (&str, Vec2) = ("100,500", Vec2::new(226.0, 305.0));
const BONUS: (&str, Vec2) = ("1,000", Vec2::new(380.0, 329.5));
const BONUS_SCALE: f32 = 0.86;
/// The shot clock page's count (font 10, amber, at (312, 296)) and its
/// violation (`overlays.txt` 5, font 10, amber, its top at (320, 320)).
const CLOCK_FONT: usize = 10;
const AMBER: [u8; 4] = [0xfb, 0xac, 0x00, 0xff];
const COUNT_AT: Vec2 = Vec2::new(312.0, 296.0);
const VIOLATION: u16 = 0x5905;
const VIOLATION_AT: Vec2 = Vec2::new(320.0, 320.0);

/// The page's title and text ids (`FUN_00250eb8`'s arguments).
pub fn page_texts(p: u8) -> (u16, u16) {
    match p {
        page::PLAYER_INDICATOR => (0x5a1e, 0x5a44),
        page::SHOT_CLOCK => (0x5a1f, 0x5a45),
        page::TRICK_POINTS => (0x5a20, 0x5a46),
        page::GAMEBREAKER => (0x5a21, 0x5a47),
        _ => (0x5a1d, 0x5a43),
    }
}

/// The shot clock page's count (`FUN_00253088`: `+0xdc` the stage, `+0xe0`
/// the digit, `+0xd8` the updates in the stage).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Count {
    pub stage: u8,
    pub digit: u8,
    pub ticks: u32,
}

impl Default for Count {
    fn default() -> Self {
        Self {
            stage: 1,
            digit: 3,
            ticks: 0,
        }
    }
}

impl Count {
    /// One update: 3, 2 and 1 a second each, then the violation four
    /// seconds, then a second of nothing, and round again.
    pub fn step(&mut self) {
        let fps = street_sim::TICK_HZ;
        match self.stage {
            1 if self.ticks == fps => {
                self.ticks = 0;
                if self.digit >= 2 {
                    self.digit -= 1;
                } else {
                    self.stage = 2;
                }
            }
            2 if self.ticks > 4 * fps => {
                self.ticks = 0;
                self.stage = 0;
            }
            0 if self.ticks == fps => {
                self.ticks = 0;
                self.digit = 3;
                self.stage = 1;
            }
            _ => {}
        }
        // The base page's update (`FUN_002513d0`).
        self.ticks += 1;
    }
}

/// A tutorial page up over the School's game.
#[derive(Resource, Debug, Default, Clone)]
pub struct Page {
    pub open: Option<u8>,
    pub count: Count,
}

impl Page {
    pub fn open(&mut self, p: u8) {
        *self = Self {
            open: Some(p),
            count: Count::default(),
        };
    }
}

/// The resources a page's picture needs from the match.
pub struct PagePicture<'a> {
    pub atlas: Option<&'a Handle<Image>>,
    pub logo: Option<(Handle<Image>, Vec2)>,
    /// `onscr2.ssh`'s entries 0, 2 and 3.
    pub indicator: Option<&'a [(Handle<Image>, Vec2); 3]>,
}

/// The Player Indicator page's picture (`FUN_00251538`): its middle, the
/// ring's quarter, the disc and the dot.
const INDICATOR_AT: Vec2 = Vec2::new(303.8, 313.8);
/// Measured against `short-launch.png`: the picture sits (1, -2) from the
/// constructor's points.
const INDICATOR_SHIFT: Vec2 = Vec2::new(1.0, -2.0);
const INDICATOR_QUARTER: f32 = 33.8;
const INDICATOR_DISC: f32 = 50.0;
const INDICATOR_DOT: f32 = 20.3;

/// `FUN_00251538`'s pieces in their order: the dark disc (entry 0 at 50,
/// 0x9f303030), the star over it (entry 3, 0xc8616c8c), the dot (entry 0
/// at 20.3, red) and the ring's four quarters (entry 2, red, mirrored
/// about the middle).
fn indicator(out: &mut Vec<Piece>, art: &[(Handle<Image>, Vec2); 3]) {
    let argb = |c: u32| {
        Color::srgba(
            ((c >> 16) & 0xff) as f32 / 255.0,
            ((c >> 8) & 0xff) as f32 / 255.0,
            (c & 0xff) as f32 / 255.0,
            (c >> 24) as f32 / 255.0,
        )
    };
    let [(disc, _), (quarter, _), (star, _)] = art;
    for (image, side, colour) in [
        (disc, INDICATOR_DISC, 0x9f30_3030),
        (star, INDICATOR_DISC, 0xc861_6c8c),
        (disc, INDICATOR_DOT, 0xffff_0000),
    ] {
        let mut p = Piece::new(
            image.clone(),
            Rect::from_center_size(INDICATOR_AT + INDICATOR_SHIFT, Vec2::splat(side)),
        );
        p.colour = argb(colour);
        out.push(p);
    }
    let q = INDICATOR_QUARTER;
    for (k, (fx, fy)) in [(false, false), (true, false), (false, true), (true, true)]
        .into_iter()
        .enumerate()
    {
        let min = INDICATOR_AT + INDICATOR_SHIFT - Vec2::splat(q)
            + Vec2::new((k % 2) as f32, (k / 2) as f32) * q;
        let mut p = Piece::new(
            quarter.clone(),
            Rect::from_corners(min, min + Vec2::splat(q)),
        );
        p.flip = fx;
        p.flip_y = fy;
        p.colour = argb(0xffff_0000);
        out.push(p);
    }
}

/// `text` wrapped to `width` in `font`, its spaces kept (the texts put
/// two after a full stop).
fn wrap(look: &Look, font: usize, text: &str, width: f32) -> String {
    let Some(f) = look.fonts.get(font) else {
        return text.to_owned();
    };
    let mut out = Vec::new();
    for paragraph in text.replace("\\n", "\n").split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
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
    out.join("\n")
}

/// The open page's pieces, back to front.
#[allow(clippy::too_many_arguments)]
pub fn page_pieces(
    look: &Look,
    gm: &Gm,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    panels: &mut Panels,
    page_state: &Page,
    picture: PagePicture,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(p) = page_state.open else {
        return out;
    };
    let mut dim = Piece::new(gm.white.clone(), Rect::new(0.0, 0.0, 640.0, 480.0));
    dim.colour = Color::srgba(0.0, 0.0, 0.0, look::black_alpha(DIM));
    out.push(dim);
    out.push(panels.get_inked(PANEL, 0xff, PANEL_INK, gm, images));
    let (title, body) = page_texts(p);
    let title_at = if title == 0x5a21 {
        Vec2::new(103.0, 33.0)
    } else {
        Vec2::new(108.0, 33.0)
    };
    look::title(
        &mut out,
        labels,
        look,
        images,
        TITLE_FONT,
        gm.text(title),
        (TITLE_INNER, TITLE_OUTER),
        title_at,
        0,
    );
    let mut ink = Writer {
        labels,
        look,
        images,
    };
    let text = wrap(look, BODY_FONT, gm.text(body), BODY_WRAP);
    let height = ink.height(BODY_FONT, &text, 1.0);
    ink.text(
        &mut out,
        &text,
        TextStyle::shadowed(BODY_FONT, BODY_COLOUR),
        BODY_AT - Vec2::new(BODY_WRAP / 2.0, height / 2.0),
        0,
        1.0,
    );
    // The picture.
    match p {
        page::PLAYER_INDICATOR => {
            if let Some(art) = picture.indicator {
                indicator(&mut out, art);
            }
        }
        page::SHOT_CLOCK => {
            let c = page_state.count;
            if c.stage == 1 {
                ink.text(
                    &mut out,
                    &c.digit.to_string(),
                    TextStyle::plain(CLOCK_FONT, AMBER),
                    COUNT_AT,
                    1 | (1 << 2),
                    1.0,
                );
            } else if c.stage == 2 {
                ink.text(
                    &mut out,
                    gm.text(VIOLATION),
                    TextStyle::plain(CLOCK_FONT, AMBER),
                    VIOLATION_AT,
                    1,
                    1.0,
                );
            }
        }
        _ => {
            if let Some(atlas) = picture.atlas {
                let shift = PANEL_SHIFT[usize::from(p != page::TURBO)];
                let meter = match p {
                    page::GAMEBREAKER => 1.0,
                    page::TRICK_POINTS => 0.25,
                    _ => 0.0,
                };
                crate::hud::page_panel(
                    &mut out,
                    &mut ink,
                    atlas,
                    picture.logo.clone(),
                    shift,
                    meter,
                    "0",
                );
                if p == page::TRICK_POINTS {
                    use crate::hud::data::{colour, font};
                    for (f, scale, (s, at)) in [
                        (font::TRICK_POINTS, 1.0, POINTS),
                        (font::POPUP, BONUS_SCALE, BONUS),
                    ] {
                        ink.text(
                            &mut out,
                            s,
                            TextStyle {
                                scale,
                                ..TextStyle::shadowed(f, colour::AMBER)
                            },
                            at,
                            0,
                            1.0,
                        );
                    }
                }
            }
        }
    }
    // Continue: the legend kind 0x22 (`FUN_0021a530`), its pair the Cross.
    if let (Some((atlas, frames)), Some(f)) = (look.icons.as_ref(), look.fonts.get(BODY_FONT)) {
        let words = gm.text(CONTINUE);
        let icon = frames.get(3).copied().unwrap_or_default();
        let width = icon.width() + CONTINUE_GAP + f.measure(words) as f32 * CONTINUE_SCALE;
        let left = CONTINUE_AT.x - width / 2.0;
        let min = Vec2::new(left, CONTINUE_AT.y - icon.height() / 2.0);
        out.push(Piece::part(
            atlas.clone(),
            icon,
            Rect::from_corners(min, min + icon.size()),
        ));
        ink.text(
            &mut out,
            words,
            TextStyle {
                scale: CONTINUE_SCALE,
                ..TextStyle::plain(BODY_FONT, look::colours::LEGEND)
            },
            Vec2::new(left + icon.width() + CONTINUE_GAP, CONTINUE_AT.y),
            4,
            1.0,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_takes_its_records_words_and_fades_as_a_signed_byte() {
        let mut p = Prompt::default();
        p.launch(4);
        assert!(!p.shown);
        assert_eq!((p.name, p.word, p.button), (0x5a84, 0x5a6f, Some(0)));
        // Dunk to Lay-up's first prompt, then the lay-up's Shoot: the name
        // stays the lesson's.
        p.post(4, 4);
        assert!(p.shown && p.name_shown);
        p.post(1, 4);
        assert_eq!((p.name, p.word, p.button), (0x5a84, 0x5a6e, Some(0)));
        // The kick pass: press, L3, +, Pass.
        let mut k = Prompt::default();
        k.launch(0x11);
        k.post(0x11, 0x11);
        assert_eq!((k.button, k.kick), (Some(0xe), Some(3)));
        // 255, 251, ... 3, then 255 again: 64 updates round.
        let mut seen = Vec::new();
        for _ in 0..65 {
            p.tick();
            seen.push(p.alpha);
        }
        assert_eq!(&seen[..2], &[251, 247]);
        assert_eq!(seen[62], 3);
        assert_eq!(seen[63], 255);
        p.hide();
        p.tick();
        assert_eq!(p.alpha, 251);
    }

    #[test]
    fn the_buttons_follow_fun_002b4288_for_the_first_configuration() {
        let frames: Vec<_> = (0..10).map(button_frame).collect();
        assert_eq!(
            frames,
            [
                Some(3),
                Some(0),
                Some(1),
                Some(2),
                Some(0x12),
                Some(0x12),
                Some(0xf),
                Some(0x11),
                Some(0xe),
                Some(0x10)
            ]
        );
        assert_eq!(button_frame(-1), None);
    }

    #[test]
    fn the_shot_clock_page_counts_three_then_shows_the_violation() {
        let mut c = Count::default();
        let fps = street_sim::TICK_HZ;
        let mut stages = Vec::new();
        for _ in 0..(10 * fps) {
            c.step();
            stages.push((c.stage, c.digit));
        }
        assert_eq!(stages[0], (1, 3));
        assert_eq!(stages[fps as usize], (1, 2));
        assert_eq!(stages[2 * fps as usize], (1, 1));
        assert_eq!(stages[3 * fps as usize], (2, 1));
        // Four seconds of the violation, a second of nothing, 3 again.
        assert_eq!(stages[(7 * fps + 1) as usize], (0, 1));
        assert_eq!(stages[(8 * fps + 2) as usize], (1, 3));
    }
}
