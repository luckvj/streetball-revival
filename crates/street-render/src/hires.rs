//! HD texture packs: a Dolphin pack for the GameCube Vol. 2 (`GNZE69`), such
//! as the Legendary Edition's, standing in for the PS2 images (see
//! [`street_assets::hires`]).
//!
//! Set one at startup with [`set_pack`]; from then on [`convert::image`]
//! (players, jerseys, shoes, logos, courts) gives the pack's image wherever
//! it has one, and [`swap`], [`join`] and [`picture`] serve images made
//! outside it (portraits, backdrops, loading screens).
//!
//! [`convert::image`]: crate::convert::image

use std::{
    borrow::Cow,
    path::Path,
    sync::{Arc, RwLock},
};

use street_assets::{hires::Pack, ssh};

static PACK: RwLock<Option<Arc<Pack>>> = RwLock::new(None);

/// Uses the pack in `dir` (searched recursively, e.g. the Legendary
/// Edition's `User/Textures/GNZE69`) for every image made from now on.
/// Returns the pack, so the caller can report how much of it is used.
pub fn set_pack(dir: impl AsRef<Path>) -> street_assets::Result<Arc<Pack>> {
    let pack = Arc::new(Pack::open(dir)?);
    *PACK.write().unwrap_or_else(|e| e.into_inner()) = Some(pack.clone());
    Ok(pack)
}

/// Back to the disc's own images.
pub fn clear_pack() {
    *PACK.write().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The pack in use, if any.
pub fn pack() -> Option<Arc<Pack>> {
    PACK.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `t`, or the pack's image for it.
pub fn swap(t: &ssh::Texture) -> Cow<'_, ssh::Texture> {
    match pack().and_then(|p| p.replace(t)) {
        Some(hd) => Cow::Owned(hd),
        None => Cow::Borrowed(t),
    }
}

/// A picture the PS2 cuts into `strips` laid left to right (the front
/// end's backdrops), `width` × `height` of the PS2's pixels from the top
/// left, at the pack's resolution; `None` unless the pack has every strip.
pub fn join(strips: &[&ssh::Texture], width: u32, height: u32) -> Option<ssh::Texture> {
    let pack = pack()?;
    let hd: Vec<ssh::Texture> = strips
        .iter()
        .map(|t| pack.replace(t))
        .collect::<Option<_>>()?;
    // The pack's texels per PS2 pixel, from the first strip.
    let scale = hd.first()?.height / strips.first()?.height.max(1);
    if scale == 0 {
        return None;
    }
    let (w, h) = (width * scale, height * scale);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut x0 = 0;
    for (t, s) in hd.iter().zip(strips) {
        // Each strip at the same scale (the pack's strips scale alike).
        let sw = s.width * scale;
        for y in 0..h.min(s.height * scale) {
            for x in 0..sw.min(w.saturating_sub(x0)) {
                let sx = (x * t.width / sw.max(1)) as usize;
                let sy = (y * t.height / (s.height * scale)) as usize;
                let from = (sy * t.width as usize + sx) * 4;
                let to = ((y * w + x0 + x) * 4) as usize;
                rgba[to..to + 4].copy_from_slice(&t.rgba[from..from + 4]);
            }
        }
        x0 += sw;
    }
    Some(ssh::Texture {
        name: strips.first()?.name.clone(),
        width: w,
        height: h,
        rgba,
        indices: None,
    })
}

/// The pack's whole loading screen for `/FE/XAFELOAD.BIN` entry `entry`,
/// given the PS2's (`original`, 640x512): at the pack's resolution, unless
/// the pack's is only a smaller copy of the same picture.
pub fn picture(entry: usize, original: &ssh::Texture) -> Option<ssh::Texture> {
    pack()?.picture_over(entry, original)
}
