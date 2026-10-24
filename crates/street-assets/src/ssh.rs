//! SHPS texture sheets (`.ssh`), PS2 flavour.
//!
//! A sheet holds named images (4-character names such as `gras` or `ban1`).
//! Each image is a chain of 16-byte-headed blocks: the pixels first, then a
//! palette and metadata blocks (`0x69`, `0x70`, full name) linked by the
//! "next" offset in each header.
//!
//! PS2 details handled here:
//! - 8-bit palettes are stored in the GS's CSM1 order: in every group of 32
//!   entries, entries 8..15 and 16..23 are swapped. The palette block's width
//!   counts the colours used, which is fewer than the entries stored when the
//!   swap moves a used colour past that count, so the whole block is read.
//! - Alpha runs 0..=0x80, where 0x80 is opaque. It is doubled to 0..=255.

use crate::{Error, Result, bytes};

const PIXELS_4BIT: u8 = 0x01;
const PIXELS_8BIT: u8 = 0x02;
/// The front end's strips: plain 24-bit RGB (`/FE/XAFEBG.BIN`, its pixel
/// block bodies are width*height*3 with no palette along).
const PIXELS_24BIT: u8 = 0x04;
const PIXELS_32BIT: u8 = 0x05;
const PALETTE_32BIT: u8 = 0x21;

#[derive(Clone, Debug)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Straight (non-premultiplied) RGBA8, row-major, top row first.
    pub rgba: Vec<u8>,
    /// A palettised image's index per pixel, in palette-entry order (the
    /// CSM1 swizzle undone), for drawing it in another palette as the
    /// game's palette swaps do (`FUN_00210368`); `None` for direct colour.
    pub indices: Option<Vec<u8>>,
}

/// A palette as the game applies one: 256 entries in index order, RGBA
/// with the PS2's alpha doubled.
pub type Palette = Vec<[u8; 4]>;

impl Texture {
    /// True when any pixel is not fully opaque.
    pub fn has_alpha(&self) -> bool {
        self.rgba.as_chunks::<4>().0.iter().any(|p| p[3] < 255)
    }

    /// True when some pixels are partly (not fully) transparent, which needs
    /// blending rather than an alpha test.
    pub fn has_partial_alpha(&self) -> bool {
        let partial = self
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 16 && p[3] < 239)
            .count();
        partial * 50 > self.rgba.len() / 4 // more than 2% of pixels
    }
}

/// Four authored texture-coordinate corners from a 0x7c block.
/// Coordinates are signed integer pixels, in the native draw order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteFrame {
    pub corners: [[i32; 2]; 4],
}

impl SpriteFrame {
    /// Bounding rectangle in source pixels. Corner order remains available
    /// for native flips/rotation; this helper alone does not reproduce them.
    pub fn bounds(self) -> [i32; 4] {
        let min_x = self.corners.iter().map(|p| p[0]).min().unwrap();
        let min_y = self.corners.iter().map(|p| p[1]).min().unwrap();
        let max_x = self.corners.iter().map(|p| p[0]).max().unwrap();
        let max_y = self.corners.iter().map(|p| p[1]).max().unwrap();
        [min_x, min_y, max_x - min_x, max_y - min_y]
    }
}

#[derive(Debug, Default)]
pub struct Sheet {
    pub textures: Vec<Texture>,
    /// Sprite frames, parallel to textures. Empty when no 0x7c block exists.
    pub frames: Vec<Vec<SpriteFrame>>,
    /// Images in a pixel or palette format this decoder does not handle yet:
    /// (name, pixel block type).
    pub skipped: Vec<(String, u8)>,
    /// Entries that are a palette alone (`/FE/XPAL_FE.BIN`'s 256x1 palette
    /// blocks with no pixels), in the archive's order: the colour sets the
    /// front end swaps into its palettised sprites.
    pub palettes: Vec<Palette>,
}

struct Block {
    start: usize,
    kind: u8,
    width: u16,
    height: u16,
    /// Bytes after the 16-byte header, up to the next block.
    body: std::ops::Range<usize>,
}

pub fn parse(buf: &[u8]) -> Result<Sheet> {
    if bytes::range(buf, 0, 4)? != b"SHPS" {
        return Err(Error::format("not an SHPS texture sheet"));
    }
    let count = bytes::u32_le(buf, 8)? as usize;
    if count > buf.len() / 8 {
        return Err(Error::format(format!("SHPS image count {count} too large")));
    }
    let mut dir = Vec::with_capacity(count);
    for i in 0..count {
        let raw = bytes::range(buf, 16 + i * 8, 4)?;
        let name: String = raw
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect();
        dir.push((name, bytes::u32_le(buf, 20 + i * 8)? as usize));
    }

    let mut sheet = Sheet::default();
    for (i, (name, start)) in dir.iter().enumerate() {
        let end = dir.get(i + 1).map_or(buf.len(), |(_, o)| *o).min(buf.len());
        let blocks = blocks(buf, *start, end)?;
        if let Some(first) = blocks.first()
            && first.kind == PALETTE_32BIT
        {
            sheet.palettes.push(palette(&buf[first.body.clone()]));
            continue;
        }
        match decode(buf, &blocks)? {
            Some((width, height, rgba, indices)) => {
                sheet.frames.push(sprite_frames(buf, &blocks)?);
                sheet.textures.push(Texture {
                    name: name.clone(),
                    width,
                    height,
                    rgba,
                    indices,
                });
            }
            None => sheet
                .skipped
                .push((name.clone(), blocks.first().map_or(0, |b| b.kind))),
        }
    }
    Ok(sheet)
}

fn blocks(buf: &[u8], start: usize, end: usize) -> Result<Vec<Block>> {
    let mut out = Vec::new();
    let mut pos = start;
    while pos + 16 <= end {
        let head = bytes::u32_le(buf, pos)?;
        let (kind, next) = ((head & 0xff) as u8, (head >> 8) as usize);
        let width = bytes::u16_le(buf, pos + 4)?;
        let height = bytes::u16_le(buf, pos + 6)?;
        let block_end = if next == 0 {
            end
        } else {
            (pos + next).min(end)
        };
        out.push(Block {
            start: pos,
            kind,
            width,
            height,
            body: pos + 16..block_end,
        });
        if next == 0 || out.len() > 64 {
            break;
        }
        pos += next;
    }
    Ok(out)
}

/// FUN_0039a900 returns block+8, with the corner count at block+4.
/// FUN_00210048 reads four signed (x,y) i32 pairs per frame, not the
/// ordinary pixel blocks' sixteen-byte header/body layout.
fn sprite_frames(buf: &[u8], blocks: &[Block]) -> Result<Vec<SpriteFrame>> {
    let Some(block) = blocks.iter().find(|b| b.kind == 0x7c) else {
        return Ok(Vec::new());
    };
    let points = bytes::u32_le(buf, block.start + 4)? as usize;
    let n = points
        .checked_mul(8)
        .ok_or_else(|| Error::format("sprite corner count overflow"))?;
    let start = block.start + 8;
    let end = start
        .checked_add(n)
        .ok_or_else(|| Error::format("sprite corners overflow"))?;
    if end > block.body.end {
        return Err(Error::format("sprite corners outside their metadata block"));
    }
    let data = bytes::range(buf, start, n)?;
    data.as_chunks::<32>()
        .0
        .iter()
        .map(|frame| {
            let mut corners = [[0; 2]; 4];
            for (i, p) in corners.iter_mut().enumerate() {
                p[0] = bytes::u32_le(frame, i * 8)? as i32;
                p[1] = bytes::u32_le(frame, i * 8 + 4)? as i32;
            }
            Ok(SpriteFrame { corners })
        })
        .collect()
}

/// The PS2's alpha (0x80 opaque) as 0..=255.
fn ps2_alpha(a: u8) -> u8 {
    (a as u16 * 2).min(255) as u8
}

/// A palette block's body as the game indexes it: the CSM1 storage order
/// undone, so entry `i` is the colour pixel index `i` names. Short blocks
/// are padded with magenta, as an index past a short palette reads.
pub fn palette(body: &[u8]) -> Palette {
    let stored: Vec<[u8; 4]> = body
        .as_chunks::<4>()
        .0
        .iter()
        .take(256)
        .map(|c| [c[0], c[1], c[2], ps2_alpha(c[3])])
        .collect();
    (0..256)
        .map(|i| stored.get(csm1(i)).copied().unwrap_or([255, 0, 255, 255]))
        .collect()
}

/// `Ok(None)` for pixel formats not handled yet: the size, the pixels and,
/// for a palettised image, its index plane.
#[allow(clippy::type_complexity)]
fn decode(buf: &[u8], blocks: &[Block]) -> Result<Option<(u32, u32, Vec<u8>, Option<Vec<u8>>)>> {
    let Some(img) = blocks.first() else {
        return Ok(None);
    };
    let (w, h) = (img.width as usize, img.height as usize);
    let pixels = &buf[img.body.clone()];
    match img.kind {
        PIXELS_8BIT | PIXELS_4BIT => {
            let pal = blocks[1..]
                .iter()
                .find(|b| b.kind == PALETTE_32BIT)
                .ok_or_else(|| Error::format("palettised image without a 32-bit palette"))?;
            // The palette's width is the number of colours used, but the block
            // stores them in CSM1 order, so read everything up to the next block.
            let pal_bytes = &buf[pal.body.clone()];
            let pal_bytes = &pal_bytes[..(pal_bytes.len() / 4).min(256) * 4];
            let need = if img.kind == PIXELS_8BIT {
                w * h
            } else {
                (w * h).div_ceil(2)
            };
            let px = bytes::range(pixels, 0, need)?;
            let mut rgba = Vec::with_capacity(w * h * 4);
            let mut indices = Vec::with_capacity(w * h);
            for i in 0..w * h {
                let index = if img.kind == PIXELS_8BIT {
                    px[i] as usize
                } else {
                    ((px[i / 2] >> (4 * (i % 2))) & 0xf) as usize
                };
                let stored = if img.kind == PIXELS_8BIT {
                    csm1(index)
                } else {
                    index
                };
                indices.push(index as u8);
                match pal_bytes.get(stored * 4..stored * 4 + 4) {
                    Some(c) => rgba.extend([c[0], c[1], c[2], ps2_alpha(c[3])]),
                    None => rgba.extend([255, 0, 255, 255]), // index past a short palette
                }
            }
            Ok(Some((w as u32, h as u32, rgba, Some(indices))))
        }
        PIXELS_32BIT => {
            let px = bytes::range(pixels, 0, w * h * 4)?;
            let rgba = px
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|c| [c[0], c[1], c[2], ps2_alpha(c[3])])
                .collect();
            Ok(Some((w as u32, h as u32, rgba, None)))
        }
        PIXELS_24BIT => {
            // The front end's strips: plain RGB triples, every pixel opaque.
            let px = bytes::range(pixels, 0, w * h * 3)?;
            let rgba = px
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|c| [c[0], c[1], c[2], 255])
                .collect();
            Ok(Some((w as u32, h as u32, rgba, None)))
        }
        _ => Ok(None),
    }
}

impl Texture {
    /// This image drawn in another palette, as the game's palette swap
    /// (`FUN_00210368`) shows a palettised sprite: each pixel's index
    /// looked up in `palette`. A direct-colour image is returned as it is.
    pub fn in_palette(&self, palette: &[[u8; 4]]) -> Texture {
        let Some(indices) = &self.indices else {
            return self.clone();
        };
        let rgba = indices
            .iter()
            .flat_map(|&i| {
                palette
                    .get(usize::from(i))
                    .copied()
                    .unwrap_or([255, 0, 255, 255])
            })
            .collect();
        Texture {
            name: self.name.clone(),
            width: self.width,
            height: self.height,
            rgba,
            indices: Some(indices.clone()),
        }
    }
}

/// Map a pixel's palette index to where the GS's CSM1 layout stored that colour.
fn csm1(i: usize) -> usize {
    (i & !0x18) | ((i & 0x08) << 1) | ((i & 0x10) >> 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csm1_swaps_the_middle_eights() {
        assert_eq!(csm1(0), 0);
        assert_eq!(csm1(7), 7);
        assert_eq!(csm1(8), 16);
        assert_eq!(csm1(16), 8);
        assert_eq!(csm1(23), 15);
        assert_eq!(csm1(24), 24);
        assert_eq!(csm1(40), 48);
    }

    /// A one-image sheet: 2x1 8-bit pixels using palette entries 0 and 8.
    fn sheet() -> Vec<u8> {
        let mut b = b"SHPS".to_vec();
        b.extend(0u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend(b"G335");
        b.extend(b"test");
        b.extend(24u32.to_le_bytes());
        // pixel block: type 2, next = 16 + 16 (header + 2 pixels padded), 2x1
        b.extend((PIXELS_8BIT as u32 | (32 << 8)).to_le_bytes());
        b.extend([2, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        b.extend([0, 8]);
        b.extend([0; 14]);
        // palette block: type 0x21, last block, 32 entries
        b.extend((PALETTE_32BIT as u32).to_le_bytes());
        b.extend([32, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        for i in 0..32u8 {
            b.extend([i, 0, 0, 0x80]);
        }
        b
    }

    #[test]
    fn sprite_metadata_keeps_first_corner_and_rejects_truncated_point_tables() {
        let mut b = sheet();
        b[56..60].copy_from_slice(&(PALETTE_32BIT as u32 | (144 << 8)).to_le_bytes());
        let start = b.len();
        b.extend((0x7cu32 | (40 << 8)).to_le_bytes());
        b.extend(4u32.to_le_bytes());
        for p in [[-3i32, 4], [7, 4], [7, 12], [-3, 12]] {
            b.extend(p[0].to_le_bytes());
            b.extend(p[1].to_le_bytes());
        }
        let parsed = parse(&b).unwrap();
        assert_eq!(
            parsed.frames[0][0].corners,
            [[-3, 4], [7, 4], [7, 12], [-3, 12]]
        );
        b[start + 4..start + 8].copy_from_slice(&5u32.to_le_bytes());
        assert!(parse(&b).is_err());
    }

    #[test]
    fn decodes_8bit_with_csm1_palette_and_ps2_alpha() {
        let s = parse(&sheet()).unwrap();
        assert!(s.skipped.is_empty());
        let t = &s.textures[0];
        assert_eq!((t.name.as_str(), t.width, t.height), ("test", 2, 1));
        // index 8 reads stored entry 16 (CSM1); alpha 0x80 becomes 255
        assert_eq!(t.rgba, [0, 0, 0, 255, 16, 0, 0, 255]);
        assert!(!t.has_alpha());
        // The index plane keeps the pixels' own indices, so another palette
        // in index order recolours them.
        assert_eq!(t.indices.as_deref(), Some(&[0u8, 8][..]));
        let mut other = vec![[0u8, 0, 0, 255]; 256];
        other[8] = [7, 7, 7, 255];
        assert_eq!(t.in_palette(&other).rgba, [0, 0, 0, 255, 7, 7, 7, 255]);
    }

    #[test]
    fn a_palette_alone_is_kept_in_index_order() {
        // A sheet whose one entry is a palette block with no pixels (as
        // `/FE/XPAL_FE.BIN` stores its colour sets).
        let mut b = b"SHPS".to_vec();
        b.extend(0u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend(b"G335");
        b.extend(b"0000");
        b.extend(24u32.to_le_bytes());
        b.extend((PALETTE_32BIT as u32).to_le_bytes());
        b.extend([32, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        for i in 0..32u8 {
            b.extend([i, 0, 0, 0x80]);
        }
        let s = parse(&b).unwrap();
        assert!(s.textures.is_empty() && s.skipped.is_empty());
        assert_eq!(s.palettes.len(), 1);
        let p = &s.palettes[0];
        assert_eq!(p.len(), 256);
        // Index 8 names stored entry 16 (CSM1), index 16 stored entry 8.
        assert_eq!(
            (p[0], p[8], p[16], p[7]),
            (
                [0, 0, 0, 255],
                [16, 0, 0, 255],
                [8, 0, 0, 255],
                [7, 0, 0, 255]
            )
        );
        assert_eq!(p[40], [255, 0, 255, 255]);
    }
}
