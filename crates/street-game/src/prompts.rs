//! The rewrite's Button Prompts (PC Settings, [`Settings::prompts`]): whose
//! buttons the screens picture. The original draws every button prompt
//! from one picture, `XACOMMON` sheet 1, image 0 (the legends, the pause
//! screen's controller rows, Help, Street School's prompts), in the
//! PlayStation's shapes. For an Xbox-style pad (any pad that is not
//! Sony's, which includes Steam Input's) the rewrite repaints that same
//! picture: the face buttons keep the original's dark button with the
//! letter in its colour (Cross A green, Circle B red, Square X blue,
//! Triangle Y yellow), the shoulders read LB RB LT RT and Select and
//! Start read VIEW and MENU, in the front end's own fonts. Both pictures
//! are made once; the one shown is written into the original's image so
//! every screen that holds its handle follows.
//!
//! [`Settings::prompts`]: street_sim::records::Settings::prompts

use bevy::prelude::*;
use street_assets::font::Font;
use street_sim::records::{PROMPTS_AUTO, PROMPTS_PLAYSTATION, PROMPTS_XBOX};

use crate::pads::{ButtonStyle, PadNav};

/// The face buttons' frames (`legend_icon`'s: 0 Circle, 1 Square,
/// 2 Triangle, 3 Cross), each 25 by 25, and what an Xbox pad has there:
/// its letter and the letter's top and bottom colours.
const FACES: [(usize, &str, [u8; 3], [u8; 3]); 4] = [
    (0, "B", [0xff, 0xa0, 0x90], [0xd0, 0x30, 0x30]),
    (1, "X", [0xa0, 0xc8, 0xff], [0x30, 0x70, 0xe0]),
    (2, "Y", [0xff, 0xf0, 0x90], [0xe0, 0xb0, 0x10]),
    (3, "A", [0xb8, 0xf0, 0x90], [0x50, 0xb0, 0x30]),
];

/// The shoulders' frames: 5 L1, 6 R1, 7 R2, 8 L2.
const SHOULDERS: [(usize, &str); 4] = [(5, "LB"), (6, "RB"), (7, "RT"), (8, "LT")];

/// Frames with a word under the button (19 SELECT, 20 START): the row
/// the old word's soft edge starts on, its letters' first row, and the
/// new word.
const WORDS: [(usize, u32, u32, &str); 2] = [(19, 19, 20, "VIEW"), (20, 18, 19, "MENU")];

/// The word under a button's height, as the original's.
const WORD_HEIGHT: u32 = 6;

/// A pixel counts as part of a symbol past this channel spread.
const GREY: u8 = 10;
/// A shoulder's lettering: brighter than this (the channels' sum); its
/// soft edge, this much brighter than the pill.
const BRIGHT: u32 = 450;
const EDGE: u32 = 45;

/// The picture the screens show for this style and setting: Xbox for an
/// Xbox pad, and for the keyboard when the setting asks for Xbox.
pub fn xbox_pictures(style: ButtonStyle, prompts: u8) -> bool {
    match style {
        ButtonStyle::Xbox => true,
        ButtonStyle::PlayStation => false,
        ButtonStyle::Keyboard => prompts == PROMPTS_XBOX,
    }
}

/// A pad's style under the setting: Auto names Sony's pads' buttons as
/// PlayStation ones and the rest as Xbox ones.
pub fn pad_style(sony: bool, prompts: u8) -> ButtonStyle {
    match prompts {
        PROMPTS_PLAYSTATION => ButtonStyle::PlayStation,
        PROMPTS_XBOX => ButtonStyle::Xbox,
        _ if sony => ButtonStyle::PlayStation,
        _ => ButtonStyle::Xbox,
    }
}

/// The Button Prompts row's choice names.
pub fn prompts_name(prompts: u8) -> &'static str {
    match prompts {
        PROMPTS_PLAYSTATION => "PlayStation",
        PROMPTS_XBOX => "Xbox",
        _ => "Auto",
    }
}

/// The next choice (Auto, PlayStation, Xbox, round again).
pub fn next_prompts(prompts: u8) -> u8 {
    match prompts {
        PROMPTS_AUTO => PROMPTS_PLAYSTATION,
        PROMPTS_PLAYSTATION => PROMPTS_XBOX,
        _ => PROMPTS_AUTO,
    }
}

/// A frame's corners in whole pixels: x, y, width, height.
fn bounds(r: Rect) -> (u32, u32, u32, u32) {
    (
        r.min.x as u32,
        r.min.y as u32,
        r.width() as u32,
        r.height() as u32,
    )
}

fn spread(p: [u8; 4]) -> u8 {
    p[..3].iter().max().unwrap() - p[..3].iter().min().unwrap()
}

fn sum(p: [u8; 4]) -> u32 {
    p[..3].iter().map(|&c| u32::from(c)).sum()
}

/// A straight-alpha picture: width, height, RGBA.
struct Ink {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Ink {
    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let k = ((y * self.width + x) * 4) as usize;
        self.rgba[k..k + 4].try_into().unwrap()
    }
}

/// `text` in `font` cut to its ink, `top` fading to `bottom`.
fn letters(font: &Font, text: &str, top: [u8; 3], bottom: [u8; 3]) -> Option<Ink> {
    let r = font.render(
        text,
        [top[0], top[1], top[2], 255],
        [bottom[0], bottom[1], bottom[2], 255],
    );
    let alpha = |x: u32, y: u32| r.rgba[((y * r.width + x) * 4 + 3) as usize];
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..r.height {
        for x in 0..r.width {
            if alpha(x, y) > 0 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    if x0 > x1 {
        return None;
    }
    let (width, height) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in y0..=y1 {
        let row = ((y * r.width + x0) * 4) as usize;
        rgba.extend_from_slice(&r.rgba[row..row + (width * 4) as usize]);
    }
    // The ink runs top to bottom of its own box (the font fades by line).
    for y in 0..height {
        let t = y as f32 / (height - 1).max(1) as f32;
        for x in 0..width {
            let k = ((y * width + x) * 4) as usize;
            for c in 0..3 {
                rgba[k + c] =
                    (f32::from(top[c]) + (f32::from(bottom[c]) - f32::from(top[c])) * t) as u8;
            }
        }
    }
    Some(Ink {
        width,
        height,
        rgba,
    })
}

/// `ink` scaled to `height` rows by area (its width in proportion).
fn shrink(ink: &Ink, height: u32) -> Ink {
    let s = height as f32 / ink.height as f32;
    let width = ((ink.width as f32 * s).round() as u32).max(1);
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let (sx0, sx1) = (x as f32 / s, (x + 1) as f32 / s);
            let (sy0, sy1) = (y as f32 / s, (y + 1) as f32 / s);
            let (mut a, mut c, mut area) = (0.0, [0.0f32; 3], 0.0);
            let mut yy = sy0.floor() as u32;
            while (yy as f32) < sy1 && yy < ink.height {
                let hy = (sy1.min(yy as f32 + 1.0) - sy0.max(yy as f32)).max(0.0);
                let mut xx = sx0.floor() as u32;
                while (xx as f32) < sx1 && xx < ink.width {
                    let wx = (sx1.min(xx as f32 + 1.0) - sx0.max(xx as f32)).max(0.0);
                    let p = ink.at(xx, yy);
                    let w = wx * hy;
                    let pa = f32::from(p[3]) * w;
                    a += pa;
                    for k in 0..3 {
                        c[k] += f32::from(p[k]) * pa;
                    }
                    area += w;
                    xx += 1;
                }
                yy += 1;
            }
            let k = ((y * width + x) * 4) as usize;
            if a > 0.0 {
                for i in 0..3 {
                    rgba[k + i] = (c[i] / a).round() as u8;
                }
                rgba[k + 3] = (a / area.max(1e-6)).round().min(255.0) as u8;
            }
        }
    }
    Ink {
        width,
        height,
        rgba,
    }
}

/// The atlas being repainted.
struct Atlas<'a> {
    width: u32,
    rgba: &'a mut [u8],
}

impl Atlas<'_> {
    fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let k = ((y * self.width + x) * 4) as usize;
        self.rgba[k..k + 4].try_into().unwrap()
    }

    fn set(&mut self, x: u32, y: u32, p: [u8; 4]) {
        let k = ((y * self.width + x) * 4) as usize;
        self.rgba[k..k + 4].copy_from_slice(&p);
    }

    /// `ink` over the pixel at (x, y), straight alpha.
    fn over(&mut self, x: u32, y: u32, ink: [u8; 4]) {
        let d = self.get(x, y);
        let (sa, da) = (f32::from(ink[3]) / 255.0, f32::from(d[3]) / 255.0);
        let a = sa + da * (1.0 - sa);
        if a <= 0.0 {
            return;
        }
        let mut out = [0u8; 4];
        for k in 0..3 {
            out[k] =
                ((f32::from(ink[k]) * sa + f32::from(d[k]) * da * (1.0 - sa)) / a).round() as u8;
        }
        out[3] = (a * 255.0).round() as u8;
        self.set(x, y, out);
    }

    /// `ink` with its top-left at (x, y), clipped to `frame`; `shadow`
    /// first puts a dark copy one pixel down and right.
    fn stamp(&mut self, ink: &Ink, x: i32, y: i32, frame: (u32, u32, u32, u32), shadow: bool) {
        let (fx, fy, fw, fh) = frame;
        let inside = |px: i32, py: i32| {
            px >= fx as i32 && py >= fy as i32 && px < (fx + fw) as i32 && py < (fy + fh) as i32
        };
        let passes: &[i32] = if shadow { &[1, 0] } else { &[0] };
        for &d in passes {
            for iy in 0..ink.height {
                for ix in 0..ink.width {
                    let (px, py) = (x + ix as i32 + d, y + iy as i32 + d);
                    if !inside(px, py) {
                        continue;
                    }
                    let mut p = ink.at(ix, iy);
                    if d != 0 {
                        p = [0, 0, 0, (u32::from(p[3]) * 3 / 4) as u8];
                    }
                    self.over(px as u32, py as u32, p);
                }
            }
        }
    }
}

/// The face buttons' bare button: at each pixel, the four frames' pixels
/// without a symbol's colour (their channels within [`GREY`]), the middle
/// one by brightness; pixels every symbol covers take their neighbours'.
fn bare_button(atlas: &Atlas, frames: &[(u32, u32, u32, u32)]) -> Vec<Option<[u8; 4]>> {
    let (w, h) = (frames[0].2, frames[0].3);
    let mut out: Vec<Option<[u8; 4]>> = (0..w * h)
        .map(|k| {
            let (x, y) = (k % w, k / w);
            let mut grey: Vec<[u8; 4]> = frames
                .iter()
                .map(|&(fx, fy, _, _)| atlas.get(fx + x, fy + y))
                .filter(|&p| spread(p) <= GREY)
                .collect();
            grey.sort_by_key(|&p| sum(p));
            grey.get(grey.len().saturating_sub(1) / 2).copied()
        })
        .collect();
    for _ in 0..4 {
        let before = out.clone();
        for y in 0..h {
            for x in 0..w {
                if before[(y * w + x) as usize].is_some() {
                    continue;
                }
                let near: Vec<[u8; 4]> = (y.saturating_sub(1)..(y + 2).min(h))
                    .flat_map(|j| (x.saturating_sub(1)..(x + 2).min(w)).map(move |i| (i, j)))
                    .filter_map(|(i, j)| before[(j * w + i) as usize])
                    .collect();
                if !near.is_empty() {
                    let n = near.len() as u32;
                    out[(y * w + x) as usize] = Some(std::array::from_fn(|c| {
                        (near.iter().map(|p| u32::from(p[c])).sum::<u32>() / n) as u8
                    }));
                }
            }
        }
    }
    out
}

/// The Xbox copy of the button picture (`rgba`, `width` wide, its frames
/// as the sheet authors them), lettered in `fonts`' first two.
pub fn xbox_picture(rgba: &[u8], width: u32, frames: &[Rect], fonts: &[Font]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    let (Some(small), Some(large)) = (fonts.first(), fonts.get(1)) else {
        return out;
    };
    let mut atlas = Atlas {
        width,
        rgba: &mut out,
    };
    let frame = |k: usize| frames.get(k).copied().map(bounds);

    // Face buttons: the bare button, then the letter in the middle.
    let faces: Option<Vec<_>> = FACES.iter().map(|f| frame(f.0)).collect();
    if let Some(faces) = faces.filter(|f| f.iter().all(|b| (b.2, b.3) == (f[0].2, f[0].3))) {
        let bare = bare_button(&atlas, &faces);
        for (&(_, letter, top, bottom), &(fx, fy, fw, fh)) in FACES.iter().zip(&faces) {
            for y in 0..fh {
                for x in 0..fw {
                    if let Some(p) = bare[(y * fw + x) as usize] {
                        atlas.set(fx + x, fy + y, p);
                    }
                }
            }
            // The large font's letters fill the button to its ring.
            let ink = letters(large, letter, top, bottom)
                .filter(|i| i.height <= fh - 10)
                .or_else(|| letters(small, letter, top, bottom));
            if let Some(ink) = ink {
                let x = fx as i32 + (fw as i32 - ink.width as i32) / 2;
                let y = fy as i32 + (fh as i32 - ink.height as i32) / 2;
                atlas.stamp(&ink, x, y, (fx, fy, fw, fh), true);
            }
        }
    }

    // Shoulders: the lettering (its bright pixels, in rows the bevel's
    // highlight does not fill) gives way to each row's own pill colour,
    // with its soft edge a pixel round; the new name goes on the old one's
    // middle at the old one's height.
    for &(k, name) in &SHOULDERS {
        let Some((fx, fy, fw, fh)) = frame(k) else {
            continue;
        };
        let (x0, x1) = (fx + 3, fx + fw - 3);
        let mut rows = Vec::new();
        let (mut lx0, mut ly0, mut lx1, mut ly1) = (u32::MAX, u32::MAX, 0, 0);
        for y in fy..fy + fh {
            let row: Vec<[u8; 4]> = (x0..x1).map(|x| atlas.get(x, y)).collect();
            let bright = row.iter().filter(|&&p| sum(p) > BRIGHT).count();
            if bright == 0 || bright * 5 > row.len() * 3 {
                continue;
            }
            let mut dark: Vec<[u8; 4]> = row
                .iter()
                .copied()
                .filter(|&p| sum(p) <= BRIGHT && p[3] == 255)
                .collect();
            if dark.is_empty() {
                continue;
            }
            dark.sort_by_key(|&p| sum(p));
            rows.push((y, dark[dark.len() / 2]));
            for x in x0..x1 {
                if sum(atlas.get(x, y)) > BRIGHT {
                    (lx0, ly0, lx1, ly1) = (lx0.min(x), ly0.min(y), lx1.max(x), ly1.max(y));
                }
            }
        }
        if lx0 > lx1 {
            continue;
        }
        for &(y, fill) in &rows {
            for x in lx0.saturating_sub(1).max(x0)..(lx1 + 2).min(x1) {
                let p = atlas.get(x, y);
                if sum(p) > sum(fill) + EDGE || spread(p) > GREY * 4 {
                    atlas.set(x, y, fill);
                }
            }
        }
        // The original's lettering is white over grey.
        if let Some(ink) = letters(small, name, [0xff; 3], [0xc8; 3]) {
            let height = ly1 - ly0 + 1;
            let ink = if ink.height > height {
                shrink(&ink, height)
            } else {
                ink
            };
            let cx = (lx0 + lx1 + 1) as i32 / 2;
            atlas.stamp(
                &ink,
                cx - ink.width as i32 / 2,
                ly0 as i32,
                (fx, fy, fw, fh),
                false,
            );
        }
    }

    // Select and Start: the word under the button, in white.
    for &(k, erase, top, word) in &WORDS {
        let Some((fx, fy, fw, fh)) = frame(k) else {
            continue;
        };
        for y in fy + erase..fy + fh {
            for x in fx..fx + fw {
                atlas.set(x, y, [0; 4]);
            }
        }
        if let Some(ink) = letters(small, word, [0xff; 3], [0xff; 3]) {
            let ink = shrink(&ink, WORD_HEIGHT);
            let x = fx as i32 + (fw as i32 - ink.width as i32) / 2;
            atlas.stamp(&ink, x, (fy + top) as i32, (fx, fy, fw, fh), false);
        }
    }
    out
}

/// Both button pictures and the images that show one.
#[derive(Resource)]
pub struct ButtonPictures {
    /// The handles holding the original's picture (the legends' and the
    /// controller rows').
    pub images: Vec<Handle<Image>>,
    pub playstation: Vec<u8>,
    pub xbox: Vec<u8>,
    showing_xbox: bool,
}

impl ButtonPictures {
    pub fn new(images: Vec<Handle<Image>>, playstation: Vec<u8>, xbox: Vec<u8>) -> Self {
        Self {
            images,
            playstation,
            xbox,
            showing_xbox: false,
        }
    }
}

/// Writes the picture the style and setting want into the shown images.
pub fn follow(
    nav: Res<PadNav>,
    opts: Option<Res<crate::options::Options>>,
    pictures: Option<ResMut<ButtonPictures>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut pictures) = pictures else {
        return;
    };
    let prompts = opts.map_or(PROMPTS_AUTO, |o| o.0.prompts);
    let xbox = xbox_pictures(nav.style, prompts);
    if xbox == pictures.showing_xbox {
        return;
    }
    pictures.showing_xbox = xbox;
    let data = if xbox {
        pictures.xbox.clone()
    } else {
        pictures.playstation.clone()
    };
    for handle in &pictures.images {
        if let Some(mut image) = images.get_mut(handle) {
            image.data = Some(data.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pads::ButtonStyle as S;

    #[test]
    fn auto_follows_the_pad_and_a_choice_holds() {
        assert_eq!(pad_style(true, PROMPTS_AUTO), S::PlayStation);
        assert_eq!(pad_style(false, PROMPTS_AUTO), S::Xbox);
        assert_eq!(pad_style(true, PROMPTS_XBOX), S::Xbox);
        assert_eq!(pad_style(false, PROMPTS_PLAYSTATION), S::PlayStation);
        // The keyboard keeps the original's pictures unless Xbox is asked.
        assert!(!xbox_pictures(S::Keyboard, PROMPTS_AUTO));
        assert!(xbox_pictures(S::Keyboard, PROMPTS_XBOX));
        assert!(xbox_pictures(S::Xbox, PROMPTS_AUTO));
        let mut p = PROMPTS_AUTO;
        let names: Vec<_> = (0..3)
            .map(|_| {
                p = next_prompts(p);
                prompts_name(p)
            })
            .collect();
        assert_eq!(names, ["PlayStation", "Xbox", "Auto"]);
    }

    /// On the disc's picture: the face buttons lose the PlayStation
    /// symbols' colours for the Xbox letters' and keep the original's
    /// button round them; frames the repaint leaves (the sticks, the
    /// D-pad) are untouched. `STREET_PROMPTS_DUMP` writes both pictures.
    #[test]
    fn the_disc_picture_takes_the_xbox_letters() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let fonts = street_assets::font::fonts(&mut iso).unwrap();
        let sheets = street_assets::fe::read(&mut iso, "/FE/XACOMMON.BIN").unwrap();
        let sheet = &sheets[1];
        let t = &sheet.textures[0];
        let frames: Vec<Rect> = sheet.frames[0]
            .iter()
            .map(|f| {
                let [x, y, w, h] = f.bounds();
                Rect::new(x as f32, y as f32, (x + w) as f32, (y + h) as f32)
            })
            .collect();
        let xbox = xbox_picture(&t.rgba, t.width, &frames, &fonts);
        if let Ok(out) = std::env::var("STREET_PROMPTS_DUMP") {
            std::fs::write(format!("{out}/playstation.rgba"), &t.rgba).unwrap();
            std::fs::write(format!("{out}/xbox.rgba"), &xbox).unwrap();
        }
        let pixels = |rgba: &[u8], k: usize| -> Vec<[u8; 4]> {
            let (x, y, w, h) = bounds(frames[k]);
            (y..y + h)
                .flat_map(|j| (x..x + w).map(move |i| (i, j)))
                .map(|(i, j)| {
                    let at = ((j * t.width + i) * 4) as usize;
                    rgba[at..at + 4].try_into().unwrap()
                })
                .collect()
        };
        // Cross becomes A: green where the light blue symbol was.
        let green = |p: &[u8; 4]| {
            let [r, g, b, a] = p.map(i32::from);
            a > 200 && g > r + 40 && g > b + 40
        };
        assert!(pixels(&t.rgba, 3).iter().all(|p| !green(p)));
        assert!(pixels(&xbox, 3).iter().filter(|p| green(p)).count() > 20);
        // Triangle's green is gone (Y is yellow), the ring kept.
        assert!(pixels(&xbox, 2).iter().filter(|p| green(p)).count() < 3);
        assert_eq!(pixels(&xbox, 2)[0], pixels(&t.rgba, 2)[0]);
        // The sticks' frames are left as they are.
        for k in [10, 11, 14, 15] {
            assert_eq!(pixels(&xbox, k), pixels(&t.rgba, k), "frame {k}");
        }
    }

    #[test]
    fn shrinking_keeps_the_ink() {
        let ink = Ink {
            width: 4,
            height: 4,
            rgba: [255u8, 255, 255, 255].repeat(16),
        };
        let s = shrink(&ink, 2);
        assert_eq!((s.width, s.height), (2, 2));
        assert!(s.rgba.chunks(4).all(|p| p == [255, 255, 255, 255]));
    }
}
