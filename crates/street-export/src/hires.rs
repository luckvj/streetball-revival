//! The seam for HD texture packs (`--hires`).
//!
//! The exporter asks a [`Hires`] for every image it is about to embed; a
//! replacement (a larger picture of the same texture) goes into the glTF in
//! its place, and the UVs stay as they are. `street_assets::hires` (branch
//! `pe-hires`) reads a Dolphin pack for the GameCube Vol. 2 and maps it onto
//! the PS2 images; [`Pack`] adapts it here once that branch is merged.

use street_assets::ssh;

use crate::model::Image;

/// Which image the exporter is embedding.
#[derive(Clone, Copy, Debug)]
pub struct TextureKey<'a> {
    /// The character (`vol2:12`) or front-end item it is for.
    pub character: &'a str,
    /// The model's texture slot (`jers`, `head`, ...).
    pub slot: &'a str,
    /// The image's own name in its sheet.
    pub image: &'a str,
}

pub trait Hires: Sync {
    /// A replacement for `image`, or `None` to keep the disc's.
    fn replace(&self, key: &TextureKey, image: &ssh::Texture) -> Option<Image>;

    /// Whether any replacement can happen (for the catalog's `hires`).
    fn active(&self) -> bool {
        false
    }
}

/// No texture pack: every image is the disc's.
pub struct None_;

impl Hires for None_ {
    fn replace(&self, _: &TextureKey, _: &ssh::Texture) -> Option<Image> {
        None
    }
}

/// A Dolphin texture pack folder (`Load/Textures/GNZE69`), read by
/// `street_assets::hires`: a replacement is found from the PS2 image's own
/// pixels, so every embedded image the pack covers is swapped.
pub struct Pack {
    pub pack: street_assets::hires::Pack,
}

impl Pack {
    pub fn open(dir: &std::path::Path) -> Result<Self, String> {
        let pack = street_assets::hires::Pack::open(dir)
            .map_err(|e| format!("texture pack {}: {e}", dir.display()))?;
        Ok(Self { pack })
    }
}

impl Hires for Pack {
    fn replace(&self, _: &TextureKey, image: &ssh::Texture) -> Option<Image> {
        self.pack.replace(image).map(|t| Image {
            name: t.name,
            width: t.width,
            height: t.height,
            rgba: t.rgba,
        })
    }

    fn active(&self) -> bool {
        !self.pack.is_empty()
    }
}
