//! The game's own front-end art, read from `/FE` and made into Bevy images.
//!
//! `XAFEBG.BIN` holds the screens' backdrops, three entries each: backdrop
//! `n` is entries `3n`, `3n + 1` and `3n + 2` laid side by side (strips
//! 128, 128 and 64 wide, 256 tall), of which the top-left 320x240 is the
//! picture, shown doubled on the 640x480 screen. Each screen's constructor
//! names its backdrop when it calls the base screen constructor
//! `FUN_002d7720(screen, class, backdrop)` (the manager `FUN_002e4560` then
//! opens the three entries); see [`backdrop`] for ours. Backdrop 17's first
//! entry is one 512-wide texture with the title picture in its left 320
//! columns, which the same rule crops out.
//!
//! `XATEAMS.BIN` holds one logo per entry, and the team record's `logo` field
//! picks it. `OFFIND.SSH` holds 1P..4P's numbered badges (red, yellow,
//! blue, green; 38x40 each along the top) and their arrows.
//!
//! Decoding happens once at startup; a failure keeps the plain look.

use std::io::BufReader;

use bevy::prelude::*;
use street_assets::{fe, ssh};
use street_render::convert;

use street_assets::iso::Iso;

/// Backdrops by the screen classes that use them (from the constructors'
/// calls to `FUN_002d7720`).
pub mod backdrop {
    /// Classes 3 (main menu), 5 (Play It), 6 (My Team), 0x66..0x69.
    pub const MAIN: usize = 18;
    /// The attract screen's billboard (class 99, `FUN_002d38b0`:
    /// `FUN_002d7720(screen, 99, 0x1b)`).
    pub const TITLE: usize = 27;
    /// Class 0x1d, choose player; the parameterised team pickers.
    pub const CHOOSE: usize = 5;
    /// Class 0x21, choose court.
    pub const COURT: usize = 4;
    /// Class 0x27, set challenge (My Rules).
    pub const RULES: usize = 25;
    /// Class 0x28, options (also 0x30 and 0x61).
    pub const OPTIONS: usize = 28;
    /// Class 0x29, choose region (the League Challenge).
    pub const REGION: usize = 6;
    /// Class 0x36, the story menu.
    pub const STORY: usize = 2;
    /// Class 0x38, Be A Legend's map.
    pub const MAP: usize = 19;
    /// Class 0x14, the reward posters.
    pub const REWARDS: usize = 21;
    /// Class 0x3d, create a player.
    pub const CREATE: usize = 3;
    /// Class 0x35, create team.
    pub const TEAM: usize = 9;
    /// Class 0x37, the controller layout.
    pub const CONTROLS: usize = 10;
    /// Class 9, the credits intro.
    pub const CREDITS: usize = 31;
}

/// A logo's grid tile (XATEAMS image 1, frame 0).
pub const LOGO_TILE: Vec2 = Vec2::new(55.0, 53.0);

#[derive(Resource)]
pub struct Art {
    /// Every backdrop of `XAFEBG`, by index.
    pub backdrops: Vec<Handle<Image>>,
    /// A logo per XATEAMS entry (a team's `logo` field indexes it): the
    /// grid tile, the 64x64 image's frame 0 (55x53, its own bevel), at
    /// [`LOGO_TILE`].
    pub logos: Vec<Handle<Image>>,
    /// Each logo sheet's large picture (image 0, its frame 0; the kind-24
    /// node draws image 0 with `+0xd0` set, `FUN_00222ef8`), with its
    /// size, for Choose Team's panels.
    pub logos_big: Vec<Option<(Handle<Image>, Vec2)>>,
    /// The double-width grid tiles some sheets carry (image 0's frames 1
    /// and 2, 111x55: Old School's sheet 39 and All-Stars' sheet 40),
    /// which `FUN_0026e5c8` shows for teams 38 and 39 in Choose Team's
    /// grid (`FUN_00222eb8`), with their size.
    pub logos_wide: Vec<Option<([Handle<Image>; 2], Vec2)>>,
    /// 1P..6P's numbered badges: the disc's four, then [`people_badge`]'s
    /// 5P and 6P for three on three online.
    pub badges: Vec<Handle<Image>>,
    /// The match HUD's atlas (`/FE/HUD.SSH`): meters, turbo bars, frames.
    pub hud: Option<Handle<Image>>,
    /// The atlas again in 5P's and 6P's colours ([`PEOPLE_SHIFTS`]), for
    /// their turbo bars and words online.
    pub hud_people: Vec<Handle<Image>>,
    /// The Rewards cards' pictures (`/FE/XAFESCRN.BIN`, its third sheet;
    /// a card's field 4 indexes it, see [`crate::rewards`]).
    pub cards: Vec<Handle<Image>>,
    /// Choose Court's pictures (`/FE/XACOURTS.BIN`): sheet `k` is court
    /// `k`'s, 12 the Random tile's and 13 the unknown court's
    /// (`FUN_00267f10`, `FUN_00210048(w, 3, k, 0, frame)`).
    pub courts: Vec<Option<CourtArt>>,
    /// The Play Game / My Rules dialog's picture (`/FE/XAFEBGEX.BIN` entry
    /// 6, "My_r"), cropped to its painted part, with that part's size.
    pub rules_dialog: Option<(Handle<Image>, Vec2)>,
    /// The four trophies, in the Rewards page's order (`XAFESCRN` sheet 0,
    /// image 0x0b: a 2x2 grid; which is which is `likely`, the crown the
    /// whole game's).
    pub trophies: Vec<Handle<Image>>,
    /// Each card picture cut to its frame 0 (86x111 upright, 111x86 for
    /// the wide cards), as the Rewards grid's sprites draw it
    /// (`FUN_00210048(w, 2, 2, picture, 0)`), with its size.
    pub card_frames: Vec<Option<(Handle<Image>, Vec2)>>,
    /// The Rewards screen's own sprites.
    pub reward_art: Option<RewardArt>,
    /// The picture a trophy not yet earned shows (`XAFEBGEX` entries
    /// 8..11), its strips with their sizes.
    pub trophy_unknown: Vec<(Handle<Image>, Vec2)>,
    /// The credits rolls' skyline (`XAFEBGEX` entry 3, 512x256, shown 640
    /// by 256, [`crate::credits`]).
    pub credits_picture: Option<Handle<Image>>,
    /// The loading screen's spinner and star ring.
    pub loading: Option<crate::loading_view::LoadingArt>,
    /// Street School's lesson menu's check rows.
    pub school: Option<crate::school_menu::SchoolArt>,
    /// Street School's Player Indicator page's pictures (`onscr2.ssh` in
    /// `/XF/XF000003`: entries 0, 2 and 3, `FUN_00251538`).
    pub indicator: Option<[(Handle<Image>, Vec2); 3]>,
}

/// The Rewards screen's sprites (`FUN_002bfe10`, `FUN_0021b550`).
pub struct RewardArt {
    /// The focused card's frame (`XAFESCRN` sheet 0, image 8, frame 2;
    /// `FUN_0021b550` kind 3), and the same turned a quarter for the wide
    /// cards (kind 4, `+0xec` 90).
    pub frame: (Handle<Image>, Vec2),
    pub frame_wide: (Handle<Image>, Vec2),
    /// A trophy slot's star (image 8, frame 0; `DAT_0048f5c8`).
    pub star: (Handle<Image>, Vec2),
    /// The focused trophy's frame (image 10, frame 0; kind 7).
    pub trophy_frame: (Handle<Image>, Vec2),
    /// The stars either side of a trophy's name and the three under it
    /// (`XACOMMON` sheet 0, image 1, frames 7 and 1; `DAT_0048f5f8`,
    /// `DAT_0048f5e8`).
    pub name_star: (Handle<Image>, Vec2),
    pub base_star: (Handle<Image>, Vec2),
    /// The Mitchell & Ness label under a jersey's close-up (image 0x11,
    /// frame 0; element 23, `FUN_002bfe10`'s `FUN_00210048(w, 2, 0, 0x11,
    /// 0)`).
    pub jersey_label: (Handle<Image>, Vec2),
}

/// A court sheet's authored frames: the grid tile (frame 1, 81 by 77) and
/// the big picture (frame 0, 248 by 174), which the Random sheet (12)
/// lacks: its one frame is the tile.
pub struct CourtArt {
    pub picture: Option<Handle<Image>>,
    pub tile: Handle<Image>,
}

impl Art {
    pub fn backdrop(&self, n: usize) -> Handle<Image> {
        self.backdrops
            .get(n)
            .or(self.backdrops.first())
            .cloned()
            .unwrap_or_default()
    }
}

/// Startup: decode the art and put it in `Assets<Image>`.
pub fn start(
    mut commands: Commands,
    mut disc: ResMut<crate::setup::Disc>,
    mut images: ResMut<Assets<Image>>,
) {
    let backdrops = match backdrops(&mut disc.iso) {
        Ok(all) => all
            .iter()
            .map(|t| images.add(convert::image(t)))
            .collect::<Vec<_>>(),
        Err(e) => return warn!("front-end backdrops: {e}; the plain look stays"),
    };
    let (logos, logos_big, logos_wide) = match fe::read(&mut disc.iso, "/FE/XATEAMS.BIN") {
        Ok(sheets) => {
            // A logo sheet is the large map, then the 64x64 tile the game
            // draws by its frame 0 (55x53, `FUN_00223698` puts kind 24/25's
            // picture at the tile's size); porL/porR repeat it split in two.
            let logos: Vec<Handle<Image>> = sheets
                .iter()
                .filter_map(|s| {
                    let (k, t) = s
                        .textures
                        .iter()
                        .enumerate()
                        .find(|(_, t)| t.width == 64 && t.height == 64)
                        .or_else(|| s.textures.first().map(|t| (0, t)))?;
                    let tile = match s.frames.get(k).and_then(|f| f.first()) {
                        Some(f) => {
                            let [x, y, w, h] = f.bounds().map(|v| v.max(0) as u32);
                            crop(t, x, y, w, h)
                        }
                        None => t.clone(),
                    };
                    Some(images.add(convert::image(&tile)))
                })
                .collect();
            // Image 0 by its authored frames: frame 0 the large picture,
            // frames 1 and 2 (where the sheet has them) the double tiles.
            let frame = |s: &street_assets::ssh::Sheet, k: usize| {
                let t = s.textures.first()?;
                let [x, y, w, h] = s.frames.first()?.get(k)?.bounds();
                let (x, y, w, h) = (
                    u32::try_from(x).ok()?,
                    u32::try_from(y).ok()?,
                    u32::try_from(w).ok()?,
                    u32::try_from(h).ok()?,
                );
                (w > 0 && h > 0 && x + w <= t.width && y + h <= t.height)
                    .then(|| crop(t, x, y, w, h))
            };
            let logos_big = sheets
                .iter()
                .map(|s| {
                    frame(s, 0).map(|t| {
                        let size = Vec2::new(t.width as f32, t.height as f32);
                        (images.add(convert::image(&t)), size)
                    })
                })
                .collect();
            let logos_wide = sheets
                .iter()
                .map(|s| {
                    let (a, b) = (frame(s, 1)?, frame(s, 2)?);
                    let size = Vec2::new(a.width as f32, a.height as f32);
                    Some((
                        [
                            images.add(convert::image(&a)),
                            images.add(convert::image(&b)),
                        ],
                        size,
                    ))
                })
                .collect();
            (logos, logos_big, logos_wide)
        }
        Err(e) => return warn!("team logos: {e}"),
    };
    // The numeral face of "Player N" (font 5), for 5P's and 6P's badges.
    let numerals = street_assets::font::fonts(&mut disc.iso)
        .ok()
        .and_then(|mut f| (f.len() > 5).then(|| f.swap_remove(5)));
    let badges: Vec<Handle<Image>> = match fe::sheet(&mut disc.iso, "/FE/OFFIND.SSH") {
        Ok(sheet) => sheet
            .textures
            .first()
            .map(|t| {
                let four: Vec<ssh::Texture> =
                    (0..4).map(|k| crop(t, 2 + 41 * k, 1, 38, 40)).collect();
                let more: Vec<ssh::Texture> = PEOPLE_SHIFTS
                    .iter()
                    .enumerate()
                    .map(|(k, &(from, shift))| {
                        people_badge(&four[from], shift, numerals.as_ref(), 5 + k as u32)
                    })
                    .collect();
                four.iter()
                    .chain(&more)
                    .map(|t| images.add(convert::image(t)))
                    .collect()
            })
            .unwrap_or_default(),
        Err(e) => {
            warn!("player badges: {e}");
            Vec::new()
        }
    };
    let (hud, hud_people) = match fe::sheet(&mut disc.iso, "/FE/HUD.SSH") {
        Ok(sheet) => match sheet.textures.first() {
            Some(t) => (
                Some(images.add(convert::image(t))),
                PEOPLE_SHIFTS
                    .iter()
                    .map(|&(_, shift)| images.add(convert::image(&hue_shift(t, shift))))
                    .collect(),
            ),
            None => (None, Vec::new()),
        },
        Err(e) => {
            warn!("HUD art: {e}");
            (None, Vec::new())
        }
    };
    let mut card_frames = Vec::new();
    let mut loading = None;
    let mut school = None;
    let mut reward_art = None;
    let (cards, trophies): (Vec<Handle<Image>>, Vec<Handle<Image>>) =
        match fe::read(&mut disc.iso, "/FE/XAFESCRN.BIN") {
            Ok(sheets) => {
                let sized = |images: &mut Assets<Image>, t: ssh::Texture| {
                    let size = Vec2::new(t.width as f32, t.height as f32);
                    (images.add(convert::image(&t)), size)
                };
                if let Some(s) = sheets.get(2) {
                    card_frames = (0..s.textures.len())
                        .map(|k| crate::look::sprite_crop(s, k, 0).map(|t| sized(&mut images, t)))
                        .collect();
                }
                let common = fe::read(&mut disc.iso, "/FE/XACOMMON.BIN").ok();
                // The loading screen's spinner (`XACOMMON` sheet 3, its
                // nineteen images' frame 0) and the ring's stars (sheet 0,
                // image 1, frames 6 and 5).
                loading = (|| {
                    let c = common.as_ref()?;
                    let spinner = c.get(3).map(|s| {
                        (0..s.textures.len())
                            .filter_map(|k| crate::look::sprite_crop(s, k, 0))
                            .map(|t| sized(&mut images, t))
                            .collect::<Vec<_>>()
                    })?;
                    let c0 = c.first()?;
                    Some(crate::loading_view::LoadingArt {
                        spinner,
                        star: sized(&mut images, crate::look::sprite_crop(c0, 1, 6)?),
                        star_lit: sized(&mut images, crate::look::sprite_crop(c0, 1, 5)?),
                    })
                })();
                // The lesson menu's check box and tick (`XACOMMON` sheet 0,
                // image 6, frames 4 and 5; `FUN_0021d9d0`).
                school = (|| {
                    let c0 = common.as_ref()?.first()?;
                    Some(crate::school_menu::SchoolArt {
                        check: sized(&mut images, crate::look::sprite_crop(c0, 6, 4)?),
                        tick: sized(&mut images, crate::look::sprite_crop(c0, 6, 5)?),
                    })
                })();
                reward_art = (|| {
                    let s0 = sheets.first()?;
                    let c0 = common.as_ref()?.first()?;
                    let frame = crate::look::sprite_crop(s0, 8, 2)?;
                    let mut wide = frame.clone();
                    crate::look::turn_quarter(&mut wide);
                    Some(RewardArt {
                        frame: sized(&mut images, frame),
                        frame_wide: sized(&mut images, wide),
                        star: sized(&mut images, crate::look::sprite_crop(s0, 8, 0)?),
                        trophy_frame: sized(&mut images, crate::look::sprite_crop(s0, 10, 0)?),
                        name_star: sized(&mut images, crate::look::sprite_crop(c0, 1, 7)?),
                        base_star: sized(&mut images, crate::look::sprite_crop(c0, 1, 1)?),
                        jersey_label: sized(&mut images, crate::look::sprite_crop(s0, 0x11, 0)?),
                    })
                })();
                let cards = sheets
                    .get(2)
                    .map(|s| {
                        s.textures
                            .iter()
                            .map(|t| images.add(convert::image(t)))
                            .collect()
                    })
                    .unwrap_or_default();
                let trophies = sheets
                    .first()
                    .and_then(|s| s.textures.get(0x0b))
                    .map(|t| {
                        [(14, 8), (132, 12), (14, 99), (132, 99)]
                            .iter()
                            .map(|&(x, y)| images.add(convert::image(&crop(t, x, y, 108, 80))))
                            .collect()
                    })
                    .unwrap_or_default();
                (cards, trophies)
            }
            Err(e) => {
                warn!("reward cards: {e}");
                (Vec::new(), Vec::new())
            }
        };
    // The School's Player Indicator page (`FUN_00251538` reads
    // `onscr2.ssh` from the game's shared archive).
    let indicator = (|| {
        let shared = disc.iso.read("/XF/XF000003").ok()?;
        let shared = street_assets::big::Big::parse(&shared).ok()?;
        let sheet = ssh::parse(shared.get("onscr2.ssh").ok()?).ok()?;
        let mut picture = |k: usize| {
            let t = sheet.textures.get(k)?;
            Some((
                images.add(convert::image(t)),
                Vec2::new(t.width as f32, t.height as f32),
            ))
        };
        Some([picture(0)?, picture(2)?, picture(3)?])
    })();
    let courts: Vec<Option<CourtArt>> = match fe::read(&mut disc.iso, "/FE/XACOURTS.BIN") {
        Ok(sheets) => sheets
            .iter()
            .map(|s| {
                // A court sheet's frame 0 is the picture and frame 1 the
                // tile; the Random sheet (12) has them the other way round
                // (`FUN_00267f10` picks its frame 0 for the tile), so the
                // frames are told apart by their size.
                let mut frame = |wide: bool| {
                    let t = s.textures.first()?;
                    let f = s
                        .frames
                        .first()?
                        .iter()
                        .find(|f| (f.bounds()[2] > 100) == wide)?;
                    let [x, y, w, h] = f.bounds();
                    let (x, y, w, h) = (
                        u32::try_from(x).ok()?,
                        u32::try_from(y).ok()?,
                        u32::try_from(w).ok()?,
                        u32::try_from(h).ok()?,
                    );
                    (w > 0 && h > 0 && x + w <= t.width && y + h <= t.height)
                        .then(|| images.add(convert::image(&crop(t, x, y, w, h))))
                };
                let picture = frame(true);
                Some(CourtArt {
                    picture,
                    tile: frame(false)?,
                })
            })
            .collect(),
        Err(e) => {
            warn!("court pictures: {e}");
            Vec::new()
        }
    };
    let mut trophy_unknown = Vec::new();
    let mut credits_picture = None;
    let rules_dialog = match fe::read(&mut disc.iso, "/FE/XAFEBGEX.BIN") {
        Ok(sheets) => {
            // The skyline is the entry's painted 320 columns (of 512),
            // shown 640 wide.
            credits_picture = sheets
                .get(crate::credits::PICTURE_ENTRY)
                .and_then(|s| s.textures.first())
                .map(|t| {
                    let part = crop(
                        t,
                        0,
                        0,
                        t.width.min(crate::credits::PICTURE_COLUMNS),
                        t.height,
                    );
                    images.add(convert::image(&part))
                });
            // Entries 8..11, the picture a trophy not yet earned shows on
            // the Rewards screen (`FUN_002c3948`: 128, 128, 128 and 32
            // wide, side by side).
            trophy_unknown = (8..12)
                .filter_map(|k| sheets.get(k)?.textures.first())
                .map(|t| {
                    let size = Vec2::new(t.width as f32, t.height as f32);
                    (images.add(convert::image(t)), size)
                })
                .collect();
            sheets
                .get(6)
                .and_then(|s| s.textures.first())
                .and_then(|t| {
                    // The painted part of the entry: the rest is clear.
                    let (mut x0, mut y0, mut x1, mut y1) = (t.width, t.height, 0, 0);
                    for (i, p) in t.rgba.as_chunks::<4>().0.iter().enumerate() {
                        if p[3] > 0 {
                            let (x, y) = (i as u32 % t.width, i as u32 / t.width);
                            x0 = x0.min(x);
                            y0 = y0.min(y);
                            x1 = x1.max(x + 1);
                            y1 = y1.max(y + 1);
                        }
                    }
                    (x1 > x0 && y1 > y0).then(|| {
                        let part = crop(t, x0, y0, x1 - x0, y1 - y0);
                        let size = Vec2::new(part.width as f32, part.height as f32);
                        (images.add(convert::image(&part)), size)
                    })
                })
        }
        Err(e) => {
            warn!("dialog pictures: {e}");
            None
        }
    };
    info!(
        "front end art loaded: {} backdrops, {} logos, {} badges, {} cards, {} courts",
        backdrops.len(),
        logos.len(),
        badges.len(),
        cards.len(),
        courts.len()
    );
    commands.insert_resource(Art {
        backdrops,
        logos,
        logos_big,
        logos_wide,
        badges,
        hud,
        hud_people,
        cards,
        trophies,
        courts,
        rules_dialog,
        card_frames,
        reward_art,
        trophy_unknown,
        credits_picture,
        loading,
        school,
        indicator,
    });
}

/// 5P's and 6P's colours, which the original never needed (four pads):
/// the person whose art each starts from (2 for 3P's blue, 0 for 1P's red)
/// and the turn of its hue, in degrees, to the rewrite's purple and orange
/// (`controls::SEAT_COLOURS`).
pub const PEOPLE_SHIFTS: [(usize, f32); 2] = [(2, 48.0), (0, 22.0)];

/// `t` with every coloured texel's hue turned by `degrees` (greys, whites
/// and blacks as they are).
pub(crate) fn hue_shift(t: &ssh::Texture, degrees: f32) -> ssh::Texture {
    let mut out = ssh::Texture {
        indices: None,
        ..t.clone()
    };
    for px in out.rgba.as_chunks_mut::<4>().0 {
        let [r, g, b] = [px[0], px[1], px[2]].map(|c| f32::from(c) / 255.0);
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let chroma = max - min;
        if chroma < 0.08 {
            continue;
        }
        let hue = if max == r {
            ((g - b) / chroma).rem_euclid(6.0)
        } else if max == g {
            (b - r) / chroma + 2.0
        } else {
            (r - g) / chroma + 4.0
        } * 60.0;
        let h = (hue + degrees).rem_euclid(360.0) / 60.0;
        let x = chroma * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (chroma, x, 0.0),
            1 => (x, chroma, 0.0),
            2 => (0.0, chroma, x),
            3 => (0.0, x, chroma),
            4 => (x, 0.0, chroma),
            _ => (chroma, 0.0, x),
        };
        for (c, v) in px.iter_mut().zip([r, g, b]) {
            *c = ((v + min) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// A badge for person `number` (5 or 6): `base`'s frame turned by `shift`,
/// its digit painted out with the black inside, and `number` drawn in
/// `numerals` (the face "Player N" is numbered in) as tall as the disc's
/// digit stood.
fn people_badge(
    base: &ssh::Texture,
    shift: f32,
    numerals: Option<&street_assets::font::Font>,
    number: u32,
) -> ssh::Texture {
    let mut t = hue_shift(base, shift);
    let (w, h) = (t.width as usize, t.height as usize);
    // The inside: the frame is 5 texels deep. The digit is the bright,
    // grey texels there; the black around it fills them in.
    let inside = |x: usize, y: usize| (6..w - 6).contains(&x) && (6..h - 6).contains(&y);
    let mut ink = (usize::MAX, usize::MAX, 0, 0);
    for y in 0..h {
        for x in 0..w {
            let at = (y * w + x) * 4;
            let [r, g, b] = [t.rgba[at], t.rgba[at + 1], t.rgba[at + 2]].map(i32::from);
            let bright = r.max(g).max(b);
            if inside(x, y) && bright > 60 && bright - r.min(g).min(b) < 60 {
                t.rgba[at..at + 3].copy_from_slice(&[12, 12, 12]);
                ink = (ink.0.min(x), ink.1.min(y), ink.2.max(x), ink.3.max(y));
            }
        }
    }
    let Some(font) = numerals else {
        return t;
    };
    let line = font.render(
        &number.to_string(),
        [0xf6, 0xf4, 0xd7, 0xff],
        [0xdc, 0xdc, 0xbb, 0xff],
    );
    if line.width == 0 || line.height == 0 || ink.0 > ink.2 {
        return t;
    }
    // The rendered digit's own ink, scaled to the old digit's height and
    // centred where it stood.
    let (lw, lh) = (line.width as usize, line.height as usize);
    let alpha = |x: usize, y: usize| line.rgba[(y * lw + x) * 4 + 3];
    let rows: Vec<usize> = (0..lh)
        .filter(|&y| (0..lw).any(|x| alpha(x, y) > 0))
        .collect();
    let cols: Vec<usize> = (0..lw)
        .filter(|&x| (0..lh).any(|y| alpha(x, y) > 0))
        .collect();
    let (Some(&top), Some(&bottom), Some(&left), Some(&right)) =
        (rows.first(), rows.last(), cols.first(), cols.last())
    else {
        return t;
    };
    let (gw, gh) = ((right - left + 1) as f32, (bottom - top + 1) as f32);
    let scale = (ink.3 - ink.1 + 1) as f32 / gh;
    let x0 = (ink.0 + ink.2 + 1) as f32 / 2.0 - gw * scale / 2.0;
    let y0 = ink.1 as f32;
    for y in 0..h {
        for x in 0..w {
            let (u, v) = ((x as f32 + 0.5 - x0) / scale, (y as f32 + 0.5 - y0) / scale);
            if !inside(x, y) || u < 0.0 || v < 0.0 || u >= gw || v >= gh {
                continue;
            }
            let src = ((top + v as usize) * lw + left + u as usize) * 4;
            let a = u32::from(line.rgba[src + 3]);
            let at = (y * w + x) * 4;
            for c in 0..3 {
                let (d, s) = (u32::from(t.rgba[at + c]), u32::from(line.rgba[src + c]));
                t.rgba[at + c] = ((s * a + d * (255 - a)) / 255) as u8;
            }
        }
    }
    t
}

/// A rectangle of a texture.
pub(crate) fn crop(t: &ssh::Texture, x: u32, y: u32, w: u32, h: u32) -> ssh::Texture {
    let (w, h) = (
        w.min(t.width.saturating_sub(x)),
        h.min(t.height.saturating_sub(y)),
    );
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let at = ((row * t.width + x) * 4) as usize;
        rgba.extend_from_slice(&t.rgba[at..at + (w * 4) as usize]);
    }
    ssh::Texture {
        name: t.name.clone(),
        width: w,
        height: h,
        rgba,
        indices: None,
    }
}

/// Every backdrop of the archive, 320x240 each (`fe::backdrops`).
fn backdrops(iso: &mut Iso<BufReader<std::fs::File>>) -> Result<Vec<ssh::Texture>, String> {
    let entries = fe::read(iso, "/FE/XAFEBG.BIN").map_err(|e| e.to_string())?;
    let strips: Vec<Option<&ssh::Texture>> = entries.iter().map(|s| s.textures.first()).collect();
    Ok(strips
        .chunks(3)
        .zip(fe::backdrops(&entries))
        .map(|(three, plain)| {
            // An HD pack's strips, joined at its resolution.
            let strips: Vec<&ssh::Texture> = three.iter().flatten().copied().collect();
            street_render::hires::join(
                &strips,
                fe::BACKDROP_WIDTH as u32,
                fe::BACKDROP_HEIGHT as u32,
            )
            .unwrap_or(plain)
        })
        .collect())
}
