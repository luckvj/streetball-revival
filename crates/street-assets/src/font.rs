//! The front end's bitmap fonts (`FNTS`) and layouts (`XLAY`).
//!
//! ## Fonts
//!
//! `/FE/XFONTSEN.BIN` (English) is a `c0 fb` archive of 12 fonts; a layout
//! text element names its font by index in prop 13 (titles 6, menu entries
//! 4, help lines 0). Each font, little-endian:
//!
//! | Offset | Contents |
//! |---|---|
//! | 0x00 | `FNTS`, `PD`, u16 |
//! | 0x0a | u16 glyph count |
//! | 0x14 | u32 offset of the glyph table |
//! | 0x18 | u32 offset of the kerning table (0: none) |
//! | 0x1c | u32 offset of the texture |
//! | 0x24 | u32 line height |
//!
//! A glyph is 12 bytes: u16 character, u8 width, u8 height, u16 x and u16 y
//! in the texture, u8 advance, i8 x offset, i8 top offset (from the line's
//! top; the script title font's capitals rise above it), u8. Kerning: u32 count, then (u16 left character, i8 adjustment, u8
//! right character). The texture: a format byte (1: 4-bit, 2: 8-bit), a
//! byte, u16, u16 width, u16 height, 8 bytes, then the pixels (4-bit: low
//! nibble first) as coverage, and a palette.
//!
//! ## Layouts
//!
//! `/FE/XLAY_FE.BIN` (61 screens) and `XLAY_GM.BIN` (12) are `c0 fb`
//! archives of layout blobs: u32 1, u16 element count, u16 table offset
//! (0x0c), u32 0, then per element u16 property count and u32 offset of its
//! properties, each property a u16 id and u32 value. Property 0 packs the
//! element: bits 0..3 alignment, 4..11 the widget kind, 12 up the element
//! id (the screen class for the one kind-0 element). Positions (prop 3) and
//! sizes (prop 4) are two u16 on a 640x480 screen; prop 10 is ARGB; text
//! widgets give their font in prop 13 and their text ids (`file << 8 |
//! index` into the locale table at 0x3ec2f0) in props 14..20 or 56..63.
//! See `docs/TRACE.md` section 4 for the widget kinds.

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek};

use crate::{Error, Result, bytes, iso::Iso, locale};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub x: u16,
    pub y: u16,
    pub width: u8,
    pub height: u8,
    pub advance: u8,
    pub x_offset: i8,
    pub top: i8,
}

#[derive(Clone, Debug)]
pub struct Font {
    /// The header's word at 0x24: what the text widget's measure
    /// (`FUN_00239e70`, through `FUN_003a3c68`) advances by between lines.
    pub line_height: u32,
    /// The header's byte at 0x12: the height `FUN_00239e70` gives the last
    /// (or only) line, so a one-line text's box is this tall, not
    /// `line_height`; its middle and bottom anchors and the row widget's
    /// stars (`FUN_0021c9e0`) measure by it.
    pub text_height: u32,
    pub width: u32,
    pub height: u32,
    /// Coverage 0..255, row-major.
    pub coverage: Vec<u8>,
    pub glyphs: BTreeMap<char, Glyph>,
    pub kerning: HashMap<(char, char), i8>,
}

/// The executable's own font (`DAT_004b3404` = `0x430b30`, an `FNTS` of
/// 0x1550 bytes in its data): a text widget's font until it is given one
/// (`FUN_00238a78`), so the one a widget wraps with when its text and
/// width come before its font (`FUN_00250000`'s Help rows).
pub fn system(executable: &[u8]) -> Result<Font> {
    parse(&crate::scripts::executable_bytes(
        executable, 0x430b30, 0x1550,
    )?)
}

pub fn parse(d: &[u8]) -> Result<Font> {
    if d.get(..4) != Some(b"FNTS") {
        return Err(Error::Format("not an FNTS font".into()));
    }
    let count = bytes::u16_le(d, 0x0a)? as usize;
    let table = bytes::u32_le(d, 0x14)? as usize;
    let kern = bytes::u32_le(d, 0x18)? as usize;
    let texture = bytes::u32_le(d, 0x1c)? as usize;
    let line_height = bytes::u32_le(d, 0x24)?;
    let text_height = u32::from(*d.get(0x12).unwrap_or(&0));
    let mut glyphs = BTreeMap::new();
    for i in 0..count {
        let g = table + 12 * i;
        let code = bytes::u16_le(d, g)? as u32;
        let r = bytes::range(d, g + 2, 10)?;
        if let Some(c) = char::from_u32(code) {
            glyphs.insert(
                c,
                Glyph {
                    width: r[0],
                    height: r[1],
                    x: u16::from_le_bytes([r[2], r[3]]),
                    y: u16::from_le_bytes([r[4], r[5]]),
                    advance: r[6],
                    x_offset: r[7] as i8,
                    top: r[8] as i8,
                },
            );
        }
    }
    let mut kerning = HashMap::new();
    if kern != 0 && kern < texture {
        let n = bytes::u32_le(d, kern)? as usize;
        for i in 0..n.min((texture - kern) / 4) {
            let k = bytes::range(d, kern + 4 + 4 * i, 4)?;
            let left = char::from_u32(u16::from_le_bytes([k[0], k[1]]) as u32);
            let right = char::from_u32(k[3] as u32);
            if let (Some(l), Some(r)) = (left, right) {
                kerning.insert((l, r), k[2] as i8);
            }
        }
    }
    let format = *d
        .get(texture)
        .ok_or_else(|| Error::Format("font texture missing".into()))?;
    let width = bytes::u16_le(d, texture + 4)? as u32;
    let height = bytes::u16_le(d, texture + 6)? as u32;
    let pixels = texture + 16;
    let n = (width * height) as usize;
    let coverage = match format {
        1 => {
            let raw = bytes::range(d, pixels, n.div_ceil(2))?;
            let mut out = Vec::with_capacity(n);
            for &b in raw {
                out.push((b & 15) * 17);
                out.push((b >> 4) * 17);
            }
            out.truncate(n);
            out
        }
        2 => bytes::range(d, pixels, n)?.to_vec(),
        f => {
            return Err(Error::Format(format!(
                "font texture format {f} not handled"
            )));
        }
    };
    Ok(Font {
        line_height,
        text_height,
        width,
        height,
        coverage,
        glyphs,
        kerning,
    })
}

/// The English fonts, `/FE/XFONTSEN.BIN`.
pub fn fonts<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Vec<Font>> {
    let data = iso.read("/FE/XFONTSEN.BIN")?;
    locale::c0fb(&data)?
        .iter()
        .map(|e| parse(bytes::range(&data, e.offset as usize, e.size as usize)?))
        .collect()
}

/// A rendered line of text: straight RGBA.
pub struct Rendered {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Where the line's top-left corner falls in the image (glyphs may
    /// reach left of and above it).
    pub origin: (u32, u32),
}

impl Font {
    /// The width of `text` to its last glyph's ink: the advances of the
    /// glyphs before it, then that glyph's offset and width. What a dialog
    /// fits a line to (a line ending in a glyph narrower than its advance
    /// fits where [`Font::measure`] would not).
    pub fn ink_width(&self, text: &str) -> i32 {
        let Some(last) = text.chars().rev().find_map(|c| self.glyph(c)) else {
            return 0;
        };
        self.measure(text) - i32::from(last.advance)
            + i32::from(last.x_offset)
            + i32::from(last.width)
    }

    /// The width of `text` in pixels.
    pub fn measure(&self, text: &str) -> i32 {
        let mut x = 0i32;
        let mut prev = None;
        for c in text.chars() {
            if let Some(g) = self.glyph(c) {
                if let Some(p) = prev {
                    x += i32::from(*self.kerning.get(&(p, c)).unwrap_or(&0));
                }
                x += i32::from(g.advance);
            }
            prev = Some(c);
        }
        x
    }

    fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyphs
            .get(&c)
            .or_else(|| self.glyphs.get(&c.to_ascii_uppercase()))
    }

    /// Draws one line in `top` colour fading to `bottom` down the line
    /// (RGBA, straight).
    pub fn render(&self, text: &str, top: [u8; 4], bottom: [u8; 4]) -> Rendered {
        let line = self.line_height.max(1) as i32;
        let (mut above, mut below) = (0i32, line);
        for g in text.chars().filter_map(|c| self.glyph(c)) {
            above = above.min(i32::from(g.top));
            below = below.max(i32::from(g.top) + i32::from(g.height));
        }
        let pad = 4i32;
        let width = (self.measure(text).max(1) + 2 * pad) as u32;
        let height = (below - above) as u32;
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        let mut x = pad;
        let mut prev = None;
        for c in text.chars() {
            let Some(g) = self.glyph(c) else {
                prev = Some(c);
                continue;
            };
            if let Some(p) = prev {
                x += i32::from(*self.kerning.get(&(p, c)).unwrap_or(&0));
            }
            for gy in 0..u32::from(g.height) {
                let line_y = i32::from(g.top) + gy as i32;
                let ty = (line_y - above) as u32;
                let t = (line_y as f32 / line as f32).clamp(0.0, 1.0);
                let colour: [u8; 4] = std::array::from_fn(|k| {
                    (f32::from(top[k]) + (f32::from(bottom[k]) - f32::from(top[k])) * t) as u8
                });
                for gx in 0..u32::from(g.width) {
                    let tx = x + i32::from(g.x_offset) + gx as i32;
                    if tx < 0 || tx as u32 >= width {
                        continue;
                    }
                    let (sx, sy) = (u32::from(g.x) + gx, u32::from(g.y) + gy);
                    if sx >= self.width || sy >= self.height {
                        continue;
                    }
                    let cov = self.coverage[(sy * self.width + sx) as usize];
                    let at = ((ty * width + tx as u32) * 4) as usize;
                    let a = (u32::from(cov) * u32::from(colour[3]) / 255) as u8;
                    if a > rgba[at + 3] {
                        rgba[at..at + 3].copy_from_slice(&colour[..3]);
                        rgba[at + 3] = a;
                    }
                }
            }
            x += i32::from(g.advance);
            prev = Some(c);
        }
        Rendered {
            width,
            height,
            rgba,
            origin: (pad as u32, (-above) as u32),
        }
    }
}

/// One element of a layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub kind: u8,
    pub id: u32,
    pub align: u8,
    pub props: BTreeMap<u16, u32>,
}

impl Element {
    /// Prop 3: position on the 640x480 screen.
    pub fn position(&self) -> Option<(u16, u16)> {
        self.props.get(&3).map(|&v| (v as u16, (v >> 16) as u16))
    }

    pub fn size(&self) -> Option<(u16, u16)> {
        self.props.get(&4).map(|&v| (v as u16, (v >> 16) as u16))
    }

    /// Prop 10 as RGBA.
    pub fn colour(&self) -> Option<[u8; 4]> {
        self.props.get(&10).map(|&v| argb(v))
    }

    /// The text ids it shows: props 14..20, else 56..63, non-zero.
    pub fn texts(&self) -> Vec<u16> {
        let pick = |range: std::ops::RangeInclusive<u16>| -> Vec<u16> {
            range
                .filter_map(|p| self.props.get(&p).copied())
                .filter(|&v| v != 0)
                .map(|v| v as u16)
                .collect()
        };
        let a = pick(14..=20);
        if a.is_empty() { pick(56..=63) } else { a }
    }
}

/// An ARGB word as RGBA bytes.
pub fn argb(v: u32) -> [u8; 4] {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8, (v >> 24) as u8]
}

/// A screen's layout: its class (the kind-0 element's id) and elements.
#[derive(Clone, Debug)]
pub struct Layout {
    pub class: u32,
    pub elements: Vec<Element>,
}

pub fn layout(b: &[u8]) -> Result<Layout> {
    let n = bytes::u16_le(b, 4)? as usize;
    let table = bytes::u16_le(b, 6)? as usize;
    let mut elements = Vec::with_capacity(n);
    for i in 0..n {
        let count = bytes::u16_le(b, table + 6 * i)? as usize;
        let at = bytes::u32_le(b, table + 6 * i + 2)? as usize;
        let mut props = BTreeMap::new();
        for j in 0..count {
            props.insert(
                bytes::u16_le(b, at + 6 * j)?,
                bytes::u32_le(b, at + 6 * j + 2)?,
            );
        }
        let p = props.get(&0).copied().unwrap_or(0);
        elements.push(Element {
            kind: ((p >> 4) & 0xff) as u8,
            id: p >> 12,
            align: (p & 15) as u8,
            props,
        });
    }
    let class = elements
        .iter()
        .find(|e| e.kind == 0)
        .map_or(u32::MAX, |e| e.id);
    Ok(Layout { class, elements })
}

/// Every layout of `/FE/XLAY_FE.BIN` or `XLAY_GM.BIN`.
pub fn layouts<R: Read + Seek>(iso: &mut Iso<R>, path: &str) -> Result<Vec<Layout>> {
    let data = iso.read(path)?;
    locale::c0fb(&data)?
        .iter()
        .map(|e| layout(bytes::range(&data, e.offset as usize, e.size as usize)?))
        .collect()
}

#[cfg(test)]
mod ink_tests {
    use super::*;

    #[test]
    fn the_ink_width_ends_at_the_last_glyphs_ink() {
        let glyph = |width, advance, x_offset| Glyph {
            x: 0,
            y: 0,
            width,
            height: 1,
            advance,
            x_offset,
            top: 0,
        };
        let font = Font {
            line_height: 1,
            text_height: 1,
            width: 1,
            height: 1,
            coverage: vec![0],
            glyphs: BTreeMap::from([('a', glyph(6, 9, 1)), ('b', glyph(4, 7, 0))]),
            kerning: HashMap::new(),
        };
        assert_eq!(font.measure("ab"), 16);
        assert_eq!(font.ink_width("ab"), 9 + 4);
        assert_eq!(font.ink_width("ba"), 7 + 1 + 6);
        assert_eq!(font.ink_width(""), 0);
    }
}

#[cfg(test)]
mod system_font_tests {
    #[test]
    fn the_executables_own_font_measures_narrower_than_the_front_ends() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut iso = crate::iso::Iso::open(path).unwrap();
        let elf = iso.read("/SLUS_206.51").unwrap();
        let system = super::system(&elf).unwrap();
        let fonts = super::fonts(&mut iso).unwrap();
        // The Help row the original leaves on one line at its 220.
        let row = "Back Up To Previous Screen";
        assert!(system.measure(row) <= 220, "{}", system.measure(row));
        assert!(fonts[1].measure(row) > 220, "{}", fonts[1].measure(row));
    }
}
