//! The front end's art: the `/FE/*` archives. The five plain `.SSH` files
//! (BALL, EXPLODE, HUD, HUDFR, OFFIND, REPLAY) are SHPS sheets; the `X*`
//! binaries are `c0 fb` archives whose entries are SSH sheets (the magic and
//! counts check out per entry; `docs/TRACE.md` §4: `XAFESCRN` carries 3
//! sheets, `XAFEBG` 105 backgrounds, `XATEAMS` the 50 team logos). The
//! loader reads them through 0x2e4478 (the background caches) and 0x2e4f68
//! (xateams/xafescrn/xacourts/xaports; `xafebg`'s cache picks by mode).
//!
//! `XAFELOAD.BIN` is instead a BIG archive (one entry per loading
//! screen); the fonts are `FNTS` (see `docs/TRACE.md` §4) and stay theirs.

use std::io::{Read, Seek};

use crate::{Error, Result, iso::Iso, locale, ssh};

/// Every SSH sheet of a `c0 fb` front-end archive, in entry order.
pub fn sheets(data: &[u8]) -> Result<Vec<ssh::Sheet>> {
    let entries = locale::c0fb(data)?;
    entries
        .iter()
        .map(|e| {
            let at = e.offset as usize;
            ssh::parse(
                data.get(at..at + e.size as usize)
                    .ok_or_else(|| Error::format(format!("sheet {e:?} outside the archive")))?,
            )
        })
        .collect()
}

/// The picture's size inside a backdrop's strips (shown doubled on the
/// 640x480 screen).
pub const BACKDROP_WIDTH: usize = 320;
pub const BACKDROP_HEIGHT: usize = 240;

/// `XAFEBG.BIN`'s backdrops: backdrop `n` is entries `3n..3n + 2` laid side
/// by side (strips 128, 128 and 64 wide), cropped to the top-left
/// [`BACKDROP_WIDTH`] x [`BACKDROP_HEIGHT`].
pub fn backdrops(sheets: &[ssh::Sheet]) -> Vec<ssh::Texture> {
    let (w_out, h_out) = (BACKDROP_WIDTH, BACKDROP_HEIGHT);
    let strips: Vec<Option<&ssh::Texture>> = sheets.iter().map(|s| s.textures.first()).collect();
    strips
        .chunks(3)
        .map(|three| {
            let mut out = vec![0u8; w_out * h_out * 4];
            let mut x = 0usize;
            for t in three.iter().flatten() {
                if x >= w_out {
                    break;
                }
                let w = (t.width as usize).min(w_out - x);
                for y in 0..(t.height as usize).min(h_out) {
                    let src = y * t.width as usize * 4;
                    let dst = (y * w_out + x) * 4;
                    out[dst..dst + w * 4].copy_from_slice(&t.rgba[src..src + w * 4]);
                }
                x += t.width as usize;
            }
            ssh::Texture {
                name: "backdrop".into(),
                width: w_out as u32,
                height: h_out as u32,
                rgba: out,
                indices: None,
            }
        })
        .collect()
}

/// One archive from the disc.
pub fn read<R: Read + Seek>(iso: &mut Iso<R>, path: &str) -> Result<Vec<ssh::Sheet>> {
    let size = iso
        .stat(path)
        .ok_or_else(|| Error::NotFound(path.to_owned()))?
        .size as usize;
    let data = iso.read_range(path, 0, size)?;
    sheets(&data)
}

/// A lone `.SSH` file (the HUD, the ball, the replay overlay art).
pub fn sheet<R: Read + Seek>(iso: &mut Iso<R>, path: &str) -> Result<ssh::Sheet> {
    let size = iso
        .stat(path)
        .ok_or_else(|| Error::NotFound(path.to_owned()))?
        .size as usize;
    let data = iso.read_range(path, 0, size)?;
    ssh::parse(&data)
}
