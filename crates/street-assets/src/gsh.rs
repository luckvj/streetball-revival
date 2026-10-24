//! SHPG texture sheets (`.gsh`), the GameCube flavour of the SHPS sheets in
//! [`crate::ssh`]: the same directory and chained 16-byte-headed blocks, with
//! big-endian numbers and the GameCube's (GX) tiled pixel formats.
//!
//! Header: `SHPG`, u32 size, u32 image count, a 4-byte tag (`G359` in V3),
//! then per image a 4-character name and a u32 offset. Each image block:
//! `u8 type | u24 next`, u16 width, u16 height, u16 centre x and y, u16
//! position x and y (low 12 bits; the top four are flags), pixels. V3's
//! layered player sheets use the position: a tattoo or eyebrow image is
//! drawn at it on the layers below (see [`crate::gc_character`]).
//!
//! | Type | GX format | Layout |
//! |---|---|---|
//! | `0x16` | RGBA8 | 4×4 tiles of 64 bytes: 16 alpha/red pairs, then 16 green/blue pairs |
//! | `0x19` | C8 | 8×4 tiles of 8-bit indices into the image's palette block |
//! | `0x1e` | CMPR | 8×8 tiles of four 4×4 DXT1 blocks (RGB565, big-endian) |
//! | `0x32` | palette | RGB5A3 entries, u16 big-endian, width × height of them |
//!
//! Worked out on Street V3's (`G3VE69`) player sheets: each pixel block's
//! size is the header plus width × height × bits per pixel, and the decoded
//! faces, shirts and logos come out right.

use crate::{Error, Result, bytes, ssh::Texture};

const RGBA8: u8 = 0x16;
const C8: u8 = 0x19;
const CMPR: u8 = 0x1e;
const PALETTE: u8 = 0x32;
/// The pixel block types [`parse`] decodes; it skips the others.
pub(crate) const DECODED: [u8; 3] = [RGBA8, C8, CMPR];

/// A sheet's images, and the (name, type) of any in a format not decoded yet.
pub struct Sheet {
    pub textures: Vec<Texture>,
    /// Each decoded image's position (x, y), in the same order.
    pub positions: Vec<[u16; 2]>,
    pub skipped: Vec<(String, u8)>,
}

pub fn parse(buf: &[u8]) -> Result<Sheet> {
    if bytes::range(buf, 0, 4)? != b"SHPG" {
        return Err(Error::format("not an SHPG sheet"));
    }
    let count = bytes::u32_be(buf, 8)? as usize;
    if count > buf.len() / 8 {
        return Err(Error::format(format!("SHPG image count {count} too large")));
    }
    let mut textures = Vec::new();
    let mut positions = Vec::new();
    let mut skipped = Vec::new();
    for i in 0..count {
        let name: String = bytes::range(buf, 16 + i * 8, 4)?
            .iter()
            .map(|&c| c as char)
            .collect();
        let first = bytes::u32_be(buf, 20 + i * 8)? as usize;
        // The image's blocks: pixels first, then its palette and metadata.
        let mut blocks = Vec::new();
        let mut pos = first;
        loop {
            let head = bytes::range(buf, pos, 16)?;
            let next = u32::from_be_bytes([0, head[1], head[2], head[3]]) as usize;
            blocks.push((head[0], pos));
            if next == 0 || blocks.len() > 64 {
                break;
            }
            pos = pos
                .checked_add(next)
                .ok_or_else(|| Error::format("SHPG block chain overflows"))?;
        }
        let (kind, at) = blocks[0];
        let width = bytes::u16_be(buf, at + 4)? as usize;
        let height = bytes::u16_be(buf, at + 6)? as usize;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(Error::format(format!(
                "SHPG image {name} is {width}x{height}"
            )));
        }
        let pixels = buf.get(at + 16..).unwrap_or_default();
        let rgba = match kind {
            RGBA8 => rgba8(pixels, width, height)?,
            CMPR => cmpr(pixels, width, height)?,
            C8 => {
                let &(_, pal_at) = blocks
                    .iter()
                    .find(|(k, _)| *k == PALETTE)
                    .ok_or_else(|| Error::format(format!("SHPG image {name} has no palette")))?;
                let n = bytes::u16_be(buf, pal_at + 4)? as usize
                    * bytes::u16_be(buf, pal_at + 6)? as usize;
                let palette = (0..n.min(256))
                    .map(|k| bytes::u16_be(buf, pal_at + 16 + 2 * k).map(rgb5a3))
                    .collect::<Result<Vec<_>>>()?;
                c8(pixels, width, height, &palette)?
            }
            other => {
                skipped.push((name, other));
                continue;
            }
        };
        positions.push([
            bytes::u16_be(buf, at + 12)? & 0x0fff,
            bytes::u16_be(buf, at + 14)? & 0x0fff,
        ]);
        textures.push(Texture {
            name,
            width: width as u32,
            height: height as u32,
            rgba,
            indices: None,
        });
    }
    Ok(Sheet {
        textures,
        positions,
        skipped,
    })
}

fn need(pixels: &[u8], n: usize) -> Result<()> {
    if pixels.len() < n {
        return Err(Error::format("SHPG pixels end early"));
    }
    Ok(())
}

/// Tiles are padded out to whole tiles; only pixels inside the image are kept.
fn put(out: &mut [u8], width: usize, height: usize, x: usize, y: usize, px: [u8; 4]) {
    if x < width && y < height {
        let o = (y * width + x) * 4;
        out[o..o + 4].copy_from_slice(&px);
    }
}

fn rgba8(p: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
    let (tw, th) = (width.div_ceil(4), height.div_ceil(4));
    need(p, tw * th * 64)?;
    let mut out = vec![0; width * height * 4];
    let mut i = 0;
    for ty in 0..th {
        for tx in 0..tw {
            let (ar, gb) = (&p[i..i + 32], &p[i + 32..i + 64]);
            for k in 0..16 {
                put(
                    &mut out,
                    width,
                    height,
                    tx * 4 + k % 4,
                    ty * 4 + k / 4,
                    [ar[2 * k + 1], gb[2 * k], gb[2 * k + 1], ar[2 * k]],
                );
            }
            i += 64;
        }
    }
    Ok(out)
}

fn c8(p: &[u8], width: usize, height: usize, palette: &[[u8; 4]]) -> Result<Vec<u8>> {
    let (tw, th) = (width.div_ceil(8), height.div_ceil(4));
    need(p, tw * th * 32)?;
    let mut out = vec![0; width * height * 4];
    let mut i = 0;
    for ty in 0..th {
        for tx in 0..tw {
            for y in 0..4 {
                for x in 0..8 {
                    let colour = palette
                        .get(p[i] as usize)
                        .copied()
                        .unwrap_or([255, 0, 255, 255]);
                    put(&mut out, width, height, tx * 8 + x, ty * 4 + y, colour);
                    i += 1;
                }
            }
        }
    }
    Ok(out)
}

fn cmpr(p: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
    let (tw, th) = (width.div_ceil(8), height.div_ceil(8));
    need(p, tw * th * 32)?;
    let mut out = vec![0; width * height * 4];
    let mut i = 0;
    for ty in 0..th {
        for tx in 0..tw {
            for sub in 0..4 {
                let (bx, by) = (tx * 8 + (sub & 1) * 4, ty * 8 + (sub >> 1) * 4);
                let c0 = u16::from_be_bytes([p[i], p[i + 1]]);
                let c1 = u16::from_be_bytes([p[i + 2], p[i + 3]]);
                let (a, b) = (rgb565(c0), rgb565(c1));
                let mix = |wa: u32, wb: u32, d: u32| -> [u8; 4] {
                    let m = |k: usize| ((a[k] as u32 * wa + b[k] as u32 * wb) / d) as u8;
                    [m(0), m(1), m(2), 255]
                };
                let colours = if c0 > c1 {
                    [a, b, mix(2, 1, 3), mix(1, 2, 3)]
                } else {
                    [a, b, mix(1, 1, 2), [0, 0, 0, 0]]
                };
                for y in 0..4 {
                    let row = p[i + 4 + y];
                    for x in 0..4 {
                        let idx = (row >> (6 - 2 * x)) & 3;
                        put(
                            &mut out,
                            width,
                            height,
                            bx + x,
                            by + y,
                            colours[idx as usize],
                        );
                    }
                }
                i += 8;
            }
        }
    }
    Ok(out)
}

fn rgb565(c: u16) -> [u8; 4] {
    let r = (c >> 11) & 31;
    let g = (c >> 5) & 63;
    let b = c & 31;
    [
        (r * 255 / 31) as u8,
        (g * 255 / 63) as u8,
        (b * 255 / 31) as u8,
        255,
    ]
}

/// RGB5A3: opaque RGB555 when the top bit is set, else ARGB3444.
fn rgb5a3(c: u16) -> [u8; 4] {
    if c & 0x8000 != 0 {
        let f = |s: u16| (((c >> s) & 31) * 255 / 31) as u8;
        [f(10), f(5), f(0), 255]
    } else {
        let f = |s: u16| (((c >> s) & 15) * 17) as u8;
        [f(8), f(4), f(0), (((c >> 12) & 7) * 255 / 7) as u8]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-image sheet whose first block holds `pixels`, plus optional extra blocks.
    fn sheet(
        kind: u8,
        w: u16,
        h: u16,
        pixels: &[u8],
        extra: &[(u8, u16, u16, Vec<u8>)],
    ) -> Vec<u8> {
        let mut out = b"SHPG".to_vec();
        out.extend(0u32.to_be_bytes());
        out.extend(1u32.to_be_bytes());
        out.extend(b"G359");
        out.extend(b"text");
        out.extend(24u32.to_be_bytes());
        let mut blocks: Vec<(u8, u16, u16, Vec<u8>)> = vec![(kind, w, h, pixels.to_vec())];
        blocks.extend(extra.iter().cloned());
        for (k, (kind, w, h, body)) in blocks.iter().enumerate() {
            let next = if k + 1 == blocks.len() {
                0
            } else {
                16 + body.len() as u32
            };
            out.push(*kind);
            out.extend(&next.to_be_bytes()[1..]);
            out.extend(w.to_be_bytes());
            out.extend(h.to_be_bytes());
            out.extend([0; 8]);
            out.extend(body);
        }
        out
    }

    #[test]
    fn rgba8_tiles_split_alpha_red_and_green_blue() {
        let mut px = vec![0u8; 64];
        // pixel 0: A=0x11 R=0x22 G=0x33 B=0x44; pixel 5 (x=1, y=1): A=0xff R=1 G=2 B=3
        px[0] = 0x11;
        px[1] = 0x22;
        px[32] = 0x33;
        px[33] = 0x44;
        px[10] = 0xff;
        px[11] = 1;
        px[42] = 2;
        px[43] = 3;
        let s = parse(&sheet(RGBA8, 4, 4, &px, &[])).unwrap();
        let t = &s.textures[0];
        assert_eq!(&t.rgba[0..4], &[0x22, 0x33, 0x44, 0x11]);
        assert_eq!(&t.rgba[(4 + 1) * 4..(4 + 1) * 4 + 4], &[1, 2, 3, 0xff]);
    }

    #[test]
    fn cmpr_decodes_a_dxt1_block_in_tile_order() {
        // One 8x8 tile; sub-block 1 (top right) is pure red, index 0 everywhere.
        let mut px = vec![0u8; 32];
        px[8..10].copy_from_slice(&0xf800u16.to_be_bytes());
        px[10..12].copy_from_slice(&0x0000u16.to_be_bytes());
        let s = parse(&sheet(CMPR, 8, 8, &px, &[])).unwrap();
        let t = &s.textures[0];
        let at = |x: usize, y: usize| &t.rgba[(y * 8 + x) * 4..(y * 8 + x) * 4 + 4];
        assert_eq!(at(5, 1), &[255, 0, 0, 255]);
        // Sub-block 0 has c0 == c1 == 0: the three-colour mode, index 0 = black.
        assert_eq!(at(0, 0), &[0, 0, 0, 255]);
    }

    #[test]
    fn c8_reads_indices_through_the_palette_block() {
        let mut px = vec![0u8; 32];
        px[9] = 1; // x=1, y=1 in the first 8x4 tile
        let mut pal = vec![0u8; 512];
        pal[2..4].copy_from_slice(&0xfc00u16.to_be_bytes()); // entry 1: opaque red (RGB555)
        let s = parse(&sheet(C8, 8, 4, &px, &[(PALETTE, 256, 1, pal)])).unwrap();
        let t = &s.textures[0];
        assert_eq!(&t.rgba[(8 + 1) * 4..(8 + 1) * 4 + 4], &[255, 0, 0, 255]);
    }

    #[test]
    fn unknown_formats_are_reported_not_guessed() {
        let s = parse(&sheet(0x42, 4, 4, &[0; 64], &[])).unwrap();
        assert!(s.textures.is_empty());
        assert_eq!(s.skipped, vec![("text".to_owned(), 0x42)]);
    }
}
