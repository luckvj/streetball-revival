//! The loading screen (class 0x2c, layout 44; `FUN_0029c310` over the
//! loading base `FUN_0029aac8(.., 0x2c, 0x11)`).
//!
//! While the game loads, its picture ([`crate::loading`]) fills the screen
//! with, on the picture's star, the spinning ball (group 14 at (110, 334):
//! `XACOMMON` sheet 3's nineteen 50x50 images in turn, one each 52 ms,
//! `FUN_0029b228`) and the ring of twelve small stars round it
//! (`FUN_00238478`: radius 45 from the top clockwise, sheet 0 image 1 frame
//! 6, lit as frame 5 by `FUN_002386c0`: the first at once and one more for
//! every 2.2 s the load takes, `FUN_0029b228`, all twelve at its end),
//! and "Loading" (element 25: `load.txt` 0, font 5 at 0.9, 0xffd03f, a navy
//! 3.5-pixel outline, `FUN_0029ac18`). When the load is done
//! (`FUN_0029b438`) a game other than the attract shows Continue (elements 0
//! and 1: the Cross icon and `load.txt` 1) and waits for Cross
//! (`FUN_0029b120`). With Help on (`FUN_0029c578`, option bit 2) Cross goes
//! on to the controller widget's two pages over backdrop 0x22
//! (`FUN_002144c8` at (70, 90): Offense, then Defense), the game's mode
//! (element 11, `setchallenge.txt` 57..59, or "Game Mode : " and the My
//! Rules mode) and difficulty (15, "Difficulty Level : " and `options.txt`
//! 21..23, `FUN_0029cbc0`) under it, Continue at the bottom right; Cross on
//! Defense starts the game.
//!
//! The load back to the front end (class 0x2f, `FUN_0029d368` on the same
//! base and layout) draws the same pieces over its own picture
//! ([`crate::loading::Back`]): tips 4..7 with their one line, or the
//! Street Tips page. At the load's end it hides Continue and goes straight
//! on (`FUN_0029d538`): it takes no press.

use bevy::prelude::*;

use crate::look::{self, Look, Piece};

/// Where the attract's load puts its "Loading Demo" (`FUN_0029c490`).
const DEMO_LINE: Vec2 = Vec2::new(292.0, 375.0);

/// The controller pages' backdrop (`FUN_0029b650(.., 0x22, 0)`).
pub const CONTROLS_BACKDROP: usize = 0x22;
/// Street School's welcome page's (`FUN_0029ca10`: `FUN_0029b650(.., 0x1e,
/// 0)`, Stretch Monroe's picture).
pub const WELCOME_BACKDROP: usize = 0x1e;

/// Whether a loaded game waits for Cross (a game from the menus does; a
/// quick match does not).
#[derive(Resource, Default)]
pub struct Waits(pub bool);

/// Where the screen stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    /// The game loading.
    #[default]
    Loading,
    /// Loaded: Continue waits for Cross.
    Continue,
    /// The controller widget: 0 Offense, 1 Defense.
    Controls(u8),
    /// Street School's welcome page (`FUN_0029ca10`, mode 1, last).
    Welcome,
}

/// The screen's state.
#[derive(Resource, Default)]
pub struct LoadingScreen {
    pub picture: Option<Handle<Image>>,
    /// Pick Up's street tip the picture is (`FUN_0029b828`: 0..3 going
    /// into a game, 4..7 on the way back), its words drawn over it.
    pub tip: Option<u8>,
    /// The Street Tips page of an owned tip card (the load back,
    /// `FUN_0029b720`): the picture is backdrop 0x1a.
    pub street_tip: Option<u8>,
    pub stage: Stage,
    /// When the load started (real seconds).
    pub since: f64,
    /// The game's mode line and the difficulty line for the controller
    /// pages, and the controller layout shown.
    pub mode_line: String,
    pub difficulty_line: String,
    pub layout: u8,
    /// A Street School game (the instructor's line, the welcome page).
    pub school: bool,
    /// The attract's game (mode 0): "Loading Demo" for "Loading".
    pub demo: bool,
}

/// The spinner's image after `ms` milliseconds (`FUN_0029b228`: the next
/// of nineteen every 52).
pub fn spinner_frame(ms: f64) -> usize {
    ((ms / 52.0).max(0.0) as usize) % SPINNER_FRAMES
}

/// The ring's lit stars after `ms` milliseconds of the load: the first at
/// once (`FUN_0029b228` counts one up whenever its next time, 0 as made,
/// has passed, then waits 2.2 s), twelve at most.
pub fn lit_stars(ms: f64) -> usize {
    ((ms / 2200.0).max(0.0) as usize + 1).min(RING)
}

const SPINNER_FRAMES: usize = 19;
const RING: usize = 12;
/// The spinner group's point (element 14) and the ring's offset from it
/// (`FUN_00238478`: 17.5, 16) and radius.
const SPINNER_AT: Vec2 = Vec2::new(110.0, 334.0);
const RING_AT: Vec2 = Vec2::new(17.5, 16.0);
const RING_RADIUS: f32 = 45.0;
/// The texts' colour and "Loading"'s outline (`FUN_0029ac18`).
const YELLOW: [u8; 4] = [0xff, 0xd0, 0x3f, 0xff];
const NAVY: [u8; 4] = [0x01, 0x09, 0x54, 0xff];
/// Stretch Monroe's name's outline (`FUN_0029ca10`: `+0x4a8` 0xff040735).
const NAVY_WELCOME: [u8; 4] = [0x04, 0x07, 0x35, 0xff];
const CREAM: [u8; 4] = [0xdc, 0xdc, 0xbb, 0xff];
/// The controller widget's place on the loading pages (`vt+0x11c(70, 90)`)
/// and its pad picture, 15 higher than the pause screen's.
const WIDGET_AT: Vec2 = Vec2::new(70.0, 90.0);
const PAD: Rect = Rect {
    min: Vec2::new(68.0, 121.0),
    max: Vec2::new(324.0, 419.0),
};
/// The page's name and its stars (measured on `loading_controller.png`,
/// the row widget's stars).
const PAGE_NAME_AT: Vec2 = Vec2::new(189.0, 136.0);
/// A street tip's page (`FUN_0029b828`): its name (`whatsnew.txt` by the
/// table at `0x3fa4f0`) in element 2, moved to (321, 63); the lines'
/// elements 28, 29 and 30 at (335, 385), (335, 410) and (359, 410) in
/// font 3 at 1 (0xffe1e2c0, the layout's); the button combinations
/// (`FUN_00212880`, `DAT_003fa4e8` L1 + R1 and `DAT_003fa4ec` the stick
/// toward the hoop) by their left 20 or 40 above element 29, their words 5
/// past them. Tips 4..7 (the load back's) have one line in element 28 in
/// font 2.
const TIP_NAME: [usize; 8] = [5, 8, 9, 10, 6, 7, 12, 11];
const TIP_NAME_AT: Vec2 = Vec2::new(321.0, 63.0);
const TIP_SHADOW: [u8; 4] = [0x02, 0x06, 0x35, 0xff];
/// The name's navy edge (kind 35's shadow, both colours navy here): an
/// outline and a drop this far (measured on `loading_tip_b2p.png`).
const TIP_NAME_EDGE: f32 = 2.5;
const TIP_LINE_AT: Vec2 = Vec2::new(335.0, 385.0);
const TIP_COMBO_AT: Vec2 = Vec2::new(335.0, 410.0);
const TIP_WORDS: [u8; 4] = [0xe1, 0xe2, 0xc0, 0xff];
/// The Street Tips page's title (element 2 at `FUN_0029b720`'s (321, 73))
/// and its box (element 27, kind 11) when the layout lacks them.
const STREET_TIPS_TITLE_AT: Vec2 = Vec2::new(321.0, 73.0);
const STREET_TIPS_BOX_AT: Vec2 = Vec2::new(318.0, 198.0);
const STREET_TIPS_BOX_WIDTH: f32 = 505.0;
const STREET_TIPS_GOLD: [u8; 4] = [0xfd, 0xb8, 0x13, 0xff];
const TURBOS: u32 = 0x14000;
const TOWARD: u32 = 0x800;
const PAGE_STAR_OUT: f32 = 15.0;

/// The sprites the screen draws: the spinner's nineteen images and the
/// ring's star unlit and lit.
pub struct LoadingArt {
    pub spinner: Vec<(Handle<Image>, Vec2)>,
    pub star: (Handle<Image>, Vec2),
    pub star_lit: (Handle<Image>, Vec2),
}

/// The screen's pieces for its stage.
#[allow(clippy::too_many_arguments)]
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: Option<&LoadingArt>,
    backdrop: Option<Handle<Image>>,
    s: &LoadingScreen,
    ms: f64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let layout = look.layouts.iter().find(|l| l.class == 0x2c);
    let spot = |id: u32| {
        layout?
            .elements
            .iter()
            .find(|e| e.id == id && e.kind != 0)
            .and_then(|e| e.position())
            .map(|(x, y)| Vec2::new(f32::from(x), f32::from(y)))
    };
    let load_text = |k: usize, or: &str| look.text("load.txt", k).unwrap_or(or).to_owned();
    let cross = |out: &mut Vec<Piece>, at: Vec2| {
        if let Some((atlas, frame)) = look.button_icon(0) {
            out.push(Piece::part(
                atlas,
                frame,
                Rect::from_center_size(at, frame.size()),
            ));
        }
    };
    // Continue at the bottom right of the controller and welcome pages
    // (`FUN_0029cbc0`: the icon at (490, 431), the words at (503, 420) in
    // font 1).
    let corner = |out: &mut Vec<Piece>, labels: &mut look::Labels, images: &mut Assets<Image>| {
        cross(out, Vec2::new(490.0, 431.0));
        crate::choose::place(
            out,
            labels,
            look,
            images,
            1,
            &load_text(1, "Continue"),
            CREAM,
            Vec2::new(503.0, 420.0),
            0,
        );
    };
    if s.stage == Stage::Welcome {
        // `FUN_0029ca10`: "Welcome" (21, kind 35 in its shadow colours),
        // "Street School / Featuring" (23) and "Stretch / Monroe" (20, font
        // 6 in 0xffd03f with a navy 3.5-pixel outline) over backdrop 0x1e.
        if let Some(b) = backdrop {
            out.push(Piece::new(b, Rect::new(0.0, 0.0, 640.0, 480.0)));
        }
        let element = |id: u32| layout?.elements.iter().find(|e| e.id == id && e.kind != 0);
        if let (Some(e), Some(at)) = (element(21), spot(21)) {
            let font = e.props.get(&13).map_or(6, |&f| f as usize);
            let shadow = (
                e.props
                    .get(&23)
                    .map_or(look::colours::COOL.0, |&c| street_assets::font::argb(c)),
                e.props
                    .get(&24)
                    .map_or(look::colours::COOL.1, |&c| street_assets::font::argb(c)),
            );
            if let Some(f) = look.fonts.get(font) {
                let top = at.y - f.text_height as f32 / 2.0;
                if let Some(p) = labels.text(
                    look,
                    images,
                    font,
                    &load_text(9, "Welcome"),
                    e.colour().unwrap_or(look::colours::PALE),
                    Some(shadow),
                    Vec2::new(at.x, top),
                    look::Align::Centre,
                ) {
                    out.push(p);
                }
            }
        }
        let mut w = crate::hud::Writer {
            labels,
            look,
            images,
        };
        if let (Some(e), Some(at)) = (element(23), spot(23)) {
            let font = e.props.get(&13).map_or(3, |&f| f as usize);
            let words = load_text(10, "Street School\\n Featuring").replace("\\n", "\n");
            let style =
                crate::hud::TextStyle::plain(font, e.colour().unwrap_or(look::colours::PALE));
            w.text(&mut out, &words, style, at, e.align, 1.0);
        }
        if let (Some(e), Some(at)) = (element(20), spot(20)) {
            let font = e.props.get(&13).map_or(6, |&f| f as usize);
            let words = load_text(7, "Stretch\\nMonroe").replace("\\n", "\n");
            let style = crate::hud::TextStyle {
                outline: Some((NAVY_WELCOME, Vec2::splat(3.5))),
                ..crate::hud::TextStyle::plain(font, YELLOW)
            };
            w.text(&mut out, &words, style, at, e.align, 1.0);
        }
        corner(&mut out, labels, images);
        return out;
    }
    if let Stage::Controls(view) = s.stage {
        if let Some(b) = backdrop {
            out.push(Piece::new(b, Rect::new(0.0, 0.0, 640.0, 480.0)));
        }
        // The page's name between its stars.
        let name = look
            .text("contconfig.txt", [12, 13][usize::from(view.min(1))])
            .unwrap_or("Offense")
            .to_owned();
        crate::choose::place(
            &mut out,
            labels,
            look,
            images,
            2,
            &name,
            look::colours::ROW_LIT,
            PAGE_NAME_AT,
            1 | 4,
        );
        if let Some(f) = look.fonts.get(2) {
            let star = look.row_star();
            let half = f.measure(&name) as f32 / 2.0;
            for x in [
                PAGE_NAME_AT.x - half - PAGE_STAR_OUT,
                PAGE_NAME_AT.x + half + PAGE_STAR_OUT,
            ] {
                out.push(Piece::new(
                    star.0.clone(),
                    Rect::from_center_size(Vec2::new(x, PAGE_NAME_AT.y), star.1),
                ));
            }
        }
        if let Some([_, pad]) = look.pad.clone() {
            out.push(Piece::new(pad, PAD));
        }
        crate::pause::controller_rows(&mut out, look, labels, images, WIDGET_AT, s.layout, view);
        for (id, words, font) in [(11, &s.mode_line, 3), (15, &s.difficulty_line, 2)] {
            if let Some(at) = spot(id) {
                crate::choose::place(&mut out, labels, look, images, font, words, YELLOW, at, 4);
            }
        }
        corner(&mut out, labels, images);
        return out;
    }
    if let Some(picture) = &s.picture {
        out.push(Piece::new(
            picture.clone(),
            Rect::new(0.0, 0.0, 640.0, 480.0),
        ));
    }
    if let Some(art) = art {
        if let Some((image, size)) = art.spinner.get(spinner_frame(ms)) {
            out.push(Piece::new(
                image.clone(),
                Rect::from_corners(SPINNER_AT, SPINNER_AT + *size),
            ));
        }
        // The load's end lights the whole ring (`FUN_0029b520(.., 0xc)`).
        let lit = if s.stage == Stage::Continue {
            RING
        } else {
            lit_stars(ms)
        };
        for k in 0..RING {
            let angle = k as f32 * std::f32::consts::TAU / RING as f32;
            let at = SPINNER_AT
                + RING_AT
                + Vec2::new(RING_RADIUS * angle.sin(), -RING_RADIUS * angle.cos());
            let (image, size) = if k < lit { &art.star_lit } else { &art.star };
            out.push(Piece::new(
                image.clone(),
                Rect::from_corners(at, at + *size),
            ));
        }
    }
    if let Some(card) = s.street_tip {
        street_tip_pieces(&mut out, look, labels, images, card);
    } else if let Some(tip) = s.tip {
        tip_pieces(&mut out, look, labels, images, tip);
    }
    if let Some(at) = spot(25) {
        let mut w = crate::hud::Writer {
            labels,
            look,
            images,
        };
        let style = crate::hud::TextStyle {
            scale: 0.9,
            outline: Some((NAVY, Vec2::splat(3.5))),
            ..crate::hud::TextStyle::plain(5, YELLOW)
        };
        // The attract's load says "Loading Demo" (`load.txt` 6), the
        // element moved to (292, 375) (`FUN_0029c490` in mode 0).
        let (line, at) = if s.demo {
            (load_text(6, "Loading Demo"), DEMO_LINE)
        } else {
            (load_text(0, "Loading"), at)
        };
        w.text(&mut out, &line, style, at, 1 | 4, 1.0);
    }
    if s.stage == Stage::Continue {
        if let Some(at) = spot(0) {
            cross(&mut out, at);
        }
        if let Some(at) = spot(1) {
            crate::choose::place(
                &mut out,
                labels,
                look,
                images,
                2,
                &load_text(1, "Continue"),
                [0xff, 0xff, 0xff, 0xff],
                at,
                0,
            );
        }
    }
    out
}

/// A street tip's words over its picture (`FUN_0029b828`): the name in
/// element 2's font 6, cream with its navy shadow; Back 2 Papa's "Move
/// towards hoop +" at 0.8, 20 up, over L1 + R1 "+ Pass"; Kick Pass's
/// stick "+ Pass", Off The Heezay's L1 + R1 "+ Pass" and Turbo Block's L1 +
/// R1 "+ Block", each 40 up. (The French lines differ.)
fn tip_pieces(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    tip: u8,
) {
    let words = |k: usize| look.text("whatsnew.txt", k).unwrap_or("").to_owned();
    let tip = usize::from(tip.min(7));
    let mut w = crate::hud::Writer {
        labels,
        look,
        images,
    };
    let style = crate::hud::TextStyle {
        shadow: Some((TIP_SHADOW, Vec2::splat(TIP_NAME_EDGE), 1.0)),
        outline: Some((TIP_SHADOW, Vec2::splat(TIP_NAME_EDGE))),
        ..crate::hud::TextStyle::plain(6, look::colours::PALE)
    };
    w.text(out, &words(TIP_NAME[tip]), style, TIP_NAME_AT, 1 | 4, 1.0);
    if tip >= 4 {
        // Element 28 alone in font 2 (`FUN_0029b828` cases 4..7): "Check Out
        // The "Give `n Go" Lesson" (15) for 4, "Check It Out" (13) for 5
        // and 7, the Advanced Dunk lesson's (14) for 6, 10 further left.
        let (k, left) = match tip {
            4 => (0x0f, 0.0),
            6 => (0x0e, 10.0),
            _ => (0x0d, 0.0),
        };
        let line = crate::hud::TextStyle::plain(2, TIP_WORDS);
        let at = TIP_LINE_AT - Vec2::new(left, 0.0);
        w.text(out, &words(k).replace("\\n", "\n"), line, at, 4, 1.0);
        return;
    }
    let (up, combo, end) = match tip {
        0 => (20.0, TURBOS, 0x11),
        1 => (40.0, TOWARD, 0x11),
        2 => (40.0, TURBOS, 0x11),
        _ => (40.0, TURBOS, 0x16),
    };
    // Back 2 Papa's two lines are at 0.8.
    let scale = if tip == 0 { 0.8 } else { 1.0 };
    let line = crate::hud::TextStyle {
        scale,
        ..crate::hud::TextStyle::plain(3, TIP_WORDS)
    };
    if tip == 0 {
        let at = TIP_LINE_AT - Vec2::new(0.0, up);
        w.text(out, &words(0x12), line, at, 4, 1.0);
    }
    let at = TIP_COMBO_AT - Vec2::new(0.0, up);
    let width = crate::edit_moves::combo_width(look, combo);
    crate::edit_moves::combo(
        out,
        look,
        w.labels,
        w.images,
        combo,
        at + Vec2::new(width, 0.0),
        1.0,
    );
    w.text(
        out,
        &words(end),
        line,
        at + Vec2::new(width + 5.0, 0.0),
        4,
        1.0,
    );
}

/// The Street Tips page of an owned tip card (`FUN_0029b720`) over
/// backdrop 0x1a: "Street Tips" (element 2, `load.txt` 2, as a tip's name)
/// and the card's tip (element 27, the kind-11 box: `streetTips.txt` card +
/// 1 in its font 3 and colour, broken to its width, each line centred on
/// the box's point and the lines on its middle).
fn street_tip_pieces(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    card: u8,
) {
    let layout = look.layouts.iter().find(|l| l.class == 0x2c);
    let element = |id: u32| layout?.elements.iter().find(|e| e.id == id && e.kind != 0);
    let mut w = crate::hud::Writer {
        labels,
        look,
        images,
    };
    let title = look.text("load.txt", 2).unwrap_or("Street Tips").to_owned();
    let style = crate::hud::TextStyle {
        shadow: Some((TIP_SHADOW, Vec2::splat(TIP_NAME_EDGE), 1.0)),
        outline: Some((TIP_SHADOW, Vec2::splat(TIP_NAME_EDGE))),
        ..crate::hud::TextStyle::plain(6, look::colours::PALE)
    };
    w.text(out, &title, style, STREET_TIPS_TITLE_AT, 1 | 4, 1.0);
    let e = element(27);
    let font = e.and_then(|e| e.props.get(&13)).map_or(3, |&f| f as usize);
    let width = e
        .and_then(|e| e.size())
        .map_or(STREET_TIPS_BOX_WIDTH, |(w, _)| f32::from(w));
    let at = e
        .and_then(|e| e.position())
        .map_or(STREET_TIPS_BOX_AT, |(x, y)| {
            Vec2::new(f32::from(x), f32::from(y))
        });
    let colour = e
        .and_then(|e| e.props.get(&40))
        .map_or(STREET_TIPS_GOLD, |&c| street_assets::font::argb(c));
    let (Some(f), Some(tip)) = (
        look.fonts.get(font),
        look.text("streettips.txt", usize::from(card) + 1),
    ) else {
        return;
    };
    let lines = crate::event_intro::wrap(f, tip.trim_end(), width).join("\n");
    w.text(
        out,
        &lines,
        crate::hud::TextStyle::plain(font, colour),
        at,
        1 | 4,
        1.0,
    );
}

/// The controller pages' mode line (`FUN_0029cbc0`): the School's, the
/// League Challenge's, Be A Legend's, a Pick Up game's ("Game Mode : " and the
/// My Rules mode when it is not Pick Up's own).
pub fn mode_line(
    look: &Look,
    running: Option<crate::story::Run>,
    rules: street_sim::Mode,
) -> String {
    let set = |k: usize, or: &str| look.text("setchallenge.txt", k).unwrap_or(or).to_owned();
    match running {
        Some(crate::story::Run::School(_)) => set(55, "Game Mode : Street School"),
        Some(crate::story::Run::League(..)) => set(58, "Game Mode : League Challenge"),
        Some(crate::story::Run::StoryEvent(_)) => set(59, "Game Mode : Be A Legend"),
        None if rules != street_sim::Mode::Normal => {
            let k = street_sim::Mode::ALL
                .iter()
                .position(|&m| m == rules)
                .unwrap_or(0);
            format!("{} : {}", set(1, "Game Mode"), set(6 + k, "Pick Up"))
        }
        None => set(57, "Game Mode : Pick Up"),
    }
}

/// The difficulty line ("Difficulty Level : " and `options.txt` 21..23 by
/// the difficulty, `0x4890c0`).
pub fn difficulty_line(look: &Look, d: street_sim::Difficulty) -> String {
    let k = match d {
        street_sim::Difficulty::Easy => 21,
        street_sim::Difficulty::Medium => 22,
        street_sim::Difficulty::Hard => 23,
    };
    format!(
        "{} : {}",
        look.text("load.txt", 11).unwrap_or("Difficulty Level"),
        look.text("options.txt", k).unwrap_or("Got Game (Basic)")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spinner_turns_and_the_ring_lights_with_the_load() {
        assert_eq!(spinner_frame(0.0), 0);
        assert_eq!(spinner_frame(51.9), 0);
        assert_eq!(spinner_frame(52.0), 1);
        assert_eq!(spinner_frame(52.0 * 19.0), 0);
        // The first star at once (the runtime's grabs a second into the
        // load show one lit, `loading_legend.png` two).
        assert_eq!(lit_stars(0.0), 1);
        assert_eq!(lit_stars(2199.0), 1);
        assert_eq!(lit_stars(2200.0), 2);
        assert_eq!(lit_stars(4400.0), 3);
        assert_eq!(lit_stars(1e9), 12);
    }
}
