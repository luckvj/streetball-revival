//! The match HUD as the original's overlay screen draws it: class 2 of
//! `/FE/XLAY_GM.BIN` (blob 1), built by `FUN_002ad190` (vtable `0x48d230`),
//! drawn by `FUN_002b12a0` and updated by `FUN_002b3048` once an update;
//! the Overlays task's listener (`FUN_002e23f0`, vtable `0x495b10`) calls
//! it on the game's events. The trace is in `docs/TRACE.md`, "The match
//! HUD, its pop-ups and messages (2026-10-16)".
//!
//! - **Top**: each side a panel (`0x1a`/`0x1d`), its logo in a frame
//!   (`0x1c`/`0x1f`; the team's `XATEAMS` image 2 on the left, 3 on the
//!   right), the gamebreaker meter (target `0x38`/`0x39`, fill
//!   `0x34`/`0x35`, pocketed `0x36`/`0x37`, full `0x3a`/`0x3b`) under its
//!   GAMEBREAKER word (`0x16`/`0x17`, lit `0x18`/`0x19`), the score in its
//!   frame (`0x1b`/`0x1e`, font 11) and the trick points below (font 1).
//!   Side 1 is drawn on the left, side 0 (the home side) on the right.
//! - **Turbo**: under each panel each person's TURBO word and bar in the
//!   pad's colour in a slanted frame; a side with no person shows one in a
//!   free pad's colour.
//! - **Pop-ups** (`FUN_002b87c0`, `FUN_002b4c38`): a word in font 10 flies
//!   in from its side at three times its size to the top middle, holds,
//!   then shrinks into its side's corner, its points a line below.
//! - **Messages**: the combo line and "Press START to exit demo" (bottom
//!   middle), the game-point line, the shot clock's last eight seconds and
//!   its violation, the court's name during the first intro script, the
//!   Opposition and Track Info panels, the replay prompt.
//! - **The banner** ([`crate::banner`], widget `0x41`): a replay's or a
//!   gamebreaker's bands and sprites over the hidden HUD.
//!
//! Every element is moved up 16 in a match (`FUN_002ad190` after
//! `FUN_002bb660`, not in Street School): the match's GUI space is the
//! frame's own rows, where the front end's layouts sit 16 lower (the frame
//! shows layout rows 16..464). In the rewrite's 640x480 layout space the
//! elements therefore stand at their layout points, and the points the
//! code sets at run time (the pop-ups' paths, the court's name, the
//! panel's corner, the banner) 16 lower than written ([`GUI_TO_LAYOUT`]),
//! as the original's frames grabbed into layout space show.

use bevy::prelude::*;
use street_sim::{
    Basket, Event, Goal, PassStyle, Phase, player::Action, street, timing as timing_tables,
};

use crate::{
    Screen,
    art::Art,
    boards::Gm,
    controls::{Control, MAX_PEOPLE},
    game::{Sim, TickEvents},
    look::{self, Align, Labels, Look, Piece},
    setup::{Disc, MatchSetup},
};

/// How long the court's name shows at the start, in ticks, when the match
/// has no intro to time it (and the audio's start, which waits on it).
pub const INTRO_TICKS: u64 = 4 * 60;

/// Updates a second (`FUN_0030fa50() + 0x24`, 60 on the NTSC disc).
const FPS: i32 = 60;

/// The in-game layouts' shift in a match (`FUN_002ad190`: every element's
/// y less 16 unless the game is Street School, game `+0x9c` = 4).
pub const MATCH_SHIFT: f32 = -16.0;

/// A point of the match's GUI (the frame's rows) in the rewrite's layout
/// space.
pub const GUI_TO_LAYOUT: f32 = 16.0;

/// Where a layout element's point lands (its shift into the GUI, then the
/// GUI into layout space), and where a run-time GUI point does.
const ELEMENT: Vec2 = Vec2::new(0.0, MATCH_SHIFT + GUI_TO_LAYOUT);
const RUNTIME: Vec2 = Vec2::new(0.0, GUI_TO_LAYOUT);
/// The traced constants.
pub mod data {
    use bevy::prelude::*;

    /// An atlas part as `FUN_002bc0b8` sets it: the element's size and the
    /// texture's corners, written `n / 255` of the texture (`n` here).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Part {
        pub size: Vec2,
        pub uv0: Vec2,
        pub uv1: Vec2,
    }

    pub const fn part(w: f32, h: f32, u0: f32, v0: f32, u1: f32, v1: f32) -> Part {
        Part {
            size: Vec2::new(w, h),
            uv0: Vec2::new(u0, v0),
            uv1: Vec2::new(u1, v1),
        }
    }

    /// `HUD.SSH`'s parts (the constructor's `FUN_002bc0b8` calls).
    pub const PANEL: Part = part(174.0, 34.0, 6.0, 127.0, 180.0, 161.0);
    pub const SCORE_FRAME: Part = part(47.0, 34.0, 59.0, 57.0, 107.0, 91.0);
    pub const LOGO_FRAME: Part = part(45.0, 44.0, 5.0, 55.0, 52.0, 99.0);
    /// The meter: the brown target, the orange fill, the pocketed side's
    /// yellow fill (`FUN_002b8168`), the blue pocketed bar and the cream
    /// full bar; 167 wide when full.
    pub const METER_TARGET: Part = part(167.0, 12.0, 8.0, 241.0, 176.0, 251.0);
    pub const METER_FILL: Part = part(167.0, 12.0, 8.0, 198.0, 176.0, 209.0);
    pub const METER_POCKET_FILL: Part = part(167.0, 12.0, 8.0, 213.0, 176.0, 224.0);
    pub const METER_POCKETED: Part = part(167.0, 12.0, 8.0, 227.0, 176.0, 238.0);
    pub const METER_FULL: Part = part(167.0, 12.0, 8.0, 183.0, 176.0, 194.0);
    /// The meter's full width; its texture runs `0.65882355` (168 / 255) per
    /// 167 pixels (`FUN_002b6840`), the parts' own span.
    pub const METER_WIDTH: f32 = 167.0;
    pub const GAMEBREAKER: Part = part(113.0, 17.0, 5.0, 21.0, 118.0, 38.0);
    pub const GAMEBREAKER_LIT: Part = part(113.0, 17.0, 5.0, 0.0, 118.0, 17.0);
    pub const SLASH: Part = part(20.0, 13.0, 228.0, 10.0, 246.0, 24.0);
    pub const TURBO_FRAME: Part = part(92.0, 16.0, 7.0, 163.0, 99.0, 180.0);
    /// 1P..4P's bars and TURBO words: red, yellow, blue, green
    /// (`0x2c..0x2f`, `0x30..0x33`, by the pad, `FUN_00156d98`).
    pub const TURBO_BAR: [Part; 4] = [
        part(84.0, 12.0, 114.0, 112.0, 198.0, 124.0),
        part(84.0, 12.0, 114.0, 74.0, 197.0, 87.0),
        part(84.0, 12.0, 114.0, 58.0, 198.0, 70.0),
        part(84.0, 12.0, 103.0, 164.0, 187.0, 176.0),
    ];
    pub const TURBO_WORD: [Part; 4] = [
        part(84.0, 10.0, 124.0, 38.0, 208.0, 49.0),
        part(84.0, 10.0, 124.0, 93.0, 208.0, 104.0),
        part(84.0, 10.0, 124.0, 7.0, 208.0, 18.0),
        part(84.0, 10.0, 124.0, 23.0, 208.0, 34.0),
    ];
    /// A turbo bar's and word's width at a full turbo; the texture follows
    /// at `0.32941177` (84 / 255) per full width (`FUN_002b7498`).
    pub const TURBO_WIDTH: f32 = 84.0;
    /// The replay prompt's picture (`0x3c`).
    pub const REWIND: Part = part(43.0, 37.0, 194.0, 194.0, 238.0, 232.0);

    /// Layout points (`XLAY_GM` blob 1, before the match shift) and
    /// alignments (bits 0..1 across: 0 left, 1 centre, 2 right; bits 2..3
    /// down: 0 top, 1 middle, 2 bottom), by screen side.
    pub mod at {
        use bevy::prelude::Vec2;

        pub const PANEL: [(Vec2, u8); 2] =
            [(Vec2::new(95.0, 38.0), 0), (Vec2::new(545.0, 38.0), 2)];
        pub const SCORE_FRAME: [(Vec2, u8); 2] =
            [(Vec2::new(267.0, 37.0), 0), (Vec2::new(373.0, 37.0), 2)];
        pub const LOGO_FRAME: [(Vec2, u8); 2] =
            [(Vec2::new(53.0, 38.0), 0), (Vec2::new(587.0, 38.0), 2)];
        /// The logos, moved there by the constructor (`0x14`, `0x15`).
        pub const LOGO: [(Vec2, u8); 2] = [(Vec2::new(55.0, 41.0), 0), (Vec2::new(545.0, 41.0), 0)];
        pub const METER: [(Vec2, u8); 2] =
            [(Vec2::new(96.0, 42.0), 0), (Vec2::new(544.0, 42.0), 2)];
        pub const GAMEBREAKER: [(Vec2, u8); 2] =
            [(Vec2::new(178.0, 40.0), 5), (Vec2::new(462.0, 40.0), 5)];
        pub const SLASH: [(Vec2, u8); 2] =
            [(Vec2::new(171.0, 55.0), 0), (Vec2::new(470.0, 55.0), 2)];
        /// The turbo frames, a side's first person's then the second's.
        pub const TURBO_FRAME: [[(Vec2, u8); 2]; 2] = [
            [(Vec2::new(95.0, 54.0), 0), (Vec2::new(174.0, 54.0), 0)],
            [(Vec2::new(545.0, 54.0), 2), (Vec2::new(466.0, 54.0), 2)],
        ];
        /// Where the constructor puts a side's first and second person's
        /// bar and word (left-anchored on the left, right on the right).
        pub const TURBO: [[f32; 2]; 2] = [[96.0, 179.0], [544.0, 461.0]];
        pub const TURBO_Y: f32 = 56.0;
        /// A third person's bar online (the original seats two a side):
        /// under the second's, beside the trick points.
        pub const TURBO_THIRD_DROP: f32 = 15.0;
        /// The scores (`0` side 1, `1` side 0).
        pub const SCORE: [(Vec2, u8); 2] =
            [(Vec2::new(285.0, 53.0), 5), (Vec2::new(350.0, 53.0), 5)];
        /// The trick points (`0xe` side 1, `0xf` side 0).
        pub const TRICK_POINTS: [(Vec2, u8); 2] =
            [(Vec2::new(94.0, 71.0), 0), (Vec2::new(541.0, 71.0), 2)];
        pub const SHOT_CLOCK: (Vec2, u8) = (Vec2::new(540.0, 400.0), 5);
        pub const VIOLATION: (Vec2, u8) = (Vec2::new(320.0, 435.0), 9);
        /// The combo line, or "Press START to exit demo" (`0xc`).
        pub const BOTTOM_LINE: (Vec2, u8) = (Vec2::new(314.0, 421.0), 9);
        pub const GAME_POINT: (Vec2, u8) = (Vec2::new(314.0, 402.0), 1);
        pub const REWIND: (Vec2, u8) = (Vec2::new(50.0, 449.0), 4);
        /// The court's name and place, already shifted by `FUN_002b1ef8`.
        pub const COURT: (Vec2, u8) = (Vec2::new(320.0, 294.0), 1);
        pub const PLACE: (Vec2, u8) = (Vec2::new(320.0, 338.0), 1);
        /// The panel's corner (`FUN_00213150`: bottom-left at (50, 440) or
        /// bottom-right at (590, 440), less 16).
        pub const PANEL_LEFT: Vec2 = Vec2::new(50.0, 424.0);
        pub const PANEL_RIGHT: Vec2 = Vec2::new(590.0, 424.0);
    }

    /// The pop-ups' paths (`FUN_002ad190` writes them at `+0x104..+0x158`).
    pub mod popup {
        use bevy::prelude::Vec2;

        /// By screen side: left (side 1), right (side 0).
        pub const WORD_START: [Vec2; 2] = [Vec2::new(53.0, 114.0), Vec2::new(588.0, 114.0)];
        pub const WORD_MIDDLE: Vec2 = Vec2::new(320.0, 81.0);
        pub const WORD_CORNER: [Vec2; 2] = [Vec2::new(135.0, 24.0), Vec2::new(515.0, 24.0)];
        pub const POINTS_START: [Vec2; 2] = [Vec2::new(53.0, 139.0), Vec2::new(588.0, 139.0)];
        pub const POINTS_MIDDLE: Vec2 = Vec2::new(320.0, 106.0);
        pub const POINTS_CORNER: [Vec2; 2] = [Vec2::new(135.0, 37.0), Vec2::new(515.0, 37.0)];
        /// The size it flies in at and settles to (`+0x154`, `+0x158`).
        pub const START_SCALE: f32 = 3.0;
        pub const SCALE: f32 = 1.0;
    }

    /// Text colours (`+0x4a4`, A-R-G-B), as R, G, B, A.
    pub mod colour {
        pub const POPUP: [u8; 4] = [0xff, 0x7b, 0x08, 0xff];
        /// A pop-up while its team holds a pocket (team `+0x20`).
        pub const POPUP_POCKETED: [u8; 4] = [0xfb, 0xac, 0x00, 0xff];
        pub const AMBER: [u8; 4] = [0xfb, 0xac, 0x00, 0xff];
        pub const SCORE: [u8; 4] = [0xf6, 0xf4, 0xd7, 0xff];
        pub const BOTTOM: [u8; 4] = [0xfa, 0xf5, 0x9f, 0xff];
        pub const BLACK: [u8; 4] = [0, 0, 0, 0xff];
        /// The shot clock's and the violation's outline (`0xff040837`), and
        /// the court's name's and place's (`0xff010635`).
        pub const CLOCK_OUTLINE: [u8; 4] = [0x04, 0x08, 0x37, 0xff];
        pub const COURT_OUTLINE: [u8; 4] = [0x01, 0x06, 0x35, 0xff];
        /// Track Info's and the Opposition's words (`FUN_00212dc0`).
        pub const PANEL_TEXT: [u8; 4] = [0xdc, 0xdc, 0xbb, 0xff];
        pub const PANEL_LABEL: [u8; 4] = [0x93, 0xa7, 0xbb, 0xff];
        pub const REPLAY_WORD: [u8; 4] = [0xf6, 0xf4, 0xd7, 0xff];
    }

    /// The fonts the constructor picks (`FUN_002e5578`).
    pub mod font {
        pub const POPUP: usize = 10;
        pub const SCORE: usize = 11;
        pub const TRICK_POINTS: usize = 1;
        pub const CLOCK: usize = 6;
        pub const COURT: usize = 6;
        pub const PLACE: usize = 4;
        pub const DEMO: usize = 3;
        pub const COMBO: usize = 2;
        pub const GAME_POINT: usize = 2;
        pub const PANEL: usize = 1;
        pub const REPLAY_WORD: usize = 2;
    }

    /// `overlays.txt` (`XLOC_GM` file 0x59) by index.
    pub mod words {
        pub const GAMEBREAKER: u16 = 0x5903;
        pub const VIOLATION: u16 = 0x5905;
        pub const SCORE_NOW: u16 = 0x5906;
        pub const SUPER: u16 = 0x590c;
        pub const DEFENSE: u16 = 0x590e;
        pub const COMBO: u16 = 0x590f;
        pub const POCKET: u16 = 0x5925;
        pub const CANCEL: u16 = 0x5926;
        pub const DEMO: u16 = 0x592c;
        pub const OPPOSITION: u16 = 0x592f;
        pub const EA_TRAX: u16 = 0x5932;
        pub const WIN_BY_TWO: u16 = 0x5934;
        pub const REPLAY: u16 = 0x5935;
        /// The replay prompt's word (`0x3e`, its layout text).
        pub const PROMPT: u16 = 0x5929;
    }

    /// The pop-up kinds' words (`0x48c250`, by kind) and points
    /// (`0x48d3b0`); kinds 0, 1, 4, 5, 6, 9 and 17 name their move instead.
    pub const KIND_WORD: [u16; 19] = [
        0, 0, 0x5913, 0x5914, 0, 0, 0x5911, 0x591e, 0x591f, 0x5903, 0x591a, 0x591b, 0x591d, 0x5915,
        0x5916, 0x5920, 0x5922, 0x5921, 0x5926,
    ];
    pub const KIND_POINTS: [i32; 19] = [
        500, 1_000, 750, 250, 1_000, 1_500, 3_000, 2_500, 1_000, 25_000, 500, 500, 500, 2_000,
        25_000, 500, 3_000, 3_000, 5_000,
    ];

    /// The combo line's word per ring class (`0x48de50`).
    pub const CLASS_WORD: [u16; 22] = [
        0x5910, 0x5911, 0x5912, 0x5913, 0x5914, 0x5915, 0x5916, 0x5903, 0x5917, 0x5933, 0x5918,
        0x5919, 0x591a, 0x591b, 0x591c, 0x591d, 0x5920, 0x5919, 0x5926, 0x591e, 0x591f, 0x5922,
    ];
}

/// Pop-up kinds (`FUN_002b87c0`'s third argument).
pub mod kind {
    pub const LONG_RANGE: usize = 2;
    pub const SHORT_RANGE: usize = 3;
    pub const LAYUP: usize = 4;
    pub const DUNK: usize = 5;
    pub const ALLEY_OOP: usize = 6;
    pub const BLOCK: usize = 7;
    pub const STEAL: usize = 8;
    pub const GAMEBREAKER: usize = 9;
    pub const PICK: usize = 10;
    pub const PUMP_FAKE: usize = 11;
    pub const DIVE: usize = 12;
    pub const TIP_SLAM: usize = 13;
    pub const BUSTED_RIM: usize = 14;
    pub const REBOUND: usize = 15;
    pub const TRICK_COUNTER: usize = 16;
    pub const CANCEL: usize = 18;
}

/// The screen side a team's HUD sits on: side 1 left (0), side 0 right (1).
pub fn screen_side(team: usize) -> usize {
    usize::from(team == 0)
}

/// The rewrite's own aid, off unless `--play-by-play`: a line naming the
/// latest play across the top (the original has none).
#[derive(Resource)]
pub struct PlayByPlay;

/// The move names, read from the executable at startup.
#[derive(Resource, Default)]
pub struct Names(pub street_assets::overlays::MoveNames);

/// The HUD's own pictures beside `HUD.SSH`: the logos it draws
/// (`XATEAMS` images 2 and 3 of a team's sheet, `FUN_00210048(w, 1, logo,
/// 2 | 3, 0)`), `XACOMMON` sheet 0 image 5's frames 1 (the panel's box)
/// and 2 (the white star), and `REPLAY.SSH` for the banner.
#[derive(Resource, Default)]
pub struct HudArt {
    pub logos: Vec<[Option<(Handle<Image>, Vec2)>; 2]>,
    pub panel_box: Option<(Handle<Image>, Vec2)>,
    pub star: Option<(Handle<Image>, Vec2)>,
    pub replay: Option<crate::banner::ReplayArt>,
}

/// Startup: the names and the HUD's pictures.
pub(crate) fn start(
    mut commands: Commands,
    mut disc: ResMut<Disc>,
    mut images: ResMut<Assets<Image>>,
) {
    let names = street_assets::overlays::MoveNames::read(&mut disc.iso).unwrap_or_else(|e| {
        warn!("pop-up move names: {e}");
        Default::default()
    });
    commands.insert_resource(Names(names));
    let mut art = HudArt::default();
    let mut frame_image = |s: &street_assets::ssh::Sheet, image: usize, frame: usize| {
        let t = s.textures.get(image)?;
        let [x, y, w, h] = s.frames.get(image)?.get(frame)?.bounds();
        let (x, y, w, h) = (
            u32::try_from(x).ok()?,
            u32::try_from(y).ok()?,
            u32::try_from(w).ok()?,
            u32::try_from(h).ok()?,
        );
        (w > 0 && h > 0 && x + w <= t.width && y + h <= t.height).then(|| {
            let cropped = crate::art::crop(t, x, y, w, h);
            (
                images.add(street_render::convert::image(&cropped)),
                Vec2::new(w as f32, h as f32),
            )
        })
    };
    if let Ok(sheets) = street_assets::fe::read(&mut disc.iso, "/FE/XATEAMS.BIN") {
        art.logos = sheets
            .iter()
            .map(|s| [frame_image(s, 2, 0), frame_image(s, 3, 0)])
            .collect();
    }
    if let Some(s) = street_assets::fe::read(&mut disc.iso, "/FE/XACOMMON.BIN")
        .ok()
        .and_then(|s| s.into_iter().next())
    {
        art.panel_box = frame_image(&s, 5, 1);
        art.star = frame_image(&s, 5, 2);
    }
    match street_assets::fe::sheet(&mut disc.iso, "/FE/REPLAY.SSH") {
        Ok(sheet) => art.replay = crate::banner::ReplayArt::new(&sheet, &mut images),
        Err(e) => warn!("replay banner art: {e}"),
    }
    commands.insert_resource(art);
}

/// One pop-up slot (`FUN_002b87c0`, `FUN_002b4c38`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Popup {
    pub word: String,
    pub points: String,
    pub colour: [u8; 4],
    /// Updates left: the word's (`T` = fps × 1.5) and the points' (`P` =
    /// fps, started as the word arrives).
    pub word_timer: i32,
    pub points_timer: i32,
    /// The slot drawn over the other (the newest).
    pub top: bool,
}

/// A score box (`+0x160..+0x174`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScoreBox {
    pub shown: u32,
    pub target: u32,
    /// Updates into a change's "+N" (`+0x170`/`+0x174`).
    pub counter: i32,
    pub scale: f32,
}

/// A side's meter as drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeterView {
    /// The fill's width (`0x34`/`0x35`), chasing the target.
    pub fill: f32,
    /// Full (`+0x1f0`/`+0x1f4`): the cream bar and the lit word blink.
    pub full: bool,
    pub blink: u8,
    /// Pocketed (`FUN_002b8168(side, 1)`): the blue bar, the yellow fill.
    pub pocketed: bool,
    /// The lit word's pulse counter (`+0x198`/`+0x19c`) and size.
    pub pulse: i32,
    pub lit_size: Vec2,
    /// The lit word showing.
    pub lit: bool,
    /// A cancel draining this side's pocket (`+0x230`, `FUN_002b6d88`):
    /// the blue bar's width.
    pub drain: Option<f32>,
}

impl Default for MeterView {
    fn default() -> Self {
        Self {
            fill: 0.0,
            full: false,
            blink: 0xff,
            pocketed: false,
            pulse: -1,
            lit_size: data::GAMEBREAKER_LIT.size,
            lit: false,
            drain: None,
        }
    }
}

/// A fading panel (`0x3f`, `FUN_002b2690`): Track Info or the Opposition.
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub lines: Vec<(String, [u8; 4])>,
    /// Bottom-right at (590, 424) instead of bottom-left at (50, 424).
    pub right: bool,
    /// Updates since it showed or began to leave (`+0x21c`).
    pub counter: i32,
    pub leaving: bool,
    /// The Opposition (the song waits for it).
    pub opposition: bool,
}

impl Panel {
    /// Its alpha byte (`FUN_002b2690`): in over fps / 5, out over fps / 6.
    pub fn alpha(&self) -> u8 {
        let t = self.counter as f32;
        let a = if self.leaving {
            255.0 - t / (FPS as f32 / 6.0) * 255.0
        } else {
            t / (FPS as f32 / 5.0) * 255.0
        };
        a.clamp(0.0, 255.0) as u8
    }
}

/// What the HUD shows, stepped once an update like `FUN_002b3048`.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct HudState {
    /// The screen hidden (an intro script, a replay, a gamebreaker): only
    /// the banner, the prompt and the attract line draw.
    pub hidden: bool,
    /// By screen side (0 left, 1 right), two slots each.
    pub popups: [[Popup; 2]; 2],
    /// The combo line (`0xc` in a match) and its updates left (`+0x1b0`).
    pub combo: Option<(String, i32)>,
    /// "Shot clock violation!" updates left (`+0x1b4`).
    pub violation: i32,
    /// The shot clock's digit (`2`) and the poster's last value
    /// (`0x3d30c8`, the clock's whole seconds plus one).
    pub clock: Option<i32>,
    pub clock_second: Option<i32>,
    pub scores: [ScoreBox; 2],
    pub meters: [MeterView; 2],
    /// Alpha bytes stepping down by 3 an update: the game-point line, the
    /// attract line, the replay prompt.
    pub game_point: Option<u8>,
    pub attract: u8,
    pub prompt: u8,
    /// The court's name and place with their stars (`0x10..0x13`), and the
    /// fade-in's updates (`+0x220`).
    pub court: Option<i32>,
    /// Intro scripts started and ended (the Overlays task's `+4`).
    pub scripts_started: u32,
    pub scripts_ended: u32,
    /// A staged Level 2's gamebreaker task runs (the simulation's, kind 0).
    pub staged_task: bool,
    pub panel: Option<Panel>,
    /// The song waiting for the Opposition to go (`+0x1c0..+0x1d8`).
    pub pending_song: Option<Vec<(String, [u8; 4])>>,
    pub song_key: Option<String>,
    /// Each player's last trick (actor `+0x264`), last dunk id and lay-up
    /// kind (`+0x1e8`, `+0x26c`, [`HudState::note_moves`]); each team's
    /// last trick value (`+0x24`).
    pub last_trick: Vec<usize>,
    pub last_dunk: Vec<usize>,
    pub last_layup: Vec<usize>,
    /// Each player's last dunk a putback (+0x1e4 1, a tip slam).
    pub tip_slam: Vec<bool>,
    pub last_trick_points: [i32; 2],
    /// The team whose combo paid this tick (the basket shows "combo").
    pub paid: Option<usize>,
    pub banner: crate::banner::Banner,
    /// The last tick handled, and the intro's state then.
    pub tick: Option<u64>,
    pub intro_over: bool,
    pub part: Option<(usize, u32)>,
}

impl Default for HudState {
    fn default() -> Self {
        Self {
            hidden: false,
            popups: Default::default(),
            combo: None,
            violation: 0,
            clock: None,
            clock_second: None,
            scores: [ScoreBox {
                scale: 0.75,
                ..default()
            }; 2],
            meters: [MeterView::default(); 2],
            game_point: None,
            attract: 0xff,
            prompt: 0xff,
            court: None,
            scripts_started: 0,
            scripts_ended: 0,
            staged_task: false,
            panel: None,
            pending_song: None,
            song_key: None,
            last_trick: Vec::new(),
            last_dunk: Vec::new(),
            last_layup: Vec::new(),
            tip_slam: Vec::new(),
            last_trick_points: [0; 2],
            paid: None,
            banner: Default::default(),
            tick: None,
            intro_over: false,
            part: None,
        }
    }
}

/// `T`, `A` and `P` of the pop-ups (`+0xf4` = fps × 1.5, `+0xfc` = fps /
/// 4, `+0xf8` = fps).
pub const WORD_TIME: i32 = FPS * 3 / 2;
pub const MOVE_TIME: i32 = FPS / 4;
pub const POINTS_TIME: i32 = FPS;

impl HudState {
    /// `FUN_002b87c0`: a pop-up of `word` and `points` for `team`.
    pub fn popup(&mut self, team: usize, word: String, points: i32, pocketed: bool) {
        let slots = &mut self.popups[screen_side(team)];
        // The first slot when free, else the second; the new one on top.
        let k = usize::from(slots[0].word_timer != 0);
        let colour = if pocketed {
            data::colour::POPUP_POCKETED
        } else {
            data::colour::POPUP
        };
        slots[k] = Popup {
            word,
            points: points.to_string(),
            colour,
            word_timer: WORD_TIME,
            points_timer: 0,
            top: true,
        };
        slots[1 - k].top = false;
    }

    /// Each player's dunk and lay-up as the pop-ups read them: the dunk id
    /// (+0x1e8) or lay-up kind (+0x26c) of his move's row, the table's or
    /// past it the leaps', the board's and the mid-air lay-ups'
    /// ([`timing_tables::move_row`]). The putback (the sim's own) has no
    /// row: the dunk id stays as it was, and its +0x1e4 1, a tip slam, is
    /// noted.
    pub(crate) fn note_moves(&mut self, w: &street_sim::World) {
        let n = w.players.len();
        for v in [
            &mut self.last_trick,
            &mut self.last_dunk,
            &mut self.last_layup,
        ] {
            v.resize(n, 0);
        }
        self.tip_slam.resize(n, false);
        for (k, p) in w.players.iter().enumerate() {
            match p.action {
                Action::Dunking { row, style, .. } => {
                    self.tip_slam[k] = style == street_sim::player::DunkStyle::Putback;
                    if let Some(r) = timing_tables::move_row(row) {
                        self.last_dunk[k] = usize::from(r.id);
                    }
                }
                Action::Layup { row, .. } => {
                    if let Some(r) = timing_tables::move_row(row) {
                        self.last_layup[k] = usize::from(r.id);
                    }
                }
                _ => {}
            }
        }
    }

    /// `FUN_002b4c38`'s first half: the older pop-ups hurried off when
    /// another flies, then every timer down by one (the points start as
    /// their word arrives).
    fn step_popups(&mut self) {
        let live: Vec<i32> = self
            .popups
            .iter()
            .flatten()
            .map(|p| p.word_timer)
            .filter(|&t| t > 0)
            .collect();
        if live.len() > 1 {
            let newest = live.iter().copied().max().unwrap_or(0);
            for p in self.popups.iter_mut().flatten() {
                if p.word_timer > 0 && p.word_timer < newest {
                    p.word_timer = hurry(p.word_timer, WORD_TIME);
                    p.points_timer = hurry(p.points_timer, POINTS_TIME);
                }
            }
        }
        for p in self.popups.iter_mut().flatten() {
            if p.word_timer > 0 {
                if p.word_timer == WORD_TIME - MOVE_TIME {
                    p.points_timer = POINTS_TIME;
                }
                p.word_timer -= 1;
            }
            if p.points_timer > 0 && p.word_timer < WORD_TIME - MOVE_TIME - 1 {
                p.points_timer -= 1;
            }
        }
    }
}

/// A timer cut short (`FUN_002b4c38`): past the fly-in, then to the exit.
fn hurry(timer: i32, total: i32) -> i32 {
    if timer > total - MOVE_TIME {
        total - MOVE_TIME
    } else if timer > MOVE_TIME {
        MOVE_TIME
    } else {
        timer
    }
}

/// Where a pop-up's word (or points) is and how large, `timer` updates
/// left of `total`.
pub fn popup_place(side: usize, timer: i32, total: i32, points: bool) -> (Vec2, f32) {
    use data::popup::*;
    let (start, middle, corner) = if points {
        (POINTS_START[side], POINTS_MIDDLE, POINTS_CORNER[side])
    } else {
        (WORD_START[side], WORD_MIDDLE, WORD_CORNER[side])
    };
    if timer >= total - MOVE_TIME {
        let f = (total - timer) as f32 / MOVE_TIME as f32;
        (
            start + (middle - start) * f,
            START_SCALE - (START_SCALE - SCALE) * f,
        )
    } else if timer <= MOVE_TIME {
        let f = (MOVE_TIME - timer) as f32 / MOVE_TIME as f32;
        (middle + (corner - middle) * f, SCALE - f)
    } else {
        (middle, SCALE)
    }
}

/// The meter's target width (`FUN_002b7eb0`: meter / 50,000 × 167).
pub fn target_width(meter: u32) -> f32 {
    meter as f32 / street::METER_FULL as f32 * data::METER_WIDTH
}

/// `FUN_002b6840`: a fill chases its target, up a quarter pixel an update,
/// down a pixel.
pub fn chase(fill: f32, target: f32) -> f32 {
    let d = target - fill;
    if d > 0.25 {
        fill + 0.25
    } else if d < -1.0 {
        fill - 1.0
    } else {
        target
    }
}

/// `FUN_002b6210`: the lit word's pulse, `counter` updates left.
pub fn pulse(size: Vec2, counter: i32) -> Vec2 {
    match counter {
        c if c > 50 => size * 1.01,
        21..=50 => size * 0.99,
        6..=20 => size * 1.01,
        1..=5 => size * 0.99,
        _ => data::GAMEBREAKER_LIT.size,
    }
}

/// The combo line (`FUN_002b87c0`, the English branch): the walked slots'
/// words joined by " + " (a line break before slot 4's word when it is
/// the third word or later), " super" for six, " defense" when the newest
/// class is a defensive one (above 18), then " combo".
pub fn combo_line(walked: &[i8; 6], word: impl Fn(u16) -> String) -> Option<String> {
    let first = walked.iter().position(|&c| c >= 0)?;
    let class_word = |c: i8| {
        data::CLASS_WORD
            .get(usize::try_from(c).ok()?)
            .map(|&id| word(id))
    };
    let mut line = class_word(walked[first])?;
    let count = 6 - first;
    for (k, &class) in walked.iter().enumerate().skip(first + 1) {
        line.push_str(" + ");
        // The break goes before slot 4 when it is the third word or later.
        if k == 4 && k - first + 1 > 2 {
            line.push('\n');
        }
        line.push_str(&class_word(class)?);
    }
    if count == 6 {
        line.push(' ');
        line.push_str(&word(data::words::SUPER));
    }
    let newest = i32::from(walked[5]);
    if (i32::from(walked[first]) < 0x13 || newest > 7) && newest > 0x12 {
        line.push(' ');
        line.push_str(&word(data::words::DEFENSE));
    }
    line.push(' ');
    line.push_str(&word(data::words::COMBO));
    Some(line)
}

/// The shot clock's value as the poster gives it (`0x13f598`: the clock's
/// whole seconds, `FUN_00131378` truncating, plus one); the overlay shows
/// it under nine (`FUN_002e2848`).
pub fn clock_value(seconds: f32) -> i32 {
    (seconds.max(0.0) as i32 % 60) + 1
}

/// Slot 65 for a staged Level 2: its task (kind 0) is the simulation's
/// (`street::GamebreakerTask`), which posts its end the update after its
/// script's state leaves 3..7, its replay command's replay played: the
/// HUD shows again and the banner leaves (`FUN_002b7c68`). The
/// simulation's task ends in the game update that ends the script, whose
/// replay and own end hold the game (`held`): the post comes with the game
/// update after them.
fn staged_task_end(hud: &mut HudState, w: &street_sim::World, held: bool) {
    if held {
        return;
    }
    let staged_task = w.gamebreaker_task.is_some_and(|t| t.kind == 0);
    if hud.staged_task && !staged_task {
        hud.hidden = false;
        hud.banner.show(crate::banner::Mode::Out);
    }
    hud.staged_task = staged_task;
}

/// The HUD's update (`FUN_002b3048`, with the listener's calls before it).
#[allow(clippy::too_many_arguments)]
pub(crate) fn step(
    mut hud: ResMut<HudState>,
    sim: Res<Sim>,
    tick_events: Res<TickEvents>,
    control: Res<Control>,
    names: Option<Res<Names>>,
    gm: Option<Res<Gm>>,
    (setup, disc, intro, tape, task, sounds, options): (
        Res<MatchSetup>,
        Res<Disc>,
        Option<Res<crate::intro::MatchIntro>>,
        Option<Res<crate::tape::Tape>>,
        Option<Res<crate::tape::GamebreakerTask>>,
        Res<crate::audio::Sounds>,
        Option<Res<crate::options::Options>>,
    ),
    running: Option<Res<crate::story::Running>>,
) {
    let hud = &mut *hud;
    let school = running.is_some_and(|r| matches!(r.0, Some(crate::story::Run::School(_))));
    let w = tape
        .as_deref()
        .and_then(crate::tape::Tape::live)
        .unwrap_or(&sim.world);
    let word = |id: u16| {
        gm.as_deref()
            .map(|g| g.text(id).replace("\\n", "\n"))
            .unwrap_or_default()
    };
    let opts = options.map(|o| o.0).unwrap_or_default();
    hud.note_moves(w);

    // The intro's scripts (slots 61 and 62, the script manager's start and
    // end posts at `0x3371d8`, `0x337320`): the first start shows the
    // court's name and hides the HUD, the first end hides the name, the
    // last end shows the HUD and the Opposition (`FUN_002b2a98`).
    let part = intro
        .as_deref()
        .and_then(|i| i.current.as_ref().map(|p| p.script));
    let intro_over = part.is_none();
    let key = part.map(|s| (s, hud.scripts_started));
    if part.is_some() && hud.part.map(|(s, _)| s) != part {
        if hud.part.is_some() {
            hud.scripts_ended += 1;
            if hud.scripts_ended == 1 {
                hud.court = None;
            }
        }
        hud.scripts_started += 1;
        hud.hidden = true;
        if hud.scripts_started == 1 && !intro.as_deref().is_some_and(|i| i.replaying()) {
            hud.court = Some(0);
        }
    }
    hud.part = key.map(|(s, _)| (s, hud.scripts_started));
    if intro_over && !hud.intro_over {
        if hud.scripts_started > hud.scripts_ended {
            hud.scripts_ended = hud.scripts_started;
        }
        hud.part = None;
        hud.court = None;
        hud.hidden = false;
        // Not in Street School (`FUN_002b2a98`: game `+0x9c` 4).
        if !control.demo && !school && w.tick == 0 {
            hud.panel = opposition(w, &control, &setup, &disc, &word);
        }
    }
    hud.intro_over = intro_over;

    // The replay manager's start and end (slots 58 and 59, but for the
    // gamebreaker task's mode 5 and mode 7) and the gamebreaker's end
    // (slot 65).
    if let Some(t) = tape.as_deref() {
        let instant = t.mode != crate::tape::mode::GAMEBREAKER && t.mode != 7;
        if t.started && instant {
            hud.hidden = true;
            hud.banner.show(crate::banner::Mode::ReplayIn);
        }
        if t.ended && instant {
            hud.hidden = false;
            hud.banner.show(crate::banner::Mode::Out);
        }
    }
    if task.is_some_and(|t| t.ended) {
        hud.hidden = false;
        hud.banner.show(crate::banner::Mode::Out);
    }
    // The staged task's end post comes with the game update after its
    // script's replay and its script's own end.
    let held = tick_events.replay || tape.as_deref().is_some_and(|t| t.staged.is_some());
    staged_task_end(hud, w, held);

    // The song (`FUN_002b2380`, with Track Info on): shown at once, or
    // after the Opposition when it is up.
    if opts.track_info && !control.demo {
        let song = sounds.now_playing();
        let key = song.map(|s| s.folder.clone());
        if key != hud.song_key {
            hud.song_key = key;
            if let Some(s) = song {
                let lines = track_lines(s, &word);
                if !intro_over || hud.panel.as_ref().is_some_and(|p| p.opposition) {
                    hud.pending_song = Some(lines);
                } else {
                    hud.panel = Some(Panel {
                        lines,
                        right: false,
                        counter: 0,
                        leaving: false,
                        opposition: false,
                    });
                }
            }
        }
    }

    // The game's events (the listener's slots), once a tick.
    if hud.tick != Some(w.tick) && !tick_events.replay {
        hud.tick = Some(w.tick);
        hud.paid = None;
        let names = names.as_deref().map(|n| &n.0);
        for event in &tick_events.events {
            on_event(hud, w, event, names, &word, control.demo);
        }
        // The shot clock's whole seconds (slots 48 and 50).
        let value = (w.rules.shot_clock.is_some() && matches!(w.phase, Phase::Live))
            .then(|| clock_value(w.shot_clock));
        if value != hud.clock_second {
            hud.clock = match value {
                Some(v) if v < 9 && hud.clock_second.is_none_or(|c| v < c) => Some(v),
                _ => None,
            };
            hud.clock_second = value;
        }
    }

    // `FUN_002b3048` proper.
    hud.banner.step();
    if let Some(c) = hud.court.as_mut() {
        *c += 1;
    }
    hud.prompt = hud.prompt.wrapping_sub(3);
    hud.attract = hud.attract.wrapping_sub(3);
    if hud.hidden {
        return;
    }
    hud.step_popups();
    if let Some((_, left)) = hud.combo.as_mut() {
        *left -= 1;
    }
    if hud.combo.as_ref().is_some_and(|(_, left)| *left <= 0) {
        hud.combo = None;
    }
    hud.violation = (hud.violation - 1).max(0);
    for team in 0..2 {
        let side = screen_side(team);
        // The score box (`FUN_002b4c38`'s end): a change shows "+N" for
        // two seconds when the shown score was not set with it (a
        // gamebreaker's basket, `FUN_002e2568`).
        let b = &mut hud.scores[side];
        b.target = w.score[team];
        if b.target != b.shown {
            if b.counter < 2 * FPS {
                b.scale = match b.counter {
                    0 => 0.35,
                    1..=50 => b.scale + 0.0117,
                    _ => b.scale - 0.005,
                };
                b.counter += 1;
            } else {
                b.shown = b.target;
                b.counter = 0;
            }
        }
        if b.target == b.shown {
            b.scale = if b.shown < 100 { 0.75 } else { 0.6 };
        }
        // The meter.
        let target = target_width(w.teams[team].meter);
        let m = &mut hud.meters[side];
        if let Some(width) = m.drain.as_mut() {
            // `FUN_002b6d88`: the fill goes, then the blue bar down to the
            // target, two pixels an update.
            if m.fill > 0.0 {
                m.fill = (m.fill - 2.0).max(0.0);
            } else if *width - target > 2.0 {
                *width -= 2.0;
            } else {
                m.drain = None;
                m.pocketed = false;
                m.fill = target;
            }
        } else {
            m.fill = chase(m.fill, target);
        }
        if m.full {
            m.blink = m.blink.wrapping_sub(3);
        }
        if m.pulse >= 0 {
            m.lit_size = pulse(m.lit_size, m.pulse);
            m.pulse -= 1;
        }
    }
    // The game-point line (`0xd`).
    let point = game_point(w, &control, &opts, hud.combo.is_some(), hud.violation > 0);
    hud.game_point = match (point, hud.game_point) {
        (false, _) => None,
        (true, None) => Some(0xff),
        (true, Some(a)) => Some(a.wrapping_sub(3)),
    };
    // The panel (`FUN_002b2690`), held back while the shot clock, the
    // violation or the combo line shows (`FUN_002b3048`).
    let busy = hud.clock.is_some() || hud.violation > 0 || hud.combo.is_some();
    let pending = hud.pending_song.take();
    let mut next = None;
    let mut gone = false;
    if let Some(p) = hud.panel.as_mut() {
        if busy && !p.leaving && p.counter < 2 * FPS {
            p.counter = 0;
        } else if p.leaving && p.counter >= FPS / 6 {
            gone = true;
        } else if !p.leaving && p.counter >= 3 * FPS {
            match (p.opposition, &pending) {
                (true, Some(lines)) => next = Some(lines.clone()),
                _ => {
                    p.leaving = true;
                    p.counter = -1;
                }
            }
        }
        p.counter += 1;
    }
    if next.is_none() {
        hud.pending_song = pending;
    }
    if gone {
        hud.panel = None;
    }
    if let Some(lines) = next {
        hud.panel = Some(Panel {
            lines,
            right: false,
            counter: 0,
            leaving: false,
            opposition: false,
        });
    }
}

/// The listener's handlers (`0x495b10`'s slots) on the simulation's events.
fn on_event(
    hud: &mut HudState,
    w: &street_sim::World,
    event: &Event,
    names: Option<&street_assets::overlays::MoveNames>,
    word: &impl Fn(u16) -> String,
    attract: bool,
) {
    let team_of = |p: usize| w.players.get(p).map_or(0, |p| p.team);
    let pocketed = |team: usize| w.teams[team].pocketed;
    let kind_word = |k: usize| data::KIND_WORD.get(k).map_or(String::new(), |&id| word(id));
    let kind_points = |k: usize| data::KIND_POINTS.get(k).copied().unwrap_or(0);
    let name = |table: &[String], k: usize| table.get(k).cloned().unwrap_or_default();
    let simple = |hud: &mut HudState, team: usize, k: usize| {
        hud.popup(team, kind_word(k), kind_points(k), pocketed(team));
    };
    match *event {
        // Slot 13, a trick move landing with its value: kind 1, the move's
        // name between the font's crowns (`*%s*`, `0x48ccc0`) and its
        // trick points (team `+0x24`).
        Event::Trick {
            player,
            points,
            script,
            ..
        } => {
            let team = team_of(player);
            // The trick scores at its script's end: the event names it.
            let script = Some(usize::from(script));
            if let Some(s) = script {
                hud.last_trick[player] = s;
            }
            hud.last_trick_points[team] = points as i32;
            if let (Some(n), Some(s)) = (names, script) {
                let text = format!("*{}*", name(&n.tricks, s));
                hud.popup(team, text, points as i32, pocketed(team));
            }
        }
        // Slot 13 again at a dunk's release (the post at `0x14bf28` with
        // value 0): kind 0, the ball owner's actor `+0x264` (written only
        // by the trick starts, `0x15f600`, `0x15c8e8`: the last trick's
        // name) and team `+0x24`.
        Event::Slam { player, .. } => {
            let team = team_of(player);
            if let Some(n) = names {
                let text = name(&n.tricks, hud.last_trick.get(player).copied().unwrap_or(0));
                let points = hud.last_trick_points[team];
                hud.popup(team, text, points, pocketed(team));
            }
        }
        // Slot 15: a steal, or a trick counter's (kinds 8 and 16).
        Event::Steal { player, .. } => simple(hud, team_of(player), kind::STEAL),
        Event::Counter { player, .. } => simple(hud, team_of(player), kind::TRICK_COUNTER),
        // Slot 16: a block (the rewrite's swats also come as goaltends).
        Event::Block { player, .. } | Event::Goaltend { player, .. } => {
            simple(hud, team_of(player), kind::BLOCK)
        }
        // Slot 17: a rebound picked up.
        Event::Rebound { player } => simple(hud, team_of(player), kind::REBOUND),
        // Slot 19 (`FUN_002e2418`): a passout special, named (`0x3fb790`)
        // with its value (team `+0x5c`).
        Event::Pass { from, style, .. } => {
            let index = match style {
                PassStyle::Glass => Some(0),
                PassStyle::Heezay => Some(1),
                _ => None,
            };
            if let (Some(n), Some(k)) = (names, index) {
                let team = team_of(from);
                let value = style.trick_points() as i32;
                hud.popup(team, name(&n.passouts, k), value, pocketed(team));
            }
        }
        // Slot 24 (`FUN_002e2ff8`): a dive's recovery.
        Event::Dive { player } => simple(hud, team_of(player), kind::DIVE),
        // Slot 73: a pump fake, posted for the other side.
        Event::PumpFake { player } => simple(hud, 1 - team_of(player), kind::PUMP_FAKE),
        // Slot 74: a pick that lands.
        Event::Pick { screener, .. } => simple(hud, team_of(screener), kind::PICK),
        // The combo that paid on the basket (`FUN_002b87c0` with game
        // `+0x148` set): "combo", the basket's own value plus the payout,
        // and the combo line for four seconds (not in the attract match).
        Event::Combo {
            team,
            walked,
            paid,
            newest,
            ..
        } => {
            let points = newest.max(0) + paid as i32;
            hud.popup(team, word(data::words::COMBO), points, pocketed(team));
            hud.paid = Some(team);
            if !attract && let Some(line) = combo_line(&walked, word) {
                hud.combo = Some((line, 4 * FPS));
            }
        }
        // Slot 26 (`FUN_002e2568`): a basket, unless its combo paid. The
        // scores are set at once (`FUN_002b7840` and `FUN_002b7828`), but
        // a gamebreaker's (only the target, so "+N" shows).
        Event::Score {
            team,
            player,
            basket,
            ..
        } => {
            if !matches!(basket, Basket::Gamebreaker { .. }) {
                for t in 0..2 {
                    let b = &mut hud.scores[screen_side(t)];
                    b.shown = w.score[t];
                    b.target = w.score[t];
                    b.counter = 0;
                }
            }
            if hud.paid == Some(team) {
                return;
            }
            let pick =
                |table: &[String], v: &[usize]| name(table, v.get(player).copied().unwrap_or(0));
            // A dunk or an alley-oop's finish (the ball's kind 3 or 4) that
            // wins the game (vt+0xbc) pops the busted rim, kind 14, but in
            // the trick points game (config+0xe8 1, `0x2e2658..0x2e268c`,
            // `0x2e2724..0x2e2754`), a gamebreaker's too.
            let wide = matches!(
                basket,
                Basket::Dunk { .. }
                    | Basket::AlleyOop { .. }
                    | Basket::Gamebreaker { dunk: true, .. }
            );
            let busted = wide && matches!(w.phase, Phase::Over { .. }) && w.rules.game_type() != 1;
            let (text, points) = match basket {
                _ if busted => (kind_word(kind::BUSTED_RIM), kind_points(kind::BUSTED_RIM)),
                Basket::AlleyOop { advanced: false } => (
                    names.map(|n| name(&n.alley_oops, 0)).unwrap_or_default(),
                    basket.trick_points() as i32,
                ),
                // A dunk by a shooter whose +0x1e4 is 1 (the putback's
                // family) pops kind 13 (`FUN_002e2568`, `0x2e2784`).
                Basket::Dunk { .. } if hud.tip_slam.get(player).copied().unwrap_or(false) => {
                    (kind_word(kind::TIP_SLAM), kind_points(kind::TIP_SLAM))
                }
                // An advanced alley-oop's finish is the ball's kind 4, a
                // dunk's (pop-up 5, named by the finisher's dunk id).
                Basket::Dunk { .. } | Basket::AlleyOop { advanced: true } => (
                    names
                        .map(|n| pick(&n.dunks, &hud.last_dunk))
                        .unwrap_or_default(),
                    basket.trick_points() as i32,
                ),
                Basket::Layup => (
                    names
                        .map(|n| pick(&n.layups, &hud.last_layup))
                        .unwrap_or_default(),
                    kind_points(kind::LAYUP),
                ),
                Basket::Jumper { long: true } => {
                    (kind_word(kind::LONG_RANGE), kind_points(kind::LONG_RANGE))
                }
                Basket::Jumper { long: false } => {
                    (kind_word(kind::SHORT_RANGE), kind_points(kind::SHORT_RANGE))
                }
                Basket::Gamebreaker { level, .. } => (
                    format!("{}{}", word(data::words::GAMEBREAKER), level.clamp(1, 2)),
                    basket.trick_points() as i32,
                ),
            };
            hud.popup(team, text, points, pocketed(team));
        }
        // Slot 52: the violation, three seconds.
        Event::ShotClockViolation { .. } => {
            hud.violation = 3 * FPS;
            hud.clock = None;
        }
        // Slot 66: a full meter blinks.
        Event::GamebreakerReady { team } => {
            let m = &mut hud.meters[screen_side(team)];
            m.full = true;
            m.lit = true;
        }
        // Slot 67: the meter drops (`FUN_002b8168(side, 0)`).
        Event::MeterDropped { team } => reset_meter(&mut hud.meters[screen_side(team)]),
        // Slot 70: pocketed (`FUN_002e2de0`, `FUN_002b8168(side, 1)`).
        Event::Pocket { team } => {
            let m = &mut hud.meters[screen_side(team)];
            m.full = false;
            m.pocketed = true;
            m.fill = 0.0;
            m.lit = true;
            m.blink = 0xff;
            m.pulse = 70;
            m.lit_size = data::GAMEBREAKER_LIT.size;
            hud.popup(team, word(data::words::POCKET), 0, true);
        }
        // Slot 71: a cancel (`FUN_002e2f10`): its 5,000, and the other
        // side's pocket drained (`FUN_002b86e8`).
        Event::Cancel { team } => {
            let m = &mut hud.meters[screen_side(team)];
            m.full = false;
            m.lit = false;
            hud.popup(
                team,
                word(data::words::CANCEL),
                kind_points(kind::CANCEL),
                pocketed(team),
            );
            let other = &mut hud.meters[screen_side(1 - team)];
            other.lit = false;
            other.drain = Some(data::METER_WIDTH);
        }
        // Slot 64 (`FUN_002e2e78`): a gamebreaker goes up: the meter
        // resets and the HUD hides behind GB1 or GB2.
        Event::Gamebreaker { player, level } => {
            let team = team_of(player);
            reset_meter(&mut hud.meters[screen_side(team)]);
            hud.hidden = true;
            hud.banner.show(if level >= 2 {
                crate::banner::Mode::Gamebreaker2
            } else {
                crate::banner::Mode::Gamebreaker1
            });
        }
        _ => {}
    }
}

/// `FUN_002b8168(side, 0)`: the orange fill from nothing, the target back,
/// the blue bar and the lit word gone.
fn reset_meter(m: &mut MeterView) {
    m.full = false;
    m.pocketed = false;
    m.fill = 0.0;
    m.lit = false;
    m.blink = 0xff;
    m.pulse = 70;
    m.drain = None;
}

/// The game-point line's condition (`FUN_002b3048`): Onscreen Help on and
/// not over the combo line or the violation; in a points game (kind 1)
/// both sides a point from the target ("win by two"); in a Trick Point
/// Challenge (kind 2) the person's side with the ball holding the target's
/// trick points and more than the other ("score now for win"; the original
/// also asks which side stands nearer the ball, the rewrite takes the
/// possession).
fn game_point(
    w: &street_sim::World,
    control: &Control,
    opts: &street_sim::records::Settings,
    combo: bool,
    violation: bool,
) -> bool {
    if !opts.help || control.demo || combo || violation || matches!(w.phase, Phase::Over { .. }) {
        return false;
    }
    match w.rules.goal {
        Goal::Points(n) => w.score.iter().all(|&s| n.saturating_sub(s) < 2),
        Goal::TrickPoints(n) => {
            let us = w.teams[w.possession].trick_points;
            let them = w.teams[1 - w.possession].trick_points;
            w.teams[w.possession].human && us >= n && us > them
        }
        Goal::Gamebreaker(_) => false,
    }
}

/// The game-point line's words: "win by two" in a points game, else
/// "score now for win".
fn game_point_word(w: &street_sim::World) -> u16 {
    match w.rules.goal {
        Goal::Points(_) => data::words::WIN_BY_TWO,
        _ => data::words::SCORE_NOW,
    }
}

/// The Track Info panel's lines (`FUN_00213150`): the artist, the track in
/// quotes, the album unless blank, then "TRAX" under four lines.
fn track_lines(
    song: &street_audio::music::Song,
    word: &impl Fn(u16) -> String,
) -> Vec<(String, [u8; 4])> {
    let mut lines = vec![
        (song.artist.clone(), data::colour::PANEL_TEXT),
        (format!("\"{}\"", song.title), data::colour::PANEL_TEXT),
    ];
    if !song.album.is_empty() && !song.album.starts_with(' ') {
        lines.push((song.album.clone(), data::colour::PANEL_TEXT));
    }
    if lines.len() < 4 {
        lines.push((word(data::words::EA_TRAX), data::colour::PANEL_LABEL));
    }
    lines
}

/// The Opposition panel (`FUN_002b2a98`): when exactly one side has no
/// person, "The Opposition:" and that side's three players ("%1S %2S" of
/// fields 2 and 1, `0x48c170`), on the right for side 1. The team's name
/// is passed to `FUN_00213858` but never drawn.
fn opposition(
    _w: &street_sim::World,
    control: &Control,
    setup: &MatchSetup,
    disc: &Disc,
    word: &impl Fn(u16) -> String,
) -> Option<Panel> {
    let human = |side: usize| control.seats.iter().any(|s| s.team == side);
    let side = (0..2).find(|&s| !human(s))?;
    if !human(1 - side) {
        return None;
    }
    let mut lines = vec![(word(data::words::OPPOSITION), data::colour::PANEL_LABEL)];
    for &record in &setup.lineups[side] {
        if let Some(p) = disc.players.get(record) {
            lines.push((
                format!("{} {}", p.first_name, p.last_name),
                data::colour::PANEL_TEXT,
            ));
        }
    }
    Some(Panel {
        lines,
        right: side == 1,
        counter: 0,
        leaving: false,
        opposition: true,
    })
}

// ===== Drawing =====

/// A text widget's look (`FUN_00238d20`): its colour and scale, a shadow
/// (colour, offset, and the shadow pass's height scale `+0xc0`) and an
/// outline drawn eight times around it (colour and offsets; its alpha the
/// sixth power of the widget's, the text's then squared).
#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    pub font: usize,
    pub colour: [u8; 4],
    pub scale: f32,
    pub shadow: Option<([u8; 4], Vec2, f32)>,
    pub outline: Option<([u8; 4], Vec2)>,
}

impl TextStyle {
    pub const fn plain(font: usize, colour: [u8; 4]) -> Self {
        Self {
            font,
            colour,
            scale: 1.0,
            shadow: None,
            outline: None,
        }
    }

    /// Prop 7's emphasis (`FUN_00238b88`): a black shadow two right and two
    /// down.
    pub const fn shadowed(font: usize, colour: [u8; 4]) -> Self {
        Self {
            shadow: Some((data::colour::BLACK, Vec2::new(2.0, 2.0), 1.0)),
            ..Self::plain(font, colour)
        }
    }
}

/// What drawing text needs.
pub(crate) struct Writer<'a> {
    pub labels: &'a mut Labels,
    pub look: &'a Look,
    pub images: &'a mut Assets<Image>,
}

impl Writer<'_> {
    /// `s` at the widget point `at` with the layout's alignment bits: the
    /// box one text height tall plus a line height per further line, each
    /// line placed across by its own width (`FUN_00238d20`, `FUN_00239e70`).
    pub fn text(
        &mut self,
        out: &mut Vec<Piece>,
        s: &str,
        style: TextStyle,
        at: Vec2,
        align: u8,
        alpha: f32,
    ) {
        let Some(font) = self.look.fonts.get(style.font) else {
            return;
        };
        let lines: Vec<&str> = s.split('\n').collect();
        let k = style.scale;
        let line_height = font.line_height as f32;
        let height = (font.text_height as f32 + (lines.len() as f32 - 1.0) * line_height) * k;
        let top = at.y
            - match (align >> 2) & 3 {
                1 => height / 2.0,
                2 => height,
                _ => 0.0,
            };
        let widths: Vec<f32> = lines.iter().map(|l| font.measure(l) as f32 * k).collect();
        let a = alpha.clamp(0.0, 1.0);
        let mut layer = |out: &mut Vec<Piece>, colour: [u8; 4], offset: Vec2, tall: f32, a: f32| {
            for (row, line) in lines.iter().enumerate() {
                let Some(p) = self.labels.text(
                    self.look,
                    self.images,
                    style.font,
                    line,
                    colour,
                    None,
                    Vec2::ZERO,
                    Align::Left,
                ) else {
                    continue;
                };
                let left = at.x
                    - match align & 3 {
                        1 => widths[row] / 2.0,
                        2 => widths[row],
                        _ => 0.0,
                    };
                let pen = Vec2::new(left, top + row as f32 * line_height * k) + offset;
                let scale = Vec2::new(k, k * tall);
                let min = pen + p.rect.min * scale;
                let mut piece = Piece::new(
                    p.image,
                    Rect::from_corners(min, min + p.rect.size() * scale),
                );
                piece.colour = Color::srgba(1.0, 1.0, 1.0, a);
                out.push(piece);
            }
        };
        if let Some((colour, offset, tall)) = style.shadow {
            layer(out, colour, offset, tall, a);
        }
        let main = if let Some((colour, o)) = style.outline {
            let around = a.powi(6);
            for d in [
                Vec2::new(-o.x, -o.y),
                Vec2::new(-o.x, 0.0),
                Vec2::new(-o.x, o.y),
                Vec2::new(0.0, -o.y),
                Vec2::new(o.x, -o.y),
                Vec2::new(o.x, 0.0),
                Vec2::new(o.x, o.y),
                Vec2::new(0.0, o.y),
            ] {
                layer(out, colour, d, 1.0, around);
            }
            a * a
        } else {
            a
        };
        layer(out, style.colour, Vec2::ZERO, 1.0, main);
    }

    /// A text widget's height (`FUN_00239e70`).
    pub fn height(&self, font: usize, s: &str, scale: f32) -> f32 {
        self.look.fonts.get(font).map_or(0.0, |f| {
            let lines = s.split('\n').count() as f32;
            (f.text_height as f32 + (lines - 1.0) * f.line_height as f32) * scale
        })
    }
}

/// `%1N` (`FUN_001d60e0`): a number with thousands separators, as the
/// trick points read ("16,375").
pub fn grouped(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Where a part's top-left goes for its point and alignment.
pub fn anchor(at: Vec2, align: u8, size: Vec2) -> Vec2 {
    at - Vec2::new(
        match align & 3 {
            1 => size.x / 2.0,
            2 => size.x,
            _ => 0.0,
        },
        match (align >> 2) & 3 {
            1 => size.y / 2.0,
            2 => size.y,
            _ => 0.0,
        },
    )
}

/// A part of the HUD atlas (256 square) at a layout point (shifted for
/// the match), `span` of its texture in `n`, mirrored and/or upside down.
#[allow(clippy::too_many_arguments)]
fn sprite(
    out: &mut Vec<Piece>,
    atlas: &Handle<Image>,
    p: data::Part,
    (at, align): (Vec2, u8),
    size: Vec2,
    span: f32,
    (mirror, upside): (bool, bool),
    alpha: f32,
) {
    if size.x <= 0.0 || size.y <= 0.0 || alpha <= 0.0 {
        return;
    }
    let texel = 256.0 / 255.0;
    let source = Rect::new(
        p.uv0.x * texel,
        p.uv0.y * texel,
        (p.uv0.x + span) * texel,
        p.uv1.y * texel,
    );
    let min = anchor(at, align, size) + ELEMENT;
    let mut piece = Piece::part(atlas.clone(), source, Rect::from_corners(min, min + size));
    piece.flip = mirror;
    piece.flip_y = upside;
    piece.colour = Color::srgba(1.0, 1.0, 1.0, alpha);
    out.push(piece);
}

/// A whole part at its own size.
fn whole(
    out: &mut Vec<Piece>,
    atlas: &Handle<Image>,
    p: data::Part,
    at: (Vec2, u8),
    turn: (bool, bool),
) {
    sprite(out, atlas, p, at, p.size, p.uv1.x - p.uv0.x, turn, 1.0);
}

/// The persons' turbo bars by screen side, `(pad, player)` for up to two
/// a side, the lower pad first; a side without one shows the first free
/// pad (from pad 0 on the left, from pad 2 on the right) with its first
/// player (`FUN_002ad190`, `FUN_002b7498`; the original reads a computer
/// side's turbo through game `+0xb0`/`+0xbc`, which is not traced: the
/// rewrite takes the side's first player). Online, where a side seats
/// three, every person's bar shows, 5P and 6P in the rewrite's colours.
pub fn turbo_slots(w: &street_sim::World, control: &Control) -> [Vec<(usize, usize)>; 2] {
    let mut out: [Vec<(usize, usize)>; 2] = Default::default();
    let mut used = [false; MAX_PEOPLE];
    let most = if control.remote.is_empty() { 2 } else { 3 };
    for (number, team, player) in control.people() {
        let side = screen_side(team);
        let pad = number.min(MAX_PEOPLE - 1);
        if out[side].len() < most && !used[pad] {
            out[side].push((pad, player));
            used[pad] = true;
        }
    }
    for (side, first) in [(0usize, 0usize), (1, 2)] {
        if out[side].is_empty() {
            let team = usize::from(side == 0);
            let Some(pad) = (0..4)
                .map(|k| (first + k) % 4)
                .chain(4..MAX_PEOPLE)
                .find(|&p| !used[p])
            else {
                continue;
            };
            used[pad] = true;
            let player = w.players.iter().position(|p| p.team == team).unwrap_or(0);
            out[side].push((pad, player));
        }
    }
    out
}

/// Where a side's `k`th person's bar and word sit: the constructor's two
/// places, and a third under the second ([`data::at::TURBO_THIRD_DROP`]).
fn turbo_at(side: usize, k: usize) -> Vec2 {
    let drop = if k >= 2 {
        data::at::TURBO_THIRD_DROP
    } else {
        0.0
    };
    Vec2::new(data::at::TURBO[side][k.min(1)], data::at::TURBO_Y + drop)
}

/// The atlas and the 1P..4P bar a person's turbo is drawn from: their own
/// for 1P..4P; for 5P and 6P online, the bar their colour turns from in
/// the atlas turned to it ([`crate::art::PEOPLE_SHIFTS`]).
fn turbo_art<'a>(
    atlas: &'a Handle<Image>,
    people: &'a [Handle<Image>],
    pad: usize,
) -> (&'a Handle<Image>, usize) {
    match pad
        .checked_sub(4)
        .and_then(|k| Some((people.get(k)?, crate::art::PEOPLE_SHIFTS.get(k)?.0)))
    {
        Some((colours, base)) => (colours, base),
        None => (atlas, pad.min(3)),
    }
}

/// The HUD's pieces this frame, in `FUN_002b12a0`'s order.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    look: Option<Res<Look>>,
    art: Option<Res<Art>>,
    hud_art: Option<Res<HudArt>>,
    hud: Res<HudState>,
    mut labels: ResMut<Labels>,
    mut images: ResMut<Assets<Image>>,
    sim: Res<Sim>,
    cheats: Res<crate::setup::MatchCheats>,
    control: Res<Control>,
    (setup, disc, gm, tape, boards, opts, play_by_play, paused, lessons, prompt): (
        Res<MatchSetup>,
        Res<Disc>,
        Option<Res<Gm>>,
        Option<Res<crate::tape::Tape>>,
        Option<Res<crate::boards::PostGame>>,
        Option<Res<crate::options::Options>>,
        Option<Res<PlayByPlay>>,
        Option<Res<crate::game::Paused>>,
        Option<Res<crate::school_menu::LessonMenu>>,
        Option<Res<crate::school_prompt::Prompt>>,
    ),
    running: Option<Res<crate::story::Running>>,
    mut commands: Commands,
    mut slots: Query<(&look::Slot, &mut ImageNode, &mut Node, &mut Visibility)>,
) {
    let (Some(look), Some(art)) = (look.as_deref(), art.as_deref()) else {
        return;
    };
    // The post-game boards replace the overlays (the Pause task's
    // screens), and the pause screen hides them (`FUN_002b38a0`, restored
    // by `FUN_002b3970`).
    // Street School's lesson menu hides elements 0..62 (`FUN_002ba010`).
    if cheats.0.no_hud()
        || boards.is_some_and(|b| b.active())
        || paused.is_some_and(|p| p.open())
        || lessons.is_some_and(|m| m.open)
    {
        look::show(&mut commands, &mut slots, &[], Screen::Playing);
        return;
    }
    let Some(atlas) = art.hud.clone() else { return };
    labels.tidy();
    let w = tape
        .as_deref()
        .and_then(crate::tape::Tape::live)
        .unwrap_or(&sim.world);
    let word = |id: u16| {
        gm.as_deref()
            .map(|g| g.text(id).replace("\\n", "\n"))
            .unwrap_or_default()
    };
    let opts = opts.map(|o| o.0).unwrap_or_default();
    let hud_art = hud_art.as_deref();
    // Street School's elements keep their layout points (`FUN_002ad190`
    // moves them up 16 only outside it): 16 lower in layout space.
    let school = running.is_some_and(|r| matches!(r.0, Some(crate::story::Run::School(_))));
    let school_shift = Vec2::new(0.0, if school { -MATCH_SHIFT } else { 0.0 });
    let shift = ELEMENT + school_shift;
    let mut out: Vec<Piece> = Vec::new();
    let mut ink = Writer {
        labels: &mut labels,
        look,
        images: &mut images,
    };

    if !hud.hidden {
        let people = &art.hud_people[..];
        pictures(
            &mut out,
            (&atlas, people),
            &hud,
            w,
            &control,
            &setup,
            &disc,
            hud_art,
        );
        for p in &mut out {
            p.rect.min += school_shift;
            p.rect.max += school_shift;
        }
        // The texts, in the draw's order.
        for side in 0..2 {
            let team = usize::from(side == 0);
            let (at, align) = data::at::TRICK_POINTS[side];
            ink.text(
                &mut out,
                &grouped(i64::from(w.teams[team].trick_points)),
                TextStyle::shadowed(data::font::TRICK_POINTS, data::colour::AMBER),
                at + shift,
                align,
                1.0,
            );
        }
        if let Some(p) = hud.panel.as_ref() {
            draw_panel(&mut out, p, &mut ink, hud_art);
        }
        if let Some(a) = hud.game_point {
            let (at, align) = data::at::GAME_POINT;
            ink.text(
                &mut out,
                &word(game_point_word(w)),
                TextStyle::shadowed(data::font::GAME_POINT, data::colour::AMBER),
                at + shift,
                align,
                f32::from(a) / 255.0,
            );
        }
        if let Some((line, _)) = hud.combo.as_ref().filter(|_| !control.demo) {
            let style = TextStyle {
                scale: 0.8,
                shadow: Some((data::colour::BLACK, Vec2::new(-1.0, 2.0), 1.2)),
                ..TextStyle::plain(data::font::COMBO, data::colour::BOTTOM)
            };
            let (at, align) = data::at::BOTTOM_LINE;
            ink.text(&mut out, line, style, at + shift, align, 1.0);
        }
        for side in 0..2 {
            let b = &hud.scores[side];
            let (s, colour) = if b.shown == b.target {
                (b.shown.to_string(), data::colour::SCORE)
            } else {
                let d = i64::from(b.target) - i64::from(b.shown);
                (
                    format!("{}{}", if d > 0 { '+' } else { '-' }, d.abs()),
                    data::colour::AMBER,
                )
            };
            let style = TextStyle {
                scale: b.scale,
                ..TextStyle::plain(data::font::SCORE, colour)
            };
            let (at, align) = data::at::SCORE[side];
            ink.text(&mut out, &s, style, at + shift, align, 1.0);
        }
        // Street School's lesson prompt (elements 0x20..0x25 at the points
        // `FUN_002ba3b0` sets, GUI ones).
        if let Some(p) = prompt.as_deref().filter(|_| school) {
            p.draw(&mut out, &mut ink, &word, RUNTIME);
        }
        if let Some(c) = hud.court {
            draw_place(&mut out, &setup, &disc, c, &mut ink, hud_art);
        }
        // The pop-ups, the older under the newer.
        for side in 0..2 {
            let mut order = [0usize, 1];
            if hud.popups[side][0].top {
                order.swap(0, 1);
            }
            for k in order {
                let p = &hud.popups[side][k];
                for (points, timer, total) in [
                    (false, p.word_timer, WORD_TIME),
                    (true, p.points_timer, POINTS_TIME),
                ] {
                    let s = if points { &p.points } else { &p.word };
                    if timer <= 0 || (points && (s == "0" || s.is_empty())) {
                        continue;
                    }
                    let (at, scale) = popup_place(side, timer, total, points);
                    if scale > 0.0 {
                        let style = TextStyle {
                            scale,
                            ..TextStyle::plain(data::font::POPUP, p.colour)
                        };
                        ink.text(&mut out, s, style, at + RUNTIME, 5, 1.0);
                    }
                }
            }
        }
        if let Some(c) = hud.court {
            draw_court(&mut out, &setup, &disc, c, &mut ink);
        }
        if let Some(d) = hud.clock {
            let style = TextStyle {
                scale: 2.0,
                outline: Some((data::colour::CLOCK_OUTLINE, Vec2::ONE)),
                ..TextStyle::plain(data::font::CLOCK, data::colour::AMBER)
            };
            let (at, align) = data::at::SHOT_CLOCK;
            ink.text(&mut out, &d.to_string(), style, at + shift, align, 1.0);
        }
        if hud.violation > 0 {
            let style = TextStyle {
                outline: Some((data::colour::CLOCK_OUTLINE, Vec2::ONE)),
                ..TextStyle::plain(data::font::CLOCK, data::colour::AMBER)
            };
            let (at, align) = data::at::VIOLATION;
            ink.text(
                &mut out,
                &word(data::words::VIOLATION),
                style,
                at + shift,
                align,
                1.0,
            );
        }
        // The rewrite's own play-by-play, asked for.
        if play_by_play.is_some() && !sim.call.is_empty() && !matches!(w.phase, Phase::Over { .. })
        {
            ink.text(
                &mut out,
                &sim.call,
                TextStyle::shadowed(2, data::colour::SCORE),
                Vec2::new(320.0, 100.0),
                1,
                1.0,
            );
        }
    } else if let Some(c) = hud.court {
        draw_place(&mut out, &setup, &disc, c, &mut ink, hud_art);
        draw_court(&mut out, &setup, &disc, c, &mut ink);
    }

    // The banner, the replay prompt (`FUN_002bafe0`) and the attract line,
    // over everything.
    if let Some(replay) = hud_art.and_then(|a| a.replay.as_ref()) {
        let said = word(data::words::REPLAY);
        hud.banner.draw(&mut out, replay, control.demo, |out, at| {
            // Font 5 at 0.75 (`FUN_0021cdf8`), centred, with the row's
            // stars five either side, at the same scale.
            let style = TextStyle {
                scale: 0.75,
                ..TextStyle::plain(5, crate::banner::REPLAY_COLOUR)
            };
            ink.text(out, &said, style, at, 5, 1.0);
            let width = ink
                .look
                .fonts
                .get(5)
                .map_or(0.0, |f| f.measure(&said) as f32)
                * 0.75;
            let (star, size) = ink.look.row_star();
            let size = size * 0.75;
            for x in [at.x - width / 2.0 - 5.0 - size.x, at.x + width / 2.0 + 5.0] {
                let min = Vec2::new(x, at.y - size.y / 2.0);
                out.push(Piece::new(
                    star.clone(),
                    Rect::from_corners(min, min + size),
                ));
            }
        });
    }
    if tape.as_deref().and_then(|t| t.prompt).is_some() && !control.demo {
        let a = f32::from(hud.prompt) / 255.0;
        let p = data::REWIND;
        let (at, align) = data::at::REWIND;
        sprite(
            &mut out,
            &atlas,
            p,
            (at, align),
            p.size,
            p.uv1.x - p.uv0.x,
            (false, false),
            a,
        );
        if opts.help {
            // The button (`0x3d`) right of the picture, then the word
            // (`0x3e`) five right and five down of the button's corner.
            let mut x = anchor(at, align, p.size).x + p.size.x;
            let mut y = at.y;
            if let Some(pad) = look.pad.as_ref() {
                let icon = crate::diagram::Icon::Square.source();
                let min = Vec2::new(x, at.y - icon.height() / 2.0) + shift;
                let mut piece = Piece::part(
                    pad[0].clone(),
                    icon,
                    Rect::from_corners(min, min + icon.size()),
                );
                piece.colour = Color::srgba(1.0, 1.0, 1.0, a);
                out.push(piece);
                x += icon.width();
                y = at.y - icon.height() / 2.0 + icon.height();
            }
            let style = TextStyle {
                scale: 0.9,
                ..TextStyle::plain(data::font::REPLAY_WORD, data::colour::REPLAY_WORD)
            };
            ink.text(
                &mut out,
                &word(data::words::PROMPT),
                style,
                Vec2::new(x + 5.0, y + 5.0) + shift,
                4,
                a,
            );
        }
    }
    // "Press START to exit demo": not while the intro's scripts play (the
    // runtime's demos showed it from the tip-off, and through the replays).
    if control.demo && hud.intro_over {
        let style = TextStyle {
            shadow: Some((data::colour::BLACK, Vec2::new(2.0, 2.0), 1.0)),
            ..TextStyle::plain(data::font::DEMO, data::colour::BOTTOM)
        };
        let (at, align) = data::at::BOTTOM_LINE;
        ink.text(
            &mut out,
            &word(data::words::DEMO),
            style,
            at + shift,
            align,
            f32::from(hud.attract) / 255.0,
        );
    }

    look::show(&mut commands, &mut slots, &out, Screen::Playing);
}

/// The HUD atlas's pictures and the logos, back to front (the layout lists
/// them front to back and the walker builds them from the last).
#[allow(clippy::too_many_arguments)]
fn pictures(
    out: &mut Vec<Piece>,
    (atlas, people): (&Handle<Image>, &[Handle<Image>]),
    hud: &HudState,
    w: &street_sim::World,
    control: &Control,
    setup: &MatchSetup,
    disc: &Disc,
    hud_art: Option<&HudArt>,
) {
    let shift = ELEMENT;
    for side in 0..2 {
        // The left side's pieces are mirrored.
        let mirror = side == 0;
        whole(
            out,
            atlas,
            data::LOGO_FRAME,
            data::at::LOGO_FRAME[side],
            (mirror, false),
        );
        whole(
            out,
            atlas,
            data::PANEL,
            data::at::PANEL[side],
            (mirror, false),
        );
    }
    for side in 0..2 {
        let m = &hud.meters[side];
        let team = usize::from(side == 0);
        let at = data::at::METER[side];
        // The meter's bars: the right side's mirrored, right-anchored.
        let mirror = side == 1;
        let bar = |out: &mut Vec<Piece>, p: data::Part, width: f32, alpha: f32| {
            let width = width.clamp(0.0, data::METER_WIDTH);
            let span = width / data::METER_WIDTH * (p.uv1.x - p.uv0.x);
            sprite(
                out,
                atlas,
                p,
                at,
                Vec2::new(width, p.size.y),
                span,
                (mirror, false),
                alpha,
            );
        };
        if m.pocketed {
            bar(
                out,
                data::METER_POCKETED,
                m.drain.unwrap_or(data::METER_WIDTH),
                1.0,
            );
        } else {
            bar(
                out,
                data::METER_TARGET,
                target_width(w.teams[team].meter),
                1.0,
            );
        }
        let fill = if m.pocketed {
            data::METER_POCKET_FILL
        } else {
            data::METER_FILL
        };
        bar(out, fill, m.fill, 1.0);
        if m.full {
            bar(
                out,
                data::METER_FULL,
                data::METER_WIDTH,
                f32::from(m.blink) / 255.0,
            );
        }
    }
    for side in 0..2 {
        whole(
            out,
            atlas,
            data::SCORE_FRAME,
            data::at::SCORE_FRAME[side],
            (side == 0, false),
        );
    }
    // Turbo: each person's bar, the frames over them, the logos, the
    // slashes, then the TURBO words.
    let slots = turbo_slots(w, control);
    let mut words = Vec::new();
    for side in 0..2 {
        let align = if side == 0 { 0 } else { 2 };
        for (k, &(pad, player)) in slots[side].iter().enumerate() {
            let turbo = w
                .players
                .get(player)
                .map_or(0.0, |p| p.turbo.clamp(0.0, 1.0));
            let width = turbo * data::TURBO_WIDTH;
            let at = (turbo_at(side, k), align);
            let (colours, base) = turbo_art(atlas, people, pad);
            let bar = data::TURBO_BAR[base];
            sprite(
                out,
                colours,
                bar,
                at,
                Vec2::new(width, bar.size.y),
                width,
                (side == 1, false),
                1.0,
            );
            // The word is clipped from its far end and never mirrored; the
            // second person's reads two texels over (`FUN_002ad190`).
            let mut part = data::TURBO_WORD[base];
            let nudge = match (side, k) {
                (0, 1 | 2) => -2.0,
                (1, 1 | 2) => 2.0,
                _ => 0.0,
            };
            part.uv0.x += nudge;
            part.uv1.x += nudge;
            if side == 1 {
                part.uv0.x = part.uv1.x - width;
            }
            words.push((colours, part, at, width));
        }
        // The frames: the first upside down on the left and turned half
        // round on the right, the second mirrored on the left.
        let turns = [
            [(false, true), (true, false)],
            [(true, true), (false, false)],
        ];
        for k in 0..slots[side].len().max(1) {
            let (at, align) = data::at::TURBO_FRAME[side][k.min(1)];
            let drop = if k == 2 {
                data::at::TURBO_THIRD_DROP
            } else {
                0.0
            };
            whole(
                out,
                atlas,
                data::TURBO_FRAME,
                (at + Vec2::new(0.0, drop), align),
                turns[side][k.min(1)],
            );
        }
    }
    for side in 0..2 {
        let team = usize::from(side == 0);
        let logo = disc
            .teams
            .get(setup.teams[team])
            .and_then(|t| usize::try_from(t.logo).ok())
            .and_then(|l| hud_art?.logos.get(l)?[side].clone());
        if let Some((image, size)) = logo {
            let (at, align) = data::at::LOGO[side];
            let min = anchor(at, align, size) + shift;
            out.push(Piece::new(image, Rect::from_corners(min, min + size)));
        }
    }
    for side in 0..2 {
        whole(
            out,
            atlas,
            data::SLASH,
            data::at::SLASH[side],
            (side == 0, false),
        );
    }
    for (colours, part, at, width) in words {
        sprite(
            out,
            colours,
            part,
            at,
            Vec2::new(width, part.size.y),
            width,
            (false, false),
            1.0,
        );
    }
    for side in 0..2 {
        whole(
            out,
            atlas,
            data::GAMEBREAKER,
            data::at::GAMEBREAKER[side],
            (false, false),
        );
        let m = &hud.meters[side];
        if m.lit {
            let p = data::GAMEBREAKER_LIT;
            let a = if m.full {
                f32::from(m.blink) / 255.0
            } else {
                1.0
            };
            sprite(
                out,
                atlas,
                p,
                data::at::GAMEBREAKER[side],
                m.lit_size,
                p.uv1.x - p.uv0.x,
                (false, false),
                a,
            );
        }
    }
}

/// The left side's panel as Street School's tutorial pages draw it
/// (`FUN_00251f50`, `FUN_00253258`, `FUN_00255050` cut the same HUD
/// sheet's parts with `FUN_002513e0`): the logo's frame and the panel,
/// the meter at `meter` of its width, the score frame, the first pad's
/// turbo bar full, its frame, the team's logo, the slash, TURBO, the
/// GAMEBREAKER label and the score, all moved by `offset` from the HUD's
/// own points.
#[allow(clippy::too_many_arguments)]
pub(crate) fn page_panel(
    out: &mut Vec<Piece>,
    ink: &mut Writer,
    atlas: &Handle<Image>,
    logo: Option<(Handle<Image>, Vec2)>,
    offset: Vec2,
    meter: f32,
    score: &str,
) {
    let side = 0;
    let at = |(p, align): (Vec2, u8)| (p + offset, align);
    let first = out.len();
    whole(
        out,
        atlas,
        data::LOGO_FRAME,
        at(data::at::LOGO_FRAME[side]),
        (true, false),
    );
    whole(
        out,
        atlas,
        data::PANEL,
        at(data::at::PANEL[side]),
        (true, false),
    );
    let width = (meter.clamp(0.0, 1.0) * data::METER_WIDTH).max(0.0);
    let fill = data::METER_FILL;
    sprite(
        out,
        atlas,
        fill,
        at(data::at::METER[side]),
        Vec2::new(width, fill.size.y),
        width / data::METER_WIDTH * (fill.uv1.x - fill.uv0.x),
        (false, false),
        1.0,
    );
    whole(
        out,
        atlas,
        data::SCORE_FRAME,
        at(data::at::SCORE_FRAME[side]),
        (true, false),
    );
    let turbo = (Vec2::new(data::at::TURBO[side][0], data::at::TURBO_Y), 0);
    let bar = data::TURBO_BAR[0];
    sprite(
        out,
        atlas,
        bar,
        at(turbo),
        Vec2::new(data::TURBO_WIDTH, bar.size.y),
        data::TURBO_WIDTH,
        (false, false),
        1.0,
    );
    whole(
        out,
        atlas,
        data::TURBO_FRAME,
        at(data::at::TURBO_FRAME[side][0]),
        (false, true),
    );
    if let Some((image, size)) = logo {
        let (p, align) = data::at::LOGO[side];
        let min = anchor(p + offset, align, size) + ELEMENT;
        out.push(Piece::new(image, Rect::from_corners(min, min + size)));
    }
    whole(
        out,
        atlas,
        data::SLASH,
        at(data::at::SLASH[side]),
        (true, false),
    );
    let word = data::TURBO_WORD[0];
    sprite(
        out,
        atlas,
        word,
        at(turbo),
        Vec2::new(data::TURBO_WIDTH, word.size.y),
        data::TURBO_WIDTH,
        (false, false),
        1.0,
    );
    whole(
        out,
        atlas,
        data::GAMEBREAKER,
        at(data::at::GAMEBREAKER[side]),
        (false, false),
    );
    // The HUD's pieces sit the match's 16 up (`ELEMENT`); the pages' are
    // front-end points.
    for p in &mut out[first..] {
        p.rect.min -= ELEMENT;
        p.rect.max -= ELEMENT;
    }
    let (p, align) = data::at::SCORE[side];
    ink.text(
        out,
        score,
        TextStyle::plain(data::font::SCORE, data::colour::SCORE),
        p + offset,
        align,
        1.0,
    );
}

/// The court's name (`0x10`, font 6, navy outline 2) and place (`0x11`,
/// font 4, outline 1; English only), fading in over fps / 3
/// (`FUN_002b1d48`).
fn draw_court(
    out: &mut Vec<Piece>,
    setup: &MatchSetup,
    disc: &Disc,
    counter: i32,
    ink: &mut Writer,
) {
    let Some(court) = disc
        .courts
        .iter()
        .find(|c| c.id.eq_ignore_ascii_case(&setup.court))
    else {
        return;
    };
    let a = (counter as f32 / (FPS as f32 / 3.0)).clamp(0.0, 1.0);
    let name = TextStyle {
        outline: Some((data::colour::COURT_OUTLINE, Vec2::splat(2.0))),
        ..TextStyle::plain(data::font::COURT, data::colour::AMBER)
    };
    let (at, align) = data::at::COURT;
    ink.text(out, &court.name, name, at + RUNTIME, align, a);
    let place = TextStyle {
        outline: Some((data::colour::COURT_OUTLINE, Vec2::ONE)),
        ..TextStyle::plain(data::font::PLACE, data::colour::AMBER)
    };
    let (at, align) = data::at::PLACE;
    ink.text(out, &court.location, place, at + RUNTIME, align, a);
}

/// The two white stars beside the court's place (`0x12`, `0x13`: `XACOMMON`
/// sheet 0 image 5 frame 2, 4 left of the place's left edge and 4 right
/// of its right, 28 below its point, `FUN_002b1ef8`, anchored by their
/// layout alignments 10 and 8: right-bottom, left-bottom).
fn draw_place(
    out: &mut Vec<Piece>,
    setup: &MatchSetup,
    disc: &Disc,
    counter: i32,
    ink: &mut Writer,
    art: Option<&HudArt>,
) {
    let Some((star, size)) = art.and_then(|a| a.star.clone()) else {
        return;
    };
    let Some(court) = disc
        .courts
        .iter()
        .find(|c| c.id.eq_ignore_ascii_case(&setup.court))
    else {
        return;
    };
    let Some(font) = ink.look.fonts.get(data::font::PLACE) else {
        return;
    };
    let (at, _) = data::at::PLACE;
    let half = font.measure(&court.location) as f32 / 2.0;
    let a = (counter as f32 / (FPS as f32 / 3.0)).clamp(0.0, 1.0);
    for (x, align) in [(at.x - half - 4.0, 10u8), (at.x + half + 4.0, 8)] {
        let min = anchor(Vec2::new(x, at.y + 28.0), align, size) + RUNTIME;
        let mut p = Piece::new(star.clone(), Rect::from_corners(min, min + size));
        p.colour = Color::srgba(1.0, 1.0, 1.0, a);
        out.push(p);
    }
}

/// Track Info (`FUN_00213150`) or the Opposition (`FUN_00213858`) in the
/// box `XACOMMON` sheet 0 image 5 frame 1, stretched 40 wider than the
/// text's wrap (Track Info: the box's width × 1.4 − 30; the Opposition:
/// its width × 1.2 − 40, or its widest line) and 12 taller than the text;
/// the text 13 in and 6 down from its top-left. Track Info's three lines
/// are one text (a line height apart) with "TRAX" 4 below; the
/// Opposition's are four texts, each 4 below the last.
fn draw_panel(out: &mut Vec<Piece>, p: &Panel, ink: &mut Writer, art: Option<&HudArt>) {
    let Some(font) = ink.look.fonts.get(data::font::PANEL) else {
        return;
    };
    let a = f32::from(p.alpha()) / 255.0;
    let box_size = art
        .and_then(|a| a.panel_box.as_ref())
        .map_or(Vec2::new(183.0, 98.0), |(_, s)| *s);
    let mut blocks: Vec<(String, [u8; 4])> = Vec::new();
    if p.opposition {
        blocks.extend(p.lines.iter().cloned());
    } else {
        let label = p
            .lines
            .last()
            .filter(|(_, c)| *c == data::colour::PANEL_LABEL)
            .cloned();
        let body: Vec<String> = p
            .lines
            .iter()
            .filter(|(_, c)| *c != data::colour::PANEL_LABEL)
            .map(|(s, _)| s.clone())
            .collect();
        blocks.push((body.join("\n"), data::colour::PANEL_TEXT));
        blocks.extend(label);
    }
    let widest = blocks
        .iter()
        .flat_map(|(s, _)| s.split('\n'))
        .map(|l| font.measure(l) as f32)
        .fold(0.0f32, f32::max);
    let wrap = if p.opposition {
        (box_size.x * 1.2 - 40.0).max(widest)
    } else {
        box_size.x * 1.4 - 30.0
    };
    let heights: Vec<f32> = blocks
        .iter()
        .map(|(s, _)| ink.height(data::font::PANEL, s, 1.0))
        .collect();
    let text = heights.iter().sum::<f32>() + 4.0 * heights.len() as f32;
    let size = Vec2::new(
        wrap + 40.0,
        text + 12.0 - 4.0 + if p.opposition { 12.0 } else { 4.0 },
    );
    let corner = RUNTIME
        + if p.right {
            data::at::PANEL_RIGHT - size
        } else {
            Vec2::new(data::at::PANEL_LEFT.x, data::at::PANEL_LEFT.y - size.y)
        };
    if let Some((image, _)) = art.and_then(|a| a.panel_box.clone()) {
        let mut piece = Piece::new(image, Rect::from_corners(corner, corner + size));
        piece.colour = Color::srgba(1.0, 1.0, 1.0, a);
        out.push(piece);
    }
    let mut y = corner.y + 6.0;
    for ((s, colour), h) in blocks.iter().zip(&heights) {
        ink.text(
            out,
            s,
            TextStyle::plain(data::font::PANEL, *colour),
            Vec2::new(corner.x + 13.0, y),
            0,
            a,
        );
        y += h + 4.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn six_players() -> street_sim::World {
        let players = (0..6)
            .map(|k| street_sim::Player::new(k / 3, [0.0; 2], 0.0))
            .collect();
        street_sim::World::new(
            street_sim::Hoop::native(),
            players,
            street_sim::Rules::default(),
            1,
        )
    }

    fn local(number: usize, team: usize, player: usize) -> crate::controls::Seat {
        crate::controls::Seat {
            number,
            device: None,
            team,
            player,
            pad: Default::default(),
            raw: Default::default(),
            edges: Default::default(),
        }
    }

    #[test]
    fn three_on_three_online_shows_every_persons_bar_in_their_colour() {
        let w = six_players();
        // This machine plays 3P (seat 2, home); the others are elsewhere.
        let remote = [(0, 0, 0), (1, 1, 3), (3, 1, 4), (4, 0, 1), (5, 1, 5)]
            .map(|(number, team, player)| crate::controls::Remote {
                number,
                team,
                player,
            })
            .to_vec();
        let control = Control {
            seats: vec![local(2, 0, 2)],
            demo: false,
            remote,
        };
        let [left, right] = turbo_slots(&w, &control);
        // Away (team 1) on the left: 2P, 4P, 6P; home on the right: 1P,
        // 3P, 5P, each on their own player.
        assert_eq!(left, vec![(1, 3), (3, 4), (5, 5)]);
        assert_eq!(right, vec![(0, 0), (2, 2), (4, 1)]);
        // 5P's and 6P's bars come from the turned atlases, off 3P's and
        // 1P's.
        let atlas = Handle::<Image>::default();
        let people = [Handle::<Image>::default(), Handle::<Image>::default()];
        assert_eq!(turbo_art(&atlas, &people, 4).1, 2);
        assert_eq!(turbo_art(&atlas, &people, 5).1, 0);
        assert_eq!(turbo_art(&atlas, &people, 1).1, 1);
        assert_eq!(
            turbo_at(0, 2).y,
            data::at::TURBO_Y + data::at::TURBO_THIRD_DROP
        );
    }

    #[test]
    fn offline_a_side_shows_two_bars_and_the_computer_side_the_first_free_pad() {
        let w = six_players();
        let control = Control {
            seats: vec![local(0, 1, 3), local(1, 1, 4), local(2, 1, 5)],
            demo: false,
            remote: Vec::new(),
        };
        let [left, right] = turbo_slots(&w, &control);
        assert_eq!(left, vec![(0, 3), (1, 4)]);
        // 3P's bar has no place (two a side), so the computer's side takes
        // its pad, the first free from 2.
        assert_eq!(right, vec![(2, 0)]);
    }

    #[test]
    fn generic_pickups_never_overwrite_blocks_or_steals_with_rebound_points() {
        let w = street_sim::World::new(
            street_sim::Hoop::native(),
            vec![street_sim::Player::new(0, [0.0; 2], 0.0)],
            street_sim::Rules::default(),
            1,
        );
        let word = |id| format!("{id:#x}");
        let pickup = Event::Pickup { player: 0 };
        let mut hud = HudState::default();
        on_event(&mut hud, &w, &pickup, None, &word, false);
        assert!(hud.popups.iter().flatten().all(|p| p.word_timer == 0));
        for (event, kind) in [
            (
                Event::Block {
                    player: 0,
                    shooter: 0,
                },
                kind::BLOCK,
            ),
            (
                Event::Steal {
                    player: 0,
                    from: 0,
                    intercepted_pass: false,
                },
                kind::STEAL,
            ),
            (Event::Rebound { player: 0 }, kind::REBOUND),
        ] {
            let mut hud = HudState::default();
            on_event(&mut hud, &w, &event, None, &word, false);
            on_event(&mut hud, &w, &pickup, None, &word, false);
            let shown: Vec<_> = hud
                .popups
                .iter()
                .flatten()
                .filter(|p| p.word_timer != 0)
                .collect();
            assert_eq!(shown.len(), 1);
            assert_eq!(shown[0].word, word(data::KIND_WORD[kind]));
            assert_eq!(shown[0].points, data::KIND_POINTS[kind].to_string());
        }
    }

    #[test]
    fn a_staged_level_2s_banner_leaves_at_its_tasks_end() {
        // Slot 64 put GB2 up and hid the HUD; the staged script's task
        // (kind 0, the simulation's) holds them through the script and its
        // replay; its end (slot 65) takes them down.
        let mut hud = HudState {
            hidden: true,
            ..Default::default()
        };
        hud.banner.show(crate::banner::Mode::Gamebreaker2);
        let mut w = street_sim::World::new(
            [street_sim::Hoop {
                rim: [-37.0, 11.75, 0.0],
                rim_radius: 0.75,
            }; 2],
            Vec::new(),
            street_sim::Rules::default(),
            1,
        );
        w.gamebreaker_task = Some(street_sim::street::GamebreakerTask {
            kind: 0,
            state: 1,
            count: 0,
        });
        staged_task_end(&mut hud, &w, false);
        assert!(hud.hidden && hud.banner.mode == crate::banner::Mode::Gamebreaker2);
        // Its script ends with the game update before the rerun; the post
        // waits for the rerun to end.
        w.gamebreaker_task = None;
        staged_task_end(&mut hud, &w, true);
        assert!(hud.hidden && hud.banner.mode == crate::banner::Mode::Gamebreaker2);
        staged_task_end(&mut hud, &w, false);
        assert!(!hud.hidden && hud.banner.mode == crate::banner::Mode::Out);
        // The shot's task (kind 1) ends through the replays' task instead.
        let mut hud = HudState::default();
        hud.banner.show(crate::banner::Mode::Gamebreaker1);
        w.gamebreaker_task = Some(street_sim::street::GamebreakerTask {
            kind: 1,
            state: 1,
            count: 30,
        });
        staged_task_end(&mut hud, &w, false);
        w.gamebreaker_task = None;
        staged_task_end(&mut hud, &w, false);
        assert_eq!(hud.banner.mode, crate::banner::Mode::Gamebreaker1);
    }

    #[test]
    fn moves_past_the_dunk_table_pop_the_originals_names() {
        // A basket's pop-up names the shooter's dunk id (+0x1e8) or lay-up
        // kind (+0x26c) (`FUN_002b87c0`). Past `MOVE_ROWS`
        // (`timing::move_row`) the leaps carry dunk id 24, the board's
        // lay-up kind 5 and the mid-air lay-ups kind 0 (`FUN_0016b1e8`
        // writes its entry's word 8); the putback has no row and pops the
        // tip slam (`FUN_002e2568`: +0x1e4 1).
        use street_sim::{Player, Rules, World, player::DunkStyle};
        let names = street_assets::overlays::MoveNames {
            dunks: (0..60).map(|k| format!("dunk {k}")).collect(),
            layups: (0..28).map(|k| format!("layup {k}")).collect(),
            ..Default::default()
        };
        let word = |id: u16| format!("{id:#x}");
        let layup = |row| Action::Layup {
            hoop: 0,
            elapsed: 0.0,
            from: [0.0; 2],
            to: [0.0; 2],
            tick: 1,
            release: 20,
            end: 30,
            row,
        };
        let dunk = |row, style| Action::Dunking {
            hoop: 0,
            elapsed: 0.0,
            from: [0.0; 2],
            to: [0.0; 2],
            style,
            layup: false,
            tick: 1,
            release: 20,
            end: 30,
            row,
            hang: None,
        };
        let score = |hud: &mut HudState, w: &World, basket| {
            hud.popups = Default::default();
            let event = Event::Score {
                team: 0,
                player: 0,
                points: 2,
                basket,
            };
            on_event(hud, w, &event, Some(&names), &word, false);
            let p = &hud.popups[screen_side(0)][0];
            (p.word.clone(), p.points.clone())
        };
        let mut w = World::new(
            street_sim::Hoop::native(),
            vec![
                Player::new(0, [-10.0, 0.0], 0.0),
                Player::new(1, [10.0, 0.0], 0.0),
            ],
            Rules::default(),
            1,
        );
        let mut hud = HudState::default();
        for k in 0..10 {
            w.players[0].action = layup(timing_tables::MIDAIR_ROWS + k);
            hud.note_moves(&w);
            let shown = score(&mut hud, &w, Basket::Layup);
            assert_eq!(shown, ("layup 0".into(), "1000".into()), "mid-air {k}");
        }
        w.players[0].action = layup(timing_tables::BOARD_LAYUP_ROW);
        hud.note_moves(&w);
        assert_eq!(score(&mut hud, &w, Basket::Layup).0, "layup 5");
        let leaps = timing_tables::MOVE_ROWS.len() as u8;
        for row in [leaps, leaps + 1] {
            w.players[0].action = dunk(row, DunkStyle::Normal);
            hud.note_moves(&w);
            assert_eq!(hud.last_dunk[0], 24, "leap row {row}");
        }
        let row = timing_tables::DUNK_FALLBACK_ROW;
        w.players[0].action = dunk(row as u8, DunkStyle::Normal);
        hud.note_moves(&w);
        let id = timing_tables::MOVE_ROWS[row].id;
        let dunked = score(&mut hud, &w, Basket::Dunk { advanced: false });
        assert_eq!(dunked.0, format!("dunk {id}"));
        // The putback keeps the last dunk's id but pops the tip slam.
        w.players[0].action = dunk(timing_tables::NO_MOVE_ROW, DunkStyle::Putback);
        hud.note_moves(&w);
        assert_eq!(hud.last_dunk[0], usize::from(id));
        assert_eq!(
            score(&mut hud, &w, Basket::Dunk { advanced: false }),
            (word(data::KIND_WORD[kind::TIP_SLAM]), "2000".into())
        );
        // An advanced alley-oop's finish pops the finisher's dunk name.
        w.players[0].action = dunk(row as u8, DunkStyle::Normal);
        hud.note_moves(&w);
        let oop = score(&mut hud, &w, Basket::AlleyOop { advanced: true });
        assert_eq!(oop.0, format!("dunk {id}"));
        // Winning the game with a dunk pops the busted rim; a jumper keeps
        // its own; the trick points game never busts it.
        w.phase = Phase::Over { winner: 0 };
        let busted = (
            word(data::KIND_WORD[kind::BUSTED_RIM]),
            data::KIND_POINTS[kind::BUSTED_RIM].to_string(),
        );
        assert_eq!(
            score(&mut hud, &w, Basket::Dunk { advanced: false }),
            busted
        );
        assert_eq!(
            score(&mut hud, &w, Basket::AlleyOop { advanced: false }),
            busted
        );
        assert_eq!(
            score(&mut hud, &w, Basket::Jumper { long: false }).0,
            word(data::KIND_WORD[kind::SHORT_RANGE])
        );
        w.rules.goal = Goal::TrickPoints(100_000);
        assert_eq!(
            score(&mut hud, &w, Basket::Dunk { advanced: false }).0,
            format!("dunk {id}")
        );
    }

    #[test]
    fn a_popup_flies_in_holds_and_leaves_for_its_corner() {
        // FUN_002b4c38 with T = 90, A = 15: from the side at three times
        // its size to the middle by T - A, held, then to the corner.
        let (at, scale) = popup_place(0, WORD_TIME, WORD_TIME, false);
        assert_eq!((at, scale), (data::popup::WORD_START[0], 3.0));
        let (at, scale) = popup_place(0, WORD_TIME - MOVE_TIME, WORD_TIME, false);
        assert_eq!((at, scale), (data::popup::WORD_MIDDLE, 1.0));
        let (at, scale) = popup_place(1, 40, WORD_TIME, false);
        assert_eq!((at, scale), (data::popup::WORD_MIDDLE, 1.0));
        let (at, scale) = popup_place(1, 0, WORD_TIME, false);
        assert_eq!((at, scale), (data::popup::WORD_CORNER[1], 0.0));
        let (at, _) = popup_place(0, MOVE_TIME, POINTS_TIME, true);
        assert_eq!(at, data::popup::POINTS_MIDDLE);
    }

    #[test]
    fn a_new_popup_hurries_the_older_one_off() {
        let mut hud = HudState::default();
        hud.popup(0, "dunk".into(), 1_500, false);
        for _ in 0..5 {
            hud.step_popups();
        }
        hud.popup(1, "block".into(), 2_500, false);
        // One stage a frame: first to where it has landed, then to its exit.
        hud.step_popups();
        let old = &hud.popups[screen_side(0)][0];
        assert_eq!(old.word_timer, WORD_TIME - MOVE_TIME - 1, "landed at once");
        hud.step_popups();
        let old = &hud.popups[screen_side(0)][0];
        assert_eq!(old.word_timer, MOVE_TIME - 1, "then to its exit");
        assert_eq!(hud.popups[screen_side(1)][0].word_timer, WORD_TIME - 2);
    }

    #[test]
    fn the_points_start_as_the_word_arrives() {
        let mut hud = HudState::default();
        hud.popup(0, "steal".into(), 1_000, false);
        for _ in 0..MOVE_TIME {
            hud.step_popups();
            assert_eq!(hud.popups[1][0].points_timer, 0);
        }
        hud.step_popups();
        assert_eq!(hud.popups[1][0].points_timer, POINTS_TIME);
    }

    #[test]
    fn the_combo_line_reads_the_walked_slots() {
        let word = |id: u16| format!("{id:x}");
        // trick, dish, dunk: three walked slots, one line.
        let line = combo_line(&[-1, -1, -1, 10, 8, 0], word).unwrap();
        assert_eq!(line, "5918 + 5917 + 5910 590f");
        // Four: slot 4 is the third word, so the break comes before it.
        let line = combo_line(&[-1, -1, 10, 10, 8, 0], word).unwrap();
        assert_eq!(line, "5918 + 5918 + \n5917 + 5910 590f");
        // Six walked: " super"; a block on top: " defense".
        let line = combo_line(&[10, 10, 10, 10, 10, 19], word).unwrap();
        assert!(line.ends_with("591e 590c 590e 590f"), "{line}");
        assert_eq!(combo_line(&[-1; 6], word), None);
    }

    #[test]
    fn the_meter_fill_chases_its_target() {
        assert_eq!(chase(10.0, 20.0), 10.25);
        assert_eq!(chase(10.0, 10.2), 10.2);
        assert_eq!(chase(20.0, 10.0), 19.0);
        assert_eq!(target_width(25_000), 83.5);
    }

    #[test]
    fn the_shot_clock_shows_its_last_eight_seconds() {
        assert_eq!(clock_value(7.9), 8);
        assert_eq!(clock_value(8.0), 9);
        assert_eq!(clock_value(0.4), 1);
    }

    #[test]
    fn the_lit_word_pulses_back_to_its_size() {
        let mut size = data::GAMEBREAKER_LIT.size;
        for counter in (0..=70).rev() {
            size = pulse(size, counter);
        }
        assert_eq!(size, data::GAMEBREAKER_LIT.size);
    }

    #[test]
    fn side_one_is_drawn_on_the_left() {
        assert_eq!(screen_side(1), 0);
        assert_eq!(screen_side(0), 1);
    }

    #[test]
    fn the_layout_holds_the_traced_points() {
        // XLAY_GM blob 1 (class 2): the points the HUD draws at.
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let layouts = street_assets::font::layouts(&mut iso, "/FE/XLAY_GM.BIN").unwrap();
        let hud = layouts.iter().find(|l| l.class == 2).unwrap();
        let at = |id: u32| {
            let e = hud
                .elements
                .iter()
                .find(|e| e.id == id && e.kind != 10 && e.kind != 0)
                .unwrap();
            let (x, y) = e.position().unwrap();
            ((Vec2::new(f32::from(x), f32::from(y))), e.align)
        };
        use data::at;
        assert_eq!(at(0x1a), at::PANEL[0]);
        assert_eq!(at(0x1d), at::PANEL[1]);
        assert_eq!(at(0x1b), at::SCORE_FRAME[0]);
        assert_eq!(at(0x1e), at::SCORE_FRAME[1]);
        assert_eq!(at(0x1c), at::LOGO_FRAME[0]);
        assert_eq!(at(0x1f), at::LOGO_FRAME[1]);
        assert_eq!(at(0x34), at::METER[0]);
        assert_eq!(at(0x35), at::METER[1]);
        assert_eq!(at(0x16), at::GAMEBREAKER[0]);
        assert_eq!(at(0x17), at::GAMEBREAKER[1]);
        assert_eq!(at(0x2a), at::SLASH[0]);
        assert_eq!(at(0x2b), at::SLASH[1]);
        assert_eq!(at(0x26), at::TURBO_FRAME[0][0]);
        assert_eq!(at(0x27), at::TURBO_FRAME[0][1]);
        assert_eq!(at(0x28), at::TURBO_FRAME[1][0]);
        assert_eq!(at(0x29), at::TURBO_FRAME[1][1]);
        assert_eq!(at(0), at::SCORE[0]);
        assert_eq!(at(1), at::SCORE[1]);
        assert_eq!(at(0xe), at::TRICK_POINTS[0]);
        assert_eq!(at(0xf), at::TRICK_POINTS[1]);
        assert_eq!(at(2), at::SHOT_CLOCK);
        assert_eq!(at(3), at::VIOLATION);
        assert_eq!(at(0xc), at::BOTTOM_LINE);
        assert_eq!(at(0xd), at::GAME_POINT);
        assert_eq!(at(0x3c), at::REWIND);
        // The fonts and texts the layout gives the scores, the clock, the
        // violation and the game-point line.
        let e = |id: u32| {
            hud.elements
                .iter()
                .find(|e| e.id == id && e.kind == 4)
                .unwrap()
        };
        assert_eq!(e(2).props.get(&13), Some(&6));
        assert_eq!(
            e(3).props.get(&20),
            Some(&u32::from(data::words::VIOLATION))
        );
        assert_eq!(
            e(0xd).props.get(&20),
            Some(&u32::from(data::words::SCORE_NOW))
        );
        assert_eq!(e(0xe).props.get(&13), Some(&1));
        assert_eq!(e(4).props.get(&13), Some(&5));
    }

    #[test]
    fn the_names_come_from_the_executable() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let names = street_assets::overlays::MoveNames::read(&mut iso).unwrap();
        assert_eq!(names.tricks.len(), 75);
        assert_eq!(names.dunks.len(), 60);
        assert_eq!(names.layups.len(), 28);
        // The lay-up rows' ids name them: Dr. J's row is 23, Barry's 24.
        assert_eq!(
            timing_tables::MOVE_ROWS
                .iter()
                .find(|r| r.name == "layup_barry")
                .unwrap()
                .id,
            24
        );
        assert!(names.layups[24].eq_ignore_ascii_case("barry"));
    }
}
