//! The replay and gamebreaker banner: the HUD's widget `0x41`
//! (`FUN_002a9190`, vtable `0x48cd80`, art `/FE/REPLAY.SSH`), stepped by
//! `FUN_002ac0e0` once an update in one of four modes the Overlays task
//! sets: 0 a replay starting (`FUN_002b7af0`, listener slot 58), 1 leaving
//! (`FUN_002b7c68`, slots 59 and 65), 2 and 3 a Level 1 and Level 2
//! gamebreaker (`FUN_002b7bb8`, slot 64). The timings are the
//! constructor's, at 60 updates a second.
//!
//! - **Bands** (5, 6): two stripes, 640 by 84 and 83, stretched from one
//!   row of the sheet (turned a quarter round) for a replay and GB1, from
//!   the brown bars' columns for GB2, slide in over 11 updates: the top
//!   one's bottom from −16 to 63, the bottom one's top from 464 to 386.
//! - **A replay** (`FUN_002aac68`): two triangles (2, 3; pointing left,
//!   240 by 334, the second 237 behind) sweep from x 638 to −1090 between
//!   updates 12 and 60; the Vol. 2 logo (4) flies from (800, −116) to
//!   (518, 349) shrinking from 3 to 0.3 times between 54 and 62, then
//!   bounces 0.3 → 0.75 → 0.35 → 0.475 (64..74, 75..81, 82..88); at 60
//!   "Replay" (`overlays.txt` 53, font 5, 0.75, `0xff1b6bcc`) shows at
//!   (520, 420). Not the logo nor the word in the attract match.
//! - **A gamebreaker** (`FUN_002ab4b0`): GB1 or GB2 (8) at (320, 240)
//!   zooms from 6 to 1.7 times its 77 by 39 between 12 and 30, its alpha
//!   from 0x27 to 0xff, between two stars (0, 1; 0.2671 of their size, 0.53
//!   of its width either side of the middle, at y 224); then it fades
//!   between 31 and 52 while growing to 2.5 (31..41) and settling to 1.25
//!   (42..52).
//! - **Leaving** (`FUN_002aaa18`): everything but the bands goes; the bands
//!   slide back out over 11 updates and the banner hides.
//!
//! - **GB2's burst** (element 9, `FUN_002a8538`; update `FUN_002a8770`,
//!   draw `FUN_002a8c20`), shown with the word in mode 3 only: twelve
//!   wedges from the frame's middle (320, 240) to the points of the table
//!   at `0x4d4a10` (filled at the boot, `0x2bbcd4`), yellow `0xffe2c108`
//!   and orange `0xffe49416` by turns (`0x3fb4f8`); its clock (`+0x1c0`,
//!   one an update) shows wedges `max(0, c/4 - 4)..min(12, c/4 + 8)`, so
//!   eight at first, all twelve from 16 and one gone every 4 updates from
//!   20 (the wipe that uncovers the play), the widget hidden past 12. A
//!   yellow star (sheet (429, 176), 80 by 73) flies from each wedge's outer
//!   edge to (320, 224) over fps / 3 updates (`+0x1c4`), shrinking from
//!   1.65 times its size to nothing: an odd wedge's first draw starts the
//!   star before it, an even star past 0.3 the next, and a star goes with
//!   its wedge.

use bevy::prelude::*;

use crate::look::Piece;

/// "Replay"'s colour (`FUN_0021ce50`'s `0xff1b6bcc`).
pub const REPLAY_COLOUR: [u8; 4] = [0x1b, 0x6b, 0xcc, 0xff];

/// The banner's mode (`+0xbc`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Hidden,
    ReplayIn,
    Out,
    Gamebreaker1,
    Gamebreaker2,
}

/// The banner's state: its mode, the updates it has run (`+0xb8`) and
/// which bands it wears (a mode's own until it leaves).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Banner {
    pub mode: Mode,
    pub frame: u32,
    pub brown: bool,
}

/// The constructor's moments (`+0xc8..+0x1a8` at 60 updates a second).
mod at {
    pub const BANDS_END: u32 = 11;
    /// The burst's star's flight, updates (`+0x1c4`, fps / 3).
    pub const STAR_FLIGHT: f32 = 20.0;
    pub const SWEEP: (u32, u32) = (12, 60);
    pub const LOGO_FLY: (u32, u32) = (54, 62);
    pub const LOGO_BOUNCES: [(u32, u32, f32, f32); 3] = [
        (64, 74, 0.3, 0.75),
        (75, 81, 0.75, 0.35),
        (82, 88, 0.35, 0.475),
    ];
    pub const WORD: u32 = 60;
    pub const ZOOM: (u32, u32) = (12, 30);
    pub const FADE: (u32, u32) = (31, 52);
    pub const GROW: (u32, u32) = (31, 41);
    pub const SETTLE: (u32, u32) = (42, 52);
}

impl Banner {
    /// `FUN_002ac198` (the widget shown) with the mode set.
    pub fn show(&mut self, mode: Mode) {
        match mode {
            Mode::Out => {
                if self.mode == Mode::Hidden {
                    return;
                }
            }
            Mode::Gamebreaker2 => self.brown = true,
            _ => self.brown = false,
        }
        self.mode = mode;
        self.frame = 0;
    }

    /// The burst has begun its wipe (its `+0x1f8`, set by `FUN_002a8c20`
    /// once a wedge goes), which a staged Level 2 waits for
    /// (`FUN_002bb818`). False unless GB2 shows.
    pub fn burst_wiping(&self) -> bool {
        self.mode == Mode::Gamebreaker2
            && self
                .frame
                .checked_sub(1)
                .and_then(|b| b.checked_sub(at::ZOOM.0))
                .is_some_and(|n| burst(n).0.start > 0)
    }

    /// `FUN_002ac0e0`: one update.
    pub fn step(&mut self) {
        if self.mode == Mode::Hidden {
            return;
        }
        self.frame += 1;
        if self.mode == Mode::Out && self.frame > at::BANDS_END {
            self.mode = Mode::Hidden;
        }
    }

    /// The pieces, in the children's order (9, 2, 3, 5, 6, 4, 8, 0, 1, 7),
    /// the word through `say`.
    pub fn draw(
        &self,
        out: &mut Vec<Piece>,
        art: &ReplayArt,
        attract: bool,
        mut say: impl FnMut(&mut Vec<Piece>, Vec2),
    ) {
        if self.mode == Mode::Hidden {
            return;
        }
        // The last update's counter (`+0xb8` as its mode function saw it).
        let Some(b) = self.frame.checked_sub(1) else {
            return;
        };
        let lerp =
            |b: u32, (from, to): (u32, u32)| (b.saturating_sub(from)) as f32 / (to - from) as f32;
        // The banner's points are the match GUI's (the frame's rows), 16
        // lower in the rewrite's layout space (`crate::hud::GUI_TO_LAYOUT`).
        let put = |out: &mut Vec<Piece>,
                   image: &Handle<Image>,
                   source: Option<Rect>,
                   min: Vec2,
                   size: Vec2,
                   alpha: f32| {
            let min = min + Vec2::new(0.0, crate::hud::GUI_TO_LAYOUT);
            let mut p = Piece::new(image.clone(), Rect::from_corners(min, min + size));
            p.source = source;
            p.colour = Color::srgba(1.0, 1.0, 1.0, alpha.clamp(0.0, 1.0));
            out.push(p);
        };
        let replay = self.mode == Mode::ReplayIn;
        let gamebreaker = matches!(self.mode, Mode::Gamebreaker1 | Mode::Gamebreaker2);
        // GB2's burst, shown with the word; its points are the frame's
        // (the table's less 16).
        if self.mode == Mode::Gamebreaker2 && b >= at::ZOOM.0 {
            // The stars (its children) are drawn before its wedges, which
            // cover them: the original's grabs never show one.
            let (wedges, stars) = burst(b - at::ZOOM.0);
            for (k, t) in stars {
                let mid = burst_edge(k);
                let centre = Vec2::new(
                    mid.x + (320.0 - mid.x) * t,
                    (mid.y - 16.0) + (224.0 - (mid.y - 16.0)) * t,
                );
                let size = Vec2::new(80.0, 73.0) * (1.0 - t) * 1.65;
                put(
                    out,
                    &art.sheet,
                    Some(art.burst_star),
                    centre - size / 2.0,
                    size,
                    1.0,
                );
            }
            for k in wedges {
                let (image, rect) = &art.wedges[k];
                put(
                    out,
                    image,
                    None,
                    rect.min - Vec2::Y * 16.0,
                    rect.size(),
                    1.0,
                );
            }
        }
        // The triangles.
        if replay && (at::SWEEP.0..=at::SWEEP.1).contains(&b) {
            let t = lerp(b, at::SWEEP);
            let x = 638.0 + (-1090.0 - 638.0) * t;
            for dx in [0.0, 237.0] {
                put(
                    out,
                    &art.triangle,
                    None,
                    Vec2::new(x + dx, 60.0),
                    Vec2::new(240.0, 334.0),
                    1.0,
                );
            }
        }
        // The bands: in, held, or (leaving) out.
        let t = (b as f32 / at::BANDS_END as f32).min(1.0);
        let t = if self.mode == Mode::Out { 1.0 - t } else { t };
        let top = -16.0 + (63.0 - -16.0) * t;
        let bottom = 464.0 + (386.0 - 464.0) * t;
        let bands = if self.brown { &art.brown } else { &art.bands };
        put(
            out,
            &bands[0],
            None,
            Vec2::new(0.0, top - 84.0),
            Vec2::new(640.0, 84.0),
            1.0,
        );
        put(
            out,
            &bands[1],
            None,
            Vec2::new(0.0, bottom),
            Vec2::new(640.0, 83.0),
            1.0,
        );
        // The logo.
        if replay && !attract && b >= at::LOGO_FLY.0 {
            let fly = lerp(b.min(at::LOGO_FLY.1), at::LOGO_FLY);
            let centre = Vec2::new(800.0, -116.0)
                + (Vec2::new(518.0, 349.0) - Vec2::new(800.0, -116.0)) * fly;
            let mut scale = 3.0 - 2.7 * fly;
            for &(from, to, a, z) in &at::LOGO_BOUNCES {
                if b >= from {
                    scale = a + (z - a) * lerp(b.min(to), (from, to));
                }
            }
            let size = Vec2::new(174.0, 113.0) * scale;
            put(
                out,
                &art.sheet,
                Some(art.logo),
                centre - size / 2.0,
                size,
                1.0,
            );
        }
        // GB1 or GB2 between its stars.
        if gamebreaker && b >= at::ZOOM.0 {
            let mut scale = 6.0 + (1.7 - 6.0) * lerp(b.min(at::ZOOM.1), at::ZOOM);
            let mut alpha =
                (0x27 as f32 + (0xff - 0x27) as f32 * lerp(b.min(at::ZOOM.1), at::ZOOM)).round();
            if b >= at::FADE.0 {
                alpha = (255.0 * (1.0 - lerp(b.min(at::FADE.1), at::FADE))).round();
            }
            if b >= at::GROW.0 {
                scale = 1.7 + (2.5 - 1.7) * lerp(b.min(at::GROW.1), at::GROW);
            }
            if b >= at::SETTLE.0 {
                scale = 2.5 + (1.25 - 2.5) * lerp(b.min(at::SETTLE.1), at::SETTLE);
            }
            let a = alpha / 255.0;
            let size = Vec2::new(77.0, 39.0) * scale;
            let source = if self.mode == Mode::Gamebreaker2 {
                art.gb2
            } else {
                art.gb1
            };
            put(
                out,
                &art.sheet,
                Some(source),
                Vec2::new(320.0, 240.0) - size / 2.0,
                size,
                a,
            );
            let star = Vec2::new(156.0, 146.0) * scale * 0.267_100_2;
            let gap = size.x * 0.53;
            put(
                out,
                &art.star,
                None,
                Vec2::new(320.0 - gap - star.x, 224.0 - star.y / 2.0),
                star,
                a,
            );
            put(
                out,
                &art.star,
                None,
                Vec2::new(320.0 + gap, 224.0 - star.y / 2.0),
                star,
                a,
            );
        }
        // The word, on its row widget (`FUN_0021c4c0`) between the row's
        // stars: the original's replays show it 16 above the row's point
        // (520, 420), so at that point in layout space; the row widget's
        // own offset is not traced.
        if replay && !attract && b >= at::WORD {
            say(out, Vec2::new(520.0, 420.0));
        }
    }
}

/// The burst's points (`0x4d4a10`), in layout space.
pub const BURST_POINTS: [Vec2; 12] = [
    Vec2::new(310.0, 0.0),
    Vec2::new(480.0, 0.0),
    Vec2::new(700.0, 100.0),
    Vec2::new(640.0, 320.0),
    Vec2::new(640.0, 480.0),
    Vec2::new(400.0, 480.0),
    Vec2::new(250.0, 480.0),
    Vec2::new(60.0, 480.0),
    Vec2::new(0.0, 400.0),
    Vec2::new(0.0, 240.0),
    Vec2::new(0.0, 65.0),
    Vec2::new(115.0, 0.0),
];
/// Its wedges' colours by turns (`0x3fb4f8`, `0xAARRGGBB`).
pub const BURST_COLOURS: [[u8; 3]; 2] = [[0xe2, 0xc1, 0x08], [0xe4, 0x94, 0x16]];

/// Where star `k` starts: the outer edge's middle of the wedge whose first
/// draw starts it (`FUN_002a8770`).
fn burst_edge(k: usize) -> Vec2 {
    let first = if k.is_multiple_of(2) { k + 1 } else { k };
    (BURST_POINTS[first % 12] + BURST_POINTS[(first + 1) % 12]) * 0.5
}

/// The burst `n` updates after it showed: the wedges drawn and each star's
/// flight so far (0 at its edge, 1 at the middle). Each update the stars
/// move (`FUN_002a8770`, the clock then counting on) before the wedges
/// draw (`FUN_002a8c20`).
pub fn burst(n: u32) -> (std::ops::Range<usize>, Vec<(usize, f32)>) {
    let mut starts = [-1i32; 12];
    let mut wedges = 0..0;
    let mut stars = Vec::new();
    for c in 0..=n as i32 {
        stars.clear();
        for k in 0..12 {
            if starts[k] < 0 {
                continue;
            }
            let t = (c - starts[k]) as f32 / at::STAR_FLIGHT;
            if t > 0.3 && k.is_multiple_of(2) && starts[k + 1] < 0 {
                starts[k + 1] = c + 1;
            }
            if t > 0.99 {
                starts[k] = -1;
            } else {
                stars.push((k, t));
            }
        }
        let d = (c + 1) as usize;
        let (mut from, mut to) = (0, d / 4 + 8);
        if to > 12 {
            from = (d / 4 - 4).min(12);
            to = 12;
            for slot in &mut starts[..from] {
                *slot = -1;
            }
            stars.retain(|&(k, _)| k >= from);
        }
        for k in from..to {
            if !k.is_multiple_of(2) && starts[k - 1] < 0 {
                starts[k - 1] = d as i32;
            }
        }
        wedges = from..to;
    }
    (wedges, stars)
}

/// `REPLAY.SSH` (512 by 256) cut as the banner's children read it:
/// the parts turned a quarter round by their corners made upright here.
pub struct ReplayArt {
    pub sheet: Handle<Image>,
    /// The bands: a replay's and GB1's (rows 197 and 205 of u 278..356,
    /// turned), GB2's brown ones (columns 504..508 and 488..492 of v
    /// 9..86).
    pub bands: [Handle<Image>; 2],
    pub brown: [Handle<Image>; 2],
    /// The triangle (u 6..169, v 6..124), turned to point left.
    pub triangle: Handle<Image>,
    /// The star (u 265..412, v 17..174), turned.
    pub star: Handle<Image>,
    /// GB2's burst: its wedges, each drawn into its box (layout space).
    pub wedges: Vec<(Handle<Image>, Rect)>,
    /// The burst's yellow star (u 429..509, v 176..249).
    pub burst_star: Rect,
    pub logo: Rect,
    pub gb1: Rect,
    pub gb2: Rect,
}

impl ReplayArt {
    pub fn new(sheet: &street_assets::ssh::Sheet, images: &mut Assets<Image>) -> Option<Self> {
        let t = sheet.textures.first()?;
        let at = |u: u32, v: u32| -> [u8; 4] {
            let k = ((v.min(t.height - 1) * t.width + u.min(t.width - 1)) * 4) as usize;
            [t.rgba[k], t.rgba[k + 1], t.rgba[k + 2], t.rgba[k + 3]]
        };
        let mut make = |w: u32, h: u32, f: &dyn Fn(u32, u32) -> [u8; 4]| {
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    rgba.extend_from_slice(&f(x, y));
                }
            }
            let mut image = crate::look::image(w, h, rgba);
            image.sampler = bevy::image::ImageSampler::linear();
            images.add(image)
        };
        // A band turned: down its height the sheet's u, along a row v.
        let band =
            |make: &mut dyn FnMut(u32, u32, &dyn Fn(u32, u32) -> [u8; 4]) -> Handle<Image>,
             row: u32| make(1, 78, &|_, y| at(278 + y, row));
        let bands = [band(&mut make, 197), band(&mut make, 205)];
        let brown = [
            make(4, 77, &|x, y| at(504 + x, 9 + y)),
            make(4, 75, &|x, y| at(488 + x, 9 + y)),
        ];
        // The triangle's corners: top-left (u1, v0), bottom-left (u0, v0).
        let triangle = make(118, 163, &|x, y| at(168 - y, 6 + x));
        // The stars': top-left (u0, v1), top-right (u0, v0).
        let star = make(157, 147, &|x, y| at(265 + y, 173 - x));
        let sheet_image = images.add(street_render::convert::image(t));
        let wedges = (0..12)
            .map(|k| {
                let (rect, rgba) = wedge(k);
                let w = rect.width() as u32 * WEDGE_SCALE;
                let h = rect.height() as u32 * WEDGE_SCALE;
                let mut image = crate::look::image(w, h, rgba);
                image.sampler = bevy::image::ImageSampler::linear();
                (images.add(image), rect)
            })
            .collect();
        Some(Self {
            sheet: sheet_image,
            bands,
            brown,
            triangle,
            star,
            wedges,
            burst_star: Rect::new(429.0, 176.0, 509.0, 249.0),
            logo: Rect::new(0.0, 142.6, 173.4, 256.0),
            gb1: Rect::new(174.3, 211.8, 251.5, 251.0),
            gb2: Rect::new(277.6, 211.8, 361.7, 251.0),
        })
    }
}

/// The wedge images' pixels per layout unit (their edges sharp at 1080p).
const WEDGE_SCALE: u32 = 3;

/// Wedge `k`'s box on the 640x480 screen (clipped to it) and its pixels:
/// the triangle from the middle to points `k` and `k + 1` in its colour,
/// four samples a pixel at its edges.
fn wedge(k: usize) -> (Rect, Vec<u8>) {
    let corners = [
        Vec2::new(320.0, 240.0),
        BURST_POINTS[k],
        BURST_POINTS[(k + 1) % 12],
    ];
    let mut rect = Rect::from_corners(corners[0], corners[0]);
    for c in &corners[1..] {
        rect = rect.union_point(*c);
    }
    let rect = rect.intersect(Rect::new(0.0, 0.0, 640.0, 480.0));
    let rect = Rect::from_corners(rect.min.floor(), rect.max.ceil());
    let w = rect.width() as u32 * WEDGE_SCALE;
    let h = rect.height() as u32 * WEDGE_SCALE;
    let inside = |p: Vec2| {
        let side = |a: Vec2, b: Vec2| (b - a).perp_dot(p - a);
        let s = [
            side(corners[0], corners[1]),
            side(corners[1], corners[2]),
            side(corners[2], corners[0]),
        ];
        s.iter().all(|&v| v >= 0.0) || s.iter().all(|&v| v <= 0.0)
    };
    let [r, g, b] = BURST_COLOURS[k % 2];
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let px = rect.min + Vec2::new(x as f32, y as f32) / WEDGE_SCALE as f32;
            let hits = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)]
                .iter()
                .filter(|&&(dx, dy)| inside(px + Vec2::new(dx, dy) / WEDGE_SCALE as f32))
                .count();
            rgba.extend_from_slice(&[r, g, b, (hits * 255 / 4) as u8]);
        }
    }
    (rect, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_leaves_after_its_bands() {
        let mut b = Banner::default();
        b.show(Mode::ReplayIn);
        for _ in 0..100 {
            b.step();
        }
        assert_eq!(b.mode, Mode::ReplayIn);
        b.show(Mode::Out);
        for _ in 0..=at::BANDS_END {
            b.step();
        }
        assert_eq!(b.mode, Mode::Hidden);
        // Leaving a hidden banner shows nothing.
        b.show(Mode::Out);
        assert_eq!(b.mode, Mode::Hidden);
    }

    #[test]
    fn gb2s_burst_wipes_one_wedge_every_four_updates() {
        // Eight wedges at first, all twelve from 16, then one gone every
        // four updates until none at 64 (`FUN_002a8c20`).
        assert_eq!(burst(0).0, 0..8);
        assert_eq!(burst(15).0, 0..12);
        assert_eq!(burst(18).0, 0..12);
        assert_eq!(burst(19).0, 1..12);
        assert_eq!(burst(63).0, 12..12);
        // The odd wedges' first draws start the stars before them, seen
        // the next update at their edges.
        let (_, stars) = burst(1);
        assert_eq!(stars, vec![(0, 0.0), (2, 0.0), (4, 0.0), (6, 0.0)]);
        // Star 0 past 0.3 starts star 1; each flies fps / 3 updates.
        let (_, stars) = burst(9);
        assert!(stars.contains(&(1, 0.0)));
        assert!(stars.iter().all(|&(_, t)| t <= 0.99));
        // A star goes with its wedge.
        let (wedges, stars) = burst(40);
        assert!(stars.iter().all(|&(k, _)| k >= wedges.start));
    }

    #[test]
    fn a_staged_level_2_waits_for_the_bursts_first_wedge_to_go() {
        let mut b = Banner::default();
        b.show(Mode::Gamebreaker2);
        let mut updates = 0;
        while !b.burst_wiping() {
            b.step();
            updates += 1;
        }
        // The burst shows at the banner's 12th update and loses its first
        // wedge at its 20th draw.
        assert_eq!(updates, 32);
        // Online the match holds the updates after the press's for it:
        // the HUD steps the banner once on the press's update.
        assert_eq!(updates - 1, u32::from(street_net::game::LEVEL2_BURST_HOLD));
        b.show(Mode::Gamebreaker1);
        b.step();
        assert!(!b.burst_wiping());
    }

    #[test]
    fn a_wedge_is_its_colour_inside_and_clear_outside() {
        let (rect, rgba) = wedge(4);
        // Wedge 4: the middle, (640, 480) and (400, 480); yellow.
        assert_eq!(rect, Rect::new(320.0, 240.0, 640.0, 480.0));
        let w = rect.width() as usize * WEDGE_SCALE as usize;
        let at = |x: f32, y: f32| {
            let x = ((x - rect.min.x) * WEDGE_SCALE as f32) as usize;
            let y = ((y - rect.min.y) * WEDGE_SCALE as f32) as usize;
            rgba[(y * w + x) * 4..][..4].to_vec()
        };
        assert_eq!(at(500.0, 470.0), vec![0xe2, 0xc1, 0x08, 0xff]);
        assert_eq!(at(330.0, 470.0)[3], 0);
    }

    #[test]
    fn gb2_wears_the_brown_bands_until_it_leaves() {
        let mut b = Banner::default();
        b.show(Mode::Gamebreaker2);
        assert!(b.brown);
        b.show(Mode::Out);
        assert!(b.brown);
        b.show(Mode::ReplayIn);
        assert!(!b.brown);
    }
}
