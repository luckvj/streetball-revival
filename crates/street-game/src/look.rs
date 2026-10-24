//! The front end drawn the game's way: its own fonts (`/FE/XFONTSEN.BIN`),
//! its own strings (`/LOCALE/XLOC_FE.EUS`), its star bullets
//! (`/FE/XACOMMON.BIN`), and each screen's title, entries and help line
//! where the screen's layout (`/FE/XLAY_FE.BIN`) puts them.
//!
//! Everything is placed on the PS2's 640x480 screen and scales with the
//! window, as the backgrounds do. A page is described as a [`View`]; [`build`]
//! turns it into [`Piece`]s (images on the screen) and [`show`] keeps one UI
//! node per piece. Text is drawn once into an image and cached.
//!
//! Main menu (layout 0): the title "Main Menu" at (60, 42) in font 6, cream
//! (0xdcdcbb) over a dark red to orange shadow (props 23/24); entries at
//! x 372, y 143 + ~42 per entry in font 4; the star bullets (navigation
//! nodes, kind 10) at x 312 beside each; the help line at (58, 420) in font
//! 0, amber (0xfbac00). Play It (layout 2) is the same with entries at x
//! 351, ~63 apart, and a navy shadow.

use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    ecs::query::QueryFilter,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use street_assets::{
    fe,
    font::{self, Font, Layout},
    locale,
};

use crate::setup::Disc;

/// The locale files a text id's high byte picks (`file << 8 | index`).
use street_assets::locale::FE_TEXT_FILES as TEXT_FILES;

/// The fonts by role (indices into `XFONTSEN`).
pub mod fonts {
    /// Help lines and small print.
    pub const SMALL: usize = 0;
    pub const LIST_SMALL: usize = 1;
    /// Option rows (the Options screen's).
    pub const LIST: usize = 2;
    /// Menu entries (Main, Play It).
    pub const ENTRY: usize = 4;
    pub const TITLE_SMALL: usize = 5;
    /// Screen titles, the script face.
    pub const TITLE: usize = 6;
}

/// Colours from the layouts, RGBA.
pub mod colours {
    /// Entries and titles (prop 10 of layout 0's text).
    pub const CREAM: [u8; 4] = [0xdc, 0xdc, 0xbb, 0xff];
    /// Titles on the cool screens (layout 2, 13).
    pub const PALE: [u8; 4] = [0xf6, 0xf4, 0xd7, 0xff];
    /// The help line (kind 5's prop 10).
    pub const HELP: [u8; 4] = [0xfb, 0xac, 0x00, 0xff];
    /// The main menu's title shadow (props 23 and 24).
    pub const WARM: ([u8; 4], [u8; 4]) = ([0x97, 0x0a, 0x04, 0xff], [0xcd, 0x79, 0x03, 0xff]);
    /// Play It's and Options' title shadow.
    pub const COOL: ([u8; 4], [u8; 4]) = ([0x04, 0x07, 0x35, 0xff], [0x3e, 0x49, 0x9e, 0xff]);
    pub const SELECTED: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
    pub const DIM: [u8; 4] = [0xb0, 0xb0, 0xa8, 0xff];
    /// A screen row's text (kind 19): `FUN_0021c590` gives the row widget
    /// 0xff88a0be for its idle colour (+0xd4) and 0xfffbac00 for its
    /// highlighted one (+0xd8); its layout loader `FUN_0021c750` never reads
    /// the element's prop 10, so the layouts' cream does not reach the text.
    /// The words are A-R-G-B: the text draw `FUN_00238d20` packs bits 16..23
    /// into the GS red byte.
    pub const ROW: [u8; 4] = [0x88, 0xa0, 0xbe, 0xff];
    pub const ROW_LIT: [u8; 4] = [0xfb, 0xac, 0x00, 0xff];
    /// The help line (kind 5): `FUN_0021af18` sets 0xffc99b0d and its
    /// loader `FUN_0021afa8` reads only the text (prop 15), not prop 10.
    pub const HELP_LINE: [u8; 4] = [0xc9, 0x9b, 0x0d, 0xff];
    /// A button legend's labels (kind 34, `FUN_0021a530`), unless the
    /// element's prop 10 is set (`FUN_0021a600`; -1 keeps this).
    pub const LEGEND: [u8; 4] = [0x98, 0xa8, 0xb9, 0xff];
}

/// The disc's front-end look: fonts, layouts, strings and bullets.
#[derive(Resource)]
pub struct Look {
    pub fonts: Vec<Font>,
    /// The executable's own font ([`font::system`]): what a text widget
    /// measures with before it is given one.
    pub system_font: Option<Font>,
    pub layouts: Vec<Layout>,
    texts: HashMap<String, Vec<String>>,
    /// The menu bullets: a blue star beside each entry, a white one by the
    /// highlighted entry (`XACOMMON` sheet 0, image 5).
    star: Handle<Image>,
    star_lit: Handle<Image>,
    star_size: Vec2,
    star_lit_size: Vec2,
    /// The star a highlighted screen row shows either side of its text: the
    /// row widget's two arrow children (`FUN_0021c628`: `XACOMMON` sheet 0,
    /// image 1, frame 0).
    row_star: Handle<Image>,
    row_star_size: Vec2,
    /// The button icons (`XACOMMON` sheet 1, image 0) and their authored
    /// frames, for the screens' button legends (kind 34).
    pub(crate) icons: Option<(Handle<Image>, Vec<Rect>)>,
    /// Legacy skill-meter art (`XACOMMON` sheet 2, image 0): its
    /// frame (175x19) and a tick, unlit and lit (8x10 each).
    slider: Option<[Handle<Image>; 3]>,
    native_slider: Option<NativeSlider>,
    /// The generic menu's three-piece bar (`XACOMMON` sheet 0, image 6,
    /// authored frames 6/7/6; the right end is mirrored).
    segmented_bar: Option<SegmentedBar>,
    /// The button icons and the pad's picture (`XACOMMON` sheet 1, images
    /// 0 and 1), for Controller Layout's diagram.
    pub pad: Option<[Handle<Image>; 2]>,
    /// User Setup's column tab (`XACOMMON` sheet 0, image 2, its 63x18
    /// shape at the corner), white to take each player's colour.
    pub tab: Option<Handle<Image>>,
    /// The pieces User Setup and Choose Team build in code.
    pub prematch: Option<PrematchArt>,
}

/// The sprites the pre-match screens' constructors pick (traced in
/// `docs/FRONTEND_RECOVERY.md`, "User Setup" and "Choose Team"): each with
/// its authored frame's size.
pub struct PrematchArt {
    /// The arrow (kind 37, `FUN_0020f280`'s table at 0x4659f0: `XACOMMON`
    /// sheet 0, image 0, frame 0), and the same frame flipped top to
    /// bottom for the down arrow.
    pub arrow_up: Handle<Image>,
    pub arrow_down: Handle<Image>,
    pub arrow_size: Vec2,
    /// Set Challenge's value arrows: frame 1 of the same image (17x20,
    /// pointing left; `FUN_002c8d08`'s element 0x45f) and, for the right
    /// one, the same frame mirrored (the original turns element 0x460's
    /// frame 0 by 180, `+0xec`; its UV corner set is not decoded, so the
    /// rewrite mirrors the left arrow, which is what the original shows).
    pub arrow_left: Handle<Image>,
    pub arrow_right: Handle<Image>,
    pub arrow_side_size: Vec2,
    /// The controller icon (`XACOMMON` sheet 1, image 2, frame 0).
    pub pad_icon: Handle<Image>,
    pub pad_icon_size: Vec2,
    /// The heading tab (`XACOMMON` sheet 0, image 2, frame 6), the corner
    /// piece (frame 3, drawn turned a quarter) and the line piece (frame
    /// 2, stretched): as decoded (the sheet's own palette, the CPU's) at
    /// 0, then in `XPAL_FE` sheet 0's palettes 0..3, the ones
    /// `FUN_0026f0c0` and `FUN_00243730` swap in for controllers 1P..4P
    /// (`FUN_00210368(w, 0, k)`).
    pub tab_wide: [Handle<Image>; 5],
    pub tab_wide_size: Vec2,
    pub corner: [Handle<Image>; 5],
    pub corner_size: Vec2,
    pub line: [Handle<Image>; 5],
    /// User Setup's column tab (`XACOMMON` sheet 0, image 2, frame 0) in
    /// the same four palettes.
    pub tabs: [Handle<Image>; 4],
    pub tabs_size: Vec2,
    /// The grid cursors (`XAFESCRN` sheet 0, image 0, frames 0 and 1): the
    /// wide one for the double tiles, the other for a team's tile.
    pub cursor_wide: Handle<Image>,
    pub cursor_wide_size: Vec2,
    pub cursor: Handle<Image>,
    pub cursor_size: Vec2,
    /// Choose Court's tile cursor: frame 3 of the same image (80x77;
    /// `FUN_0021b550`'s case 2, which `FUN_00267f10` picks for its tag
    /// 0x15b4).
    pub cursor_court: Handle<Image>,
    pub cursor_court_size: Vec2,
    /// The stars either side of Choose Court's highlighted name (elements
    /// 0x51 and 0x52, `XACOMMON` sheet 0, image 1, frame 2).
    pub name_star: Handle<Image>,
    pub name_star_size: Vec2,
    /// The kind-20 meter (`FUN_00218458`): `XAFESCRN` sheet 1, image 0,
    /// frames 0 and 1 (the frame, the second for a full meter) and 10 and
    /// 11 (the fill, the second for a full meter).
    pub meter: [Handle<Image>; 4],
    pub meter_frame_size: Vec2,
    pub meter_fill_size: Vec2,
    /// Frame 10 in the level palettes: `0x467180` maps levels 1..5 to
    /// `XPAL_FE` sheet 1's palettes 0..4 (`FUN_00210368(fill, 1, k)`).
    pub meter_fill_levels: [Handle<Image>; 5],
    /// The CPU side's badge (`XAFESCRN` sheet 0, image 7, frame 33).
    pub badge: Handle<Image>,
    pub badge_size: Vec2,
    /// A player's portrait tile (`FUN_00222a50`, set up by `FUN_00223698`
    /// with `+0xd0` set, as Choose Players and Update Team build them):
    /// the back (`XAFESCRN` sheet 0, image 4, frame 1, 66x65) in `XPAL_FE`
    /// sheet 2's palettes (`FUN_00210368(back, 2, k)`, `k` by the
    /// portrait), the frame over it (image 5, frame 0, 74x73, the tile's
    /// size), the locked secret legend's picture (image 9, frame 0) and a
    /// pick's check (image 0xe, frame 4).
    pub tile_back: Vec<Handle<Image>>,
    pub tile_back_size: Vec2,
    pub tile_frame: Handle<Image>,
    pub tile_size: Vec2,
    pub tile_mystery: Handle<Image>,
    pub tile_mystery_size: Vec2,
    pub check: Handle<Image>,
    pub check_size: Vec2,
    /// The small tile (`+0xd0` clear, Create Team's grid): the back is
    /// image 4's frame 0 (47x45) and the frame image 6 (55x53).
    pub tile_back_small: Vec<Handle<Image>>,
    pub tile_back_small_size: Vec2,
    pub tile_frame_small: Handle<Image>,
    pub tile_small_size: Vec2,
    /// Choose Region's boss card before the region's boss is beaten
    /// (`XAFESCRN` sheet 0, image 1, frame 5: the question mark on the
    /// star, `FUN_0026c6f8`).
    pub star_card: Handle<Image>,
    pub star_card_size: Vec2,
    /// The "Vol. 2" logo of `/FE/REPLAY.SSH` (its 156x95 at (6, 143) of the
    /// 512x256 atlas), which Choose Region shows while a region is picked
    /// (`FUN_0026b408` loads the sheet).
    pub vol2: Option<(Handle<Image>, Vec2)>,
}

struct NativeSlider {
    atlas: Handle<Image>,
    track: Rect,
    end: Rect,
    middle: Rect,
    /// Frames 2 and 3: the amber end and middle a highlighted row's
    /// slider fills with (the pause screen's).
    lit: Option<(Rect, Rect)>,
}

struct SegmentedBar {
    end: Handle<Image>,
    middle: Handle<Image>,
    end_width: f32,
    height: f32,
}

/// Marks a list column as a slider: [`bar`] makes one.
const BAR: char = '\u{1}';

/// A list column drawn as a continuous native slider at `level / max`.
pub fn bar(level: u8, max: u8) -> String {
    format!("{BAR}{level}/{max}")
}

/// A slider column's level and size.
fn parse_bar(part: &str) -> Option<(u8, u8)> {
    let (level, max) = part.strip_prefix(BAR)?.split_once('/')?;
    Some((level.parse().ok()?, max.parse().ok()?))
}

impl Look {
    /// A look with nothing on it, for tests of the text the screens build.
    #[cfg(test)]
    pub(crate) fn empty() -> Look {
        Look {
            fonts: Vec::new(),
            system_font: None,
            layouts: Vec::new(),
            texts: HashMap::new(),
            star: Handle::default(),
            star_lit: Handle::default(),
            star_size: Vec2::ZERO,
            star_lit_size: Vec2::ZERO,
            row_star: Handle::default(),
            row_star_size: Vec2::ZERO,
            icons: None,
            slider: None,
            native_slider: None,
            segmented_bar: None,
            pad: None,
            tab: None,
            prematch: None,
        }
    }

    /// The row widget's star (`XACOMMON` sheet 0 image 1 frame 0) and its
    /// size.
    pub fn row_star(&self) -> (Handle<Image>, Vec2) {
        (self.row_star.clone(), self.row_star_size)
    }

    /// A button's icon (`XACOMMON` sheet 1, image 0) for an action id, as
    /// the legends and the kind-29 prompts pick it ([`legend_icon`]).
    pub fn button_icon(&self, action: u32) -> Option<(Handle<Image>, Rect)> {
        let (atlas, frames) = self.icons.as_ref()?;
        Some((atlas.clone(), *frames.get(legend_icon(action)?)?))
    }

    /// The blue star (`XACOMMON` sheet 0, image 5, frame 0: the menu
    /// bullet, and the picture Choose Baller's empty slots show,
    /// `FUN_00210048(w, 0, 0, 5, 0)`) and its size.
    pub fn bullet_star(&self) -> (Handle<Image>, Vec2) {
        (self.star.clone(), self.star_size)
    }

    /// A string of one locale file, by name and index.
    pub fn text(&self, file: &str, index: usize) -> Option<&str> {
        self.texts.get(file)?.get(index).map(String::as_str)
    }

    /// A layout's text id (`file << 8 | index`), of the first 30 files.
    pub fn text_id(&self, id: u16) -> Option<&str> {
        self.text(
            TEXT_FILES.get(usize::from(id >> 8))?,
            usize::from(id & 0xff),
        )
    }

    /// A layout's text id over the game's whole table (`0x3ec2f0`: Create
    /// Baller's `createplyr*.txt` are files 0x20..0x2c, `editplyr*.txt`
    /// 0x30..0x37).
    pub fn text_full(&self, id: u16) -> Option<&str> {
        self.text(
            street_assets::locale::FE_TEXT_FILES_ALL.get(usize::from(id >> 8))?,
            usize::from(id & 0xff),
        )
    }
}

/// Startup: read the look from the disc. Without it the menus keep their
/// plain text.
pub fn start(mut commands: Commands, mut disc: ResMut<Disc>, mut images: ResMut<Assets<Image>>) {
    let iso = &mut disc.iso;
    let fonts = match font::fonts(iso) {
        Ok(f) => f,
        Err(e) => return warn!("front-end fonts: {e}; the plain text stays"),
    };
    let layouts = match font::layouts(iso, "/FE/XLAY_FE.BIN") {
        Ok(l) => l,
        Err(e) => return warn!("front-end layouts: {e}; the plain text stays"),
    };
    let mut texts: std::collections::HashMap<String, Vec<String>> =
        locale::read(iso, "/LOCALE/XLOC_FE.EUS")
            .map(|files| files.into_iter().map(|f| (f.name, f.texts)).collect())
            .unwrap_or_default();
    // The save is a file on the PC: the memory card's lines say so.
    if let Some(card) = texts.get_mut("memcard_ps2.txt") {
        crate::card::to_pc(card);
    }
    let common = fe::read(iso, "/FE/XACOMMON.BIN").ok();
    let Some(sheet) = common.as_ref().and_then(|s| s.first()) else {
        return warn!("front-end bullets missing; the plain text stays");
    };
    let stars = (sprite_crop(sheet, 5, 0), sprite_crop(sheet, 5, 2));
    let (Some(star), Some(star_lit)) = stars else {
        return warn!("front-end bullet frames missing; the plain text stays");
    };
    let Some(row_star) = sprite_crop(sheet, 1, 0) else {
        return warn!("front-end row star missing; the plain text stays");
    };
    let star_size = Vec2::new(star.width as f32, star.height as f32);
    let star_lit_size = Vec2::new(star_lit.width as f32, star_lit.height as f32);
    let row_star_size = Vec2::new(row_star.width as f32, row_star.height as f32);
    let star = images.add(image(star.width, star.height, star.rgba));
    let star_lit = images.add(image(star_lit.width, star_lit.height, star_lit.rgba));
    let row_star = images.add(image(row_star.width, row_star.height, row_star.rgba));
    let icons = common.as_ref().and_then(|s| s.get(1)).and_then(|s| {
        let t = s.textures.first()?;
        let frames = s
            .frames
            .first()?
            .iter()
            .map(|f| {
                let [x, y, w, h] = f.bounds();
                Rect::new(x as f32, y as f32, (x + w) as f32, (y + h) as f32)
            })
            .collect();
        Some((images.add(image(t.width, t.height, t.rgba.clone())), frames))
    });
    let pad = common.as_ref().and_then(|s| s.get(1)).and_then(|s| {
        let mut t = s.textures.iter().take(2);
        let (a, b) = (t.next()?, t.next()?);
        Some([
            images.add(image(a.width, a.height, a.rgba.clone())),
            images.add(image(b.width, b.height, b.rgba.clone())),
        ])
    });
    let tab = common
        .as_ref()
        .and_then(|s| s.first())
        .and_then(|s| sprite_crop(s, 2, 0))
        .map(|mut t| {
            for pixel in t.rgba.as_chunks_mut::<4>().0 {
                pixel[..3].fill(0xff);
            }
            images.add(image(t.width, t.height, t.rgba))
        });
    let prematch = (|| {
        let common = common.as_ref()?;
        let screens = fe::read(iso, "/FE/XAFESCRN.BIN").ok()?;
        let palettes = fe::read(iso, "/FE/XPAL_FE.BIN").ok()?;
        let sheet0 = common.first()?;
        let sheet1 = common.get(1)?;
        let scrn0 = screens.first()?;
        let scrn1 = screens.get(1)?;
        // `XPAL_FE` sheet 0: the four controllers' colours; sheet 1: the
        // meter's five levels.
        let players = &palettes.first()?.palettes;
        let levels = &palettes.get(1)?.palettes;
        if players.len() < 4 || levels.len() < 5 {
            return None;
        }
        let size = |t: &street_assets::ssh::Texture| Vec2::new(t.width as f32, t.height as f32);
        let add = |images: &mut Assets<Image>, t: street_assets::ssh::Texture| {
            images.add(image(t.width, t.height, t.rgba))
        };
        // A piece as the sheet decodes it, then in each controller's
        // palette (`FUN_00210368(w, 0, k)`).
        let blended = |mut t: street_assets::ssh::Texture| {
            gs_blend(&mut t);
            t
        };
        let swapped = |images: &mut Assets<Image>, t: street_assets::ssh::Texture| {
            let per_player: [Handle<Image>; 4] =
                std::array::from_fn(|k| add(images, blended(t.in_palette(&players[k]))));
            let [a, b, c, d] = per_player;
            [add(images, blended(t)), a, b, c, d]
        };
        let arrow = sprite_crop(sheet0, 0, 0)?;
        let arrow_size = size(&arrow);
        let mut arrow_down = arrow.clone();
        flip_vertical(&mut arrow_down);
        let arrow_left = sprite_crop(sheet0, 0, 1)?;
        let arrow_side_size = size(&arrow_left);
        let mut arrow_right = arrow_left.clone();
        flip_horizontal(&mut arrow_right);
        let pad_icon = sprite_crop(sheet1, 2, 0)?;
        let pad_icon_size = size(&pad_icon);
        let tab_wide = sprite_crop(sheet0, 2, 6)?;
        let tab_wide_size = size(&tab_wide);
        let mut corner = sprite_crop(sheet0, 2, 3)?;
        turn_quarter(&mut corner);
        let corner_size = size(&corner);
        let line = sprite_crop(sheet0, 2, 2)?;
        let tab = sprite_crop(sheet0, 2, 0)?;
        let tabs_size = size(&tab);
        let cursor_wide = sprite_crop(scrn0, 0, 0)?;
        let cursor_wide_size = size(&cursor_wide);
        let cursor = sprite_crop(scrn0, 0, 1)?;
        let cursor_size = size(&cursor);
        let cursor_court = sprite_crop(scrn0, 0, 3)?;
        let cursor_court_size = size(&cursor_court);
        let name_star = sprite_crop(sheet0, 1, 2)?;
        let name_star_size = size(&name_star);
        let frame = sprite_crop(scrn1, 0, 0)?;
        let frame_full = sprite_crop(scrn1, 0, 1)?;
        let fill = sprite_crop(scrn1, 0, 10)?;
        let fill_full = sprite_crop(scrn1, 0, 11)?;
        let meter_frame_size = size(&frame);
        let meter_fill_size = size(&fill);
        let meter_fill_levels: [Handle<Image>; 5] =
            std::array::from_fn(|k| add(&mut images, fill.in_palette(&levels[k])));
        let badge = sprite_crop(scrn0, 7, 33)?;
        let badge_size = size(&badge);
        let tile_back = sprite_crop(scrn0, 4, 1)?;
        let tile_back_size = size(&tile_back);
        let tile_frame = sprite_crop(scrn0, 5, 0)?;
        let tile_size = size(&tile_frame);
        let tile_mystery = sprite_crop(scrn0, 9, 0)?;
        let tile_mystery_size = size(&tile_mystery);
        let check = sprite_crop(scrn0, 14, 4)?;
        let check_size = size(&check);
        let plates = palettes
            .get(2)
            .map(|s| s.palettes.as_slice())
            .unwrap_or(&[]);
        let tile_back: Vec<Handle<Image>> = plates
            .iter()
            .map(|p| add(&mut images, tile_back.in_palette(p)))
            .collect();
        let back_small = sprite_crop(scrn0, 4, 0)?;
        let tile_back_small_size = size(&back_small);
        let tile_back_small: Vec<Handle<Image>> = plates
            .iter()
            .map(|p| add(&mut images, back_small.in_palette(p)))
            .collect();
        let tile_frame_small = sprite_crop(scrn0, 6, 0)?;
        let tile_small_size = size(&tile_frame_small);
        let star_card = sprite_crop(scrn0, 1, 5)?;
        let star_card_size = size(&star_card);
        let vol2 = fe::sheet(iso, "/FE/REPLAY.SSH")
            .ok()
            .and_then(|s| {
                s.textures
                    .first()
                    .and_then(|t| rect_crop(t, 6, 143, 156, 95))
            })
            .map(|t| {
                let s = size(&t);
                (add(&mut images, t), s)
            });
        Some(PrematchArt {
            arrow_up: add(&mut images, arrow),
            arrow_down: add(&mut images, arrow_down),
            arrow_size,
            arrow_left: add(&mut images, arrow_left),
            arrow_right: add(&mut images, arrow_right),
            arrow_side_size,
            pad_icon: add(&mut images, pad_icon),
            pad_icon_size,
            tab_wide: swapped(&mut images, tab_wide),
            tab_wide_size,
            corner: swapped(&mut images, corner),
            corner_size,
            line: swapped(&mut images, line),
            tabs: std::array::from_fn(|k| add(&mut images, blended(tab.in_palette(&players[k])))),
            tabs_size,
            cursor_wide: add(&mut images, cursor_wide),
            cursor_wide_size,
            cursor: add(&mut images, cursor),
            cursor_size,
            cursor_court: add(&mut images, cursor_court),
            cursor_court_size,
            name_star: add(&mut images, name_star),
            name_star_size,
            meter: [
                add(&mut images, frame),
                add(&mut images, frame_full),
                add(&mut images, fill),
                add(&mut images, fill_full),
            ],
            meter_frame_size,
            meter_fill_size,
            meter_fill_levels,
            badge: add(&mut images, badge),
            badge_size,
            tile_back,
            tile_back_size,
            tile_frame: add(&mut images, tile_frame),
            tile_size,
            tile_mystery: add(&mut images, tile_mystery),
            tile_mystery_size,
            check: add(&mut images, check),
            check_size,
            tile_back_small,
            tile_back_small_size,
            tile_frame_small: add(&mut images, tile_frame_small),
            tile_small_size,
            star_card: add(&mut images, star_card),
            star_card_size,
            vol2,
        })
    })();
    if prematch.is_none() {
        warn!("pre-match art missing: User Setup and Choose Team keep plain pieces");
    }
    let segmented_bar = common.as_ref().and_then(|s| s.first()).and_then(|s| {
        let end = sprite_crop(s, 6, 6)?;
        let middle = sprite_crop(s, 6, 7)?;
        let end_width = end.width as f32;
        let height = end.height as f32;
        Some(SegmentedBar {
            end: images.add(image(end.width, end.height, end.rgba)),
            middle: images.add(image(middle.width, middle.height, middle.rgba)),
            end_width,
            height,
        })
    });
    let native_slider = common.as_ref().and_then(|s| s.get(2)).and_then(|s| {
        let t = s.textures.first()?;
        let frames = s.frames.first()?;
        let source = |index: usize| {
            let [x, y, w, h] = frames.get(index)?.bounds();
            (x >= 0
                && y >= 0
                && w > 0
                && h > 0
                && x + w <= t.width as i32
                && y + h <= t.height as i32)
                .then(|| Rect::new(x as f32, y as f32, (x + w) as f32, (y + h) as f32))
        };
        let track = source(4)?;
        let end = source(0)?;
        // FUN_002370c0 trims the middle's normalized atlas coordinates.
        let trim = |mut middle: Rect| {
            middle.min.x += 0.005 * t.width as f32;
            middle.max.x -= 0.0025 * t.width as f32;
            middle
        };
        let middle = trim(source(1)?);
        let lit = source(2).zip(source(3).map(trim));
        Some(NativeSlider {
            atlas: images.add(image(t.width, t.height, t.rgba.clone())),
            track,
            end,
            middle,
            lit,
        })
    });
    let slider = common
        .and_then(|s| s.into_iter().nth(2))
        .and_then(|s| s.textures.into_iter().next())
        .map(|t| {
            let mut crop = |x: u32, y: u32, w: u32, h: u32| {
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                for row in y..y + h {
                    let at = ((row * t.width + x) * 4) as usize;
                    rgba.extend_from_slice(&t.rgba[at..at + (w * 4) as usize]);
                }
                images.add(image(w, h, rgba))
            };
            [crop(4, 45, 175, 19), crop(8, 8, 8, 10), crop(8, 21, 8, 10)]
        });
    info!(
        "front-end look: {} fonts, {} layouts",
        fonts.len(),
        layouts.len()
    );
    let system_font = iso
        .read("/SLUS_206.51")
        .ok()
        .and_then(|elf| font::system(&elf).ok());
    commands.insert_resource(Look {
        fonts,
        system_font,
        layouts,
        texts,
        star,
        star_lit,
        star_size,
        star_lit_size,
        row_star,
        row_star_size,
        icons,
        slider,
        native_slider,
        segmented_bar,
        pad,
        tab,
        prematch,
    });
}

/// Mirrors a texture top to bottom (the kind-37 table's flag swaps the
/// frame's top and bottom corner pairs).
fn flip_vertical(t: &mut street_assets::ssh::Texture) {
    let row = (t.width * 4) as usize;
    let rows: Vec<&[u8]> = t.rgba.chunks(row).rev().collect();
    t.rgba = rows.concat();
    if let Some(indices) = &mut t.indices {
        let rows: Vec<&[u8]> = indices.chunks(t.width as usize).rev().collect();
        *indices = rows.concat();
    }
}

/// Mirrors a texture left to right.
fn flip_horizontal(t: &mut street_assets::ssh::Texture) {
    let row = (t.width * 4) as usize;
    t.rgba = t
        .rgba
        .chunks(row)
        .flat_map(|r| r.chunks(4).rev().flatten().copied().collect::<Vec<u8>>())
        .collect();
    if let Some(indices) = &mut t.indices {
        *indices = indices
            .chunks(t.width as usize)
            .flat_map(|r| r.iter().rev().copied().collect::<Vec<u8>>())
            .collect();
    }
}

/// Turns a texture a quarter anticlockwise (a picture's rotation -90,
/// `+0xec`, as `FUN_0026ee20` gives the corner pieces).
pub(crate) fn turn_quarter(t: &mut street_assets::ssh::Texture) {
    let (w, h) = (t.width as usize, t.height as usize);
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            // The source's right edge becomes the top.
            let (nx, ny) = (y, w - 1 - x);
            out[(ny * h + nx) * 4..(ny * h + nx) * 4 + 4]
                .copy_from_slice(&t.rgba[(y * w + x) * 4..(y * w + x) * 4 + 4]);
        }
    }
    t.rgba = out;
    t.width = h as u32;
    t.height = w as u32;
}

/// A rectangle of a texture's pixels (`None` past its edges).
fn rect_crop(
    t: &street_assets::ssh::Texture,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Option<street_assets::ssh::Texture> {
    if x + w > t.width || y + h > t.height {
        return None;
    }
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let at = ((row * t.width + x) * 4) as usize;
        rgba.extend_from_slice(&t.rgba[at..at + (w * 4) as usize]);
    }
    Some(street_assets::ssh::Texture {
        name: t.name.clone(),
        width: w,
        height: h,
        rgba,
        indices: None,
    })
}

/// Crop an axis-aligned native frame, retaining its authored dimensions.
/// The generic frame reader preserves corner order for future rotated/flipped
/// widgets; this crop accepts only the ordinary TL/TR/BR/BL ordering.
pub(crate) fn sprite_crop(
    sheet: &street_assets::ssh::Sheet,
    texture: usize,
    frame: usize,
) -> Option<street_assets::ssh::Texture> {
    let t = sheet.textures.get(texture)?;
    let f = *sheet.frames.get(texture)?.get(frame)?;
    let [x, y, w, h] = f.bounds();
    if f.corners != [[x, y], [x + w, y], [x + w, y + h], [x, y + h]] {
        return None;
    }
    let (x, y, w, h) = (
        u32::try_from(x).ok()?,
        u32::try_from(y).ok()?,
        u32::try_from(w).ok()?,
        u32::try_from(h).ok()?,
    );
    if w == 0 || h == 0 || x.checked_add(w)? > t.width || y.checked_add(h)? > t.height {
        return None;
    }
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    let mut indices = t
        .indices
        .as_ref()
        .map(|_| Vec::with_capacity((w * h) as usize));
    for row in y..y + h {
        let at = ((row * t.width + x) * 4) as usize;
        rgba.extend_from_slice(t.rgba.get(at..at + (w * 4) as usize)?);
        if let (Some(out), Some(all)) = (indices.as_mut(), t.indices.as_ref()) {
            let at = (row * t.width + x) as usize;
            out.extend_from_slice(all.get(at..at + w as usize)?);
        }
    }
    Some(street_assets::ssh::Texture {
        name: t.name.clone(),
        width: w,
        height: h,
        rgba,
        indices,
    })
}

pub(crate) fn image(width: u32, height: u32, rgba: Vec<u8>) -> Image {
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// The alpha a black piece needs here to darken what is under it as the
/// PS2 does at alpha `a`. The GS blends in the picture's own (gamma)
/// values, `under * (1 - a)`; the window composites the pieces in linear
/// light (an sRGB target), where a black layer at `a` leaves `(1 - a)` of
/// the linear light, about `(1 - a) ^ (1 / 2.2)` of the gamma value. So the
/// same darkening takes `1 - (1 - a) ^ 2.2`. Measured on Choose Baller's
/// dialog: the original keeps 0.26..0.45 of the list's light behind it,
/// the rewrite kept 0.50..0.57 at the PS2's alpha.
pub fn black_alpha(a: f32) -> f32 {
    1.0 - (1.0 - a.clamp(0.0, 1.0)).powf(2.2)
}

/// A translucent sprite's pixels made to blend here as the GS blends them.
/// The GS mixes in the picture's own (gamma) values, `a * c + (1 - a) * b`;
/// the window composites in linear light. Each partly transparent pixel
/// takes the alpha that darkens what is under it as the PS2 does
/// ([`black_alpha`]) and its colour scaled so that over black it leaves
/// what the PS2 leaves: exact over black, and close over the dark panels
/// these pieces lie on. The controller palettes' frame pieces need it:
/// their entries are 0x25 and 0x49 of the PS2's 0x80 (`XPAL_FE` sheet 0),
/// which in linear light came out half again as bright (145 against 100
/// red on the frame lines).
pub fn gs_blend(t: &mut street_assets::ssh::Texture) {
    for px in t.rgba.as_chunks_mut::<4>().0 {
        let a = f32::from(px[3]) / 255.0;
        if a <= 0.0 || a >= 1.0 {
            continue;
        }
        let out = black_alpha(a);
        let scale = a / out.powf(1.0 / 2.2);
        for c in &mut px[..3] {
            *c = (f32::from(*c) * scale).round().clamp(0.0, 255.0) as u8;
        }
        px[3] = (out * 255.0).round() as u8;
    }
}

/// One image on the 640x480 screen.
#[derive(Clone, Debug)]
pub struct Piece {
    pub image: Handle<Image>,
    pub rect: Rect,
    /// The part of the image to show, in its pixels (all of it if `None`).
    pub source: Option<Rect>,
    /// Mirrored left to right.
    pub flip: bool,
    /// Upside down.
    pub flip_y: bool,
    pub colour: Color,
    /// What the screen's transitions do with it ([`crate::transition`]).
    pub role: Role,
}

/// A piece's part in a screen's transitions (`FUN_00249cb0`): the title
/// widget slides, the other widgets fade, and what is not a widget of the
/// screen (the city loop over its backdrop) stays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Widget,
    Title,
    Fixed,
}

impl Piece {
    pub fn new(image: Handle<Image>, rect: Rect) -> Self {
        Self {
            image,
            rect,
            source: None,
            flip: false,
            flip_y: false,
            colour: Color::WHITE,
            role: Role::Widget,
        }
    }

    /// A part of an atlas.
    pub fn part(image: Handle<Image>, source: Rect, rect: Rect) -> Self {
        Self {
            source: Some(source),
            ..Self::new(image, rect)
        }
    }
}

/// Where a line of text sits on its point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Centre,
    Right,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    font: usize,
    text: String,
    colour: [u8; 4],
    shadow: Option<([u8; 4], [u8; 4])>,
}

struct Drawn {
    image: Handle<Image>,
    size: Vec2,
    origin: Vec2,
    used: u64,
}

/// Text already drawn, by font, string and colours.
#[derive(Resource, Default)]
pub struct Labels {
    drawn: HashMap<Key, Drawn>,
    frame: u64,
}

/// How far the title's shadow falls, in screen pixels.
const SHADOW: u32 = 3;

/// Lays `src` over `dst` at (dx, dy), both straight RGBA.
fn over(dst: &mut [u8], dw: u32, src: &[u8], sw: u32, sh: u32, dx: u32, dy: u32) {
    for y in 0..sh {
        for x in 0..sw {
            let s = ((y * sw + x) * 4) as usize;
            let a = u32::from(src[s + 3]);
            if a == 0 {
                continue;
            }
            let d = (((y + dy) * dw + x + dx) * 4) as usize;
            let da = u32::from(dst[d + 3]);
            let out = a + da * (255 - a) / 255;
            for k in 0..3 {
                let c = (u32::from(src[s + k]) * a + u32::from(dst[d + k]) * da * (255 - a) / 255)
                    / out.max(1);
                dst[d + k] = c as u8;
            }
            dst[d + 3] = out as u8;
        }
    }
}

impl Labels {
    /// `text` in `font`, its line's top at `at` on the screen, left,
    /// centred or right of it.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        look: &Look,
        images: &mut Assets<Image>,
        font: usize,
        text: &str,
        colour: [u8; 4],
        shadow: Option<([u8; 4], [u8; 4])>,
        at: Vec2,
        align: Align,
    ) -> Option<Piece> {
        let f = look.fonts.get(font)?;
        if text.trim().is_empty() {
            return None;
        }
        let key = Key {
            font,
            text: text.to_owned(),
            colour,
            shadow,
        };
        let frame = self.frame;
        let d = self.drawn.entry(key).or_insert_with(|| {
            let main = f.render(text, colour, colour);
            let (mut w, mut h, mut rgba) = (main.width, main.height, main.rgba);
            if let Some((top, bottom)) = shadow {
                let back = f.render(text, top, bottom);
                let (sw, sh) = (w + SHADOW, h + SHADOW);
                let mut both = vec![0u8; (sw * sh * 4) as usize];
                over(
                    &mut both,
                    sw,
                    &back.rgba,
                    back.width,
                    back.height,
                    SHADOW,
                    SHADOW,
                );
                over(&mut both, sw, &rgba, w, h, 0, 0);
                (w, h, rgba) = (sw, sh, both);
            }
            Drawn {
                image: images.add(image(w, h, rgba)),
                size: Vec2::new(w as f32, h as f32),
                origin: Vec2::new(main.origin.0 as f32, main.origin.1 as f32),
                used: frame,
            }
        });
        d.used = frame;
        let mut min = at - d.origin;
        let width = d.size.x - 2.0 * d.origin.x;
        match align {
            Align::Left => {}
            Align::Centre => min.x -= width / 2.0,
            Align::Right => min.x -= width,
        }
        Some(Piece::new(
            d.image.clone(),
            Rect::from_corners(min, min + d.size),
        ))
    }

    /// Drops text no page has shown for a while (once a frame).
    pub fn tidy(&mut self) {
        self.frame += 1;
        if self.drawn.len() > 160 {
            let frame = self.frame;
            self.drawn.retain(|_, d| frame - d.used < 30);
        }
    }
}

/// Where a page's entries go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The title screen: only "Press Start", centred low; "Loading
    /// Demo..." in its place, without the stars, while the idle demo loads
    /// (`FUN_002d3d68`).
    Title { demo: bool },
    /// Big entries placed by a layout's text elements (Main, Play It).
    Layout(usize),
    /// A screen's authored text positions, fonts, colours and navigation
    /// points, selected by screen class rather than archive position.
    Screen(u32),
    /// A list of rows in a smaller font, columns kept.
    List,
    /// Text to read: no bullets, no highlight but on its last row.
    Read,
}

/// A page to draw.
pub struct View<'a> {
    pub style: Style,
    pub title: &'a str,
    pub warm: bool,
    pub items: &'a [String],
    pub cursor: usize,
    /// The help line; empty for none.
    pub help: &'a str,
    /// The keys, small along the bottom.
    pub keys: &'a str,
    /// Where a list's rows start, screen pixels ([`LIST_X`] but on Create
    /// A Baller).
    pub left: f32,
    /// Front-end updates since the page opened: the highlighted screen row
    /// pulses by them ([`row_pulse`]).
    pub ticks: u64,
}

/// The screen's bottom: the help line's top.
const HELP_Y: f32 = 420.0;
/// The help line's font (`FUN_0021afa8`).
const HELP_FONT: usize = 1;
const HELP_X: f32 = 58.0;
const TITLE_AT: Vec2 = Vec2::new(60.0, 42.0);
/// Lists start below the title, at the Options screen's left column.
pub const LIST_X: f32 = 92.0;
const LIST_Y: f32 = 108.0;
/// The lists end above the backdrops' bottom bar, which carries the help.
const LIST_BOTTOM: f32 = 374.0;

/// One step along a screen's authored navigation graph, as `FUN_0021f8f0`
/// takes it: the kind-10 node on the cursor's row (prop 34) in the screen's
/// list (prop 33), its mode for the direction (`FUN_00222590`: props 25, 27,
/// 29, 31) and its link (`FUN_00222678`: props 26, 28, 30, 32).
///
/// Directions: `-1`/`0` up, `1` down, `-2`/`2` left, `3` right.
///
/// Returns the row the step lands on; `cursor` when the mode is 0 or the
/// link names no node of the list (no step); `None` for mode 1, which scrolls
/// the list (`FUN_0021fa48`, the caller's), or when the cursor's row has no
/// node. A target whose row `is_available` refuses (`FUN_00222770`: no bound
/// widget, or one switched off) is stepped over in the same direction from
/// that node, as `FUN_0021f8f0` recurses; a dead end there is no step.
pub fn screen_step_filtered(
    layout: &Layout,
    cursor: usize,
    count: usize,
    dir: i32,
    is_available: impl Fn(usize) -> bool,
) -> Option<usize> {
    if count == 0 || cursor >= count {
        return None;
    }
    let mut groups: Vec<(u32, usize)> = Vec::new();
    for e in layout.elements.iter().filter(|e| e.kind == 10) {
        let (Some(&row), Some(&tag)) = (e.props.get(&34), e.props.get(&33)) else {
            continue;
        };
        if (row as usize) >= count {
            continue;
        }
        match groups.iter_mut().find(|g| g.0 == tag) {
            Some(g) => g.1 += 1,
            None => groups.push((tag, 1)),
        }
    }
    // The list the screen's constructor registers its rows on; without a
    // traced constructor, the dominant (largest) in-range group, ties going
    // to the cursor's own group (the rewrite's own choice).
    let tag = match screen_binding(layout.class) {
        Some(binding) => binding.list,
        None => {
            let size = groups.iter().map(|g| g.1).max()?;
            groups
                .iter()
                .filter(|g| g.1 == size)
                .find(|g| {
                    layout.elements.iter().any(|e| {
                        e.kind == 10
                            && e.props.get(&33) == Some(&g.0)
                            && e.props.get(&34) == Some(&(cursor as u32))
                    })
                })
                .or_else(|| groups.iter().find(|g| g.1 == size))
                .map(|g| g.0)?
        }
    };

    // Native direction index (`FUN_00222590`): 0 up, 1 down, 2 left, 3 right.
    let (mode_prop, link_prop) = match dir {
        -1 | 0 => (25, 26),
        -2 | 2 => (29, 30),
        3 => (31, 32),
        _ => (27, 28),
    };
    let mut current_cursor = cursor;
    let mut visited = Vec::new();

    loop {
        visited.push(current_cursor);
        let Some(node) = layout.elements.iter().find(|e| {
            e.kind == 10
                && e.props.get(&34) == Some(&(current_cursor as u32))
                && e.props.get(&33) == Some(&tag)
        }) else {
            return if current_cursor == cursor {
                None
            } else {
                Some(cursor)
            };
        };

        // `FUN_00221ba8` reads the mode and link props through
        // `FUN_002e3b18`, which answers 0 for a prop the element lacks: a
        // missing mode is 0 (disabled) and a missing link is node 0.
        // `FUN_00222590`/`FUN_00222678` force mode 2 and take the direction
        // from a bound kind-36 widget's byte +0xb0; no layout authors kind
        // 36 and none of these screens binds one, so the node's own props
        // decide here.
        match node.props.get(&mode_prop).copied().unwrap_or(0) {
            // Mode 1 scrolls the list (`FUN_0021fa48`): the caller's.
            1 => return None,
            2 => {}
            // Mode 0, or any other value: no step (`FUN_0021f8f0` -1).
            _ => return Some(cursor),
        }

        // A link is a node id (a halfword: -1 is 0xffff, which no node
        // has), looked up among the list's own nodes.
        let link = node.props.get(&link_prop).copied().unwrap_or(0) & 0xffff;
        let Some(target) = layout
            .elements
            .iter()
            .find(|e| e.kind == 10 && e.id == link && e.props.get(&33) == Some(&tag))
        else {
            return Some(cursor);
        };

        let Some(&target_row) = target.props.get(&34) else {
            return Some(cursor);
        };
        let row = target_row as usize;
        // A row past the bound widgets has none: unavailable.
        if row < count && is_available(row) {
            return Some(row);
        }
        // The original would recurse for ever round a ring of unavailable
        // nodes; the rewrite stops there without a step.
        if visited.contains(&row) {
            return Some(cursor);
        }

        current_cursor = row;
    }
}

pub fn screen_step(layout: &Layout, cursor: usize, count: usize, dir: i32) -> Option<usize> {
    screen_step_filtered(layout, cursor, count, dir, |_| true)
}

/// Native screen row-element registration IDs (recovered from native screen setup functions
/// in SLUS_206.51: FUN_0029e230, FUN_002beef8, FUN_002a3c60, FUN_002a03c0, FUN_00289a10,
/// FUN_0028a970, FUN_00288be0).
pub fn screen_registered_row_element_ids(screen_class: u32) -> Option<&'static [u32]> {
    match screen_class {
        // Main Menu (class 3, FUN_0029e230): list 19 registers templates 0, 1, 23, 21, 26,
        // and dynamic kind-19 element 31 (text id 0x0306).
        3 => Some(&[0, 1, 23, 21, 26, 31]),
        // Play It (class 5, FUN_002beef8): list 0 registers elements 15, 17, 18, 16.
        5 => Some(&[15, 17, 18, 16]),
        // My Team (class 6, FUN_002a3c60): list 5 registers elements 1, 2.
        6 => Some(&[1, 2]),
        // Be A Legend map (class 56, FUN_002a03c0): list 0 registers landmarks 100, 102, 101, 103, 104, 105
        // which pair with texts 27, 29, 28, 30, 31, 32.
        56 => Some(&[27, 29, 28, 30, 31, 32]),
        // Inside Vol. 2 (class 102, FUN_00289a10): list 0 registers elements 15, 17, 19, 22, 18.
        102 => Some(&[15, 17, 19, 22, 18]),
        // What's New (class 103, FUN_0028a970): list 0 registers elements 22, 16, 18, 19, 21.
        103 => Some(&[22, 16, 18, 19, 21]),
        // Bobbito (class 104, FUN_00288be0): list 0 registers elements 15, 16.
        104 => Some(&[15, 16]),
        // Difficulty (class 105, FUN_00284280): list 0 registers elements
        // 17, 16, 15 (Legendary, Mad Game, Got Game, the hardest on top).
        105 => Some(&[17, 16, 15]),
        // Set Challenge (class 39, FUN_002c8d08): list 38 registers
        // elements 37, 39, 41, 40, 36 (Game Mode, Play To, Spot Points,
        // Shot Clock, Play Game).
        39 => Some(&[37, 39, 41, 40, 36]),
        // Game Data (class 97, FUN_00299600): list 9 registers elements 10
        // (Save), 11 (Load, made by the code: font 2, text 0x3e02), 12, 13,
        // 14 (the three deletes) and 15 (Exit).
        97 => Some(&[10, 11, 12, 13, 14, 15]),
        _ => None,
    }
}

/// How a screen's constructor binds its rows: the kind-9 list they go on,
/// the row widgets in registration order (`FUN_0021f508`), and how far it
/// moves the list's navigation nodes first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Binding {
    pub list: u32,
    pub rows: &'static [u32],
    /// `FUN_0029e230` moves each Main Menu node by (0, -20) (raw 0x29e2d4)
    /// before the rows bind; the other constructors leave them.
    pub node_dy: f32,
    /// Whether the rows carry the highlight stars: the constructors give
    /// them to their rows with `FUN_0021c628(row, 1)`; Set Challenge's
    /// (`FUN_002c8d08`) and Game Data's (`FUN_00299600`) never do, so
    /// their rows only change colour.
    pub stars: bool,
}

/// The screens whose row registration is traced, by class.
pub fn screen_binding(class: u32) -> Option<Binding> {
    let rows = screen_registered_row_element_ids(class)?;
    let (list, node_dy) = match class {
        3 => (19, -20.0),
        5 => (0, 0.0),
        6 => (5, 0.0),
        39 => (38, 0.0),
        97 => (9, 0.0),
        102..=105 => (0, 0.0),
        _ => return None,
    };
    Some(Binding {
        list,
        rows,
        node_dy,
        stars: !matches!(class, 39 | 97),
    })
}

/// The rows a constructor creates rather than loading them from the
/// layout, and their fonts: the Main Menu's sixth (`FUN_0029e230`,
/// `FUN_0021c4c0(.., 0x1f)`, font 4 through `FUN_002e5578(4)`, text 0x0306)
/// and Game Data's Load (`FUN_00299600`, `FUN_0021c4c0(.., 0xb)`, font 2).
fn dynamic_row(class: u32, id: u32) -> Option<usize> {
    match (class, id) {
        (3, 31) => Some(4),
        (97, 11) => Some(2),
        _ => None,
    }
}

/// Where a bound screen row draws: its text's anchor point and the row
/// widget's font and alignment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowSpot {
    pub at: Vec2,
    pub font: usize,
    pub align: Align,
    /// Vertical anchor: 0 top, 1 middle, 2 bottom (`(align >> 2) & 3`,
    /// `FUN_00248df8`).
    pub valign: u8,
}

/// Each registered row's place. `FUN_0021f580` keeps the list's nodes in
/// prop-34 order and `FUN_0021f230` pairs them with the registered widgets
/// in turn; `FUN_00222420` then moves each widget, children and all, onto
/// its node's point (`+0x124`), so a row draws where its node is, not where
/// the layout authored the row's template. A widget beyond the nodes stays
/// hidden (`None`). The text keeps the template's font (prop 13, through
/// `FUN_0021c750`) and alignment.
pub fn screen_rows(layout: &Layout) -> Option<Vec<Option<RowSpot>>> {
    let binding = screen_binding(layout.class)?;
    let mut nodes: Vec<(u32, Vec2)> = layout
        .elements
        .iter()
        .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&binding.list))
        .filter_map(|e| {
            let (x, y) = e.position()?;
            Some((
                *e.props.get(&34)?,
                Vec2::new(f32::from(x), f32::from(y) + binding.node_dy),
            ))
        })
        .collect();
    nodes.sort_by_key(|n| n.0);
    Some(
        binding
            .rows
            .iter()
            .enumerate()
            .map(|(k, &id)| {
                let &(_, at) = nodes.get(k)?;
                let template = layout.elements.iter().find(|e| e.kind == 19 && e.id == id);
                let (font, align, valign) = match template {
                    Some(e) => (
                        e.props.get(&13).map_or(fonts::ENTRY, |&f| f as usize),
                        match e.align & 3 {
                            1 => Align::Centre,
                            2 => Align::Right,
                            _ => Align::Left,
                        },
                        (e.align >> 2) & 3,
                    ),
                    None => (dynamic_row(layout.class, id)?, Align::Left, 0),
                };
                Some(RowSpot {
                    at,
                    font,
                    align,
                    valign,
                })
            })
            .collect(),
    )
}

/// The highlighted row's text alpha after `ticks` updates: `FUN_0021cd28`
/// (the row widget's update) walks byte +0xc0 from 255 down by 5 until it
/// drops under 127 (it keeps that value, 125), then up by 5 until it would
/// reach 255 (it stores 255), and so on; the selected row's text takes it
/// (`+0x12c`). Every row starts at 255 when the screen builds.
pub fn row_pulse(ticks: u64) -> u8 {
    // One period: 255, 250, .. 125 (27 values), then 130, .. 250, 255.
    let period = 52;
    let k = (ticks % period) as i32;
    if k <= 26 {
        (255 - 5 * k) as u8
    } else {
        (125 + 5 * (k - 26)).min(255) as u8
    }
}

/// The icon frame (`XACOMMON` sheet 1, image 0) for a legend's action id,
/// as `FUN_002127c0` picks it with the region word (`FUN_0030fa50()+0x1c`)
/// at 0, the US default: 0 Cross, 1 Triangle, 2 Square, 3 and 5 Circle,
/// 4 frame 9, 6 Select.
pub fn legend_icon(action: u32) -> Option<usize> {
    Some(match action {
        0 => 3,
        1 => 2,
        2 => 1,
        3 | 5 => 0,
        4 => 9,
        6 => 19,
        _ => return None,
    })
}

/// `FUN_0021ab10`: a legend's pairs from its last to its first, each icon
/// then 6 pixels then its label, 22 pixels between pairs.
const LEGEND_LABEL_GAP: f32 = 6.0;
const LEGEND_PAIR_GAP: f32 = 22.0;

/// A screen row's text and, when highlighted, its two stars
/// (`FUN_0021c9e0`): the text's left edge less the 5-pixel margin is the
/// left star's right edge, its right edge plus 5 the right star's left
/// edge, both on the text's middle (the arrows' anchors: right/middle and
/// left/middle, `FUN_0021c628`). The text's box is as the text widget
/// measures it (`FUN_00239e70`): one line is the font's text height (the
/// header byte at 0x12, 29 for font 3), not its line spacing (37), so the
/// middle and bottom anchors and the stars' middle sit by that height.
#[allow(clippy::too_many_arguments)]
pub(crate) fn screen_row(
    out: &mut Vec<Piece>,
    stars: &mut Vec<Piece>,
    labels: &mut Labels,
    look: &Look,
    images: &mut Assets<Image>,
    spot: RowSpot,
    item: &str,
    lit: bool,
    stars_shown: bool,
    ticks: u64,
) {
    let Some(font) = look.fonts.get(spot.font) else {
        return;
    };
    let line = font.text_height as f32;
    let width = font.measure(item) as f32;
    let mut top = spot.at.y;
    match spot.valign {
        1 => top -= line / 2.0,
        2 => top -= line,
        _ => {}
    }
    let left = match spot.align {
        Align::Left => spot.at.x,
        Align::Centre => spot.at.x - width / 2.0,
        Align::Right => spot.at.x - width,
    };
    let colour = if lit { colours::ROW_LIT } else { colours::ROW };
    if let Some(mut p) = labels.text(
        look,
        images,
        spot.font,
        item,
        colour,
        None,
        Vec2::new(spot.at.x, top),
        spot.align,
    ) {
        if lit {
            p.colour = Color::srgba(1.0, 1.0, 1.0, f32::from(row_pulse(ticks)) / 255.0);
        }
        out.push(p);
    }
    if lit && stars_shown {
        let size = look.row_star_size;
        let middle = top + line / 2.0;
        let gap = 5.0;
        let left_min = Vec2::new(left - gap - size.x, middle - size.y / 2.0);
        let right_min = Vec2::new(left + width + gap, middle - size.y / 2.0);
        for min in [left_min, right_min] {
            stars.push(Piece::new(
                look.row_star.clone(),
                Rect::from_corners(min, min + size),
            ));
        }
    }
}

/// A screen's button legends (kind 34, `FUN_0021a600`): up to four pairs,
/// a label (props 56..59, text ids; the first 0 ends them) and an icon
/// (props 60..63, action ids), in the element's font (prop 13). The
/// loader flips the element's horizontal anchor (left to right, right to
/// left) and `FUN_0021ab10` lays the pairs out from the last; the base
/// draw `FUN_002495d0` then offsets the children by the legend's anchor
/// (`FUN_00249448`), so a right-anchored legend ends at its point. Each
/// piece sits on the legend's vertical anchor.
fn legends(
    out: &mut Vec<Piece>,
    labels: &mut Labels,
    look: &Look,
    images: &mut Assets<Image>,
    (layout, title): (&Layout, &str),
) {
    for e in layout.elements.iter().filter(|e| e.kind == 34) {
        // The Main Menu's legend (layout 3) gains the rewrite's own first
        // pair, Select for Online (Vj, 2026-10-21: one press from the Main
        // Menu, which has no Select question of its own); not on the Online
        // page drawn on the same layout.
        let main;
        let e = if layout.class == MAIN_CLASS && title != ONLINE_TITLE {
            main = with_online_pair(e);
            &main
        } else {
            e
        };
        // Over the game's whole table (the credits' screen's are file
        // 0x44, `creditsintro.txt`).
        legend(out, labels, look, images, e, &|id| {
            if id == ONLINE_WORD {
                return Some("Online".to_owned());
            }
            look.text_full(id).map(str::to_owned)
        });
    }
}

/// The Main Menu's layout class.
const MAIN_CLASS: u32 = 3;
/// The rewrite's Online page's title (drawn on the Main Menu's layout).
pub const ONLINE_TITLE: &str = "Online";
/// A text id no file uses, standing for the rewrite's "Online".
const ONLINE_WORD: u16 = 0xfffe;
/// The legend action for Select ([`legend_icon`]).
const SELECT_ACTION: u32 = 6;

/// `e` with Select for Online put first, its own pairs after (a legend
/// holds four; a full one keeps its first three).
fn with_online_pair(e: &street_assets::font::Element) -> street_assets::font::Element {
    let mut out = e.clone();
    for k in (0..3u16).rev() {
        for base in [56u16, 60] {
            match e.props.get(&(base + k)) {
                Some(&v) => out.props.insert(base + k + 1, v),
                None => out.props.remove(&(base + k + 1)),
            };
        }
    }
    out.props.insert(56, u32::from(ONLINE_WORD));
    out.props.insert(60, SELECT_ACTION);
    out
}

/// A tab strip's shoulders: the list's arrows (kind 37, the elements
/// `arrows`) and the L1 and R1 icons beside them (kind 29, the elements
/// `icons`, given icons 5 and 6 by `FUN_00212748`), each on its element's
/// point.
pub(crate) fn shoulder_tabs(
    out: &mut Vec<Piece>,
    look: &Look,
    layout: &Layout,
    (left_arrow, right_arrow): (u32, u32),
    (left_icon, right_icon): (u32, u32),
) {
    let spot = |kind: u8, id: u32| {
        layout
            .elements
            .iter()
            .find(|e| e.kind == kind && e.id == id)
            .and_then(|e| e.position())
            .map(|(x, y)| Vec2::new(f32::from(x), f32::from(y)))
    };
    if let Some(pm) = look.prematch.as_ref() {
        for (id, image) in [(left_arrow, &pm.arrow_left), (right_arrow, &pm.arrow_right)] {
            if let Some(at) = spot(37, id) {
                out.push(Piece::new(
                    image.clone(),
                    Rect::from_center_size(at, pm.arrow_side_size),
                ));
            }
        }
    }
    if let Some((atlas, frames)) = look.icons.as_ref() {
        for (id, frame) in [(left_icon, 5), (right_icon, 6)] {
            if let (Some(at), Some(&f)) = (spot(29, id), frames.get(frame)) {
                out.push(Piece::part(
                    atlas.clone(),
                    f,
                    Rect::from_center_size(at, f.size()),
                ));
            }
        }
    }
}

/// One legend element (kind 34), its words looked up by `text` (the
/// in-game screens' come from `XLOC_GM`).
pub(crate) fn legend(
    out: &mut Vec<Piece>,
    labels: &mut Labels,
    look: &Look,
    images: &mut Assets<Image>,
    e: &street_assets::font::Element,
    text: &dyn Fn(u16) -> Option<String>,
) {
    let Some((atlas, frames)) = &look.icons else {
        return;
    };
    {
        let Some((x, y)) = e.position() else {
            return;
        };
        let font_index = e.props.get(&13).map_or(0, |&f| f as usize);
        let Some(font) = look.fonts.get(font_index) else {
            return;
        };
        let colour = match e.props.get(&10) {
            Some(&c) if c != u32::MAX => font::argb(c),
            _ => colours::LEGEND,
        };
        let pairs: Vec<(String, Rect)> = (0..4)
            .map_while(|k| {
                let id = *e.props.get(&(56 + k))?;
                (id != 0).then_some((k, id))
            })
            .filter_map(|(k, id)| {
                let action = e.props.get(&(60 + k)).copied().unwrap_or(0);
                Some((text(id as u16)?, *frames.get(legend_icon(action)?)?))
            })
            .collect();
        if pairs.is_empty() {
            return;
        }
        // Laid out from x, last pair first; then shifted by the anchor.
        let mut placed = Vec::new();
        let mut off = 0.0;
        for (k, (text, icon)) in pairs.iter().enumerate().rev() {
            placed.push((off, *icon, None));
            off += icon.width() + LEGEND_LABEL_GAP;
            placed.push((off, *icon, Some(text.as_str())));
            off += font.measure(text) as f32;
            if k != 0 {
                off += LEGEND_PAIR_GAP;
            }
        }
        // The flipped anchor: an element authored left (or centred, which
        // `FUN_0021a600` leaves) draws right of or about its point only
        // when it was authored right.
        let shift = match e.align & 3 {
            0 => -off,
            1 => -off / 2.0,
            _ => 0.0,
        };
        // `FUN_0021ab10` sizes the legend to its tallest piece and puts
        // each piece's middle on the legend's point; the base draw then
        // moves them all by the legend's own anchor over that height.
        // Measured: a middle-anchored legend's pieces sit half a 24-pixel
        // icon above its point (Pause's Continue at (320, 420) and User
        // Setup's Help at (555, 67) in the original's captures).
        let tall = placed
            .iter()
            .map(|&(_, icon, text): &(f32, Rect, Option<&str>)| match text {
                None => icon.height(),
                Some(_) => font.text_height as f32,
            })
            .fold(0.0, f32::max);
        let middle = f32::from(y)
            - match (e.align >> 2) & 3 {
                1 => tall / 2.0,
                2 => tall,
                _ => 0.0,
            };
        let base = f32::from(x) + shift;
        for (at, icon, text) in placed {
            match text {
                None => {
                    let min = Vec2::new(base + at, middle - icon.height() / 2.0);
                    out.push(Piece::part(
                        atlas.clone(),
                        icon,
                        Rect::from_corners(min, min + icon.size()),
                    ));
                }
                Some(text) => {
                    let top = middle - font.line_height as f32 / 2.0;
                    if let Some(p) = labels.text(
                        look,
                        images,
                        font_index,
                        text,
                        colour,
                        None,
                        Vec2::new(base + at, top),
                        Align::Left,
                    ) {
                        out.push(p);
                    }
                }
            }
        }
    }
}

/// The entries' positions on a layout: kind-19 text elements, top to
/// bottom, `count` of them (spread further apart or squeezed to fit above
/// the help line).
fn entry_spots(layout: &Layout, count: usize, line: f32) -> Vec<Vec2> {
    let mut spots: Vec<Vec2> = if let Some(ids) = screen_registered_row_element_ids(layout.class) {
        ids.iter()
            .filter_map(|&id| {
                layout
                    .elements
                    .iter()
                    .find(|e| e.kind == 19 && e.id == id)
                    .and_then(|e| e.position())
            })
            .map(|(x, y)| Vec2::new(f32::from(x), f32::from(y)))
            .collect()
    } else {
        Vec::new()
    };
    if spots.is_empty()
        || (spots.len() <= 1 && layout.elements.iter().filter(|e| e.kind == 19).count() > 1)
    {
        spots = layout
            .elements
            .iter()
            .filter(|e| e.kind == 19)
            .filter_map(|e| e.position())
            .map(|(x, y)| Vec2::new(f32::from(x), f32::from(y)))
            .collect();
        spots.sort_by(|a, b| a.y.total_cmp(&b.y));
    }
    let Some(&first) = spots.first() else {
        return (0..count)
            .map(|k| Vec2::new(LIST_X, LIST_Y + 30.0 * k as f32))
            .collect();
    };
    let step = match spots.last() {
        Some(last) if spots.len() > 1 => (last.y - first.y) / (spots.len() - 1) as f32,
        _ => line,
    };
    let fit = if count > 1 {
        (HELP_Y - 6.0 - line - first.y) / (count - 1) as f32
    } else {
        step
    };
    let step = step.min(fit);
    (0..count)
        .map(|k| Vec2::new(first.x, first.y + step * k as f32))
        .collect()
}

/// Splits a row at runs of two or more spaces: (character column, text).
fn columns(row: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = row.char_indices().collect();
    let mut k = 0;
    while k < chars.len() {
        while k < chars.len() && chars[k].1 == ' ' {
            k += 1;
        }
        if k == chars.len() {
            break;
        }
        let start = k;
        while k < chars.len()
            && !(chars[k].1 == ' ' && chars.get(k + 1).is_some_and(|c| c.1 == ' '))
        {
            k += 1;
        }
        let end = chars.get(k).map_or(row.len(), |c| c.0);
        out.push((start, row[chars[start].0..end].trim_end()));
    }
    out
}

/// Narrows a piece toward `left` by `s` (0..1): text too wide for its room
/// even in the smallest face.
fn squeeze(p: &mut Piece, left: f32, s: f32) {
    p.rect.min.x = left + (p.rect.min.x - left) * s;
    p.rect.max.x = left + (p.rect.max.x - left) * s;
}

/// Keeps a line inside the screen's right margin.
fn keep_inside(p: &mut Piece, left: f32) {
    let right = 640.0 - 12.0;
    if p.rect.max.x > right {
        squeeze(p, left, (right - left) / (p.rect.max.x - left));
    }
}

/// The title widget's outlines (`FUN_0020f6c0`: `+0x584`/`+0x588` 3.5,
/// `+0x4b8`/`+0x4bc` 1.3).
const TITLE_OUTER: f32 = 3.5;
const TITLE_INNER: f32 = 1.3;
/// The title's text colour, which its draw sets whatever prop 10 says.
pub const TITLE_TEXT: [u8; 4] = [0xdc, 0xdc, 0xbb, 0xff];

/// A screen title (kind 35, `FUN_0020f6c0`; its loader `FUN_0020f778` takes
/// prop 23 as the outline colour `+0x4a8` and prop 24 as `+0x580`). The
/// draw `FUN_0020f7d0` draws the text twice through the text widget's draw
/// (`FUN_00238d20`, which lays its outline down as eight copies at the
/// outline's x and y offsets each way): first in prop 24's colour with a
/// 3.5-pixel outline in the same colour and no shadow, then in 0xdcdcbb
/// with a 1.3-pixel outline in prop 23's.
#[allow(clippy::too_many_arguments)]
pub(crate) fn title(
    out: &mut Vec<Piece>,
    labels: &mut Labels,
    look: &Look,
    images: &mut Assets<Image>,
    font: usize,
    s: &str,
    (inner, outer): ([u8; 4], [u8; 4]),
    at: Vec2,
    align: u8,
) {
    let mut w = crate::hud::Writer {
        labels,
        look,
        images,
    };
    let first = out.len();
    for (colour, outline, offset) in [
        (outer, outer, TITLE_OUTER),
        (TITLE_TEXT, inner, TITLE_INNER),
    ] {
        let style = crate::hud::TextStyle {
            outline: Some((outline, Vec2::splat(offset))),
            ..crate::hud::TextStyle::plain(font, colour)
        };
        w.text(out, s, style, at, align, 1.0);
    }
    for p in &mut out[first..] {
        p.role = Role::Title;
    }
}

/// The pieces of a page, back to front.
/// Lines of small print under a list page's rows, the last ending at the
/// lists' bottom: the project's notice on PC Settings.
pub fn notes(
    look: &Look,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    lines: &[&str],
) -> Vec<Piece> {
    let Some(f) = look.fonts.get(fonts::SMALL) else {
        return Vec::new();
    };
    // The lists' own pitch (`Style::List`'s rows).
    let line = (f.line_height as f32 * 0.86).round();
    let top = LIST_BOTTOM - line * lines.len() as f32;
    lines
        .iter()
        .enumerate()
        .filter_map(|(k, l)| {
            labels.text(
                look,
                images,
                fonts::SMALL,
                l,
                colours::DIM,
                None,
                Vec2::new(LIST_X, top + line * k as f32),
                Align::Left,
            )
        })
        .collect()
}

pub fn build(
    view: &View,
    look: &Look,
    labels: &mut Labels,
    images: &mut Assets<Image>,
) -> Vec<Piece> {
    labels.tidy();
    let mut out = Vec::new();
    let text = |out: &mut Vec<Piece>,
                labels: &mut Labels,
                images: &mut Assets<Image>,
                font: usize,
                s: &str,
                colour,
                shadow,
                at,
                align| {
        if let Some(p) = labels.text(look, images, font, s, colour, shadow, at, align) {
            out.push(p);
        }
    };
    let layout = match view.style {
        Style::Layout(n) => look.layouts.get(n),
        Style::Screen(class) => look.layouts.iter().find(|l| l.class == class),
        _ => None,
    };
    if let (Some(layout), Some(bar)) = (layout, &look.segmented_bar) {
        for element in layout.elements.iter().filter(|e| e.kind == 31) {
            let (Some((x, y)), Some((width, _))) = (element.position(), element.size()) else {
                continue;
            };
            segmented_bar(
                &mut out,
                bar,
                Vec2::new(f32::from(x), f32::from(y)),
                f32::from(width),
            );
        }
    }
    // The screen's title widget: the first kind-35 element with words
    // (Rewards' layout 20 puts a wordless one, the close-up's card name,
    // first).
    let title_element = layout.and_then(|l| {
        l.elements
            .iter()
            .find(|e| e.kind == 35 && !e.texts().is_empty())
            .or_else(|| l.elements.iter().find(|e| e.kind == 35))
    });
    let title_at = title_element
        .and_then(|e| e.position())
        .map_or(TITLE_AT, |(x, y)| Vec2::new(f32::from(x), f32::from(y)));
    let shadow = if let Some(e) = title_element
        && let (Some(&top), Some(&bottom)) = (e.props.get(&23), e.props.get(&24))
    {
        (font::argb(top), font::argb(bottom))
    } else if view.warm {
        colours::WARM
    } else {
        colours::COOL
    };
    let width = 640.0 - title_at.x - 20.0;
    // The title: the script face when it fits.
    if !view.title.is_empty() {
        let font = [fonts::TITLE, fonts::TITLE_SMALL, fonts::ENTRY, fonts::LIST]
            .into_iter()
            .find(|&f| {
                look.fonts
                    .get(f)
                    .is_some_and(|f| f.measure(view.title) as f32 <= width)
            })
            .unwrap_or(fonts::LIST);
        title(
            &mut out,
            labels,
            look,
            images,
            font,
            view.title,
            shadow,
            title_at,
            title_element.map_or(0, |e| e.align),
        );
    }
    let mut bullets = Vec::new();
    let mut row_stars = Vec::new();
    match view.style {
        Style::Title { demo } => {
            // The attract's screen (class 99, `FUN_002d3980`) over its
            // billboard (backdrop 27): "Press START button"
            // (`titlescreen_ps2.txt` 0, font 2, centred on (320, 338),
            // pulsing) between two stars at 1.35 (12 left of it, 10 right),
            // and the copyright (1, font 0, 0xa0a0a0 over black 1.5 down
            // and right) from (320, 360). When the idle demo's timer runs
            // out the same widget (0xe) says "Loading Demo..."
            // (`main_help.txt` 6) and the stars (0x14, 0x15) hide
            // (`FUN_002d3d68`).
            let press = if demo {
                look.text("main_help.txt", 6).unwrap_or("Loading Demo...")
            } else {
                look.text("titlescreen_ps2.txt", 0)
                    .unwrap_or("Press START button")
            };
            if let Some(f) = look.fonts.get(fonts::LIST) {
                let (h, width) = (f.text_height as f32, f.measure(press) as f32);
                let middle = 338.0;
                if let Some(mut p) = labels.text(
                    look,
                    images,
                    fonts::LIST,
                    press,
                    colours::ROW_LIT,
                    None,
                    Vec2::new(320.0, middle - h / 2.0),
                    Align::Centre,
                ) {
                    p.colour =
                        Color::srgba(1.0, 1.0, 1.0, f32::from(row_pulse(view.ticks)) / 255.0);
                    out.push(p);
                }
                let (star, size) = look.row_star();
                let size = size * 1.35;
                let stars = if demo {
                    Vec::new()
                } else {
                    vec![
                        320.0 - width / 2.0 - 12.0 - size.x,
                        320.0 + width / 2.0 + 10.0,
                    ]
                };
                for x in stars {
                    let min = Vec2::new(x, middle - size.y / 2.0);
                    out.push(Piece::new(
                        star.clone(),
                        Rect::from_corners(min, min + size),
                    ));
                }
            }
            let copyright = look.text("titlescreen_ps2.txt", 1).unwrap_or("");
            text(
                &mut out,
                labels,
                images,
                0,
                copyright,
                [0xa0, 0xa0, 0xa0, 0xff],
                Some(([0, 0, 0, 0xff], [0, 0, 0, 0xff])),
                Vec2::new(320.0, 360.0),
                Align::Centre,
            );
        }
        Style::Layout(n) => {
            let line = look.fonts[fonts::ENTRY].line_height as f32;
            let spots = look
                .layouts
                .get(n)
                .map(|l| entry_spots(l, view.items.len(), line))
                .unwrap_or_default();
            for (k, (item, at)) in view.items.iter().zip(spots).enumerate() {
                let lit = k == view.cursor;
                let colour = if lit {
                    colours::SELECTED
                } else {
                    colours::CREAM
                };
                text(
                    &mut out,
                    labels,
                    images,
                    fonts::ENTRY,
                    item,
                    colour,
                    None,
                    at,
                    Align::Left,
                );
                // The bullet sits 60 to the left, on the entry's middle.
                bullets.push((Vec2::new(at.x - 60.0, at.y + line * 0.45), lit));
            }
        }
        Style::Screen(_) if layout.and_then(screen_rows).is_some() => {
            let (Some(layout), Some(rows)) = (layout, layout.and_then(screen_rows)) else {
                unreachable!()
            };
            let stars_shown = screen_binding(layout.class).is_none_or(|b| b.stars);
            for (k, item) in view.items.iter().enumerate() {
                if let Some(Some(spot)) = rows.get(k) {
                    screen_row(
                        &mut out,
                        &mut row_stars,
                        labels,
                        look,
                        images,
                        *spot,
                        item,
                        k == view.cursor,
                        stars_shown,
                        view.ticks,
                    );
                }
            }
            legends(&mut out, labels, look, images, (layout, view.title));
        }
        Style::Screen(61 | 79) => {
            // Create Baller and Edit Baller: their panels draw the legends
            // with their own words (`cab_view.rs`).
        }
        Style::Screen(
            7 | 9 | 10 | 11 | 26 | 29 | 33 | 40 | 41 | 48..=51 | 53 | 54 | 56 | 59 | 60 | 77 | 96,
        ) => {
            // User Setup, Choose Team, Choose Court, Choose Baller, the
            // completion boards and the credits: the screens draw their own
            // widgets (`users.rs`, `chooseteam.rs`, `choosecourt.rs`,
            // `careers.rs`, `completed_view.rs`, `credits.rs`); here the
            // layout's button legends (kind 34).
            if let Some(layout) = layout {
                legends(&mut out, labels, look, images, (layout, view.title));
            }
        }
        Style::Screen(_) => {
            if let Some(layout) = layout {
                let registered_ids = screen_registered_row_element_ids(layout.class);
                let (entries, dominant_tag): (Vec<&font::Element>, Option<u32>) = if let Some(ids) =
                    registered_ids
                {
                    let mut matched = Vec::new();
                    for &id in ids {
                        if let Some(e) = layout.elements.iter().find(|e| e.kind == 19 && e.id == id)
                        {
                            matched.push(e);
                        }
                    }
                    if matched.len() <= 1
                        && layout.elements.iter().filter(|e| e.kind == 19).count() > 1
                    {
                        let mut e: Vec<_> = layout
                            .elements
                            .iter()
                            .filter(|e| e.kind == 19 && e.position().is_some())
                            .collect();
                        e.sort_by_key(|e| e.position().map(|(x, y)| (y, x)));
                        (e, None)
                    } else {
                        let tag = match layout.class {
                            3 => Some(19),
                            5 => Some(0),
                            6 => Some(5),
                            56 => Some(0),
                            102..=104 => Some(0),
                            _ => None,
                        };
                        (matched, tag)
                    }
                } else {
                    let mut e: Vec<_> = layout
                        .elements
                        .iter()
                        .filter(|e| e.kind == 19 && e.position().is_some())
                        .collect();
                    e.sort_by_key(|e| e.position().map(|(x, y)| (y, x)));
                    (e, None)
                };
                for (k, item) in view.items.iter().enumerate() {
                    let Some(e) = entries.get(k).or_else(|| entries.last()) else {
                        continue;
                    };
                    let (x, y) = e.position().unwrap();
                    let font = e.props.get(&13).copied().unwrap_or(fonts::ENTRY as u32) as usize;
                    let line = look.fonts.get(font).map_or(20.0, |f| f.line_height as f32);
                    let node = layout.elements.iter().find(|node| {
                        node.kind == 10
                            && node.props.get(&34) == Some(&(k as u32))
                            && dominant_tag.is_none_or(|tag| node.props.get(&33) == Some(&tag))
                    });
                    let (pos_x, pos_y) = if entries.get(k).is_some() {
                        (f32::from(x), f32::from(y))
                    } else if let Some(n) = node.and_then(|n| n.position()) {
                        (f32::from(x), f32::from(n.1) - 6.0)
                    } else {
                        let extra = k.saturating_sub(entries.len().saturating_sub(1));
                        let calc_y = f32::from(y) + extra as f32 * line;
                        if calc_y > 380.0 && !entries.is_empty() {
                            let (first_x, first_y) = entries[0].position().unwrap();
                            (
                                f32::from(first_x) + 160.0,
                                f32::from(first_y) + (extra - 1) as f32 * line,
                            )
                        } else {
                            (f32::from(x), calc_y)
                        }
                    };
                    let at = Vec2::new(pos_x, pos_y);
                    let lit = k == view.cursor;
                    let colour = if lit {
                        colours::SELECTED
                    } else {
                        e.colour().unwrap_or(colours::CREAM)
                    };
                    let align = match e.align & 3 {
                        1 => Align::Centre,
                        2 => Align::Right,
                        _ => Align::Left,
                    };
                    text(
                        &mut out, labels, images, font, item, colour, None, at, align,
                    );
                    // The star is the row widget's own arrow child
                    // (`FUN_0021c628`), so it follows the row's text; the
                    // navigation node gives only its column. On Main and
                    // Inside the nodes sit within a few pixels of the text,
                    // on Play It they do not (nodes 44 apart, rows 64), and
                    // stars at the nodes missed their rows. Centred on the row (the
                    // arrow's own offset is not traced).
                    let centre = node
                        .and_then(|e| e.position())
                        .map_or(Vec2::new(at.x - 40.0, at.y + line * 0.45), |(x, _)| {
                            Vec2::new(f32::from(x), at.y + line * 0.45)
                        });
                    bullets.push((centre, lit));
                }
            }
        }
        Style::List | Style::Read => {
            let room = 640.0 - view.left - 16.0;
            // Rows split at runs of spaces into columns; each column starts
            // clear of the widest entry before it in the rows that have it,
            // so tables and forms ("Label  < value >") line up.
            let split: Vec<Vec<(usize, &str)>> = view.items.iter().map(|s| columns(s)).collect();
            let place = |f: &Font| -> (Vec<f32>, f32) {
                let gap = f.measure("0") as f32 * 1.5;
                let mut col_x = vec![0.0f32];
                for j in 1.. {
                    let start = split
                        .iter()
                        .filter(|row| row.len() > j)
                        .map(|row| col_x[j - 1] + f.measure(row[j - 1].1) as f32 + gap)
                        .reduce(f32::max);
                    match start {
                        Some(x) => col_x.push(x),
                        None => break,
                    }
                }
                let width = split
                    .iter()
                    .filter_map(|row| {
                        row.last()
                            .map(|l| col_x[row.len() - 1] + f.measure(l.1) as f32)
                    })
                    .fold(0.0, f32::max);
                (col_x, width)
            };
            // The largest list font that fits the widest row; past the
            // smallest, the rows narrow to fit.
            let (font, (col_x, width)) = [fonts::LIST, fonts::LIST_SMALL, fonts::SMALL]
                .into_iter()
                .filter_map(|f| look.fonts.get(f).map(|ff| (f, place(ff))))
                .find(|(_, (_, width))| *width <= room)
                .or_else(|| {
                    look.fonts
                        .get(fonts::SMALL)
                        .map(|ff| (fonts::SMALL, place(ff)))
                })
                .unwrap_or((fonts::SMALL, (vec![0.0], 0.0)));
            let narrow = if width > room { room / width } else { 1.0 };
            let from = out.len();
            let line = (look.fonts[font].line_height as f32 * 0.86).round();
            let rows = (((LIST_BOTTOM - LIST_Y) / line) as usize).max(1);
            let first = view
                .cursor
                .saturating_sub(rows / 2)
                .min(view.items.len().saturating_sub(rows));
            for (k, item) in split.iter().enumerate().skip(first).take(rows) {
                let y = LIST_Y + line * (k - first) as f32;
                let last = k + 1 == view.items.len();
                let lit = k == view.cursor && (view.style == Style::List || last);
                let colour = match (lit, view.style) {
                    (true, _) => colours::SELECTED,
                    (false, Style::Read) => colours::DIM,
                    _ => colours::CREAM,
                };
                for (j, &(_, part)) in item.iter().enumerate() {
                    let at = Vec2::new(view.left + col_x[j], y);
                    if let (Some((level, max)), Some(art)) = (parse_bar(part), &look.native_slider)
                    {
                        let top = at.y + (line - art.track.height()) / 2.0;
                        native_slider(&mut out, art, Vec2::new(at.x, top), level, max);
                        continue;
                    }
                    text(
                        &mut out,
                        labels,
                        images,
                        font,
                        part,
                        colour,
                        None,
                        at,
                        Align::Left,
                    );
                }
                if view.style == Style::List || last {
                    bullets.push((Vec2::new(view.left - 22.0, y + line * 0.5), lit));
                }
            }
            if narrow < 1.0 {
                for p in &mut out[from..] {
                    squeeze(p, view.left, narrow);
                }
            }
            // More above or below: the list continues.
            if first > 0 {
                text(
                    &mut out,
                    labels,
                    images,
                    fonts::SMALL,
                    "...",
                    colours::DIM,
                    None,
                    Vec2::new(view.left, LIST_Y - 18.0),
                    Align::Left,
                );
            }
            if first + rows < view.items.len() {
                text(
                    &mut out,
                    labels,
                    images,
                    fonts::SMALL,
                    "...",
                    colours::DIM,
                    None,
                    Vec2::new(view.left, LIST_BOTTOM),
                    Align::Left,
                );
            }
        }
    }
    let from = out.len();
    // The help line is the layout's kind-5 text (`FUN_0021af18`): font 1
    // (its loader `FUN_0021afa8` passes 1 to `FUN_002e5578`, whatever the
    // element's prop 13 says) in 0xffc99b0d, at the element's point and
    // anchor; its lines break where the text does. Pages without a layout
    // put it where the layouts do, (58, 420).
    let mut bottom = HELP_Y;
    if !view.help.is_empty()
        && let Some(f) = look.fonts.get(HELP_FONT)
    {
        let e = layout.and_then(|l| l.elements.iter().find(|e| e.kind == 5));
        let at = e
            .and_then(|e| e.position())
            .map_or(Vec2::new(HELP_X, HELP_Y), |(x, y)| {
                Vec2::new(f32::from(x), f32::from(y))
            });
        let lines: Vec<&str> = view.help.lines().collect();
        let line = f.line_height as f32;
        let height = line * lines.len() as f32;
        let align = e.map_or(0, |e| e.align);
        let top = at.y
            - match (align >> 2) & 3 {
                1 => height / 2.0,
                2 => height,
                _ => 0.0,
            };
        let h = match align & 3 {
            1 => Align::Centre,
            2 => Align::Right,
            _ => Align::Left,
        };
        for (k, l) in lines.iter().enumerate() {
            text(
                &mut out,
                labels,
                images,
                HELP_FONT,
                l,
                colours::HELP_LINE,
                None,
                Vec2::new(at.x, top + line * k as f32),
                h,
            );
        }
        bottom = top + height;
    }
    // The rewrite's own key prompt, under the help: a screen that draws its
    // own button legends (kind 34) has the original's instead.
    let own_legends = matches!(
        view.style,
        Style::Screen(11 | 26 | 29 | 33 | 41 | 49..=51 | 53 | 54 | 56 | 96)
    ) || (matches!(view.style, Style::Screen(_))
        && layout.and_then(screen_rows).is_some());
    if !view.keys.is_empty() && !own_legends {
        text(
            &mut out,
            labels,
            images,
            fonts::SMALL,
            view.keys,
            colours::LEGEND,
            None,
            Vec2::new(HELP_X, bottom.max(444.0) + 2.0),
            Align::Left,
        );
    }
    for p in &mut out[from..] {
        keep_inside(p, HELP_X);
    }
    out.extend(row_stars);
    for (centre, lit) in bullets {
        let (image, size) = if lit {
            (look.star_lit.clone(), look.star_lit_size)
        } else {
            (look.star.clone(), look.star_size)
        };
        let min = centre - size / 2.0;
        out.push(Piece::new(image, Rect::from_corners(min, min + size)));
    }
    out
}

/// The game's slider filling `rect` (175x19 at full size): its frame, and
/// `max` ticks along it, the first `level` lit.
fn slider(out: &mut Vec<Piece>, art: &[Handle<Image>; 3], rect: Rect, level: u8, max: u8) {
    let s = rect.width() / 175.0;
    out.push(Piece::new(art[0].clone(), rect));
    let step = (175.0 - 18.0) * s / f32::from(max.max(1));
    let (w, h) = (8.0 * s, 10.0 * s);
    let top = rect.min.y + (rect.height() - h) / 2.0;
    for k in 0..max {
        let x = rect.min.x + 9.0 * s + step * f32::from(k) + (step - w) / 2.0;
        let tick = if k < level { &art[2] } else { &art[1] };
        out.push(Piece::new(tick.clone(), Rect::new(x, top, x + w, top + h)));
    }
}

/// Kind 6: FUN_00237948 computes value/max * 170; FUN_0021d410
/// changes the end/middle visibility at widths 4 and 8. Skill meters use
/// their separate tick renderer below.
fn native_slider(out: &mut Vec<Piece>, art: &NativeSlider, at: Vec2, level: u8, max: u8) {
    native_slider_in(out, art, at, level, max, (art.end, art.middle));
}

/// [`native_slider`] filled with `fill`'s end and middle frames.
fn native_slider_in(
    out: &mut Vec<Piece>,
    art: &NativeSlider,
    at: Vec2,
    level: u8,
    max: u8,
    (end, middle): (Rect, Rect),
) {
    let piece = |source: Rect, offset: Vec2, width: f32| {
        Piece::part(
            art.atlas.clone(),
            source,
            Rect::from_corners(at + offset, at + offset + Vec2::new(width, source.height())),
        )
    };
    out.push(piece(art.track, Vec2::ZERO, art.track.width()));
    let fill = f32::from(level.min(max)) / f32::from(max.max(1)) * 170.0;
    out.push(piece(end, Vec2::new(6.0, 3.0), fill.min(4.0)));
    if fill <= 4.0 {
        return;
    }
    let right_x = if fill > 8.0 {
        let width = (fill - 8.0).round_ties_even();
        out.push(piece(middle, Vec2::new(9.0, 3.0), width));
        9.0 + width - 1.0
    } else {
        6.0 + fill - 4.0
    };
    let mut right = piece(end, Vec2::new(right_x, 3.0), 4.0);
    right.flip = true;
    out.push(right);
}

impl Look {
    /// The pause screen's volume slider (`FUN_0021d148`, `0x48e240`): the
    /// slider's art centred on `centre` (the widget is centred on its
    /// node, `FUN_0024a4e8(0x20)`), `level` of `max` filled.
    /// A highlighted row's fills amber (frames 2 and 3).
    pub(crate) fn slider_at(
        &self,
        out: &mut Vec<Piece>,
        centre: Vec2,
        level: u8,
        max: u8,
        lit: bool,
    ) {
        if let Some(art) = &self.native_slider {
            let at = centre - art.track.size() / 2.0;
            let fill = match art.lit {
                Some(amber) if lit => amber,
                _ => (art.end, art.middle),
            };
            native_slider_in(out, art, at, level, max, fill);
        }
    }
}

/// Draw kind 31's authored end, stretched middle and mirrored end. The two
/// pixels of overlap at each join are part of `FUN_00218a80`'s placement.
fn segmented_bar(out: &mut Vec<Piece>, art: &SegmentedBar, at: Vec2, width: f32) {
    let inset = art.end_width - 2.0;
    let middle_width = width - 2.0 * inset;
    if width <= 0.0 || middle_width < 0.0 {
        return;
    }
    let height = art.height;
    let end_size = Vec2::new(art.end_width, height);
    out.push(Piece::new(
        art.end.clone(),
        Rect::from_corners(at, at + end_size),
    ));
    let middle_at = at + Vec2::new(inset, 0.0);
    out.push(Piece::new(
        art.middle.clone(),
        Rect::from_corners(middle_at, middle_at + Vec2::new(middle_width, height)),
    ));
    let right_at = at + Vec2::new(width - art.end_width, 0.0);
    let mut right = Piece::new(
        art.end.clone(),
        Rect::from_corners(right_at, right_at + end_size),
    );
    right.flip = true;
    out.push(right);
}

/// A meter of `max` of the slider's ticks at their own size, `level` lit,
/// spread across `rect` (the choose-player screen's 60x25 skill boxes,
/// layout kind 20).
pub fn meter(look: &Look, rect: Rect, level: u8, max: u8) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(art) = &look.slider else {
        return out;
    };
    let pitch = rect.width() / f32::from(max.max(1));
    let (w, h) = (8.0, 10.0);
    let top = rect.min.y + (rect.height() - h) / 2.0;
    for k in 0..max {
        let x = rect.min.x + pitch * f32::from(k) + (pitch - w) / 2.0;
        let tick = if k < level { &art[2] } else { &art[1] };
        out.push(Piece::new(tick.clone(), Rect::new(x, top, x + w, top + h)));
    }
    out
}

/// A panel of named sliders from `at` down, `width` wide: each row its
/// name above its slider, `level` of 10 ticks lit (the choose-player
/// screen's skill read-out, in the game's slider art and the layout's
/// label colour, 0x8394df).
pub fn bars(
    look: &Look,
    labels: &mut Labels,
    images: &mut Assets<Image>,
    at: Vec2,
    width: f32,
    rows: &[(String, u8)],
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(art) = &look.slider else {
        return out;
    };
    let h = 19.0 * width / 175.0;
    let pitch = 18.0 + h + 6.0;
    for (k, (name, level)) in rows.iter().enumerate() {
        let y = at.y + pitch * k as f32;
        if let Some(p) = labels.text(
            look,
            images,
            fonts::SMALL,
            name,
            [0x83, 0x94, 0xdf, 0xff],
            None,
            Vec2::new(at.x, y),
            Align::Left,
        ) {
            out.push(p);
        }
        let rect = Rect::new(at.x, y + 18.0, at.x + width, y + 18.0 + h);
        slider(&mut out, art, rect, *level, 10);
    }
    out
}

/// One UI node showing a [`Piece`]; spare nodes hide.
#[derive(Component)]
pub struct Slot(pub usize);

/// Points the slots at this frame's pieces, spawning more when short.
pub fn show<F: QueryFilter>(
    commands: &mut Commands,
    slots: &mut Query<(&Slot, &mut ImageNode, &mut Node, &mut Visibility), F>,
    pieces: &[Piece],
    despawn_on: crate::Screen,
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
            // Above the backdrop, which may arrive later, and in the
            // pieces' order (slots spawned later may get older entity
            // ids, so equal indices would stack at random).
            GlobalZIndex(5 + k as i32),
            Slot(k),
            DespawnOnExit(despawn_on),
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use street_assets::font::Element;

    use super::*;

    #[test]
    fn rows_split_at_runs_of_spaces() {
        assert_eq!(
            columns("Game Mode  < Normal >"),
            vec![(0, "Game Mode"), (11, "< Normal >")]
        );
        assert_eq!(columns("  Back"), vec![(2, "Back")]);
        assert_eq!(
            columns("Height  < 5' 9\" >  "),
            vec![(0, "Height"), (8, "< 5' 9\" >")]
        );
        assert!(columns("   ").is_empty());
    }

    fn entry(x: u16, y: u16) -> Element {
        Element {
            kind: 19,
            id: 0,
            align: 0,
            props: BTreeMap::from([(3, u32::from(x) | u32::from(y) << 16)]),
        }
    }

    /// The main menu's five entries keep their spacing; seven squeeze in
    /// above the help line.
    #[test]
    fn entries_follow_the_layout() {
        let layout = Layout {
            class: 3,
            elements: [143, 184, 230, 271, 312].map(|y| entry(372, y)).to_vec(),
        };
        let five = entry_spots(&layout, 5, 43.0);
        assert_eq!(five[0], Vec2::new(372.0, 143.0));
        assert_eq!(five[4].y, 312.0);
        let seven = entry_spots(&layout, 7, 43.0);
        assert!(seven[6].y + 43.0 <= HELP_Y);
        assert!(seven[1].y - seven[0].y < 42.25);
    }

    /// A navigation node as the layouts author them: up and down in mode
    /// 2 (follow the link); a missing mode reads as 0, no step
    /// (`FUN_002e3b18`).
    fn node(id: u32, row: u32, tag: u32, up: u32, down: u32) -> Element {
        Element {
            kind: 10,
            id,
            align: 0,
            props: BTreeMap::from([(25, 2), (26, up), (27, 2), (28, down), (33, tag), (34, row)]),
        }
    }

    /// Set Challenge (class 39) binds its five rows to list 38's nodes in
    /// the build's order (37, 39, 41, 40, 36: Game Mode, Play To, Spot
    /// Points, Shot Clock, Play Game), the last right-aligned, and Play
    /// Game's node links down to Game Mode's.
    #[test]
    fn set_challenge_rows_bind_to_list_38_in_registration_order() {
        let at = |x: u32, y: u32| (3u16, x | (y << 16));
        let mut elements = Vec::new();
        for (k, (id, y, up)) in [
            (30u32, 104u32, 34u32),
            (31, 169, 30),
            (32, 243, 31),
            (33, 315, 32),
        ]
        .into_iter()
        .enumerate()
        {
            let mut n = node(id, k as u32, 38, up, id + 1);
            n.props.insert(3, 151 | (y << 16));
            elements.push(n);
        }
        let mut last = node(34, 4, 38, 33, 30);
        last.props.insert(3, 395 | (374 << 16));
        elements.push(last);
        for (id, align, x, y) in [
            (37u32, 1u8, 187u32, 118u32),
            (39, 1, 185, 175),
            (41, 1, 189, 253),
            (40, 1, 189, 323),
            (36, 2, 349, 385),
        ] {
            elements.push(Element {
                kind: 19,
                id,
                align,
                props: BTreeMap::from([(13, 3), at(x, y)]),
            });
        }
        let layout = Layout {
            class: 39,
            elements,
        };
        let rows = screen_rows(&layout).unwrap();
        let spots: Vec<(f32, f32, Align)> = rows
            .iter()
            .flatten()
            .map(|r| (r.at.x, r.at.y, r.align))
            .collect();
        assert_eq!(
            spots,
            [
                (151.0, 104.0, Align::Centre),
                (151.0, 169.0, Align::Centre),
                (151.0, 243.0, Align::Centre),
                (151.0, 315.0, Align::Centre),
                (395.0, 374.0, Align::Right),
            ]
        );
        assert!(rows.iter().flatten().all(|r| r.font == 3 && r.valign == 0));
        assert_eq!(
            screen_step(&layout, 4, 5, 1),
            Some(0),
            "Play Game wraps to Game Mode"
        );
        assert_eq!(screen_step(&layout, 0, 5, -1), Some(4));
    }

    /// A row's stars sit on the middle of the text's box as the text
    /// widget measures it (`FUN_00239e70`): the font's text height, not
    /// its line spacing.
    #[test]
    fn row_stars_sit_on_the_text_height_middle() {
        let mut look = Look::empty();
        look.fonts.push(Font {
            line_height: 37,
            text_height: 29,
            width: 1,
            height: 1,
            coverage: vec![0],
            glyphs: BTreeMap::new(),
            kerning: HashMap::new(),
        });
        look.row_star_size = Vec2::new(19.0, 18.0);
        let mut images = Assets::<Image>::default();
        let mut labels = Labels::default();
        let (mut out, mut stars) = (Vec::new(), Vec::new());
        let spot = RowSpot {
            at: Vec2::new(300.0, 100.0),
            font: 0,
            align: Align::Left,
            valign: 0,
        };
        screen_row(
            &mut out,
            &mut stars,
            &mut labels,
            &look,
            &mut images,
            spot,
            "A",
            true,
            true,
            0,
        );
        assert_eq!(stars.len(), 2);
        let middle = stars[0].rect.center().y;
        assert_eq!(middle, 100.0 + 29.0 / 2.0);
        // A middle-anchored row's text top is half the text height up.
        let (mut out, mut stars) = (Vec::new(), Vec::new());
        screen_row(
            &mut out,
            &mut stars,
            &mut labels,
            &look,
            &mut images,
            RowSpot { valign: 1, ..spot },
            "A",
            true,
            true,
            0,
        );
        assert_eq!(stars[0].rect.center().y, 100.0);
    }

    /// Up/down follow the authored links, stay at dead ends, and fall back
    /// past the authored rows; a second prop-33 group (map landmarks) loses
    /// to the dominant list.
    #[test]
    fn navigation_flags_disable_or_scroll_instead_of_following_links() {
        let mut start = node(6, 0, 0, u32::MAX, 7);
        start.props.insert(27, 0);
        let mut layout = Layout {
            class: 102,
            elements: vec![start, node(7, 1, 0, 6, u32::MAX)],
        };
        assert_eq!(screen_step(&layout, 0, 2, 1), Some(0));
        layout.elements[0].props.insert(27, 1);
        assert_eq!(screen_step(&layout, 0, 2, 1), None);
        layout.elements[0].props.insert(27, 2);
        assert_eq!(screen_step(&layout, 0, 2, 1), Some(1));
        layout.elements[0].props.insert(28, 999);
        assert_eq!(screen_step(&layout, 0, 2, 1), Some(0));
    }

    #[test]
    fn native_option_slider_matches_small_fill_boundaries_and_full_width() {
        let art = NativeSlider {
            atlas: Handle::default(),
            track: Rect::new(3.0, 44.0, 179.0, 64.0),
            end: Rect::new(7.0, 7.0, 11.0, 18.0),
            middle: Rect::new(14.0, 7.0, 16.0, 18.0),
            lit: None,
        };
        for (level, count, end_x) in [
            (0, 2, None),
            (4, 2, None),
            (8, 3, Some(10.0)),
            (17, 4, Some(17.0)),
            (170, 4, Some(170.0)),
        ] {
            let mut out = Vec::new();
            native_slider(&mut out, &art, Vec2::ZERO, level, 170);
            assert_eq!(out.len(), count);
            assert_eq!(out[0].rect, Rect::new(0.0, 0.0, 176.0, 20.0));
            assert_eq!(out[1].rect.width(), f32::from(level).min(4.0));
            if let Some(x) = end_x {
                assert_eq!(out.last().unwrap().rect.min.x, x);
                assert!(out.last().unwrap().flip);
            }
        }
        // Native default FCSR round-to-nearest-even for a stretched middle.
        let mut out = Vec::new();
        native_slider(&mut out, &art, Vec2::ZERO, 1, 16);
        assert_eq!(out[2].rect.width(), 3.0);
    }

    #[test]
    fn native_slider_preserves_child_widths_across_shrink_then_grow_lifecycle() {
        let art = NativeSlider {
            atlas: Handle::default(),
            track: Rect::new(3.0, 44.0, 179.0, 64.0),
            end: Rect::new(7.0, 7.0, 11.0, 18.0),
            middle: Rect::new(14.0, 7.0, 16.0, 18.0),
            lit: None,
        };

        // 1. Initial nominal level (e.g. 6/10)
        let mut out = Vec::new();
        native_slider(&mut out, &art, Vec2::ZERO, 6, 10);
        assert_eq!(out.len(), 4);
        assert_eq!(out[1].rect.width(), 4.0); // left end full width

        // 2. Shrink to zero (fill <= 4.0 threshold: child 1 shrinks to 0.0, middle/right hidden)
        let mut out = Vec::new();
        native_slider(&mut out, &art, Vec2::ZERO, 0, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].rect.width(), 0.0);

        // 3. Grow back to 1/10 (fill = 17.0 > 8.0: left end restored to 4.0, middle & right visible)
        let mut out = Vec::new();
        native_slider(&mut out, &art, Vec2::ZERO, 1, 10);
        assert_eq!(out.len(), 4);
        assert_eq!(out[1].rect.width(), 4.0);
        assert_eq!(out[2].rect.width(), 9.0);
        assert_eq!(out[3].rect.min.x, 17.0);

        // 4. Grow to max 10/10 (fill = 170.0: left end 4.0, middle 162.0, right at 170.0)
        let mut out = Vec::new();
        native_slider(&mut out, &art, Vec2::ZERO, 10, 10);
        assert_eq!(out.len(), 4);
        assert_eq!(out[1].rect.width(), 4.0);
        assert_eq!(out[2].rect.width(), 162.0);
        assert_eq!(out[3].rect.min.x, 170.0);
    }

    #[test]
    fn segmented_menu_bar_stretches_the_middle_and_mirrors_the_far_end() {
        let art = SegmentedBar {
            end: Handle::default(),
            middle: Handle::default(),
            end_width: 11.0,
            height: 25.0,
        };
        let mut pieces = Vec::new();
        segmented_bar(&mut pieces, &art, Vec2::new(10.0, 20.0), 50.0);

        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].rect, Rect::new(10.0, 20.0, 21.0, 45.0));
        assert_eq!(pieces[1].rect, Rect::new(19.0, 20.0, 51.0, 45.0));
        assert_eq!(pieces[2].rect, Rect::new(49.0, 20.0, 60.0, 45.0));
        assert!(pieces[2].flip);
    }

    #[test]
    fn steps_follow_the_navigation_graph() {
        let layout = Layout {
            class: 102,
            elements: vec![
                node(6, 0, 0, u32::MAX, 7),
                node(7, 1, 0, 6, 8),
                node(8, 2, 0, 7, 9),
                node(9, 3, 0, 8, 9),
            ],
        };
        assert_eq!(screen_step(&layout, 0, 4, 1), Some(1));
        assert_eq!(screen_step(&layout, 1, 4, -1), Some(0));
        assert_eq!(screen_step(&layout, 0, 4, -1), Some(0));
        assert_eq!(screen_step(&layout, 3, 4, 1), Some(3));
        assert_eq!(screen_step(&layout, 3, 2, 1), None);
        assert_eq!(screen_step(&layout, 4, 6, 1), None);
        assert_eq!(screen_step(&layout, 0, 0, 1), None);
        // The map screen's landmark nodes (prop 33 == 0) must not capture
        // the read-out list (prop 33 == 26): the landmark id 4 sits first
        // in layout order with the same prop-34 row as list node 22.
        let mixed = Layout {
            class: 56,
            elements: vec![
                node(4, 1, 0, u32::MAX, 5),
                node(21, 0, 26, u32::MAX, 22),
                node(22, 1, 26, 21, 23),
                node(23, 2, 26, 22, u32::MAX),
                node(5, 2, 0, 4, u32::MAX),
            ],
        };
        assert_eq!(screen_step(&mixed, 0, 8, 1), Some(1));
        assert_eq!(screen_step(&mixed, 1, 8, 1), Some(2));
        assert_eq!(screen_step(&mixed, 1, 8, -1), Some(0));
        assert_eq!(screen_step(&mixed, 2, 8, 1), Some(2));
    }

    #[test]
    fn dynamic_navigation_recursively_skips_unavailable_widgets() {
        let layout = Layout {
            class: 102,
            elements: vec![
                node(6, 0, 0, u32::MAX, 7),
                node(7, 1, 0, 6, 8),
                node(8, 2, 0, 7, 9),
                node(9, 3, 0, 8, u32::MAX),
            ],
        };

        // When row 1 is unavailable, stepping down from 0 skips row 1 and lands on row 2.
        assert_eq!(
            screen_step_filtered(&layout, 0, 4, 1, |row| row != 1),
            Some(2)
        );

        // When stepping up from 2 with row 1 unavailable, it lands on 0.
        assert_eq!(
            screen_step_filtered(&layout, 2, 4, -1, |row| row != 1),
            Some(0)
        );

        // When both rows 1 and 2 are unavailable, stepping down from 0 lands on row 3.
        assert_eq!(
            screen_step_filtered(&layout, 0, 4, 1, |row| row != 1 && row != 2),
            Some(3)
        );

        // When all downstream rows are unavailable, it remains at cursor 0.
        assert_eq!(
            screen_step_filtered(&layout, 0, 4, 1, |row| row == 0),
            Some(0)
        );
    }

    /// `FUN_00222590`/`FUN_00222678` force mode 2 and take the direction
    /// from a bound kind-36 widget's byte +0xb0, never from a node's own
    /// prop: a node whose up mode is 0 takes no step, whatever prop 36
    /// says (no layout authors kind 36 and none of these screens binds
    /// one).
    #[test]
    fn a_disabled_mode_takes_no_step_and_node_props_carry_no_kind36_override() {
        let mut start = node(6, 0, 0, u32::MAX, 7);
        start.props.insert(25, 0);
        start.props.insert(36, 1);
        let layout = Layout {
            class: 102,
            elements: vec![start, node(7, 1, 0, 6, u32::MAX)],
        };
        assert_eq!(screen_step(&layout, 0, 2, -1), Some(0));
        // Down keeps its own mode 2 and link.
        assert_eq!(screen_step(&layout, 0, 2, 1), Some(1));
    }

    /// `FUN_0021cd28`: the highlighted row's alpha walks from 255 down by
    /// 5 to 125, then up by 5 to 255, 52 updates a period.
    #[test]
    fn the_highlighted_row_pulses_like_fun_0021cd28() {
        assert_eq!(row_pulse(0), 255);
        assert_eq!(row_pulse(1), 250);
        assert_eq!(row_pulse(26), 125);
        assert_eq!(row_pulse(27), 130);
        assert_eq!(row_pulse(51), 250);
        assert_eq!(row_pulse(52), 255);
        assert_eq!(row_pulse(78), 125);
        let period: Vec<u8> = (0..52).map(row_pulse).collect();
        assert!(period.iter().all(|&a| (125..=255).contains(&a)));
        assert_eq!(period.iter().filter(|&&a| a == 255).count(), 1);
    }

    /// `FUN_002127c0` with the region word 0: the legend icons by action.
    #[test]
    fn black_layers_darken_as_the_ps2_blends_them() {
        // Nothing and everything stay put; in between the alpha grows so
        // that linear-light compositing keeps the PS2's share of the light:
        // (1 - a') ^ (1 / 2.2) = 1 - a.
        assert_eq!(black_alpha(0.0), 0.0);
        assert_eq!(black_alpha(1.0), 1.0);
        for a in [0.1f32, 0.31, 0.5, 0.66, 0.86] {
            let b = black_alpha(a);
            assert!(b > a, "{a} -> {b}");
            assert!(((1.0 - b).powf(1.0 / 2.2) - (1.0 - a)).abs() < 1e-5);
        }
    }

    #[test]
    fn legend_icons_follow_the_us_region_table() {
        assert_eq!(
            (0..8).map(legend_icon).collect::<Vec<_>>(),
            [
                Some(3),
                Some(2),
                Some(1),
                Some(0),
                Some(9),
                Some(0),
                Some(19),
                None
            ]
        );
    }

    /// `FUN_0029e230`: the Main Menu's rows draw at their nodes' points
    /// moved by (0, -20), in registration order (widgets 0, 1, 23, 21 and
    /// 26, then the created row 31 in font 4); a widget past the nodes is
    /// hidden, and a screen without a traced constructor has no binding.
    #[test]
    fn screen_rows_sit_on_their_nodes_in_registration_order() {
        let at = |x: u32, y: u32| (3u16, x | (y << 16));
        let mut elements = Vec::new();
        for (k, y) in [230u32, 262, 294, 326, 358].into_iter().enumerate() {
            elements.push(Element {
                kind: 10,
                id: 100 + k as u32,
                align: 0,
                props: BTreeMap::from([(33, 19), (34, k as u32), at(560, y)]),
            });
        }
        for (id, font) in [(0u32, 2u32), (1, 2), (23, 3), (21, 2), (26, 2)] {
            elements.push(Element {
                kind: 19,
                id,
                align: 1,
                props: BTreeMap::from([(13, font), at(9, 9)]),
            });
        }
        let mut layout = Layout { class: 3, elements };
        let rows = screen_rows(&layout).unwrap();
        assert_eq!(rows.len(), 6);
        let spots: Vec<(f32, usize)> = rows.iter().flatten().map(|r| (r.at.y, r.font)).collect();
        assert_eq!(
            spots,
            [(210.0, 2), (242.0, 2), (274.0, 3), (306.0, 2), (338.0, 2)]
        );
        assert!(
            rows.iter()
                .flatten()
                .all(|r| r.at.x == 560.0 && r.align == Align::Centre && r.valign == 0)
        );
        assert!(rows[5].is_none(), "the created row has no sixth node here");
        layout.elements.push(Element {
            kind: 10,
            id: 105,
            align: 0,
            props: BTreeMap::from([(33, 19), (34, 5), at(560, 390)]),
        });
        let rows = screen_rows(&layout).unwrap();
        assert_eq!(
            rows[5].map(|r| (r.at.y, r.font, r.align)),
            Some((370.0, 4, Align::Left))
        );
        assert!(
            screen_rows(&Layout {
                class: 56,
                elements: Vec::new()
            })
            .is_none()
        );
    }

    #[test]
    fn screen_row_registrations_preserve_native_order_over_geometry() {
        assert_eq!(
            screen_registered_row_element_ids(3),
            Some(&[0, 1, 23, 21, 26, 31][..])
        );
        assert_eq!(
            screen_registered_row_element_ids(5),
            Some(&[15, 17, 18, 16][..])
        );
        assert_eq!(screen_registered_row_element_ids(6), Some(&[1, 2][..]));
        assert_eq!(
            screen_registered_row_element_ids(56),
            Some(&[27, 29, 28, 30, 31, 32][..])
        );
        assert_eq!(
            screen_registered_row_element_ids(102),
            Some(&[15, 17, 19, 22, 18][..])
        );
        assert_eq!(
            screen_registered_row_element_ids(103),
            Some(&[22, 16, 18, 19, 21][..])
        );
        assert_eq!(screen_registered_row_element_ids(104), Some(&[15, 16][..]));
        assert_eq!(screen_registered_row_element_ids(999), None);
    }
}
