//! The players' portraits: `/FE/XAPORTS.BIN`, the front end's sheet archive
//! 4 (`FUN_00210048(w, 4, sheet, 0, 0)`), one 128x128 picture a sheet. A
//! player record's field 14 names its sheet; a created baller's icon is
//! sheet 0xb0 + its field 14 (`FUN_00223d88`), and sheet 0x4d stands in
//! for a record with none (`FUN_00223698`).
//!
//! The archive is read once at startup and each sheet made into an image
//! the first time a screen draws it (the result boards, Choose Players'
//! grids, Update Team).

use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use street_assets::{locale, ssh};

use crate::setup::Disc;

/// The sheet a created baller's icon starts from (`FUN_00223d88`: icons
/// 0..6 are sheets 0xb0..0xb6).
pub const ICONS: usize = 0xb0;
/// The sheet a tile shows for a record without a portrait (`+0xc0` -1).
pub const NONE: usize = 0x4d;

#[derive(Resource, Default)]
pub struct Portraits {
    data: Vec<u8>,
    /// Each sheet's offset and size in `data`.
    entries: Vec<(usize, usize)>,
    made: HashMap<usize, Option<Handle<Image>>>,
}

impl Portraits {
    /// Reads the archive (nothing, without the disc's file).
    pub fn read(disc: &mut Disc) -> Self {
        let path = "/FE/XAPORTS.BIN";
        let Some(size) = disc.iso.stat(path).map(|s| s.size as usize) else {
            return Self::default();
        };
        let Ok(data) = disc.iso.read_range(path, 0, size) else {
            return Self::default();
        };
        let entries = locale::c0fb(&data)
            .map(|e| {
                e.iter()
                    .map(|e| (e.offset as usize, e.size as usize))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            data,
            entries,
            made: HashMap::new(),
        }
    }

    /// Sheet `index`'s picture: its image 0 cut to frame 0, the part
    /// `FUN_00210048(w, 4, sheet, 0, 0)` draws (75x84 from (26, 21), or
    /// 74x83 from the corner, of the 128x128), which the tiles stretch to
    /// their size; an HD pack's image in its place, cut at its scale.
    pub fn get(&mut self, images: &mut Assets<Image>, index: usize) -> Option<Handle<Image>> {
        if let Some(made) = self.made.get(&index) {
            return made.clone();
        }
        let made = self.entries.get(index).and_then(|&(at, size)| {
            let sheet = ssh::parse(self.data.get(at..at + size)?).ok()?;
            let base = sheet.textures.first()?;
            let t = street_render::hires::swap(base);
            let [x, y, w, h] = sheet
                .frames
                .first()
                .and_then(|f| f.first())
                .map(|f| f.bounds())
                .filter(|&[x, y, w, h]| {
                    x >= 0
                        && y >= 0
                        && w > 0
                        && h > 0
                        && (x + w) as u32 <= base.width
                        && (y + h) as u32 <= base.height
                })
                .unwrap_or([0, 0, base.width as i32, base.height as i32]);
            let scale = t.width as f32 / base.width as f32;
            let cut = |v: i32| (v as f32 * scale).round() as u32;
            let (x, y, w, h) = (cut(x), cut(y), cut(w).max(1), cut(h).max(1));
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for row in y..(y + h).min(t.height) {
                let from = ((row * t.width + x) * 4) as usize;
                let to = from + (w.min(t.width - x) * 4) as usize;
                rgba.extend_from_slice(t.rgba.get(from..to)?);
            }
            let h = (rgba.len() / (w as usize * 4)) as u32;
            Some(images.add(Image::new(
                Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                rgba,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            )))
        });
        self.made.insert(index, made.clone());
        made
    }
}

/// Startup: read the archive.
pub fn start(mut commands: Commands, mut disc: ResMut<Disc>) {
    commands.insert_resource(Portraits::read(&mut disc));
}
